import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { ask, open, save } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import type { ControlRef, LiveState, ModuleKind } from "../deck";
import { api, GAMES_ENABLED, pageReachable, setPreviewSalt, type AppInfo, type AppsState, type CrashReport, type LightSettings, LED_EFFECTS, METRICS, type EngineView, type GameSummary, type Imported, type Metric, type Moved, type PluginInfo, type CatalogPlugin, type InstalledItem, type PresetSummary, type Profile, type ProfileSummary, type Screens } from "../engine";
import { Icon } from "./Icon";
import { AssistantPanel, refreshNudge, useNudge, useSetUp } from "./Assistant";
import { InstalledPicker, pickImage } from "./Inspector";
import { ModuleThumb } from "./DeckView";
import { demoParam } from "../demo/params";
import { setTheme, useTheme, type Theme } from "../theme";
import { osName, updates, useAppVersion, useUpdateAvailable, type UpdateInfo } from "../update";
import { share, type GalleryEntry } from "../share";
import { detachedKinds, keysOfKind, MODULE_KINDS, ownedKey, placeModules, type Placement } from "../layout";
import { getLanguage, localizeError, localizeGame, localizePreset, t, useLanguage, type Key, type Lang } from "../i18n";

const moduleName = (kind: ModuleKind) => t(`module.${kind}`);

/** "Key 2", "Dial", "Knob 1": the name of a control as the user sees it. */
export const controlName = (c: ControlRef) => (c.kind === "dial" ? t("control.dial") : `${t(`control.${c.kind}`)} ${c.index + 1}`);

/** The name of a stored control id (`dial`, `faders.1`, `controls/keys.3`), with its page. */
function controlIdName(id: string, pages: Profile["pages"] = []): string {
  const [page, rest] = id.includes("/") ? (id.split("/", 2) as [string, string]) : [null, id];
  const [kind, index] = rest.split(":")[0].split(".");
  const name = kind === "dial" ? t("control.dial") : `${t(`control.${kind as ModuleKind}`)} ${Number(index) + 1}`;
  return page ? `${name} (${pages.find((p) => p.id === page)?.name ?? page})` : name;
}

/** A control id from the engine ("keys.2", "dial:hold", "page/knobs.0") as its name; other text as is. */
function controlLabel(id: string, pages: Profile["pages"] = []): string {
  return /^([^/]+\/)?((keys|knobs|faders|rollers)\.\d+|dial)(:|$)/.test(id) ? controlIdName(id, pages) : id;
}

/** The note after an import or "Adapt to my modules": "Volume: Dial → Knob 1; ...". */
export function movedNote(moved: Moved[] | undefined, pages: Profile["pages"] = []): string {
  if (!moved?.length) return "";
  const list = moved.map((m) => `${m.label}: ${controlIdName(m.from, pages)} → ${m.to.map((c) => controlIdName(c, pages)).join(" / ")}`).join("; ");
  return t("profile.moved", { list });
}

/** The app icon: a purple tile with a dial and a column of keys. */
export const Logo = () => (
  <svg className="logo" viewBox="0 0 64 64" aria-hidden="true">
    <rect x="2" y="2" width="60" height="60" rx="14" fill="#8b7cf6" />
    <circle cx="25" cy="32" r="14" fill="none" stroke="#fff" strokeWidth="5" />
    <line x1="25" y1="32" x2="25" y2="22" stroke="#fff" strokeWidth="5" strokeLinecap="round" />
    <rect x="44" y="12" width="10" height="10" rx="2.5" fill="#fff" />
    <rect x="44" y="27" width="10" height="10" rx="2.5" fill="#fff" />
    <rect x="44" y="42" width="10" height="10" rx="2.5" fill="#fff" />
  </svg>
);

export type LeftTab = "profiles" | "assistant";

/** Suggestions whose dot was seen (per viewer, a convenience: lost storage only brings a dot back). */
function nudgeSeen(subject: string): boolean {
  try {
    return (JSON.parse(localStorage.getItem("hubdeck.nudgeSeen") ?? "[]") as string[]).includes(subject);
  } catch {
    return false;
  }
}
function markNudgeSeen(subject: string) {
  try {
    const seen = JSON.parse(localStorage.getItem("hubdeck.nudgeSeen") ?? "[]") as string[];
    localStorage.setItem("hubdeck.nudgeSeen", JSON.stringify([...seen.filter((s) => s !== subject), subject].slice(-50)));
  } catch {
    // not remembered: the dot may come back once
  }
}

/**
 * The left column: Get started (the assistant) and the profiles list as two tabs. Actions are chosen
 * only in the right panel, where the control is known (owner's pick 1a, 2026-10-16, variant A).
 * Once set up, Profiles comes first and is open at start; Get started keeps the same places
 * (owner's pick 2a, 2026-10-16: "zawsze to samo" stays; docs/FLOWS.md difference 3).
 */
export function LeftPanel({ view, tab, onTab, onOpenProfile, onNewProfile, onBuild, onTour }: { view: EngineView; tab: LeftTab; onTab: (tab: LeftTab) => void; onOpenProfile: () => void; onNewProfile: () => void; onBuild?: () => void; onTour?: () => void }) {
  useLanguage();
  const setUp = useSetUp(view);
  // HubDeck's own suggestion (an app without a deck): a dot on the Assistant tab, looked at now and then
  const nudge = useNudge();
  // the dot goes once the tab was looked at with that suggestion in it, as badges do
  const [, seenNow] = useState(0);
  useEffect(() => {
    if (tab === "assistant" && nudge && !nudgeSeen(nudge.subject)) {
      markNudgeSeen(nudge.subject);
      seenNow((n) => n + 1);
    }
  }, [tab, nudge]);
  useEffect(() => {
    refreshNudge();
    const timer = setInterval(refreshNudge, 15 * 60 * 1000);
    return () => clearInterval(timer);
  }, []);
  return (
    // one width for every tab (owner, 2026-10-14: Get started widening the column looked odd);
    // Get started comes first and is open at start
    <aside data-guide="library" className="library" aria-label={tab === "profiles" ? t("profiles.title") : t("assistant.title")}>
      <div className={`lib-tabs seg full${setUp ? " is-set-up" : ""}`} role="tablist">
        <button role="tab" aria-selected={tab === "assistant"} className={`tab${tab === "assistant" ? " is-active" : ""}`} onClick={() => onTab("assistant")}>
          {t("assistant.tab")}
          {nudge && tab !== "assistant" && !nudgeSeen(nudge.subject) && <span className="tab-dot" aria-label={t("nudge.ask")} />}
        </button>
        <button role="tab" data-guide="profiles" aria-selected={tab === "profiles"} className={`tab${tab === "profiles" ? " is-active" : ""}`} onClick={() => onTab("profiles")}>
          {t("profiles.title")}
        </button>
      </div>
      {tab === "profiles" ? (
        <ProfilesList view={view} onOpenProfile={onOpenProfile} onNewProfile={onNewProfile} />
      ) : (
        <AssistantPanel view={view} onTour={onTour} onBuild={onBuild} />
      )}
    </aside>
  );
}

/** Ready-made profiles; picking one adds it as a new profile and makes it active. */
export function PresetList({ onPicked, kinds }: { onPicked?: () => void; kinds?: string[] }) {
  useLanguage();
  const [presets, setPresets] = useState<PresetSummary[]>([]);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    api.presets().then(setPresets).catch((e) => setError(String(e)));
  }, []);
  const [open, setOpen] = useState<string | null>(null);
  const row = (p: PresetSummary) => (
    <button
      key={p.id}
      className="menu-row compact preset-row"
      title={p.description}
      onClick={() =>
        api
          .addPreset(p.id, kinds)
          .then((profile) => api.activate(profile.id))
          .then(() => onPicked?.(), (e) => setError(String(e)))
      }
    >
      <span className="menu-text">
        <b>{p.name}</b>
      </span>
    </button>
  );
  // the test benches are for developing HubDeck: only in a development build
  const all = presets.map(localizePreset).filter((p) => import.meta.env.DEV || !p.test);
  // one line per group, only the clicked one opens (owner's pick 1A); the description is the tooltip
  const groups = PRESET_GROUPS.map(([id, icon]) => ({ id, icon, items: all.filter((p) => (p.group ?? "tools") === id) })).filter((g) => g.items.length);
  return (
    <div className="menu-list">
      {groups.map((g) => (
        <div key={g.id} className="preset-group">
          <button className={`menu-row compact group-row${open === g.id ? " is-open" : ""}`} aria-expanded={open === g.id} onClick={() => setOpen(open === g.id ? null : g.id)}>
            <span className={`menu-ic is-plain${g.id === "test" ? " is-test" : ""}`}>
              <Icon name={g.icon} />
            </span>
            <span className="menu-text">
              <b>{t(`presets.group.${g.id}` as Key)}</b>
            </span>
            <span className="count">{g.items.length}</span>
            <Icon name={open === g.id ? "chevron-down" : "chevron-right"} />
          </button>
          {open === g.id && g.items.map(row)}
        </div>
      ))}
      {error && <p className="help">{error}</p>}
    </div>
  );
}

/** The groups of ready-made profiles in menu order, with their icons. */
const PRESET_GROUPS: [string, string][] = [
  ["start", "compass"],
  ["streaming", "radio"],
  ["photo", "palette"],
  ["video", "video"],
  ["games", "gamepad-2"],
  ["tools", "terminal"],
  ["showcase", "sparkles"],
  ["test", "crosshair"],
];


/** Closes a popover on a click outside `ref` or on Escape. */
function useDismiss(open: boolean, ref: React.RefObject<HTMLElement | null>, close: () => void) {
  useEffect(() => {
    if (!open) return;
    const down = (e: MouseEvent) => ref.current && !ref.current.contains(e.target as Node) && close();
    const key = (e: KeyboardEvent) => e.key === "Escape" && close();
    document.addEventListener("mousedown", down);
    document.addEventListener("keydown", key);
    return () => {
      document.removeEventListener("mousedown", down);
      document.removeEventListener("keydown", key);
    };
  }, [open, ref, close]);
}

/** Apps with their icons (open now and seen recently), read again when the app in front changes. */
function useApps(foreground?: string): AppInfo[] {
  const [apps, setApps] = useState<AppInfo[]>([]);
  useEffect(() => {
    api.apps().then(setApps, () => {});
  }, [foreground]);
  return apps;
}

/** The exe part of a profile's app entry (`chrome.exe: YouTube` -> `chrome.exe`). */
const entryExe = (entry: string) => entry.split(":")[0].trim();

/** Icon of an app entry, if HubDeck has seen the app. */
function appIcon(apps: AppInfo[], entry: string): string | undefined {
  const exe = entryExe(entry).toLowerCase().replace(/\.exe$/, "");
  return apps.find((a) => a.exe.toLowerCase().replace(/\.exe$/, "") === exe)?.icon ?? linkedIcons.get(entryExe(entry).toLowerCase()) ?? undefined;
}

/** Icons of programs profiles link to that were not in front lately (games): asked once each. */
const linkedIcons = new Map<string, string | null>();
const askedIcons = new Set<string>();

