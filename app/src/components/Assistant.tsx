// The deck assistant (owner's pick 4C: a third tab of the left column, so the live deck and the
// key editor stay): a scan the user sees before anything is sent, an interview with answer
// buttons, then drafts to try on the deck, keep or throw away.
import { Fragment, useEffect, useRef, useState, useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { api, usePreview, type AssistantEvent, type AssistantInfo, type AssistantScan, type EngineView, type Idea, type Look, type Nudge, type Profile, type Taste } from "../engine";
import { getLanguage, t, useLanguage, type Key } from "../i18n";
import { Icon } from "./Icon";

type Line =
  | {
      who: "user" | "assistant" | "error";
      text: string;
      choices?: string[];
      ideas?: Idea[];
      sketches?: [string, string][];
      /** Still being written (pieces of it came in). */
      streaming?: boolean;
      /** The cards of this turn's ideas (propose_idea and HubDeck's quick sketches). */
      ideasLine?: boolean;
    }
  | { who: "work"; tools: string[] }
  | { who: "drafts"; ids: string[] }
  | { who: "remembered"; text: string }
  | { who: "permission"; id: number; tool: string; detail: string; answer?: "once" | "talk" | "deny" };

/** A message line (said by the user, the assistant or an error). */
type Message = Exclude<Line, { who: "work" } | { who: "drafts" } | { who: "remembered" } | { who: "permission" }>;

/** Tools the user does not need to hear about (their result shows by itself). */
const QUIET_TOOLS = ["offer_choices", "remember", "propose_idea"];

/** The conversation lives outside the panel, so switching tabs keeps it. */
const talk = {
  phase: "start" as "start" | "scanning" | "consent" | "chat",
  scan: null as AssistantScan | null,
  lines: [] as Line[],
  busy: false,
  sentScan: false,
  /** Drafts there were before the running turn: the new ones are shown in the talk when it ends. */
  knownDrafts: [] as string[],
  /** Tokens in the context now and the model's window, from the last call. */
  context: { used: 0, window: 0 },
  /** Answers offered as buttons (offer_choices), until the next message; several may be ticked. */
  choices: [] as string[],
  multiple: false,
  /** A first message to start the talk with (from a suggestion). */
  prefill: "",
  /** Look at the apps at once when the panel shows (a message came from the tour's end). */
  wantScan: false,
  /** HubDeck's own suggestion for now (an app without a deck, a new kind of module). */
  nudge: null as Nudge | null,
};

/** Start a talk with this message (the tour's "Ask the assistant"): the apps are looked at, the
 * message waits in the first box with what will be sent. */
export function askAssistant(message: string) {
  talk.prefill = message;
  talk.phase = "start";
  talk.wantScan = true;
  changed();
}

/** Ask HubDeck whether it has a suggestion now (the tab shows a dot, the panel a card). */
export function refreshNudge() {
  if (!("__TAURI_INTERNALS__" in window)) return;
  api.nudgeGet().then(
    (n) => ((talk.nudge = n), changed()),
    () => {},
  );
}

/** The suggestion for now, for the tab's dot. */
export function useNudge() {
  useSyncExternalStore(subscribe, () => version);
  return talk.nudge;
}
const listeners = new Set<() => void>();
let version = 0;
const changed = () => {
  version++;
  listeners.forEach((l) => l());
};
const subscribe = (l: () => void) => {
  listeners.add(l);
  return () => listeners.delete(l);
};

/** The cards line of this turn (after the user's last message), made when the first idea comes. */
function ideasLine(): Message {
  for (let i = talk.lines.length - 1; i >= 0; i--) {
    const l = talk.lines[i];
    if (l.who === "user") break;
    if (l.who === "assistant" && l.ideasLine) return l;
  }
  const line: Message = { who: "assistant", text: "", ideas: [], sketches: [], ideasLine: true };
  talk.lines = [...talk.lines, line];
  return line;
}

function onEvent(e: AssistantEvent) {
  const last = talk.lines[talk.lines.length - 1];
  if (e.kind === "context") talk.context = { used: e.used, window: e.window || talk.context.window };
  else if (e.kind === "delta") {
    // the message grows as it is written; the whole text replaces it
    if (last?.who === "assistant" && last.streaming) talk.lines = [...talk.lines.slice(0, -1), { ...last, text: last.text + e.text }];
    else talk.lines = [...talk.lines, { who: "assistant", text: e.text, streaming: true }];
  } else if (e.kind === "idea") {
    const line = ideasLine();
    const next: Line = { ...line, ideas: [...(line.ideas ?? []), e.idea], sketches: e.draft ? [...(line.sketches ?? []), [e.idea.title, e.draft]] : line.sketches };
    if (e.draft) talk.knownDrafts = [...talk.knownDrafts, e.draft];
    talk.lines = talk.lines.map((l) => (l === line ? next : l));
  } else if (e.kind === "choices") {
    talk.choices = e.choices;
    talk.multiple = e.multiple;
  }
  else if (e.kind === "remembered") talk.lines = [...talk.lines, { who: "remembered", text: e.text }];
  else if (e.kind === "permission") talk.lines = [...talk.lines, { who: "permission", id: e.id, tool: e.tool, detail: e.detail }];
  else if (e.kind === "text") {
    const line: Line = { who: "assistant", text: e.text, choices: e.choices, ideas: e.ideas };
    const streamed = talk.lines.length > 0 && last?.who === "assistant" && last.streaming;
    talk.lines = streamed ? [...talk.lines.slice(0, -1), line] : [...talk.lines, line];
    // ideas with a whole deck sketched: HubDeck makes drafts of them at once, to see and try
    if (e.ideas.some((i) => i.deck))
      api.sketchesMake(e.ideas.filter((i) => i.deck)).then((made) => {
        talk.knownDrafts = [...talk.knownDrafts, ...made.map(([, id]) => id)];
        talk.lines = talk.lines.map((l) => (l === line ? { ...line, sketches: made } : l));
        changed();
      }, () => {});
  }
  else if (e.kind === "tool") {
    if (QUIET_TOOLS.includes(e.name)) return changed();
    // one "working" line per stretch of tool use, listing what it did
    if (last?.who === "work") talk.lines = [...talk.lines.slice(0, -1), { who: "work", tools: [...last.tools, e.name] }];
    else talk.lines = [...talk.lines, { who: "work", tools: [e.name] }];
  } else if (e.kind === "error") talk.lines = [...talk.lines, { who: "error", text: e.message }];
  else if (e.kind === "done") {
    talk.busy = false;
    // drafts made in this turn: shown as pictures of the deck, right in the talk
    api.state().then((st) => {
      const fresh = st.profiles.filter((p) => p.draft && !talk.knownDrafts.includes(p.id)).map((p) => p.id);
      if (!fresh.length) return;
      talk.knownDrafts = [...talk.knownDrafts, ...fresh];
      const end = talk.lines.length && talk.lines[talk.lines.length - 1].who === "assistant" ? talk.lines.length - 1 : talk.lines.length;
      talk.lines = [...talk.lines.slice(0, end), { who: "drafts", ids: fresh }, ...talk.lines.slice(end)];
      changed();
    });
  }
  changed();
}

let listening = false;
function listenOnce() {
  if (listening || !("__TAURI_INTERNALS__" in window)) return;
  listening = true;
  listen<AssistantEvent>("assistant://event", (e) => onEvent(e.payload)).catch(() => (listening = false));
}

/** Send a message; `context` goes to the assistant after it but is not shown in the talk. */
function send(message: string, context = "") {
  const text = message.trim();
  if (!text || talk.busy) return;
  const scan = !talk.sentScan && talk.scan ? talk.scan.text : null;
  talk.lines = [...talk.lines, { who: "user", text }];
  talk.choices = [];
  talk.busy = true;
  talk.phase = "chat";
  changed();
  // the first message starts a fresh talk: the engine may still hold one from before a reload
  const fresh = !talk.sentScan ? api.assistantReset() : Promise.resolve();
  fresh
    .then(() => api.state())
    .then((st) => (talk.knownDrafts = st.profiles.filter((p) => p.draft).map((p) => p.id)))
    .then(() => api.assistantSend(context ? `${text}\n\n${context}` : text, scan, getLanguage()))
    .then(
    () => (talk.sentScan = true),
    (err) => {
      talk.busy = false;
      talk.lines = [...talk.lines, { who: "error", text: String(err) === "no-cli" ? t("assistant.noCli") : String(err) }];
      changed();
    },
  );
}

function startOver() {
  api.assistantReset().catch(() => {});
  Object.assign(talk, { phase: "start", scan: null, lines: [], busy: false, sentScan: false, knownDrafts: [], context: { used: 0, window: 0 }, choices: [], multiple: false });
  changed();
}

/** `**words**` in a message (the assistant sometimes marks names so) as bold, the rest as text. */
function bold(text: string) {
  return text.split(/(\*\*[^*]+\*\*)/g).map((part, i) => (part.startsWith("**") && part.endsWith("**") && part.length > 4 ? <b key={i}>{part.slice(2, -2)}</b> : part));
}

const planName = (info: AssistantInfo) =>
  info.authMethod && info.authMethod !== "claude.ai" ? t("assistant.apiKey") : info.plan ? `Claude ${info.plan.charAt(0).toUpperCase()}${info.plan.slice(1)}` : t("assistant.claudePlan");

/** The account, the plan and everything the assistant is started with, to look at any time. */
function RunsOn({ info, onCheck }: { info: AssistantInfo | null; onCheck: () => void }) {
  if (!info) return <div className="as-runs">{t("assistant.checking")}</div>;
  if (!info.cli)
    return (
      <div className="as-runs is-missing">
        <b>{t("assistant.needsTitle")}</b>
        <p>{t("assistant.needs")}</p>
        <button className="link-btn align-start" onClick={() => api.openLink("https://claude.com/claude-code").catch(() => {})}>
          claude.com/claude-code
        </button>
        <p>{t("assistant.without")}</p>
        <button className="tool small align-start" onClick={onCheck}>
          {t("assistant.checkAgain")}
        </button>
      </div>
    );
  if (!info.loggedIn)
    return (
      <div className="as-runs is-missing">
        <b>{t("assistant.signInTitle")}</b>
        <p>{t("assistant.signIn")}</p>
        <button className="tool small align-start" onClick={onCheck}>
          {t("assistant.checkAgain")}
        </button>
        <UnderTheHood info={info} onChanged={onCheck} />
      </div>
    );
  return (
    <div className="as-runs">
      <b>{t("assistant.runsTitle")}</b>
      <p>{t("assistant.runs", { account: info.email ?? "", plan: planName(info) })}</p>
      <p>{info.authMethod && info.authMethod !== "claude.ai" ? t("assistant.costApi") : t("assistant.cost")}</p>
      <PowerSwitch info={info} onChanged={onCheck} />
      <UnderTheHood info={info} onChanged={onCheck} />
    </div>
  );
}

/** One line over the talk: what it runs on and whose usage it counts toward. */
function SessionBar({ info }: { info: AssistantInfo }) {
  const used = talk.context.used || info.contextUsed;
  const window = talk.context.window || info.contextWindow;
  return (
    <div className="as-session" title={info.session ? `${t("assistant.session")}: ${info.session}` : undefined}>
      <Icon name="terminal" />
      <span className="as-session-text">{["Claude Code", info.model, (info.chosenEffort ?? info.defaultEffort) && `effort ${info.chosenEffort ?? info.defaultEffort}`, planName(info)].filter(Boolean).join(" · ")}</span>
      {window > 0 && <ContextRing used={used} window={window} />}
    </div>
  );
}

/** How full the conversation's context is, as a ring with the percent (like the Claude app). */
function ContextRing({ used, window }: { used: number; window: number }) {
  const share = Math.min(1, used / window);
  const pct = Math.round(share * 100);
  const r = 7;
  const c = 2 * Math.PI * r;
  const fmt = (n: number) => n.toLocaleString(getLanguage() === "pl" ? "pl-PL" : "en-US");
  const level = share > 0.8 ? " is-full" : share > 0.5 ? " is-half" : "";
  const help = t("assistant.contextHelp", { used: fmt(used), window: fmt(window), pct });
  return (
    <span className={`as-ring${level}`} title={help} aria-label={help}>
      <svg viewBox="0 0 18 18" width="16" height="16" aria-hidden="true">
        <circle cx="9" cy="9" r={r} className="as-ring-bg" />
        <circle cx="9" cy="9" r={r} className="as-ring-fill" strokeDasharray={`${c * share} ${c}`} transform="rotate(-90 9 9)" />
      </svg>
      <span>{pct < 1 && used > 0 ? "<1%" : `${pct}%`}</span>
    </span>
  );
}

const MODELS = ["fable", "opus", "sonnet", "haiku"];
const EFFORTS = ["low", "medium", "high", "xhigh", "max"];

/** Model and effort: Claude Code's default unless picked here (aliases follow the newest model). */
function ModelPicker({ info, onChanged }: { info: AssistantInfo; onChanged: () => void }) {
  // both picks kept here: the second change must not send the first one's old value
  const [pick, setPick] = useState<[string | null, string | null]>([info.chosenModel, info.chosenEffort]);
  const set = (model: string | null, effort: string | null) => {
    setPick([model, effort]);
    api.assistantSet(model, effort).then(onChanged, () => {});
  };
  const def = (v: string | null) => (v ? t("assistant.sameAsCodeIs", { value: v }) : t("assistant.sameAsCode"));
  return (
    <div className="as-pick">
      <label className="field">
        <span className="label">{t("assistant.model")}</span>
        <select className="input" value={pick[0] ?? ""} onChange={(e) => set(e.target.value || null, pick[1])}>
          <option value="">{def(info.defaultModel)}</option>
          {MODELS.map((m) => (
            <option key={m} value={m}>
              {m.charAt(0).toUpperCase() + m.slice(1)}
            </option>
          ))}
        </select>
      </label>
      <label className="field">
        <span className="label">{t("assistant.effort")}</span>
        <select className="input" value={pick[1] ?? ""} onChange={(e) => set(pick[0], e.target.value || null)}>
          <option value="">{def(info.defaultEffort)}</option>
          {EFFORTS.map((x) => (
            <option key={x} value={x}>
              {x}
            </option>
          ))}
        </select>
      </label>
      <p className="help">{t("assistant.pickHelp")}</p>
    </div>
  );
}

/** Everything the assistant is started with: version, model, session, folder, command, rules. */
function UnderTheHood({ info, onChanged }: { info: AssistantInfo; onChanged: () => void }) {
  const rows: [string, string | null | undefined][] = [
    ["Claude Code", [info.version, info.cli].filter(Boolean).join(" · ")],
    [t("assistant.model"), info.model ?? t("assistant.modelLater")],
    [t("assistant.session"), info.session ?? t("assistant.sessionLater")],
    [t("assistant.folder"), info.folder],
  ];
  return (
    <details className="as-hood">
      <summary>{t("assistant.hood")}</summary>
      <ModelPicker info={info} onChanged={onChanged} />
      <dl className="facts">
        {rows.map(([k, v]) => (
          <Fragment key={k}>
            <dt>{k}</dt>
            <dd className="mono">{v}</dd>
          </Fragment>
        ))}
      </dl>
      {info.session && <p className="help">{t("assistant.resume", { folder: info.folder, session: info.session })}</p>}
      <span className="label">{t("assistant.command")}</span>
      <pre className="as-pre">{info.command}</pre>
      <p className="help">{t("assistant.toolsOnly")}</p>
      <span className="label">{t("assistant.rules")}</span>
      <pre className="as-pre">{info.rules}</pre>
    </details>
  );
}

/** Icon colors of the house look for the sample keys on idea cards. */
const TINTS = ["#8c79ec", "#79ecc6", "#79c6ec", "#ecb379", "#ec798c"];

/** The assistant's ideas as cards: pick some and build them (owner: ideas, not a survey). */
function IdeaCards({ ideas, live }: { ideas: Idea[]; live: boolean }) {
  const [picked, setPicked] = useState<string[]>([]);
  const toggle = (title: string) => setPicked((p) => (p.includes(title) ? p.filter((x) => x !== title) : [...p, title]));
  return (
    <div className="as-ideas">
      {ideas.map((idea) => {
        const on = picked.includes(idea.title);
        return (
          <button key={idea.title} className={`as-idea${on ? " is-on" : ""}`} disabled={!live} aria-pressed={on} onClick={() => toggle(idea.title)}>
            <span className="as-idea-head">
              <b>{idea.title}</b>
              <span className={`as-tick${on ? " is-on" : ""}`}>{on && <Icon name="check" />}</span>
            </span>
            {idea.keys.length > 0 && (
              <span className="as-idea-keys" aria-hidden="true">
                {idea.keys.slice(0, 5).map((k, i) => (
                  <MiniFace key={i} screen="key" look={{ label: k.label, icon: k.icon, bg: "#1b1a20", fg: "#e6e4ee", iconColor: TINTS[i % TINTS.length] }} />
                ))}
              </span>
            )}
            <span className="as-idea-pitch">{idea.pitch}</span>
            {idea.extras.length > 0 && (
              <span className="as-idea-extras">
                {idea.extras.map((x) => (
                  <span key={x} className="tag">
                    <Icon name="sparkles" />
                    {x}
                  </span>
                ))}
              </span>
            )}
          </button>
        );
      })}
      {live && (
        <button className="tool primary" disabled={!picked.length} onClick={() => send(t("assistant.buildThese", { list: picked.join("; ") }))}>
          <Icon name="sparkles" />
          {picked.length ? t("assistant.build", { count: picked.length }) : t("assistant.pickIdeas")}
        </button>
      )}
    </div>
  );
}

/** How many shortcuts of a draft are not in HubDeck's checked packs (profile_check). */
function useUnchecked(id: string, version?: string) {
  const [unchecked, setUnchecked] = useState(0);
  useEffect(() => {
    api.checkProfile(id).then((list) => setUnchecked(list.filter((i) => i.message.startsWith("Shortcut not checked")).length), () => setUnchecked(0));
  }, [id, version]);
  return unchecked;
}

/** The ideas as decks, one at a time (owner's pick C, 2026-10-12, "zlewa mi się wszystko"): the
 * arrows, the dots or a turn of the dial flip through them, and while one is on the deck the
 * window shows the same one. */
function Sketches({ sketches, ideas, view, live }: { sketches: [string, string][]; ideas: Idea[]; view: EngineView; live: boolean }) {
  const shown = sketches.filter(([, id]) => view.state?.profiles.some((p) => p.id === id && p.draft));
  const ids = shown.map(([, id]) => id);
  const [at, setAt] = useState(0);
  // the deck leads while it shows one of these (a turn of the dial flips it)
  const onDeck = view.state?.trying ? ids.indexOf(view.state.trying) : -1;
  useEffect(() => {
    if (onDeck >= 0) setAt(onDeck);
  }, [onDeck]);
  if (!shown.length) return null;
  const i = onDeck >= 0 ? onDeck : Math.min(at, ids.length - 1);
  const go = (n: number) => {
    const next = (n + ids.length) % ids.length;
    setAt(next);
    if (onDeck >= 0) api.compareDrafts(ids, next).catch(() => {});
  };
  return (
    <div className="as-carousel">
      <div className="as-car-nav">
        <button className="icon-btn" aria-label={t("assistant.prev")} disabled={ids.length < 2} onClick={() => go(i - 1)}>
          <Icon name="chevron-left" />
        </button>
        <span className="as-car-pos">{t("assistant.pos", { n: i + 1, total: ids.length })}</span>
        <button className="icon-btn" aria-label={t("assistant.next")} disabled={ids.length < 2} onClick={() => go(i + 1)}>
          <Icon name="chevron-right" />
        </button>
      </div>
      {shown.map(([title, id], n) => (
        <SketchCard
          key={id}
          hidden={n !== i}
          id={id}
          title={title}
          idea={ideas.find((x) => x.title === title)}
          view={view}
          onDeck={onDeck === n}
          live={live}
          tryIt={() => api.compareDrafts(ids, n).catch(() => {})}
        />
      ))}
      {ids.length > 1 && (
        <div className="as-dots">
          {ids.map((id, n) => (
            <button key={id} className={n === i ? "is-on" : ""} aria-label={t("assistant.pos", { n: n + 1, total: ids.length })} onClick={() => go(n)} />
          ))}
        </div>
      )}
    </div>
  );
}

/** One sketch, everything about it in one place: its name, the deck, what it does, the buttons. */
function SketchCard({ id, title, idea, view, onDeck, live, tryIt, hidden }: { id: string; title: string; idea?: Idea; view: EngineView; onDeck: boolean; live: boolean; tryIt: () => void; hidden: boolean }) {
  const [profile, setProfile] = useState<Profile | null>(null);
  const summary = view.state?.profiles.find((p) => p.id === id);
  const unchecked = useUnchecked(id, summary?.name);
  useEffect(() => {
    api.profile(id).then(setProfile, () => setProfile(null));
  }, [id, summary?.name]);
  return (
    <div className={`as-sketch${onDeck ? " is-trying" : ""}`} hidden={hidden}>
      <b className="as-draft-name">{summary?.name ?? title}</b>
      <div className="as-mini is-big">
        <div className="as-mini-keys">
          {Array.from({ length: 15 }, (_, k) => (
            <MiniFace key={k} screen="key" look={profile?.controls[`keys.${k}`]?.look} />
          ))}
        </div>
        <MiniFace screen="dial" look={profile?.controls.dial?.look} />
      </div>
      {idea && (
        <div className="as-sketch-about">
          <span className="as-idea-pitch">{idea.pitch}</span>
          {idea.extras.length > 0 && (
            <span className="as-idea-extras">
              {idea.extras.map((x) => (
                <span key={x} className="tag">
                  <Icon name="sparkles" />
                  {x}
                </span>
              ))}
            </span>
          )}
        </div>
      )}
      {profile && <TurnLine profile={profile} />}
      {unchecked > 0 && (
        <span className="as-unchecked" title={t("assistant.uncheckedHelp")}>
          <Icon name="alert" />
          {t("assistant.unchecked", { count: unchecked })}
        </span>
      )}
      <div className="row wrap">
        {!onDeck && (
          <button className="tool small" onClick={tryIt} title={t("assistant.tryHelp")}>
            <Icon name="play" />
            {t("assistant.tryOnDeck")}
          </button>
        )}
        <button className="tool small primary" disabled={!live} onClick={() => send(t("assistant.pickSketch", { title, id }))} title={t("assistant.pickSketchHelp")}>
          <Icon name="sparkles" />
          {t("assistant.pick")}
        </button>
        <button className="icon-btn" title={t("assistant.discard")} aria-label={t("assistant.discard")} onClick={() => api.settleDraft(id, false)}>
          <Icon name="x" />
        </button>
      </div>
      {onDeck && (
        <div className="as-sync">
          <span>{t("assistant.onDeckNow")}</span>
          <button className="link-btn" onClick={() => api.compareDrafts(null).catch(() => {})}>
            {t("assistant.backToMine")}
          </button>
        </div>
      )}
    </div>
  );
}

/** A draft as the deck would show it: its 15 keys and the dial, with Try, Use this and Throw away. */
function DraftCard({ id, view, pick }: { id: string; view: EngineView; pick?: () => void }) {
  const [profile, setProfile] = useState<Profile | null>(null);
  const summary = view.state?.profiles.find((p) => p.id === id);
  // shortcuts the assistant chose that are not in HubDeck's checked packs (profile_check)
  const unchecked = useUnchecked(id, summary?.name);
  useEffect(() => {
    api.profile(id).then(setProfile, () => setProfile(null));
  }, [id, summary?.name]);
  if (!summary) return <p className="as-work">{t("assistant.draftGone")}</p>;
  const trying = view.state?.trying === id;
  const draft = summary.draft;
  return (
    <div className={`as-draft-card${trying ? " is-trying" : ""}`}>
      <div className="as-mini">
        <div className="as-mini-keys">
          {Array.from({ length: 15 }, (_, i) => (
            <MiniFace key={i} screen="key" look={profile?.controls[`keys.${i}`]?.look} />
          ))}
        </div>
        <MiniFace screen="dial" look={profile?.controls.dial?.look} />
      </div>
      {/* what the knobs, rollers and faders do: they have no screen to show it */}
      {profile && <TurnLine profile={profile} />}
      <b className="as-draft-name">{summary.name}</b>
      {unchecked > 0 && (
        <span className="as-unchecked" title={t("assistant.uncheckedHelp")}>
          <Icon name="alert" />
          {t("assistant.unchecked", { count: unchecked })}
        </span>
      )}
      {draft ? (
        <div className="row wrap">
          {trying ? (
            <button className="tool small" onClick={() => api.tryProfile(null)}>
              {t("assistant.back")}
            </button>
          ) : (
            <button className="tool small" onClick={() => api.tryProfile(id)} title={t("assistant.tryHelp")}>
              <Icon name="play" />
              {t("assistant.try")}
            </button>
          )}
          {pick ? (
            <button className="tool small primary" onClick={pick} title={t("assistant.pickSketchHelp")}>
              <Icon name="sparkles" />
              {t("assistant.pick")}
            </button>
          ) : (
            <button className="tool small primary" onClick={() => api.settleDraft(id, true)} title={t("assistant.useHelp")}>
              <Icon name="check" />
              {t("assistant.use")}
            </button>
          )}
          <button className="icon-btn" title={t("assistant.discard")} aria-label={t("assistant.discard")} onClick={() => api.settleDraft(id, false)}>
            <Icon name="x" />
          </button>
        </div>
      ) : (
        <span className="tag is-acc">{t("assistant.kept")}</span>
      )}
    </div>
  );
}

/** "Knobs: Spotify · Firefox · System   Rollers: Scroll · Seek" for a draft. */
function TurnLine({ profile }: { profile: Profile }) {
  const names = (kind: string, n: number) =>
    Array.from({ length: n }, (_, i) => profile.controls[`${kind}.${i}`]?.look.label?.replace(/\s+/g, " "))
      .filter(Boolean)
      .join(" · ");
  const parts = ([["knobs", 3, "assistant.knobs"], ["rollers", 2, "assistant.rollers"], ["faders", 5, "assistant.faders"]] as [string, number, Key][])
    .map(([kind, n, key]) => [t(key), names(kind, n)])
    .filter(([, v]) => v);
  if (!parts.length) return null;
  return (
    <p className="as-turnline">
      {parts.map(([k, v]) => (
        <span key={k}>
          <b>{k}:</b> {v}
        </span>
      ))}
    </p>
  );
}

/** One key or the dial of a draft, drawn by the engine like on the deck. */
function MiniFace({ screen, look }: { screen: "key" | "dial"; look?: Look }) {
  const svg = usePreview(screen, look);
  return <span className={`as-face is-${screen}${look ? "" : " is-empty"}`} dangerouslySetInnerHTML={look && svg ? { __html: svg } : undefined} />;
}

/** What each tool use means to the user, in a few words. */
const TOOL_WORDS: Record<string, Key> = {
  design_guide: "assistant.tool.guide",
  list_modules: "assistant.tool.modules",
  list_actions: "assistant.tool.actions",
  list_icons: "assistant.tool.icons",
  list_apps: "assistant.tool.apps",
  list_games: "assistant.tool.apps",
  list_profiles: "assistant.tool.profiles",
  get_profile: "assistant.tool.profiles",
  create_profile: "assistant.tool.draft",
  set_control: "assistant.tool.change",
  clear_control: "assistant.tool.change",
  set_lights: "assistant.tool.lights",
  add_page: "assistant.tool.change",
  add_image: "assistant.tool.picture",
  add_animation: "assistant.tool.picture",
  preview_control: "assistant.tool.preview",
  app_picture: "assistant.tool.picture",
  make_tour: "assistant.tool.tour",
  list_shortcuts: "assistant.tool.shortcuts",
  try_on_deck: "assistant.tool.deck",
};
const toolWords = (tools: string[]) => [...new Set(tools.map((n) => t(TOOL_WORDS[n] ?? "assistant.tool.other")))].join(" · ");

/** The answers it offers: one press sends one; with `multiple` (owner: "opcja zaznaczenia
 * wszystkich opcji") they are ticked, All ticks every one, and Send sends the ticked ones. */
function Choices({ choices, multiple }: { choices: string[]; multiple: boolean }) {
  const [picked, setPicked] = useState<string[]>([]);
  if (!multiple)
    return (
      <div className="as-choices">
        {choices.map((c) => (
          <button key={c} className="chip" onClick={() => send(c)}>
            {c}
          </button>
        ))}
      </div>
    );
  const all = picked.length === choices.length;
  const toggle = (c: string) => setPicked((p) => (p.includes(c) ? p.filter((x) => x !== c) : [...p, c]));
  return (
    <div className="as-choices is-multi">
      {choices.map((c) => (
        <button key={c} className={`chip${picked.includes(c) ? " is-on" : ""}`} aria-pressed={picked.includes(c)} onClick={() => toggle(c)}>
          {picked.includes(c) && <Icon name="check" />}
          {c}
        </button>
      ))}
      <div className="row wrap">
        <button className="tool small" onClick={() => setPicked(all ? [] : choices)}>
          {all ? t("assistant.none") : t("assistant.all")}
        </button>
        <button className="tool small primary" disabled={!picked.length} onClick={() => send(choices.filter((c) => picked.includes(c)).join("; "))}>
          {t("assistant.sendPicked", { count: picked.length })}
        </button>
      </div>
    </div>
  );
}

/** What each of Claude Code's own tools does, in a few words. */
const ASK_WORDS: Record<string, Key> = { Bash: "assistant.ask.Bash", WebFetch: "assistant.ask.WebFetch", WebSearch: "assistant.ask.WebSearch", Read: "assistant.ask.Read", Glob: "assistant.ask.Read", Grep: "assistant.ask.Read" };

/** More power: Claude Code wants to use one of its own tools; the user says yes once, yes for
 * this talk, or no (owner, 2026-10-12: more power, with consent). */
function Permission({ line }: { line: Extract<Line, { who: "permission" }> }) {
  const answer = (choice: "once" | "talk" | "deny") => {
    api.assistantPermit(line.id, choice).catch(() => {});
    talk.lines = talk.lines.map((l) => (l === line ? { ...line, answer: choice } : l));
    changed();
  };
  return (
    <div className={`as-permission${line.answer ? " is-done" : ""}`}>
      <span className="as-perm-head">
        <Icon name="alert" />
        {t(ASK_WORDS[line.tool] ?? "assistant.ask.other", { tool: line.tool })}
      </span>
      <code className="as-perm-detail">{line.detail}</code>
      {line.answer ? (
        <span className="as-perm-answer">{line.answer === "deny" ? t("assistant.ask.denied") : t("assistant.ask.allowed")}</span>
      ) : (
        <div className="row wrap">
          <button className="tool small primary" onClick={() => answer("once")}>
            {t("assistant.ask.once")}
          </button>
          <button className="tool small" onClick={() => answer("talk")}>
            {t("assistant.ask.talk")}
          </button>
          <button className="tool small" onClick={() => answer("deny")}>
            {t("assistant.ask.deny")}
          </button>
        </div>
      )}
    </div>
  );
}

/** Counting presses on or off (off by default; off forgets the counts). */
function CountSwitch() {
  const [on, setOn] = useState<boolean | null>(null);
  useEffect(() => {
    invoke<boolean>("presses_counting").then(setOn, () => setOn(null));
  }, []);
  if (on === null) return null;
  return (
    <label className="row as-switch">
      <input type="checkbox" checked={on} onChange={(e) => (setOn(e.target.checked), invoke("presses_count_set", { on: e.target.checked }).catch(() => {}))} />
      <span>{t("assistant.countPresses")}</span>
    </label>
  );
}

/** More power on or off (off by default). */
function PowerSwitch({ info, onChanged }: { info: AssistantInfo; onChanged: () => void }) {
  return (
    <label className="row as-switch">
      <input type="checkbox" checked={info.power} onChange={(e) => api.assistantPowerSet(e.target.checked).then(onChanged, () => {})} />
      <span>
        <b>{t("assistant.power")}</b> {t("assistant.powerHelp")}
      </span>
    </label>
  );
}

/** Remove one note from what the assistant remembers. */
function forget(text: string) {
  return api.tasteGet().then((t) => api.tasteSet(t.notes.map((n) => n.text).filter((n) => n !== text)));
}

/** What the assistant remembers about the user (taste.json): read, remove, add, forget all. */
function TasteCard() {
  const [taste, setTaste] = useState<Taste | null>(null);
  const [add, setAdd] = useState("");
  useEffect(() => {
    api.tasteGet().then(setTaste, () => setTaste(null));
  }, []);
  if (!taste) return null;
  const notes = taste.notes.map((n) => n.text);
  const save = (list: string[]) => api.tasteSet(list).then(setTaste, () => {});
  return (
    <details className="as-taste">
      <summary className="label">{t("assistant.tasteTitle", { count: notes.length })}</summary>
      <p className="help">{t("assistant.tasteHelp")}</p>
      {notes.length === 0 && <p className="help">{t("assistant.tasteNone")}</p>}
      <ul className="as-found">
        {notes.map((n) => (
          <li key={n} className="row">
            <span className="grow">{n}</span>
            <button className="icon-btn" title={t("assistant.forgetThis")} aria-label={t("assistant.forgetThis")} onClick={() => save(notes.filter((x) => x !== n))}>
              <Icon name="x" />
            </button>
          </li>
        ))}
      </ul>
      <form
        className="row"
        onSubmit={(e) => {
          e.preventDefault();
          if (add.trim()) save([...notes, add.trim()]).then(() => setAdd(""));
        }}
      >
        <input className="input" placeholder={t("assistant.tasteAdd")} value={add} onChange={(e) => setAdd(e.target.value)} />
      </form>
      <NudgeSwitch />
      <CountSwitch />
      {(notes.length > 0 || taste.history.length > 0) && (
        <button className="link-btn align-start" onClick={() => api.tasteForget().then(setTaste, () => {})}>
          {t("assistant.tasteForgetAll")}
        </button>
      )}
    </details>
  );
}

/** HubDeck's suggestion as a calm card: try its sketch, ask the assistant, or let it be. */
function NudgeCard({ nudge, onAsk }: { nudge: Nudge; onAsk: (message: string) => void }) {
  const kind = nudge.kind === "module" ? t(`kind.${nudge.exe}` as Key) : "";
  // a control of the tour's deck: "Knob 3"
  const [ctlKind, ctlIndex] = nudge.exe === "dial" ? ["dial", null] : nudge.exe.split(".");
  const control = nudge.kind === "unused" ? `${t(`control.${ctlKind}` as Key)}${ctlIndex !== null ? ` ${Number(ctlIndex) + 1}` : ""}` : "";
  const text = nudge.kind === "app" ? t("nudge.app", { name: nudge.name, hours: nudge.hours.toFixed(1) }) : nudge.kind === "unused" ? t("nudge.unused", { control, label: nudge.name, deck: nudge.deck ?? "", since: nudge.since ? new Date(`${nudge.since}T12:00:00`).toLocaleDateString(getLanguage(), { weekday: "long", day: "numeric", month: "long" }) : "" }) : t("nudge.module", { kind });
  const ask = nudge.kind === "app" ? t("nudge.askApp", { name: nudge.name }) : nudge.kind === "unused" ? t("nudge.askUnused", { control, label: nudge.name, deck: nudge.deck ?? "" }) : t("nudge.askModule", { kind });
  const act = (action: "later" | "never" | "done") => {
    api.nudgeAct(nudge.subject, action).catch(() => {});
    talk.nudge = null;
    changed();
  };
  const sketch = () =>
    api.assistantSketchApp(nudge.exe, nudge.name, getLanguage()).then((id) => {
      api.tryProfile(id).catch(() => {});
      act("done");
    }, () => {});
  return (
    <div className="as-nudge">
      <p>{text}</p>
      {nudge.kind === "unused" && <p className="help">{t("nudge.unusedHow")}</p>}
      <div className="row wrap">
        {nudge.kind === "app" && (
          <button className="tool small primary" onClick={sketch}>
            <Icon name="play" />
            {t("nudge.sketch")}
          </button>
        )}
        <button className={`tool small${nudge.kind === "unused" ? " primary" : ""}`} onClick={() => (act("done"), onAsk(ask))}>
          <Icon name="sparkles" />
          {t("nudge.ask")}
        </button>
        <button className="link-btn" onClick={() => act("later")}>
          {t("nudge.later")}
        </button>
        <button className="link-btn" onClick={() => act("never")}>
          {t("nudge.never")}
        </button>
      </div>
    </div>
  );
}

/** Suggestions on or off. */
function NudgeSwitch() {
  const [on, setOn] = useState<boolean | null>(null);
  useEffect(() => {
    api.nudgeEnabled().then(setOn, () => setOn(null));
  }, []);
  if (on === null) return null;
  return (
    <label className="row as-switch">
      <input type="checkbox" checked={on} onChange={(e) => (setOn(e.target.checked), api.nudgeEnable(e.target.checked), e.target.checked ? refreshNudge() : ((talk.nudge = null), changed()))} />
      <span>{t("nudge.toggle")}</span>
    </label>
  );
}

/** "Show me on my deck": HubDeck makes the personal tour at once and puts it on the deck; the
 * assistant only walks the user through it. */
function showTour() {
  const text = t("assistant.startShow");
  api.assistantTour(getLanguage()).then(
    (id) => {
      api.tryProfile(id).catch(() => {});
      send(text, `HubDeck made the personal tour draft ${id} (make_tour) and it is on the deck now: walk them through it in three or four short steps.`);
      talk.lines = [...talk.lines, { who: "drafts", ids: [id] }];
      changed();
    },
    () => send(text),
  );
}

export function AssistantPanel({ view, onTour, onBuild }: { view: EngineView; onTour?: () => void; onBuild?: () => void }) {
  useLanguage();
  useSyncExternalStore(subscribe, () => version);
  useEffect(listenOnce, []);
  const [draft, setDraft] = useState("");
  const [first, setFirst] = useState("");
  const [know, setKnow] = useState("");
  // what it runs on: read again after every answer (the session and model appear then)
  const [info, setInfo] = useState<AssistantInfo | null>(null);
  const reload = () => api.assistantInfo(getLanguage()).then(setInfo, () => setInfo(null));
  useEffect(() => {
    reload();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [talk.busy]);
  const ready = !!info?.cli && info.loggedIn;
  const end = useRef<HTMLDivElement>(null);
  // braces: in new WebView2 scrollIntoView returns a promise, and an effect must not return one
  useEffect(() => {
    end.current?.scrollIntoView({ block: "end" });
  }, [talk.lines.length, talk.busy]);
  const state = view.state;
  const drafts = state?.profiles.filter((p) => p.draft) ?? [];

  useEffect(refreshNudge, []);
  // a suggestion's "Ask the assistant": its message waits in the first box
  useEffect(() => {
    if (talk.phase === "consent" && talk.prefill) {
      setFirst(talk.prefill);
      talk.prefill = "";
    }
  }, [talk.phase]);
  // a message from the tour's end: look at the apps at once
  useEffect(() => {
    if (talk.wantScan && ready && talk.phase === "start") {
      talk.wantScan = false;
      scan();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ready]);
  const scan = () => {
    talk.phase = "scanning";
    changed();
    api.assistantScan().then(
      (s) => ((talk.scan = s), (talk.phase = "consent"), changed()),
      (e) => ((talk.phase = "start"), (talk.lines = [{ who: "error", text: String(e) }]), changed()),
    );
  };

  return (
    <div className="assistant">
      <div className="as-head">
        <span className="as-title">
          <Icon name="sparkles" />
          {t("assistant.title")}
        </span>
        {talk.phase !== "start" && (
          <button className="icon-btn" title={t("assistant.new")} aria-label={t("assistant.new")} onClick={startOver}>
            <Icon name="refresh-cw" />
          </button>
        )}
      </div>

      {talk.phase === "chat" && info && <SessionBar info={info} />}

      <div className="as-body">
        {talk.nudge && talk.phase !== "chat" && (
          <NudgeCard
            nudge={talk.nudge}
            onAsk={(message) => {
              talk.prefill = message;
              if (talk.phase === "consent") setFirst(message);
              else if (ready) scan();
            }}
          />
        )}
        {(talk.phase === "start" || talk.phase === "scanning") && (
          // one way in (owner, 2026-10-13: "za dużo na start"): the tour, or straight to the talk
          <div className="as-intro">
            <h3 className="as-lets">{t("assistant.letsSet")}</h3>
            <p className="help">{t("assistant.letsSetHelp")}</p>
            {onTour && (
              <button className="tool primary as-show" onClick={onTour}>
                <Icon name="play" />
                <span className="as-show-words">
                  <b>{t("assistant.showMe")}</b>
                  <small>{t("assistant.showMeHelp")}</small>
                </span>
              </button>
            )}
            <div className="as-or">
              <span>{t("assistant.or")}</span>
            </div>
            <label className="label" htmlFor="as-know">
              {t("assistant.knowWhat")}
            </label>
            <div className="row">
              <input id="as-know" className="input grow" placeholder={t("assistant.knowPlaceholder")} value={know} onChange={(e) => setKnow(e.target.value)} onKeyDown={(e) => e.key === "Enter" && know.trim() && ready && ((talk.prefill = know.trim()), scan())} />
              <button className="tool" aria-label={t("assistant.send")} disabled={!know.trim() || !ready || talk.phase === "scanning"} onClick={() => ((talk.prefill = know.trim()), scan())}>
                <Icon name={talk.phase === "scanning" ? "refresh-cw" : "chevron-right"} />
              </button>
            </div>
            <details className="as-details">
              <summary>
                {ready && info ? t("assistant.runsLine", { plan: planName(info) }) : info ? t("assistant.needsLine") : t("assistant.checking")} · {t("assistant.details")}
              </summary>
              <RunsOn info={info} onCheck={reload} />
              {ready && <p className="help">{t("assistant.privacy")}</p>}
              {ready && <TasteCard />}
              {onBuild && (
                <button className="link-btn align-start" onClick={onBuild}>
                  {t("builder.open")}
                </button>
              )}
            </details>
          </div>
        )}

        {talk.phase === "consent" && talk.scan && (
          <div className="as-consent">
            <b>{t("assistant.sendsTitle")}</b>
            <ul className="as-found">
              <li>
                {t("assistant.foundUsed", { list: talk.scan.used.slice(0, 6).map(([n, h]) => `${n} ${h.toFixed(1)} h`).join(", ") || t("assistant.foundNone") })}
                <small className="as-since">{t("assistant.since", { since: talk.scan.since })}</small>
              </li>
              {talk.scan.recent.length > 0 && <li>{t("assistant.foundRecent", { list: talk.scan.recent.slice(0, 8).join(", ") })}</li>}
              <li>{t("assistant.foundApps", { count: talk.scan.installed, games: talk.scan.games.length })}</li>
              <li>
                {t("assistant.foundDeck", { modules: talk.scan.modules.join(", "), profiles: talk.scan.profiles.length })}
                {talk.scan.detached.length > 0 && <small className="as-since">{t("assistant.detached", { list: talk.scan.detached.join(", ") })}</small>}
              </li>
            </ul>
            <details>
              <summary>{t("assistant.exact")}</summary>
              <pre className="as-pre">{talk.scan.text}</pre>
            </details>
            <p className="help">{t("assistant.notSent")}</p>
            <p className="help">{t("assistant.viaCli")}</p>
            {/* a quick way in, or the user's own words (owner: encourage describing in words) */}
            <span className="label">{t("assistant.startWith")}</span>
            <div className="as-starters">
              {/* the interview first (owner's pick, 2026-10-12): ideas then come from the answers */}
              {(["assistant.startInterview", "assistant.startSurprise", "assistant.startShow", "assistant.startMain"] as Key[]).map((k) => (
                <button key={k} className={`as-starter${k === "assistant.startInterview" ? " is-main" : ""}`} onClick={() => (k === "assistant.startShow" ? showTour() : send(t(k)))}>
                  <Icon name={k === "assistant.startShow" ? "play" : k === "assistant.startMain" ? "app-window" : k === "assistant.startInterview" ? "bot" : "sparkles"} />
                  {t(k)}
                </button>
              ))}
            </div>
            <label className="label" htmlFor="as-first">
              {t("assistant.firstLabel")}
            </label>
            <textarea id="as-first" className="input" rows={3} placeholder={t("assistant.firstPlaceholder")} value={first} onChange={(e) => setFirst(e.target.value)} />
            <div className="row">
              <button className="tool primary" disabled={!first.trim()} onClick={() => send(first)}>
                <Icon name="sparkles" />
                {t("assistant.send")}
              </button>
              <button className="tool ghost" onClick={startOver}>
                {t("assistant.cancel")}
              </button>
            </div>
          </div>
        )}

        {talk.phase === "chat" &&
          talk.lines.map((l, i) =>
            l.who === "work" ? (
              <p key={i} className="as-work">
                <Icon name="settings" />
                {toolWords(l.tools)}
              </p>
            ) : l.who === "permission" ? (
              <Permission key={i} line={l} />
            ) : l.who === "remembered" ? (
              <p key={i} className="as-work is-remembered">
                <Icon name="bot" />
                {t("assistant.remembered", { text: l.text })}
                <button className="link-btn" onClick={() => forget(l.text).then(() => ((talk.lines = talk.lines.filter((x) => x !== l)), changed()))}>
                  {t("assistant.forgetThis")}
                </button>
              </p>
            ) : l.who === "drafts" ? (
              <div key={i} className="as-draft-cards">
                {l.ids.map((id) => (
                  <DraftCard key={id} id={id} view={view} />
                ))}
              </div>
            ) : (
              <div key={i} className={`as-msg is-${l.who}${l.who === "assistant" && l.ideasLine ? " is-cards" : ""}`}>
                {/* while it is written, a tag it may still add at the end is not shown */}
                {(l.who === "assistant" && l.streaming ? l.text.split(/<(?:ideas|choices)>/)[0] : l.text)
                  .split(/\n{2,}/)
                  .filter((para) => para.trim())
                  .map((para, j) => {
                  // "- a" lines (or "intro: - a - b" run together): a bulleted list after its intro
                  const parts = para.split(/(?:^|\s)-\s(?=\S)/);
                  if (parts.length >= 3 && /(^|\n|:)\s*-\s/.test(para)) {
                    const [intro, ...bullets] = parts;
                    return (
                      <Fragment key={j}>
                        {intro.trim() && <p>{bold(intro.trim())}</p>}
                        <ul className="as-list">
                          {bullets.map((x, n) => (
                            <li key={n}>{bold(x.trim())}</li>
                          ))}
                        </ul>
                      </Fragment>
                    );
                  }
                  // "1. ... 2. ..." in one paragraph: shown as a list
                  const items = para.split(/(?:^|\s)(?=\d{1,2}\.\s)/).map((x) => x.trim()).filter(Boolean);
                  return items.length >= 2 && items.every((x) => /^\d{1,2}\.\s/.test(x)) ? (
                    <ol key={j} className="as-list">
                      {items.map((x, n) => (
                        <li key={n}>{bold(x.replace(/^\d{1,2}\.\s/, ""))}</li>
                      ))}
                    </ol>
                  ) : (
                    <p key={j}>{bold(para)}</p>
                  );
                })}
                {l.who === "assistant" && l.sketches && l.sketches.length > 0 && <Sketches sketches={l.sketches} ideas={l.ideas ?? []} view={view} live={!talk.busy && (!!l.ideasLine || i === talk.lines.length - 1)} />}
                {l.who === "assistant" && l.ideas && l.ideas.length > 0 && !l.ideas.some((x) => x.deck) && <IdeaCards ideas={l.ideas} live={i === talk.lines.length - 1 && !talk.busy} />}
                {l.who === "assistant" && l.choices && l.choices.length > 0 && i === talk.lines.length - 1 && !talk.busy && (
                  <div className="as-choices">
                    {l.choices.map((c) => (
                      <button key={c} className="chip" onClick={() => send(c)}>
                        {c}
                      </button>
                    ))}
                  </div>
                )}
              </div>
            ),
          )}
        {talk.phase === "chat" && !talk.busy && talk.choices.length > 0 && <Choices key={talk.choices.join("|")} choices={talk.choices} multiple={talk.multiple} />}
        {talk.busy && (
          <p className="as-work is-live">
            <span className="as-dots" aria-hidden="true" />
            {t("assistant.thinking")}
          </p>
        )}
        <div ref={end} />
      </div>

      {drafts.length > 0 && (
        <details className="as-drafts">
          <summary className="label">{t("assistant.drafts", { count: drafts.length })}</summary>
          {drafts.map((d) => {
            const trying = state?.trying === d.id;
            return (
              <div key={d.id} className={`as-draft${trying ? " is-trying" : ""}`}>
                <span className="as-draft-name" title={d.apps.join(", ")}>
                  {d.name}
                </span>
                {trying ? (
                  <button className="tool small" onClick={() => api.tryProfile(null)}>
                    {t("assistant.back")}
                  </button>
                ) : (
                  <button className="tool small" onClick={() => api.tryProfile(d.id)} title={t("assistant.tryHelp")}>
                    <Icon name="play" />
                    {t("assistant.try")}
                  </button>
                )}
                <button className="tool small primary" onClick={() => api.settleDraft(d.id, true)} title={t("assistant.useHelp")}>
                  <Icon name="check" />
                  {t("assistant.use")}
                </button>
                <button className="icon-btn" title={t("assistant.discard")} aria-label={t("assistant.discard")} onClick={() => api.settleDraft(d.id, false)}>
                  <Icon name="x" />
                </button>
              </div>
            );
          })}
        </details>
      )}

      {talk.phase === "chat" && (
        <form
          className="as-input"
          onSubmit={(e) => {
            e.preventDefault();
            send(draft);
            setDraft("");
          }}
        >
          <input className="input" placeholder={t("assistant.placeholder")} value={draft} disabled={talk.busy} onChange={(e) => setDraft(e.target.value)} />
          {talk.busy ? (
            <button type="button" className="tool" onClick={() => api.assistantStop()}>
              {t("assistant.stop")}
            </button>
          ) : (
            <button type="submit" className="tool primary" disabled={!draft.trim()}>
              {t("assistant.sendShort")}
            </button>
          )}
        </form>
      )}
    </div>
  );
}
