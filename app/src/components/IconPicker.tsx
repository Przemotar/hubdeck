import { useEffect, useMemo, useState } from "react";
import { confirmed, open } from "../confirm";
import { api, type EngineView } from "../engine";
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

type Category = "recent" | "interface" | "emoji" | "games" | `pack:${string}`;

/** Elgato's icon packs (Stream Deck's Marketplace); a pack downloaded there opens in HubDeck. */
const MARKETPLACE_ICONS = "https://marketplace.elgato.com/stream-deck/icons";

/** Emoji for a key, drawn in the system's color emoji font; looks name them `emoji:<emoji>`. A
 * list, not a typed field (owner's rule, 2026-10-16); the names are for the search. */
const EMOJI_LIST: [string, string][] = [
  ["🎵", "music note"], ["🎶", "music notes"], ["🎧", "headphones"], ["🔊", "speaker loud"], ["🔇", "mute"], ["🎤", "microphone"], ["🎙️", "studio microphone"], ["📻", "radio"],
  ["▶️", "play"], ["⏸️", "pause"], ["⏭️", "next"], ["⏮️", "previous"], ["⏹️", "stop"], ["⏺️", "record"], ["🔁", "repeat"], ["🔀", "shuffle"],
  ["❤️", "heart love like"], ["⭐", "star"], ["🔥", "fire hot"], ["👍", "thumbs up"], ["👎", "thumbs down"], ["🗑️", "trash bin delete"], ["🏷️", "tag label"], ["✅", "check done"],
  ["❌", "cross no"], ["⚠️", "warning"], ["❓", "question"], ["💡", "light bulb idea"], ["🔒", "lock"], ["🔓", "unlock"], ["🔔", "bell notification"], ["🔕", "bell off"],
  ["💻", "laptop computer"], ["🖥️", "desktop computer"], ["⌨️", "keyboard"], ["🖱️", "mouse"], ["📁", "folder files"], ["📄", "document"], ["📋", "clipboard"], ["✂️", "scissors cut"],
  ["📷", "camera"], ["🎥", "movie camera video"], ["📸", "camera flash screenshot"], ["🖼️", "picture"], ["🎨", "palette art"], ["✏️", "pencil edit"], ["🔍", "search"], ["⚙️", "settings gear"],
  ["📧", "mail email"], ["💬", "chat message"], ["📞", "phone call"], ["📅", "calendar"], ["⏰", "alarm clock"], ["⏱️", "stopwatch timer"], ["🍅", "tomato pomodoro"], ["☕", "coffee break"],
  ["🎮", "game controller"], ["🕹️", "joystick"], ["🏆", "trophy"], ["🎯", "target"], ["🎲", "dice"], ["👾", "alien game"], ["⚔️", "swords"], ["🛡️", "shield"],
  ["🏠", "home"], ["💡", "lamp light"], ["🌙", "moon night"], ["☀️", "sun day"], ["🌡️", "thermometer temperature"], ["🔋", "battery"], ["🔌", "plug power"], ["📶", "signal wifi"],
  ["🚀", "rocket launch"], ["🤖", "robot ai"], ["🧠", "brain"], ["👀", "eyes look"], ["😀", "smile"], ["😎", "cool"], ["🙂", "slight smile"], ["🎉", "party"],
  ["🔴", "red circle"], ["🟠", "orange circle"], ["🟡", "yellow circle"], ["🟢", "green circle"], ["🔵", "blue circle"], ["🟣", "purple circle"], ["⚫", "black circle"], ["⚪", "white circle"],
];
const EMOJI = EMOJI_LIST.filter(([e], i) => EMOJI_LIST.findIndex(([x]) => x === e) === i);

