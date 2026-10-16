import { useEffect, useId, useMemo, useRef, useState, type CSSProperties, type ReactNode } from "react";
import "../deck.css";
import { controlKey, type ControlRef, type LiveState, type ModuleInfo, type ModuleKind } from "../deck";
import { FADER_COLORS, KNOB_COLORS } from "../identify";
import { api, controlId, useDialFrame, useKeysFrame, useLeds, useLiveLooks, usePreview, useSaverShown, useTicker, type Binding, type LedsShown, type LedView, type Look, type Profile } from "../engine";
import { localizeError, t, useLanguage, type Key } from "../i18n";
import { clampToGrid, detachedKinds, detachedModule, FOOTPRINT, GRID_COLS, GRID_ROWS, keysOfKind, MODULE_KINDS, ownedKey, placeModules, placementKey, rotatedSize, type Placed, type Placement, type Rotation } from "../layout";
import { saverBody } from "../saver";

/*
 * The live deck, drawn as the hardware seen from above (look variant B, chosen 2026-10-11):
 * a base with its light edge, modules with glass keys, a metal dial ring, knobs and faders with
 * their LEDs. Everything is one SVG in base-grid units (100 per cell), so any placement, module
 * rotation and base turn is a transform. A second SVG behind it holds the static, filter-heavy
 * layers (edge glow, board), so live updates do not repaint them.
 */

const U = 100; // one base cell
const MARGIN = 20; // board rim around the grid
const PAD = 14; // room around the board; the glow may spill past the drawing
const GW = GRID_COLS * U;
const GH = GRID_ROWS * U;
const CX = GW / 2;
const CY = GH / 2;

// key windows keep the hardware's 79 x 75 aspect
// proportions from photos of the module (owner, 2026-10-11): the keys span about 3/4 of the
// module width with narrow gaps; the grey panel around them leaves a wide black frame
const KW = 54.5;
const KH = Math.round(((KW * 75) / 79) * 10) / 10;
const KGX = 7.6;
const KGY = 7.6;
const KX0 = (400 - (5 * KW + 4 * KGX)) / 2;
/** The matte recessed panel around the keys and the plate, an even margin on every side. */
const PANEL_M = 20;
const PLATE_H = 18;
const PANEL_H = PANEL_M + 3 * KH + 2 * KGY + 12 + PLATE_H + PANEL_M;
const PANEL_Y = (300 - PANEL_H) / 2;
const KY0 = PANEL_Y + PANEL_M;
/** Top of the blank plate, right under the key rows. */
const PLATE_Y = KY0 + 3 * KH + 2 * KGY + 12;
const GRID_W = 5 * KW + 4 * KGX;


const RAINBOW = ["#ff3b3b", "#ffb21a", "#46e06b", "#22d3ee", "#5b7cff", "#c05bff", "#ff3bb4"];

interface Props {
  live: LiveState;
  /** What the LEDs show now (`useLeds`); null = worked out from the profile (the demo). */
  leds?: LedsShown | null;
  selected: ControlRef | null;
  onSelect: (c: ControlRef) => void;
  arranging?: boolean;
  profile?: Profile | null;
  /** Key page shown on the deck; null = the main page. */
  page?: string | null;
  /** An action was dropped from the library onto a control. */
  onDropAction?: (c: ControlRef, action: string) => void;
}

export const ACTION_MIME = "application/x-hubdeck-action";

/** Everything a module drawing needs. */
interface Ctx extends Props {
  m: ModuleInfo;
  /** Unique SVG id for this deck instance. */
  id: (name: string) => string;
  /** Degrees that turn content back upright on screen (undoes module rotation and base turn). */
  upright: number;
  /** Controls take no clicks (the module is being arranged). */
  inert: boolean;
  dropOn: string | null;
  setDropOn: (k: string | null) => void;
  /** Drawn greyed out: a module the user owns that is not attached. */
  muted?: boolean;
  /** Controls that point at something this PC lacks (`profile_check`), by control id: the reasons. */
  issues?: Record<string, string>;
  /** What plugins show on controls now, by control id (over the saved looks). */
  liveLooks: Record<string, Look>;
}

const range = (n: number) => Array.from({ length: n }, (_, i) => i);
const r2 = (n: number) => Math.round(n * 100) / 100;
const polar = (cx: number, cy: number, r: number, deg: number): [number, number] => {
  const a = (deg * Math.PI) / 180;
  return [r2(cx + r * Math.cos(a)), r2(cy + r * Math.sin(a))];
};
const arcPath = (cx: number, cy: number, r: number, a0: number, a1: number) => {
  const [x0, y0] = polar(cx, cy, r, a0);
  const [x1, y1] = polar(cx, cy, r, a1);
  return `M${x0} ${y0}A${r} ${r} 0 ${a1 - a0 > 180 ? 1 : 0} 1 ${x1} ${y1}`;
};

/** A lit LED in the drawing: its color and the effect it runs (`static`, `breathing`, ...). */
export interface Lit {
  color: string;
  effect: string;
}

/** Effects that run through colors: drawn as a hue cycle. */
const COLOR_EFFECTS = ["rainbow", "cycling", "random", "wave"];

/** A LED as the engine set it; null = dark. */
function litFrom(led: LedView, rainbow: string): Lit | null {
  // HubDeck's own rainbow: the window works the color out from the clock (useLeds sets the variable)
  if (led.effect === "hubRainbow") return { color: "var(--hub-rainbow, #ff3b3b)", effect: "hub" };
  if (led.effect === "off" || led.color === "#000000") return null;
  return COLOR_EFFECTS.includes(led.effect) ? { color: rainbow, effect: "rainbow" } : led;
}

/**
 * LEDs of knobs or faders: what the deck shows now (`shown`, from the engine), else the profile's
 * own colors, else the module's rainbow. null = dark.
 */
export function ledColors(profile: Profile | null | undefined, kind: "knobs" | "faders", arranging: boolean, shown?: LedView[] | null): (Lit | null)[] {
  const base = kind === "knobs" ? KNOB_COLORS : FADER_COLORS;
  // the module's rainbow: all its LEDs show the same color and turn through the hues together
  const rainbow = base.map(() => ({ color: RAINBOW[0], effect: "rainbow" }));
  if (arranging) return base.map((color) => ({ color, effect: "static" }));
  if (shown === null) return rainbow;
  if (shown) return base.map((_, i) => (shown[i] ? litFrom(shown[i], RAINBOW[0]) : null));
  const own = base.map((_, i) => profile?.controls[controlId(kind, i)]?.look.led ?? null);
  return own.some(Boolean) ? own.map((color) => (color ? { color, effect: "static" } : null)) : rainbow;
}

/** The base edge: what the deck shows now, else the profile's color; undefined = rainbow. */
function edgeLight(profile: Profile | null | undefined, shown: LedView[] | null | undefined): Lit | null | undefined {
  if (shown === undefined) return profile?.edge ? { color: profile.edge, effect: "static" } : undefined;
  const led = shown?.[0];
  if (!led) return undefined;
  const lit = litFrom(led, RAINBOW[0]);
  return lit?.effect === "rainbow" ? undefined : lit;
}

/**
 * Puts the engine's SVG for a screen inline (so the page's IBM Plex font applies), scaled to the
 * window. Ids, if any, get a prefix so several copies can live in one document.
 */
function inlineSvg(svg: string, prefix: string) {
  const open = svg.match(/<svg\b[^>]*>/);
  const close = svg.lastIndexOf("</svg>");
  if (!open || close < 0) return null;
  const viewBox = open[0].match(/viewBox="([^"]+)"/)?.[1] ?? `0 0 ${open[0].match(/width="(\d+)/)?.[1] ?? 72} ${open[0].match(/height="(\d+)/)?.[1] ?? 72}`;
  const inner = svg
    .slice(open.index! + open[0].length, close)
    .replace(/\bid="([^"]+)"/g, `id="${prefix}$1"`)
    .replace(/url\(#([^)]+)\)/g, `url(#${prefix}$1)`)
    .replace(/href="#([^"]+)"/g, `href="#${prefix}$1"`);
  return { viewBox, inner };
}

function Screen({ svg, prefix, x, y, w, h, clip }: { svg: string; prefix: string; x: number; y: number; w: number; h: number; clip?: string }) {
  // parsed once per picture: a key with a GIF carries the whole file
  const parsed = useMemo(() => inlineSvg(svg, prefix), [svg, prefix]);
  if (!parsed) return null;
  return (
    <g clipPath={clip}>
      <svg x={x} y={y} width={w} height={h} viewBox={parsed.viewBox} preserveAspectRatio="none" dangerouslySetInnerHTML={{ __html: parsed.inner }} />
    </g>
  );
}

/** One control: click and keyboard select it, library actions drop on it; rings show hover and selection. */
function Ctl(props: { ctx: Ctx; index: number; kind: ModuleKind; label: string; ring: (cls: string) => ReactNode; children: ReactNode; badge?: [number, number] }) {
  const { ctx, index, kind, label, ring, children, badge } = props;
  const issue = ctx.issues?.[controlId(kind, index, kind === "keys" ? (ctx.page ?? null) : null)];
  const c: ControlRef = { module: ctx.m.id, kind, index };
  const k = controlKey(c.module, index);
  const selected = ctx.selected?.module === c.module && ctx.selected.index === index;
  const droppable = !!ctx.onDropAction && !ctx.arranging;
  return (
    <g
      className={`dk-ctl${selected ? " is-selected" : ""}${ctx.dropOn === k ? " is-drop" : ""}${ctx.inert ? " is-inert" : ""}`}
      role="button"
      tabIndex={ctx.inert ? -1 : 0}
      aria-label={label}
      aria-pressed={selected}
      onClick={() => ctx.onSelect(c)}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          ctx.onSelect(c);
        }
      }}
      onDragOver={
        droppable
          ? (e) => {
              if (e.dataTransfer.types.includes(ACTION_MIME)) {
                e.preventDefault();
                e.dataTransfer.dropEffect = "copy";
                if (ctx.dropOn !== k) ctx.setDropOn(k);
              }
            }
          : undefined
      }
      onDragLeave={droppable ? () => ctx.dropOn === k && ctx.setDropOn(null) : undefined}
      onDrop={
        droppable
          ? (e) => {
              const action = e.dataTransfer.getData(ACTION_MIME);
              ctx.setDropOn(null);
              if (action) {
                e.preventDefault();
                ctx.onDropAction?.(c, action);
              }
            }
          : undefined
      }
    >
      <title>{label}</title>
      {ctx.muted ? <g filter={`url(#${ctx.id("desat")})`}>{children}</g> : children}
      {ring("dk-hov")}
      {ring("dk-sel-glow")}
      {ring("dk-sel")}
      {issue && badge && (
        <g className="dk-check" transform={`rotate(${ctx.upright} ${badge[0]} ${badge[1]})`}>
          <title>{issue}</title>
          <circle cx={badge[0]} cy={badge[1]} r="7" />
          <path d={`M${badge[0]} ${badge[1] - 3.6}v4.2`} />
          <circle className="dot" cx={badge[0]} cy={badge[1] + 3.4} r=".9" />
        </g>
      )}
    </g>
  );
}

