import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { t, tNode, useLanguage, type Key } from "../i18n";
import { Icon } from "./Icon";

/** Mirrors PanelGeometry in src-tauri/src/engine/render.rs. */
interface Geometry {
  x0: number;
  y0: number;
  dx: number;
  dy: number;
  w: number;
  h: number;
}

interface Calibration {
  geometry: Geometry;
  selected: number;
}

const PARAMS: (keyof Geometry)[] = ["x0", "y0", "dx", "dy", "w", "h"];

const DEFAULTS: Geometry = { x0: 0, y0: 0, dx: 100, dy: 99, w: 79, h: 75 };

const VARIANTS: [string, Key, Key][] = [
  ["A", "compare.a", "compare.aHelp"],
  ["B", "compare.b", "compare.bHelp"],
  ["C", "compare.c", "compare.cHelp"],
];

/** Switch the keys between drawing methods to compare them on the hardware. */
export function ComparePicker() {
  useLanguage();
  const [variant, setVariant] = useState<string | null>(null);
  const pick = (v: string | null) => {
    setVariant(v);
    invoke("render_compare", { variant: v }).catch((e) => console.error(e));
  };
  useEffect(() => () => void invoke("render_compare", { variant: null }).catch(() => {}), []);
  return (
    <section className="sec compare">
      <h3 className="sec-title">{t("compare.title")}</h3>
      <p className="help">{t("compare.help")}</p>
      <div className="row wrap">
        {VARIANTS.map(([v, label, help]) => (
          <button key={v} className={`tool small${variant === v ? " is-active" : ""}`} title={t(help)} onClick={() => pick(v)}>
            {t(label)}
          </button>
        ))}
        <button className={`tool small${variant === null ? " is-active" : ""}`} onClick={() => pick(null)}>
          {t("compare.off")}
        </button>
      </div>
      {variant && <p className="help">{t(VARIANTS.find(([v]) => v === variant)?.[2] ?? "compare.off")}</p>}
    </section>
  );
}

/** Fine-tune where the keys sit on the panel, with double borders shown on the hardware. */
export function CalibrationPanel({ onClose }: { onClose: () => void }) {
  useLanguage();
  const [cal, setCal] = useState<Calibration | null>(null);

  useEffect(() => {
    invoke<Calibration>("calibration_start").then(setCal);
    const un = listen<Calibration | null>("engine://calibration", (e) => e.payload && setCal(e.payload));
    return () => {
      un.then((u) => u());
    };
  }, []);

  if (!cal) return <aside className="inspector sectioned side-panel" />;

  const push = (geometry: Geometry, selected = cal.selected) => {
    setCal({ geometry, selected });
    invoke("calibration_set", { geometry, selected }).catch((e) => console.error(e));
  };
  const finish = (save: boolean) => invoke("calibration_stop", { save }).finally(onClose);

  return (
    <aside className="inspector sectioned side-panel">
      <div className="insp-head panel-head">
        <span className="ph-icon">
          <Icon name="crosshair" />
        </span>
        <div className="grow">
          <div className="crumb">{t("module.keys")}</div>
          <h2>{t("calib.title")}</h2>
        </div>
        <button className="icon-btn" onClick={() => finish(false)} aria-label={t("calib.cancel")} title={t("calib.cancel")}>
          <Icon name="x" />
        </button>
      </div>
      <section className="sec">
        <p className="help">
          {tNode("calib.help", {
            green: <b style={{ color: "#2dff6a" }}>{t("calib.green")}</b>,
            red: <b style={{ color: "#ff2d2d" }}>{t("calib.red")}</b>,
          })}
        </p>
        <div className="calib-tip">
          <Icon name="info" />
          <span>{t("calib.deck")}</span>
        </div>
      </section>
      <section className="sec">
        <div className="calib-list" role="list">
          {PARAMS.map((key, i) => {
            const name = t(`calib.${key}`);
            return (
              <div key={key} role="listitem" className={`calib-row${cal.selected === i ? " is-active" : ""}`}>
                <button className="calib-name" onClick={() => push(cal.geometry, i)}>
                  <b>{name}</b>
                  <small>{t(`calib.${key}Help`)}</small>
                </button>
                <button className="icon-btn" aria-label={t("calib.minus", { name })} onClick={() => push({ ...cal.geometry, [key]: cal.geometry[key] - 1 }, i)}>
                  <Icon name="minus" />
                </button>
                <span className="mono calib-value">{cal.geometry[key]}</span>
                <button className="icon-btn" aria-label={t("calib.plus", { name })} onClick={() => push({ ...cal.geometry, [key]: cal.geometry[key] + 1 }, i)}>
                  <Icon name="plus" />
                </button>
              </div>
            );
          })}
        </div>
      </section>
      {/* diagnostic, dev builds only (owner's decision 2026-10-09) */}
      {import.meta.env.DEV && <ComparePicker />}
      <section className="sec end">
        <div className="row">
          <button className="tool primary grow" onClick={() => finish(true)}>
            <Icon name="check" />
            {t("calib.save")}
          </button>
          <button className="tool" onClick={() => finish(false)}>
            {t("calib.cancel")}
          </button>
        </div>
        <button className="tool ghost small align-start" onClick={() => push(DEFAULTS)}>
          <Icon name="rotate-cw" />
          {t("calib.reset")}
        </button>
      </section>
    </aside>
  );
}
