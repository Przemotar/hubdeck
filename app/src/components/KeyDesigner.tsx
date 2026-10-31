import { useEffect, useState } from "react";
import { api, BACKGROUNDS, FLASHING_MOTIONS, SKIN_MOTION, SKINS, usePreview, type Look, type Skin } from "../engine";
import { t, type Key } from "../i18n";
import { LookRules } from "./LookRules";
import { backgroundKind, BUILT_IN, finish, looksLike, templateOf, withFinish, withKind, withTemplate, type LookTemplate } from "../lookTemplates";

const HOUSE_BG = "#1b1a20";
const HOUSE_LABEL = "#e6e4ee";
const LABEL_SIZES = [8, 10, 12, 14, 16, 20, 24];
const TABS = ["templates", "background", "icon", "label", "extras", "when"] as const;
type Tab = (typeof TABS)[number];

/** Another key of the page the look can go to ("Apply to other keys"). */
export interface OtherKey {
  id: string;
  name: string;
}

/** One template drawn with the key's own label and icon; a click puts it on the key. */
function TemplateTile({ look, template, name, onPick, onDelete }: { look: Look; template: Look; name: string; onPick: () => void; onDelete?: () => void }) {
  const svg = usePreview("key", withTemplate(look, template));
  const on = looksLike(look, template);
  return (
    <div className="tpl-tile">
      <button role="option" aria-selected={on} className={`tpl-key${on ? " is-active" : ""}`} title={name} aria-label={name} onClick={onPick} dangerouslySetInnerHTML={{ __html: svg ?? "" }} />
      <span className="tpl-name">{name}</span>
      {onDelete && (
        <button className="tpl-del link-btn" aria-label={t("designer.deleteTemplate", { name })} title={t("designer.deleteTemplate", { name })} onClick={onDelete}>
          ×
        </button>
      )}
    </div>
  );
}

/** One whole key style drawn with the key's own label and icon; a click puts it on or takes it off. */
function SkinTile({ look, skin, onPick }: { look: Look; skin: Skin; onPick: () => void }) {
  const svg = usePreview("key", { ...look, skin });
  const on = look.skin === skin;
  const name = t(`skin.${skin}` as Key);
  return (
    <div className="tpl-tile">
      <button role="option" aria-selected={on} className={`tpl-key${on ? " is-active" : ""}`} title={name} aria-label={name} onClick={onPick} dangerouslySetInnerHTML={{ __html: svg ?? "" }} />
      <span className="tpl-name">{name}</span>
    </div>
  );
}

/** The motions a skin was made with, offered in Extras while the key wears it. */
const SKIN_MOTIONS: Partial<Record<Skin, NonNullable<Look["motion"]>[]>> = { film: ["film"], glitch: ["glitch"], neon: ["flicker", "sparks"] };

/** A skin on (with the motion it was made with) or off (its own motion goes with it). */
export function withSkin(look: Look, skin: Skin): Look {
  if (look.skin === skin) return { ...look, skin: undefined, motion: look.motion === SKIN_MOTION[skin] ? undefined : look.motion };
  const own = look.skin && look.motion === SKIN_MOTION[look.skin];
  return { ...look, skin, motion: SKIN_MOTION[skin] ?? (own ? undefined : look.motion) };
}

function Seg<V extends string | undefined>({ label, value, options, onChange }: { label: string; value: V; options: [V, string][]; onChange: (v: V) => void }) {
  return (
    <div className="field">
      <span className="label">{label}</span>
      <div className="seg full designer-seg" role="radiogroup" aria-label={label}>
        {options.map(([v, name]) => (
          <button key={v ?? "auto"} role="radio" aria-checked={value === v} className={`tab${value === v ? " is-active" : ""}`} onClick={() => onChange(v)}>
            {name}
          </button>
        ))}
      </div>
    </div>
  );
}

/**
 * The full icon editor of a key (owner's "1 a" twice, 2026-10-25; mockup variant A with the
 * templates of C as its first tab): templates, background, icon, label and extras, in the key's
 * Look panel; the preview is the one above it and the key on the deck.
 */