/** Icons by category (interface, games, installed icon packs) with search. A pack icon is a picture. */
export function IconPicker({ value, onChange, onPicture, view }: { value?: string; onChange: (v?: string) => void; onPicture: (image: string) => void; view: EngineView }) {
  useLanguage();
  const [category, setCategory] = useState<Category>(value?.startsWith("game:") ? "games" : value?.startsWith("emoji:") ? "emoji" : ((demoParam("icons") as Category | null) ?? "interface"));
  // the icons picked lately (as Stream Deck's and the official app's "Frequently used"): the
  // first tab, opened first when there are some and the key has no game or emoji icon
  const [recent, setRecent] = useState<string[]>([]);
  useEffect(() => {
    api.recentIcons().then(
      (list) => {
        const l = Array.isArray(list) ? list : [];
        setRecent(l);
        if (l.length && !value?.startsWith("game:") && !value?.startsWith("emoji:") && !demoParam("icons")) setCategory("recent");
      },
      () => {},
    );
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  const pick = (icon?: string) => {
    onChange(icon);
    if (icon) api.recentIconAdd(icon).then((l) => Array.isArray(l) && setRecent(l), () => {});
  };
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
    if ((category === "games" || q || (category === "recent" && recent.some((r) => r.startsWith("game:")))) && !games) loadGameIcons().then(setGames);
  }, [category, q, games, recent]);
  // a search looks in every installed pack too: their icons are loaded once a search starts
  useEffect(() => {
    if (!q) return;
    for (const p of packs) if (!packIcons[p.id]) iconPacks.icons(p.id).then((icons) => setPackIcons((all) => ({ ...all, [p.id]: icons })), () => {});
  }, [q, packs, packIcons]);
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
  const shownEmoji = category === "emoji" || q ? EMOJI.filter(([, name]) => matches(name)) : [];
  // pack icons that match a search, from every pack (at most 60 shown)
  const foundInPacks = q ? packs.flatMap((p) => (packIcons[p.id] ?? []).filter((i) => matches(i.name, i.tags)).map((i) => ({ pack: p.id, icon: i }))).slice(0, 60) : [];
  const svgOf = (name: string) => view.icons.find((i) => i.name === name)?.svg ?? games?.find((g) => g.name === name)?.svg;

  const tabs: [Category, string][] = [
    ...(recent.length ? [["recent", t("icons.recent")] as [Category, string]] : []),
    ["interface", t("icons.interface")],
    ["emoji", t("icons.emoji")],
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
      {category === "recent" && !q ? (
        <div className="icon-grid" role="listbox" aria-label={t("icons.recent")}>
          {recent.map((name) =>
            name.startsWith("emoji:") ? (
              <button key={name} className={`icon-cell emoji${value === name ? " is-active" : ""}`} onClick={() => pick(name)} title={name.slice(6)} aria-label={EMOJI.find(([e]) => e === name.slice(6))?.[1] ?? name.slice(6)}>
                {name.slice(6)}
              </button>
            ) : svgOf(name) ? (
              <button key={name} className={`icon-cell${name.startsWith("game:") ? " game" : ""}${value === name ? " is-active" : ""}`} onClick={() => pick(name)} title={name.replace(/^game:/, "")} aria-label={name.replace(/^game:/, "")}>
                <span dangerouslySetInnerHTML={{ __html: svgOf(name)! }} />
              </button>
            ) : null,
          )}
        </div>
      ) : pack && !q ? (
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
            <button className={`icon-cell${!value ? " is-active" : ""}`} onClick={() => pick(undefined)} aria-label={t("look.noIcon")}>
              –
            </button>
          )}
          {(category === "emoji" || q) &&
            shownEmoji.map(([e, name]) => (
              <button key={e} className={`icon-cell emoji${value === `emoji:${e}` ? " is-active" : ""}`} onClick={() => pick(`emoji:${e}`)} title={name} aria-label={name}>
                {e}
              </button>
            ))}
          {shownIcons.map((i) => (
            <button key={i.name} className={`icon-cell${i.name.startsWith("game:") ? " game" : ""}${value === i.name ? " is-active" : ""}`} onClick={() => pick(i.name)} title={i.name.replace(/^game:/, "")} aria-label={i.name.replace(/^game:/, "")}>
              <span dangerouslySetInnerHTML={{ __html: i.svg }} />
            </button>
          ))}
          {(category === "games" || q) && !games && <span className="help">{t("icons.loading")}</span>}
          {q && shownIcons.length === 0 && shownEmoji.length === 0 && foundInPacks.length === 0 && <span className="help">{t("icons.none")}</span>}
        </div>
      )}
      {q && foundInPacks.length > 0 && (
        <>
          <span className="label">{t("icons.inPacks")}</span>
          <div className="icon-grid pack" role="listbox" aria-label={t("icons.inPacks")}>
            {foundInPacks.map(({ pack: p, icon: i }) => (
              <button key={`${p}/${i.file}`} className="icon-cell picture" title={i.name} aria-label={i.name} onClick={() => iconPacks.use(p, i.file).then(onPicture, (e) => setError(String(e)))}>
                <img src={i.uri} alt="" loading="lazy" />
              </button>
            ))}
          </div>
        </>
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
      {!q && (
        <button className="link-btn align-start" title={t("icons.moreHelp")} onClick={() => api.openLink(MARKETPLACE_ICONS)}>
          {t("icons.more")}
        </button>
      )}
      {error && <p className="help">{error}</p>}
    </div>
  );
}
