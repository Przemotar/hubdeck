import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api, baseKind, DISCORD, presetLights, presetThumb, PRESETS_HIDDEN, SAVER_FROM_LOOK, themeTitle, controlId, FX_GROUPS, FX_PENDING, FX_RETIRED, GPU_KINDS, fxName, layersOf, SKINS, THEMES, useKeysFrame, type DeckEffects, type EngineView, type FrameCost, type FxKind, type FxLayer, type MyLook, type Profile, type Skin } from "../engine";
import { t, useLanguage, type Key } from "../i18n";
import { Icon } from "./Icon";
import { NoneThumb, Thumb } from "./Thumb";
import { FX_COST, FX_ORDER, FxOptions, skinned } from "./Panels";

/**
 * The Deck look view (owner, 2026-10-28: "edycja może powinna być w dedykowanym oknie … żeby to
 * było mega przyjemne do ustawiania, używania, kombinowania i testowania"; mockups v1-v5 and two UX
 * reviews, https://claude.ai/artifact/YUqpk7baMNg5sePA8e7w8e): a preset to start from, then every
 * element by category, switched on and off; the deck shows the edit live (`effects_try`) and the
 * profile changes only on Apply. On the right the preview of his deck, the smoothness and the
 * elements on, or the settings of the one picked.
 */

type Group = (typeof FX_GROUPS)[number]["group"];
type Cat = Group | "skin";
/** Back to front, then what happens in time (owner: categories, one at a time). */
const ORDER: Cat[] = ["scene", "canvas", "blocks", "light", "inhabitant", "reaction", "skin"];
const DEPTH: Record<Cat, [number, number, number]> = { scene: [0, 0, 1], canvas: [1, 0, 0], blocks: [0, 1, 0], light: [0, 1, 0], inhabitant: [1, 0, 0], reaction: [1, 0, 0], page: [1, 0, 0], finish: [1, 0, 0], skin: [0, 1, 0] };
/** Each preset's mark: its icon on its own colour (a grey line icon did not read, UX review). */
const MARK: Record<string, [string, string]> = {
  "halloween-hill": ["ghost", "#7c4dff"], "halloween-hollow": ["flame", "#f97316"], "japan-tokyo-rain": ["cloud-rain", "#db2777"],
  "halloween-night": ["ghost", "#7c4dff"], "halloween-lanterns": ["flame", "#f97316"], "halloween-spooky": ["skull", "#22a35a"],
  "poland-autumn": ["leaf", "#d4a017"], "poland-hunt": ["trees", "#8a5a2b"], "japan-sakura": ["mountain-snow", "#e0607e"], "japan-koi": ["fish", "#1f8fbf"],
  "matrix-rain": ["binary", "#16a34a"], "dev-editor": ["code", "#3b82f6"], "neon-synthwave": ["sunset", "#ec4899"], "neon-circuit": ["cpu", "#06b6d4"],
  "studio-ink": ["droplets", "#6366f1"], "studio-impasto": ["paintbrush", "#b45309"], "space-nebula": ["sparkles", "#a855f7"], "space-orbit": ["orbit", "#0ea5e9"],
  "music-spectrum": ["audio-lines", "#10b981"], "music-vinyl": ["disc-3", "#e11d48"],
};
/** Presets taken off the list (owner, 2026-10-28: "code editor imo do usunięcia", "oba painters studio do
 * usunięcia", "space in orbit … chyba bym usunął", turntable "bym schował"); their elements stay. */
// The two Halloween ones hidden on 2026-10-28 ("halloween zredukujemy do dwóch opcji") are back (owner,
// 2026-10-29: "przywrócisz mi w programie te nasze halloweenowe presety")
// The three first Halloween ones give way to the two scenes kept for 0.2.0 (owner, 2026-10-29 "1 a",
// 2026-10-30 "2 a"); of the upright copies only those drawn anew for both turns stay (owner, 2026-10-30:
// "Zostawiamy tylko te kopie które obsługują poziom i pion"); the originals of those drawn anew give way to
// them ("2/ b")
const HIDDEN = PRESETS_HIDDEN;
/** Elements taken off the lists (owner, 2026-10-28: the pumpkins "wyglądają brzydko", the frames round the
 * lanterns "tanio wygląda", and the scenes of the presets taken off); shown only where one is still on. */
const GONE: FxKind[] = ["pumpkins", "carved", "code", "ink", "impasto", "orbit", "vinyl",
  // his pass through the elements (2026-10-28): backgrounds "cover aquarium, haunted landscape, ukiyo-e moon";
  // blocks "frames and light (jakoś słabo to wygląda), stone frames"; light "gold, chrome, crystal jakoś
  // dziwnie wyglądają"; presses "heart on like … ukryłbym", "firework … na sylwestra", "color bloom - słaby"
  "cover", "haunt", "ukiyo", "frames", "stone", "gold", "chrome", "crystal", "heart", "firework", "bloom", "flip", "finish"];
/** The mixer of elements only in the dev app, where presets are made (owner, 2026-10-29: "same presety
 * (plus keystyle i ewentualnie light on the icons - ale może te rzeczy powinny być połączone)", "1 a"):
 * people pick a preset, then the keys' style and light in one Keys section. */
const MIXER = import.meta.env.DEV;
/** The lights on the icons offered in the Keys section, one at a time. */
const LIGHTS = ((FX_GROUPS.find((g) => g.group === "light")?.kinds ?? []) as FxKind[]).filter((k) => !FX_RETIRED.includes(k) && !FX_PENDING.includes(k) && !GONE.includes(k));
/** Looks being tried as presets (owner, 2026-10-29: "te nowe testy i iteracje niech będą jako nowe sloty …
 * dorzucać bez nowych buildów"): one per `x-<name>.wgsl` in the shader folder, named in its `trials.json`;
 * only where trial layers run (a test build, `living_trials`). */
