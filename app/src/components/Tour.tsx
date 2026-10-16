// The tour (owner, 2026-10-13: show, then ask, then build; docs/ASSISTANT-TOUR.md): one question
// about what the person does at the computer (HubDeck suggests the answer), then the deck is made
// at once and shown on the hardware, and the tour walks through it part by part (layout 2A: the
// parts on the left, the part in the middle, the deck on the right). Each part is kept or left out
// and has up to two looks; the finale shows HubDeck's showpieces; the end keeps the deck or throws
// it away. Nothing of the user's changes; Exit puts everything back.
import { listen } from "@tauri-apps/api/event";
import { useEffect, useMemo, useRef, useState } from "react";

import type { ControlRef, LiveState } from "../deck";
import { api, inWindow, type DialFace, type EngineView, type TourFocus, type TourStop, type TourSuggestion, type TourView } from "../engine";
import { getLanguage, localizeAction, t, useLanguage, type Key } from "../i18n";
import type { Placement } from "../layout";
import { DeckView } from "./DeckView";
import { Icon } from "./Icon";

const FOCUSES: TourFocus[] = ["games", "streaming", "calls", "music", "mixed"];
const FOCUS_ICON: Record<TourFocus, string> = { games: "gamepad-2", streaming: "radio", calls: "video", music: "music", mixed: "layout-grid" };

/** The control a part's "Try it" is about, on the modules attached now. */
function refOf(live: LiveState, control: string | null): ControlRef | null {
  if (!control) return null;
  const [kind, index] = control === "dial" ? ["dial", "0"] : control.split(".");
  const m = live.deck.modules.find((x) => x.kind === kind);
  return m ? ({ module: m.id, kind: m.kind, index: Number(index) } as ControlRef) : null;
}

const same = (a: ControlRef | null, b: ControlRef | null) => !!a && !!b && a.kind === b.kind && a.index === b.index;

/** The "Try it" words of a part, by the kit the deck has. */
function tryWords(stop: TourStop, view: TourView): string {
  if (stop.id === "volume") {
    const c = stop.tryControl ?? "";
    return t(c.startsWith("faders") ? "tour.volume.tryFaders" : c.startsWith("knobs") ? "tour.volume.tryKnobs" : c === "dial" ? "tour.volume.tryDial" : "tour.volume.tryKeys");
  }
  if (stop.id === "music") return t(stop.tryControl === "dial" ? "tour.music.tryDial" : "tour.music.tryKey");
  if (stop.id === "mute" && view.call) return t("tour.mute.tryCall", { app: view.call === "discord" ? "Discord" : view.call === "teams" ? "Teams" : "Zoom" });
  return t(`tour.${stop.id}.try` as Key);
}

/** `deskCheck`: the first start (owner's pick 1a, 2026-10-14: straight into the tour) begins with
 * the deck drawing and "Does this look like your desk?"; Fix it goes to Edit layout (`onArrange`)
 * and the page comes back here after it. Skip, Exit and the end mark the first start done. */
