// The tour (owner, 2026-10-13: show, then ask, then build; docs/ASSISTANT-TOUR.md): one question
// about what the person does at the computer (HubDeck suggests the answer), then the deck is made
// at once and shown on the hardware, and the tour walks through it part by part (layout 2A: the
// parts on the left, the part in the middle, the deck on the right). Each part is kept or left out
// and has up to two looks; the finale shows HubDeck's showpieces; the end keeps the deck or throws
// it away. Nothing of the user's changes; Exit puts everything back.
import { useEffect, useMemo, useRef, useState } from "react";

import type { ControlRef, LiveState } from "../deck";
import { api, type EngineView, type TourFocus, type TourStop, type TourSuggestion, type TourView } from "../engine";
import { getLanguage, t, useLanguage, type Key } from "../i18n";
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

export function Tour({ live, view, saved, onPlace, onClose, onBuild, onAsk }: { live: LiveState; view: EngineView; saved: Record<string, Placement>; onPlace: (key: string, p: Placement | null) => void; onClose: () => void; onBuild: () => void; onAsk: (message: string) => void }) {
  useLanguage();
  const [phase, setPhase] = useState<"focus" | "making" | "stops" | "finale" | "end">("focus");
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
  const tryRef = refOf(live, stop?.tryControl ?? (phase === "finale" ? "keys.9" : null));

  // the volume part plays HubDeck's own sound, so turning is heard even when nothing plays
  useEffect(() => {
    if (stop?.id !== "volume") return;
    api.tourSound(true).catch(() => {});
    return () => void api.tourSound(false).catch(() => {});
  }, [stop?.id]);

  // the control to try blinks on the hardware too, wherever its module sits (until it is touched)
  const pointAt = stop?.tryControl ?? (phase === "finale" && tour?.stops.some((s) => s.id === "superAnimations" && s.keep) ? "keys.9" : null);
  useEffect(() => {
    if (!pointAt) return;
    api.tourHighlight(pointAt).catch(() => {});
    return () => void api.tourHighlight(null).catch(() => {});
  }, [pointAt]);

  // the live check: the control of "Try it" was touched on the deck
  useEffect(() => {
    if (stop && same(live.lastTouched, tryRef)) setTried((x) => ({ ...x, [stop.id]: true }));
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
  const set = (id: string, keep: boolean | null, look: number | null) => api.tourSet(id, keep, look).then(setTour, (e) => setError(String(e)));
  const exit = () => {
    done.current = true;
    api.tourExit().finally(onClose);
  };
  const finish = (keep: boolean, home: boolean, then: () => void) => {
    done.current = true;
    api.tourFinish(keep, home).then(then, (e) => setError(String(e)));
  };
  const next = () => (at + 1 < main.length ? setAt(at + 1) : setPhase("finale"));
  const kept = tour ? tour.stops.filter((s) => s.keep && !s.id.startsWith("super")).length : 0;

  const header = (
    <header className="settings-head tour-head">
      <button className="tool ghost" onClick={exit}>
        <Icon name="chevron-left" />
        {t("tour.exit")}
      </button>
      <h2>{phase === "focus" || phase === "making" ? t("tour.title") : t("tour.walkTitle")}</h2>
      {tour && phase !== "end" && <span className="tour-made-for">{t("tour.madeFor", { focus: t(`tour.focus.${tour.focus}` as Key) })}</span>}
    </header>
  );

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
              {phase === "making" ? t("tour.making") : t("tour.make")}
            </button>
          </div>
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
            <span className="tour-count">{t("tour.stopOf", { n: at + 1, count: main.length })}</span>
            <h3>{t(`tour.${stop.id}.title` as Key)}</h3>
            <p className="tour-text">{t((stop.id === "volume" && tour.mac ? "tour.volume.bodyMac" : `tour.${stop.id}.body`) as Key)}</p>
            {at === 0 && <p className="help">{t("tour.firstHint")}</p>}
            <div className="tour-try">
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
            </div>
            {stop.looks > 1 && (
              <div className="tour-looks">
                <span className="tour-label">{t("tour.look")}</span>
                <div className="row wrap">
                  {Array.from({ length: stop.looks }, (_, i) => (
                    <button key={i} className={`tour-look${stop.look === i ? " is-on" : ""}`} aria-pressed={stop.look === i} onClick={() => set(stop.id, null, i)}>
                      {t(`tour.look.${stop.id}.${i}` as Key)}
                    </button>
                  ))}
                </div>
              </div>
            )}
            <div className="tour-nav">
              <label className="check tour-keep">
                <input type="checkbox" checked={stop.keep} onChange={(e) => set(stop.id, e.target.checked, null)} />
                {t("tour.keep")}
              </label>
              <button className="tool primary" onClick={next}>
                {t("tour.next")}
              </button>
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
                  <Toggle on={s.keep} label={t("tour.keepPart", { part: t(`tour.${s.id}.title` as Key) })} onChange={(on) => set(s.id, on, null)} />
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
            <button className="tool primary" onClick={() => finish(true, true, onClose)}>
              <Icon name="check" />
              {t("tour.useHome")}
            </button>
            <button className="tool" onClick={() => finish(true, true, onBuild)}>
              {t("tour.changeSteps")}
            </button>
            <button className="tool ghost" onClick={() => finish(false, false, onClose)}>
              {t("tour.throwAway")}
            </button>
          </div>
          <label className="check tour-count-use">
            <input type="checkbox" checked={counting} onChange={(e) => (setCounting(e.target.checked), api.pressesCountSet(e.target.checked).catch(() => {}))} />
            {t("tour.countUse")}
          </label>
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