/** Fetch the icons of these profiles' programs that the seen apps lack; re-renders when they come. */
function useLinkedIcons(profiles: ProfileSummary[], apps: AppInfo[]) {
  const [, setVersion] = useState(0);
  const seen = new Set(apps.map((a) => a.exe.toLowerCase()));
  const missing = [...new Set(profiles.flatMap((p) => p.apps.map((a) => entryExe(a).toLowerCase())))].filter((e) => e && !seen.has(e) && !askedIcons.has(e));
  const key = missing.join("|");
  useEffect(() => {
    if (!missing.length || !("__TAURI_INTERNALS__" in window)) return;
    missing.forEach((e) => askedIcons.add(e));
    api.appIcons(missing).then((found) => {
      for (const [exe, icon] of Object.entries(found ?? {})) linkedIcons.set(exe.toLowerCase(), icon);
      setVersion((v) => v + 1);
    }, () => {});
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);
}

/** Linked apps as chips with their real icons, and a picker of the apps open now or seen lately. */
function AppLinks({ apps, value, onChange, foreground }: { apps: AppInfo[]; value: string[]; onChange: (apps: string[]) => void; foreground: string }) {
  useLanguage();
  const [picking, setPicking] = useState(false);
  const ref = useRef<HTMLSpanElement>(null);
  const close = useCallback(() => setPicking(false), []);
  useDismiss(picking, ref, close);
  const linked = value.map((v) => entryExe(v).toLowerCase());
  const offer = apps.filter((a) => !linked.includes(a.exe.toLowerCase()));
  return (
    <div className="field">
      <div className="label">{t("profile.apps")}</div>
      <div className="row wrap">
        {value.map((entry) => {
          const icon = appIcon(apps, entry);
          return (
            <button key={entry} className="chip" title={t("profile.unlinkApp", { app: entry })} onClick={() => onChange(value.filter((v) => v !== entry))}>
              {icon ? <img src={icon} alt="" width={16} height={16} /> : <Icon name="app-window" />}
              {appLabel(entry)}
              <Icon name="x" />
            </button>
          );
        })}
        <span className="pop-anchor" ref={ref}>
          <button className="chip" aria-expanded={picking} onClick={() => setPicking((p) => !p)}>
            <Icon name="plus" />
            {t("profile.linkApp")}
          </button>
          {picking && (
            <div className="popover" role="menu">
              {offer.length === 0 && <div className="pop-label">{t("profile.noApps")}</div>}
              {offer.some((a) => a.running) && <div className="pop-label">{t("profile.appsOpen")}</div>}
              <div className="menu-list">
                {offer.map((a, i) => (
                  <div key={a.exe} style={{ display: "contents" }}>
                    {!a.running && (i === 0 || offer[i - 1].running) && <div className="pop-label">{t("profile.appsRecent")}</div>}
                    <button role="menuitem" className="menu-row compact" onClick={() => (onChange([...value, a.exe]), setPicking(false))}>
                      <span className="menu-ic is-plain">{a.icon ? <img src={a.icon} alt="" width={20} height={20} /> : <Icon name="app-window" />}</span>
                      <span className="menu-text">
                        <b>{appLabel(a.exe)}</b>
                        {a.title && <small>{a.title}</small>}
                      </span>
                    </button>
                  </div>
                ))}
              </div>
            </div>
          )}
        </span>
        {foreground && !linked.includes(foreground.toLowerCase()) && (
          <button className="chip" onClick={() => onChange([...value, foreground])}>
            <Icon name="plus" />
            {t("profile.addApp", { app: appLabel(foreground).replace(/^./, (c) => c.toUpperCase()) })}
          </button>
        )}
      </div>
    </div>
  );
}

/** `Photoshop.exe` -> `Photoshop`, `chrome.exe: YouTube` -> `chrome: YouTube`. */
const appLabel = (entry: string) => entry.replace(/\.exe\b/i, "");

/** Shown as a tab: as stored, else profiles without apps are and app profiles are not. */
const inBar = (p: ProfileSummary) => p.inBar ?? p.apps.length === 0;

/** Change one stored profile (rename, show in the bar...). */
async function editProfile(id: string, change: (p: Profile) => Profile) {
  const p = await api.profile(id);
  await api.saveProfile(change(p));
}

function deleteProfile(p: ProfileSummary) {
  if (window.confirm(t("profile.confirmDelete", { name: p.name }))) api.deleteProfile(p.id).catch((e) => window.alert(String(e)));
}

/** True while HubDeck's own window has the focus. */
function useWindowFocus() {
  const [focused, setFocused] = useState(() => document.hasFocus());
  useEffect(() => {
    const on = () => setFocused(true);
    const off = () => setFocused(false);
    window.addEventListener("focus", on);
    window.addEventListener("blur", off);
    return () => {
      window.removeEventListener("focus", on);
      window.removeEventListener("blur", off);
    };
  }, []);
  return focused;
}

/** Right-click menu of a profile tab: rename, duplicate, show in or hide from the bar, delete. */
function ProfileMenu({ profile, x, y, count, onClose }: { profile: ProfileSummary; x: number; y: number; count: number; onClose: () => void }) {
  useLanguage();
  const ref = useRef<HTMLDivElement>(null);
  const [name, setName] = useState<string | null>(null);
  useDismiss(true, ref, onClose);
  const left = Math.min(x, window.innerWidth - 240);
  return (
    <div className="popover ctx-menu" role="menu" ref={ref} style={{ position: "fixed", left, top: y + 6 }}>
      <div className="pop-label">{profile.name}</div>
      {name === null ? (
        <div className="menu-list">
          <button role="menuitem" className="menu-row compact" onClick={() => setName(profile.name)}>
            <Icon name="pencil" />
            <span className="menu-text">{t("profiles.rename")}</span>
          </button>
          <button role="menuitem" className="menu-row compact" onClick={() => (api.createProfile(t("profile.copyName", { name: profile.name }), profile.id), onClose())}>
            <Icon name="copy" />
            <span className="menu-text">{t("profile.duplicate")}</span>
          </button>
          <button role="menuitem" className="menu-row compact" onClick={() => (editProfile(profile.id, (p) => ({ ...p, inBar: !inBar(profile) })), onClose())}>
            <Icon name={inBar(profile) ? "eye-off" : "eye"} />
            <span className="menu-text">{inBar(profile) ? t("profiles.hide") : t("profiles.show")}</span>
          </button>
          <button role="menuitem" className="menu-row compact danger" disabled={count <= 1} onClick={() => (onClose(), deleteProfile(profile))}>
            <Icon name="trash" />
            <span className="menu-text">{t("profile.delete")}</span>
          </button>
        </div>
      ) : (
        <form
          className="ctx-rename"
          onSubmit={(e) => {
            e.preventDefault();
            const next = name.trim();
            if (next) editProfile(profile.id, (p) => ({ ...p, name: next }));
            onClose();
          }}
        >
          <input className="input" autoFocus value={name} aria-label={t("profile.name")} onChange={(e) => setName(e.target.value)} />
          <button className="tool small primary" type="submit">
            {t("connections.save")}
          </button>
        </form>
      )}
    </div>
  );
}

/** Every profile: search, switch, rename, duplicate, delete, show in the top bar. */
function ProfilesList({ view, onOpenProfile, onNewProfile }: { view: EngineView; onOpenProfile: () => void; onNewProfile: () => void }) {
  useLanguage();
  const state = view.state;
  const apps = useApps(state?.foreground);
  useLinkedIcons(state?.profiles ?? [], apps);
  const [query, setQuery] = useState("");
  const [renaming, setRenaming] = useState<{ id: string; name: string } | null>(null);
  if (!state) return null;
  const q = query.trim().toLowerCase();
  // drafts live in the assistant until picked; ready profiles have their own group
  const found = state.profiles.filter((p) => !p.draft).filter((p) => !q || p.name.toLowerCase().includes(q) || p.apps.some((a) => a.toLowerCase().includes(q)));
  const list = found.filter((p) => !p.ready);
  const ready = found.filter((p) => p.ready);
  return (
    <>
      <div className="lib-head">
        <div className="search">
          <Icon name="search" />
          <input className="input" placeholder={t("profiles.search")} aria-label={t("profiles.search")} value={query} onChange={(e) => setQuery(e.target.value)} />
        </div>
      </div>
      <div className="lib-groups lib-profiles">
        <div className="plist-head" aria-hidden="true">
          <span>{t("profiles.title")}</span>
          <span>{t("profiles.inBar")}</span>
        </div>
        <ul className="plist">
          {list.map((p) => {
            const icons = p.apps.map((a) => appIcon(apps, a)).filter(Boolean).slice(0, 3) as string[];
            const active = state.active === p.id;
            return (
              <li key={p.id} className={`prow${active ? " is-active" : ""}`}>
                {renaming?.id === p.id ? (
                  <form
                    className="prow-rename"
                    onSubmit={(e) => {
                      e.preventDefault();
                      const name = renaming.name.trim();
                      setRenaming(null);
                      if (name && name !== p.name) editProfile(p.id, (x) => ({ ...x, name }));
                    }}
                  >
                    <input className="input" autoFocus value={renaming.name} aria-label={t("profile.name")} onChange={(e) => setRenaming({ id: p.id, name: e.target.value })} onBlur={() => setRenaming(null)} />
                  </form>
                ) : (
                  <button className="prow-main" onClick={() => (active ? onOpenProfile() : api.activate(p.id))} title={active ? t("topbar.profileSettings") : t("topbar.activate")}>
                    <span className="prow-ic">{icons.length ? icons.map((src) => <img key={src} src={src} alt="" width={16} height={16} />) : <Icon name={state.home === p.id ? "home" : "layers"} />}</span>
                    <span className="prow-text">
                      <b>{p.name}</b>
                      <small>
                        {state.home === p.id && <span className="tag">{t("profiles.home")}</span>}
                        {active && <span className="tag is-acc">{t("profiles.active")}</span>}
                        {state.autoMade?.includes(p.id) && <span className="tag is-acc">{t("auto.new")}</span>}
                        {p.apps.length > 0 && <span className="mono">{p.apps.join(", ")}</span>}
                      </small>
                    </span>
                  </button>
                )}
                <label className="bar-toggle" title={inBar(p) ? t("profiles.hide") : t("profiles.show")}>
                  <input
                    type="checkbox"
                    checked={inBar(p)}
                    aria-label={t("profiles.inBar")}
                    onChange={(e) => {
                      // read now: after editProfile's await React has already put the box back
                      const on = e.target.checked;
                      editProfile(p.id, (x) => ({ ...x, inBar: on }));
                    }}
                  />
                </label>
                <span className="prow-tools">
                  <button className="icon-btn" title={t("profiles.rename")} aria-label={t("profiles.rename")} onClick={() => setRenaming({ id: p.id, name: p.name })}>
                    <Icon name="pencil" />
                  </button>
                  <button className="icon-btn" title={t("profile.duplicate")} aria-label={t("profile.duplicate")} onClick={() => api.createProfile(t("profile.copyName", { name: p.name }), p.id)}>
                    <Icon name="copy" />
                  </button>
                  <button className="icon-btn danger" title={t("profile.delete")} aria-label={t("profile.delete")} disabled={state.profiles.length <= 1} onClick={() => deleteProfile(p)}>
                    <Icon name="trash" />
                  </button>
                </span>
              </li>
            );
          })}
        </ul>
        {list.length === 0 && ready.length === 0 && <p className="help">{t("profiles.none")}</p>}
        {ready.length > 0 && (
          // owner's pick 1a, 2026-10-14: made at once for the apps installed, off until switched on
          <section className="ready-apps" aria-labelledby="ready-title">
            <div className="plist-head">
              <span id="ready-title">{t("profiles.ready", { count: ready.length })}</span>
            </div>
            <p className="help">{t("profiles.readyHelp")}</p>
            <ul className="plist">
              {ready.map((p) => {
                const icon = p.apps.map((a) => appIcon(apps, a)).find(Boolean);
                const active = state.active === p.id;
                return (
                  <li key={p.id} className={`prow${active ? " is-active" : ""}`}>
                    <button className="prow-main" onClick={() => (active ? onOpenProfile() : api.activate(p.id))} title={active ? t("topbar.profileSettings") : t("topbar.activate")}>
                      <span className="prow-ic">{icon ? <img src={icon} alt="" width={16} height={16} /> : <Icon name="app-window" />}</span>
                      <span className="prow-text">
                        <b>{p.name}</b>
                        <small>
                          {active && <span className="tag is-acc">{t("profiles.active")}</span>}
                          <span className="mono">{p.apps.join(", ")}</span>
                        </small>
                      </span>
                    </button>
                    <label className="bar-toggle" title={t("profiles.switchInHelp", { app: p.name })}>
                      <input type="checkbox" checked={p.switchIn !== false} aria-label={t("profiles.switchInFor", { app: p.name })} onChange={(e) => api.profileSwitchIn(p.id, e.target.checked).catch(() => {})} />
                    </label>
                    <span className="prow-tools">
                      <button className="icon-btn danger" title={t("profile.delete")} aria-label={t("profile.delete")} disabled={state.profiles.length <= 1} onClick={() => deleteProfile(p)}>
                        <Icon name="trash" />
                      </button>
                    </span>
                  </li>
                );
              })}
            </ul>
          </section>
        )}
        <button className="tool lib-new" onClick={onNewProfile}>
          <Icon name="plus" />
          {t("topbar.newDeck")}…
        </button>
        <p className="help">{t("profiles.help")}</p>
      </div>
    </>
  );
}

/** "+" > For an app...: pick an app (open now or installed) and get a profile made for it. */
export function ForAppPanel({ view, kinds, onClose, onMade }: { view: EngineView; kinds?: string[]; onClose: () => void; onMade: (profileId: string) => void }) {
  useLanguage();
  const open = useApps(view.state?.foreground);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [ai, setAi] = useState(false);
  const [note, setNote] = useState<string | null>(null);
  const [codeCli, setCodeCli] = useState<boolean | null>(null);
  useEffect(() => {
    invoke<{ codeCli: boolean }>("mcp_info").then((i) => setCodeCli(i.codeCli), () => setCodeCli(false));
  }, []);
  /** "Build with AI": Claude Code builds it with HubDeck's tools; without Claude Code the request is copied. */
  const buildWithAi = async (exe: string, name: string, target: string, picture: string | null) => {
    const prompt = await api.aiPrompt(name, exe, target, picture, getLanguage());
    const before = new Set((await api.state()).profiles.map((p) => p.id));
    try {
      const said = await api.aiBuild(prompt);
      const file = (exe.split(/[\\/]/).pop() ?? exe).toLowerCase();
      const fresh = (await api.state()).profiles.filter((p) => !before.has(p.id));
      const made = fresh.find((p) => p.apps.some((a) => a.split(":")[0].toLowerCase() === file)) ?? fresh[0];
      setBusy(null);
      if (made) {
        await api.activate(made.id);
        onMade(made.id);
      } else {
        setNote(said || t("ai.nothing"));
      }
    } catch (e) {
      setBusy(null);
      if (String(e) !== "no-cli") throw e;
      await navigator.clipboard.writeText(prompt).catch(() => {});
      setNote(t("ai.copied"));
    }
  };
  const make = async (exe: string, name: string, target: string, item?: InstalledItem) => {
    setBusy(name);
    setError(null);
    setNote(null);
    try {
      const picture = item ? await api.installedPicture(item).catch(() => null) : null;
      if (ai) return await buildWithAi(exe, name, target, picture);
      const p = await api.profileForApp(exe, name, target, picture, kinds);
      await api.activate(p.id);
      onMade(p.id);
    } catch (e) {
      setError(String(e));
      setBusy(null);
    }
  };
  const running = open.filter((a) => a.running && !/^hubdeck/i.test(a.exe));
  return (
    <aside className="inspector sectioned side-panel">
      <PanelHead title={t("forApp.title")} crumb={t("topbar.newDeck")} icon="app-window" onClose={onClose} />
      <section className="sec">
        <p className="help">{t("forApp.help")}</p>
        <label className="check">
          <input type="checkbox" checked={ai} disabled={!!busy} onChange={(e) => setAi(e.target.checked)} />
          {t("ai.toggle")}
        </label>
        {ai && <p className="help">{codeCli === false ? t("ai.helpCopy") : t("ai.help")}</p>}
        {busy && (
          <p className="hint">
            <Icon name="sparkles" />
            {ai ? t("ai.making", { app: busy }) : t("forApp.making", { app: busy })}
          </p>
        )}
        {note && (
          <p className="hint">
            <Icon name="info" />
            {note}
          </p>
        )}
        {error && <p className="help">{error}</p>}
      </section>
      {running.length > 0 && (
        <section className="sec">
          <h3 className="sec-title">{t("forApp.open")}</h3>
          <div className="menu-list">
            {running.map((a) => (
              <button key={a.exe} className="menu-row compact" disabled={!!busy} title={a.exe} onClick={() => make(a.exe, a.name ?? appName(a.exe), a.path || a.exe)}>
                <span className="menu-ic is-plain">{a.icon ? <img src={a.icon} alt="" width={20} height={20} /> : <Icon name="app-window" />}</span>
                <span className="menu-text">
                  <b>{a.name ?? appName(a.exe)}</b>
                </span>
              </button>
            ))}
          </div>
        </section>
      )}
      <section className="sec">
        <h3 className="sec-title">{t("forApp.installed")}</h3>
        <InstalledPicker games={false} onPick={(item) => !busy && make(item.exe || item.target, item.name, item.target, item)} />
      </section>
    </aside>
  );
}

/** Profile tabs that fit the top bar; the rest go into a "More" menu (the active one always shows). */
function ProfileTabs({ view, onOpenProfile, onOpenGallery, onOpenForApp, onOpenAssistant, onOpenTour, kinds, newRequest }: { view: EngineView; onOpenProfile: () => void; onOpenGallery: () => void; onOpenForApp: () => void; onOpenAssistant: () => void; onOpenTour: () => void; kinds?: string[]; newRequest?: number }) {
  const state = view.state;
  const all = state?.profiles ?? [];
  // the bar: profiles shown in it, the home profile and the active one
  const profiles = all.filter((p) => inBar(p) || p.id === state?.active || p.id === state?.home);
  const [menu, setMenu] = useState<{ id: string; x: number; y: number } | null>(null);
  const apps = useApps(state?.foreground);
  useLinkedIcons(all, apps);
  const [naming, setNaming] = useState<string | null>(null);
  const [adding, setAdding] = useState(demoParam("popover") === "new");
  const [moreOpen, setMoreOpen] = useState(demoParam("popover") === "more");
  const [readyOpen, setReadyOpen] = useState(false);
  const [fit, setFit] = useState(Infinity);
  const navRef = useRef<HTMLElement>(null);
  const measureRef = useRef<HTMLDivElement>(null);
  const addRef = useRef<HTMLSpanElement>(null);
  const moreRef = useRef<HTMLSpanElement>(null);
  const closeAdd = useCallback(() => setAdding(false), []);
  const closeMore = useCallback(() => setMoreOpen(false), []);
  useDismiss(adding, addRef, closeAdd);
  useDismiss(moreOpen, moreRef, closeMore);
  // "New profile..." in the profiles tab opens this same menu
  useEffect(() => {
    if (newRequest) setAdding(true);
  }, [newRequest]);
  const signature = profiles.map((p) => `${p.name}${state?.autoSwitch && p.apps.length ? "*" : ""}`).join("|");

  // measure every tab once (hidden copies), then keep as many as the bar has room for
  useLayoutEffect(() => {
    const nav = navRef.current;
    const measure = measureRef.current;
    if (!nav || !measure) return;
    const calc = () => {
      // fractional widths rounded up, and a little room: the active tab (its dot, its weight) is wider
      // than its measured copy (the last tab was cut by 4px at 1440; release audit)
      const widths = Array.from(measure.children, (c) => Math.ceil(c.getBoundingClientRect().width) + 2);
      const tabs = widths.slice(0, profiles.length);
      const more = widths[profiles.length] ?? 90;
      const gap = 2;
      // the New deck button sits after the tabs: its own width, not a fixed icon's
      const room = nav.clientWidth - (addRef.current?.getBoundingClientRect().width ?? 36) - 8;
      if (tabs.reduce((s, w) => s + w + gap, 0) <= room) return setFit(profiles.length);
      let used = more + gap;
      let n = 0;
      while (n < tabs.length && used + tabs[n] + gap <= room) used += tabs[n++] + gap;
      setFit(Math.max(1, n));
    };
    calc();
    const ro = new ResizeObserver(calc);
    ro.observe(nav);
    return () => ro.disconnect();
  }, [signature, profiles.length]);

  const activeIndex = profiles.findIndex((p) => p.id === state?.active);
  let shown = profiles.slice(0, fit);
  if (activeIndex >= fit && fit < profiles.length) shown = [...profiles.slice(0, Math.max(0, fit - 1)), profiles[activeIndex]];
  const hidden = profiles.filter((p) => !shown.includes(p));
  const tab = (p: (typeof profiles)[number], measuring = false) => (
    <button
      key={p.id}
      className={`tab${state?.active === p.id ? " is-active" : ""}`}
      tabIndex={measuring ? -1 : undefined}
      onClick={measuring ? undefined : () => (state?.active === p.id ? onOpenProfile() : api.activate(p.id))}
      onContextMenu={
        measuring
          ? undefined
          : (e) => {
              e.preventDefault();
              setMenu({ id: p.id, x: e.clientX, y: e.clientY });
            }
      }
      title={measuring ? undefined : state?.active === p.id ? t("topbar.profileSettings") : p.apps.length ? t("topbar.switchesFor", { apps: p.apps.join(", ") }) : t("topbar.activate")}
    >
      {state?.autoSwitch && p.apps.length > 0 && <span className="auto" aria-hidden="true" />}
      {state?.home === p.id && <Icon name="home" />}
      {/* pinned (3A): the tab of the profile the deck stays on carries the pin */}
      {state?.pinned && state.active === p.id && <Icon name="pin" className="tab-pin" />}
      {p.apps.slice(0, 3).map((a) => {
        const icon = appIcon(apps, a);
        return icon ? <img key={a} src={icon} alt="" width={14} height={14} /> : null;
      })}
      {p.name}
    </button>
  );

  return (
    <>
      <nav className="profiles" aria-label={t("topbar.profiles")} ref={navRef}>
        <div className="tabs-row">{shown.map((p) => tab(p))}</div>
        {hidden.length > 0 && (
          <span className="pop-anchor" ref={moreRef}>
            <button className={`tab more${moreOpen ? " is-open" : ""}`} aria-haspopup="menu" aria-expanded={moreOpen} onClick={() => setMoreOpen((o) => !o)}>
              {t("topbar.more")}
              <span className="count">{hidden.length}</span>
              <Icon name="chevron-down" />
            </button>
            {moreOpen && (
              <div className="popover" role="menu">
                <div className="pop-label">{t("topbar.profiles")}</div>
                <div className="menu-list">
                  {hidden.map((p) => (
                    <button key={p.id} role="menuitem" className="menu-row compact" onClick={() => (api.activate(p.id), setMoreOpen(false))}>
                      <span className="menu-dot">{state?.autoSwitch && p.apps.length > 0 && <span className="auto" />}</span>
                      <span className="menu-text">
                        <b>{p.name}</b>
                        {p.apps.length > 0 && <small className="mono">{p.apps.join(", ")}</small>}
                      </span>
                    </button>
                  ))}
                </div>
              </div>
            )}
          </span>
        )}
        {naming === null ? (
          <span className="pop-anchor" ref={addRef}>
            <button data-guide="newDeck" className={`tab new-deck${adding ? " is-active" : ""}`} aria-expanded={adding} onClick={() => (setAdding((a) => !a), setReadyOpen(false))}>
              <Icon name="plus" />
              {t("topbar.newDeck")}
            </button>
            {adding && (
              <div className="popover new-deck-pop" role="dialog" aria-label={t("topbar.newDeck")}>
                {/* the one way into a new deck (owner's pick after the release audit, 2026-10-13) */}
                <div className="pop-title">{t("topbar.newDeck")}</div>
                <button
                  className="menu-row is-primary"
                  onClick={() => {
                    setAdding(false);
                    onOpenTour();
                  }}
                >
                  <span className="menu-ic">
                    <Icon name="play" />
                  </span>
                  <span className="menu-text">
                    <b>{t("newDeck.tour")}</b>
                    <small>{t("newDeck.tourHelp")}</small>
                  </span>
                </button>
                <button
                  className="menu-row"
                  onClick={() => {
                    setAdding(false);
                    onOpenForApp();
                  }}
                >
                  <span className="menu-ic is-plain">
                    <Icon name="app-window" />
                  </span>
                  <span className="menu-text">
                    <b>{t("forApp.menu")}</b>
                    <small>{t("forApp.menuHelp")}</small>
                  </span>
                </button>
                <button className="menu-row" aria-expanded={readyOpen} onClick={() => setReadyOpen((o) => !o)}>
                  <span className="menu-ic is-plain">
                    <Icon name="layout-grid" />
                  </span>
                  <span className="menu-text">
                    <b>{t("newDeck.ready")}</b>
                    <small>{t("newDeck.readyHelp")}</small>
                  </span>
                  <Icon name={readyOpen ? "chevron-up" : "chevron-down"} />
                </button>
                {readyOpen && <PresetList kinds={kinds} onPicked={() => setAdding(false)} />}
                <button
                  className="menu-row"
                  onClick={() => {
                    setAdding(false);
                    onOpenAssistant();
                  }}
                >
                  <span className="menu-ic is-plain">
                    <Icon name="sparkles" />
                  </span>
                  <span className="menu-text">
                    <b>{t("assistant.menu")}</b>
                    <small>{t("assistant.menuHelp")}</small>
                  </span>
                </button>
                <button
                  className="menu-row"
                  onClick={() => {
                    setAdding(false);
                    setNaming("");
                  }}
                >
                  <span className="menu-ic is-plain">
                    <Icon name="file-plus" />
                  </span>
                  <span className="menu-text">
                    <b>{t("newDeck.empty")}</b>
                  </span>
                </button>
                {GALLERY_ONLINE && (
                  <button
                    className="menu-row"
                    onClick={() => {
                      setAdding(false);
                      onOpenGallery();
                    }}
                  >
                    <span className="menu-ic is-plain">
                      <Icon name="globe" />
                    </span>
                    <span className="menu-text">
                      <b>{t("gallery.browse")}</b>
                      <small>{t("gallery.browseHelp")}</small>
                    </span>
                  </button>
                )}
              </div>
            )}
          </span>
        ) : (
          <form
            onSubmit={(e) => {
              e.preventDefault();
              const name = naming.trim();
              setNaming(null);
              if (name) api.createProfile(name).then((p) => api.activate(p.id));
            }}
          >
            <input className="input tab-input" autoFocus placeholder={t("topbar.profileName")} value={naming} onChange={(e) => setNaming(e.target.value)} onBlur={() => setNaming(null)} />
          </form>
        )}
      </nav>
      {menu &&
        (() => {
          const p = all.find((x) => x.id === menu.id);
          return p ? <ProfileMenu profile={p} x={menu.x} y={menu.y} count={all.length} onClose={() => setMenu(null)} /> : null;
        })()}
      <div className="tabs-measure" ref={measureRef} aria-hidden="true">
        {profiles.map((p) => tab(p, true))}
        <button className="tab more" tabIndex={-1}>
          {t("topbar.more")}
          <span className="count">{hidden.length || 9}</span>
          <Icon name="chevron-down" />
        </button>
      </div>
    </>
  );
}

/** `Photoshop.exe` -> `Photoshop`. */
const appName = (exe: string) => exe.replace(/\.exe$/i, "");

/** Shown only while an app's own profile took over the deck: that app's icon, the why in its tooltip. */
function TakeoverMark({ app, focused }: { app: string; focused: boolean }) {
  const apps = useApps(app);
  const icon = appIcon(apps, app);
  const name = appName(app);
  const help = `${t("topbar.autoHelp", { app: name })}${focused ? ` ${t("topbar.lastAppHelp")}` : ""}`;
  return (
    <span className="fg" title={help} aria-label={help}>
      {icon ? <img src={icon} alt="" width={16} height={16} /> : <Icon name="app-window" />}
    </span>
  );
}

export function TopBar({
  live,
  view,
  onOpenProfile,
  onOpenSettings,
  onOpenGames,
  onOpenGallery,
  onOpenForApp,
  kinds,
  newRequest,
  onOpenAssistant,
  onOpenTour,
}: {
  kinds?: string[];
  newRequest?: number;
  onOpenAssistant: () => void;
  onOpenTour: () => void;
  onOpenForApp: () => void;
  live: LiveState;
  view: EngineView;
  onOpenGallery: () => void;
  onOpenProfile: () => void;
  onOpenSettings: () => void;
  onOpenGames: () => void;
}) {
  useLanguage();
  const count = live.deck.modules.filter((m) => m.kind !== "base").length;
  const orientation = live.deck.orientation;
  const state = view.state;
  const focused = useWindowFocus();
  return (
    <header className="topbar">
      <div className="brand">
        <Logo />
        HubDeck
      </div>
      <ProfileTabs view={view} onOpenProfile={onOpenProfile} onOpenGallery={onOpenGallery} onOpenForApp={onOpenForApp} onOpenAssistant={onOpenAssistant} onOpenTour={onOpenTour} kinds={kinds} newRequest={newRequest} />
      {state?.autoApp && <TakeoverMark app={state.autoApp} focused={focused} />}
      {state && (
        <label className="check auto-switch" title={state.pinned ? t("topbar.autoOffHelp", { profile: state.profiles.find((p) => p.id === state.active)?.name ?? "" }) : t("topbar.autoOnHelp")}>
          <input type="checkbox" checked={!state.pinned} onChange={(e) => api.pin(!e.target.checked)} />
          {t("topbar.autoSwitch")}
        </label>
      )}
      <span className="sep" />
      {GAMES_ENABLED && (
        <button className="tool ghost" onClick={onOpenGames}>
          {t("topbar.games")}
        </button>
      )}
      <HelpMenu />
      <button className="tool ghost" onClick={onOpenSettings}>
        <Icon name="settings" />
        {t("topbar.settings")}
      </button>
      <span className="sep" />
      <div className="status" title={orientation ? t("topbar.baseLies", { orientation: t(`orientation.${orientation}`) }) : undefined}>
        <span className={`dot${count > 0 ? " is-on" : ""}`} />
        {t("topbar.modules", { count })}
      </div>
    </header>
  );
}

/** Help and feedback in the top bar (owner's pick 1a, 2026-10-16: "Report a problem" was only in
 * Settings): a bug report with the short diagnostics filled in (his 2a), a feature idea, a
 * question. Its button says Feedback, the owner's pick of three (C, 2026-10-16: people should report). */
function HelpMenu() {
  useLanguage();
  const version = useAppVersion();
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLSpanElement>(null);
  const close = useCallback(() => setOpen(false), []);
  useDismiss(open, ref, close);
  const go = (url: string) => {
    setOpen(false);
    api.openLink(url).catch(() => {});
  };
  const report = () => {
    const versionField = `&version=${encodeURIComponent(`HubDeck ${version ?? "?"}, ${osName()}`)}`;
    share
      .diagnosticsBrief()
      .catch(() => "")
      .then((brief) => go(`${REPO}/issues/new?template=bug.yml${versionField}${brief ? `&diagnostics=${encodeURIComponent(brief)}` : ""}`));
  };
  return (
    <span className="pop-anchor help-menu" ref={ref}>
      <button className="tool ghost" aria-haspopup="menu" aria-expanded={open} title={t("help.title")} onClick={() => setOpen((o) => !o)}>
        <Icon name="message-square" />
        {t("help.button")}
      </button>
      {open && (
        <div className="popover help-pop" role="menu">
          <div className="pop-label">{t("help.title")}</div>
          <div className="menu-list">
            <button role="menuitem" className="menu-row compact" onClick={report}>
              <Icon name="alert" />
              <span className="menu-text">
                <b>{t("help.report")}</b>
                <small>{t("help.reportHelp")}</small>
              </span>
            </button>
            <button role="menuitem" className="menu-row compact" onClick={() => go(`${REPO}/issues/new?template=feature.yml`)}>
              <Icon name="sparkles" />
              <span className="menu-text">
                <b>{t("help.suggest")}</b>
              </span>
            </button>
            <button role="menuitem" className="menu-row compact" onClick={() => go(`${REPO}/discussions`)}>
              <Icon name="message-square" />
              <span className="menu-text">
                <b>{t("help.ask")}</b>
              </span>
            </button>
          </div>
          <p className="help">{t("help.menuText")}</p>
        </div>
      )}
    </span>
  );
}

/** The hardware light effect and its speed (base edge, knob and fader LEDs). */
export function EffectFields({ idPrefix, effect, speed, onChange }: { idPrefix: string; effect?: string; speed?: number; onChange: (effect?: string, speed?: number) => void }) {
  useLanguage();
  const current = effect ?? "static";
  const value = speed ?? 50;
  return (
    <div className="effect-fields">
      <div className="field">
        <label htmlFor={`${idPrefix}-effect`} className="label">
          {t("look.effect")}
        </label>
        <select id={`${idPrefix}-effect`} className="input" value={current} onChange={(e) => onChange(e.target.value === "static" ? undefined : e.target.value, e.target.value === "static" ? undefined : speed)}>
          {LED_EFFECTS.map((fx) => (
            <option key={fx} value={fx}>
              {t(`effect.${fx}`)}
            </option>
          ))}
        </select>
      </div>
      {current !== "static" && (
        <div className="field">
          <div className="row between">
            <label htmlFor={`${idPrefix}-speed`} className="label">
              {t("look.speed")}
            </label>
            <span className="mono value">{value}</span>
          </div>
          <input
            id={`${idPrefix}-speed`}
            className="range"
            type="range"
            min={0}
            max={100}
            step={5}
            value={value}
            style={{ "--fill": `${value}%` } as React.CSSProperties}
            onChange={(e) => onChange(effect, Number(e.target.value))}
          />
        </div>
      )}
    </div>
  );
}

/** Title row of a side panel: icon, small caption, title, close. */
/** A panel over the window (For an app, Gallery, Games, a deck's settings): the editor on the right stays. Esc or a click outside closes it. */
export function Sheet({ onClose, children }: { onClose: () => void; children: React.ReactNode }) {
  const close = useRef(onClose);
  close.current = onClose;
  useEffect(() => {
    const on = (e: KeyboardEvent) => {
      // a popover inside closes first (its own Escape); the sheet only when nothing else is open
      if (e.key === "Escape" && !document.querySelector(".sheet .popover")) close.current();
    };
    window.addEventListener("keydown", on);
    return () => window.removeEventListener("keydown", on);
  }, []);
  return (
    <div className="sheet-backdrop" onMouseDown={(e) => e.target === e.currentTarget && close.current()}>
      <div className="sheet" role="dialog" aria-modal="true">
        {children}
      </div>
    </div>
  );
}

function PanelHead({ title, crumb, icon, onClose }: { title: string; crumb?: string; icon?: string; onClose?: () => void }) {
  return (
    <div className="insp-head panel-head">
      {icon && (
        <span className="ph-icon">
          <Icon name={icon} />
        </span>
      )}
      <div className="grow">
        {crumb && <div className="crumb">{crumb}</div>}
        <h2>{title}</h2>
      </div>
      {onClose && (
        <button className="icon-btn" onClick={onClose} aria-label={t("profile.close")} title={t("profile.close")}>
          <Icon name="x" />
        </button>
      )}
    </div>
  );
}

/** Name, apps that switch to it, auto-switch, export / import / duplicate / delete. */
/**
 * "Edge" under the drawing: this profile's lights (its color, its own edge) and the way to every
 * profile's lights in Settings, so lights live in two places, not three (owner's pick 3a,
 * 2026-10-16; docs/FLOWS.md difference 6).
 */
export function EdgePanel({ view, onClose, onAll }: { view: EngineView; onClose: () => void; onAll: () => void }) {
  useLanguage();
  const profile = view.profile;
  const [draft, setDraft] = useState<Profile | null>(profile);
  const [message, setMessage] = useState<string | null>(null);
  // the style every profile follows, named in the first choice (owner's pick 2a, 2026-10-16: a
  // profile color that did nothing under Rainbow was confusing; one choice that works at once)
  const [style, setStyle] = useState<string | null>(null);
  useEffect(() => setDraft(profile), [profile?.id]);
  useEffect(() => {
    api.lights().then((l) => setStyle(l.style), () => setStyle(null));
  }, []);
  if (!profile || !draft) return null;
  const commit = (next: Profile) => {
    setDraft(next);
    api.saveProfile(next).catch((e) => setMessage(String(e)));
  };
  const mode = !draft.edge ? "follow" : draft.edgeEffect ? "effect" : "color";
  const styleName = style === "rainbow" ? t("lights.rainbow") : style === "color" ? t("lights.oneColor") : style === "profile" ? t("lights.profileColor") : style === "off" ? t("lights.off") : "";
  const pick = (next: "follow" | "color" | "effect") => {
    if (next === "follow") commit({ ...draft, edge: undefined, edgeEffect: undefined, edgeSpeed: undefined });
    else if (next === "color") commit({ ...draft, edge: draft.edge ?? draft.color ?? "#8b7cf6", edgeEffect: undefined, edgeSpeed: undefined });
    else commit({ ...draft, edge: draft.edge ?? draft.color ?? "#8b7cf6", edgeEffect: draft.edgeEffect ?? "breathing", edgeSpeed: draft.edgeSpeed ?? 50 });
  };
  return (
    <aside className="inspector sectioned side-panel edge-panel">
      <PanelHead title={t("edge.title")} crumb={draft.name} icon="sun" onClose={onClose} />
      <section className="sec">
        <h3 className="sec-title">{t("profile.edgeTitle")}</h3>
        <div className="seg full edge-modes" role="radiogroup" aria-label={t("profile.edgeTitle")}>
          {(["follow", "color", "effect"] as const).map((m) => (
            <button key={m} role="radio" aria-checked={mode === m} className={`tab${mode === m ? " is-active" : ""}`} onClick={() => pick(m)}>
              {t(`edge.mode.${m}` as Key)}
            </button>
          ))}
        </div>
        {mode === "follow" && (
          <>
            <p className="help">{styleName ? t("edge.followStyle", { style: styleName }) : t("edge.followHelp")}</p>
            {style === "profile" && (
              <div className="row between">
                <span className="label">{t("profile.color")}</span>
                <label className="color">
                  <input type="color" value={draft.color ?? "#8b7cf6"} aria-label={t("profile.color")} onChange={(e) => setDraft({ ...draft, color: e.target.value })} onBlur={() => commit(draft)} />
                </label>
              </div>
            )}
          </>
        )}
        {mode !== "follow" && draft.edge && (
          <div className="row between">
            <span className="label">{t("look.color")}</span>
            <label className="color">
              <input type="color" value={draft.edge} aria-label={t("look.color")} onChange={(e) => setDraft({ ...draft, edge: e.target.value })} onBlur={() => commit(draft)} />
              <span className="mono">{draft.edge.toUpperCase()}</span>
            </label>
          </div>
        )}
        {mode === "effect" && <EffectFields idPrefix="edge" effect={draft.edgeEffect} speed={draft.edgeSpeed} onChange={(edgeEffect, edgeSpeed) => commit({ ...draft, edgeEffect: edgeEffect ?? "breathing", edgeSpeed })} />}
      </section>
      <section className="sec end">
        <button className="tool align-start" onClick={onAll}>
          <Icon name="sun" />
          {t("edge.all")}
        </button>
        <p className="help">{t("edge.allHelp")}</p>
        {message && <p className="help">{message}</p>}
      </section>
    </aside>
  );
}

export function ProfilePanel({ view, onClose }: { view: EngineView; onClose: () => void }) {
  useLanguage();
  const profile = view.profile;
  const state = view.state;
  const [draft, setDraft] = useState<Profile | null>(profile);
  const [apps, setApps] = useState("");
  const known = useApps(view.state?.foreground);
  const [issues, setIssues] = useState<{ control: string; message: string }[]>([]);
  useEffect(() => {
    if (profile?.id) api.checkProfile(profile.id).then(setIssues, () => setIssues([]));
  }, [profile?.id, profile]);
  const [message, setMessage] = useState<string | null>(null);
  useEffect(() => {
    setDraft(profile);
    setApps(profile?.apps.join(", ") ?? "");
  }, [profile?.id]);
  if (!profile || !draft || !state) return null;
  const commit = (next: Profile) => {
    setDraft(next);
    api.saveProfile(next).catch((e) => setMessage(String(e)));
  };
  const isFallback = state.fallback === profile.id;
  return (
    <aside className="inspector sectioned side-panel">
      <PanelHead title={draft.name || profile.name} crumb={t("topbar.profileSettings")} icon="layers" onClose={onClose} />
      <section className="sec">
        <div className="field">
          <label htmlFor="profile-name" className="label">
            {t("profile.name")}
          </label>
          <input id="profile-name" className="input" value={draft.name} onChange={(e) => setDraft({ ...draft, name: e.target.value })} onBlur={() => draft.name.trim() && commit(draft)} />
        </div>
        <AppLinks
          apps={known}
          value={draft.apps}
          foreground={state.foreground}
          onChange={(next) => {
            setApps(next.join(", "));
            commit({ ...draft, apps: next });
          }}
        />
        <details className="field">
          <summary className="label">{t("profile.appsAdvanced")}</summary>
          <input
            id="profile-apps"
            className="input mono"
            placeholder="obs64.exe, chrome.exe: YouTube"
            value={apps}
            onChange={(e) => setApps(e.target.value)}
            onBlur={() => commit({ ...draft, apps: apps.split(",").map((a) => a.trim()).filter(Boolean) })}
          />
          <p className="help">{t("profile.appsHelp")}</p>
        </details>
      </section>
      <section className="sec">
        <h3 className="sec-title">{t("profile.switching")}</h3>
        {/* only what belongs to this profile; the switches for every profile are in Settings > Profiles */}
        <label className="check">
          <input type="checkbox" checked={isFallback} disabled={isFallback} onChange={() => api.settings({ fallback: profile.id })} />
          {t("profile.fallback")}
        </label>
        <p className="help">{t("profile.switchingWhere")}</p>
      </section>
      <section className="sec">
        <h3 className="sec-title">{t("profile.panel")}</h3>
        <select
          id="panel-content"
          aria-label={t("profile.panel")}
          className="input"
          value={draft.panel?.kind ?? "keys"}
          onChange={async (e) => {
            const kind = e.target.value;
            if (kind === "keys") return commit({ ...draft, panel: undefined });
            if (kind === "clock") return commit({ ...draft, panel: { kind: "clock" } });
            if (kind === "graph") return commit({ ...draft, panel: { kind: "graph", metric: "cpu" } });
            const image = await pickImage().catch((err) => (setMessage(String(err)), null));
            if (image) commit({ ...draft, panel: { kind: "picture", image } });
          }}
        >
          <option value="keys">{t("profile.panelKeys")}</option>
          <option value="picture">{t("profile.panelPicture")}</option>
          <option value="clock">{t("profile.panelClock")}</option>
          <option value="graph">{t("profile.panelGraph")}</option>
        </select>
        {draft.panel?.kind === "graph" && (
          <select className="input" aria-label={t("profile.panelGraph")} value={draft.panel.metric} onChange={(e) => commit({ ...draft, panel: { kind: "graph", metric: e.target.value as Metric } })}>
            {METRICS.map((m) => (
              <option key={m} value={m}>
                {t(`metric.${m}`)}
              </option>
            ))}
          </select>
        )}
        {draft.panel?.kind === "picture" && (
          <button className="tool align-start" onClick={() => pickImage().then((image) => image && commit({ ...draft, panel: { kind: "picture", image } }), (err) => setMessage(String(err)))}>
            <Icon name="image" />
            {t("look.changePicture")}
          </button>
        )}
        <p className="help">{t("profile.panelHelp")}</p>
      </section>
      {issues.length > 0 && (
        <section className="sec">
          <h3 className="sec-title">{t("profile.checkTitle")}</h3>
          {issues.map((i) => (
            <p key={`${i.control}${i.message}`} className="help">
              <span aria-hidden="true" style={{ display: "inline-block", width: 8, height: 8, borderRadius: 4, background: "#f5c518", marginRight: 6 }} />
              <b>{controlLabel(i.control, draft.pages)}</b> {localizeError(i.message)}
            </p>
          ))}
          <p className="help">{t("profile.checkHelp")}</p>
        </section>
      )}
      <section className="sec">
        <h3 className="sec-title">{t("profile.manage")}</h3>
        <div className="btn-grid">
          <button className="tool" title={t("suggest.fillHelp")} onClick={() => api.profileFill(profile.id, view.state?.page ?? null)}>
            <Icon name="sparkles" />
            {t("suggest.fill")}
          </button>
          <button className="tool" onClick={() => api.createProfile(t("profile.copyName", { name: profile.name }), profile.id).then((p) => api.activate(p.id))}>
            <Icon name="copy" />
            {t("profile.duplicate")}
          </button>
          <button
            className="tool"
            onClick={async () => {
              const path = await save({
                defaultPath: `${profile.name}.hubdeck`,
                filters: [
                  { name: t("profile.fileTypePackage"), extensions: ["hubdeck"] },
                  { name: t("profile.fileTypeJson"), extensions: ["json"] },
                ],
              });
              if (path) api.exportProfile(profile.id, path).then(() => setMessage(t("profile.savedTo", { path })), (e) => setMessage(String(e)));
            }}
          >
            <Icon name="download" />
            {t("profile.export")}
          </button>
          <button
            className="tool"
            onClick={async () => {
              const path = await open({ multiple: false, filters: [{ name: t("profile.fileType"), extensions: ["hubdeck", "json", "mhz"] }] });
              if (typeof path === "string")
                api.importProfile(path).then(
                  async (p) => (await keepImported(p)) && (setMessage(movedNote(p.moved, p.pages) || null), api.activate(p.id)),
                  (e) => setMessage(String(e)),
                );
            }}
          >
            <Icon name="upload" />
            {t("profile.import")}
          </button>
          <button
            className="tool"
            title={t("profile.adaptHelp")}
            onClick={() =>
              api.adaptProfile(profile.id).then(
                (moved) => {
                  setMessage(movedNote(moved, profile.pages) || t("profile.adaptNothing"));
                  // the panel keeps a draft of the whole profile: take the adapted one
                  if (moved.length) api.profile(profile.id).then(setDraft, () => {});
                },
                (e) => setMessage(String(e)),
              )
            }
          >
            <Icon name="layout-grid" />
            {t("profile.adapt")}
          </button>
          <button
            className="tool danger"
            disabled={state.profiles.length <= 1}
            onClick={() => {
              if (window.confirm(t("profile.confirmDelete", { name: profile.name }))) api.deleteProfile(profile.id).catch((e) => setMessage(String(e)));
            }}
          >
            <Icon name="trash" />
            {t("profile.delete")}
          </button>
        </div>
        {message && <p className="help">{message}</p>}
      </section>
    </aside>
  );
}

/** Settings of the whole app: language, look, screens, profiles folder, backups, connections, updates, help, about. */
export type SettingsSection = "general" | "lights" | "screens" | "connections" | "plugins" | "backups" | "updates" | "help";

const SETTINGS_SECTIONS: { id: SettingsSection; icon: string; label: Key }[] = [
  { id: "general", icon: "settings", label: "settings.general" },
  // lights and screens in one section (owner's pick 5a); "screens" still opens it
  { id: "lights", icon: "sun", label: "settings.lightsScreens" },
  { id: "connections", icon: "link", label: "settings.connections" },
  { id: "plugins", icon: "plug", label: "topbar.plugins" },
  { id: "backups", icon: "archive", label: "settings.backups" },
  { id: "updates", icon: "download", label: "updates.title" },
  { id: "help", icon: "info", label: "settings.help" },
];

/** Settings as a page of its own over the whole window (owner, 2026-10-13: a panel in the right
 *  column covered the key editor): sections on the left, one section's content on the right. */
export function SettingsPanel({ live, view, section, onSection, onClose, onCalibrate, onTour, onBuild }: { live: LiveState; view: EngineView; section: SettingsSection; onSection: (s: SettingsSection) => void; onClose: () => void; onCalibrate: () => void; onTour?: () => void; onBuild?: () => void }) {
  const { lang, setLanguage } = useLanguage();
  return (
    <div className="settings-page">
      <header className="settings-head">
        <button className="tool ghost" onClick={onClose}>
          <Icon name="chevron-left" />
          {t("settings.back")}
        </button>
        <h2>{t("settings.title")}</h2>
      </header>
      <div className="settings-body">
        <nav className="settings-nav" aria-label={t("settings.title")}>
          {SETTINGS_SECTIONS.map((x) => (
            <button key={x.id} className={`settings-nav-item${x.id === section || (x.id === "lights" && section === "screens") ? " is-active" : ""}`} aria-current={x.id === section ? "page" : undefined} onClick={() => onSection(x.id)}>
              <Icon name={x.icon} />
              {t(x.label)}
            </button>
          ))}
        </nav>
        <div className="settings-content sectioned" key={section}>
          {section === "general" && (
            <>
              <section className="sec">
                <h3 className="sec-title">{t("topbar.language")}</h3>
                <select id="language" aria-label={t("topbar.language")} className="input" value={lang} onChange={(e) => setLanguage(e.target.value as Lang)}>
                  <option value="en">English</option>
                  <option value="pl">Polski</option>
                </select>
              </section>
              <Appearance view={view} />
              <SwitchingSettings view={view} />
              <ProfilesFolder />
            </>
          )}
          {(section === "lights" || section === "screens") && (
            <>
              <LightsSettings />
              <ScreensSettings />
              {live.deck.modules.some((m) => m.kind === "keys") && (
                <section className="sec">
                  <h3 className="sec-title">{t("settings.advanced")}</h3>
                  <p className="help">{t("settings.calibrateHelp")}</p>
                  <button className="tool align-start" onClick={onCalibrate}>
                    <Icon name="crosshair" />
                    {t("topbar.calibrate")}
                  </button>
                </section>
              )}
            </>
          )}
          {section === "connections" && <Connections />}
          {section === "plugins" && <PluginsList />}
          {section === "backups" && <Backups />}
          {section === "updates" && <UpdatesSettings />}
          {section === "help" && (
            <>
              {(onTour || onBuild) && (
                <section className="sec">
                  <h3 className="sec-title">{t("help.startTitle")}</h3>
                  <p className="help">{t("help.startHelp")}</p>
                  <div className="btn-grid">
                    {onTour && (
                      <button className="tool" onClick={onTour}>
                        <Icon name="play" />
                        {t("assistant.showMe")}
                      </button>
                    )}
                    {onBuild && (
                      <button className="tool" onClick={onBuild}>
                        <Icon name="layers" />
                        {t("assistant.buildYourself")}
                      </button>
                    )}
                  </div>
                </section>
              )}
              <HelpFeedback />
              <About />
            </>
          )}
        </div>
      </div>
    </div>
  );
}

/** How the deck switches profiles, for every profile. */
function SwitchingSettings({ view }: { view: EngineView }) {
  useLanguage();
  const state = view.state;
  if (!state) return null;
  return (
    <section className="sec">
      <h3 className="sec-title">{t("settings.profiles")}</h3>
      <label className="check">
        <input type="checkbox" checked={state.autoSwitch} onChange={(e) => api.settings({ autoSwitch: e.target.checked })} />
        {t("switching.main")}
      </label>
      {/* one switch on top, the rest folded (owner's pick 10a) */}
      <details className="more-opts">
        <summary>{t("switching.more")}</summary>
        <label className="check">
          <input type="checkbox" checked={state.autoProfiles ?? true} onChange={(e) => api.autoSet(e.target.checked)} />
          {t("auto.setting")}
        </label>
        <label className="check" title={t("call.settingHelp")}>
          <input type="checkbox" checked={state.callSwitch ?? true} onChange={(e) => api.callSwitchSet(e.target.checked)} />
          {t("call.setting")}
        </label>
        <p className="help">{t("profile.homeHelp")}</p>
      </details>
      <p className="help">{t("switching.example")}</p>
    </section>
  );
}

/** System, dark or light. */
function LightsSettings() {
  useLanguage();
  const [l, setL] = useState<LightSettings | null>(null);
  useEffect(() => {
    api.lights().then(setL, () => {});
  }, []);
  if (!l) return null;
  const save = (next: LightSettings) => {
    setL(next);
    api.setLights(next).catch(() => {});
  };
  const reactions: [keyof LightSettings["react"] & ("mic" | "live" | "agents" | "timer" | "call"), Key][] = [
    ["call", "lights.reactCall"],
    ["mic", "lights.reactMic"],
    ["live", "lights.reactLive"],
    ["agents", "lights.reactAgents"],
    ["timer", "lights.reactTimer"],
  ];
  return (
    <section className="sec">
      <h3 className="sec-title">{t("lights.title")}</h3>
      <div className="row wrap">
        <select className="input" aria-label={t("lights.style")} value={l.style} onChange={(e) => save({ ...l, style: e.target.value })}>
          <option value="rainbow">{t("lights.rainbow")}</option>
          <option value="color">{t("lights.oneColor")}</option>
          <option value="profile">{t("lights.profileColor")}</option>
          <option value="off">{t("lights.off")}</option>
        </select>
        {l.style === "color" && (
          <label className="color">
            <input type="color" value={l.color} aria-label={t("look.color")} onChange={(e) => setL({ ...l, color: e.target.value })} onBlur={() => save(l)} />
          </label>
        )}
      </div>
      <label htmlFor="lights-brightness" className="label">
        {t("lights.brightness")}
      </label>
      <input
        id="lights-brightness"
        className="range"
        type="range"
        min={0}
        max={100}
        step={5}
        value={l.brightness}
        style={{ "--fill": `${l.brightness}%` } as React.CSSProperties}
        onChange={(e) => setL({ ...l, brightness: Number(e.target.value) })}
        onPointerUp={() => save(l)}
        onKeyUp={() => save(l)}
      />
      {l.style === "rainbow" && (
        <label className="check" title={t("lights.hubRainbowHelp")}>
          <input type="checkbox" checked={l.hubRainbow ?? true} onChange={(e) => save({ ...l, hubRainbow: e.target.checked })} />
          {t("lights.hubRainbow")}
        </label>
      )}
      <div className="label">{t("lights.reactTo")}</div>
      {reactions.map(([key, label]) => (
        <div className="row between" key={key}>
          <label className="check">
            <input type="checkbox" checked={l.react[key].on} onChange={(e) => save({ ...l, react: { ...l.react, [key]: { ...l.react[key], on: e.target.checked } } })} />
            {t(label)}
          </label>
          <label className="color">
            <input
              type="color"
              value={l.react[key].color}
              aria-label={t("look.color")}
              onChange={(e) => setL({ ...l, react: { ...l.react, [key]: { ...l.react[key], color: e.target.value } } })}
              onBlur={() => save(l)}
            />
          </label>
        </div>
      ))}
      <label className="check">
        <input type="checkbox" checked={l.react.levels} onChange={(e) => save({ ...l, react: { ...l.react, levels: e.target.checked } })} />
        {t("lights.reactLevels")}
      </label>
      <p className="help">{t("lights.help")}</p>
    </section>
  );
}

function Appearance({ view }: { view: EngineView }) {
  useLanguage();
  const theme = useTheme();
  const options: [Theme, Key][] = [
    ["system", "theme.system"],
    ["dark", "theme.dark"],
    ["light", "theme.light"],
  ];
  return (
    <section className="sec">
      <h3 className="sec-title">{t("settings.appearance")}</h3>
      <div className="seg full" role="radiogroup" aria-label={t("settings.appearance")}>
        {options.map(([value, label]) => (
          <button key={value} role="radio" aria-checked={theme === value} className={`tab${theme === value ? " is-active" : ""}`} onClick={() => setTheme(value)}>
            {t(label)}
          </button>
        ))}
      </div>
      <p className="help">{t("theme.help")}</p>
      {view.state && (
        <>
          <label className="check">
            <input type="checkbox" checked={view.state.reducedMotion === true} onChange={(e) => api.reducedMotionSet(e.target.checked)} />
            {t("settings.reducedMotion")}
          </label>
          <p className="help">{t("settings.reducedMotionHelp")}</p>
        </>
      )}
    </section>
  );
}

/** What a found update says, with the one button that installs it. */
function UpdateCard({ info }: { info: UpdateInfo }) {
  useLanguage();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  return (
    <div className="update-card">
      <div className="row">
        <span className="ph-icon small">
          <Icon name="sparkles" />
        </span>
        <div className="grow">
          <b>{t("updates.available", { version: info.version })}</b>
          {info.date && <div className="help mono">{info.date.slice(0, 10)}</div>}
        </div>
      </div>
      {info.notes && <div className="notes">{info.notes}</div>}
      <button
        className="tool primary"
        disabled={busy}
        onClick={() => {
          setBusy(true);
          setError(null);
          updates.install().catch((e) => (setError(String(e)), setBusy(false)));
        }}
      >
        <Icon name="download" />
        {busy ? t("updates.installing") : t("updates.install")}
      </button>
      {error && <p className="help">{error}</p>}
    </div>
  );
}

/** Look for a new version at start or now; install only on request. */
function UpdatesSettings() {
  useLanguage();
  const [cfg, setCfg] = useState<[boolean, boolean] | null>(null);
  const [result, setResult] = useState<"checking" | "latest" | UpdateInfo | { error: string } | null>(null);
  useEffect(() => {
    updates.settings().then(setCfg, () => setCfg([false, false]));
  }, []);
  if (!cfg) return null;
  const [enabled, atStart] = cfg;
  // a build without updates: say so, no switch and button that cannot work
  if (!enabled)
    return (
      <section className="sec">
        <h3 className="sec-title">{t("updates.title")}</h3>
        <p className="help">{t("updates.notSetUp")}</p>
      </section>
    );
  return (
    <section className="sec">
      <h3 className="sec-title">{t("updates.title")}</h3>
      <label className="check">
        <input
          type="checkbox"
          checked={atStart}
          disabled={!enabled}
          onChange={(e) => {
            setCfg([enabled, e.target.checked]);
            updates.setCheckAtStart(e.target.checked).catch(() => {});
          }}
        />
        {t("updates.atStart")}
      </label>
      <div className="row wrap">
        <button
          className="tool"
          disabled={!enabled || result === "checking"}
          onClick={() => {
            setResult("checking");
            updates.check().then(
              (info) => setResult(info ?? "latest"),
              (e) => setResult({ error: String(e) }),
            );
          }}
        >
          <Icon name="refresh-cw" />
          {result === "checking" ? t("updates.checking") : t("updates.checkNow")}
        </button>
        {result === "latest" && (
          <span className="pill is-ok">{t("updates.latest")}</span>
        )}
      </div>
      {result && typeof result === "object" && "version" in result && <UpdateCard info={result} />}
      {result && typeof result === "object" && "error" in result && <p className="help">{result.error}</p>}
      <p className="help">{enabled ? t("updates.help") : t("updates.notSetUp")}</p>
      <GoBack />
    </section>
  );
}

/** After an update that broke something: back to the version before, signed like any update,
 *  the profiles backed up first (owner's pick, 2026-10-16; PAIN-POINTS RD1). */
function GoBack() {
  useLanguage();
  const [previous, setPrevious] = useState<UpdateInfo | null>(null);
  const [state, setState] = useState<"idle" | "going" | { error: string }>("idle");
  useEffect(() => {
    updates.previous().then(setPrevious, () => setPrevious(null));
  }, []);
  if (!previous) return null;
  return (
    <div className="field go-back">
      <span className="label">{t("updates.backTitle")}</span>
      <p className="help">{t("updates.backHelp", { version: previous.version })}</p>
      <button
        className="tool align-start"
        disabled={state === "going"}
        onClick={() => {
          if (!window.confirm(t("updates.backConfirm", { version: previous.version }))) return;
          setState("going");
          updates.rollback(previous.version).catch((e) => setState({ error: String(e) }));
        }}
      >
        <Icon name="chevron-left" />
        {state === "going" ? t("updates.going") : t("updates.goBack", { version: previous.version })}
      </button>
      {typeof state === "object" && <p className="help">{state.error}</p>}
    </div>
  );
}

/** The shared profiles gallery (repo `hubdeck-profiles`) is not online yet: its entry stays hidden. */
const GALLERY_ONLINE = false;

const REPO = "https://github.com/Przemotar/hubdeck";
const CONTACT = "przemotar+hubdeck@gmail.com";

/** Opens a web or mail link in the system's browser or mail app. */
function ExternalLink({ href, children }: { href: string; children: React.ReactNode }) {
  return (
    <a
      className="link"
      href={href}
      onClick={(e) => {
        e.preventDefault();
        api.openLink(href).catch((err) => console.error("open", href, err));
      }}
    >
      {children}
    </a>
  );
}

/** Report a problem or suggest a feature: GitHub issue forms with the version filled in. */
function HelpFeedback() {
  useLanguage();
  const version = useAppVersion();
  const [saved, setSaved] = useState<string | null>(null);
  const form = (template: string, extra = "") => `${REPO}/issues/new?template=${template}${extra}`;
  const versionField = `&version=${encodeURIComponent(`HubDeck ${version ?? "?"}, ${osName()}`)}`;
  return (
    <section className="sec">
      <h3 className="sec-title">{t("help.title")}</h3>
      <div className="btn-grid">
        <button className="tool" onClick={() => api.openLink(form("bug.yml", versionField)).catch(() => {})}>
          <Icon name="alert" />
          {t("help.report")}
        </button>
        <button className="tool" onClick={() => api.openLink(form("feature.yml")).catch(() => {})}>
          <Icon name="sparkles" />
          {t("help.suggest")}
        </button>
      </div>
      <p className="help">{t("help.text")}</p>
      <p className="hint">
        <Icon name="info" />
        {t("help.diagnostics")}
      </p>
      <button className="tool align-start" onClick={() => share.saveDiagnostics().then(() => setSaved(t("help.diagnosticsSaved")), (e) => setSaved(String(e)))}>
        <Icon name="download" />
        {t("help.saveDiagnostics")}
      </button>
      {saved && <p className="help">{saved}</p>}
    </section>
  );
}

/** Version, author, contact, source, licence and credits. */
function About() {
  useLanguage();
  const version = useAppVersion();
  return (
    <section className="sec about">
      <div className="about-head">
        <Logo />
        <div>
          <b>HubDeck</b>
          <span className="mono">{version ? `v${version}` : ""}</span>
        </div>
      </div>
      <dl className="facts">
        <dt>{t("about.madeBy")}</dt>
        <dd className="plain">Przemysław Tarkowski</dd>
        <dt>{t("about.contact")}</dt>
        <dd>
          <ExternalLink href={`mailto:${CONTACT}`}>{CONTACT}</ExternalLink>
        </dd>
        <dt>{t("about.source")}</dt>
        <dd>
          <ExternalLink href={REPO}>github.com/Przemotar/hubdeck</ExternalLink>
        </dd>
        <dt>{t("about.licence")}</dt>
        <dd>MIT OR Apache-2.0</dd>
      </dl>
      <p className="help">{t("about.notAffiliated")}</p>
      <p className="help">{t("about.credits")}</p>
      <p className="help">{t("about.gameIcons")}</p>
    </section>
  );
}

/** A newer version found at start: a calm banner; installs only when asked. */
/** macOS: without Accessibility, shortcuts and typed text from the deck do nothing; say so and
 *  lead to the switch. Checked again every few seconds until it is on. */
function AccessibilityBanner() {
  useLanguage();
  const [ok, setOk] = useState(true);
  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    let live = true;
    const check = () => api.accessibilityOk().then((v) => live && setOk(v !== false), () => {});
    check();
    const timer = window.setInterval(check, 4000);
    return () => {
      live = false;
      window.clearInterval(timer);
    };
  }, []);
  if (ok) return null;
  return (
    <div className="banner is-info" role="status">
      <Icon name="keyboard" />
      <span className="grow">{t("access.banner")}</span>
      <button className="tool small" onClick={() => api.accessibilityAsk().catch(() => {})}>
        {t("access.allow")}
      </button>
    </div>
  );
}

