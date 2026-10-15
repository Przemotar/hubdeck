// Build the deck step by step (owner, 2026-10-13, layout A): a page of its own; one control at a
// time, a gallery of options for it in the middle, each tried on the real deck before it is picked;
// the talk in a column on the left, the deck so far and the steps on the right. Everything goes
// into a working draft, which the review saves as a profile or throws away.
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useMemo, useRef, useState } from "react";

import type { ControlRef, LiveState } from "../deck";
import { api, svgDataUrl, type Binding, type EngineView, type Profile } from "../engine";
import { t, useLanguage } from "../i18n";
import type { Placement } from "../layout";
import { DeckView } from "./DeckView";
import { Icon } from "./Icon";

interface BuilderOption {
  id: string;
  group: string;
  name: string;
  does: string;
  source: string;
  setup: boolean;
  binding: Binding;
  svg: string | null;
  /** The double press and hold that go with it (`double`, `hold`). */
  extra?: Record<string, Binding>;
}

/** One step: a control (the dial, one knob) or the review at the end. */
interface Step {
  key: string;
  label: string;
  /** Control id in the profile (`dial`, `knobs.0`); null for the review. */
  control: string | null;
  kind: "dial" | "knobs" | null;
  ref: ControlRef | null;
}


export function Builder({ live, view, saved, onPlace, onClose }: { live: LiveState; view: EngineView; saved: Record<string, Placement>; onPlace: (key: string, p: Placement | null) => void; onClose: () => void }) {
  useLanguage();
  const [draft, setDraft] = useState<Profile | null>(null);
  const [options, setOptions] = useState<Record<string, BuilderOption[]>>({});
  const [at, setAt] = useState(0);
  const [trying, setTrying] = useState<Record<string, string>>({});
  const [picked, setPicked] = useState<Record<string, string>>({});
  const [filter, setFilter] = useState("");
  const [name, setName] = useState(t("builder.defaultName"));
  const [saving, setSaving] = useState(false);
  const kept = useRef(false);
  const draftRef = useRef<Profile | null>(null);

  // the steps follow the modules attached: the dial, then each knob, then the review
  const steps = useMemo<Step[]>(() => {
    const out: Step[] = [];
    const dial = live.deck.modules.find((m) => m.kind === "dial");
    if (dial) out.push({ key: "dial", label: t("builder.stepDial"), control: "dial", kind: "dial", ref: { module: dial.id, kind: "dial", index: 0 } });
    const knobs = live.deck.modules.find((m) => m.kind === "knobs");
    if (knobs) for (let i = 0; i < 3; i++) out.push({ key: `knobs.${i}`, label: t("builder.stepKnob", { n: i + 1 }), control: `knobs.${i}`, kind: "knobs", ref: { module: knobs.id, kind: "knobs", index: i } });
    out.push({ key: "review", label: t("builder.stepReview"), control: null, kind: null, ref: null });
    return out;
  }, [live.deck.modules]);
  const step = steps[Math.min(at, steps.length - 1)];

  // a working draft from the profile on the deck now; thrown away unless the review keeps it
  useEffect(() => {
    let live = true;
    invoke<Profile>("builder_draft", { from: view.state?.active ?? null, name: t("builder.defaultName") }).then((p) => {
      if (!live) return;
      draftRef.current = p;
      setDraft(p);
    });
    for (const kind of ["dial", "knobs"]) invoke<BuilderOption[]>("builder_options", { kind }).then((o) => live && setOptions((all) => ({ ...all, [kind]: o })));
    return () => {
      live = false;
      const d = draftRef.current;
      if (d && !kept.current) {
        api.tryProfile(null).catch(() => {});
        api.settleDraft(d.id, false).catch(() => {});
      }
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  /** Put an option on the draft's control and show the draft on the deck. */
  const put = async (o: BuilderOption) => {
    const d = draftRef.current;
    if (!d || !step.control) return;
    const controls = { ...d.controls, [step.control]: o.binding };
    // each option brings its own double press and hold; without them, the ones from before go
    for (const g of ["double", "hold"]) {
      const id = `${step.control}:${g}`;
      if (o.extra?.[g]) controls[id] = o.extra[g];
      else delete controls[id];
    }
    const next: Profile = { ...d, controls };
    draftRef.current = next;
    setDraft(next);
    await api.saveProfile(next);
    await api.tryProfile(next.id);
    setTrying((tr) => ({ ...tr, [step.control as string]: o.id }));
  };
  const pick = async (o: BuilderOption) => {
    await put(o);
    setPicked((p) => ({ ...p, [step.control as string]: o.id }));
    setFilter("");
    setAt((i) => Math.min(i + 1, steps.length - 1));
  };
  const save = async () => {
    const d = draftRef.current;
    if (!d) return;
    setSaving(true);
    const named: Profile = { ...d, name: name.trim() || t("builder.defaultName") };
    await api.saveProfile(named);
    await api.settleDraft(named.id, true);
    kept.current = true;
    await api.activate(named.id);
    onClose();
  };

  const all = step.kind ? options[step.kind] ?? [] : [];
  const words = filter.trim().toLowerCase();
  const shown = words ? all.filter((o) => `${o.name} ${o.does} ${o.source} ${o.group}`.toLowerCase().includes(words)) : all;
  const groups = [...new Set(shown.map((o) => o.group))];
  const iconSvg = (name?: string) => view.icons.find((i) => i.name === name)?.svg ?? "";
  const talk = step.kind === "dial" ? t("builder.talkDial") : step.kind === "knobs" ? t("builder.talkKnob", { n: (step.ref?.index ?? 0) + 1 }) : t("builder.talkReview");
  const chips = step.kind === "dial" ? [t("builder.chipMusic"), t("builder.chipWork"), t("builder.chipAll")] : step.kind === "knobs" ? [t("builder.chipVolume"), t("builder.chipMusic"), t("builder.chipAll")] : [];
  const chipWords: Record<string, string> = { [t("builder.chipMusic")]: "song", [t("builder.chipWork")]: "load", [t("builder.chipVolume")]: "volume", [t("builder.chipAll")]: "" };

  return (
    <div className="builder-page">
      <header className="settings-head builder-head">
        <button className="tool ghost" onClick={onClose}>
          <Icon name="chevron-left" />
          {t("settings.back")}
        </button>
        <h2>{t("builder.title")}</h2>
        <nav className="builder-steps" aria-label={t("builder.steps")}>
          {steps.map((s, i) => (
            <button key={s.key} className={`builder-step${i === at ? " is-current" : ""}${s.control && picked[s.control] ? " is-done" : ""}`} aria-current={i === at ? "step" : undefined} onClick={() => setAt(i)}>
              {s.control && picked[s.control] && <Icon name="check" />}
              {s.label}
            </button>
          ))}
        </nav>
      </header>
      <div className="builder-body">
        <aside className="builder-talk" aria-label={t("builder.talk")}>
          <div className="builder-msgs">
            <p className="builder-msg">{talk}</p>
            {chips.length > 0 && (
              <div className="builder-chips">
                {chips.map((c) => (
                  <button key={c} className="builder-chip" onClick={() => setFilter(chipWords[c] ?? "")}>
                    {c}
                  </button>
                ))}
              </div>
            )}
          </div>
          <label className="builder-input">
            <span className="sr-only">{t("builder.narrow")}</span>
            <input value={filter} onChange={(e) => setFilter(e.target.value)} placeholder={t("builder.narrow")} />
          </label>
        </aside>
        <main className="builder-main">
          {step.kind ? (
            <>
              <div className="builder-title">
                <h3>{step.label}</h3>
                <span>{t("builder.stepOf", { n: at + 1, count: steps.length })}</span>
              </div>
              {groups.map((g) => (
                <section key={g} className="builder-group">
                  <h4 className="builder-group-title">{g}</h4>
                  <div className="builder-grid">
                    {shown
                      .filter((o) => o.group === g)
                      .map((o) => {
                        const isTrying = step.control && trying[step.control] === o.id;
                        const isPicked = step.control && picked[step.control] === o.id;
                        return (
                          // a click anywhere on the card tries it (owner, 2026-10-14: not only the button)
                          <article
                            key={o.id}
                            className={`builder-card${isTrying ? " is-trying" : ""}${isPicked ? " is-picked" : ""}`}
                            role="button"
                            tabIndex={0}
                            aria-pressed={!!isTrying}
                            onClick={() => draft && !isTrying && put(o)}
                            onKeyDown={(e) => {
                              if ((e.key === "Enter" || e.key === " ") && e.target === e.currentTarget && draft) {
                                e.preventDefault();
                                put(o);
                              }
                            }}
                          >
                            {o.svg ? (
                              <img className="builder-face" src={svgDataUrl(o.svg)} alt="" />
                            ) : (
                              <span className="builder-knob" aria-hidden="true" dangerouslySetInnerHTML={{ __html: iconSvg(o.binding.look.icon) }} />
                            )}
                            <h5>{o.name}</h5>
                            <span className="builder-does">{o.does}</span>
                            <span className={`builder-source${o.setup ? " needs-setup" : ""}`}>
                              {o.source}
                              {o.setup ? ` · ${t("builder.needsSetup")}` : ""}
                            </span>
                            <div className="builder-card-actions">
                              <button
                                className="tool small"
                                onClick={(e) => {
                                  e.stopPropagation();
                                  put(o);
                                }}
                                disabled={!draft}
                              >
                                {isTrying ? t("builder.onDeck") : t("builder.try")}
                              </button>
                              <button
                                className="tool small primary"
                                onClick={(e) => {
                                  e.stopPropagation();
                                  pick(o);
                                }}
                                disabled={!draft}
                              >
                                {t("builder.pick")}
                              </button>
                            </div>
                          </article>
                        );
                      })}
                  </div>
                </section>
              ))}
              {shown.length === 0 && <p className="help">{t("builder.nothing")}</p>}
              <p className="help">{t("builder.tryHelp")}</p>
              <div className="builder-nav">
                <button className="tool ghost" onClick={() => setAt((i) => Math.min(i + 1, steps.length - 1))}>
                  {t("builder.skip")}
                </button>
              </div>
            </>
          ) : (
            <div className="builder-review">
              <h3>{t("builder.reviewTitle")}</h3>
              <ul>
                {steps
                  .filter((s) => s.control)
                  .map((s) => {
                    const o = (s.kind ? options[s.kind] ?? [] : []).find((x) => x.id === picked[s.control as string]);
                    return (
                      <li key={s.key}>
                        <b>{s.label}</b>: {o ? o.name : t("builder.unchanged")}
                      </li>
                    );
                  })}
              </ul>
              <label className="field">
                <span className="label">{t("builder.name")}</span>
                <input className="input" value={name} onChange={(e) => setName(e.target.value)} />
              </label>
              <div className="row wrap">
                <button className="tool primary" onClick={save} disabled={!draft || saving}>
                  <Icon name="check" />
                  {t("builder.save")}
                </button>
                <button className="tool ghost" onClick={onClose}>
                  {t("builder.throwAway")}
                </button>
              </div>
            </div>
          )}
        </main>
        <aside className="builder-side" aria-label={t("builder.soFar")}>
          <h3 className="sec-title">{t("builder.soFar")}</h3>
          <div className="builder-deck">
            <DeckView live={live} selected={step.ref} onSelect={() => {}} saved={saved} onPlace={onPlace} arranging={false} profile={draft} page={null} />
          </div>
          <ol className="builder-list">
            {steps.map((s, i) => {
              const o = s.control && s.kind ? (options[s.kind] ?? []).find((x) => x.id === picked[s.control as string]) : null;
              return (
                <li key={s.key} className={i === at ? "is-current" : ""}>
                  {s.label}
                  {o ? `: ${o.name}` : ""}
                </li>
              );
            })}
          </ol>
        </aside>
      </div>
    </div>
  );
}
