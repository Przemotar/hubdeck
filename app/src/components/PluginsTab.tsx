import { useEffect, useMemo, useState } from "react";
import { api, markPluginNew, type CatalogPlugin } from "../engine";
import { localizeError, t, useLanguage, type Key } from "../i18n";
import { Icon } from "./Icon";
import { askUninstall } from "./Panels";

/** The system this window runs on, as the catalog names systems. */
const HERE = typeof navigator !== "undefined" && /Mac/i.test(navigator.platform || navigator.userAgent) ? "macOS" : "Windows";
const SOURCES = ["elgato", "openaction", "github"] as const;
const CATEGORIES = ["Audio", "Music", "Streaming", "Communication", "Lighting", "Smart home", "Monitoring", "Productivity", "Developer", "Gaming", "Other"] as const;
type Sort = "popular" | "updated" | "tried" | "name";
/** Cards shown before "Show more". */
const PAGE = 60;

/** Elgato's rank first, then GitHub stars: the order "Most popular" means. */
const popularity = (a: CatalogPlugin, b: CatalogPlugin) => (a.popular ?? 1e9) - (b.popular ?? 1e9) || (b.stars ?? 0) - (a.stars ?? 0) || a.name.localeCompare(b.name);
const SORTS: Record<Sort, (a: CatalogPlugin, b: CatalogPlugin) => number> = {
  popular: popularity,
  updated: (a, b) => (b.updated ?? "").localeCompare(a.updated ?? "") || popularity(a, b),
  tried: (a, b) => Number(!!b.tried) - Number(!!a.tried) || popularity(a, b),
  name: (a, b) => a.name.localeCompare(b.name),
};

/** The plugin catalog as its own tab (owner's pick A, 2026-10-18): free Stream Deck plugins from
 * Elgato Marketplace, OpenAction and GitHub that install here with one click; filters on the left,
 * search and sort above the cards. */
