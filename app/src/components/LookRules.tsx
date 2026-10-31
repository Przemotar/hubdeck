import { useEffect, useState } from "react";
import { api, type AppInfo, type Look, type LookRule, type RuleCondition } from "../engine";
import { t, type Key } from "../i18n";
import { Icon } from "./Icon";

const KINDS: RuleCondition["kind"][] = ["metric", "playing", "micMuted", "appFront", "time"];
const METRICS = ["cpu", "ram", "gpu", "cpuTemp", "gpuTemp"] as const;

/** A new rule of a kind, with a sensible start (CPU over 80 %, the evening). */
function fresh(kind: RuleCondition["kind"]): RuleCondition {
  switch (kind) {
    case "metric":
      return { kind, metric: "cpu", above: 80 };
    case "appFront":
      return { kind, app: "" };
    case "time":
      return { kind, from: "22:00", to: "06:00" };
    default:
      return { kind };
  }
}

/**
 * Looks by conditions (owner's "2 a", 2026-10-25, step 4; as Bitfocus Companion's feedbacks): each
 * rule says when, and how the key looks then; a later rule wins. The deck checks them four times
 * a second.
 */
export function LookRules({ look, onChange }: { look: Look; onChange: (l: Look) => void }) {
  const rules = look.rules ?? [];
  const [apps, setApps] = useState<AppInfo[]>([]);
  useEffect(() => {
    api.apps().then((a) => setApps(Array.isArray(a) ? a : []), () => {});
  }, []);
  const set = (next: LookRule[]) => onChange({ ...look, rules: next.length ? next : undefined });
  const at = (i: number, r: LookRule) => set(rules.map((x, j) => (j === i ? r : x)));
  const appName = (exe: string) => exe.split(/[\\/]/).pop()!;
  return (
    <div className="rules">
      <p className="help">{t("rules.help")}</p>
      {rules.map((r, i) => (
        <section key={i} className="rule" aria-label={t("rules.rule", { n: i + 1 })}>
          <div className="row rule-when">
            <select className="input" aria-label={t("rules.when")} value={r.when.kind} onChange={(e) => at(i, { ...r, when: fresh(e.target.value as RuleCondition["kind"]) })}>
              {KINDS.map((k) => (
                <option key={k} value={k}>
                  {t(`rules.kind.${k}` as Key)}
                </option>
              ))}
            </select>
            <button className="tool small ghost icon-only" aria-label={t("rules.remove", { n: i + 1 })} title={t("rules.remove", { n: i + 1 })} onClick={() => set(rules.filter((_, j) => j !== i))}>
              <Icon name="trash" />
            </button>
          </div>
          {r.when.kind === "metric" && (
            <div className="row rule-when">
              <select className="input" aria-label={t("rules.metric")} value={r.when.metric} onChange={(e) => at(i, { ...r, when: { ...r.when, metric: e.target.value } as RuleCondition })}>
                {METRICS.map((m) => (
                  <option key={m} value={m}>
                    {t(`rules.metric.${m}` as Key)}
                  </option>
                ))}
              </select>
              <span className="label">{t("rules.above")}</span>
              <input className="input rule-num" type="number" aria-label={t("rules.above")} min={0} max={150} value={r.when.above} onChange={(e) => at(i, { ...r, when: { ...r.when, above: Number(e.target.value) } as RuleCondition })} />
            </div>
          )}
          {r.when.kind === "appFront" && (
            <select className="input" aria-label={t("rules.app")} value={r.when.app} onChange={(e) => at(i, { ...r, when: { kind: "appFront", app: e.target.value } })}>
              <option value="">{t("rules.pickApp")}</option>
              {(() => {
                // the app the rule names, kept in the list when it is not running now
                const app = r.when.kind === "appFront" ? r.when.app : "";
                return app && !apps.some((a) => appName(a.exe) === app) ? <option value={app}>{app}</option> : null;
              })()}
              {apps.map((a) => (
                <option key={a.exe} value={appName(a.exe)}>
                  {a.title || appName(a.exe)}
                </option>
              ))}
            </select>
          )}
          {r.when.kind === "time" && (
            <div className="row rule-when">
              <input className="input" type="time" aria-label={t("rules.from")} value={r.when.from} onChange={(e) => at(i, { ...r, when: { ...r.when, from: e.target.value } as RuleCondition })} />
              <span className="label">–</span>
              <input className="input" type="time" aria-label={t("rules.to")} value={r.when.to} onChange={(e) => at(i, { ...r, when: { ...r.when, to: e.target.value } as RuleCondition })} />
            </div>
          )}
          <span className="label">{t("rules.then")}</span>
          <div className="row rule-then">
            {/* each part on or off, with its color: a rule changes only what it turns on */}
            <label className="check">
              <input type="checkbox" checked={!!r.look.bg} onChange={(e) => at(i, { ...r, look: { ...r.look, bg: e.target.checked ? "#c0392b" : undefined } })} />
              {t("look.background")}
            </label>
            {r.look.bg && <input type="color" aria-label={t("rules.bgColor")} value={r.look.bg} onChange={(e) => at(i, { ...r, look: { ...r.look, bg: e.target.value } })} />}
            <label className="check">
              <input type="checkbox" checked={!!r.look.ring} onChange={(e) => at(i, { ...r, look: { ...r.look, ring: e.target.checked ? "#f5c542" : undefined } })} />
              {t("designer.frameColor")}
            </label>
            {r.look.ring && <input type="color" aria-label={t("designer.ringColor")} value={r.look.ring} onChange={(e) => at(i, { ...r, look: { ...r.look, ring: e.target.value } })} />}
            <select className="input" aria-label={t("designer.motion")} value={r.look.motion ?? ""} onChange={(e) => at(i, { ...r, look: { ...r.look, motion: (e.target.value || undefined) as Look["motion"] } })}>
              <option value="">{t("rules.noMotion")}</option>
              <option value="pulse">{t("designer.motion.pulse")}</option>
              <option value="shine">{t("designer.motion.shine")}</option>
              <option value="glow">{t("designer.motion.glow")}</option>
              <option value="sweep">{t("designer.motion.sweep")}</option>
              <option value="sparkle">{t("designer.motion.sparkle")}</option>
            </select>
          </div>
        </section>
      ))}
      <button className="tool small align-start" onClick={() => set([...rules, { when: fresh("metric"), look: { bg: "#c0392b" } }])}>
        <Icon name="plus" />
        {t("rules.add")}
      </button>
    </div>
  );
}

/** The values a label can show, to put in with a click (`{cpu}` and the like, `rules.rs`). */
export const LABEL_VALUES = ["cpu", "ram", "gpu", "cpuTemp", "gpuTemp", "time", "date", "song", "artist", "app"] as const;

export function LabelValues({ label, onChange }: { label: string; onChange: (label: string) => void }) {
  return (
    <div className="label-values" role="group" aria-label={t("rules.values")}>
      <span className="label">{t("rules.values")}</span>
      {LABEL_VALUES.map((v) => (
        <button key={v} className="chip small" title={t(`rules.value.${v}` as Key)} onClick={() => onChange(`${label}${label && !label.endsWith(" ") && !label.endsWith("\n") ? " " : ""}{${v}}`)}>
          {`{${v}}`}
        </button>
      ))}
    </div>
  );
}