type Trial = { id: string; name: string; layers: FxLayer[] };
let TRIALS: Trial[] = [];
const themeOf = (id: string | null) => THEMES.find((x) => x.id === id) ?? TRIALS.find((x) => x.id === id);
const MY_ICONS = ["sparkles", "moon", "sun", "ghost", "flame", "leaf", "music", "palette"];
const BUDGET = 1000 / 24;
const GUARD = 0.8;
/** Kinds that only answer a press: they cost nothing until one. */
const PRESS_ONLY: FxKind[] = (FX_GROUPS.find((g) => g.group === "reaction")?.kinds ?? []) as FxKind[];
const groupOf = (kind: FxKind): Group => FX_GROUPS.find((g) => g.kinds.includes(baseKind(kind)))?.group ?? "canvas";
const usedBy = (kind: FxKind) => THEMES.filter((th) => th.layers.some((l) => l.kind === kind)).map((th) => th.id);
const hue = (kind: string) => [...kind].reduce((h, c) => (h * 31 + c.charCodeAt(0)) % 360, 7);
const themeName = (id: string) => TRIALS.find((x) => x.id === id)?.name ?? themeTitle(id);
const shortName = (id: string) => themeName(id).replace(/^[^:]+:\s*/, "");
const sorted = (layers: FxLayer[]) => [...layers].sort((x, y) => FX_ORDER.indexOf(baseKind(x.kind)) - FX_ORDER.indexOf(baseKind(y.kind)));

interface Draft {
  theme: string | null;
  layers: FxLayer[];
  /** The preset's colours on the lights, and the preset as this profile's screensaver (owner's "1 a",
   * 2026-10-30): on for a preset picked here, as they were for one applied before. */
  presetEdge: boolean;
  presetControls: boolean;
  presetSaver: boolean;
  /** A name once saved as his own look. */
  own?: string;
  changed: boolean;
}
const draftOf = (fx: DeckEffects | undefined, saver?: Profile["saver"]): Draft => ({
  theme: fx?.theme ?? null,
  layers: layersOf(fx).map((l) => ({ ...l })),
  // a preset applied before these switches existed has them off; a profile without one starts with them on
  presetEdge: (!!fx?.lights && (!fx.lightsTo || fx.lightsTo.includes("edge"))) || !fx?.theme,
  presetControls: (!!fx?.lights && (!fx.lightsTo || fx.lightsTo.includes("controls"))) || !fx?.theme,
  presetSaver: saver?.effect === SAVER_FROM_LOOK || !fx?.theme,
  changed: false,
});
const effectsOf = (layers: FxLayer[], d: Draft): DeckEffects => ({ layers: sorted(layers.filter((l) => l.on !== false)), theme: d.theme ?? undefined,
  lights: d.presetEdge || d.presetControls ? presetLights(d.theme) : undefined,
  lightsTo: d.presetEdge && d.presetControls ? undefined : d.presetEdge ? ["edge"] : d.presetControls ? ["controls"] : undefined,
});

function Mark({ id, dim }: { id: string; dim?: boolean }) {
  // an upright copy wears its original's mark
  const m = MARK[id.replace(/-up$/, "")];
  if (!m) return null;
  return (
    <span className={`dl-mark${dim ? " is-dim" : ""}`} style={{ background: m[1] }} title={themeName(id)}>
      <Icon name={m[0]} />
    </span>
  );
}

/** Back with changes not applied: stay, discard or apply. Escape and a click beside it stay. */
function LeaveQuestion({ onStay, onDiscard, onApply }: { onStay: () => void; onDiscard: () => void; onApply: () => void }) {
  const applyRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    applyRef.current?.focus();
    const on = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        onStay();
      }
    };
    window.addEventListener("keydown", on, true);
    return () => window.removeEventListener("keydown", on, true);
  }, [onStay]);
  return (
    <div className="sheet-backdrop confirm-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onStay()}>
      <div className="confirm dl-leave" role="alertdialog" aria-modal="true" aria-labelledby="dl-leave-title" aria-describedby="dl-leave-body">
        <h2 id="dl-leave-title">{t("look.leaveTitle")}</h2>
        <p id="dl-leave-body">{t("look.unsaved")}</p>
        <div className="confirm-foot">
          <button type="button" className="tool ghost" onClick={onStay}>
            {t("look.stay")}
          </button>
          <button type="button" className="tool" onClick={onDiscard}>
            {t("look.discard")}
          </button>
          <button type="button" ref={applyRef} className="tool primary" onClick={onApply}>
            {t("look.apply")}
          </button>
        </div>
      </div>
    </div>
  );
}


/**
 * His deck 1:1 as the keys show it (owner, 2026-10-28: "chcę mieć to oddane 1:1 jak na decku"): the
 * engine's 480x272 picture with only the 15 key windows open (79x75 every 100 and 99 px, his
 * calibration; `PROTOCOL.md`), the gaps the frames hide drawn as the frames. A click on a key presses it.
 * `turn` is how the keys module stands on screen, clockwise (its placement and the base's turn, as the
 * deck view draws it): the panel is turned the same way, so an upright deck is upright here too (owner,
 * 2026-10-30: "jak się weźmie w pion decka to nie przekręca się podgląd").
 */
function Preview({ profile, page, module, turn, onPress }: { profile?: string; page: string | null; module?: string; turn: number; onPress: (key: number) => void }) {
  const frame = useKeysFrame(profile, page, module ?? "");
  const rects = Array.from({ length: 15 }, (_, i) => frame?.rects[i] ?? [(i % 5) * 100, Math.floor(i / 5) * 99, 79, 75]);
  const quarter = (((Math.round(turn / 90) % 4) + 4) % 4) as 0 | 1 | 2 | 3;
  const upright = quarter % 2 === 1;
  // the panel's box is -6..486 by -6..278; each turn moves it back to start at -6, -6
  const place = ["", "translate(272 0) rotate(90)", "translate(480 272) rotate(180)", "translate(0 480) rotate(270)"][quarter];
  return (
    <div className="dl-preview" aria-label={t("look.title")}>
      {frame ? (
        <svg className={`dl-panel${upright ? " is-upright" : ""}`} viewBox={upright ? "-6 -6 284 492" : "-6 -6 492 284"}>
          <defs>
            <clipPath id="dl-windows">
              {rects.map((r, i) => (
                <rect key={i} x={r[0]} y={r[1]} width={r[2]} height={r[3]} rx="2" />
              ))}
            </clipPath>
          </defs>
          <g transform={place}>
            <rect x="-6" y="-6" width="492" height="284" rx="8" className="dl-frame" />
            <image href={frame.src} width="480" height="272" clipPath="url(#dl-windows)" />
            {rects.map((r, i) => (
              <rect key={i} x={r[0]} y={r[1]} width={r[2]} height={r[3]} rx="2" className="dl-hit" role="button" aria-label={`${i + 1}`} onClick={() => onPress(i)} />
            ))}
          </g>
        </svg>
      ) : (
        <p className="help dl-wait">{t("effects.cost.waiting")}</p>
      )}
    </div>
  );
}