export function PluginsTab({ onClose, onInstalled, onUse }: { onClose: () => void; onInstalled: () => void; onUse: (pluginId: string) => void }) {
  useLanguage();
  const [list, setList] = useState<CatalogPlugin[] | "loading" | "offline">("loading");
  const [installed, setInstalled] = useState<Set<string>>(new Set());
  const [state, setState] = useState<Record<string, "installing" | string>>({});
  const [query, setQuery] = useState("");
  const [sources, setSources] = useState<Set<string>>(new Set(SOURCES));
  const [hereOnly, setHereOnly] = useState(true);
  const [dialOnly, setDialOnly] = useState(false);
  const [triedOnly, setTriedOnly] = useState(false);
  const [category, setCategory] = useState<string | null>(null);
  const [sort, setSort] = useState<Sort>("popular");
  const [shown, setShown] = useState(PAGE);
  const load = () => {
    setList("loading");
    api.sdCatalog().then(setList, () => setList("offline"));
  };
  const refreshInstalled = () => api.plugins().then((p) => setInstalled(new Set(p.map((x) => x.id))), () => {});
  useEffect(() => {
    load();
    refreshInstalled();
  }, []);
  useEffect(() => setShown(PAGE), [query, sources, hereOnly, dialOnly, triedOnly, category, sort]);
  const install = (p: CatalogPlugin) => {
    setState((s) => ({ ...s, [p.id]: "installing" }));
    api.sdCatalogInstall(p.id).then(
      (id) => (markPluginNew(id || p.id), setState((s) => ({ ...s, [p.id]: "" })), refreshInstalled(), onInstalled()),
      (e) => setState((s) => ({ ...s, [p.id]: localizeError(String(e)) })),
    );
  };
  const uninstall = (p: CatalogPlugin) =>
    askUninstall({ id: p.id, name: p.name }).then(
      (done) => {
        if (done) refreshInstalled();
      },
      (e) => (setState((s) => ({ ...s, [p.id]: localizeError(String(e)) })), refreshInstalled()),
    );
  const all = Array.isArray(list) ? list : [];
  // every filter but the one being counted, so a count says what picking it would show
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  const passes = (p: CatalogPlugin, skip?: "source" | "category") =>
    (skip === "source" || sources.has(p.source)) &&
    (skip === "category" || !category || (p.category ?? "Other") === category) &&
    (!hereOnly || p.systems.length === 0 || p.systems.includes(HERE)) &&
    (!dialOnly || p.dial) &&
    (!triedOnly || !!p.tried) &&
    words.every((w) => `${p.name} ${p.author} ${p.description}`.toLowerCase().includes(w));
  const found = useMemo(() => all.filter((p) => passes(p)).sort(SORTS[sort]), [list, query, sources, hereOnly, dialOnly, triedOnly, category, sort]);
  const count = (pred: (p: CatalogPlugin) => boolean, skip: "source" | "category") => all.filter((p) => pred(p) && passes(p, skip)).length;
  const toggleSource = (s: string) => setSources((now) => (now.has(s) ? new Set([...now].filter((x) => x !== s)) : new Set([...now, s])));
  return (
    <div className="settings-page plugins-tab">
      <header className="settings-head">
        <button className="tool ghost" onClick={onClose}>
          <Icon name="chevron-left" />
          {t("settings.back")}
        </button>
        <h2>{t("topbar.plugins")}</h2>
        <span className="grow" />
      </header>
      <div className="settings-body">
        <aside className="settings-nav plugins-filters" aria-label={t("pluginsTab.filters")}>
          <fieldset>
            <legend>{t("pluginsTab.source")}</legend>
            {SOURCES.map((s) => (
              <label key={s} className="check">
                <input type="checkbox" checked={sources.has(s)} onChange={() => toggleSource(s)} />
                <span className="grow">{t(`pluginsTab.src.${s}` as Key)}</span>
                <span className="n">{count((p) => p.source === s, "source")}</span>
              </label>
            ))}
          </fieldset>
          <fieldset>
            <legend>{t("pluginsTab.worksWith")}</legend>
            <label className="check">
              <input type="checkbox" checked={hereOnly} onChange={(e) => setHereOnly(e.target.checked)} />
              {t("pluginsTab.thisSystem", { os: HERE })}
            </label>
            <label className="check">
              <input type="checkbox" checked={dialOnly} onChange={(e) => setDialOnly(e.target.checked)} />
              {t("pluginsTab.dial")}
            </label>
            <label className="check">
              <input type="checkbox" checked={triedOnly} onChange={(e) => setTriedOnly(e.target.checked)} />
              {t("pluginsTab.triedOnly")}
            </label>
          </fieldset>
          <fieldset>
            <legend>{t("pluginsTab.category")}</legend>
            <button className={`settings-nav-item${category === null ? " is-active" : ""}`} aria-pressed={category === null} onClick={() => setCategory(null)}>
              <span className="grow">{t("pluginsTab.all")}</span>
            </button>
            {CATEGORIES.map((c) => {
              const n = count((p) => (p.category ?? "Other") === c, "category");
              return n === 0 && category !== c ? null : (
                <button key={c} className={`settings-nav-item${category === c ? " is-active" : ""}`} aria-pressed={category === c} onClick={() => setCategory(c)}>
                  <span className="grow">{t(`pluginsTab.cat.${c}` as Key)}</span>
                  <span className="n">{n}</span>
                </button>
              );
            })}
          </fieldset>
        </aside>
        <div className="settings-content plugins-main">
          <div className="plugins-bar">
            <input className="input grow" type="search" placeholder={t("plugins.catalogSearch")} aria-label={t("plugins.catalogSearch")} value={query} onChange={(e) => setQuery(e.target.value)} />
            <label className="plugins-sort">
              {t("pluginsTab.sort")}
              <select className="input" value={sort} onChange={(e) => setSort(e.target.value as Sort)}>
                <option value="popular">{t("pluginsTab.sortPopular")}</option>
                <option value="updated">{t("pluginsTab.sortUpdated")}</option>
                <option value="tried">{t("pluginsTab.sortTried")}</option>
                <option value="name">{t("pluginsTab.sortName")}</option>
              </select>
            </label>
          </div>
          <p className="help">{t("pluginsTab.help")}</p>
          {list === "loading" ? (
            <p className="help">{t("plugins.catalogLoading")}</p>
          ) : list === "offline" ? (
            <div className="row wrap">
              <p className="help grow">{t("plugins.catalogOffline")}</p>
              <button className="tool small" onClick={load}>
                <Icon name="refresh-cw" />
                {t("gallery.retry")}
              </button>
            </div>
          ) : found.length === 0 ? (
            <p className="help">{t("plugins.catalogNone")}</p>
          ) : (
            <>
              <ul className="plugin-grid">
                {found.slice(0, shown).map((p) => (
                  <PluginCard key={p.id} plugin={p} installed={installed.has(p.id)} state={state[p.id]} onInstall={() => install(p)} onUse={() => onUse(p.id)} onUninstall={() => uninstall(p)} />
                ))}
              </ul>
              {found.length > shown && (
                <button className="tool align-start" onClick={() => setShown((n) => n + PAGE)}>
                  {t("pluginsTab.more", { count: found.length - shown })}
                </button>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
}

/** What a plugin needs set up in another program before it shows anything, from its README
 * (owner, 2026-10-20: the HWiNFO plugin listed no sensors until HWiNFO's shared memory was on and
 * HWiNFO restarted; a user with an AMD card saw only its logo). */
const SETUP: Record<string, Key> = {
  "com.moeilijk.hwinfo": "pluginsTab.setup.hwinfoShm",
  "com.5e.hwinfo-reader": "pluginsTab.setup.hwinfoGadget",
};

function PluginCard({ plugin: p, installed, state, onInstall, onUse, onUninstall }: { plugin: CatalogPlugin; installed: boolean; state?: string; onInstall: () => void; onUse: () => void; onUninstall: () => void }) {
  const { lang } = useLanguage();
  // in the window's language, not the system's (a Polish system wrote "maj 2024" in English)
  const updated = p.updated ? new Date(`${p.updated}T12:00:00`).toLocaleDateString(lang === "en" ? "en-GB" : lang, { month: "short", year: "numeric" }) : null;
  return (
    <li className="card plugin-card">
      <div className="row">
        <img className="catalog-icon" src={p.icon} alt="" loading="lazy" onError={(e) => (e.currentTarget.style.visibility = "hidden")} />
        <div className="grow plugin-card-name">
          <strong>{p.name}</strong>
          {p.author && <span className="help">{t("plugins.byAuthor", { author: p.author })}</span>}
        </div>
      </div>
      {p.description && <p className="help plugin-card-about">{p.description}</p>}
      {SETUP[p.id] && <p className="help plugin-card-setup">{t(SETUP[p.id])}</p>}
      <div className="plugin-card-tags">
        <span className="pill">{t(`pluginsTab.src.${p.source}` as Key)}</span>
        {p.popular ? <span className="pill">{t("pluginsTab.popElgato", { n: p.popular })}</span> : p.stars ? <span className="pill">{t("pluginsTab.stars", { n: p.stars })}</span> : null}
        {/* tried on another system says which (owner, 2026-10-23 on the Mac: "Tried" on a plugin
            tried only on Windows) */}
        {p.tried ? <span className="pill is-ok">{p.tried.includes(HERE) ? t("pluginsTab.tried") : t("pluginsTab.triedOn", { os: p.tried })}</span> : p.source === "github" && <span className="pill">{t("pluginsTab.notTried")}</span>}
        {p.dial && <span className="pill">{t("pluginsTab.dialTag")}</span>}
      </div>
      {/* systems and date on a line of their own: next to the buttons they broke into four lines */}
      <span className="help plugin-card-meta">{[p.systems.join(" · "), updated && t("pluginsTab.updated", { date: updated })].filter(Boolean).join(" — ")}</span>
      <div className="row plugin-card-foot">
        <span className="grow" />
        {p.repository && (
          <button className="link" onClick={() => api.openLink(p.repository).catch(() => {})}>
            {t("plugins.source")}
          </button>
        )}
        {installed ? (
          <>
            <span className="pill is-ok">{t("plugins.isInstalled")}</span>
            {/* where he looked for it (owner's pick 1a, 2026-10-23); an icon: a word does not fit the line */}
            <button className="icon-btn" title={t("plugins.uninstall")} aria-label={t("plugins.uninstall")} onClick={onUninstall}>
              <Icon name="trash" />
            </button>
            {/* where its actions are: the deck, with the library open at them */}
            <button className="tool small primary" onClick={onUse}>
              <Icon name="chevron-right" />
              {t("pluginsTab.use")}
            </button>
          </>
        ) : (
          <button className="tool small primary" disabled={state === "installing"} onClick={onInstall}>
            <Icon name="download" />
            {state === "installing" ? t("plugins.installing") : t("plugins.install")}
          </button>
        )}
      </div>
      {state && state !== "installing" && <div className="help catalog-err">{state}</div>}
    </li>
  );
}
