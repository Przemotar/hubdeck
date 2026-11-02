import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { DeckEvent, LiveState, ModuleInfo, ModuleKind } from "../deck";
import { api, DISCORD } from "../engine";
import { told } from "../confirm";
import { osName, useAppVersion } from "../update";
import { t, useLanguage, type Key } from "../i18n";
import { Icon } from "./Icon";
import { ModuleThumb } from "./DeckView";

/**
 * Settings > Diagnostics: check one module (owner, 2026-11-02: "panel do debugowania, w którym można
 * zaznaczyć wybrany moduł i zdebugować czy działa, czy jest ok i wychwycić ewentualnie jakieś błędy"),
 * after a faders user asked whether HubDeck or the module made one light stay red.
 *
 * A lights or screen test (the module is held: the deck's own pictures and lights wait until it ends),
 * live input, every known read of the module, and what happened to it since the start; one report to
 * paste on the Discord or GitHub. Nothing is written to the module's memory.
 */

const REPO = "https://github.com/Przemotar/hubdeck";
const inTauri = () => "__TAURI_INTERNALS__" in window;

interface Read {
  name: string;
  id: number;
  hex: string | null;
  text: string | null;
  error: string | null;
  ms: number;
}
interface Note {
  at: number;
  module: string;
  what: string;
}
interface Notes {
  notes: Note[];
  counts: { module: string; kind: string; count: number }[];
}

type Rgb = [number, number, number];
const COLORS: { key: Key; rgb: Rgb }[] = [
  { key: "diag.red", rgb: [255, 0, 0] },
  { key: "diag.green", rgb: [0, 255, 0] },
  { key: "diag.blue", rgb: [0, 0, 255] },
  { key: "diag.white", rgb: [255, 255, 255] },
  { key: "diag.off", rgb: [0, 0, 0] },
];
/** Lights of each kind with LEDs (the base's edge is one, addressed as one). */
const LIGHTS: Partial<Record<ModuleKind, number>> = { faders: 5, knobs: 3, base: 1 };
const CONTROLS: Partial<Record<ModuleKind, number>> = { faders: 5, knobs: 3, rollers: 2, keys: 15 };
const STEP_MS = 1500;
/** The hold ends by itself after 45 s unrenewed (module_check.rs). */
const RENEW_MS = 15000;
/** Problems in the last hour turn a module's dot amber. */
const HOUR = 3600_000;
const PROBLEMS = ["gone", "not answering", "picture failed", "lights failed"];

interface Step {
  label: string;
  /** One colour per light (lights), or the screen's colour. */
  colors: Rgb[];
  /** The light lit alone in this step (0-based), for the swatches. */
  alone?: number;
}

function stepsFor(kind: ModuleKind): Step[] {
  const n = LIGHTS[kind];
  if (n) {
    const all = COLORS.map((c) => ({ label: t(c.key), colors: Array.from({ length: n }, () => c.rgb) }));
    // then each light alone in white: which one is which, and one that never goes out shows
    const alone =
      n > 1
        ? Array.from({ length: n }, (_, i) => ({
            label: t("diag.alone", { n: i + 1 }),
            colors: Array.from({ length: n }, (_, j) => (j === i ? ([255, 255, 255] as Rgb) : ([0, 0, 0] as Rgb))),
            alone: i,
          }))
        : [];
    return [...all, ...alone];
  }
  if (kind === "keys" || kind === "dial") return COLORS.map((c) => ({ label: t(c.key), colors: [c.rgb] }));
  return [];
}

async function solidJpeg(w: number, h: number, rgb: Rgb): Promise<number[]> {
  const c = document.createElement("canvas");
  c.width = w;
  c.height = h;
  const g = c.getContext("2d")!;
  g.fillStyle = `rgb(${rgb.join(",")})`;
  g.fillRect(0, 0, w, h);
  const blob = await new Promise<Blob>((resolve, reject) => c.toBlob((b) => (b ? resolve(b) : reject(new Error("toBlob failed"))), "image/jpeg", 0.92));
  return Array.from(new Uint8Array(await blob.arrayBuffer()));
}

async function show(m: ModuleInfo, step: Step) {
  if (!inTauri()) return;
  if (m.kind === "keys") await invoke("draw_jpeg", { module: m.id, screen: "panel", index: 0, jpeg: await solidJpeg(480, 272, step.colors[0]) });
  else if (m.kind === "dial") await invoke("draw_jpeg", { module: m.id, screen: "dial", index: 0, jpeg: await solidJpeg(240, 240, step.colors[0]) });
  else await invoke("set_leds", { module: m.id, colors: step.colors });
}