/** After HubDeck ended without quitting (a crash, Task Manager): when, and the details to copy. */
function CrashBanner() {
  useLanguage();
  const [report, setReport] = useState<CrashReport | null>(null);
  const [copied, setCopied] = useState(false);
  const appVersion = useAppVersion();
  useEffect(() => {
    api.crashReport().then(setReport, () => setReport(null));
  }, []);
  if (!report) return null;
  const at = report.at.slice(11, 16);
  const details = [`HubDeck ended without quitting at ${report.at}${report.restarted ? " (started again by its guard)" : ""}.`, report.panic ?? "", "", ...report.log].join("\n");
  return (
    <div className="banner is-warn" role="status">
      <Icon name="alert" />
      <span className="grow">{t(report.restarted ? "crash.back" : "crash.stopped", { at })}</span>
      <button className="tool small" onClick={() => navigator.clipboard.writeText(details).then(() => setCopied(true), () => {})}>
        <Icon name="copy" />
        {copied ? t("crash.copied") : t("crash.copy")}
      </button>
      <button
        className="tool small primary"
        onClick={() => {
          // the moment people report most willingly (owner's pick 1a, 2026-10-16); the log is
          // scrubbed by crash_report, cut to what the form's address takes
          const head = [`HubDeck ended without quitting at ${report.at}${report.restarted ? " (started again by its guard)" : ""}.`, report.panic ?? ""];
          const brief = [...head, ...report.log.slice(-25).map((l) => l.slice(0, 220))].join("\n").slice(0, 5000);
          const version = `&version=${encodeURIComponent(`HubDeck ${appVersion ?? "?"}, ${osName()}`)}`;
          api.openLink(`${REPO}/issues/new?template=bug.yml&title=${encodeURIComponent(`HubDeck stopped at ${at}`)}${version}&diagnostics=${encodeURIComponent(brief)}`).catch(() => {});
        }}
      >
        <Icon name="alert" />
        {t("crash.report")}
      </button>
      <button className="tool small" onClick={() => (api.crashSeen(), setReport(null))}>
        {t("crash.ok")}
      </button>
    </div>
  );
}