export function KeyDesigner({ look, onChange, others, onApplyTo, appIcon }: { look: Look; onChange: (l: Look) => void; others: OtherKey[]; onApplyTo: (ids: string[]) => Promise<unknown>; appIcon?: () => void }) {
  const [tab, setTab] = useState<Tab>("templates");
  const [mine, setMine] = useState<LookTemplate[]>([]);
  const [naming, setNaming] = useState<string | null>(null);
  const [picking, setPicking] = useState<Set<string> | null>(null);
  const [applied, setApplied] = useState<string | null>(null);
  const [fonts, setFonts] = useState<string[]>([]);
  useEffect(() => {
    if (tab === "label" && fonts.length === 0) api.deckFonts().then(setFonts, () => {});
  }, [tab, fonts.length]);
  useEffect(() => {
    // anything but a list (an older engine, the browser demo) is no own templates, never a crash
    api.lookTemplates().then((list) => setMine(Array.isArray(list) ? list : []), () => setMine([]));
  }, []);
  const saveMine = (next: LookTemplate[]) => {
    setMine(next);
    api.setLookTemplates(next).catch(() => api.lookTemplates().then((list) => setMine(Array.isArray(list) ? list : []), () => {}));
  };
  const kind = backgroundKind(look);
  const angle = look.angle ?? 180;

  return (
    <div className="designer">
      <div className="seg full designer-tabs" role="tablist" aria-label={t("designer.title")}>
        {TABS.map((id) => (
          <button key={id} role="tab" aria-selected={tab === id} className={`tab${tab === id ? " is-active" : ""}`} onClick={() => setTab(id)}>
            {t(`designer.tab.${id}`)}
          </button>
        ))}
      </div>

      {tab === "templates" && (
        <>
          <span className="label">{t("designer.skins")}</span>
          <div className="tpl-grid" role="listbox" aria-label={t("designer.skins")}>
            {SKINS.map((skin) => (
              <SkinTile key={skin} look={look} skin={skin} onPick={() => onChange(withSkin(look, skin))} />
            ))}
          </div>
          <p className="help">{t("designer.skinsHelp")}</p>
          <span className="label">{t("designer.tab.templates")}</span>
          <p className="help">{t("designer.templatesHelp")}</p>
          <div className="tpl-grid" role="listbox" aria-label={t("designer.tab.templates")}>
            {BUILT_IN.map((b) => (
              <TemplateTile key={b.id} look={look} template={b.look} name={t(`template.${b.id}` as Key)} onPick={() => onChange(withTemplate(look, b.look))} />
            ))}
            {mine.map((m, i) => (
              <TemplateTile key={`mine-${i}`} look={look} template={m.look} name={m.name} onPick={() => onChange(withTemplate(look, m.look))} onDelete={() => saveMine(mine.filter((_, j) => j !== i))} />
            ))}
          </div>
          {naming === null ? (
            <button className="tool small align-start" onClick={() => setNaming("")}>
              {t("designer.saveTemplate")}
            </button>
          ) : (
            <form
              className="row designer-name"
              onSubmit={(e) => {
                e.preventDefault();
                if (!naming.trim()) return;
                saveMine([...mine, { name: naming.trim(), look: templateOf(look) }]);
                setNaming(null);
              }}
            >
              <input className="input grow" autoFocus maxLength={40} aria-label={t("designer.templateName")} placeholder={t("designer.templateName")} value={naming} onChange={(e) => setNaming(e.target.value)} />
              <button className="tool small" type="submit" disabled={!naming.trim()}>
                {t("designer.save")}
              </button>
              <button className="tool small ghost" type="button" onClick={() => setNaming(null)}>
                {t("designer.cancel")}
              </button>
            </form>
          )}
        </>
      )}

      {tab === "background" && (
        <>
          <Seg
            label={t("designer.kind")}
            value={kind}
            options={[
              ["flat", t("designer.kind.flat")],
              ["gradient", t("designer.kind.gradient")],
              ["outline", t("designer.kind.outline")],
            ]}
            onChange={(k) => onChange(withKind(look, k))}
          />
          <div className="look-colors">
            <label className="color">
              <input type="color" value={look.bg ?? HOUSE_BG} onChange={(e) => onChange({ ...look, bg: e.target.value })} />
              {t(kind === "gradient" ? "designer.from" : kind === "outline" ? "designer.frameColor" : "look.background")}
            </label>
            {kind === "gradient" && (
              <label className="color">
                <input type="color" value={look.bg2 ?? look.bg ?? HOUSE_BG} onChange={(e) => onChange({ ...look, bg2: e.target.value })} />
                {t("designer.to")}
              </label>
            )}
          </div>
          {kind === "gradient" && (
            <label className="field">
              <span className="label">
                {t("designer.angle")} <span className="tx2">{angle}°</span>
              </span>
              <input type="range" min={0} max={345} step={15} value={angle} onChange={(e) => onChange({ ...look, angle: Number(e.target.value) })} />
            </label>
          )}
          <Seg
            label={t("designer.finish")}
            value={finish(look)}
            options={[
              ["none", t("designer.finish.none")],
              ["glass", t("designer.finish.glass")],
              ["matte", t("designer.finish.matte")],
            ]}
            onChange={(f) => onChange(withFinish(look, f))}
          />
          <div className="field">
            <span className="label">{t("look.presets")}</span>
            <div className="swatches" role="listbox" aria-label={t("look.presets")}>
              {BACKGROUNDS.map(([bg, fg]) => (
                <button key={bg} role="option" aria-selected={look.bg === bg} className={`swatch${look.bg === bg ? " is-active" : ""}`} style={{ background: bg, color: fg }} title={bg} onClick={() => onChange({ ...look, bg, fg })}>
                  Aa
                </button>
              ))}
            </div>
          </div>
        </>
      )}

      {tab === "icon" && (
        <>
          <div className="look-colors">
            <label className="color">
              <input type="color" value={look.iconColor ?? look.fg ?? HOUSE_LABEL} onChange={(e) => onChange({ ...look, iconColor: e.target.value })} />
              {t("look.iconColor")}
            </label>
          </div>
          <Seg
            label={t("look.iconSize")}
            value={look.iconSize}
            options={[
              [undefined, t("look.auto")],
              ["small", t("look.size.small")],
              ["large", t("look.size.large")],
            ]}
            onChange={(iconSize) => onChange({ ...look, iconSize })}
          />
          <p className="help">{t("designer.iconHelp")}</p>
          {appIcon && (
            <button className="tool small align-start" onClick={appIcon}>
              {t("look.appIcon")}
            </button>
          )}
        </>
      )}

      {tab === "label" && (
        <>
          <div className="look-colors">
            <label className="color">
              <input type="color" value={look.fg ?? HOUSE_LABEL} onChange={(e) => onChange({ ...look, fg: e.target.value })} />
              {t("look.text")}
            </label>
          </div>
          <Seg
            label={t("look.labelPos")}
            value={look.labelPos}
            options={[
              [undefined, t("look.auto")],
              ["top", t("look.pos.top")],
              ["middle", t("look.pos.middle")],
              ["bottom", t("look.pos.bottom")],
            ]}
            onChange={(labelPos) => onChange({ ...look, labelPos })}
          />
          <label className="field">
            <span className="label">{t("designer.font")}</span>
            <select className="input" value={look.font ?? ""} onChange={(e) => onChange({ ...look, font: e.target.value || undefined })}>
              <option value="">{t("designer.fontDeck")}</option>
              {look.font && !fonts.includes(look.font) && <option value={look.font}>{look.font}</option>}
              {fonts.map((f) => (
                <option key={f} value={f}>
                  {f}
                </option>
              ))}
            </select>
          </label>
          <label className="field">
            <span className="label">{t("look.labelSize")}</span>
            <select className="input" value={look.labelSize ?? ""} onChange={(e) => onChange({ ...look, labelSize: e.target.value ? Number(e.target.value) : undefined })}>
              <option value="">{t("look.auto")}</option>
              {LABEL_SIZES.map((z) => (
                <option key={z} value={z}>
                  {z}
                </option>
              ))}
            </select>
          </label>
          <Seg
            label={t("designer.effect")}
            value={look.labelEffect}
            options={[
              [undefined, t("look.auto")],
              ["shadow", t("designer.effect.shadow")],
              ["outline", t("designer.effect.outline")],
            ]}
            onChange={(labelEffect) => onChange({ ...look, labelEffect })}
          />
        </>
      )}

      {tab === "extras" && (
        <>
          <div className="row designer-badge">
            <label className="field grow">
              <span className="label">{t("designer.badge")}</span>
              <input className="input" maxLength={3} placeholder={t("designer.badgeHint")} value={look.badge ?? ""} onChange={(e) => onChange({ ...look, badge: e.target.value || undefined })} />
            </label>
            <label className="color">
              <input type="color" value={look.badgeColor ?? "#e5484d"} onChange={(e) => onChange({ ...look, badgeColor: e.target.value })} />
              {t("designer.badgeColor")}
            </label>
          </div>
          <Seg
            label={t("designer.pattern")}
            value={look.pattern}
            options={[
              [undefined, t("designer.finish.none")],
              ["stripes", t("designer.pattern.stripes")],
              ["dots", t("designer.pattern.dots")],
              ["grid", t("designer.pattern.grid")],
              ["noise", t("designer.pattern.noise")],
              ["waves", t("designer.pattern.waves")],
              ["hex", t("designer.pattern.hex")],
            ]}
            onChange={(pattern) => onChange({ ...look, pattern, patternOpacity: pattern ? look.patternOpacity : undefined, motion: !pattern && look.motion === "slide" ? undefined : look.motion })}
          />
          {look.pattern && (
            <label className="field">
              <span className="label">
                {t("designer.patternStrength")} <span className="tx2">{look.patternOpacity ?? 18}%</span>
              </span>
              <input type="range" min={5} max={60} step={1} value={look.patternOpacity ?? 18} onChange={(e) => onChange({ ...look, patternOpacity: Number(e.target.value) })} />
            </label>
          )}
          <Seg
            label={t("designer.motion")}
            value={look.motion}
            options={[
              [undefined, t("designer.finish.none")],
              ["pulse", t("designer.motion.pulse")],
              ["glow", t("designer.motion.glow")],
              ["shine", t("designer.motion.shine")],
              ["sweep", t("designer.motion.sweep")],
              ["sparkle", t("designer.motion.sparkle")],
              ["flow", t("designer.motion.flow")],
              ...(look.pattern ? [["slide", t("designer.motion.slide")] as ["slide", string]] : []),
              ...(look.skin ? (SKIN_MOTIONS[look.skin] ?? []).map((m) => [m, t(`designer.motion.${m}` as Key)] as [NonNullable<Look["motion"]>, string]) : []),
            ]}
            onChange={(motion) => onChange({ ...look, motion })}
          />
          {look.motion && <p className="help">{t("designer.motionHelp")}</p>}
          {look.motion && FLASHING_MOTIONS.includes(look.motion) && <p className="help warn">{t("designer.motionFlashes")}</p>}
          <div className="row designer-ring">
            <label className="check">
              <input type="checkbox" checked={!!look.ring} onChange={(e) => onChange({ ...look, ring: e.target.checked ? "#f5c542" : undefined })} />
              {t("designer.ring")}
            </label>
            {look.ring && (
              <label className="color">
                <input type="color" value={look.ring} onChange={(e) => onChange({ ...look, ring: e.target.value })} />
                {t("designer.ringColor")}
              </label>
            )}
          </div>
        </>
      )}

      {tab === "when" && <LookRules look={look} onChange={onChange} />}

      {/* the same look on other keys of the page: like Paste look, the label, icon and picture stay theirs */}
      {others.length > 0 &&
        (picking === null ? (
          <>
            <button className="tool small align-start" onClick={() => (setPicking(new Set()), setApplied(null))}>
              {t("designer.applyOthers")}
            </button>
            {applied && (
              <p className="help" role="status">
                {applied}
              </p>
            )}
          </>
        ) : (
          <div className="designer-apply" role="group" aria-label={t("designer.applyOthers")}>
            <p className="help">{t("designer.applyHelp")}</p>
            <div className="apply-list">
              {others.map((o) => (
                <label key={o.id} className="check">
                  <input
                    type="checkbox"
                    checked={picking.has(o.id)}
                    onChange={(e) => {
                      const next = new Set(picking);
                      if (e.target.checked) next.add(o.id);
                      else next.delete(o.id);
                      setPicking(next);
                    }}
                  />
                  {o.name}
                </label>
              ))}
            </div>
            <div className="row">
              <button className="tool small" disabled={picking.size === 0} onClick={() => {
                  const ids = [...picking];
                  setPicking(null);
                  onApplyTo(ids).then(() => setApplied(t("designer.applied", { count: ids.length })), (e) => setApplied(String(e)));
                }}
              >
                {t("designer.apply", { n: picking.size })}
              </button>
              <button className="tool small ghost" onClick={() => setPicking(new Set(others.map((o) => o.id)))}>
                {t("designer.all")}
              </button>
              <button className="tool small ghost" onClick={() => setPicking(null)}>
                {t("designer.cancel")}
              </button>
            </div>
          </div>
        ))}
    </div>
  );
}