/** LED modes as the firmware numbers them (PROTOCOL.md, watched on the deck 2026-10-11). */
const LED_MODES: Record<number, string> = { 1: "static", 2: "breathing", 3: "strobe", 4: "blinking", 5: "cycling", 6: "random", 7: "wave", 8: "water", 9: "rainbow" };

/** A read in words where its meaning is known; the raw bytes otherwise (and always in the report). */
function decoded(r: Read): string {
  if (r.error) return r.error;
  if (r.text) return r.text;
  const bytes = (r.hex ?? "").split(" ").filter(Boolean).map((b) => parseInt(b, 16));
  const b0 = bytes[0];
  if (b0 !== undefined) {
    // one value per light (the faders: five); said once when they are all the same
    const same = bytes.every((b) => b === b0);
    if ([0x32, 0x3c, 0x4c].includes(r.id)) return same ? `${b0} %` : `${bytes.join(" ")} %`;
    if (r.id === 0x34) return same ? `${b0}${LED_MODES[b0] ? ` (${LED_MODES[b0]})` : ""}` : bytes.map((b) => `${b}${LED_MODES[b] ? ` ${LED_MODES[b]}` : ""}`).join(", ");
    if (r.id === 0x33) return bytes.slice(0, 5).join("  ");
    if (r.id === 0x3b || r.id === 0x4b) return bytes.length >= 8 ? `${bytes[4] | (bytes[5] << 8)}×${bytes[6] | (bytes[7] << 8)}` : r.hex ?? "";
  }
  return r.hex || "—";
}

const hold = (module: string | null) => (inTauri() ? invoke("diag_hold", { module }).catch(() => {}) : Promise.resolve());

const css = (rgb: Rgb) => `rgb(${rgb.join(",")})`;
const clock = (ms: number) => new Date(ms).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
const moduleName = (kind: ModuleKind) => t(`module.${kind}` as Key);

type Answer = { ok: true } | { ok: false; marked: number[]; note: string };

/** What was seen from the module's controls while the panel was open. */
interface Seen {
  moved: Set<number>;
  pressed: Set<number>;
  values: Record<number, number>;
  left: boolean;
  right: boolean;
  orientations: Set<string>;
}
const noneSeen = (): Seen => ({ moved: new Set(), pressed: new Set(), values: {}, left: false, right: false, orientations: new Set() });

export function DiagnosticsSettings({ live }: { live: LiveState }) {
  useLanguage();
  const version = useAppVersion();
  const modules = [...live.deck.modules].sort((a, b) => a.kind.localeCompare(b.kind) || (a.slot ?? "").localeCompare(b.slot ?? ""));
  const [picked, setPicked] = useState<string | null>(null);
  const m = modules.find((x) => x.id === picked) ?? modules[0];
  const [notes, setNotes] = useState<Notes>({ notes: [], counts: [] });

  useEffect(() => {
    if (!inTauri()) return;
    const load = () => invoke<Notes>("diag_notes").then(setNotes, () => {});
    load();
    const timer = setInterval(load, 5000);
    return () => clearInterval(timer);
  }, []);

  const state = (x: ModuleInfo): "bad" | "warn" | "ok" => {
    if (live.deck.unresponsive.includes(x.kind)) return "bad";
    const since = Date.now() - HOUR;
    return notes.notes.some((n) => n.module === x.id && n.at >= since && PROBLEMS.some((p) => n.what.startsWith(p))) ? "warn" : "ok";
  };

  if (!m)
    return (
      <section className="sec">
        <h3 className="sec-title">{t("diag.title")}</h3>
        <p className="help">{t("diag.none")}</p>
      </section>
    );

  return (
    <section className="sec diag">
      <h3 className="sec-title">{t("diag.title")}</h3>
      <p className="help">{t("diag.help")}</p>
      <div className="diag-body">
        <nav className="diag-list" aria-label={t("diag.modules")}>
          {modules.map((x) => {
            const s = state(x);
            return (
              <button key={x.id} className={`diag-mod${x.id === m.id ? " is-active" : ""}`} aria-current={x.id === m.id ? "true" : undefined} onClick={() => setPicked(x.id)}>
                <span className="diag-thumb">{x.kind === "base" ? <span className="diag-base" /> : <ModuleThumb kind={x.kind} />}</span>
                <span className="diag-mod-text">
                  <b>{moduleName(x.kind)}</b>
                  <small className={`diag-state is-${s}`}>{t(s === "bad" ? "diag.notAnswering" : s === "warn" ? "diag.hadProblems" : "diag.ok")}</small>
                </span>
              </button>
            );
          })}
        </nav>
        <ModuleCheck key={m.id} m={m} live={live} notes={notes} version={version} unresponsive={state(m) === "bad"} />
      </div>
    </section>
  );
}