/** A timer that ran out says where it is and stops from here (a timer on another page or profile
 * only lights the base's edge; owner, 2026-10-16). */
function RingingBanner({ view }: { view: EngineView }) {
  useLanguage();
  const ringing = view.state?.ringing ?? [];
  return (
    <>
      {ringing.map((r) => {
        const name = r.label ?? t("ringing.timer");
        const page = r.page && view.profile?.id === r.profile ? (view.profile.pages?.find((p) => p.id === r.page)?.name ?? r.page) : r.page;
        const where = r.profile !== view.state?.active ? t("ringing.onProfile", { profile: r.profileName }) : page ? t("ringing.onPage", { page }) : "";
        return (
          <div key={`${r.profile}/${r.control}`} className="banner is-warn" role="alert">
            <Icon name="timer" />
            <span className="grow">{t("ringing.text", { name, where })}</span>
            <button className="tool small primary" onClick={() => api.timerStop(r.profile, r.control).catch(() => {})}>
              {t("ringing.stop")}
            </button>
          </div>
        );
      })}
    </>
  );
}

function UpdateBanner() {
  useLanguage();
  const [info, dismiss] = useUpdateAvailable();
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  if (!info) return null;
  return (
    <>
      <div className="banner is-info" role="status">
        <Icon name="sparkles" />
        <span className="grow">
          {t("updates.available", { version: info.version })}
          {error && <span className="banner-note"> ({error})</span>}
        </span>
        {info.notes && (
          <button className="tool ghost small" aria-expanded={open} onClick={() => setOpen((o) => !o)}>
            {t("updates.whatsNew")}
          </button>
        )}
        <button
          className="tool small primary"
          disabled={busy}
          onClick={() => {
            setBusy(true);
            setError(null);
            updates.install().catch((e) => (setError(String(e)), setBusy(false)));
          }}
        >
          <Icon name="download" />
          {busy ? t("updates.installing") : t("updates.install")}
        </button>
        <button className="icon-btn" aria-label={t("updates.later")} title={t("updates.later")} onClick={dismiss}>
          <Icon name="x" />
        </button>
      </div>
      {open && <div className="banner-notes">{info.notes}</div>}
    </>
  );
}