export function Tour({
  live,
  view,
  saved,
  onPlace,
  onClose,
  onBuild,
  onAsk,
  deskCheck = false,
  onArrange,
}: {
  live: LiveState;
  view: EngineView;
  saved: Record<string, Placement>;
  onPlace: (key: string, p: Placement | null) => void;
  /** `home`: the deck was kept as the home deck ("Use as my home deck"). */
  onClose: (home?: boolean) => void;
  onBuild: () => void;
  onAsk: (message: string) => void;
  deskCheck?: boolean;
  onArrange?: () => void;
}) {
  useLanguage();
  const [phase, setPhase] = useState<"desk" | "focus" | "making" | "stops" | "finale" | "end">(deskCheck ? "desk" : "focus");
  const [suggestion, setSuggestion] = useState<TourSuggestion | null>(null);
  const [focus, setFocus] = useState<TourFocus>("mixed");
  const [tour, setTour] = useState<TourView | null>(null);
  const [at, setAt] = useState(0);
  const [tried, setTried] = useState<Record<string, boolean>>({});
  const [error, setError] = useState<string | null>(null);
  const [ask, setAsk] = useState("");
  const done = useRef(false);
  // the next-day card ("knob 3 never turned, swap it?") needs counted presses: the user's yes
  const [counting, setCounting] = useState(false);
  useEffect(() => {
    if (phase === "end") api.pressesCounting().then((on) => setCounting(!!on), () => {});
  }, [phase]);

  useEffect(() => {
    api.tourSuggest(getLanguage()).then(
      (s) => (setSuggestion(s), setFocus(s.focus)),
      () => {},
    );
    // leaving the page any other way than the end puts everything back
    return () => {
      if (!done.current) api.tourExit().catch(() => {});
    };
  }, []);

  const main = useMemo(() => tour?.stops.filter((s) => !s.id.startsWith("super")) ?? [], [tour]);
  const finale = useMemo(() => tour?.stops.filter((s) => s.id.startsWith("super")) ?? [], [tour]);
  const stop = phase === "stops" ? main[Math.min(at, main.length - 1)] : null;

  // the volume part plays HubDeck's own sound, so turning is heard even when nothing plays
  useEffect(() => {
    if (stop?.id !== "volume") return;
    api.tourSound(true).catch(() => {});
    return () => void api.tourSound(false).catch(() => {});
  }, [stop?.id]);

  // "One control, many jobs": the holds and double presses done on the deck (the engine says)
  const [gestured, setGestured] = useState<Record<string, boolean>>({});
  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    const off = listen<string>("tour://gesture", (e) => setGestured((g) => ({ ...g, [e.payload]: true })));
    return () => void off.then((f) => f());
  }, []);
  const nextTask = stop?.id === "gestures" ? (stop.tasks ?? []).find((task) => !gestured[task]) : undefined;

  // "Your dial, many faces": the dial's looks and the gestures done on the one shown (a new look starts over)
  const [gallery, setGallery] = useState<DialFace[]>([]);
  useEffect(() => {
    if (stop?.id === "dial" && gallery.length === 0) api.dialGallery().then(setGallery, () => {});
  }, [stop?.id, gallery.length]);
  const [faceTicked, setFaceTicked] = useState<Record<string, boolean>>({});
  useEffect(() => setFaceTicked({}), [stop?.id, stop?.look]);
  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window) && !("demoEmit" in window)) return;
    const off = listen<string>("tour://gesture", (e) => {
      const kind = e.payload.startsWith("dial:") ? e.payload.slice(5) : "";
      if (kind) setFaceTicked((f) => (f[kind] ? f : { ...f, [kind]: true }));
    });
    return () => void off.then((f) => f());
  }, []);
  const onFace = stop?.id === "dial" ? gallery.find((f) => f.id === (stop.faces ?? [])[stop.look]) : undefined;
  const faceNext = onFace?.gestures.find((g) => !faceTicked[g.kind])?.kind;
  useEffect(() => {
    if (onFace && onFace.gestures.length > 0 && !faceNext) setTried((x) => ({ ...x, dial: true }));
  }, [onFace, faceNext]);

  // a little party for every task done (owner's pick C1): a "Done!" bubble and confetti
  const [cheer, setCheer] = useState<{ key: number; text: string } | null>(null);
  const celebrate = (text: string) => setCheer({ key: Date.now(), text });
  useEffect(() => {
    if (!cheer) return;
    const id = setTimeout(() => setCheer(null), 1700);
    return () => clearTimeout(id);
  }, [cheer]);
  const gestureCount = Object.keys(gestured).filter((g) => (stop?.tasks ?? []).includes(g)).length;
  const faceCount = Object.keys(faceTicked).length;
  const prevCounts = useRef({ gestures: 0, face: 0, tried: 0 });
  const triedCount = Object.values(tried).filter(Boolean).length;
  useEffect(() => {
    const p = prevCounts.current;
    if (stop?.id === "gestures" && gestureCount > p.gestures) celebrate(t("tour.cheer.count", { n: gestureCount, count: (stop.tasks ?? []).length }));
    else if (stop?.id === "dial" && faceCount > p.face && onFace) celebrate(t("tour.cheer.count", { n: faceCount, count: onFace.gestures.length }));
    else if (triedCount > p.tried) celebrate(t("tour.cheer.done"));
    prevCounts.current = { gestures: gestureCount, face: faceCount, tried: triedCount };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [gestureCount, faceCount, triedCount]);
  useEffect(() => {
    if (stop?.id === "gestures" && (stop.tasks ?? []).length > 0 && !nextTask) setTried((x) => ({ ...x, gestures: true }));
  }, [stop, nextTask]);

  // the control to try blinks on the hardware too, wherever its module sits (until it is touched)
  const pointAt =
    stop?.id === "gestures"
      ? (nextTask?.split(":")[0] ?? null)
      : (stop?.tryControl ?? (phase === "finale" && tour?.stops.some((s) => s.id === "superAnimations" && s.keep) ? "keys.9" : null));
  // the drawing marks the same control the deck blinks: for the gestures the one to do now, not
  // the first (owner, 2026-10-14: it said the System knob and marked the Mic key)
  const tryRef = refOf(live, pointAt);
  useEffect(() => {
    if (!pointAt) return;
    api.tourHighlight(pointAt).catch(() => {});
    return () => void api.tourHighlight(null).catch(() => {});
  }, [pointAt]);

  // the live check: the control of "Try it" was touched on the deck
  useEffect(() => {
    if (stop && stop.id !== "gestures" && same(live.lastTouched, tryRef)) setTried((x) => ({ ...x, [stop.id]: true }));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [live.lastTouched]);

  const start = () => {
    setPhase("making");
    setError(null);
    api.tourStart(focus, getLanguage()).then(
      (v) => (setTour(v), setAt(0), setPhase("stops")),
      (e) => (setError(String(e)), setPhase("focus")),
    );
  };
  // the button shows the choice at once; the deck follows when HubDeck has made it
  const set = (id: string, keep: boolean | null, look: number | null) => {
    setTour((v) => v && { ...v, stops: v.stops.map((s) => (s.id === id ? { ...s, keep: keep ?? s.keep, look: look ?? s.look } : s)) });
    return api.tourSet(id, keep, look).then(setTour, (e) => setError(String(e)));
  };
  // the first start is over once the tour ends, whichever way
  const onboarded = () => {
    if (view.state && !view.state.onboarded) api.onboardingDone().catch(() => {});
  };
  const exit = () => {
    done.current = true;
    api.tourExit().finally(() => (onboarded(), onClose()));
  };
  const finish = (keep: boolean, home: boolean, then: () => void) => {
    done.current = true;
    api.tourFinish(keep, home).then(() => (onboarded(), then()), (e) => setError(String(e)));
  };
  const next = () => (at + 1 < main.length ? setAt(at + 1) : setPhase("finale"));
  const kept = tour ? tour.stops.filter((s) => s.keep && !s.id.startsWith("super")).length : 0;

  const header = (
    <header className="settings-head tour-head">
      <button className="tool ghost" onClick={exit}>
        <Icon name="chevron-left" />
        {t("tour.exit")}
      </button>
      {(phase === "stops" || phase === "finale") && tour ? (
        <div className="tour-progress" role="progressbar" aria-label={t("tour.walkTitle")} aria-valuemin={0} aria-valuemax={main.length + 1} aria-valuenow={phase === "finale" ? main.length + 1 : at + 1}>
          {main.map((s, i) => {
            const share = s.id === "gestures" && (s.tasks ?? []).length ? gestureCount / (s.tasks ?? []).length : s.id === "dial" && onFace?.gestures.length ? faceCount / onFace.gestures.length : 0;
            const full = tried[s.id] || phase === "finale";
            return (
              <span key={s.id} className={`tour-seg${full ? " is-done" : ""}${phase === "stops" && i === at ? " is-now" : ""}`}>
                <i style={{ width: `${full ? 100 : phase === "stops" && i === at ? Math.max(12, share * 100) : 0}%` }} />
              </span>
            );
          })}
          <span className={`tour-seg${phase === "finale" ? " is-now" : ""}`}>
            <i style={{ width: phase === "finale" ? "50%" : "0%" }} />
          </span>
          <span className="tour-progress-n">{t("tour.stopOf", { n: phase === "finale" ? main.length + 1 : at + 1, count: main.length + 1 })}</span>
        </div>
      ) : (
        <h2>{phase === "focus" || phase === "making" ? t("tour.title") : t("tour.walkTitle")}</h2>
      )}
      {tour && phase !== "end" && <span className="tour-made-for">{t("tour.madeFor", { focus: t(`tour.focus.${tour.focus}` as Key) })}</span>}
    </header>
  );

  if (phase === "desk") {
    const found = live.deck.modules.filter((m) => m.kind !== "base");
    return (
      <div className="tour-page">
        <header className="settings-head tour-head">
          <h2>{t("welcome.title")}</h2>
          <button className="tool ghost tour-skip" onClick={exit}>
            {t("welcome.skip")}
          </button>
        </header>
        <div className="tour-focus tour-desk">
          <div className="tour-focus-head">
            <h3>{t("welcome.deskTitle")}</h3>
            <p>{found.length === 0 ? t("welcome.none") : `${t("welcome.found", { count: found.length, list: found.map((m) => t(`module.${m.kind}` as Key)).join(", ") })} ${t("welcome.desk")}`}</p>
          </div>
          {found.length > 0 && (
            <div className="builder-deck tour-desk-deck">
              <DeckView live={live} selected={null} onSelect={() => {}} saved={saved} onPlace={onPlace} arranging={false} profile={view.profile} page={null} />
            </div>
          )}
          <div className="row wrap tour-desk-actions">
            <button className="tool primary" onClick={() => setPhase("focus")}>
              <Icon name="check" />
              {t("tour.deskYes")}
            </button>
            {found.length > 0 && onArrange && (
              <button className="tool" onClick={onArrange}>
                <Icon name="layout-grid" />
                {t("welcome.deskFix")}
              </button>
            )}
          </div>
        </div>
      </div>
    );
  }

  if (phase === "focus" || phase === "making") {
    return (
      <div className="tour-page">
        {header}
        <div className="tour-focus">
          <div className="tour-focus-head">
            <h3>{t("tour.focusQuestion")}</h3>
            <p>{suggestion ? t("tour.focusHelp", { why: why(suggestion) }) : t("tour.focusHelpPlain")}</p>
          </div>
          <div className="tour-tiles" role="radiogroup" aria-label={t("tour.focusQuestion")}>
            {FOCUSES.map((f) => (
              <button key={f} role="radio" aria-checked={focus === f} className={`tour-tile${focus === f ? " is-on" : ""}`} onClick={() => setFocus(f)}>
                <Icon name={FOCUS_ICON[f]} />
                <b>{t(`tour.focus.${f}` as Key)}</b>
                <span>{t(`tour.focusDoes.${f}` as Key)}</span>
                {suggestion?.focus === f && <em className="tour-suggested">{t("tour.suggested")}</em>}
              </button>
            ))}
          </div>
          {suggestion?.kit === "none" && <p className="help tour-warn">{t("tour.noModules")}</p>}
          <div className="tour-focus-foot">
            <span className="help">{t("tour.promise")}</span>
            <button className="tool primary tour-go" disabled={phase === "making" || suggestion?.kit === "none"} onClick={start}>
              {phase === "making" && <Icon name="refresh-cw" className="tour-spin" />}
              {phase === "making" ? t("tour.making") : t("tour.make")}
            </button>
          </div>
          {/* what HubDeck is doing while it makes the deck (owner: show it works, not that it hangs) */}
          {phase === "making" && <MakingSteps />}
          {error && <p className="help catalog-err">{error}</p>}
        </div>
      </div>
    );
  }

  const deck = (
    <aside className="tour-deck" aria-label={t("tour.deckNow")}>
      <span className="tour-live">{t("tour.deckNow")}</span>
      <div className="builder-deck">
        <DeckView live={live} selected={tryRef} onSelect={() => {}} saved={saved} onPlace={onPlace} arranging={false} profile={view.profile} page={null} />
      </div>
    </aside>
  );

  if (phase === "stops" && stop && tour) {
    // what to do now, in one line (owner's pick C1)
    const nowWords =
      stop.id === "gestures"
        ? nextTask
          ? gestureWords(nextTask, tour.music)[0]
          : t("tour.now.done")
        : stop.id === "dial"
          ? faceNext
            ? t(`tour.now.dial.${faceNext}` as Key)
            : t("tour.now.pickLook")
          : tried[stop.id]
            ? t("tour.now.done")
            : tryWords(stop, tour);
    return (
      <div className="tour-page">
        {header}
        <div className="tour-body">
          <nav className="tour-parts" aria-label={t("tour.parts")}>
            <span className="tour-parts-title">{t("tour.parts")}</span>
            {main.map((s, i) => (
              <div key={s.id} className={`tour-part${i === at ? " is-current" : ""}`}>
                <button className="tour-part-name" aria-current={i === at ? "step" : undefined} onClick={() => setAt(i)}>
                  {tried[s.id] && <Icon name="check" />}
                  {t(`tour.${s.id}.title` as Key)}
                </button>
                <Toggle on={s.keep} label={t("tour.keepPart", { part: t(`tour.${s.id}.title` as Key) })} onChange={(on) => set(s.id, on, null)} />
              </div>
            ))}
            <span className="help tour-parts-help">{t("tour.partsHelp")}</span>
          </nav>
          <main className="tour-stop">
            <h3>{t(`tour.${stop.id}.title` as Key)}</h3>
            <p className="tour-text">{t((stop.id === "volume" && tour.mac ? "tour.volume.bodyMac" : `tour.${stop.id}.body`) as Key)}</p>
            {at === 0 && <p className="help">{t("tour.firstHint")}</p>}
            {stop.id === "gestures" && <GestureTasks tasks={stop.tasks ?? []} done={gestured} next={nextTask} music={tour.music} />}
            {stop.id === "gestures" && <PressCard live={live} view={view} />}
            {stop.id === "dial" && <DialFaces stop={stop} onPick={(i) => set(stop.id, null, i)} gallery={gallery} ticked={faceTicked} />}
            {stop.id !== "gestures" && stop.id !== "dial" && <div className="tour-try">
              <span className="tour-label">{t("tour.tryIt")}</span>
              <span className="tour-try-text">{tryWords(stop, tour)}</span>
              {tried[stop.id] ? (
                <span className="tour-check" role="status">
                  <Icon name="check" />
                  {t("tour.done")}
                </span>
              ) : (
                <span className="tour-wait" role="status">
                  {t("tour.waiting")}
                </span>
              )}
              {stop.id === "volume" && (
                <button className="link-btn align-start" onClick={() => api.tourSound(true).catch(() => {})}>
                  {t("tour.playAgain")}
                </button>
              )}
              {stop.id === "streaming" && <span className="help">{t("tour.dryRun")}</span>}
              {stop.id === "music" && tour.mac && <span className="help">{t("tour.macAutomation")}</span>}
            </div>}
            {stop.looks > 1 && stop.id !== "dial" && (
              <div className="tour-looks">
                <span className="tour-label">{t("tour.look")}</span>
                {/* what the picked look changes, and where to see it (owner, 2026-10-13: "nie do końca rozumiem") */}
                <span className="help">{t(`tour.lookHelp.${stop.id}.${stop.look}` as Key)}</span>
                <div className="row wrap">
                  {Array.from({ length: stop.looks }, (_, i) => (
                    <button key={i} className={`tour-look${stop.look === i ? " is-on" : ""}`} aria-pressed={stop.look === i} onClick={() => set(stop.id, null, i)}>
                      {t(`tour.look.${stop.id}.${i}` as Key)}
                    </button>
                  ))}
                </div>
              </div>
            )}
            <div className={`tour-nav tour-foot${tried[stop.id] ? " is-ready" : ""}`}>
              <p className="tour-now">
                <b>{t("tour.nowLabel")}</b> {nowWords}
                <br />
                <b>{t("tour.nextLabel")}</b> {at + 1 < main.length ? t(`tour.${main[at + 1].id}.title` as Key) : t("tour.superTitle")}
              </p>
              <label className="check tour-keep">
                <input type="checkbox" checked={stop.keep} onChange={(e) => set(stop.id, e.target.checked, null)} />
                {t("tour.keep")}
              </label>
              <button className="tool primary" onClick={next}>
                {t("tour.next")}
              </button>
              {cheer && (
                <div key={cheer.key} className="tour-cheer" role="status">
                  <span className="tour-cheer-pill">
                    <Icon name="check" />
                    {cheer.text}
                  </span>
                  <span className="tour-confetti" aria-hidden="true">
                    {/* a burst, not a row (owner, 2026-10-14: "lecą w jednej linii"): each piece its own
                        angle, height, spin and delay, then it falls */}
                    {CONFETTI.map((c, i) => (
                      <i key={i} style={{ ["--dx" as string]: `${c.dx}px`, ["--dy" as string]: `${c.dy}px`, ["--r" as string]: `${c.r}deg`, ["--d" as string]: `${c.d}s`, ["--c" as string]: c.color }} />
                    ))}
                  </span>
                </div>
              )}
            </div>

            {error && <p className="help catalog-err">{error}</p>}
          </main>
          {deck}
        </div>
      </div>
    );
  }

  if (phase === "finale" && tour) {
    return (
      <div className="tour-page">
        {header}
        <div className="tour-body">
          <main className="tour-stop tour-finale">
            <span className="tour-count">{t("tour.finale")}</span>
            <h3>{t("tour.superTitle")}</h3>
            <p className="tour-text">{t("tour.superBody")}</p>
            <div className="tour-super">
              {finale.map((s) => (
                <div key={s.id} className="tour-super-item">
                  <Icon name={s.id === "superAnimations" ? "sparkles" : s.id === "superSaver" ? "image" : "palette"} />
                  <div className="grow">
                    <b>{t(`tour.${s.id}.title` as Key)}</b>
                    <span className="help">{t(`tour.${s.id}.body` as Key)}</span>
                    {s.id === "superSaver" && (
                      <button className="link-btn align-start" onClick={() => api.tourSaver("rain").catch(() => {})}>
                        {t("tour.showSaver")}
                      </button>
                    )}
                  </div>
                  {/* the Fun page is always on the deck (owner, 2026-10-14): nothing to keep or drop */}
                  {s.id !== "superAnimations" && (
                    <Toggle on={s.keep} label={t("tour.keepPart", { part: t(`tour.${s.id}.title` as Key) })} onChange={(on) => set(s.id, on, null)} />
                  )}
                </div>
              ))}
            </div>
            <div className="tour-nav">
              <button className="tool ghost" onClick={() => (setPhase("stops"), setAt(main.length - 1))}>
                {t("tour.back")}
              </button>
              <button className="tool primary" onClick={() => setPhase("end")}>
                {t("tour.seeDeck")}
              </button>
            </div>
          </main>
          {deck}
        </div>
      </div>
    );
  }

  return (
    <div className="tour-page">
      {header}
      <div className="tour-end">
        <div className="tour-end-main">
          <h3>{t("tour.endTitle")}</h3>
          <p className="help">{t("tour.endKept", { count: kept })}</p>
          <div className="builder-deck tour-end-deck">
            <DeckView live={live} selected={null} onSelect={() => {}} saved={saved} onPlace={onPlace} arranging={false} profile={view.profile} page={null} />
          </div>
          <div className="row wrap tour-end-actions">
            <button className="tool primary" onClick={() => finish(true, true, () => onClose(true))}>
              <Icon name="check" />
              {t("tour.useHome")}
            </button>
            <button className="tool" onClick={() => finish(true, true, onBuild)}>
              {t("tour.changeSteps")}
            </button>
            <button className="tool ghost" onClick={() => finish(false, false, () => onClose())}>
              {t("tour.throwAway")}
            </button>
          </div>
          <label className="check tour-count-use">
            <input type="checkbox" checked={counting} onChange={(e) => (setCounting(e.target.checked), api.pressesCountSet(e.target.checked).catch(() => {}))} />
            {t("tour.countUse")}
          </label>
          <p className="help">{t("tour.editLater")}</p>
          <p className="help">{t("tour.appDecks")}</p>
          <p className="help">{t("tour.oldStays")}</p>
          {error && <p className="help catalog-err">{error}</p>}
        </div>
        <aside className="tour-end-ask">
          <span className="tour-label">{t("tour.askTitle")}</span>
          <p className="help">{t("tour.askHelp")}</p>
          <textarea className="input" rows={3} value={ask} placeholder={t("tour.askPlaceholder")} onChange={(e) => setAsk(e.target.value)} />
          <button className="tool small align-start" disabled={!ask.trim()} onClick={() => finish(true, true, () => onAsk(ask.trim()))}>
            <Icon name="sparkles" />
            {t("tour.askSend")}
          </button>
        </aside>
      </div>
    </div>
  );
}

/** The steps of making the deck, one after another while it is made. */
function MakingSteps() {
  const steps: Key[] = ["tour.makingModules", "tour.makingApps", "tour.makingDeck"];
  const [at, setAt] = useState(0);
  useEffect(() => {
    const timer = window.setInterval(() => setAt((i) => Math.min(i + 1, steps.length - 1)), 1100);
    return () => window.clearInterval(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  return (
    <ol className="tour-making" aria-live="polite">
      {steps.map((k, i) => (
        <li key={k} className={i < at ? "is-done" : i === at ? "is-now" : ""}>
          <Icon name={i < at ? "check" : i === at ? "refresh-cw" : "circle"} className={i === at ? "tour-spin" : undefined} />
          {t(k)}
        </li>
      ))}
    </ol>
  );
}

/** "One control, many jobs": a hold and a double press on a key, a knob and the dial, ticked off live. */
/** What a "One control, many jobs" task asks, and what it shows. */
function gestureWords(task: string, music?: string | null): [string, string] {
  if (task.startsWith("keys.5")) return [t("tour.gestures.keyHold"), t("tour.gestures.keyHoldHelp")];
  if (task.startsWith("knobs")) return [t("tour.gestures.knobDouble"), t("tour.gestures.knobDoubleHelp")];
  if (task.startsWith("keys")) return [t("tour.gestures.keyDouble"), t("tour.gestures.keyDoubleHelp")];
  return [t("tour.gestures.dialHold"), t(music ? "tour.gestures.dialHoldHelp" : "tour.gestures.dialHoldHelpNext", { app: music ?? "" })];
}

function GestureTasks({ tasks, done, next, music }: { tasks: string[]; done: Record<string, boolean>; next?: string; music?: string | null }) {
  const words = (task: string) => gestureWords(task, music);
  return (
    <div className="tour-try">
      <span className="tour-label">{t("tour.tryIt")}</span>
      <ul className="tour-tasks">
        {tasks.map((task) => {
          const [what, how] = words(task);
          const state = done[task] ? "is-done" : task === next ? "is-now" : "";
          return (
            <li key={task} className={state}>
              <Icon name={done[task] ? "check" : task === next ? "refresh-cw" : "circle"} className={task === next ? "tour-spin" : undefined} />
              <span>
                <b>{what}</b>
                <span className="help">{how}</span>
              </span>
            </li>
          );
        })}
      </ul>
    </div>
  );
}

/** What the control just pressed does on each gesture, from the tour's deck (and that a press opens its editor later). */
function PressCard({ live, view }: { live: LiveState; view: EngineView }) {
  const c = live.lastTouched;
  const id = c ? (c.kind === "dial" ? "dial" : `${c.kind}.${c.index}`) : null;
  const controls = view.profile?.controls ?? {};
  const actions = view.catalog?.actions ?? [];
  const say = (control: string) => {
    const b = controls[control];
    if (!b) return null;
    const def = actions.find((a) => a.id === b.action);
    const name = def ? localizeAction(def).name : b.action;
    const label = (b.look?.label ?? "").replace(/\n/g, " ").trim();
    return label && label !== name ? `${label} · ${name}` : name;
  };
  const turns = c && c.kind !== "keys";
  return (
    <div className="tour-try">
      <span className="tour-label">{id ? t("tour.gestures.pressed", { control: t(`control.${c!.kind}` as Key) + (c!.kind === "dial" ? "" : ` ${c!.index + 1}`) }) : t("tour.gestures.pressAny")}</span>
      {id ? (
        <dl className="tour-gesture-list">
          <dt>{turns ? t("tour.gestures.turnPress") : t("gesture.tap")}</dt>
          <dd>{say(id) ?? t("tour.gestures.free")}</dd>
          <dt>{t("gesture.double")}</dt>
          <dd className={say(`${id}:double`) ? "" : "is-free"}>{say(`${id}:double`) ?? t("tour.gestures.free")}</dd>
          <dt>{t("gesture.hold")}</dt>
          <dd className={say(`${id}:hold`) ? "" : "is-free"}>{say(`${id}:hold`) ?? t("tour.gestures.free")}</dd>
        </dl>
      ) : (
        <span className="tour-try-text">{t("tour.gestures.pressAnyHelp")}</span>
      )}
      <span className="help">{t("tour.gestures.editLater")}</span>
    </div>
  );
}

/** The pieces of the cheer's burst: spread around the pill, mostly up, fixed so it looks the same each time. */
const CONFETTI = Array.from({ length: 18 }, (_, i) => {
  const angle = -Math.PI / 2 + ((i / 17) - 0.5) * Math.PI * 1.25 + (i % 3) * 0.07;
  const reach = 46 + ((i * 37) % 5) * 11;
  return {
    dx: Math.round(Math.cos(angle) * reach * 1.6),
    dy: Math.round(Math.sin(angle) * reach),
    r: ((i * 97) % 360) + 180,
    d: ((i * 13) % 7) * 0.025,
    color: ["#ffd166", "#4ade80", "#e879f9", "#60a5fa", "#f472b6"][i % 5],
  };
});

const GESTURE_ICON: Record<string, string> = { turn: "rotate-cw", press: "circle-dot", double: "copy", hold: "hand" };

/** The look on the dial: what each gesture does, ticked off when done on the deck; the next one to try stands out. */
function FaceGestures({ face, ticked }: { face: DialFace; ticked: Record<string, boolean> }) {
  const pl = getLanguage() === "pl";
  const next = face.gestures.find((g) => !ticked[g.kind])?.kind;
  const all = !next;
  return (
    <div className={`tour-face-gestures${all ? " is-all" : ""}`} aria-live="polite">
      <span className="tour-label">{all ? t("tour.dial.allDone") : t("tour.dial.tryOn", { face: pl ? face.captionPl : face.caption })}</span>
      <ul>
        {face.gestures.map((g) => {
          const done = !!ticked[g.kind];
          return (
            <li key={g.kind} className={done ? "is-done" : g.kind === next ? "is-now" : ""}>
              <span className="tour-gesture-mark">
                <Icon name={done ? "check" : GESTURE_ICON[g.kind]} />
              </span>
              <b>{t(`tour.dial.gesture.${g.kind}` as Key)}</b>
              <span>{pl ? g.whatPl : g.what}</span>
            </li>
          );
        })}
      </ul>
    </div>
  );
}

/** "Your dial, many faces": HubDeck's own dial looks; Try puts one on the dial (the tour's deck keeps it). */
function DialFaces({ stop, onPick, gallery, ticked }: { stop: TourStop; onPick: (i: number) => void; gallery: DialFace[]; ticked: Record<string, boolean> }) {
  const ids = stop.faces ?? [];
  // the look on the dial: its four gestures, ticked off as they happen on the deck (a new look starts over)
  const onFace = gallery.find((f) => f.id === ids[stop.look]);
  return (
    <div className="tour-faces">
      {onFace && onFace.gestures.length > 0 && <FaceGestures face={onFace} ticked={ticked} />}
      {ids.map((id, i) => {
        const face = gallery.find((f) => f.id === id);
        const on = stop.look === i;
        return (
          <div key={id} className={`tour-face${on ? " is-on" : ""}`}>
            <div className="tour-face-dial" dangerouslySetInnerHTML={{ __html: inWindow(face?.svg ?? "") }} />
            <b>{face ? (getLanguage() === "pl" ? face.captionPl : face.caption) : id}</b>
            <span className="help">{t(`tour.dial.face.${id}` as Key)}</span>
            {on ? (
              <span className="tour-face-on">{t("tour.dial.onDial")}</span>
            ) : (
              <button className="tool small" onClick={() => onPick(i)}>
                {t("tour.dial.try")}
              </button>
            )}
          </div>
        );
      })}
      <p className="help tour-faces-more">{t("tour.dial.more")}</p>
    </div>
  );
}

/** Why HubDeck suggests what it suggests, in a few words. */
function why(s: TourSuggestion): string {
  if (s.focus === "games" && s.games > 0) return t("tour.whyGames", { count: s.games });
  if (s.focus === "streaming") return t("tour.whyStreaming");
  if (s.focus === "calls" && s.call) return t("tour.whyCalls", { app: s.call === "discord" ? "Discord" : s.call === "teams" ? "Teams" : "Zoom" });
  if (s.focus === "music" && s.music) return t("tour.whyMusic", { app: s.music });
  return t("tour.whyMixed");
}

/** The house switch (a checkbox drawn as a switch, as in Settings). */
function Toggle({ on, label, onChange }: { on: boolean; label: string; onChange: (on: boolean) => void }) {
  return (
    <label className="check tour-toggle">
      <input type="checkbox" checked={on} aria-label={label} onChange={(e) => onChange(e.target.checked)} />
    </label>
  );
}