function ModuleCheck({ m, live, notes, version, unresponsive }: { m: ModuleInfo; live: LiveState; notes: Notes; version: string | null; unresponsive: boolean }) {
  const steps = stepsFor(m.kind);
  const [step, setStep] = useState<number | null>(null);
  const [answer, setAnswer] = useState<Answer | null>(null);
  const [asking, setAsking] = useState(false);
  const [marked, setMarked] = useState<number[]>([]);
  const [note, setNote] = useState("");
  const [reads, setReads] = useState<Read[] | null>(null);
  const [readError, setReadError] = useState<string | null>(null);
  const [seen, setSeen] = useState<Seen>(noneSeen);
  const [copied, setCopied] = useState(false);
  const running = useRef(false);

  const read = useCallback(() => {
    setReads(null);
    setReadError(null);
    if (!inTauri()) return;
    invoke<Read[]>("diag_read", { module: m.id }).then(setReads, (e) => setReadError(String(e)));
  }, [m.id]);
  useEffect(read, [read]);

  // live input of this module
  useEffect(() => {
    if (!inTauri()) return;
    const first = m.first ?? 0;
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    listen<DeckEvent>("deck://event", (e) => {
      const ev = e.payload;
      if (ev.type === "snapshot") {
        if (m.kind === "base" && ev.deck.orientation) setSeen((s) => ({ ...s, orientations: new Set(s.orientations).add(ev.deck.orientation!) }));
        return;
      }
      if (ev.module !== m.id) return;
      setSeen((s) => {
        const n = { ...s, moved: new Set(s.moved), pressed: new Set(s.pressed), values: { ...s.values } };
        if (ev.type === "fader") {
          n.moved.add(ev.index - first);
          n.values[ev.index - first] = ev.value;
        } else if (ev.type === "knob" || ev.type === "roller") n.moved.add(ev.index - first);
        else if (ev.type === "keyDown" || ev.type === "knobDown") n.pressed.add(ev.index - (ev.type === "knobDown" ? first : 0));
        else if (ev.type === "dial") {
          if (ev.delta > 0) n.right = true;
          else n.left = true;
        } else if (ev.type === "dialDown") n.pressed.add(0);
        return n;
      });
    }).then((u) => (cancelled ? u() : (unlisten = u)));
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [m.id, m.kind, m.first]);

  // a test left half-way (another module picked, Settings closed) lets the module go
  useEffect(
    () => () => {
      if (running.current) {
        running.current = false;
        void hold(null);
      }
    },
    [],
  );

  const runTest = async () => {
    running.current = true;
    setAnswer(null);
    setAsking(false);
    setMarked([]);
    setNote("");
    await hold(m.id);
    const renew = setInterval(() => void hold(m.id), RENEW_MS);
    try {
      for (let i = 0; i < steps.length && running.current; i++) {
        setStep(i);
        await show(m, steps[i]).catch(() => {});
        await new Promise((r) => setTimeout(r, STEP_MS));
      }
    } finally {
      clearInterval(renew);
      const finished = running.current;
      running.current = false;
      setStep(null);
      await hold(null);
      if (finished) setAsking(true);
    }
  };

  const lights = LIGHTS[m.kind] ?? 0;
  const markable = m.kind === "keys" ? 15 : lights > 1 ? lights : 0;
  const current = step === null ? null : steps[step];
  const controls = CONTROLS[m.kind] ?? 0;
  const myNotes = notes.notes.filter((n) => n.module === m.id);
  const lastHour = myNotes.filter((n) => n.at >= Date.now() - HOUR);
  const counts = notes.counts.filter((c) => c.module === m.id && c.kind !== "attached");

  const report = () => {
    const lines = [`HubDeck ${version ?? "?"}, ${osName()}`, `Module: ${m.kind}${m.slot ? `, slot ${m.slot}` : ""}${m.firmware ? `, firmware ${m.firmware}` : ""}${unresponsive ? ", NOT ANSWERING" : ""}`];
    if (steps.length)
      lines.push(
        `Test (${m.kind === "keys" || m.kind === "dial" ? "screen" : "lights"}): ${
          !answer ? "not run" : answer.ok ? "everything followed every colour" : `did not follow: ${answer.marked.length ? answer.marked.map((i) => i + 1).join(", ") : "-"}${answer.note ? ` (${answer.note})` : ""}`
        }`,
      );
    if (controls) lines.push(`Input seen: moved ${[...seen.moved].sort().map((i) => i + 1).join(" ") || "-"}; pressed ${[...seen.pressed].sort().map((i) => i + 1).join(" ") || "-"}`);
    if (m.kind === "dial") lines.push(`Input seen: left ${seen.left ? "yes" : "no"}, right ${seen.right ? "yes" : "no"}, press ${seen.pressed.has(0) ? "yes" : "no"}`);
    lines.push("Reads:");
    for (const r of reads ?? []) lines.push(`  ${r.name} (0x${r.id.toString(16)}): ${r.error ? `ERROR ${r.error}` : `${r.text ? `"${r.text}" ` : ""}[${r.hex}]`} ${r.ms.toFixed(1)} ms`);
    if (!reads) lines.push(`  ${readError ?? "not read"}`);
    lines.push(`Since start: ${counts.length ? counts.map((c) => `${c.kind} x${c.count}`).join(", ") : "no problems"}`);
    for (const n of myNotes.slice(-12)) lines.push(`  ${clock(n.at)} ${n.what}`);
    // no serial number in a report pasted in public, not even inside an error ("faders:MH… is not connected")
    return m.serial ? lines.join("\n").split(m.serial).join("…") : lines.join("\n");
  };
  const copy = () => navigator.clipboard.writeText(report()).then(() => setCopied(true), () => {});
  const onDiscord = () =>
    navigator.clipboard
      .writeText(report())
      .catch(() => {})
      .then(() => {
        api.openLink(DISCORD).catch(() => {});
        void told(t("help.discordReportCopied"), t("diag.discordText"));
      });
  const onGitHub = () => {
    const field = `&version=${encodeURIComponent(`HubDeck ${version ?? "?"}, ${osName()}`)}&diagnostics=${encodeURIComponent(report())}`;
    api.openLink(`${REPO}/issues/new?template=bug.yml${field}`).catch(() => {});
  };

  return (
    <div className="diag-check">
      <div className="diag-head">
        <b>{moduleName(m.kind)}</b>
        <span className="diag-meta">
          {[m.slot && t("diag.slot", { slot: m.slot }), m.firmware && t("diag.firmware", { v: m.firmware })].filter(Boolean).join(" · ")}
        </span>
      </div>

      {unresponsive && (
        <div className="diag-block diag-alert">
          <p>{t("diag.notAnsweringHelp")}</p>
          <button className="tool" onClick={() => invoke("module_restart", { kind: m.kind }).catch(() => {})}>
            <Icon name="refresh-cw" />
            {t("diag.restart")}
          </button>
        </div>
      )}

      {steps.length > 0 && (
        <div className="diag-block">
          <div className="diag-row">
            <h4>{t(m.kind === "keys" || m.kind === "dial" ? "diag.screenTest" : "diag.lightsTest")}</h4>
            <button className="tool primary" disabled={step !== null || unresponsive} onClick={() => void runTest()}>
              <Icon name="play" />
              {step !== null ? t("diag.testing") : answer || asking ? t("diag.again") : t("diag.run")}
            </button>
          </div>
          <p className="help">{t(m.kind === "keys" || m.kind === "dial" ? "diag.screenHelp" : "diag.lightsHelp")}</p>
          {(lights > 0 || current) && (
            <div className="diag-swatches" aria-live="polite">
              {lights > 0 &&
                Array.from({ length: lights }, (_, i) => (
                  <span key={i} className="diag-swatch">
                    <i style={{ background: current ? css(current.colors[i] ?? current.colors[0]) : "var(--bg3)" }} />
                    {lights > 1 && <small>{i + 1}</small>}
                  </span>
                ))}
              {lights === 0 && current && (
                <span className="diag-swatch is-screen">
                  <i style={{ background: css(current.colors[0]) }} />
                </span>
              )}
              {current && <span className="diag-now">{t("diag.now", { what: current.label })}</span>}
            </div>
          )}
          {asking && !answer && (
            <div className="diag-ask">
              <p>{t(m.kind === "keys" || m.kind === "dial" ? "diag.askScreen" : "diag.askLights")}</p>
              <div className="row wrap">
                <button className="tool" onClick={() => setAnswer({ ok: true })}>
                  <Icon name="check" />
                  {t("diag.yesAll")}
                </button>
              </div>
              {markable > 0 && <p className="help">{t(m.kind === "keys" ? "diag.markKeys" : "diag.markLights")}</p>}
              {markable > 0 && (
                <div className="diag-marks">
                  {Array.from({ length: markable }, (_, i) => (
                    <button key={i} className={`chip${marked.includes(i) ? " is-on" : ""}`} aria-pressed={marked.includes(i)} onClick={() => setMarked((x) => (x.includes(i) ? x.filter((y) => y !== i) : [...x, i]))}>
                      {i + 1}
                    </button>
                  ))}
                </div>
              )}
              <input className="input" value={note} placeholder={t("diag.notePlaceholder")} onChange={(e) => setNote(e.target.value)} />
              <div className="row wrap">
                <button className="tool" onClick={() => setAnswer({ ok: false, marked: [...marked].sort((a, b) => a - b), note: note.trim() })}>
                  <Icon name="alert" />
                  {t("diag.notAll")}
                </button>
              </div>
            </div>
          )}
          {answer && (
            <p className={`diag-verdict ${answer.ok ? "is-ok" : "is-bad"}`}>
              {answer.ok ? t("diag.verdictOk") : t("diag.verdictBad")}
            </p>
          )}
        </div>
      )}

      <div className="diag-block">
        <h4>{t("diag.input")}</h4>
        <p className="help">{t(`diag.inputHelp.${m.kind}` as Key)}</p>
        {controls > 0 && (
          <div className={`diag-controls is-${m.kind}`}>
            {Array.from({ length: controls }, (_, i) => {
              const ok = seen.moved.has(i) || seen.pressed.has(i);
              return (
                <span key={i} className={`diag-control${ok ? " is-seen" : ""}`} title={String(i + 1)}>
                  {m.kind === "faders" && <span className="diag-level" style={{ height: `${seen.values[i] ?? live.faders[`${m.id}#${i + (m.first ?? 0)}`] ?? 0}%` }} />}
                  <small>{i + 1}</small>
                  {m.kind === "faders" && seen.values[i] !== undefined && <em>{seen.values[i]}</em>}
                  {m.kind === "knobs" && (
                    <em>
                      {seen.moved.has(i) ? "↻" : "·"} {seen.pressed.has(i) ? "●" : "·"}
                    </em>
                  )}
                </span>
              );
            })}
          </div>
        )}
        {m.kind === "dial" && (
          <div className="row wrap">
            <span className={`pill ${seen.left ? "is-ok" : ""}`}>{t("diag.dialLeft")}</span>
            <span className={`pill ${seen.right ? "is-ok" : ""}`}>{t("diag.dialRight")}</span>
            <span className={`pill ${seen.pressed.has(0) ? "is-ok" : ""}`}>{t("diag.dialPress")}</span>
          </div>
        )}
        {m.kind === "base" && (
          <div className="row wrap">
            <span className="pill">{t("diag.orientation", { o: live.deck.orientation ? t(`diag.orientation.${live.deck.orientation}` as Key) : "?" })}</span>
            {seen.orientations.size > 1 && <span className="pill is-ok">{t("diag.sensorWorks")}</span>}
          </div>
        )}
      </div>

      <div className="diag-block">
        <div className="diag-row">
          <h4>{t("diag.reads")}</h4>
          <button className="tool ghost" onClick={read}>
            <Icon name="refresh-cw" />
            {t("diag.readAgain")}
          </button>
        </div>
        {readError && <p className="help is-error">{readError}</p>}
        {!reads && !readError && <p className="help">{inTauri() ? t("diag.reading") : "-"}</p>}
        {reads && (
          <table className="diag-reads">
            <tbody>
              {reads.map((r) => (
                <tr key={r.id} className={r.error ? "is-error" : undefined}>
                  <th>{t(`diag.read.${r.name.replace(/ /g, "-").toLowerCase()}` as Key)}</th>
                  <td title={r.hex ?? undefined}>{decoded(r)}</td>
                  <td className="diag-ms">{r.ms.toFixed(1)} ms</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      <div className="diag-block">
        <h4>{t("diag.happened")}</h4>
        {lastHour.length === 0 ? (
          <p className="help">{t("diag.nothingHappened")}</p>
        ) : (
          <ul className="diag-notes">
            {lastHour.slice(-8).map((n, i) => (
              <li key={i} className={PROBLEMS.some((p) => n.what.startsWith(p)) ? "is-problem" : undefined}>
                <time>{clock(n.at)}</time> {n.what}
              </li>
            ))}
          </ul>
        )}
      </div>

      <div className="diag-actions row wrap">
        <button className="tool" onClick={copy}>
          <Icon name="copy" />
          {copied ? t("diag.copied") : t("diag.copy")}
        </button>
        <button className="tool" onClick={onDiscord}>
          <Icon name="users" />
          {t("help.discordReport")}
        </button>
        <button className="tool ghost" onClick={onGitHub}>
          <Icon name="alert" />
          {t("diag.github")}
        </button>
      </div>
    </div>
  );
}