const IS_WINDOWS = navigator.userAgent.includes("Windows");

/** Restart a wedged module's USB device (Windows, with a UAC prompt); reseating always works. */
function Unresponsive({ kind }: { kind: string }) {
  useLanguage();
  const [state, setState] = useState<"idle" | "busy" | string>("idle");
  const name = ["base", "keys", "faders", "knobs", "rollers", "dial"].includes(kind) ? moduleName(kind as ModuleKind) : kind;
  return (
    <div className="banner is-warn" role="alert">
      <Icon name="alert" />
      <span className="grow">
        {t(IS_WINDOWS ? "banner.unresponsiveWin" : "banner.unresponsive", { name })}
        {state !== "idle" && state !== "busy" && <span className="banner-note"> ({state})</span>}
      </span>
      {IS_WINDOWS && (
        <button
          className="tool small"
          disabled={state === "busy"}
          onClick={() => {
            setState("busy");
            api.restartModule(kind).then(
              () => setState(t("banner.restarted")),
              (e) => setState(String(e)),
            );
          }}
        >
          <Icon name="refresh-cw" />
          {state === "busy" ? t("banner.restarting") : t("banner.restart")}
        </button>
      )}
    </div>
  );
}

const TURNS: (number | null)[] = [null, 0, 90, 180, 270];