/**
 * What a knob, roller or fader does, written next to it on the drawing (owner's pick A): upright
 * whatever the module's turn, `down` units below its point as seen on screen.
 */
/**
 * The LED's color at full brightness, for a caption: a light that shows the level dims with it
 * and its caption went nearly black at a low volume (owner, 2026-10-14: "system knob nie jest podpisany").
 */
export function readable(color: string | null | undefined): string | undefined {
  const m = /^#?([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})/i.exec(color ?? "");
  if (!m) return undefined;
  const rgb = [m[1], m[2], m[3]].map((h) => parseInt(h, 16));
  const top = Math.max(...rgb);
  if (top < 24) return undefined;
  // same hue, brightest channel at full, a little toward white so dark hues stay legible
  const lift = rgb.map((v) => Math.round((v / top) * 255 * 0.8 + 255 * 0.2));
  return `#${lift.map((v) => v.toString(16).padStart(2, "0")).join("")}`;
}

function CtlLabel({ ctx, x, y, down, text, color, outward }: { ctx: Ctx; x: number; y: number; down: number; text?: string; color?: string | null; outward?: boolean }) {
  if (!text || ctx.arranging) return null;
  const a = (ctx.upright * Math.PI) / 180;
  // `outward`: the label ends a track that runs along the module's y. When the track lies across
  // the screen, the text sits just above it and ends at the track's end, running back along it:
  // centred on the end it covered the slot, and past the end there is no room in the module
  // (a turned fader's caption sat on its slot, 2026-10-15)
  const along = Math.sin(a);
  const across = outward && Math.abs(along) > 0.5;
  const anchor = across ? (along > 0 ? "end" : "start") : "middle";
  const lift = across ? -22 : down;
  // screen-down in the module's own frame: (0, 1) turned by `upright`
  const [lx, ly] = [r2(x - Math.sin(a) * lift), r2(y + Math.cos(a) * lift)];
  const shown = text.replace(/\s+/g, " ").trim();
  const short = shown.length > 12 ? `${shown.slice(0, 11)}…` : shown;
  return (
    <text className="dk-ctl-label" x={lx} y={ly} textAnchor={anchor} dominantBaseline="central" transform={`rotate(${ctx.upright} ${lx} ${ly})`} style={readable(color) ? { fill: readable(color) } : undefined}>
      {short}
    </text>
  );
}

/**
 * The caption of a knob, roller or fader: its label, else what its action says (the app of a
 * volume, "System", "Scroll"...), since assistants and users often leave turning controls unnamed.
 */
function caption(b: Binding | undefined): string | undefined {
  if (!b) return undefined;
  if (b.look.label) return b.look.label;
  const p = (b.params ?? {}) as Record<string, unknown>;
  const app = typeof p.app === "string" && p.app ? p.app.replace(/\.(exe|app)$/i, "") : "";
  if (b.action === "core.volume") return app ? app.charAt(0).toUpperCase() + app.slice(1) : t(`caption.volume.${p.target === "mic" || p.target === "others" || p.target === "front" ? p.target : "system"}` as Key);
  const words: Record<string, Key> = { "core.scroll": "caption.scroll", "core.seek": "caption.seek", "core.media": "caption.media", "core.brightness": "caption.brightness", "core.deckBrightness": "caption.brightness", "core.profile": "caption.profiles", "core.page": "caption.pages", "core.hotkey": "caption.keys" };
  return words[b.action] ? t(words[b.action]) : undefined;
}

const nameOf = (kind: ModuleKind, i: number, look?: Look) => `${t(`control.${kind}`)}${kind === "dial" ? "" : ` ${i + 1}`}${look?.label ? ` · ${look.label}` : ""}`;

/** A module housing: glossy black, rounded, a lit bevel around its top face and a light-grey accent on one short side. */
function ModuleBody({ ctx, w, h }: { ctx: Ctx; w: number; h: number }) {
  const { id } = ctx;
  return (
    <>
      <rect x="3" y="3" width={w - 6} height={h - 6} rx="17" fill={`url(#${id("bevel")})`} />
      <rect x="3.5" y="3.5" width={w - 7} height={h - 7} rx="16.5" fill="none" stroke="#000" strokeOpacity=".7" />
      {/* the light-grey accent on the right short side */}
      <path d={`M${w - 5.5} 20V${h - 20}`} stroke={`url(#${id("accent")})`} strokeWidth="3" strokeLinecap="round" />
      <rect x="8" y="8" width={w - 16} height={h - 16} rx="12" fill={`url(#${id("mod")})`} />
      <rect x="8.5" y="8.5" width={w - 17} height={h - 17} rx="11.5" fill="none" stroke={`url(#${id("hl")})`} />
      <rect x="8" y="8" width={w - 16} height={h - 16} rx="12" fill={`url(#${id("gloss")})`} pointerEvents="none" />
    </>
  );
}

/** A shallow recess in a module's top: darker, shadowed along its upper edge, lit along the lower. */
function Recess({ ctx, x, y, w, h, rx, fill = "#0c0c0f" }: { ctx: Ctx; x: number; y: number; w: number; h: number; rx: number; fill?: string }) {
  return (
    <>
      <rect x={x} y={y} width={w} height={h} rx={rx} fill={fill} />
      <rect x={x + 0.5} y={y + 0.5} width={w - 1} height={h - 1} rx={rx - 0.5} fill="none" stroke={`url(#${ctx.id("recess")})`} strokeWidth="1.2" />
    </>
  );
}

// the picture fills the clear cap almost to its edge
const KP = 0.8;

/** `mirror.src` is the `#id` of the panel picture, held once in the keys' defs. */
/** A GIF on a key plays in the window itself, as in Stream Deck's window, not as a stream of the
 * deck's pictures (owner, 2026-10-16). */
const GIFS_IN_WINDOW = false; // WINDOW_PLAYS_GIFS in engine/mod.rs: smooth, but WebView2 440-600 MB instead of ~320
const playsItself = (look?: Look) => GIFS_IN_WINDOW && !!look?.image && /\.gif$/i.test(look.image);

function KeyFace({ ctx, i, look, mirror }: { ctx: Ctx; i: number; look?: Look; mirror?: { src: string; rect: [number, number, number, number]; saver?: boolean } | null }) {
  const own = playsItself(look) && !mirror?.saver;
  const svg = usePreview("key", own && look ? { ...look, image: `${look.image}#anim` } : look);
  const [pw, ph] = [KW - 2 * KP, KH - 2 * KP];
  if (mirror && !own) {
    // this key's window cut out of the picture the hardware got (live values, covers, clocks)
    const [rx, ry, rw, rh] = mirror.rect;
    return (
      <g clipPath={`url(#${ctx.id("kclip")})`}>
        <svg x={KP} y={KP} width={pw} height={ph} viewBox={`${rx} ${ry} ${rw} ${rh}`} preserveAspectRatio="none">
          <use href={mirror.src} />
        </svg>
      </g>
    );
  }
  if (svg) {
    // a key turned a quarter shows its upright picture in a window that is taller than wide
    // as on the hardware (render.rs panel_tiles): the square picture keeps its proportions,
    // centred in the cap, and the key's background fills the rest (2026-10-15, the owner: icons
    // looked flattened; the picture was stretched to the cap's 79x75)
    const side = Math.min(pw, ph);
    const bg = look?.style === "outline" ? "#0c0b10" : look?.bg ?? "#1f1d26";
    return (
      <g clipPath={`url(#${ctx.id("kclip")})`}>
        <rect x={KP} y={KP} width={pw} height={ph} fill={bg} />
        <g transform={`rotate(${ctx.upright} ${KW / 2} ${KH / 2})`}>
          <Screen svg={svg} prefix={`${ctx.id("k")}${i}-`} x={(KW - side) / 2} y={(KH - side) / 2} w={side} h={side} />
        </g>
      </g>
    );
  }
  return look?.label ? (
    <text className="dk-face-text" x={KW / 2} y={KH / 2 + 4} textAnchor="middle" transform={`rotate(${ctx.upright} ${KW / 2} ${KH / 2})`}>
      {look.label}
    </text>
  ) : (
    <text className="dk-face-num" x={KW / 2} y={KH / 2 + 5} textAnchor="middle" transform={`rotate(${ctx.upright} ${KW / 2} ${KH / 2})`}>
      {i + 1}
    </text>
  );
}

