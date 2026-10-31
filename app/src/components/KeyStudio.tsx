import { useEffect, useRef, useState } from "react";
import { api, controlId, inTauri, inWindow, usePreview, type Binding, type EngineView, type Look } from "../engine";
import { withLook } from "../editing";
import { save as saveDialog } from "../confirm";
import { looksLike, templateOf } from "../lookTemplates";
import { t } from "../i18n";
import { Icon } from "./Icon";
import { IconPicker } from "./IconPicker";
import { KeyDesigner } from "./KeyDesigner";
import { LabelValues } from "./LookRules";

/** Sixteenths of a second in one cycle of each motion (the engine's own, `render.rs`). */
const CYCLE: Record<string, (look: Look) => number> = {
  pulse: () => 40,
  shine: () => 48,
  glow: () => 48,
  sweep: () => 48,
  sparkle: () => 32,
  flow: () => 96,
  slide: (l) => 4 * (l.pattern === "stripes" ? 8 : l.pattern === "noise" ? 24 : l.pattern === "waves" ? 18 : l.pattern === "hex" ? 15 : 9),
};

/**
 * The key's drawing as the deck shows it, moving when the look moves: the engine draws each frame
 * of the cycle once (kept), the window plays them at the deck's 16 a second.
 */
function useLivePreview(look: Look): string | null {
  const still = usePreview("key", look);
  const [frame, setFrame] = useState<string | null>(null);
  const frames = useRef(new Map<string, string>());
  const reduce = typeof window !== "undefined" && window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
  useEffect(() => {
    setFrame(null);
    const cycle = look.motion && !reduce ? CYCLE[look.motion]?.(look) : undefined;
    if (!cycle || !inTauri()) return;
    const base = JSON.stringify(look);
    let step = 0;
    let live = true;
    const timer = window.setInterval(() => {
      step = (step + 1) % cycle;
      const key = `${base}#${step}`;
      const kept = frames.current.get(key);
      if (kept) return setFrame(kept);
      api.preview("key", look, step / 16 + 0.001).then((raw) => {
        const svg = inWindow(raw);
        if (frames.current.size > 400) frames.current.clear();
        frames.current.set(key, svg);
        if (live) setFrame(svg);
      });
    }, 1000 / 16);
    return () => {
      live = false;
      window.clearInterval(timer);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [JSON.stringify(look), reduce]);
  return frame ?? still;
}

function KeyChip({ look, on, current, name, onToggle }: { look: Look; on: boolean; current: boolean; name: string; onToggle: () => void }) {
  const svg = usePreview("key", look);
  return (
    <button
      className={`studio-chip${on ? " is-on" : ""}${current ? " is-current" : ""}`}
      aria-pressed={on}
      title={name}
      aria-label={name}
      disabled={current}
      onClick={onToggle}
      dangerouslySetInnerHTML={{ __html: svg ?? "" }}
    />
  );
}

/**
 * The full key editor (owner's "2 a", 2026-10-25: mockup A, "w tym małym okienku słabo się tym
 * operuje"): the icon library on the left, the key big and live in the middle (drag the icon or
 * the label), the look's tabs on the right, and the page's other keys to change together.
 */
export function KeyStudio({ view, control, onClose }: { view: EngineView; control: string; onClose: () => void }) {
  const profile = view.profile;
  const page = view.state?.page ?? null;
  const start = profile?.controls[control];
  const [look, setLook] = useState<Look>(start?.look ?? {});
  const [also, setAlso] = useState<Set<string>>(new Set());
  const [moving, setMoving] = useState<"icon" | "label">("icon");
  const svg = useLivePreview(look);
  const pending = useRef<number | null>(null);
  const [note, setNote] = useState<string | null>(null);
  // save the key as a picture, or the page's keys as a Stream Deck icon pack
  const savePng = async () => {
    const path = await saveDialog({ defaultPath: `${(look.label ?? "key").replace(/[\\/:*?"<>|\n]/g, " ").trim() || "key"}.png`, filters: [{ name: "PNG", extensions: ["png"] }] });
    if (path) api.keyPngSave(look, path).then(() => setNote(t("studio.pngSaved", { path })), (e) => setNote(String(e)));
  };
  const savePack = async () => {
    if (!profile) return;
    if (pending.current) (window.clearTimeout(pending.current), save());
    const path = await saveDialog({ defaultPath: `${profile.name.replace(/[\\/:*?"<>|]/g, " ").trim() || "keys"}.streamDeckIconPack`, filters: [{ name: "Stream Deck icon pack", extensions: ["streamDeckIconPack"] }] });
    if (path) api.exportKeysPack(profile.id, page, path).then((count) => setNote(t("studio.packSaved", { count, path })), (e) => setNote(String(e)));
  };
  // the profile's style: new keys start in it (only the look, as a template carries)
  // field by field (the engine sends a look's fields in its own order); shown at once on a click
  const [styleOn, setStyleOn] = useState<boolean | null>(null);
  const styled = styleOn ?? (!!profile?.keyStyle && looksLike(look, profile.keyStyle));
  useEffect(() => setStyleOn(null), [profile?.keyStyle, look]);
  const setStyle = (on: boolean) => {
    if (!profile) return;
    setStyleOn(on);
    api
      .saveProfile({ ...profile, keyStyle: on ? templateOf(look) : undefined })
      .then(() => setNote(t(on ? "studio.styleSet" : "studio.styleOff", { name: profile.name })), (e) => setNote(String(e)));
  };
  const latest = useRef({ look, also });
  latest.current = { look, also };

  // saved a moment after the last change (as the panel does), the ticked keys with it
  const save = () => {
    pending.current = null;
    const p = view.profile;
    const b = p?.controls[control];
    if (!p || !b) return;
    const { look: l, also: more } = latest.current;
    const changes: Record<string, Binding> = { [control]: { ...b, look: l } };
    for (const id of more) if (p.controls[id]) changes[id] = { ...p.controls[id], look: withLook(p.controls[id].look ?? {}, l) };
    api.setBindings(p.id, changes).catch((e) => console.error("bindings_set", e));
  };
  // undo and redo (owner, 2026-10-25: "brakuje mi opcji cofania i powtarzania akcji i skrótów
  // klawiszowych"): a quick run of changes (a drag, a color picker held, typing) is one step
  const past = useRef<Look[]>([]);
  const future = useRef<Look[]>([]);
  const lastStep = useRef(0);
  const holding = useRef(false);
  const [, setSteps] = useState(0);
  const apply = (next: Look) => {
    setLook(next);
    if (pending.current) window.clearTimeout(pending.current);
    pending.current = window.setTimeout(save, 300);
  };
  const change = (next: Look) => {
    const now = Date.now();
    if (!holding.current && now - lastStep.current > 700) {
      past.current = [...past.current, look].slice(-100);
      future.current = [];
      setSteps((n) => n + 1);
    }
    lastStep.current = now;
    apply(next);
  };
  const undo = () => {
    const prev = past.current.pop();
    if (!prev) return;
    future.current.push(look);
    lastStep.current = 0;
    setSteps((n) => n + 1);
    apply(prev);
  };
  const redo = () => {
    const next = future.current.pop();
    if (!next) return;
    past.current.push(look);
    lastStep.current = 0;
    setSteps((n) => n + 1);
    apply(next);
  };
  // Ctrl+Z, Ctrl+Y and Ctrl+Shift+Z (Cmd on a Mac) undo the editor's steps, before the deck view's
  // own Ctrl+Z (the whole profile) sees them; a text field keeps its own undo
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      const el = e.target as HTMLElement | null;
      if (el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.isContentEditable) && (el as HTMLInputElement).type !== "range" && (el as HTMLInputElement).type !== "color") return;
      if (!(e.ctrlKey || e.metaKey) || e.altKey) return;
      const k = e.key.toLowerCase();
      if (k === "z" && !e.shiftKey) (e.preventDefault(), e.stopImmediatePropagation(), undo());
      else if (k === "y" || (k === "z" && e.shiftKey)) (e.preventDefault(), e.stopImmediatePropagation(), redo());
    };
    window.addEventListener("keydown", key, true);
    return () => window.removeEventListener("keydown", key, true);
  });
  const close = () => {
    if (pending.current) {
      window.clearTimeout(pending.current);
      save();
    }
    onClose();
  };
  useEffect(() => {
    const key = (e: KeyboardEvent) => e.key === "Escape" && !(e.target as HTMLElement)?.closest?.("input, select, textarea") && close();
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  });

  // drag on the big key: the icon or the label moves by whole key pixels
  const drag = useRef<{ x: number; y: number; dx: number; dy: number; zoom: number } | null>(null);
  const field = moving === "icon" ? (["iconDx", "iconDy"] as const) : (["labelDx", "labelDy"] as const);
  const clamp = (v: number) => Math.max(-36, Math.min(36, Math.round(v)));
  const onDown = (e: React.PointerEvent) => {
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    // screen pixels per key pixel (a key is 72; shown at 288, or 216 in a narrow window)
    const zoom = (e.currentTarget as HTMLElement).getBoundingClientRect().width / 72 || 4;
    drag.current = { x: e.clientX, y: e.clientY, dx: look[field[0]] ?? 0, dy: look[field[1]] ?? 0, zoom };
    // the whole drag is one step to undo
    past.current = [...past.current, look].slice(-100);
    future.current = [];
    holding.current = true;
  };
  const onMove = (e: React.PointerEvent) => {
    const d = drag.current;
    if (!d) return;
    const dx = clamp(d.dx + (e.clientX - d.x) / d.zoom);
    const dy = clamp(d.dy + (e.clientY - d.y) / d.zoom);
    if (dx !== (look[field[0]] ?? 0) || dy !== (look[field[1]] ?? 0)) change({ ...look, [field[0]]: dx || undefined, [field[1]]: dy || undefined });
  };
  const nudge = (e: React.KeyboardEvent) => {
    const by: Record<string, [number, number]> = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] };
    const step = by[e.key];
    if (!step) return;
    e.preventDefault();
    change({ ...look, [field[0]]: clamp((look[field[0]] ?? 0) + step[0]) || undefined, [field[1]]: clamp((look[field[1]] ?? 0) + step[1]) || undefined });
  };
  const moved = !!(look.iconDx || look.iconDy || look.labelDx || look.labelDy);

  // the page's keys that do something: tick them to change them together
  const keys = profile
    ? Array.from({ length: 15 }, (_, i) => controlId("keys", i, page))
        .filter((id) => profile.controls[id])
        .map((id) => ({ id, n: Number(id.split(".").pop()) + 1, look: profile.controls[id].look ?? {} }))
    : [];
  const number = Number(control.split(".").pop()) + 1;

  if (!start) {
    return (
      <div className="studio">
        <p className="help">{t("studio.gone")}</p>
        <button className="tool" onClick={onClose}>
          {t("studio.done")}
        </button>
      </div>
    );
  }
  return (
    <div className="studio" role="region" aria-label={t("studio.title")}>
      <div className="studio-bar">
        <button className="tool small ghost" onClick={close}>
          <Icon name="chevron-left" />
          {t("studio.back")}
        </button>
        <span className="studio-title">
          {t("studio.title")} <span className="tx2">· {profile?.name} · {t("studio.key", { n: number })}</span>
        </span>
        <span className="spacer" />
        {note && (
          <span className="studio-note help" role="status">
            {note}
          </span>
        )}
        <button className="tool small ghost" onClick={undo} disabled={past.current.length === 0} aria-label={t("studio.undo")} title={t("studio.undoHelp")}>
          <Icon name="undo" />
          {t("studio.undo")}
        </button>
        <button className="tool small ghost" onClick={redo} disabled={future.current.length === 0} aria-label={t("studio.redo")} title={t("studio.redoHelp")}>
          <Icon name="redo" />
          {t("studio.redo")}
        </button>
        <button className="tool small" onClick={savePng} title={t("studio.pngHelp")}>
          <Icon name="download" />
          {t("studio.png")}
        </button>
        <button className="tool small" onClick={savePack} title={t("studio.packHelp")}>
          <Icon name="archive" />
          {t("studio.pack")}
        </button>
        <button className="tool small primary" onClick={close}>
          {t("studio.done")}
        </button>
      </div>
      <div className="studio-grid">
        <section className="studio-pane studio-library" aria-label={t("look.icon")}>
          <IconPicker value={look.icon} onChange={(icon) => change({ ...look, icon, image: undefined })} onPicture={(image) => change({ ...look, image })} view={view} />
        </section>
        <section className="studio-pane studio-stage">
          <div className="studio-canvas">
            <div
              className="studio-key"
              role="img"
              aria-label={t("look.preview")}
              tabIndex={0}
              onPointerDown={onDown}
              onPointerMove={onMove}
              onPointerUp={() => ((drag.current = null), (holding.current = false), (lastStep.current = Date.now()))}
              onKeyDown={nudge}
              dangerouslySetInnerHTML={{ __html: svg ?? "" }}
            />
          </div>
          <div className="row studio-move">
            <span className="label">{t("studio.drag")}</span>
            <div className="seg" role="radiogroup" aria-label={t("studio.drag")}>
              {(["icon", "label"] as const).map((m) => (
                <button key={m} role="radio" aria-checked={moving === m} className={`tab${moving === m ? " is-active" : ""}`} onClick={() => setMoving(m)}>
                  {t(m === "icon" ? "look.icon" : "look.label")}
                </button>
              ))}
            </div>
            {moved && (
              <button className="tool small ghost" onClick={() => change({ ...look, iconDx: undefined, iconDy: undefined, labelDx: undefined, labelDy: undefined })}>
                {t("studio.resetPlace")}
              </button>
            )}
          </div>
          <p className="help">{t("studio.dragHelp")}</p>
          {keys.length > 1 && (
            <>
              <div className="studio-keys" role="group" aria-label={t("studio.together")}>
                {keys.map((k) => (
                  <KeyChip
                    key={k.id}
                    look={k.id === control ? look : k.look}
                    current={k.id === control}
                    on={k.id === control || also.has(k.id)}
                    name={`${k.n}. ${k.look.label?.trim() || ""}`}
                    onToggle={() => {
                      const next = new Set(also);
                      if (next.has(k.id)) next.delete(k.id);
                      else next.add(k.id);
                      setAlso(next);
                    }}
                  />
                ))}
              </div>
              <p className="help">{t("studio.togetherHelp", { count: also.size })}</p>
            </>
          )}
        </section>
        <section className="studio-pane studio-props">
          <label className="field">
            <span className="label">{t("look.label")}</span>
            <input className="input" value={look.label ?? ""} onChange={(e) => change({ ...look, label: e.target.value || undefined })} />
          </label>
          <LabelValues label={look.label ?? ""} onChange={(label) => change({ ...look, label })} />
          <KeyDesigner look={look} onChange={change} others={[]} onApplyTo={() => Promise.resolve()} />
          <label className="check" title={t("studio.styleHelp")}>
            <input type="checkbox" checked={styled} onChange={(e) => setStyle(e.target.checked)} />
            {t("studio.style", { name: profile?.name ?? "" })}
          </label>
        </section>
      </div>
    </div>
  );
}