export function DeckLook({ view, keysModule, keysTurn = 0, onClose }: { view: EngineView; keysModule?: string; keysTurn?: number; onClose: () => void }) {
  useLanguage();
  const profile = view.profile;
  const page = view.state?.page ?? null;
  const start = useRef<{ effects?: DeckEffects; controls?: Profile["controls"]; id?: string; saver?: Profile["saver"] }>({ effects: profile?.effects, controls: profile?.controls, id: profile?.id, saver: profile?.saver });
  const [draft, setDraft] = useState<Draft>(() => draftOf(profile?.effects, profile?.saver));
  const undo = useRef<Draft[]>([]);
  const [tab, setTab] = useState<"presets" | "elements">("presets");
  const [cat, setCat] = useState<Cat>("scene");
  const [focus, setFocus] = useState<FxKind | null>(null);
  const [hover, setHover] = useState<FxKind | null>(null);
  const hoverTimer = useRef(0);
  const [solo, setSolo] = useState<FxKind | null>(null);
  const [compare, setCompare] = useState(false);
  const [peek, setPeek] = useState(false);
  const [iconsOn, setIconsOn] = useState(true);
  const [dev, setDev] = useState(() => {
    try {
      return localStorage.getItem("hubdeck.lookCost") === "1";
    } catch {
      return false;
    }
  });
  const [guard, setGuard] = useState<{ kind: FxKind; ms: number } | null>(null);
  const [toast, setToast] = useState<string | null>(null);
  const [leaving, setLeaving] = useState(false);
  const [saving, setSaving] = useState<{ name: string; icon: string } | null>(null);
  const [mine, setMine] = useState<MyLook[]>([]);
  const [skinMoved, setSkinMoved] = useState(false);
  const [cost, setCost] = useState<FrameCost | null>(null);
  const [rate, setRate] = useState<{ fps: number; cpu: number | null } | null>(null);
  const [gpu, setGpu] = useState(true);
  const [trials, setTrials] = useState<Trial[]>([]);

  useEffect(() => {
    api.looksList().then((l) => setMine(Array.isArray(l) ? l : [])).catch(() => {});
    api.livingGpu().then(setGpu).catch(() => {});
    api
      .livingTrials()
      .then((list) => {
        TRIALS = list.map(([kind, name]) => ({ id: `trial:${kind}`, name, layers: [{ kind: kind as FxKind }] }));
        setTrials(TRIALS);
      })
      .catch(() => {});
    let alive = true;
    api.frameCost().catch(() => {});
    const tick = () => {
      api.frameCost().then((c) => alive && setCost(c.frames > 0 ? c : null)).catch(() => {});
      api.deckRate().then((r) => alive && setRate(r)).catch(() => {});
    };
    const timer = window.setInterval(tick, 1000);
    return () => {
      alive = false;
      window.clearInterval(timer);
      // the deck goes back to the profile's own look (Apply saved it there)
      api.effectsTry(null).catch(() => {});
    };
  }, []);
  useEffect(() => {
    if (!toast) return;
    const timer = window.setTimeout(() => setToast(null), 3500);
    return () => window.clearTimeout(timer);
  }, [toast]);

  // what the deck shows now: the original preset (Space), one element alone (solo), the mix with
  // the element under the mouse, or the mix; renewed every 10 s (the engine lets a try go after 30)
  const shown = useMemo<DeckEffects>(() => {
    if (compare && draft.theme) {
      const th = themeOf(draft.theme);
      if (th) return { layers: sorted(th.layers), theme: th.id };
    }
    if (compare) return start.current.effects ?? { layers: [] };
    const solo1 = solo && draft.layers.find((l) => l.kind === solo);
    if (solo1) return { layers: [{ ...solo1, on: true }] };
    if (hover && !draft.layers.some((l) => l.kind === hover && l.on !== false)) return effectsOf([...draft.layers, { kind: hover }], draft);
    return effectsOf(draft.layers, draft);
  }, [draft, compare, solo, hover]);
  const bare = !iconsOn || peek;
  useEffect(() => {
    const send = () => api.effectsTry(shown, bare).catch(() => {});
    send();
    const timer = window.setInterval(send, 10000);
    return () => window.clearInterval(timer);
  }, [shown, bare]);

  const edit = useCallback((next: (d: Draft) => Draft) => {
    setDraft((d) => {
      undo.current = [...undo.current, d].slice(-100);
      return { ...next(d), changed: true };
    });
  }, []);
  const doUndo = () => {
    const last = undo.current.pop();
    if (last) setDraft(last);
  };
  const isOn = (kind: FxKind) => draft.layers.some((l) => l.kind === kind && l.on !== false);
  const measured = (kind: string) => cost?.layers.find((l) => l.kind === kind)?.ms;
  const msOf = (kind: FxKind) => measured(kind) ?? FX_COST[kind]?.ms ?? 0.5;
  const total = (layers: FxLayer[]) => layers.filter((l) => l.on !== false).reduce((s, l) => s + msOf(l.kind), 0);

  const switchOn = (kind: FxKind, force = false) => {
    const g = groupOf(kind);
    const after = draft.layers.filter((l) => l.on !== false && l.kind !== kind && !(g === "page" && groupOf(l.kind) === "page"));
    const ms = total(after) + msOf(kind);
    if (!force && ms > BUDGET * GUARD) return setGuard({ kind, ms });
    setGuard(null);
    edit((d) => {
      let layers = d.layers.map((l) => ({ ...l }));
      const swapped = g === "page" ? layers.find((l) => l.on !== false && l.kind !== kind && groupOf(l.kind) === "page") : undefined;
      if (swapped) swapped.on = false;
      const have = layers.find((l) => l.kind === kind);
      if (have) have.on = true;
      else layers = [...layers, { kind }];
      // at most two backgrounds: a third switches off the oldest one
      const scenes = layers.filter((l) => l.on !== false && groupOf(l.kind) === "scene");
      if (g === "scene" && scenes.length > 2) {
        const old = scenes.find((l) => l.kind !== kind);
        if (old) {
          old.on = false;
          setToast(t("look.muted3", { name: fxName(old.kind) }));
        }
      }
      if (swapped) setToast(t("look.swapped", { old: fxName(swapped.kind), new: fxName(kind) }));
      return { ...d, layers };
    });
    setFocus(kind);
  };
  const switchOff = (kind: FxKind) => edit((d) => ({ ...d, layers: d.layers.map((l) => (l.kind === kind ? { ...l, on: false } : l)) }));
  const toggle = (kind: FxKind) => (isOn(kind) ? switchOff(kind) : switchOn(kind));
  const choosePreset = (id: string | null) => {
    setGuard(null);
    setFocus(null);
    setDraft((d) => {
      undo.current = [...undo.current, d].slice(-100);
      const th = themeOf(id);
      const layers = th ? th.layers.map((l) => ({ ...l })) : [];
      // the light on the icons belongs to the keys: it stays across presets unless the preset brings its own
      const keep = layers.some((l) => LIGHTS.includes(l.kind)) ? [] : d.layers.filter((l) => l.on !== false && LIGHTS.includes(l.kind)).map((l) => ({ ...l }));
      // a preset picked here brings its colours and its screensaver, on until switched off (his "1 a")
      return { theme: id, layers: sorted([...layers, ...keep]), presetEdge: true, presetControls: true, presetSaver: true, changed: false };
    });
  };
  const light = draft.layers.find((l) => l.on !== false && LIGHTS.includes(l.kind));
  /** One light on the icons or none; the preset stays picked (the light is the keys', not a change of it). */
  const shine = (kind: FxKind | null) =>
    setDraft((d) => {
      undo.current = [...undo.current, d].slice(-100);
      const had = d.layers.find((l) => l.kind === kind);
      return { ...d, layers: sorted([...d.layers.filter((l) => !LIGHTS.includes(l.kind)), ...(kind ? [{ ...(had ?? { kind }), on: true }] : [])]) };
    });
  const chooseMine = (m: MyLook) => {
    setFocus(null);
    edit((d) => ({ theme: m.base || null, layers: layersOf(m.effects).map((l) => ({ ...l })), presetEdge: d.presetEdge, presetControls: d.presetControls, presetSaver: d.presetSaver, own: m.name, changed: true }));
    if (m.skin !== undefined) wear((m.skin || undefined) as Skin | undefined);
  };

  // the key style of the page shown: worn by the keys themselves (the profile changes at once;
  // Discard puts the keys back)
  const ids = profile ? Array.from({ length: 15 }, (_, i) => controlId("keys", i, page)).filter((id) => profile.controls[id]) : [];
  const looks = ids.map((id) => profile?.controls[id].look ?? {});
  const skin = looks.length && looks.every((l) => l.skin === looks[0].skin) ? (looks[0].skin ?? null) : null;
  const wear = (s: Skin | undefined) => {
    if (!profile || !ids.length) return;
    const controls = { ...profile.controls };
    for (const id of ids) controls[id] = { ...controls[id], look: skinned(controls[id].look ?? {}, s) };
    setSkinMoved(true);
    api.saveProfile({ ...profile, controls }).catch((e) => setToast(String(e)));
  };

  // knobs and faders with a colour of their own: they keep it over the preset (owner's "2 a", 2026-10-30)
  const ownLeds = profile
    ? Object.entries(profile.controls)
        .filter(([id, b]) => /^(knobs|faders)\.\d+$/.test(id) && b.look?.led)
        .map(([id, b]) => ({ id, color: b.look!.led!, name: b.look?.label || id.replace(/^knobs\./, `${t("control.knobs")} `).replace(/^faders\./, `${t("control.faders")} `) }))
    : [];
  const backToPreset = (ids: string[]) => {
    if (!profile) return;
    const controls = { ...profile.controls };
    for (const id of ids) controls[id] = { ...controls[id], look: { ...controls[id].look, led: undefined, ledEffect: undefined, ledSpeed: undefined } };
    api.saveProfile({ ...profile, controls }).catch((e) => setToast(String(e)));
  };
  // the preset as this profile's own screensaver, or back to what it had (the shared one) when switched off
  const saverOf = (d: Draft): Profile["saver"] =>
    d.presetSaver && d.theme ? { effect: SAVER_FROM_LOOK, afterMinutes: start.current.saver?.afterMinutes } : start.current.saver?.effect === SAVER_FROM_LOOK ? undefined : start.current.saver;
  const dirty =
    JSON.stringify(effectsOf(draft.layers, draft)) !== JSON.stringify(effectsOf(layersOf(start.current.effects), draftOf(start.current.effects, start.current.saver))) ||
    JSON.stringify(saverOf(draft) ?? null) !== JSON.stringify(start.current.saver ?? null) ||
    skinMoved;
  const apply = () => {
    if (!profile) return;
    const fx = effectsOf(draft.layers, draft);
    const saver = saverOf(draft);
    api
      .saveProfile({ ...profile, effects: fx.layers?.length ? fx : undefined, saver })
      .then(() => {
        start.current = { effects: fx.layers?.length ? fx : undefined, controls: profile.controls, id: profile.id, saver };
        setDraft((d) => ({ ...d, layers: d.layers.filter((l) => l.on !== false), changed: false }));
        setSkinMoved(false);
        undo.current = [];
        setToast(t("look.applied"));
      })
      .catch((e) => setToast(String(e)));
  };
  const discard = () => {
    setDraft(draftOf(start.current.effects, start.current.saver));
    undo.current = [];
    setFocus(null);
    if (skinMoved && profile && start.current.controls) api.saveProfile({ ...profile, controls: start.current.controls }).catch(() => {});
    setSkinMoved(false);
    setToast(t("look.discarded"));
  };
  const back = () => (dirty ? setLeaving(true) : onClose());

  // Space shows the original and I hides the icons while held, taken before any button sees them
  // (a focused button pressed itself again, UX review); Ctrl+Z undoes; Escape closes the settings
  useEffect(() => {
    const typing = (e: KeyboardEvent) => !!(e.target as HTMLElement)?.closest?.("input[type=text], textarea, select");
    const down = (e: KeyboardEvent) => {
      if (saving || typing(e)) return;
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "z") return e.preventDefault(), doUndo();
      if (e.key === " ") return e.preventDefault(), e.stopPropagation(), !e.repeat && setCompare(true);
      if (e.key.toLowerCase() === "i" && !e.ctrlKey && !e.altKey) return !e.repeat && setPeek(true);
      if (e.key === "Escape") return focus ? setFocus(null) : back();
    };
    const up = (e: KeyboardEvent) => {
      if (e.key === " ") e.preventDefault(), e.stopPropagation(), setCompare(false);
      if (e.key.toLowerCase() === "i") setPeek(false);
    };
    window.addEventListener("keydown", down, true);
    window.addEventListener("keyup", up, true);
    return () => {
      window.removeEventListener("keydown", down, true);
      window.removeEventListener("keyup", up, true);
    };
  });

  const enterTile = (kind: FxKind) => {
    window.clearTimeout(hoverTimer.current);
    hoverTimer.current = window.setTimeout(() => setHover(kind), 150);
  };
  const leaveTile = () => {
    window.clearTimeout(hoverTimer.current);
    hoverTimer.current = window.setTimeout(() => setHover(null), 300);
  };

  const costChip = (kind: FxKind) => {
    const ms = msOf(kind);
    const est = measured(kind) === undefined;
    const press = PRESS_ONLY.includes(kind);
    const level = ms < 1 ? "l" : ms < 2.5 ? "m" : "h";
    return (
      <span className={`dl-cost is-${level}`} title={t("look.costShare", { ms: ms.toFixed(2), pct: Math.round((ms / BUDGET) * 100), where: t(GPU_KINDS.includes(kind) ? "look.gpu" : "look.cpu") }) + (est ? ` (${t("look.estimate")})` : "")}>
        {press ? t("look.onPress") : `${est ? "~" : ""}${ms.toFixed(1)} ms · ${Math.round((ms / BUDGET) * 100)}%`}
        <em>{GPU_KINDS.includes(kind) ? "GPU" : "CPU"}</em>
      </span>
    );
  };

  if (!profile) return null;
  const on = draft.layers.filter((l) => l.on !== false);
  const ms = cost ? cost.layersMs : total(draft.layers);
  const share = ms / BUDGET;
  // what the deck really sends a second, while it sends (a copy without a deck sends nothing)
  const fps = rate && rate.fps > 0 ? Math.min(24, rate.fps) : share > 1 ? Math.floor(24 / share) : 24;
  const ghost = hover && !isOn(hover) ? msOf(hover) : 0;
  const focusLayer = focus ? draft.layers.find((l) => l.kind === focus) : undefined;
  const title = draft.own ?? (draft.changed || !draft.theme ? (draft.theme ? t("look.own") : t("look.none")) : themeName(draft.theme));

  const rail = ORDER.map((g) => {
    const kinds = g === "skin" ? [] : (FX_GROUPS.find((x) => x.group === g)?.kinds ?? []).filter((k) => (!FX_RETIRED.includes(k) && !FX_PENDING.includes(k) && !GONE.includes(k)) || isOn(k));
    const count = g === "skin" ? SKINS.length : kinds.length;
    const n = g === "skin" ? (skin ? 1 : 0) : kinds.filter(isOn).length;
    return { g, kinds, count, n };
  });
  const current = rail.find((r) => r.g === cat)!;
  const skinTiles = [undefined, ...SKINS].map((s) => {
    const sel = (skin ?? undefined) === s;
    return (
      <button key={s ?? "none"} className={`dl-tile dl-el${sel ? " is-on" : ""}`} role="radio" aria-checked={sel} onClick={() => !sel && wear(s)}>
        {s ? <Thumb id={`S-${s}`} /> : <span className="dl-thumb" />}
        <span className="dl-row">
          <span className="dl-name">{s ? t(`skin.${s}` as Key) : t("look.skinNone")}</span>
          <span className="dl-check" aria-hidden="true">
            <Icon name="check" />
          </span>
        </span>
      </button>
    );
  });

  return (
    <div className="studio dl" role="region" aria-label={t("look.title")}>
      <div className="studio-bar">
        <button className="tool small ghost" onClick={back}>
          <Icon name="chevron-left" />
          {t("look.back")}
        </button>
        <span className="studio-title">
          {t("look.title")} <span className="tx2">· {profile.name}</span>
        </span>
        <span className="dl-live">{t("look.live")}</span>
        <span className="spacer" />
        <button className="tool small ghost" onClick={doUndo} disabled={!undo.current.length} title="Ctrl+Z">
          <Icon name="undo" />
          {t("look.undo")}
        </button>
        <button className="tool small" onClick={() => setSaving({ name: draft.own ?? t("look.myName", { name: draft.theme ? shortName(draft.theme) : t("look.own") }), icon: MY_ICONS[0] })}>
          <Icon name="save" />
          {t("look.save")}
        </button>
        <button className="tool small" onClick={discard} disabled={!dirty}>
          {t("look.discard")}
        </button>
        <button className="tool small primary" onClick={apply} disabled={!dirty}>
          {t("look.apply")}
        </button>
      </div>
      {/* a question over the view, not a bar that pushes it down (owner, 2026-10-30: "nie podoba mi
          sie ze jak sie cofam z living decku to tam pokazuje sie na gorze linijka", his "1 b") */}
      {leaving && <LeaveQuestion onStay={() => setLeaving(false)} onDiscard={() => (discard(), onClose())} onApply={() => (apply(), onClose())} />}
      <div className="dl-grid">
        <section className="studio-pane dl-left">
          {MIXER && (
            <div className="dl-tabs" role="tablist">
              <button role="tab" aria-selected={tab === "presets"} className={tab === "presets" ? "is-on" : ""} onClick={() => setTab("presets")}>
                {t("dl.presets")}
              </button>
              <button role="tab" aria-selected={tab === "elements"} className={tab === "elements" ? "is-on" : ""} onClick={() => setTab("elements")}>
                {t("look.elements", { n: on.length })}
              </button>
            </div>
          )}
          {!gpu && <p className="help warn">{t("effects.noGpu")}</p>}
          {tab === "presets" || !MIXER ? (
            <>
              {mine.length > 0 && (
                <div className="field">
                  <span className="label">{t("look.mine")}</span>
                  <div className="dl-tiles">
                    {mine.map((m) => (
                      <button key={m.name} className={`dl-tile${draft.own === m.name ? " is-sel" : ""}`} onClick={() => chooseMine(m)}>
                        <Thumb id={m.base} />
                        <span className="dl-row">
                          <span className="dl-name">{m.name}</span>
                          <span className="dl-mark" style={{ background: MARK[m.base]?.[1] ?? "var(--acc)" }}>
                            <Icon name={m.icon} />
                          </span>
                        </span>
                      </button>
                    ))}
                  </div>
                </div>
              )}
              <div className="field">
                <span className="label">{t("dl.presets")}</span>
                <div className="dl-tiles">
                  {/* "none" first (owner, 2026-10-30: "opcja none nie powinna być … jako pierwsza?") */}
                  <button className={`dl-tile dl-none${!draft.theme && !on.length ? " is-sel" : ""}`} onClick={() => choosePreset(null)}>
                    <NoneThumb label={t("look.none")} />
                    <span className="dl-row">
                      <span className="dl-name">{t("look.none")}</span>
                    </span>
                  </button>
                  {[...THEMES.filter((th) => !HIDDEN.has(th.id) || draft.theme === th.id), ...trials].map((th) => {
                    const sel = draft.theme === th.id && !draft.changed && !draft.own;
                    return (
                      <button key={th.id} className={`dl-tile${sel ? " is-sel" : ""}`} onClick={() => choosePreset(th.id)} title={themeName(th.id)}>
                        {/* an upright copy lying draws what its original draws: the same thumbnail; one
                            drawn anew (Fuji, koi, the mushroom hunt) has its own */}
                        <Thumb id={presetThumb(th.id)} moving={sel} />
                        {/* no mark on the tile: the thumbnail says it (owner, 2026-10-30: "nie potrzebujemy już ikonek na presetach") */}
                        <span className="dl-row">
                          <span className="dl-name">{shortName(th.id)}</span>
                        </span>
                      </button>
                    );
                  })}
                </div>
                {/* one line, where people want to say something about a preset (owner's "2 a", 2026-10-30) */}
                <p className="help dl-discord">
                  {t("look.discord")}{" "}
                  <button type="button" className="link-btn" onClick={() => api.openLink(DISCORD).catch(() => {})}>
                    {t("look.discordLink")}
                  </button>
                </p>
              </div>
              <div className="field dl-keys">
                <span className="label">{t("look.keys")}</span>
                <span className="help">{t("look.keys.help")}</span>
                <span className="dl-sub">{t("look.keys.style")}</span>
                <div className="dl-tiles" role="radiogroup" aria-label={t("look.keys.style")}>
                  {skinTiles}
                </div>
                <span className="dl-sub">{t("look.keys.light")}</span>
                <div className="dl-tiles" role="radiogroup" aria-label={t("look.keys.light")}>
                  {[null, ...LIGHTS].map((k) => {
                    const sel = (light?.kind ?? null) === k;
                    return (
                      <button key={k ?? "none"} className={`dl-tile dl-el${sel ? " is-on" : ""}`} role="radio" aria-checked={sel} onClick={() => !sel && shine(k)}>
                        {k ? <Thumb id={`L-${k}`} /> : <span className="dl-thumb" />}
                        <span className="dl-row">
                          <span className="dl-name">{k ? fxName(k) : t("look.lightNone")}</span>
                          <span className="dl-check" aria-hidden="true">
                            <Icon name="check" />
                          </span>
                        </span>
                      </button>
                    );
                  })}
                </div>
                {light && (
                  <div className="dl-insp">
                    <FxOptions layer={light} onChange={(l) => setDraft((d) => ({ ...d, layers: d.layers.map((x) => (x.kind === l.kind ? { ...l, on: x.on } : x)) }))} />
                  </div>
                )}
              </div>
            </>
          ) : (
            <div className="dl-elems">
              <nav className="dl-rail" aria-label={t("look.elements", { n: on.length })}>
                {rail.map((r) => (
                  <button key={r.g} className={r.g === cat ? "is-on" : ""} onClick={() => (setCat(r.g), setGuard(null))}>
                    <span className="dl-depth" aria-hidden="true">
                      {DEPTH[r.g].map((d, i) => (
                        <i key={i} className={d ? "is-on" : ""} />
                      ))}
                    </span>
                    <span className="dl-rn">
                      {r.g === "skin" ? t("look.cat.skin") : t(`effects.${r.g}` as Key)}
                      <span className="tx2">{t("look.count", { count: r.count })}</span>
                    </span>
                    {r.n > 0 && <b className="dl-rc">{r.n}</b>}
                  </button>
                ))}
              </nav>
              <div className="dl-cat">
                <div className="dl-cathead">
                  <div>
                    <strong>{cat === "skin" ? t("look.cat.skin") : t(`effects.${cat}` as Key)}</strong>
                    <span className="help">{t((cat === "skin" ? "look.cat.skinHelp" : `look.cat.${cat}`) as Key)}</span>
                  </div>
                  {(cat === "scene" || cat === "page" || cat === "skin") && <span className="dl-rule">{t(cat === "scene" ? "look.rule.two" : "look.rule.one")}</span>}
                </div>
                {guard && (
                  <div className="dl-guard" role="alert">
                    <Icon name="alert" />
                    <span>{t("look.guard", { name: fxName(guard.kind), ms: msOf(guard.kind).toFixed(1), total: guard.ms.toFixed(1) })}</span>
                    <span className="spacer" />
                    <button className="tool small" onClick={() => setGuard(null)}>
                      {t("look.guardNo")}
                    </button>
                    <button className="tool small primary" onClick={() => switchOn(guard.kind, true)}>
                      {t("look.guardYes")}
                    </button>
                  </div>
                )}
                <div className="dl-tiles">
                  {cat === "skin"
                    ? skinTiles
                    : current.kinds.map((k) => {
                        const kOn = isOn(k);
                        return (
                          <div
                            key={k}
                            className={`dl-tile dl-el${kOn ? " is-on" : ""}${focus === k ? " is-focus" : ""}`}
                            tabIndex={0}
                            onClick={() => (kOn ? setFocus(k) : switchOn(k))}
                            onKeyDown={(e) => (e.key === "Enter" ? setFocus(k) : e.key === " " ? (e.preventDefault(), e.stopPropagation(), toggle(k)) : undefined)}
                            onMouseEnter={() => enterTile(k)}
                            onMouseLeave={leaveTile}
                          >
                            <Thumb id={`L-${k}`} />
                            <span className="dl-row">
                              <span className="dl-name">{fxName(k)}</span>
                              <span className="dl-marks">
                                {usedBy(k).map((id) => (
                                  <Mark key={id} id={id} dim={id !== draft.theme} />
                                ))}
                              </span>
                              <button
                                type="button"
                                className="dl-check"
                                role="checkbox"
                                aria-checked={kOn}
                                aria-label={`${fxName(k)}: ${kOn ? t("look.switchOff") : t("look.switchOn")}`}
                                onClick={(e) => (e.stopPropagation(), toggle(k))}
                              >
                                <Icon name="check" />
                              </button>
                            </span>
                            {dev && <span className="dl-row">{costChip(k)}</span>}
                          </div>
                        );
                      })}
                </div>
              </div>
            </div>
          )}
        </section>
        <section className="studio-pane dl-right">
          <Preview profile={profile.id} page={page} module={keysModule} turn={keysTurn} onPress={(i) => api.effectsPress(i).catch(() => {})} />
          <div className="dl-under">
            <button className="tool small" onClick={() => setIconsOn((v) => !v)} aria-pressed={!iconsOn}>
              <Icon name={iconsOn ? "eye" : "eye-off"} />
              {t(iconsOn ? "look.icons.on" : "look.icons.off")}
            </button>
            <button className="tool small" onPointerDown={() => setCompare(true)} onPointerUp={() => setCompare(false)} onPointerLeave={() => setCompare(false)}>
              {t("look.compare")}
            </button>
            <span className="help">{compare ? t("look.comparing", { name: draft.theme ? themeName(draft.theme) : t("look.none") }) : solo ? t("look.solo.on", { name: fxName(solo) }) : hover && !isOn(hover) ? t("look.hover", { name: fxName(hover) }) : t("look.hint")}</span>
          </div>
          <div className="field">
            <div className="dl-meterhead">
              <span className="label">{t("look.smooth")}</span>
              <span className="spacer" />
              <b className={`mono is-${share < 0.5 ? "l" : share < GUARD ? "m" : "h"}`} title={cost ? t("look.measured") : t("look.estimate")}>
                {t("look.meter", { fps: fps.toFixed(0), ms: ms.toFixed(1), free: Math.max(0, Math.round((1 - share) * 100)) })}
              </b>
            </div>
            <div className="dl-meter">
              {on.map((l) => (
                <i key={l.kind} style={{ width: `${(msOf(l.kind) / BUDGET) * 100}%`, background: `hsl(${hue(l.kind)} 70% 62%)` }} title={`${fxName(l.kind)}: ${msOf(l.kind).toFixed(1)} ms`} />
              ))}
              {ghost > 0 && <i className="is-ghost" style={{ width: `${(ghost / BUDGET) * 100}%` }} />}
            </div>
            {MIXER && (
              <label className="dl-devsw">
                <input type="checkbox" checked={dev} onChange={(e) => (setDev(e.target.checked), localStorage.setItem("hubdeck.lookCost", e.target.checked ? "1" : "0"))} />
                {t("look.cost")}
              </label>
            )}
          </div>
          <div className="field">
            <div className="dl-mixhead">
              <span className="label">{t("look.active")}</span>
              <span className="tx2 dl-mixtitle">
                {title}
                {draft.changed && draft.theme ? ` · ${t("look.basedOn", { name: shortName(draft.theme) })}` : ""}
              </span>
              <span className="spacer" />
              {draft.theme && draft.changed && (
                <button className="tool small ghost" onClick={() => choosePreset(draft.theme)}>
                  <Icon name="rotate-ccw" />
                  {t("look.reset")}
                </button>
              )}
            </div>
            {!MIXER ? null : focusLayer ? (
              <>
                <button className="dl-oneline" onClick={() => setFocus(null)}>
                  {t("look.activeLine", { n: on.length, names: on.map((l) => fxName(l.kind)).join(", ") })}
                </button>
                <div className="dl-insp">
                  <div className="dl-insphead">
                    <strong>{fxName(focusLayer.kind)}</strong>
                    <span className="dl-marks">
                      {usedBy(focusLayer.kind).map((id) => (
                        <Mark key={id} id={id} dim={id !== draft.theme} />
                      ))}
                    </span>
                    <span className="spacer" />
                    {costChip(focusLayer.kind)}
                    <button className="tool small ghost" onClick={() => setFocus(null)} aria-label={t("look.close")}>
                      <Icon name="x" />
                    </button>
                  </div>
                  <FxOptions layer={focusLayer} onChange={(l) => edit((d) => ({ ...d, layers: d.layers.map((x) => (x.kind === l.kind ? { ...l, on: x.on } : x)) }))} />
                </div>
              </>
            ) : on.length || draft.layers.length ? (
              <div className="dl-mix">
                {sorted(draft.layers).map((l) => {
                  const lOn = l.on !== false;
                  return (
                    <div key={l.kind} className={`dl-mrow${lOn ? "" : " is-off"}`} onClick={() => (setFocus(l.kind), setCat(groupOf(l.kind)), setTab("elements"))}>
                      <span className="dl-sw" style={{ background: `hsl(${hue(l.kind)} 70% 62%)` }} />
                      <span className="dl-mname">
                        <span>{fxName(l.kind)}</span>
                        {usedBy(l.kind)
                          .filter((id) => id === draft.theme)
                          .map((id) => (
                            <Mark key={id} id={id} />
                          ))}
                      </span>
                      <span className="tx2 dl-mcat">{t(`effects.${groupOf(l.kind)}` as Key)}</span>
                      <span className="mono dl-mms">{lOn ? (PRESS_ONLY.includes(l.kind) ? t("look.onPress") : `${msOf(l.kind).toFixed(1)} ms`) : "—"}</span>
                      <button
                        type="button"
                        className={`dl-mb${solo === l.kind ? " is-on" : ""}`}
                        title={t("look.solo")}
                        aria-label={t("look.solo")}
                        onClick={(e) => e.stopPropagation()}
                        onPointerDown={(e) => (e.stopPropagation(), setSolo(l.kind))}
                        onPointerUp={() => setSolo(null)}
                        onPointerLeave={() => setSolo((s) => (s === l.kind ? null : s))}
                        onKeyDown={(e) => e.key === "Enter" && setSolo(l.kind)}
                        onKeyUp={(e) => e.key === "Enter" && setSolo(null)}
                      >
                        <Icon name="search" />
                      </button>
                      <button type="button" className={`dl-mb${lOn ? "" : " is-off"}`} title={lOn ? t("look.switchOff") : t("look.switchOn")} aria-label={lOn ? t("look.switchOff") : t("look.switchOn")} onClick={(e) => (e.stopPropagation(), toggle(l.kind))}>
                        <Icon name={lOn ? "eye" : "eye-off"} />
                      </button>
                    </div>
                  );
                })}
              </div>
            ) : (
              <p className="help">{t("look.empty")}</p>
            )}
            {/* the preset on the rest of the deck too (owner's "1 a", 2026-10-30): its colours on the lights and it
                as the screensaver; the lights' style, the edge's effect and the screensaver's minutes stay his,
                set where they always are */}
            {draft.theme && presetLights(draft.theme) && (
              <div className="dl-also">
                <div className="dl-also-row">
                  <span className="dl-also-sw">
                    {presetLights(draft.theme)!.map((c) => (
                      <i key={c} style={{ background: c }} />
                    ))}
                  </span>
                  {t("look.presetLights")}
                </div>
                <div className="dl-also-parts">
                  <label className="check">
                    <input type="checkbox" checked={draft.presetEdge} onChange={(e) => edit((d) => ({ ...d, presetEdge: e.target.checked }))} />
                    {t("look.presetEdge")}
                  </label>
                  <label className="check">
                    <input type="checkbox" checked={draft.presetControls} onChange={(e) => edit((d) => ({ ...d, presetControls: e.target.checked }))} />
                    {t("look.presetControls")}
                  </label>
                </div>
                {/* knobs and faders with their own colour keep it; each can go back to the preset here */}
                {draft.presetControls && ownLeds.length > 0 && (
                  <div className="dl-also-own">
                    {ownLeds.map((o) => (
                      <span key={o.id} className="dl-own">
                        <i style={{ background: o.color }} />
                        {o.name}: {t("look.ownLed")}
                        <button type="button" className="link-btn" onClick={() => backToPreset([o.id])}>
                          {t("look.backToPreset")}
                        </button>
                      </span>
                    ))}
                    {ownLeds.length > 1 && (
                      <button type="button" className="link-btn" onClick={() => backToPreset(ownLeds.map((o) => o.id))}>
                        {t("look.allBackToPreset")}
                      </button>
                    )}
                  </div>
                )}
                <label className="check">
                  <input type="checkbox" checked={draft.presetSaver} onChange={(e) => edit((d) => ({ ...d, presetSaver: e.target.checked }))} />
                  {t("look.presetSaver")}
                </label>
                <p className="help">{t("look.presetAlsoHelp")}</p>
              </div>
            )}
          </div>
        </section>
      </div>
      {saving && (
        <div className="dl-pop" onClick={() => setSaving(null)}>
          <div role="dialog" aria-label={t("look.saveTitle")} onClick={(e) => e.stopPropagation()}>
            <strong>{t("look.saveTitle")}</strong>
            <label className="field">
              <span className="label">{t("dl.name")}</span>
              <input
                type="text"
                autoFocus
                maxLength={40}
                value={saving.name}
                onChange={(e) => setSaving({ ...saving, name: e.target.value })}
                onKeyDown={(e) => e.key === "Enter" && saveMine(mine.some((m) => m.name === saving.name.trim()))}
              />
            </label>
            <span className="label">{t("dl.icon")}</span>
            <div className="dl-icpick">
              {MY_ICONS.map((n) => (
                <button key={n} type="button" className={saving.icon === n ? "is-on" : ""} onClick={() => setSaving({ ...saving, icon: n })} aria-label={n}>
                  <Icon name={n} />
                </button>
              ))}
            </div>
            {mine.some((m) => m.name === saving.name.trim()) && <p className="help">{t("look.exists")}</p>}
            <div className="dl-row">
              <span className="spacer" />
              <button className="tool small" onClick={() => setSaving(null)}>
                {t("look.cancel")}
              </button>
              {mine.some((m) => m.name === saving.name.trim()) && (
                <button className="tool small" onClick={() => saveMine(true)}>
                  {t("look.replace")}
                </button>
              )}
              <button className="tool small primary" onClick={() => saveMine(false)}>
                {t("look.saveNew")}
              </button>
            </div>
          </div>
        </div>
      )}
      {toast && (
        <div className="toast" role="status">
          {toast}
        </div>
      )}
    </div>
  );

  function saveMine(over: boolean) {
    if (!saving) return;
    let name = saving.name.trim() || t("look.own");
    if (!over) {
      const base = name;
      for (let n = 2; mine.some((m) => m.name === name); n++) name = `${base} ${n}`;
    }
    const look: MyLook = { name, icon: saving.icon, base: draft.theme ?? "", effects: effectsOf(draft.layers, draft), skin: skin ?? "" };
    const next = [look, ...mine.filter((m) => m.name !== name)];
    setMine(next);
    setDraft((d) => ({ ...d, own: name }));
    setSaving(null);
    api.looksSave(next).then(() => setToast(t("look.saved", { name }))).catch((e) => setToast(String(e)));
  }
}