/** Off: the screensaver drawn here through <use> grew WebView2 by 45 MB a minute (window stage,
 * 2026-10-16); the keys show the deck's pictures of it until a canvas version measures well. */
const SAVER_IN_WINDOW = false;

/** The screensaver's panel, 15 frames a second, for the keys to show their windows of it. */
function SaverPanel({ id, effect, color }: { id: string; effect: string; color: string }) {
  const t = useTicker(true, 15);
  return (
    <defs>
      <svg id={id} viewBox="0 0 480 272" width="480" height="272">
        <rect width="480" height="272" fill="#000" />
        <g dangerouslySetInnerHTML={{ __html: saverBody(effect, t, color) ?? "" }} />
      </svg>
    </defs>
  );
}

function Keys({ ctx }: { ctx: Ctx }) {
  const { m, live, profile, page, arranging, id } = ctx;
  // what the hardware shows right now, while this profile and page are on the deck
  const frame = useKeysFrame(profile?.id, page, m.id);
  // the screensaver is drawn here, from the same effects as the deck's; its frames change only
  // SaverPanel, the keys show it through <use> (a tick that redrew the whole panel of keys took
  // WebView2 to 850 MB, 2026-10-16)
  const saver = SAVER_IN_WINDOW ? useSaverShown() : null;
  return (
    <>
      <ModuleBody ctx={ctx} w={400} h={300} />
      {saver && <SaverPanel id={id("ksaver")} effect={saver.effect} color={saver.color} />}
      {/* the panel picture the hardware got, once: each key shows its window of it (fifteen
          copies of each frame made WebKit decode and repaint it fifteen times) */}
      {frame && (
        <defs>
          <image id={id("kpanel")} href={frame.src} x="0" y="0" width="480" height="272" />
        </defs>
      )}
      {/* the slightly recessed top panel that holds the keys and the plate */}
      <Recess ctx={ctx} x={KX0 - PANEL_M} y={PANEL_Y} w={GRID_W + 2 * PANEL_M} h={PANEL_H} rx={8} fill="#0c0c0e" />
      {/* matte black, only a faint flat sheen */}
      <rect x={KX0 - PANEL_M + 1} y={PANEL_Y + 1} width={GRID_W + 2 * PANEL_M - 2} height={PANEL_H - 2} rx="7" fill={`url(#${id("matte")})`} pointerEvents="none" />
      {/* the blank plate right under the keys, sunk a little deeper */}
      <rect x={KX0} y={PLATE_Y} width={GRID_W} height={PLATE_H} rx="3" fill="#0e0e11" stroke="#000" strokeOpacity=".85" />
      <rect x={KX0 + 1} y={PLATE_Y + PLATE_H - 5} width={GRID_W - 2} height="4" rx="2" fill="#fff" opacity=".035" />
      {range(15).map((i) => {
        const x = KX0 + (i % 5) * (KW + KGX);
        const y = KY0 + Math.floor(i / 5) * (KH + KGY);
        const cid = controlId("keys", i, page ?? null);
        const look = ctx.liveLooks[cid] ?? profile?.controls[cid]?.look;
        const pressed = live.pressedKeys[controlKey(m.id, i)];
        return (
          <Ctl key={i} ctx={ctx} index={i} kind="keys" label={nameOf("keys", i, look)} badge={[x + KW - 3, y + 3]} ring={(cls) => <rect className={cls} x={x - 4} y={y - 4} width={KW + 8} height={KH + 8} rx="13" />}>
            <g transform={`translate(${x} ${y + (pressed ? 1.2 : 0)})`}>
              {/* a small glass cube: its shadow, then its dark sides showing below and to the right */}
              {!pressed && <rect x="0.5" y="3.5" width={KW + 2} height={KH + 2} rx="9" fill="#000" opacity=".55" filter={`url(#${id("soft")})`} />}
              {!pressed && <rect x="1.4" y="2.6" width={KW} height={KH} rx="8" fill={`url(#${id("capside")})`} />}
              <rect width={KW} height={KH} rx="8" fill="#060607" />
              <rect x={KP} y={KP} width={KW - 2 * KP} height={KH - 2 * KP} rx="7.4" fill="#121216" />
              {arranging ? (
                <>
                  <rect x={KP} y={KP} width={KW - 2 * KP} height={KH - 2 * KP} rx="9" fill="#2a2733" />
                  <rect x={KW / 2 - 8} y={KP} width="16" height="5" fill="#c4b5fd" />
                  <text className="dk-face-num is-big" x={KW / 2} y={KH / 2 + 9} textAnchor="middle" transform={`rotate(${ctx.upright} ${KW / 2} ${KH / 2})`}>
                    {i + 1}
                  </text>
                </>
              ) : (
                <KeyFace
                  ctx={ctx}
                  i={i}
                  look={look}
                  mirror={
                    saver
                      ? { src: `#${id("ksaver")}`, rect: frame?.rects[i] ?? [(i % 5) * 96, Math.floor(i / 5) * 90.67, 96, 90.67], saver: true }
                      : frame && frame.rects[i]
                        ? { src: `#${id("kpanel")}`, rect: frame.rects[i] }
                        : null
                  }
                />
              )}
              {/* glossy glass top: a thin bright rim, a sheen over the upper part, a hard highlight on the top edge */}
              <rect width={KW} height={KH} rx="8" fill={`url(#${id("vign")})`} pointerEvents="none" />
              <rect width={KW} height={KH} rx="8" fill={`url(#${id("dome")})`} pointerEvents="none" />
              <rect x=".5" y=".5" width={KW - 1} height={KH - 1} rx="7.5" fill="none" stroke="#fff" strokeOpacity=".28" />
              <path d={`M7 1.1H${KW - 7}`} fill="none" stroke="#fff" strokeOpacity=".8" strokeWidth="1.1" strokeLinecap="round" />
              <path d={`M1.1 7V${KH * 0.45}`} fill="none" stroke="#fff" strokeOpacity=".35" strokeWidth=".8" strokeLinecap="round" />
              {pressed && <rect className="dk-pressed" width={KW} height={KH} rx="8" />}
            </g>
          </Ctl>
        );
      })}
    </>
  );
}

/** A knob: a tall knurled cylinder with a brushed-metal top, its LED glowing as a ring at the base. */
function Knob({ ctx, x, y, led, angle, pressed, number }: { ctx: Ctx; x: number; y: number; led: Lit | null; angle: number; pressed: boolean; number?: number }) {
  const { id } = ctx;
  const color = led?.color;
  return (
    <>
      {/* the well and the LED ring at the base */}
      <circle cx={x} cy={y} r="33" fill="#08080a" />
      <circle cx={x} cy={y} r="29.5" fill="none" stroke="#1c1c21" strokeWidth="2.2" />
      {color && (
        <g className={`dk-fx-${led.effect}`}>
          <circle cx={x} cy={y} r="42" style={{ fill: color }} opacity=".18" filter={`url(#${id("halo")})`} />
          <circle cx={x} cy={y} r="29.5" fill="none" style={{ stroke: color }} strokeWidth="5" opacity=".8" filter={`url(#${id("soft")})`} />
          <circle cx={x} cy={y} r="29.5" fill="none" style={{ stroke: color }} strokeWidth="2.2" />
          <circle cx={x} cy={y} r="29.5" fill="none" stroke="#fff" strokeOpacity=".5" strokeWidth=".6" />
        </g>
      )}
      <circle cx={x} cy={y + 2.5} r="26" fill="#000" opacity=".7" filter={`url(#${id("soft")})`} />
      <g transform={pressed ? `translate(${x} ${y}) scale(.96) translate(${-x} ${-y})` : undefined}>
        {/* knurled side, seen around the top */}
        <circle cx={x} cy={y} r="26" fill={`url(#${id("knurl")})`} />
        <circle cx={x} cy={y} r="25" fill="none" stroke="#000" strokeOpacity=".55" strokeWidth="2.2" strokeDasharray="1 1.2" transform={`rotate(${angle} ${x} ${y})`} />
        {/* brushed top */}
        <circle cx={x} cy={y} r="21" fill={`url(#${id("cap")})`} stroke="#fff" strokeOpacity=".14" />
        {[16, 11, 6].map((r) => (
          <circle key={r} cx={x} cy={y} r={r} fill="none" stroke="#fff" strokeOpacity=".05" />
        ))}
        <path d={arcPath(x, y, 18.5, 200, 255)} fill="none" stroke="#fff" strokeOpacity=".32" strokeWidth="1.4" strokeLinecap="round" />
        {number ? (
          <text className="dk-face-num is-big" x={x} y={y + 7} textAnchor="middle" transform={`rotate(${ctx.upright} ${x} ${y})`}>
            {number}
          </text>
        ) : (
          <line x1={x} y1={y - 11} x2={x} y2={y - 18} stroke="#fff" strokeOpacity=".85" strokeWidth="2" strokeLinecap="round" transform={`rotate(${angle} ${x} ${y})`} />
        )}
      </g>
    </>
  );
}