/** How the base lies on the desk. The sensor only tells "turned right" (90°) from the rest. */
/** The right column while arranging modules: what to check and how to finish (no key editor). */
export function ArrangePanel({ onDone, live, saved, onPlace }: { onDone: () => void; live: LiveState; saved: Record<string, Placement>; onPlace: (key: string, placement: Placement | null) => void }) {
  useLanguage();
  return (
    <aside className="inspector sectioned side-panel">
      <PanelHead title={t("stage.editLayout")} icon="layout-grid" />
      <section className="sec">
        <p className="help">{t("app.hintArranging")}</p>
        <button className="tool primary align-start" onClick={onDone}>
          <Icon name="check" />
          {t("stage.doneLayout")}
        </button>
      </section>
      {/* modules owned but not attached: set up before they are on the base (from the old welcome) */}
      <section className="sec">
        <h3 className="sec-title">{t("welcome.ownedTitle")}</h3>
        <p className="help">{t("welcome.owned")}</p>
        <OwnedKinds live={live} saved={saved} onPlace={onPlace} />
      </section>
    </aside>
  );
}

export function BaseTurnPicker({ live }: { live: LiveState }) {
  useLanguage();
  const { turnOverride, orientation } = live.deck;
  const sensed = orientation === "vertical" ? t("turn.standing") : orientation === "horizontal" ? t("turn.lying") : t("turn.noSensor");
  return (
    <div className="row wrap turn-picker" role="group" aria-label={t("turn.group")}>
      <span className="muted small">{t("turn.label")}</span>
      {TURNS.map((turn) => {
        const label = turn === null ? t("turn.auto") : `${turn}°`;
        return (
          <button key={String(turn)} className={`tool small${turnOverride === turn ? " is-active" : ""}`} onClick={() => api.setBaseTurn(turn)} title={turn === null ? t("turn.follow", { sensed }) : t("turn.always", { turn: label })}>
            {label}
          </button>
        );
      })}
    </div>
  );
}

/**
 * Arrange mode: share the modules the user placed by hand (slot, position, turn) on a GitHub issue
 * form, so slots not yet verified on hardware get right for everyone. Nothing else goes along.
 */
export function ShareArrangement({ live, saved }: { live: LiveState; saved: Record<string, Placement> }) {
  useLanguage();
  const version = useAppVersion();
  const placed = placeModules(live.deck.modules, saved).filter((p) => p.source === "user" && p.module.slot);
  if (!placed.length) return null;
  const lines = [`base turned ${live.deck.turn}°`, ...placed.map((p) => `${p.module.kind} slot ${p.module.slot}: column ${p.col}, row ${p.row}, turned ${p.rotation}°`)];
  const url = `${REPO}/issues/new?template=slots.yml&modules=${encodeURIComponent(lines.join("\n"))}&version=${encodeURIComponent(`HubDeck ${version ?? "?"}, ${osName()}`)}`;
  return (
    <button className="tool small ghost" title={t("arrange.shareHelp")} onClick={() => api.openLink(url).catch(() => {})}>
      <Icon name="globe" />
      {t("arrange.share")}
    </button>
  );
}

/** Key pages of the active profile: the deck shows the selected one. */
export function PageBar({ view }: { view: EngineView }) {
  useLanguage();
  const profile = view.profile;
  const current = view.state?.page ?? null;
  const [naming, setNaming] = useState<{ id: string | null; name: string } | null>(null);
  if (!profile) return null;
  const pages = profile.pages ?? [];
  const currentPage = pages.find((p) => p.id === current);
  const submit = () => {
    if (!naming) return;
    const name = naming.name.trim();
    const editing = naming;
    setNaming(null);
    if (!name) return;
    if (editing.id) api.renamePage(profile.id, editing.id, name).catch((e) => console.error(e));
    else api.createPage(profile.id, name).then((p) => api.setPage(p.id)).catch((e) => console.error(e));
  };
  return (
    <nav
      className="page-bar row wrap"
      aria-label={t("pages.title")}
      // the tabs may scroll sideways (narrow window): the mouse wheel moves them
      onWheel={(e) => {
        if (e.deltaY) e.currentTarget.scrollLeft += e.deltaY;
      }}
    >
      <span className="label">{t("pages.keys")}</span>
      <span className="seg">
        <button className={`tab${current === null ? " is-active" : ""}`} onClick={() => api.setPage(null)}>
          {t("pages.main")}
        </button>
        {pages.map((p) => (
          <button key={p.id} className={`tab${current === p.id ? " is-active" : ""}`} onClick={() => api.setPage(p.id)}>
            {p.name}
          </button>
        ))}
      </span>
      {naming ? (
        <form onSubmit={(e) => (e.preventDefault(), submit())}>
          <input className="input tab-input" autoFocus placeholder={t("pages.name")} value={naming.name} onChange={(e) => setNaming({ ...naming, name: e.target.value })} onBlur={submit} />
        </form>
      ) : (
        <button className="tab icon-only" aria-label={t("pages.new")} title={t("pages.newHelp")} onClick={() => setNaming({ id: null, name: "" })}>
          <Icon name="plus" />
        </button>
      )}
      {currentPage && !naming && (
        <>
          {!pageReachable(profile, currentPage.id) && (
            <span className="page-warn row">
              <Icon name="alert" />
              {t("pages.unreachable")}
              <button className="tool small" title={t("pages.addWayHelp")} onClick={() => api.addWayToPage(profile.id, currentPage.id).catch((e) => window.alert(localizeError(String(e))))}>
                {t("pages.addWay")}
              </button>
            </span>
          )}
          <button className="tool ghost small" onClick={() => setNaming({ id: currentPage.id, name: currentPage.name })}>
            {t("pages.rename")}
          </button>
          <button
            className="tool danger small"
            onClick={() => {
              if (window.confirm(t("pages.confirmDelete", { name: currentPage.name }))) api.deletePage(profile.id, currentPage.id).catch((e) => console.error(e));
            }}
          >
            {t("pages.delete")}
          </button>
        </>
      )}
    </nav>
  );
}

/** Brightness of the key panel and the dial, and when they turn off. */
function ScreensSettings() {
  useLanguage();
  const [s, setS] = useState<Screens | null>(null);
  useEffect(() => {
    api.screens().then(setS, () => {});
  }, []);
  if (!s) return null;
  const save = (next: Screens) => {
    setS(next);
    setPreviewSalt(next.appIcons ?? "");
    api.setScreens(next).catch(() => {});
  };
  return (
    <section className="sec">
      <h3 className="sec-title">{t("settings.screens")}</h3>
      <div className="field">
        <div className="row between">
          <label htmlFor="screen-brightness" className="label">
            {t("settings.brightness")}
          </label>
          <span className="mono value">{s.brightness}%</span>
        </div>
        <input id="screen-brightness" className="range" type="range" min={5} max={100} step={5} value={s.brightness} style={{ "--fill": `${((s.brightness - 5) / 95) * 100}%` } as React.CSSProperties} onChange={(e) => save({ ...s, brightness: Number(e.target.value) })} />
      </div>
      <label className="check">
        <input type="checkbox" checked={s.blankWhenLocked} onChange={(e) => save({ ...s, blankWhenLocked: e.target.checked })} />
        {t("screens.locked")}
      </label>
      <div className="field">
        <label htmlFor="blank-after" className="label">
          {t("screens.after")}
        </label>
        <select id="blank-after" className="input" value={s.blankAfterMinutes} onChange={(e) => save({ ...s, blankAfterMinutes: Number(e.target.value) })}>
          {[0, 5, 10, 15, 30, 60].map((m) => (
            <option key={m} value={m}>
              {m === 0 ? t("screens.never") : t("screens.minutes", { count: m })}
            </option>
          ))}
        </select>
      </div>
      <p className="help">{t("screens.help")}</p>
      {/* the screensaver: mood while you are away, nothing moving while you work */}
      <div className="field">
        <label htmlFor="saver" className="label">
          {t("screens.saver")}
        </label>
        <div className="row">
          <select id="saver" className="input grow" value={s.saver ?? ""} onChange={(e) => save({ ...s, saver: e.target.value })}>
            <option value="">{t("screens.saverOff")}</option>
            <option value="rain">{t("screens.saverRain")}</option>
            <option value="stars">{t("screens.saverStars")}</option>
            <option value="waves">{t("screens.saverWaves")}</option>
          </select>
          {!!s.saver && (
            <select id="saver-after" className="input" aria-label={t("screens.saverAfter")} value={s.saverAfterMinutes ?? 5} onChange={(e) => save({ ...s, saverAfterMinutes: Number(e.target.value) })}>
              {[1, 2, 5, 10, 15, 30].map((m) => (
                <option key={m} value={m}>
                  {t("screens.minutes", { count: m })}
                </option>
              ))}
            </select>
          )}
        </div>
        <p className="help">{t("screens.saverHelp")}</p>
      </div>
      <div className="field">
        <label htmlFor="app-icons" className="label">
          {t("screens.appIcons")}
        </label>
        <select id="app-icons" className="input" value={s.appIcons ?? "icon"} onChange={(e) => save({ ...s, appIcons: e.target.value as Screens["appIcons"] })}>
          <option value="icon">{t("screens.appIconsIcon")}</option>
          <option value="fill">{t("screens.appIconsFill")}</option>
          <option value="fillLabel">{t("screens.appIconsFillLabel")}</option>
        </select>
        <p className="help">{t("screens.appIconsHelp")}</p>
      </div>
    </section>
  );
}

/** Where the profiles are kept: the app's own folder, or a shared one (OneDrive, Dropbox...). */
function ProfilesFolder() {
  useLanguage();
  const [dir, setDir] = useState<[string, boolean] | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const load = () => api.profilesDir().then(setDir, () => {});
  useEffect(() => {
    load();
  }, []);
  if (!dir) return null;
  const change = (next: string | null) =>
    api.setProfilesDir(next).then(
      () => (setMessage(t("folder.changed")), load()),
      (e) => setMessage(String(e)),
    );
  return (
    <section className="sec">
      <h3 className="sec-title">{t("folder.title")}</h3>
      <div className="pathbox" title={dir[0]}>
        <Icon name="folder" />
        <span className="mono">{dir[0]}</span>
      </div>
      <div className="row wrap">
        <button
          className="tool"
          onClick={async () => {
            const picked = await open({ directory: true, multiple: false });
            if (typeof picked === "string") change(picked);
          }}
        >
          <Icon name="folder-open" />
          {t("folder.choose")}
        </button>
        {dir[1] && (
          <button className="tool ghost" onClick={() => change(null)}>
            {t("folder.default")}
          </button>
        )}
      </div>
      <p className="help">{message ?? t("folder.help")}</p>
    </section>
  );
}

/** Copies of all profiles and settings: made daily and before risky changes; restore any. */
function Backups() {
  useLanguage();
  const [list, setList] = useState<{ id: string; when: string; reason: string; profiles: number }[]>([]);
  const [message, setMessage] = useState<string | null>(null);
  const [all, setAll] = useState(false);
  const load = () => api.backups().then(setList, () => {});
  useEffect(() => {
    load();
  }, []);
  const reasons: Record<string, string> = { daily: t("backups.daily"), "before import": t("backups.beforeImport"), "before delete": t("backups.beforeDelete"), "before restore": t("backups.beforeRestore"), "by hand": t("backups.byHand") };
  return (
    <section className="sec">
      <div className="sec-head">
        <h3 className="sec-title">{t("settings.backups")}</h3>
        <button className="tool ghost small" onClick={() => api.backupNow().then(() => (setMessage(t("backups.made")), load()), (e) => setMessage(String(e)))}>
          <Icon name="archive" />
          {t("backups.now")}
        </button>
      </div>
      <p className="help">{t("backups.help")}</p>
      {list.length > 0 && (
        <ul className="rows">
          {(all ? list : list.slice(0, 5)).map((b) => (
            <li key={b.id} className="row-item">
              <span className="row-ic">
                <Icon name="archive" />
              </span>
              <span className="grow row-text">
                <span className="mono">{b.when}</span>
                <small>
                  {reasons[b.reason] ?? b.reason} · {t("backups.profiles", { count: b.profiles })}
                </small>
              </span>
              <button
                className="tool ghost small"
                onClick={() => {
                  if (window.confirm(t("backups.confirm", { when: b.when })))
                    api.restoreBackup(b.id).then(
                      () => (setMessage(t("backups.restored")), load()),
                      (e) => setMessage(String(e)),
                    );
                }}
              >
                {t("backups.restore")}
              </button>
            </li>
          ))}
        </ul>
      )}
      {list.length > 5 && (
        <button className="tool ghost small align-start" onClick={() => setAll((a) => !a)}>
          {all ? t("backups.fewer") : t("backups.all", { count: list.length })}
        </button>
      )}
      {message && <p className="help">{message}</p>}
    </section>
  );
}

/** One connected service: a row with its state that opens to its fields. */
function Service({ icon, hue, name, state, open, children }: { icon: string; hue: string; name: string; state: [ok: boolean, text: string]; open?: boolean; children: React.ReactNode }) {
  return (
    <details className="svc" open={open}>
      <summary>
        <span className="svc-ic" style={{ "--h": hue } as React.CSSProperties}>
          <Icon name={icon} />
        </span>
        <span className="grow svc-name">{name}</span>
        <span className={`pill${state[0] ? " is-ok" : ""}`}>{state[1]}</span>
        <Icon name="chevron-down" className="svc-chev" />
      </summary>
      <div className="svc-body">{children}</div>
    </details>
  );
}

/** Connections shared by every profile. Passwords and tokens go to the system credential store. */
function Connections() {
  useLanguage();
  return (
    <section className="sec">
      <h3 className="sec-title">{t("settings.connections")}</h3>
      <div className="svc-list">
        <ObsConnection />
        <StreamlabsConnection />
        <TwitchConnection />
        <HomeAssistantConnection />
        <SpotifyConnection />
        <ClaudeConnection />
        <AssistantConnection />
        <YourApps />
      </div>
    </section>
  );
}

/** Your own apps (docs/APP-CONTRACT.md): HubDeck listens to their state; keys show it. */
function YourApps() {
  useLanguage();
  const [state, setState] = useState<AppsState | null>(null);
  const [url, setUrl] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const load = () => api.appsState().then(setState, () => {});
  useEffect(() => {
    load();
    const timer = setInterval(load, 3000);
    return () => clearInterval(timer);
  }, []);
  const apps = state?.apps ?? [];
  const items = (id: string) => Object.entries(state?.states ?? {}).filter(([k]) => k.startsWith(`${id}.`));
  const add = () => {
    setBusy(true);
    setMessage(null);
    api.appConnect(url.trim()).then(
      (a) => (setMessage(t("apps.added", { name: a.name, id: a.id })), setUrl(""), setBusy(false), load()),
      (e) => (setMessage(String(e)), setBusy(false)),
    );
  };
  const copyPrompt = () =>
    api
      .appContractPrompt()
      .then((p) => navigator.clipboard.writeText(p))
      .then(() => setMessage(t("apps.promptCopied")), (e) => setMessage(String(e)));
  return (
    <Service icon="app-window" hue="#3b82f6" name={t("apps.title")} state={[apps.length > 0, apps.length ? t("apps.count", { count: apps.length }) : t("connections.stateOff")]}>
      <p className="help">{t("apps.help")}</p>
      {apps.map((a) => {
        const list = items(a.id);
        const waiting = list.filter(([, i]) => i.state === "waiting").length;
        return (
          <div key={a.id} className="row wrap app-row">
            <span className="grow">
              <b>{a.name}</b> <span className="help mono">{a.id}.*</span>
              <br />
              <span className="help">{list.length ? t("apps.items", { count: list.length, waiting }) : t("apps.noItems")}</span>
            </span>
            <button className="icon-btn" title={t("apps.remove")} aria-label={t("apps.remove")} onClick={() => api.appRemove(a.id).then(load, () => {})}>
              <Icon name="x" />
            </button>
          </div>
        );
      })}
      <div className="field">
        <label htmlFor="app-url" className="label">
          {t("apps.address")}
        </label>
        <div className="row">
          <input id="app-url" className="input mono grow" placeholder="http://127.0.0.1:4317/hubdeck/manifest" value={url} onChange={(e) => setUrl(e.target.value)} />
          <button className="tool primary" disabled={busy || !url.trim()} onClick={add}>
            {t("apps.add")}
          </button>
        </div>
      </div>
      <button className="link-btn align-start" onClick={copyPrompt}>
        {t("apps.copyPrompt")}
      </button>
      {message && <p className="help">{message}</p>}
    </Service>
  );
}

