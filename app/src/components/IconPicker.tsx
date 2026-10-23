import { useEffect, useMemo, useState } from "react";
import { confirmed, open } from "../confirm";
import type { EngineView } from "../engine";
import { iconPacks, type PackIcon, type PackInfo } from "../iconpacks";
import { t, useLanguage } from "../i18n";
import { Icon } from "./Icon";
import { demoParam } from "../demo/params";

/** Game icons (game-icons.net, CC BY 3.0), loaded when first shown; looks name them `game:<name>`. */
const GAME_FILES = import.meta.glob("../../src-tauri/assets/game-icons/*.svg", { query: "?raw", import: "default" }) as Record<string, () => Promise<string>>;
let gameIcons: Promise<{ name: string; svg: string }[]> | null = null;
function loadGameIcons() {
  gameIcons ??= Promise.all(
    Object.entries(GAME_FILES).map(async ([path, load]) => ({ name: `game:${path.split("/").pop()!.replace(/\.svg$/, "")}`, svg: await load() })),
  );
  return gameIcons;
}

type Category = "interface" | "games" | `pack:${string}`;

/** Icons by category (interface, games, installed icon packs) with search. A pack icon is a picture. */
export function IconPicker({ value, onChange, onPicture, view }: { value?: string; onChange: (v?: string) => void; onPicture: (image: string) => void; view: EngineView }) {
  useLanguage();
  const [category, setCategory] = useState<Category>(value?.startsWith("game:") ? "games" : ((demoParam("icons") as Category | null) ?? "interface"));
  const [query, setQuery] = useState("");
  const [games, setGames] = useState<{ name: string; svg: string }[] | null>(null);
  const [packs, setPacks] = useState<PackInfo[]>([]);
  const [packIcons, setPackIcons] = useState<Record<string, PackIcon[]>>({});
  const [error, setError] = useState<string | null>(null);
  const q = query.trim().toLowerCase();

  const loadPacks = () => iconPacks.list().then((p) => setPacks(Array.isArray(p) ? p : []), () => setPacks([]));
  useEffect(() => {
    loadPacks();
  }, []);
  const install = async (directory: boolean) => {
    setError(null);
    const path = await open(directory ? { directory: true, multiple: false } : { multiple: false, filters: [{ name: t("icons.packFile"), extensions: ["zip", "streamDeckIconPack"] }] });
    if (typeof path !== "string") return;
    iconPacks.install(path).then((p) => (loadPacks(), setCategory(`pack:${p.id}`)), (e) => setError(String(e)));
  };
  // game icons are loaded when their tab opens or a search may need them
  useEffect(() => {
    if ((category === "games" || q) && !games) loadGameIcons().then(setGames);
  }, [category, q, games]);
  useEffect(() => {
    if (!category.startsWith("pack:")) return;
    const id = category.slice(5);
    if (!packIcons[id]) iconPacks.icons(id).then((icons) => setPackIcons((p) => ({ ...p, [id]: icons })), (e) => setError(String(e)));
  }, [category, packIcons]);

  const matches = (name: string, tags: string[] = []) => !q || name.toLowerCase().includes(q) || tags.some((tag) => tag.toLowerCase().includes(q));
  const shownIcons = useMemo(() => {
    const ui = view.icons.map((i) => ({ name: i.name, svg: i.svg }));
    if (q) return [...ui, ...(games ?? [])].filter((i) => matches(i.name.replace(/^game:/, "").replace(/-/g, " ")));
    return category === "games" ? (games ?? []) : category === "interface" ? ui : [];
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [q, category, games, view.icons]);
  const pack = category.startsWith("pack:") ? category.slice(5) : null;
  const shownPack = pack ? (packIcons[pack] ?? []).filter((i) => matches(i.name, i.tags)) : [];

  const tabs: [Category, string][] = [
    ["interface", t("icons.interface")],
    ["games", t("icons.games")],
    ...packs.map((p) => [`pack:${p.id}` as Category, p.name] as [Category, string]),
  ];
  return (
    <div className="icon-picker">
      <div className="search">
        <Icon name="search" />
        <input className="input" placeholder={t("icons.search")} aria-label={t("icons.search")} value={query} onChange={(e) => setQuery(e.target.value)} />
      </div>
      {!q && (
        <div className="icon-tabs" role="tablist">
          {tabs.map(([id, label]) => (
            <button key={id} role="tab" aria-selected={category === id} className={`tab${category === id ? " is-active" : ""}`} onClick={() => setCategory(id)}>
              {label}
            </button>
          ))}
          <button className="tab icon-only" title={`${t("icons.addPack")}. ${t("icons.packHelp")}`} aria-label={t("icons.addPack")} onClick={() => install(false)}>
            <Icon name="plus" />
          </button>
        </div>
      )}
      {pack && !q ? (
        <div className="icon-grid pack" role="listbox" aria-label={t("look.icon")}>
          {shownPack.map((i) => (
            <button
              key={i.file}
              className="icon-cell picture"
              title={i.name}
              aria-label={i.name}
              onClick={() => iconPacks.use(pack, i.file).then(onPicture, (e) => setError(String(e)))}
            >
              <img src={i.uri} alt="" loading="lazy" />
            </button>
          ))}
        </div>
      ) : (
        <div className="icon-grid" role="listbox" aria-label={t("look.icon")}>
          {!q && category === "interface" && (
            <button className={`icon-cell${!value ? " is-active" : ""}`} onClick={() => onChange(undefined)} aria-label={t("look.noIcon")}>
              –
            </button>
          )}
          {shownIcons.map((i) => (
            <button key={i.name} className={`icon-cell${i.name.startsWith("game:") ? " game" : ""}${value === i.name ? " is-active" : ""}`} onClick={() => onChange(i.name)} title={i.name.replace(/^game:/, "")} aria-label={i.name.replace(/^game:/, "")}>
              <span dangerouslySetInnerHTML={{ __html: i.svg }} />
            </button>
          ))}
          {(category === "games" || q) && !games && <span className="help">{t("icons.loading")}</span>}
          {q && shownIcons.length === 0 && <span className="help">{t("icons.none")}</span>}
        </div>
      )}
      {category === "games" && !q && <p className="help">{t("icons.gamesCredit")}</p>}
      {pack && !q && (
        <div className="row wrap">
          <button
            className="tool ghost small"
            onClick={() => {
              void confirmed(t("icons.removePackConfirm"), t("confirm.remove")).then((yes) => {
                if (yes) iconPacks.remove(pack).then(() => (loadPacks(), setCategory("interface")), (e) => setError(String(e)));
              });
            }}
          >
            <Icon name="trash" />
            {t("icons.removePack")}
          </button>
        </div>
      )}
      {/* what a pack is and where it may come from: in the "+" tooltip, not under every key */}
      {category === "interface" && !q && (
        <button className="link-btn align-start" title={t("icons.packHelp")} onClick={() => install(true)}>
          {t("icons.addFolder")}
        </button>
      )}
      {error && <p className="help">{error}</p>}
    </div>
  );
}