// physical order in the module's own frame: knob 2 top left, knob 1 top right, knob 3 below
// (confirmed with the identify colors on 2026-10-09)
const KNOB_AT: [number, number][] = [
  [146, 60],
  [54, 60],
  [100, 142],
];

function Knobs({ ctx }: { ctx: Ctx }) {
  const { m, live, profile, arranging, leds } = ctx;
  const colors = ledColors(profile, "knobs", !!arranging, leds?.[m.id]);
  // the Y-shaped ridge between the three wells, from the middle of the triangle
  const [cx, cy] = [100, 92];
  const ridge = `M${cx} ${cy}V16M${cx} ${cy}L22 178M${cx} ${cy}L178 178`;
  return (
    <>
      <ModuleBody ctx={ctx} w={200} h={200} />
      <Recess ctx={ctx} x={14} y={14} w={172} h={172} rx={10} />
      <path d={ridge} stroke="#000" strokeOpacity=".8" strokeWidth="3.5" strokeLinecap="round" />
      <path d={ridge} stroke="#fff" strokeOpacity=".07" strokeWidth="1.2" strokeLinecap="round" transform="translate(-1 -1)" />
      {[1, 0, 2].map((i) => {
        const [x, y] = KNOB_AT[i];
        const k = controlKey(m.id, i);
        const look = profile?.controls[controlId("knobs", i)]?.look;
        return (
          <Ctl key={i} ctx={ctx} index={i} kind="knobs" label={nameOf("knobs", i, look)} badge={[x + 26, y - 26]} ring={(cls) => <circle className={cls} cx={x} cy={y} r="38" />}>
            <Knob ctx={ctx} x={x} y={y} led={colors[i]} angle={(live.counters[k] ?? 0) * 12} pressed={!!live.pressedKeys[k]} number={arranging ? i + 1 : undefined} />
            <CtlLabel ctx={ctx} x={x} y={y} down={41} text={caption(profile?.controls[controlId("knobs", i)])} color={colors[i]?.effect === "hub" ? null : colors[i]?.color} />
          </Ctl>
        );
      })}
    </>
  );
}

// the dial: a big knurled cylinder on a raised square pad; its top is the round screen in a thin bezel
const DIAL_SCREEN = 50;
/** The top of the turning cylinder: a thick glossy bezel around the screen. */
const DIAL_TOP = 76;

function Dial({ ctx }: { ctx: Ctx }) {
  const { m, live, profile, arranging, id } = ctx;
  const look = ctx.liveLooks.dial ?? profile?.controls.dial?.look;
  const preview = usePreview("dial", look && (look.label || look.icon) ? look : profile ? { label: profile.name } : undefined);
  // what the hardware shows right now (the clock's time, a live value, a cover), when this profile
  // is the one on the deck; the saved look's preview otherwise
  const frame = useDialFrame(profile?.id, m.id);
  const k = controlKey(m.id, 0);
  const turns = live.counters[k] ?? 0;
  const pressed = !!live.pressedKeys[k];
  const [cx, cy] = [100, 100];
  const r = DIAL_SCREEN;
  return (
    <>
      <ModuleBody ctx={ctx} w={200} h={200} />
      {/* raised square pad */}
      <rect x="16" y="16" width="168" height="168" rx="22" fill={`url(#${id("bevel")})`} />
      <rect x="16.5" y="16.5" width="167" height="167" rx="21.5" fill="none" stroke="#fff" strokeOpacity=".12" />
      <rect x="20" y="20" width="160" height="160" rx="19" fill={`url(#${id("mod")})`} />
      <Ctl ctx={ctx} index={0} kind="dial" label={nameOf("dial", 0, look)} badge={[cx + 62, cy - 62]} ring={(cls) => <circle className={cls} cx={cx} cy={cy} r="88" />}>
        <circle cx={cx} cy={cy + 3} r="84" fill="#000" opacity=".65" filter={`url(#${id("soft")})`} />
        {/* knurled side of the turning cylinder */}
        <circle cx={cx} cy={cy} r="83" fill={`url(#${id("knurl")})`} />
        <circle cx={cx} cy={cy} r="81.6" fill="none" stroke="#000" strokeOpacity=".55" strokeWidth="2.6" strokeDasharray="1 1.3" transform={`rotate(${turns * 6} ${cx} ${cy})`} />
        <circle cx={cx} cy={cy} r="82.6" fill="none" stroke="#fff" strokeOpacity=".16" />
        <rect x={cx - 1.2} y={cy - 83} width="2.4" height="6" rx="1.2" fill="#fff" opacity=".55" transform={`rotate(${turns * 12} ${cx} ${cy})`} />
        {/* the thick glossy bezel on top, the screen sunk in its middle */}
        <circle cx={cx} cy={cy} r={DIAL_TOP} fill={`url(#${id("bezel")})`} stroke="#000" strokeOpacity=".8" />
        <path d={arcPath(cx, cy, DIAL_TOP - 4, 200, 300)} fill="none" stroke="#fff" strokeOpacity=".28" strokeWidth="2" strokeLinecap="round" />
        <circle cx={cx} cy={cy} r={r + 2.5} fill="#000" stroke="#fff" strokeOpacity=".08" />
        <circle cx={cx} cy={cy} r={r} fill={`url(#${id("scr")})`} />
        {arranging ? (
          <g>
            <path d={`M${cx} ${cy - 34}l18 24h-11v24h-14v-24h-11z`} fill="#c4b5fd" />
            <text className="dk-face-num" x={cx} y={cy + 34} textAnchor="middle">
              {t("deck.top")}
            </text>
          </g>
        ) : frame ? (
          <image href={frame} x={cx - r} y={cy - r} width={2 * r} height={2 * r} clipPath={`url(#${id("dclip")})`} />
        ) : preview ? (
          <g transform={`rotate(${ctx.upright} ${cx} ${cy})`}>
            <Screen svg={preview} prefix={`${id("d")}-`} x={cx - r} y={cy - r} w={2 * r} h={2 * r} clip={`url(#${id("dclip")})`} />
          </g>
        ) : (
          <text className="dk-face-text" x={cx} y={cy + 4} textAnchor="middle" transform={`rotate(${ctx.upright} ${cx} ${cy})`}>
            {t("deck.dial")}
          </text>
        )}
        {/* glass over the screen */}
        <circle cx={cx} cy={cy} r={r} fill={`url(#${id("dome")})`} pointerEvents="none" />
        <path d={arcPath(cx, cy, r - 3, 205, 290)} fill="none" stroke="#fff" strokeOpacity=".22" strokeWidth="1.4" strokeLinecap="round" pointerEvents="none" />
        {pressed && <circle className="dk-pressed" cx={cx} cy={cy} r={r} />}
      </Ctl>
    </>
  );
}

/** A long pill with a recessed trough: two ribbed cylinders with end caps, a ribbed coupling between them. */
function Rollers({ ctx }: { ctx: Ctx }) {
  const { m, live, profile, id } = ctx;
  const capW = 12;
  return (
    <>
      <ModuleBody ctx={ctx} w={400} h={100} />
      <Recess ctx={ctx} x={18} y={22} w={364} h={56} rx={14} />
      {/* end caps */}
      {[24, 376 - capW].map((x) => (
        <rect key={x} x={x} y="28" width={capW} height="44" rx="5" fill={`url(#${id("cyl")})`} stroke="#000" strokeOpacity=".6" />
      ))}
      {/* the ribbed coupling in the middle */}
      <rect x="191" y="27" width="18" height="46" rx="5" fill={`url(#${id("cyl")})`} stroke="#000" strokeOpacity=".7" />
      <path d="M195.5 28V72M200 28V72M204.5 28V72" stroke="#000" strokeOpacity=".7" strokeWidth="1.3" />
      {range(2).map((i) => {
        const [x0, x1] = i === 0 ? [24 + capW + 2, 189] : [211, 376 - capW - 2];
        const k = controlKey(m.id, i);
        // a notch moves the ribs a fifth of their spacing, so the direction is visible; the sign
        // follows how the owner sees his rollers turn
        const phase = ((((live.counters[k] ?? 0) * 0.2) % 1) + 1) % 1;
        const ribs = range(14)
          .map((n) => r2(50 + 20 * Math.sin(-Math.PI / 2 + ((n + phase) * Math.PI) / 14)))
          .filter((y) => y > 30.5 && y < 69.5)
          .map((y) => `M${x0 + 1} ${y}H${x1 - 1}`)
          .join("");
        const pulse = live.pulses[k] ?? 0;
        const look = profile?.controls[controlId("rollers", i)]?.look;
        return (
          <Ctl key={i} ctx={ctx} index={i} kind="rollers" label={nameOf("rollers", i, look)} badge={[x1 - 2, 28]} ring={(cls) => <rect className={cls} x={x0 - 5} y="23" width={x1 - x0 + 10} height="54" rx="12" />}>
            <rect x={x0} y="30" width={x1 - x0} height="40" rx="4" fill={`url(#${id("cyl")})`} />
            <path d={ribs} stroke="#000" strokeOpacity=".65" strokeWidth="1.2" />
            <path d={ribs} stroke="#fff" strokeOpacity=".07" strokeWidth=".6" transform="translate(0 1)" />
            {pulse > 0 && <rect key={pulse} className="dk-flash" x={x0} y="30" width={x1 - x0} height="40" rx="4" />}
            <CtlLabel ctx={ctx} x={(x0 + x1) / 2} y={50} down={0} text={caption(profile?.controls[controlId("rollers", i)])} />
          </Ctl>
        );
      })}
    </>
  );
}