function ObsConnection() {
  useLanguage();
  const [url, setUrl] = useState("");
  const [passwordSet, setPasswordSet] = useState(false);
  const [password, setPassword] = useState("");
  const [message, setMessage] = useState<string | null>(null);
  useEffect(() => {
    api.integrations().then((i) => {
      setUrl(i.obsUrl);
      setPasswordSet(i.obsPasswordSet);
    }, () => {});
  }, []);
  return (
    <Service icon="video" hue="#f0506e" name="OBS Studio" state={[passwordSet, passwordSet ? t("connections.stateSet") : t("connections.stateNotSet")]} open={demoParam("svc") === "obs"}>
      <div className="field">
        <label htmlFor="obs-url" className="label">
          {t("connections.obsUrl")}
        </label>
        <input id="obs-url" className="input mono" value={url} onChange={(e) => setUrl(e.target.value)} />
      </div>
      <div className="field">
        <label htmlFor="obs-password" className="label">
          {t("connections.obsPassword")}
        </label>
        <input
          id="obs-password"
          className="input"
          type="password"
          autoComplete="off"
          placeholder={passwordSet ? t("connections.passwordKept") : t("connections.passwordNone")}
          value={password}
          onChange={(e) => setPassword(e.target.value)}
        />
      </div>
      <div className="row wrap">
        <button
          className="tool"
          onClick={() =>
            api.setIntegrations(url, password ? password : null).then(
              () => {
                if (password) setPasswordSet(true);
                setPassword("");
                setMessage(t("connections.saved"));
              },
              (e) => setMessage(String(e)),
            )
          }
        >
          {t("connections.save")}
        </button>
        {passwordSet && (
          <button
            className="tool ghost"
            onClick={() =>
              api.setIntegrations(url, "").then(
                () => {
                  setPasswordSet(false);
                  setMessage(t("connections.passwordRemoved"));
                },
                (e) => setMessage(String(e)),
              )
            }
          >
            {t("connections.removePassword")}
          </button>
        )}
      </div>
      <p className="help">{message ?? t("connections.obsHelp")}</p>
    </Service>
  );
}

/** Claude Code: HubDeck's hooks in ~/.claude/settings.json, so session keys show each session's state. */
function ClaudeConnection() {
  useLanguage();
  const [on, setOn] = useState(false);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [approve, setApprove] = useState(false);
  useEffect(() => {
    invoke<boolean>("claude_status").then((v) => setOn(!!v), () => {});
    invoke<boolean>("claude_approving").then((v) => setApprove(!!v), () => {});
  }, []);
  return (
    <Service icon="bot" hue="#d97757" name="Claude Code" state={[on, on ? t("connections.stateConnected") : t("connections.stateOff")]}>
      <div className="row wrap">
        <button
          className={`tool${on ? " ghost" : ""}`}
          disabled={busy}
          onClick={() => {
            setBusy(true);
            setMessage(null);
            invoke<boolean>("claude_connect", { on: !on }).then(
              (v) => (setOn(!!v), setBusy(false)),
              (e) => (setMessage(String(e)), setBusy(false)),
            );
          }}
        >
          <Icon name="link" />
          {on ? t("connections.claudeDisconnect") : t("connections.claudeConnect")}
        </button>
      </div>
      {on && (
        <label className="check" title={t("connections.claudeApproveHelp")}>
          <input
            type="checkbox"
            checked={approve}
            disabled={busy}
            onChange={(e) => {
              const next = e.target.checked;
              setBusy(true);
              invoke<boolean>("claude_connect", { on: true, approve: next }).then(
                () => (setApprove(next), setBusy(false)),
                (err) => (setMessage(String(err)), setBusy(false)),
              );
            }}
          />
          {t("connections.claudeApprove")}
        </label>
      )}
      <p className="help">{message ?? t("connections.claudeHelp")}</p>
    </Service>
  );
}

/** What `mcp_info` tells about connecting an AI assistant (`engine/mcp.rs`). */
interface McpInfo {
  exe: string;
  desktop: boolean;
  codeCli: boolean;
  codeCommand: string;
  deckCommand?: string;
  snippet: string;
}

/** The user's own AI assistant builds profiles through `hubdeck --mcp` (docs/MCP.md). */
function AssistantConnection() {
  useLanguage();
  const [info, setInfo] = useState<McpInfo | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const load = () => invoke<McpInfo>("mcp_info").then(setInfo, () => {});
  useEffect(() => {
    load();
  }, []);
  const copy = (text: string) => navigator.clipboard.writeText(text).then(() => setMessage(t("connections.mcpCopied")), (e) => setMessage(String(e)));
  const run = (job: Promise<string>) => {
    setBusy(true);
    setMessage(null);
    job.then(
      (m) => (setMessage(m), setBusy(false), load()),
      (e) => (setMessage(String(e)), setBusy(false)),
    );
  };
  return (
    <Service icon="sparkles" hue="#8b7cf6" name={t("connections.mcpName")} state={[!!info?.desktop, info?.desktop ? t("connections.mcpDesktopOn") : t("connections.stateOff")]}>
      <div className="row wrap">
        <button
          className="tool"
          disabled={busy || !info}
          onClick={() => run(invoke<string | null>("mcp_connect_desktop").then((backup) => t("connections.mcpDesktopDone", { backup: backup ?? "-" })))}
        >
          <Icon name="link" />
          {t("connections.mcpDesktop")}
        </button>
        <button
          className="tool"
          disabled={busy || !info}
          onClick={() => (info?.codeCli ? run(invoke<string>("mcp_connect_code").then(() => t("connections.mcpCodeDone"))) : info && copy(info.codeCommand))}
        >
          <Icon name={info?.codeCli ? "link" : "copy"} />
          {info?.codeCli ? t("connections.mcpCode") : t("connections.mcpCodeCopy")}
        </button>
        <button className="tool ghost" disabled={!info} onClick={() => info && copy(info.snippet)}>
          <Icon name="copy" />
          {t("connections.mcpSnippet")}
        </button>
        <button className="tool ghost" disabled={!info?.deckCommand} title={t("connections.mcpDeckHelp")} onClick={() => info?.deckCommand && copy(info.deckCommand)}>
          <Icon name="copy" />
          {t("connections.mcpDeck")}
        </button>
      </div>
      <p className="help">{message ?? t("connections.mcpHelp")}</p>
    </Service>
  );
}

function SpotifyConnection() {
  useLanguage();
  const [clientId, setClientId] = useState("");
  const [connected, setConnected] = useState(false);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  useEffect(() => {
    api.spotifyStatus().then(([id, ok]) => (setClientId(id), setConnected(ok)), () => {});
  }, []);
  return (
    <Service icon="music" hue="#1ed760" name="Spotify" state={[connected, connected ? t("connections.stateConnected") : t("connections.stateOff")]}>
      <div className="field">
        <label htmlFor="spotify-id" className="label">
          {t("connections.spotifyId")}
        </label>
        <input id="spotify-id" className="input mono" placeholder="32 letters and digits" value={clientId} onChange={(e) => setClientId(e.target.value)} />
      </div>
      <div className="row wrap">
        <button
          className="tool"
          disabled={busy}
          onClick={() => {
            setBusy(true);
            setMessage(t("connections.spotifyWaiting"));
            api.spotifyConnect(clientId).then(
              () => (setConnected(true), setMessage(t("connections.spotifyConnected")), setBusy(false)),
              (e) => (setMessage(String(e)), setBusy(false)),
            );
          }}
        >
          <Icon name="link" />
          {connected ? t("connections.spotifyReconnect") : t("connections.spotifyConnect")}
        </button>
        {connected && (
          <button className="tool ghost" onClick={() => api.spotifyDisconnect().then(() => (setConnected(false), setMessage(null)))}>
            {t("connections.spotifyDisconnect")}
          </button>
        )}
      </div>
      <p className="help">{message ?? t("connections.spotifyHelp")}</p>
    </Service>
  );
}

function TwitchConnection() {
  useLanguage();
  const [available, setAvailable] = useState(false);
  const [connected, setConnected] = useState(false);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  useEffect(() => {
    api.twitchStatus().then(([ok, signedIn]) => (setAvailable(ok), setConnected(signedIn)), () => {});
  }, []);
  // a build without a Twitch app id cannot connect: not shown at all
  if (!available) return null;
  return (
    <Service icon="twitch" hue="#a970ff" name={t("connections.twitch")} state={[connected, connected ? t("connections.stateConnected") : available ? t("connections.stateOff") : t("connections.stateUnavailable")]}>
      <div className="row wrap">
        <button
          className="tool"
          disabled={busy || !available}
          onClick={() => {
            setBusy(true);
            setMessage(t("connections.twitchWaiting"));
            api.twitchConnect().then(
              () => (setConnected(true), setMessage(t("connections.twitchConnected")), setBusy(false)),
              (e) => (setMessage(String(e)), setBusy(false)),
            );
          }}
        >
          <Icon name="link" />
          {connected ? t("connections.spotifyReconnect") : t("connections.twitchConnect")}
        </button>
        {connected && (
          <button className="tool ghost" onClick={() => api.twitchDisconnect().then(() => (setConnected(false), setMessage(null)))}>
            {t("connections.spotifyDisconnect")}
          </button>
        )}
      </div>
      <p className="help">{message ?? (available ? t("connections.twitchHelp") : t("connections.twitchNotSetUp"))}</p>
    </Service>
  );
}

function StreamlabsConnection() {
  useLanguage();
  const [url, setUrl] = useState("");
  const [tokenSet, setTokenSet] = useState(false);
  const [token, setToken] = useState("");
  const [message, setMessage] = useState<string | null>(null);
  useEffect(() => {
    api.integrations().then((i) => {
      setUrl(i.streamlabsUrl);
      setTokenSet(i.streamlabsTokenSet);
    }, () => {});
  }, []);
  const save = (tok: string | null) =>
    api.setStreamlabs(url, tok).then(
      () => {
        if (tok !== null) setTokenSet(tok !== "");
        setToken("");
        setMessage(t("connections.saved"));
      },
      (e) => setMessage(String(e)),
    );
  return (
    <Service icon="radio" hue="#80f5d2" name="Streamlabs Desktop" state={[tokenSet, tokenSet ? t("connections.stateSet") : t("connections.stateOff")]}>
      <div className="field">
        <label htmlFor="sl-url" className="label">
          {t("connections.streamlabsUrl")}
        </label>
        <input id="sl-url" className="input mono" value={url} onChange={(e) => setUrl(e.target.value)} />
      </div>
      <div className="field">
        <label htmlFor="sl-token" className="label">
          {t("connections.streamlabsToken")}
        </label>
        <input
          id="sl-token"
          className="input"
          type="password"
          autoComplete="off"
          placeholder={tokenSet ? t("connections.passwordKept") : t("connections.tokenNone")}
          value={token}
          onChange={(e) => setToken(e.target.value)}
        />
      </div>
      <div className="row wrap">
        <button className="tool" onClick={() => save(token ? token : null)}>
          {t("connections.save")}
        </button>
        {tokenSet && (
          <button className="tool ghost" onClick={() => save("")}>
            {t("connections.removeToken")}
          </button>
        )}
      </div>
      <p className="help">{message ?? t("connections.streamlabsHelp")}</p>
    </Service>
  );
}

function HomeAssistantConnection() {
  useLanguage();
  const [url, setUrl] = useState("");
  const [tokenSet, setTokenSet] = useState(false);
  const [token, setToken] = useState("");
  const [message, setMessage] = useState<string | null>(null);
  useEffect(() => {
    api.integrations().then((i) => {
      setUrl(i.haUrl);
      setTokenSet(i.haTokenSet);
    }, () => {});
  }, []);
  const save = (tok: string | null) =>
    api.setHomeAssistant(url, tok).then(
      () => {
        if (tok !== null) setTokenSet(tok !== "");
        setToken("");
        setMessage(t("connections.saved"));
      },
      (e) => setMessage(String(e)),
    );
  return (
    <Service icon="home" hue="#41bdf5" name="Home Assistant" state={[tokenSet && !!url, tokenSet && url ? t("connections.stateSet") : t("connections.stateOff")]}>
      <div className="field">
        <label htmlFor="ha-url" className="label">
          {t("connections.haUrl")}
        </label>
        <input id="ha-url" className="input mono" placeholder="http://homeassistant.local:8123" value={url} onChange={(e) => setUrl(e.target.value)} />
      </div>
      <div className="field">
        <label htmlFor="ha-token" className="label">
          {t("connections.haToken")}
        </label>
        <input
          id="ha-token"
          className="input"
          type="password"
          autoComplete="off"
          placeholder={tokenSet ? t("connections.passwordKept") : t("connections.tokenNone")}
          value={token}
          onChange={(e) => setToken(e.target.value)}
        />
      </div>
      <div className="row wrap">
        <button className="tool" onClick={() => save(token ? token : null)}>
          {t("connections.save")}
        </button>
        {tokenSet && (
          <button className="tool ghost" onClick={() => save("")}>
            {t("connections.removeToken")}
          </button>
        )}
      </div>
      <p className="help">{message ?? t("connections.haHelp")}</p>
    </Service>
  );
}

/** The games the deck can play, with the best scores. */
export function GamesPanel({ view, onClose }: { view: EngineView; onClose: () => void }) {
  useLanguage();
  const [games, setGames] = useState<GameSummary[]>([]);
  const playing = !!view.state?.playing;
  useEffect(() => {
    api.games().then(setGames, () => {});
  }, [playing]);
  return (
    <aside className="inspector">
      <div className="row">
        <h2 className="grow">{t("games.title")}</h2>
        <button className="tool" onClick={onClose}>
          {t("profile.close")}
        </button>
      </div>
      <p className="muted small">{t("games.help")}</p>
      <button className={`tool${playing ? "" : " is-active"}`} onClick={() => api.playGames(!playing)}>
        {playing ? t("games.stop") : t("games.play")}
      </button>
      <ul className="plugin-list">
        {games.map((g) => {
          const icon = view.icons.find((i) => i.name === g.icon);
          return (
            <li key={g.id} className="plugin-item">
              <div className="row">
                {icon && <img className="game-icon" src={`data:image/svg+xml;utf8,${encodeURIComponent(icon.svg.replace(/currentColor/g, "#c4b5fd"))}`} alt="" />}
                <strong className="grow">{localizeGame(g.id, "name", g.name)}</strong>
                <span className="muted small">{g.best !== null ? t("games.best", { score: g.best }) : t("games.noBest")}</span>
              </div>
              <div className="muted small">{localizeGame(g.id, "about", g.about)}</div>
            </li>
          );
        })}
      </ul>
      <WheelChoices />
      <p className="muted small">{t("games.key")}</p>
    </aside>
  );
}

/** The wheel's fields, one per line. */
function WheelChoices() {
  useLanguage();
  const [text, setText] = useState("");
  useEffect(() => {
    api.wheel().then((c) => setText(c.join("\n")), () => {});
  }, []);
  return (
    <div className="field">
      <label htmlFor="wheel-choices" className="label">
        {t("games.wheel")}
      </label>
      <textarea
        id="wheel-choices"
        className="input"
        rows={4}
        value={text}
        onChange={(e) => setText(e.target.value)}
        onBlur={() => api.setWheel(text.split("\n")).then((c) => setText(c.join("\n")), () => {})}
      />
      <div className="muted small">{t("games.wheelHelp")}</div>
    </div>
  );
}