/** Five slots in a recessed tray; half-cylinder caps with a ribbed front; the LED glows in the slot below the cap. */
function Faders({ ctx }: { ctx: Ctx }) {
  const { m, live, profile, arranging, id, leds } = ctx;
  const colors = ledColors(profile, "faders", !!arranging, leds?.[m.id]);
  const [top, bottom] = [42, 162];
  return (
    <>
      <ModuleBody ctx={ctx} w={400} h={200} />
      <Recess ctx={ctx} x={16} y={16} w={368} h={168} rx={10} />
      {range(5).map((i) => {
        const cx = 56 + i * 72;
        const value = live.faders[controlKey(m.id, i)];
        const v = value ?? 50;
        const ty = bottom - ((bottom - top) * v) / 100;
        const look = profile?.controls[controlId("faders", i)]?.look;
        const color = colors[i]?.color;
        // the engine's colors already follow the value; the profile's alone do not
        const level = look?.ledLevel && value !== undefined && !leds?.[m.id] ? 0.25 + (0.75 * value) / 100 : 1;
        return (
          <Ctl key={i} ctx={ctx} index={i} kind="faders" label={nameOf("faders", i, look)} badge={[cx + 20, 26]} ring={(cls) => <rect className={cls} x={cx - 28} y="20" width="56" height="160" rx="12" />}>
            <rect x={cx - 28} y="20" width="56" height="160" fill="transparent" />
            {/* the slot, and the LED lit inside it below the cap */}
            <rect x={cx - 3.5} y={top - 12} width="7" height={bottom - top + 24} rx="3.5" fill="#030304" stroke="#fff" strokeOpacity=".06" />
            {color && (
              <g className={`dk-fx-${colors[i]?.effect}`}>
                {/* the glow as a gradient, not a blur filter: a cap moving 25 times a second re-ran
                    the blur each time and the window lagged behind the fader (WebKit, 2026-10-15) */}
                <defs>
                  <linearGradient id={id(`fglow${i}`)} x1="0" x2="1" y1="0" y2="0">
                    <stop offset="0" style={{ stopColor: color }} stopOpacity="0" />
                    <stop offset=".5" style={{ stopColor: color }} stopOpacity=".7" />
                    <stop offset="1" style={{ stopColor: color }} stopOpacity="0" />
                  </linearGradient>
                </defs>
                <rect x={cx - 6} y={ty + 6} width="12" height={Math.max(0, bottom + 12 - ty - 6)} rx="6" fill={`url(#${id(`fglow${i}`)})`} opacity={0.6 * level} />
                <rect x={cx - 1.2} y={ty + 8} width="2.4" height={Math.max(0, bottom + 10 - ty - 8)} rx="1.2" style={{ fill: color }} opacity={level} />
              </g>
            )}
            <g opacity={value === undefined && !arranging ? 0.9 : 1}>
              {/* the cap's soft shadow from stacked rounded rects (no blur filter, see the glow) */}
              <rect x={cx - 24} y={ty - 13} width="48" height="31" rx="12" fill="#000" opacity=".15" />
              <rect x={cx - 22.5} y={ty - 12} width="45" height="28.5" rx="10.5" fill="#000" opacity=".25" />
              <rect x={cx - 21} y={ty - 11} width="42" height="26" rx="9" fill="#000" opacity=".35" />
              {/* the cap: rounded back, a ribbed front face toward the user */}
              <rect x={cx - 20} y={ty - 13} width="40" height="25" rx="8" fill={`url(#${id("thumb")})`} stroke="#fff" strokeOpacity=".14" />
              <g clipPath={`url(#${id("fclip")})`} transform={`translate(${cx - 20} ${ty - 13})`}>
                <path d="M-4 25L6 11M2 25L12 11M8 25L18 11M14 25L24 11M20 25L30 11M26 25L36 11M32 25L42 11M38 25L48 11" stroke="#000" strokeOpacity=".6" strokeWidth="1.3" />
              </g>
              <path d={`M${cx - 14} ${ty - 11.5}h28`} stroke="#fff" strokeOpacity=".25" strokeWidth=".8" strokeLinecap="round" />
            </g>
            <CtlLabel ctx={ctx} x={cx} y={bottom + 14} down={0} text={caption(profile?.controls[controlId("faders", i)])} color={color} outward />
          </Ctl>
        );
      })}
    </>
  );
}

const DRAW: Partial<Record<ModuleKind, (p: { ctx: Ctx }) => ReactNode>> = { keys: Keys, faders: Faders, knobs: Knobs, dial: Dial, rollers: Rollers };

/** Static defs shared by the drawing (gradients, filters, clips). */
function Defs({ id, edge }: { id: (n: string) => string; edge: string | undefined }) {
  const stops = edge ? [edge, edge] : RAINBOW;
  return (
    <defs>
      <linearGradient id={id("edge")} gradientUnits="userSpaceOnUse" x1={-MARGIN} y1={-MARGIN} x2={GW + MARGIN} y2={GH + MARGIN}>
        {stops.map((c, i) => (
          <stop key={i} offset={i / (stops.length - 1)} style={{ stopColor: c }} />
        ))}
      </linearGradient>
      <filter id={id("blur")} x="-25%" y="-25%" width="150%" height="150%">
        <feGaussianBlur stdDeviation="19" />
      </filter>
      <filter id={id("soft")} x="-50%" y="-50%" width="200%" height="200%">
        <feGaussianBlur stdDeviation="3" />
      </filter>
      <filter id={id("halo")} x="-50%" y="-50%" width="200%" height="200%">
        <feGaussianBlur stdDeviation="7" />
      </filter>
      <filter id={id("sh")} x="-20%" y="-20%" width="140%" height="160%">
        <feDropShadow dx="0" dy="22" stdDeviation="22" floodColor="#000" floodOpacity=".8" />
      </filter>
      <filter id={id("noise")} x="0" y="0" width="100%" height="100%">
        <feTurbulence type="fractalNoise" baseFrequency=".9" numOctaves="2" stitchTiles="stitch" />
        <feColorMatrix values="0 0 0 0 1 0 0 0 0 1 0 0 0 0 1 0 0 0 .55 0" />
        <feComposite in2="SourceAlpha" operator="in" />
      </filter>
      <linearGradient id={id("board")} x1="0" y1="0" x2="0" y2="1">
        <stop offset="0" stopColor="#25242c" />
        <stop offset="1" stopColor="#121116" />
      </linearGradient>
      <linearGradient id={id("mod")} x1="0" y1="0" x2="0" y2="1">
        <stop offset="0" stopColor="#1b1b20" />
        <stop offset="1" stopColor="#111115" />
      </linearGradient>
      <linearGradient id={id("bevel")} x1="0" y1="0" x2="0" y2="1">
        <stop offset="0" stopColor="#34343c" />
        <stop offset=".35" stopColor="#16161a" />
        <stop offset="1" stopColor="#060607" />
      </linearGradient>
      <linearGradient id={id("gloss")} x1="0" y1="0" x2="1" y2="1">
        <stop offset="0" stopColor="#fff" stopOpacity=".1" />
        <stop offset=".35" stopColor="#fff" stopOpacity=".02" />
        <stop offset=".36" stopColor="#fff" stopOpacity="0" />
      </linearGradient>
      <radialGradient id={id("dome")} cx=".42" cy=".18" r=".75">
        <stop offset="0" stopColor="#fff" stopOpacity=".2" />
        <stop offset=".45" stopColor="#fff" stopOpacity=".04" />
        <stop offset="1" stopColor="#fff" stopOpacity="0" />
      </radialGradient>
      <radialGradient id={id("vign")} cx=".5" cy=".5" r=".72">
        <stop offset=".62" stopColor="#000" stopOpacity="0" />
        <stop offset="1" stopColor="#000" stopOpacity=".55" />
      </radialGradient>
      <linearGradient id={id("knurl")} x1="0" y1="0" x2="1" y2="1">
        <stop offset="0" stopColor="#5a5a63" />
        <stop offset=".3" stopColor="#1e1e23" />
        <stop offset=".55" stopColor="#3c3c44" />
        <stop offset="1" stopColor="#0c0c0e" />
      </linearGradient>
      <linearGradient id={id("accent")} x1="0" y1="0" x2="0" y2="1">
        <stop offset="0" stopColor="#c9c9d2" stopOpacity=".2" />
        <stop offset=".5" stopColor="#d6d6de" stopOpacity=".75" />
        <stop offset="1" stopColor="#c9c9d2" stopOpacity=".2" />
      </linearGradient>
      <linearGradient id={id("recess")} x1="0" y1="0" x2="0" y2="1">
        <stop offset="0" stopColor="#000" stopOpacity=".9" />
        <stop offset=".5" stopColor="#000" stopOpacity=".3" />
        <stop offset="1" stopColor="#fff" stopOpacity=".09" />
      </linearGradient>
      <clipPath id={id("fclip")}>
        <rect width="40" height="25" rx="8" />
      </clipPath>
      <radialGradient id={id("bezel")} cx=".4" cy=".3" r=".8">
        <stop offset="0" stopColor="#2a2a30" />
        <stop offset=".55" stopColor="#0d0d10" />
        <stop offset="1" stopColor="#050506" />
      </radialGradient>
      <radialGradient id={id("kbase")} cx=".45" cy=".35" r=".7">
        <stop offset="0" stopColor="#2c2c33" />
        <stop offset="1" stopColor="#141417" />
      </radialGradient>
      <linearGradient id={id("hl")} x1="0" y1="0" x2="0" y2="1">
        <stop offset="0" stopColor="#fff" stopOpacity=".16" />
        <stop offset=".3" stopColor="#fff" stopOpacity=".04" />
        <stop offset="1" stopColor="#fff" stopOpacity=".015" />
      </linearGradient>
      <linearGradient id={id("glass")} x1="0" y1="0" x2="0" y2="1">
        <stop offset="0" stopColor="#fff" stopOpacity=".2" />
        <stop offset=".44" stopColor="#fff" stopOpacity=".03" />
        <stop offset=".5" stopColor="#fff" stopOpacity="0" />
        <stop offset="1" stopColor="#000" stopOpacity=".3" />
      </linearGradient>
      <linearGradient id={id("cyl")} x1="0" y1="0" x2="0" y2="1">
        <stop offset="0" stopColor="#040405" />
        <stop offset=".2" stopColor="#2c2b33" />
        <stop offset=".4" stopColor="#72717d" />
        <stop offset=".56" stopColor="#3b3a43" />
        <stop offset="1" stopColor="#040405" />
      </linearGradient>
      <linearGradient id={id("rend")} x1="0" y1="0" x2="1" y2="0">
        <stop offset="0" stopColor="#000" stopOpacity=".75" />
        <stop offset=".14" stopColor="#000" stopOpacity="0" />
        <stop offset=".86" stopColor="#000" stopOpacity="0" />
        <stop offset="1" stopColor="#000" stopOpacity=".75" />
      </linearGradient>
      <linearGradient id={id("thumb")} x1="0" y1="0" x2="0" y2="1">
        <stop offset="0" stopColor="#4a4954" />
        <stop offset=".5" stopColor="#24232b" />
        <stop offset="1" stopColor="#15141a" />
      </linearGradient>
      <linearGradient id={id("sheen")} x1="0" y1="0" x2="1" y2="1">
        <stop offset="0" stopColor="#fff" stopOpacity=".09" />
        <stop offset=".45" stopColor="#fff" stopOpacity="0" />
      </linearGradient>
      <radialGradient id={id("knob")} cx=".38" cy=".3" r=".8">
        <stop offset="0" stopColor="#55545f" />
        <stop offset=".55" stopColor="#26252d" />
        <stop offset="1" stopColor="#0f0f13" />
      </radialGradient>
      <radialGradient id={id("cap")} cx=".42" cy=".36" r=".72">
        <stop offset="0" stopColor="#34333c" />
        <stop offset="1" stopColor="#17161c" />
      </radialGradient>
      <linearGradient id={id("metal")} x1="0" y1="0" x2="1" y2="1">
        {["#9a99a3", "#3a3941", "#c9c8d1", "#2c2b32", "#7d7c86"].map((c, i) => (
          <stop key={c} offset={i / 4} stopColor={c} />
        ))}
      </linearGradient>
      <radialGradient id={id("scr")} cx=".5" cy=".4" r=".62">
        <stop offset="0" stopColor="#1a1524" />
        <stop offset="1" stopColor="#050408" />
      </radialGradient>
      <radialGradient id={id("halo-desk")}>
        <stop offset="0" style={{ stopColor: edge ?? "#8b5cf6" }} stopOpacity=".3" />
        <stop offset=".55" style={{ stopColor: edge ?? "#8b5cf6" }} stopOpacity=".1" />
        <stop offset="1" style={{ stopColor: edge ?? "#8b5cf6" }} stopOpacity="0" />
      </radialGradient>
      <filter id={id("desat")}>
        <feColorMatrix type="saturate" values=".12" />
        <feComponentTransfer>
          <feFuncA type="linear" slope=".8" />
        </feComponentTransfer>
      </filter>
      <clipPath id={id("kclip")}>
        <rect x={KP} y={KP} width={KW - 2 * KP} height={KH - 2 * KP} rx="7.4" />
      </clipPath>
      <linearGradient id={id("matte")} x1="0" y1="0" x2="0" y2="1">
        <stop offset="0" stopColor="#fff" stopOpacity=".035" />
        <stop offset="1" stopColor="#fff" stopOpacity="0" />
      </linearGradient>
      <linearGradient id={id("capside")} x1="0" y1="0" x2="1" y2="1">
        <stop offset="0" stopColor="#1a1a1f" />
        <stop offset="1" stopColor="#020203" />
      </linearGradient>
      <clipPath id={id("dclip")}>
        <circle cx="100" cy="100" r={DIAL_SCREEN} />
      </clipPath>
    </defs>
  );
}

const span = (cells: number) => cells * U;

interface DeckViewProps extends Props {
  saved: Record<string, Placement>;
  /** Stores a placement; null forgets the entry. */
  onPlace: (key: string, placement: Placement | null) => void;
  arranging: boolean;
  /** The Edge chip under the deck, and the base's rim in the drawing, open the lights' settings
   * (owner's pick 5a; 2026-10-16 he tried to click the rim itself). */
  onEdge?: () => void;
  /** A click on the drawing away from every control: nothing selected, as in Stream Deck. */
  onDeselect?: () => void;
}

interface Drag {
  id: string;
  /** Pointer at the start, in base-grid units. */
  x: number;
  y: number;
  col: number;
  row: number;
}

const NO_LOOKS: Record<string, Look> = {};

/** What `profile_check` flags in a profile (a missing program, sound, picture, device), by control id. */
/** How long after a start "Connecting to your deck" shows before "No MasterHUB found" (a restart
 * takes every module back in about 6 s). */
const CONNECTING_MS = 15000;

/** True while the hub has been looking for less than `CONNECTING_MS`; turns false by itself. */
function useConnecting(lookingMs: number | null | undefined): boolean {
  const left = lookingMs == null ? 0 : CONNECTING_MS - lookingMs;
  const [over, setOver] = useState(false);
  useEffect(() => {
    setOver(left <= 0);
    if (left <= 0) return;
    const timer = window.setTimeout(() => setOver(true), left);
    return () => window.clearTimeout(timer);
  }, [left]);
  return left > 0 && !over;
}