/** Installed plugins and where to put new ones. */
/** Installed plugins (Settings > Plugins). */
function PluginsList() {
  useLanguage();
  const [plugins, setPlugins] = useState<PluginInfo[]>([]);
  const [message, setMessage] = useState<string | null>(null);
  useEffect(() => {
    const load = () => api.plugins().then(setPlugins, () => {});
    load();
    const timer = window.setInterval(load, 2000);
    return () => window.clearInterval(timer);
  }, []);
  return (
    <>
      <section className="sec">
        <p className="help">
          {t("plugins.help")} <ExternalLink href={`${REPO}/blob/main/docs/PLUGINS.md`}>{t("plugins.howTo")}</ExternalLink>
        </p>
        <div className="row wrap">
          <button
            className="tool primary align-start"
            onClick={async () => {
              const path = await open({ multiple: false, filters: [{ name: t("plugins.sdFile"), extensions: ["streamDeckPlugin", "zip"] }] });
              if (typeof path !== "string") return;
              setMessage(t("plugins.installing"));
              api.sdInstall(path).then(
                (id) => (setMessage(t("plugins.installed1", { id })), api.plugins().then(setPlugins)),
                (e) => setMessage(localizeError(String(e))),
              );
            }}
          >
            <Icon name="download" />
            {t("plugins.installSd")}
          </button>
          <button className="tool align-start" onClick={() => api.openPluginsFolder().catch((e) => setMessage(String(e)))}>
            <Icon name="folder-open" />
            {t("plugins.openFolder")}
          </button>
        </div>
        <p className="help">{t("plugins.sdHelp")}</p>
        {message && <p className="help">{message}</p>}
      </section>
      <SdCatalog installed={new Set(plugins.map((p) => p.id))} onInstalled={() => api.plugins().then(setPlugins, () => {})} />
      <section className="sec">
        <h3 className="sec-title">
          {t("plugins.installed")} <span className="n">{plugins.length}</span>
        </h3>
        {plugins.length === 0 ? (
          <div className="empty compact">
            <span className="eic">
              <Icon name="plug" />
            </span>
            <b>{t("plugins.none")}</b>
            <span>{t("plugins.noneHelp")}</span>
          </div>
        ) : (
          <ul className="cards">
            {plugins.map((p) => (
              <li key={p.id} className="card">
                <div className="row">
                  <strong className="grow">{p.name}</strong>
                  <span className="mono chip-ver">{p.version}</span>
                  <span className={`pill${p.connected ? " is-ok" : " is-err"}`}>{p.connected ? t("plugins.running") : t("plugins.stopped")}</span>
                </div>
                <div className="mono help">{p.id}</div>
                <div className="meta">
                  <span>{t("plugins.actions", { count: p.actions })}</span>
                  {p.cpu !== null && <span className="mono">CPU {p.cpu.toFixed(1)}%</span>}
                  {p.memoryMb !== null && <span className="mono">{p.memoryMb} MB</span>}
                  {!p.connected && <span>{localizeError(p.status)}</span>}
                  {p.restarts > 0 && <span>{t("plugins.restarts", { count: p.restarts })}</span>}
                  {/* a plugin that eats the CPU says so (PAIN-POINTS SD11) */}
                  {p.cpu !== null && p.cpu > 20 && <span className="pill is-warn">{t("plugins.busy")}</span>}
                </div>
                <div className="row">
                  {p.held || !p.connected ? (
                    <button className="tool" onClick={() => api.pluginStart(p.id).then(() => api.plugins().then(setPlugins), (e) => setMessage(String(e)))}>
                      <Icon name="play" />
                      {t("plugins.start")}
                    </button>
                  ) : (
                    <button className="tool" onClick={() => api.pluginStop(p.id).then(() => api.plugins().then(setPlugins), (e) => setMessage(String(e)))}>
                      <Icon name="x" />
                      {t("plugins.stop")}
                    </button>
                  )}
                </div>
                {p.permissions.length > 0 && (
                  <div className="perm">
                    {t("plugins.uses")}
                    {p.permissions.map((x) => (
                      <span key={x} className="pill">
                        {x}
                      </span>
                    ))}
                  </div>
                )}
              </li>
            ))}
          </ul>
        )}
      </section>
    </>
  );
}

/** How many catalog plugins show before the user searches. */
const CATALOG_SHOWN = 30;

/** The open-source plugin catalog (Settings > Plugins): fetched only when asked, search, Install. */
function SdCatalog({ installed, onInstalled }: { installed: Set<string>; onInstalled: () => void }) {
  useLanguage();
  const [list, setList] = useState<CatalogPlugin[] | "loading" | "offline" | null>(null);
  const [query, setQuery] = useState("");
  const [state, setState] = useState<Record<string, "installing" | string>>({});
  const load = () => {
    setList("loading");
    api.sdCatalog().then(setList, () => setList("offline"));
  };
  const install = (p: CatalogPlugin) => {
    setState((s) => ({ ...s, [p.id]: "installing" }));
    api.sdCatalogInstall(p.id).then(
      () => (setState((s) => ({ ...s, [p.id]: "" })), onInstalled()),
      (e) => setState((s) => ({ ...s, [p.id]: localizeError(String(e)) })),
    );
  };
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  const found = Array.isArray(list) ? list.filter((p) => words.every((w) => `${p.name} ${p.author} ${p.description}`.toLowerCase().includes(w))) : [];
  return (
    <section className="sec">
      <h3 className="sec-title">{t("plugins.catalog")}</h3>
      <p className="help">{t("plugins.catalogHelp")}</p>
      {list === null ? (
        <button className="tool align-start" onClick={load}>
          <Icon name="search" />
          {t("plugins.catalogOpen")}
        </button>
      ) : list === "loading" ? (
        <p className="help">{t("plugins.catalogLoading")}</p>
      ) : list === "offline" ? (
        <div className="row wrap">
          <p className="help grow">{t("plugins.catalogOffline")}</p>
          <button className="tool small" onClick={load}>
            <Icon name="refresh-cw" />
            {t("gallery.retry")}
          </button>
        </div>
      ) : (
        <>
          <input className="input" type="search" placeholder={t("plugins.catalogSearch")} aria-label={t("plugins.catalogSearch")} value={query} onChange={(e) => setQuery(e.target.value)} />
          {found.length === 0 ? (
            <p className="help">{t("plugins.catalogNone")}</p>
          ) : (
            <ul className="cards">
              {found.slice(0, CATALOG_SHOWN).map((p) => {
                const s = state[p.id];
                return (
                  <li key={p.id} className="card catalog-item">
                    <img className="catalog-icon" src={p.icon} alt="" loading="lazy" onError={(e) => (e.currentTarget.style.visibility = "hidden")} />
                    <div className="grow">
                      <div className="row">
                        <strong className="grow">{p.name}</strong>
                        {installed.has(p.id) ? (
                          <span className="pill is-ok">{t("plugins.isInstalled")}</span>
                        ) : (
                          <button className="tool small" disabled={s === "installing"} onClick={() => install(p)}>
                            <Icon name="download" />
                            {s === "installing" ? t("plugins.installing") : t("plugins.install")}
                          </button>
                        )}
                      </div>
                      <div className="help">{p.description}</div>
                      <div className="meta">
                        <span>{t("plugins.byAuthor", { author: p.author })}</span>
                        {p.tried && <span className="is-ok">{t("plugins.tried", { systems: p.tried })}</span>}
                        {p.aiAssisted && <span>{t("plugins.aiAssisted")}</span>}
                        <button className="link" onClick={() => api.openLink(p.repository).catch(() => {})}>
                          {t("plugins.source")}
                        </button>
                      </div>
                      {s && s !== "installing" && <div className="help catalog-err">{s}</div>}
                    </div>
                  </li>
                );
              })}
            </ul>
          )}
          {found.length > CATALOG_SHOWN && <p className="help">{t("plugins.catalogMore", { shown: CATALOG_SHOWN, count: found.length })}</p>}
        </>
      )}
    </section>
  );
}

const KINDS = ["keys", "faders", "dial", "knobs", "rollers"] as const;

/** Profiles other people share (the gallery): preview, modules it needs, Add. */
export function GalleryPanel({ live, saved, onClose }: { live: LiveState; saved: Record<string, Placement>; onClose: () => void }) {
  useLanguage();
  const [entries, setEntries] = useState<GalleryEntry[] | null>(null);
  const [failed, setFailed] = useState(false);
  const [state, setState] = useState<Record<string, "adding" | "added" | string>>({});
  // what was copied to fit the user's modules, per added entry
  const [notes, setNotes] = useState<Record<string, string>>({});
  const load = () => {
    setFailed(false);
    setEntries(null);
    share.gallery().then(setEntries, () => setFailed(true));
  };
  useEffect(load, []);
  // the user's modules: attached ones and the ones on the shelf
  const owned = new Set<string>([...live.deck.modules.map((m) => m.kind), ...detachedKinds(live.deck.modules, saved)]);
  const add = (e: GalleryEntry) => {
    setState((s) => ({ ...s, [e.id]: "adding" }));
    share.add(e.package).then(
      async (p) => {
        if (!(await keepImported(p))) return setState((s) => ({ ...s, [e.id]: "" }));
        setState((s) => ({ ...s, [e.id]: "added" }));
        setNotes((n) => ({ ...n, [e.id]: movedNote(p.moved, p.pages) }));
        api.activate(p.id);
      },
      (err) => setState((s) => ({ ...s, [e.id]: String(err) })),
    );
  };
  return (
    <aside className="inspector sectioned side-panel">
      <PanelHead title={t("gallery.title")} crumb={t("gallery.crumb")} icon="layers" onClose={onClose} />
      {failed ? (
        <div className="empty">
          <span className="eic">
            <Icon name="globe" />
          </span>
          <b>{t("gallery.offlineTitle")}</b>
          <span>{t("gallery.offline")}</span>
          <button className="tool small" onClick={load}>
            <Icon name="refresh-cw" />
            {t("gallery.retry")}
          </button>
        </div>
      ) : entries === null ? (
        <div className="empty">
          <span className="eic spin">
            <Icon name="refresh-cw" />
          </span>
          <span>{t("gallery.loading")}</span>
        </div>
      ) : entries.length === 0 ? (
        <div className="empty">
          <span className="eic">
            <Icon name="layers" />
          </span>
          <b>{t("gallery.emptyTitle")}</b>
          <span>{t("gallery.offline")}</span>
        </div>
      ) : (
        <section className="sec">
          <ul className="cards">
            {entries.map((e) => {
              const missing = e.kinds.filter((k) => !owned.has(k));
              const st = state[e.id];
              return (
                <li key={e.id} className="card gallery-card">
                  <div className="gallery-preview">
                    {e.preview ? (
                      <img src={e.preview} alt="" loading="lazy" />
                    ) : (
                      <span className="gallery-kinds">
                        {KINDS.filter((k) => e.kinds.includes(k)).map((k) => (
                          <ModuleThumb key={k} kind={k} muted={!owned.has(k)} />
                        ))}
                      </span>
                    )}
                  </div>
                  <div className="row">
                    <div className="grow">
                      <strong>{e.name}</strong>
                      {e.author && <div className="help">{t("gallery.by", { author: e.author })}</div>}
                    </div>
                    <button className={`tool small${st === "added" ? "" : " primary"}`} disabled={st === "adding" || st === "added"} onClick={() => add(e)}>
                      <Icon name={st === "added" ? "check" : "download"} />
                      {st === "adding" ? t("gallery.adding") : st === "added" ? t("gallery.added") : t("gallery.add")}
                    </button>
                  </div>
                  {e.description && <p className="gallery-desc">{e.description}</p>}
                  <div className="perm">
                    {t("gallery.needs")}
                    {KINDS.filter((k) => e.kinds.includes(k)).map((k) => (
                      <span key={k} className={`pill${owned.has(k) ? "" : " is-missing"}`} title={owned.has(k) ? undefined : t("gallery.missing")}>
                        {t(`kind.${k}`)}
                      </span>
                    ))}
                    {e.tags.map((tag) => (
                      <span key={tag} className="tag">
                        #{tag}
                      </span>
                    ))}
                  </div>
                  {missing.length > 0 && <p className="help">{t("gallery.missingHelp", { list: missing.map((k) => t(`kind.${k as ModuleKind}`)).join(", ") })}</p>}
                  {st && st !== "adding" && st !== "added" && <p className="help">{st}</p>}
                  {notes[e.id] && <p className="help">{notes[e.id]}</p>}
                </li>
              );
            })}
          </ul>
        </section>
      )}
      <section className="sec end">
        <p className="help">
          {t("gallery.share")} <ExternalLink href={`${REPO}/blob/main/docs/SHARING.md`}>{t("gallery.howTo")}</ExternalLink>
        </p>
      </section>
    </aside>
  );
}

/** The five kinds of module as tiles: attached ones ticked and fixed, the others toggle "I own one". */
function OwnedKinds({ live, saved, onPlace }: { live: LiveState; saved: Record<string, Placement>; onPlace: (key: string, placement: Placement | null) => void }) {
  const attached = new Set(live.deck.modules.map((m) => m.kind));
  return (
    <div className="owned-kinds" role="group" aria-label={t("welcome.ownedTitle")}>
      {MODULE_KINDS.map((kind) => {
        const isAttached = attached.has(kind);
        const owned = isAttached || keysOfKind(saved, kind).length > 0;
        return (
          <button
            key={kind}
            className="owned-kind"
            role="checkbox"
            aria-checked={owned}
            disabled={isAttached}
            onClick={() => (owned ? keysOfKind(saved, kind).forEach((k) => onPlace(k, null)) : onPlace(ownedKey(kind), { col: 0, row: 0, rotation: 0 }))}
          >
            <span className="owned-tick" aria-hidden="true">
              {owned && (
                <svg viewBox="0 0 24 24">
                  <path d="M20 6 9 17l-5-5" />
                </svg>
              )}
            </span>
            <ModuleThumb kind={kind} muted={!owned} />
            {t(`module.${kind}`)}
            {isAttached && <small>{t("welcome.attached")}</small>}
          </button>
        );
      })}
    </div>
  );
}

export function Banner({ live, view, onOpenProfiles, onOpenConnections }: { live: LiveState; view: EngineView; onOpenProfiles?: () => void; onOpenConnections?: () => void }) {
  useLanguage();
  const made = (view.state?.autoMade ?? []).map((id) => view.state?.profiles.find((p) => p.id === id)?.name).filter(Boolean) as string[];
  const names = made.length > 4 ? `${made.slice(0, 4).join(", ")} +${made.length - 4}` : made.join(", ");
  return (
    <>
      <UpdateBanner />
      <RingingBanner view={view} />
      <CrashBanner />
      <AccessibilityBanner />
      {made.length > 0 && (
        <div className="banner is-info" role="status">
          <Icon name="sparkles" />
          <span className="grow">{t(made.length === 1 ? "auto.bannerOne" : "auto.banner", { names })}</span>
          {onOpenProfiles && (
            <button className="tool small" onClick={onOpenProfiles}>
              {t("auto.show")}
            </button>
          )}
          <button className="tool small" onClick={() => api.autoSeen()}>
            {t("auto.ok")}
          </button>
        </div>
      )}
      {(view.state?.appsJoined?.length ?? 0) > 0 && (
        <div className="banner is-info" role="status">
          <Icon name="link" />
          <span className="grow">{t("apps.joined", { names: view.state?.appsJoined?.join(", ") ?? "" })}</span>
          {onOpenConnections && (
            <button className="tool small" onClick={() => (onOpenConnections(), api.appsSeen())}>
              {t("auto.show")}
            </button>
          )}
          <button className="tool small" onClick={() => api.appsSeen()}>
            {t("auto.ok")}
          </button>
        </div>
      )}
      {view.state?.playing && (
        <div className="banner is-info" role="status">
          <Icon name="play" />
          <span className="grow">{t("games.banner")}</span>
          <button className="tool small" onClick={() => api.playGames(false)}>
            {t("games.stop")}
          </button>
        </div>
      )}
      {live.deck.paused && (
        <div className="banner is-info" role="status">
          <Icon name="pause" />
          <span className="grow">{t("banner.paused")}</span>
          <button className="tool small" onClick={() => api.pauseDeck(false)}>
            <Icon name="play" />
            {t("banner.resume")}
          </button>
        </div>
      )}
      {live.deck.unresponsive.map((kind) => (
        <Unresponsive key={kind} kind={kind} />
      ))}
      {view.error && (
        <div className="banner is-err" role="alert">
          <Icon name="alert" />
          <span className="grow">
            <b>{controlLabel(view.error.control)}</b> {localizeError(view.error.message)}
          </span>
        </div>
      )}
    </>
  );
}

/**
 * An imported profile that runs commands, web requests or scripts: the user sees them and keeps it
 * or removes it before it is ever used (release audit, 2026-10-13). True = keep.
 */
async function keepImported(p: Imported): Promise<boolean> {
  const runs = p.runs ?? [];
  if (!runs.length) return true;
  const list = runs.slice(0, 8).map((r) => `• ${r.length > 90 ? `${r.slice(0, 90)}…` : r}`).join("\n") + (runs.length > 8 ? `\n${t("import.runsMore", { count: runs.length - 8 })}` : "");
  const keep = await ask(t("import.runsAsk", { name: p.name, list }), { title: t("import.runsTitle"), kind: "warning", okLabel: t("import.keep"), cancelLabel: t("import.remove") }).catch(() => false);
  if (!keep) await api.deleteProfile(p.id).catch(() => {});
  return keep;
}