function useIssues(profile: Profile | null | undefined): Record<string, string> {
  const [issues, setIssues] = useState<Record<string, string>>({});
  const signature = profile ? profile.id + JSON.stringify(profile.controls) : "";
  useEffect(() => {
    if (!profile || !("__TAURI_INTERNALS__" in window)) return setIssues({});
    let current = true;
    api.checkProfile(profile.id).then(
      (list) => {
        if (!current) return;
        const map: Record<string, string> = {};
        // a hold or double press shows on its control too
        for (const i of Array.isArray(list) ? list : []) {
          const id = i.control.split(":")[0];
          const message = localizeError(i.message);
          map[id] = map[id] ? `${map[id]}
${message}` : message;
        }
        setIssues(map);
      },
      () => current && setIssues({}),
    );
    return () => {
      current = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [signature]);
  return issues;
}

export function DeckView(props: DeckViewProps) {
  useLanguage();
  const { live, saved, onPlace, arranging, profile, selected, onSelect } = props;
  const uid = useId().replace(/[^a-zA-Z0-9]/g, "");
  const id = (n: string) => `dk${uid}-${n}`;
  const [drag, setDrag] = useState<Drag | null>(null);
  const [dragTo, setDragTo] = useState<{ col: number; row: number } | null>(null);
  const [dropOn, setDropOn] = useState<string | null>(null);
  const baseRef = useRef<SVGGElement>(null);
  const issues = useIssues(profile);
  // what plugins show now, only for the profile they are for (the assistant draws other profiles)
  const shown = useLiveLooks();
  const liveLooks = profile && shown.profile === profile.id ? shown.looks : NO_LOOKS;
  const leds = useLeds();
  const placed = placeModules(live.deck.modules, saved);
  const hasBase = live.deck.modules.some((m) => m.kind === "base");
  // the first seconds after a start the hub is still taking the deck: "Connecting", then "No MasterHUB"
  const connecting = useConnecting(live.deck.lookingMs);

  const shelf = arranging ? null : <Shelf {...props} id={id} dropOn={dropOn} setDropOn={setDropOn} issues={issues} />;
  if (!hasBase && placed.length === 0) {
    return (
      <div className="dk">
        <div className="deck-empty">
          <p>{t(connecting ? "deck.connectingTitle" : "deck.noneTitle")}</p>
          <p className="muted">{t(connecting ? "deck.connecting" : "deck.none")}</p>
        </div>
        {shelf}
      </div>
    );
  }

  /** Clockwise degrees the whole base drawing is turned on screen, in -90..180. */
  const baseTurn = live.deck.turn === 270 ? -90 : live.deck.turn;
  const sideways = Math.abs(baseTurn) === 90;
  const bw = (sideways ? GH : GW) + 2 * MARGIN;
  const bh = (sideways ? GW : GH) + 2 * MARGIN;
  const vb = [CX - bw / 2 - PAD, CY - bh / 2 - PAD, bw + 2 * PAD, bh + 2 * PAD];
  const baseId = live.deck.modules.find((m) => m.kind === "base")?.id;
  const edgeLit = edgeLight(profile, baseId && leds ? leds[baseId] : undefined);
  // dark when the edge is off, the rainbow when undefined
  const edge = edgeLit === null ? "#1c1c21" : edgeLit?.color;

  /** Pointer position in base-grid units (undoes the base turn and the scaling). */
  const toBase = (clientX: number, clientY: number) => {
    const ctm = baseRef.current?.getScreenCTM();
    if (!ctm) return { x: 0, y: 0 };
    const p = new DOMPoint(clientX, clientY).matrixTransform(ctm.inverse());
    return { x: p.x, y: p.y };
  };

  const commit = (p: Placed, col: number, row: number, rotation: Rotation) => {
    const key = placementKey(p.module);
    if (!key) return;
    const [cols, rows] = rotatedSize(p.module.kind, rotation);
    const [c, r] = clampToGrid(col, row, cols, rows);
    onPlace(key, { col: c, row: r, rotation });
  };

  const viewBox = vb.join(" ");
  const stageStyle = { "--dk-ratio": vb[2] / vb[3] } as CSSProperties;

  return (
    <div
      className={`dk${arranging ? " is-arranging" : ""}${live.deck.paused ? " is-paused" : ""}${!arranging && detachedKinds(live.deck.modules, saved).length ? " has-shelf" : ""}`}
      onClick={(e) => {
        if (!arranging && !(e.target as Element).closest(".dk-ctl, .dk-rim, button, a, input")) props.onDeselect?.();
      }}
    >
      <div className="dk-stage" style={stageStyle}>
        {/* static layers: light on the desk, edge glow, the board */}
        <svg className="dk-layer dk-back" viewBox={viewBox} aria-hidden="true">
          <Defs id={(n) => id(`b-${n}`)} edge={edge} />
          <ellipse cx={CX} cy={CY} rx={bw * 0.62} ry={bh * 0.68} fill={`url(#${id("b-halo-desk")})`} />
          <ellipse cx={CX} cy={CY + bh / 2 + 18} rx={bw / 2} ry="22" style={{ fill: edge ?? "#8b5cf6" }} opacity=".12" filter={`url(#${id("b-blur")})`} />
          <g transform={`rotate(${baseTurn} ${CX} ${CY})`}>
            <rect className="dk-edge-glow" opacity=".32" x={-MARGIN} y={-MARGIN} width={GW + 2 * MARGIN} height={GH + 2 * MARGIN} rx="30" fill="none" stroke={`url(#${id("b-edge")})`} strokeWidth="14" filter={`url(#${id("b-blur")})`} />
            <rect x={-MARGIN} y={-MARGIN} width={GW + 2 * MARGIN} height={GH + 2 * MARGIN} rx="30" fill={`url(#${id("b-board")})`} filter={`url(#${id("b-sh")})`} />
            <rect x={-MARGIN} y={-MARGIN} width={GW + 2 * MARGIN} height={GH + 2 * MARGIN} rx="30" fill="#000" filter={`url(#${id("b-noise")})`} opacity=".07" />
            <rect x={-MARGIN + 1.5} y={-MARGIN + 1.5} width={GW + 2 * MARGIN - 3} height={GH + 2 * MARGIN - 3} rx="28.5" fill="none" stroke={`url(#${id("b-edge")})`} strokeWidth="3" />
            <rect x={-MARGIN + 1.5} y={-MARGIN + 1.5} width={GW + 2 * MARGIN - 3} height={GH + 2 * MARGIN - 3} rx="28.5" fill="none" stroke="#fff" strokeOpacity=".45" strokeWidth=".7" />
            <rect className="dk-edge-inner" opacity=".35" x={-13} y={-13} width={GW + 26} height={GH + 26} rx="24" fill="none" stroke={`url(#${id("b-edge")})`} strokeWidth="5" filter={`url(#${id("b-soft")})`} />
            <rect x={-7} y={-7} width={GW + 14} height={GH + 14} rx="19" fill="#060608" />
          </g>
        </svg>

        {/* live layer: modules and controls */}
        <svg className="dk-layer dk-front" viewBox={viewBox} role="group" aria-label={t("module.base")}>
          <Defs id={id} edge={edge} />
          <g ref={baseRef} transform={`rotate(${baseTurn} ${CX} ${CY})`}>
            {!arranging && props.onEdge && (
              <rect className="dk-rim" x={-MARGIN / 2 - 4} y={-MARGIN / 2 - 4} width={GW + MARGIN + 8} height={GH + MARGIN + 8} rx="26" onClick={props.onEdge}>
                <title>{t("deck.edgeOpen")}</title>
              </rect>
            )}
            {arranging &&
              range(GRID_COLS * GRID_ROWS).map((i) => <rect key={`cell${i}`} className="dk-cell" x={(i % GRID_COLS) * U + 6} y={Math.floor(i / GRID_COLS) * U + 6} width={U - 12} height={U - 12} rx="10" />)}
            {placed.map((p) => {
              const Draw = DRAW[p.module.kind];
              if (!Draw) return null;
              const moving = drag?.id === p.module.id && dragTo;
              const col = moving ? dragTo.col : p.col;
              const row = moving ? dragTo.row : p.row;
              const canArrange = arranging && !!p.module.slot;
              const [x, y, w, h] = [col * U, row * U, span(p.cols), span(p.rows)];
              const [w0, h0] = [span(p.baseCols), span(p.baseRows)];
              const ctx: Ctx = { ...props, leds, m: p.module, id, upright: -(p.rotation + baseTurn), inert: canArrange, dropOn, setDropOn, issues, liveLooks };
              const labelTurn = -baseTurn;
              // the slot's top edge and top right corner as seen on screen, in base units
              const at = (up: number, right: number) => {
                const a = (labelTurn * Math.PI) / 180;
                const [ux, uy] = [Math.round(Math.sin(a)), -Math.round(Math.cos(a))];
                const [rx, ry] = [Math.round(Math.cos(a)), Math.round(Math.sin(a))];
                const halfUp = (Math.abs(ux) * w + Math.abs(uy) * h) / 2;
                const halfRight = (Math.abs(rx) * w + Math.abs(ry) * h) / 2;
                const [cx, cy] = [x + w / 2, y + h / 2];
                const [du, dr] = [halfUp - up, right ? halfRight - right : 0];
                return [cx + ux * du + rx * dr, cy + uy * du + ry * dr];
              };
              const [nx, ny] = at(22, 0);
              const [bx, by] = at(26, 26);
              const [qx, qy] = at(18, 18);
              return (
                <g
                  key={p.module.id}
                  className={`dk-slot${canArrange ? " is-draggable" : ""}${moving ? " is-moving" : ""}`}
                  onPointerDown={(e) => {
                    if (!canArrange) return;
                    e.currentTarget.setPointerCapture(e.pointerId);
                    const at = toBase(e.clientX, e.clientY);
                    setDrag({ id: p.module.id, x: at.x, y: at.y, col: p.col, row: p.row });
                    setDragTo({ col: p.col, row: p.row });
                  }}
                  onPointerMove={(e) => {
                    if (!drag || drag.id !== p.module.id) return;
                    const at = toBase(e.clientX, e.clientY);
                    const [c, r] = clampToGrid(drag.col + Math.round((at.x - drag.x) / U), drag.row + Math.round((at.y - drag.y) / U), p.cols, p.rows);
                    if (c !== dragTo?.col || r !== dragTo?.row) setDragTo({ col: c, row: r });
                  }}
                  onPointerUp={() => {
                    if (drag?.id === p.module.id && dragTo) commit(p, dragTo.col, dragTo.row, p.rotation);
                    setDrag(null);
                    setDragTo(null);
                  }}
                >
                  <g transform={`translate(${x + w / 2} ${y + h / 2}) rotate(${p.rotation}) translate(${-w0 / 2} ${-h0 / 2})`}>
                    <Draw ctx={ctx} />
                  </g>
                  {arranging && (
                    <>
                      <rect className="dk-arrange-frame" x={x + 3} y={y + 3} width={w - 6} height={h - 6} rx="15" />
                      <text className="dk-arrange-name" x={nx} y={ny} textAnchor="middle" dominantBaseline="middle" transform={`rotate(${labelTurn} ${nx} ${ny})`}>
                        {/* the slot code only in the tooltip: it means nothing to most people */}
                        {p.module.slot && <title>{p.module.slot}</title>}
                        {t(`kind.${p.module.kind}`)}
                        {!p.module.slot && ` · ${t("deck.slotUnknown")}`}
                      </text>
                      {canArrange && (
                        <g
                          className="dk-rotate"
                          role="button"
                          tabIndex={0}
                          aria-label={t("deck.rotate", { kind: t(`kind.${p.module.kind}`) })}
                          onPointerDown={(e) => e.stopPropagation()}
                          onClick={() => commit(p, p.col, p.row, ((p.rotation + 90) % 360) as Rotation)}
                          onKeyDown={(e) => {
                            if (e.key === "Enter" || e.key === " ") {
                              e.preventDefault();
                              commit(p, p.col, p.row, ((p.rotation + 90) % 360) as Rotation);
                            }
                          }}
                        >
                          <title>{t("deck.rotate", { kind: t(`kind.${p.module.kind}`) })}</title>
                          <circle cx={bx} cy={by} r="17" />
                          <path d="M20 12a8 8 0 1 1-2.3-5.7M20 4v5h-5" transform={`translate(${bx - 12} ${by - 12}) rotate(${labelTurn} 12 12)`} />
                        </g>
                      )}
                    </>
                  )}
                  {!p.known && !arranging && (
                    <g className="dk-unknown">
                      <title>{t("deck.guessed")}</title>
                      <circle cx={qx} cy={qy} r="11" />
                      <text x={qx} y={qy} textAnchor="middle" dominantBaseline="central" transform={`rotate(${labelTurn} ${qx} ${qy})`}>
                        ?
                      </text>
                    </g>
                  )}
                </g>
              );
            })}
            <rect x={-MARGIN} y={-MARGIN} width={GW + 2 * MARGIN} height={GH + 2 * MARGIN} rx="30" fill={`url(#${id("sheen")})`} pointerEvents="none" />
          </g>
        </svg>
      </div>
      {!arranging && <LedStrip live={live} profile={profile} leds={leds} edge={edgeLit} placed={placed} selected={selected} onSelect={onSelect} onEdge={props.onEdge} />}
      {shelf}
    </div>
  );
}

/** The chips of every knob, fader and the dial under the deck (before labels on the drawing). */
const SHOW_CONTROL_CHIPS = false;

/** Chips under the deck: the edge light (and, switched off, every knob and fader with its LED and value). */
function LedStrip({ live, profile, leds, edge, placed, selected, onSelect, onEdge }: { live: LiveState; profile?: Profile | null; leds: LedsShown | null; edge: Lit | null | undefined; placed: Placed[]; selected: ControlRef | null; onSelect: (c: ControlRef) => void; onEdge?: () => void }) {
  const EdgeChip = onEdge ? "button" : "span";
  const chips: ReactNode[] = [
    <EdgeChip key="edge" data-guide={onEdge ? "edge" : undefined} className={`dk-chip${onEdge ? "" : " is-static"}`} onClick={onEdge} title={onEdge ? t("deck.edgeOpen") : undefined}>
      <i className={edge ? `dk-fx-${edge.effect}` : undefined} style={{ background: edge === null ? "var(--tx3, #6c6b7b)" : edge?.color ?? `conic-gradient(${RAINBOW.join(",")}, ${RAINBOW[0]})` }} />
      {t("deck.edge")}
      <b>{edge === null ? t("deck.off") : edge && edge.effect !== "hub" ? edge.color.toUpperCase() : t("deck.rainbow")}</b>
    </EdgeChip>,
  ];
  const chip = (c: ControlRef, led: Lit | null, value?: string) => {
    const color = led?.color;
    const look = profile?.controls[controlId(c.kind, c.index)]?.look;
    const isSel = selected?.module === c.module && selected.index === c.index;
    return (
      <button key={controlKey(c.module, c.index)} className={`dk-chip${isSel ? " is-selected" : ""}`} onClick={() => onSelect(c)}>
        <i className={led ? `dk-fx-${led.effect}` : undefined} style={{ background: color ?? "var(--tx3, #6c6b7b)", color: color ?? "transparent" }} />
        {t(`control.${c.kind}`)}
        {c.kind === "dial" ? "" : ` ${c.index + 1}`}
        {(look?.label || value) && <b>{[look?.label, value].filter(Boolean).join(" ")}</b>}
      </button>
    );
  };
  // the controls' chips are gone: the drawing names what each knob, roller and fader does
  for (const p of SHOW_CONTROL_CHIPS ? placed : []) {
    const m = p.module;
    if (m.kind === "dial") chips.push(chip({ module: m.id, kind: "dial", index: 0 }, null));
    if (m.kind === "knobs") {
      const colors = ledColors(profile, "knobs", false, leds?.[m.id]);
      for (const i of [0, 1, 2]) chips.push(chip({ module: m.id, kind: "knobs", index: i }, colors[i]));
    }
    if (m.kind === "faders") {
      const colors = ledColors(profile, "faders", false, leds?.[m.id]);
      for (const i of range(5)) {
        const v = live.faders[controlKey(m.id, i)];
        chips.push(chip({ module: m.id, kind: "faders", index: i }, colors[i], v === undefined ? undefined : `${v}%`));
      }
    }
  }
  return <div className="dk-strip">{chips}</div>;
}

/** A module drawn on its own, unrotated: the shelf and the first-run tiles. */
export function ModuleSvg({ ctx, className }: { ctx: Ctx; className?: string }) {
  const Draw = DRAW[ctx.m.kind];
  const [cols, rows] = FOOTPRINT[ctx.m.kind];
  if (!Draw) return null;
  return (
    <svg className={className} aria-hidden={ctx.inert || undefined} viewBox={`0 0 ${cols * U} ${rows * U}`} style={{ aspectRatio: `${cols} / ${rows}` }}>
      <Defs id={ctx.id} edge={undefined} />
      <Draw ctx={ctx} />
    </svg>
  );
}

/** A small, non-interactive picture of a module kind. */
export function ModuleThumb({ kind, muted }: { kind: ModuleKind; muted?: boolean }) {
  const uid = useId().replace(/[^a-zA-Z0-9]/g, "");
  const live: LiveState = { deck: { modules: [], orientation: null, turn: 0, turnOverride: null, unresponsive: [], paused: false }, pressedKeys: {}, faders: {}, counters: {}, lastTouched: null, pulses: {} };
  const ctx: Ctx = { live, selected: null, onSelect: () => {}, m: detachedModule(kind), id: (n) => `th${uid}-${n}`, upright: 0, inert: true, dropOn: null, setDropOn: () => {}, muted, liveLooks: {} };
  return <ModuleSvg ctx={ctx} className="dk-thumb" />;
}

/**
 * Modules the user owns that are not attached: drawn greyed beside the deck, their controls
 * selectable and editable like attached ones (profiles bind controls by kind).
 */
function Shelf(props: DeckViewProps & { id: (n: string) => string; dropOn: string | null; setDropOn: (k: string | null) => void; issues: Record<string, string> }) {
  const { live, saved, onPlace } = props;
  const [picking, setPicking] = useState(false);
  const kinds = detachedKinds(live.deck.modules, saved);
  const attached = new Set(live.deck.modules.map((m) => m.kind));
  const addable = MODULE_KINDS.filter((k) => !attached.has(k) && !kinds.includes(k));
  if (kinds.length === 0 && addable.length === 0) return null;
  return (
    <section className="dk-shelf" aria-label={t("shelf.title")}>
      {kinds.map((kind) => {
        const ctx: Ctx = { ...props, m: detachedModule(kind), id: (n) => props.id(`s${kind}-${n}`), upright: 0, inert: false, muted: true, issues: props.issues, liveLooks: {} };
        return (
          <figure key={kind} className={`dk-shelf-item is-${kind}`}>
            <ModuleSvg ctx={ctx} className="dk-shelf-art" />
            <figcaption>
              <b>{t(`module.${kind}`)}</b>
              <span>{t("shelf.notAttached")}</span>
            </figcaption>
            <button className="dk-shelf-remove" aria-label={t("shelf.remove", { kind: t(`module.${kind}`) })} title={t("shelf.remove", { kind: t(`module.${kind}`) })} onClick={() => keysOfKind(saved, kind).forEach((k) => onPlace(k, null))}>
              <svg viewBox="0 0 24 24" aria-hidden="true">
                <path d="M18 6 6 18M6 6l12 12" />
              </svg>
            </button>
          </figure>
        );
      })}
      {addable.length > 0 && (
        <div className={`dk-shelf-add${picking ? " is-open" : ""}`}>
          {picking ? (
            <>
              <span className="dk-shelf-ask">{t("shelf.pick")}</span>
              <div className="dk-shelf-kinds">
                {addable.map((kind) => (
                  <button key={kind} onClick={() => (onPlace(ownedKey(kind), { col: 0, row: 0, rotation: 0 }), setPicking(false))}>
                    <ModuleThumb kind={kind} />
                    {t(`module.${kind}`)}
                  </button>
                ))}
              </div>
              <button className="dk-shelf-cancel" onClick={() => setPicking(false)}>
                {t("calib.cancel")}
              </button>
            </>
          ) : (
            <button className="dk-shelf-plus" onClick={() => setPicking(true)}>
              <svg viewBox="0 0 24 24" aria-hidden="true">
                <path d="M5 12h14M12 5v14" />
              </svg>
              {t("shelf.add")}
            </button>
          )}
        </div>
      )}
    </section>
  );
}
