//! The catalog of open-source plugins for Stream Deck and OpenAction (the OpenAction Marketplace's
//! list, about 200 plugins), browsed in Settings > Plugins and installed with one click. The list
//! and the plugin files come from GitHub; nothing is fetched until the user opens the catalog.
//! Elgato Marketplace's most downloaded plugins join it by name: the ones whose authors publish an
//! open plugin file install the same way; the others say they come only through Elgato's Stream
//! Deck app (its Marketplace installs into that app, and many of its files are encrypted for it).

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;

const CATALOG: &str = "https://openactionapi.github.io/plugins/catalogue.json";
/// Plugin icons: `<ICONS><id>.png`.
const ICONS: &str = "https://openactionapi.github.io/plugins/icons/";
/// Elgato Marketplace's public list of plugins, most downloaded first (the one its own site reads;
/// not documented, so any failure only leaves Elgato's names out).
const ELGATO_LIST: &str = "https://mp-gateway.elgato.com/products?extension=plugins&ordering=-download_count&limit=100";
/// How many of Elgato's most downloaded plugins join the catalog.
const ELGATO_TOP: usize = 300;
const ELGATO_PAGE: &str = "https://marketplace.elgato.com/product/";
const KEEP_FOR: Duration = Duration::from_secs(3600);
const MAX_PLUGIN: u64 = 200 * 1024 * 1024;

/// Catalog plugins checked in HubDeck, and on which systems: installed from the catalog, started,
/// connected and drew a key (`scripts/ui/plugin-check.mjs`, 2026-10-16/17; Spotify on the owner's
/// Mac deck, 2026-10-13). Not every action of each was pressed.
const TRIED: &[(&str, &str)] = &[
    ("com.ntanis.essentials-for-spotify", "Windows, macOS"),
    ("me.amankhanna.oadiscord", "Windows"),
    ("me.amankhanna.oasystem", "Windows"),
    ("com.jk.weather", "Windows"),
    ("de.perdoctus.streamdeck.homeassistant", "Windows"),
    ("com.chrisregado.googlemeet", "Windows"),
    ("fr.twitchat", "Windows"),
    ("com.fredemmott.audiooutputswitch", "Windows"),
    ("com.moeilijk.hwinfo", "Windows"),
    ("com.elgato.philips-hue", "Windows"),
    ("com.elgato.cpu", "Windows"),
    ("de.tobimori.streamdeck.ifttt", "Windows"),
    ("com.fredemmott.micmutetoggle", "Windows"),
    ("org.tynsoe.streamdeck.wsproxy", "Windows"),
    ("fun.shiro.ytmd", "Windows"),
    ("com.jetbrains.ide", "Windows"),
    ("cz.danol.discordmixer", "Windows"),
    ("com.frostycoolslug.goxlr-utility", "Windows"),
    ("org.xiphis.ohs", "Windows"),
    ("com.github.mjbnz.sd-api-request", "Windows"),
    ("com.marlburrow.uptime-kuma", "Windows"),
    ("com.5e.hwinfo-reader", "Windows"),
    ("com.geekyeggo.sounddeck", "Windows"),
    ("dev.santiagomartin.devops", "Windows"),
    ("tech.flighttracker.streamdeck", "Windows"),
    ("com.start-automating.scriptdeck", "Windows"),
];

/// Elgato Marketplace plugins whose authors publish an open plugin file: Elgato's product id, the
/// plugin's id, and its GitHub repository when the OpenAction catalog does not have it. Matched by
/// hand on 2026-10-17 (author and plugin id checked; 36 of Elgato's 100 most downloaded).
const ELGATO_OPEN: &[(&str, &str, Option<&str>)] = &[
    ("cf41cd3d-ef80-4122-bef2-7f4fb82fcaac", "com.fredemmott.audiooutputswitch", None),
    ("705c5433-1e05-4d8a-844f-b5914b7f642f", "com.fredemmott.micmutetoggle", None),
    ("abaf7a76-225a-42db-b911-fc3f66f3878c", "cz.danol.discordmixer", None),
    ("131c7cee-6a63-4e89-840e-156b0d17c0bb", "com.geekyeggo.sounddeck", None),
    ("884c8c3e-8477-4ecb-99e0-f3101bbfa0aa", "de.perdoctus.streamdeck.homeassistant", None),
    ("6cf535ec-acc0-436a-a7da-5a7697c56d0b", "com.javanpoirier.audiomixer", None),
    ("27f49792-2de3-455f-8892-fd382716f548", "com.elgato.philips-hue", None),
    ("ecbbfd1e-6a47-43b3-8056-3aa69d35e8e1", "com.geekyeggo.goveecontroller", None),
    ("37c38d3a-a366-426b-a56d-d56c89e5960c", "com.dahnandpartners.ytticker", None),
    ("d7164662-bf3f-41aa-9e16-cfc29c2212d4", "com.nthompson.gpu", None),
    ("61254b24-a60c-4a87-acae-70c39eda8a19", "com.nicollasr.streamdeckvsc", None),
    ("4dc3acfb-f1ae-496d-8c17-26975316af2d", "fun.shiro.ytmd", None),
    ("a7db5477-c76c-49b1-bce1-b6039f2d41d2", "io.bitfocus.companion-plugin", None),
    ("0ba9f87f-d5ec-44ca-98ac-6b95e6e477d3", "dev.cazzar.streamdeck.vtubestudio", None),
    ("378b180d-1380-4a25-9528-19c482f5a7cf", "com.ntanis.essentials-for-spotify", None),
    ("913e25f8-92d2-4cd2-973f-76b9d1cdc90e", "de.tobimori.streamdeck.ifttt", None),
    ("6926228a-2efa-4fb9-849a-3f7d9ad86a9b", "dev.theca11.multiobs", None),
    ("57b209e9-b4a7-4ca4-a0a5-53c79fe6bea1", "com.rgpaul.vlc", None),
    ("9bfc5a15-c5b3-4865-9bec-1375cc65c25d", "fail.marc.onairclock", None),
    ("cb3993d5-9e95-40bd-bb1d-22b3b499f5a7", "com.geekyeggo.windowsdisplaysettings", None),
    ("00cb3238-508b-4e7d-a380-3b0c6b21417c", "com.victorgrycuk.colorpicker", None),
    ("c81ceed8-448d-4649-925e-d84b58296f1e", "com.gallowaylabs.tomato", None),
    ("48907651-231b-4981-917e-db58885a64f8", "com.exension.stocks", None),
    ("45474a5c-6884-41cb-a896-411da79e0804", "com.viddie.dice", None),
    ("af576388-8cbb-4d59-bdec-206dc3f4168e", "com.moeilijk.lhm", None),
    ("d1e420bd-5437-421d-a7e1-378877908f76", "eu.dirondin.streamdeck.ram", Some("dirondin/streamdeck-ram")),
    ("5316b393-1dbc-43a4-b9b4-ad3ba9f58ff3", "com.nagopy.streamdeck.tasktimer", None),
    ("916c9a3f-453e-47f5-a194-5719448cdd41", "com.dlprows.focusvolumecontrol", None),
    ("657b3d18-a53f-45ac-a787-12b7e1adbee7", "com.baptiewright.nanoleaf", None),
    ("6e64a83c-f4c8-4769-b3a7-311d308295f5", "com.zergo0.teamspeak3integration", None),
    ("820f1bb5-465d-408b-aabf-2d46a05a10d9", "fr.twitchat", None),
    ("77f73a30-0336-4f4e-80b6-42e22a1a127e", "me.hckr.appswitcher", None),
    ("737415fa-1957-489d-a62e-1becec068b6e", "co.meldstudio.streamdeck", Some("MeldStudio/streamdeck")),
    ("ea8cc86d-1a3b-45de-893d-592d174089c9", "com.5e.hwinfo-reader", None),
    ("1c4afa3d-157a-4d2f-91bc-eb71865dad1c", "tech.flighttracker.streamdeck", None),
    ("29d301d8-e245-497e-97e0-23fb09542bb1", "com.davidgolunski.goveelightcontroller", None),
];

/// The id of a plugin that comes only through Elgato's Stream Deck app.
const ELGATO_ONLY: &str = "elgato:";

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub author: String,
    pub description: String,
    pub repository: String,
    /// The plugin's icon, a web address.
    pub icon: String,
    #[serde(skip)]
    pub download_url: Option<String>,
    /// The catalog marks plugins written with an AI's help.
    pub ai_assisted: bool,
    /// Checked in HubDeck on these systems ("Windows", "Windows, macOS"); None = not tried yet.
    pub tried: Option<String>,
    /// Its page on Elgato Marketplace, when it is there.
    pub elgato: Option<String>,
    /// Its place among Elgato Marketplace's most downloaded plugins (1 = most).
    pub popular: Option<u32>,
    /// Only through Elgato's Stream Deck app: no open file to install here.
    pub elgato_only: bool,
}

/// Every field may be missing or null in the real catalog.
#[derive(Deserialize)]
struct Raw {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    author: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    repository: Option<String>,
    #[serde(default)]
    download_url: Option<String>,
    #[serde(default)]
    flags: Value,
}

/// The catalog's `catalogue.json` (`{ id: { name, author, description, repository, download_url?,
/// flags? } }`), sorted by name.
pub fn parse(bytes: &[u8]) -> Result<Vec<Entry>, String> {
    let raw: std::collections::BTreeMap<String, Raw> = serde_json::from_slice(bytes).map_err(|e| format!("the plugin catalog is not readable: {e}"))?;
    let mut list: Vec<Entry> = raw
        .into_iter()
        .filter_map(|(id, r)| {
            Some(Entry {
                icon: format!("{ICONS}{id}.png"),
                name: r.name.filter(|n| !n.trim().is_empty())?,
                repository: r.repository.unwrap_or_default(),
                author: r.author.unwrap_or_default(),
                description: r.description.unwrap_or_default(),
                download_url: r.download_url.filter(|u| !u.is_empty()),
                ai_assisted: r.flags.get("ai_assisted").and_then(Value::as_bool).unwrap_or(false),
                tried: TRIED.iter().find(|(t, _)| *t == id).map(|(_, systems)| systems.to_string()),
                elgato: None,
                popular: None,
                elgato_only: false,
                id,
            })
        })
        .collect();
    list.sort_by_key(|e| e.name.to_lowercase());
    Ok(list)
}

/// A plugin on Elgato Marketplace (one entry of its `results`).
#[derive(Deserialize)]
struct ElgatoRaw {
    id: String,
    name: String,
    #[serde(default)]
    org_name: Option<String>,
    #[serde(default)]
    slug: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    app_icon_cdn: Option<String>,
    #[serde(default)]
    variants: Vec<Value>,
}

/// One page of Elgato Marketplace's list.
fn parse_elgato(bytes: &[u8]) -> Result<Vec<ElgatoRaw>, String> {
    #[derive(Deserialize)]
    struct Page {
        results: Vec<ElgatoRaw>,
    }
    serde_json::from_slice::<Page>(bytes).map(|p| p.results).map_err(|e| format!("Elgato's list is not readable: {e}"))
}

/// The first paragraph of a Marketplace description, without its Markdown, at most 220 letters.
fn first_paragraph(text: &str) -> String {
    let para = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    let plain: String = para.chars().filter(|c| !matches!(c, '*' | '_' | '#' | '`')).collect();
    let plain = plain.trim();
    if plain.chars().count() > 220 { format!("{}…", plain.chars().take(220).collect::<String>().trim_end()) } else { plain.to_string() }
}

fn norm(s: &str) -> String {
    s.chars().filter(char::is_ascii_alphanumeric).collect::<String>().to_lowercase()
}

/// Elgato's most downloaded plugins (in order) joined to the catalog: a plugin matched by hand
/// (`ELGATO_OPEN`) or by the same name and author marks its catalog entry (or comes in with its
/// GitHub repository); any other becomes an entry that only Elgato's app installs.
fn merge_elgato(list: &mut Vec<Entry>, elgato: Vec<ElgatoRaw>) {
    for (rank, e) in elgato.into_iter().enumerate() {
        let popular = Some(rank as u32 + 1);
        let page = e.slug.as_deref().map(|s| format!("{ELGATO_PAGE}{s}"));
        let author = e.org_name.clone().unwrap_or_default();
        let open = ELGATO_OPEN.iter().find(|(id, ..)| *id == e.id);
        let at = match open {
            Some((_, plugin, _)) => list.iter().position(|c| c.id == *plugin),
            None => list.iter().position(|c| c.popular.is_none() && !author.is_empty() && norm(&c.name) == norm(&e.name) && norm(&c.author) == norm(&author)),
        };
        if let Some(at) = at {
            list[at].elgato = page;
            list[at].popular = popular;
            continue;
        }
        let mut description = first_paragraph(e.description.as_deref().unwrap_or(""));
        if e.variants.iter().any(|v| v["is_free"].as_bool() == Some(false)) {
            description = format!("(paid) {description}");
        }
        let (id, repository, elgato_only) = match open {
            Some((_, plugin, Some(repo))) => (plugin.to_string(), format!("https://github.com/{repo}"), false),
            _ => (format!("{ELGATO_ONLY}{}", e.id), String::new(), true),
        };
        list.push(Entry {
            tried: TRIED.iter().find(|(t, _)| *t == id).map(|(_, systems)| systems.to_string()),
            id,
            name: e.name,
            author,
            description,
            repository,
            icon: e.app_icon_cdn.unwrap_or_default(),
            download_url: None,
            ai_assisted: false,
            elgato: page,
            popular,
            elgato_only,
        });
    }
    // Elgato's most downloaded first, then the rest by name
    list.sort_by(|a, b| a.popular.unwrap_or(u32::MAX).cmp(&b.popular.unwrap_or(u32::MAX)).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
}

/// Elgato Marketplace's most downloaded plugins; empty when its list cannot be read.
fn elgato_top() -> Vec<ElgatoRaw> {
    let mut all = Vec::new();
    for offset in (0..ELGATO_TOP).step_by(100) {
        match super::share::download(&format!("{ELGATO_LIST}&offset={offset}"), 4 * 1024 * 1024).and_then(|b| parse_elgato(&b)) {
            Ok(page) if !page.is_empty() => all.extend(page),
            Ok(_) => break,
            Err(e) => {
                log::warn!("plugin catalog: Elgato Marketplace's list: {e}");
                break;
            }
        }
    }
    all
}

static CACHE: Mutex<Option<(Instant, Vec<Entry>)>> = Mutex::new(None);

fn catalog() -> Result<Vec<Entry>, String> {
    if let Some((at, list)) = CACHE.lock().unwrap_or_else(|e| e.into_inner()).as_ref()
        && at.elapsed() < KEEP_FOR
    {
        return Ok(list.clone());
    }
    let elgato = std::thread::spawn(elgato_top);
    let mut list = parse(&super::share::download(CATALOG, 4 * 1024 * 1024)?)?;
    merge_elgato(&mut list, elgato.join().unwrap_or_default());
    *CACHE.lock().unwrap_or_else(|e| e.into_inner()) = Some((Instant::now(), list.clone()));
    Ok(list)
}

/// `owner/repo` of a GitHub repository address.
fn github_repo(url: &str) -> Option<String> {
    let rest = url.strip_prefix("https://github.com/")?;
    let mut parts = rest.trim_end_matches('/').trim_end_matches(".git").split('/');
    let (owner, repo) = (parts.next()?, parts.next()?);
    (!owner.is_empty() && !repo.is_empty()).then(|| format!("{owner}/{repo}"))
}

/// The plugin file to download from a release's assets: a `.streamDeckPlugin`, else a `.zip`; of
/// several, one named for this system before one named only for another (ScriptDeck's first file
/// needs PowerShell 7, its `windowsscriptdeck` one the PowerShell every Windows has, 2026-10-17).
pub fn pick_asset(release: &Value) -> Option<String> {
    pick_asset_for(release, cfg!(windows))
}

fn pick_asset_for(release: &Value, windows: bool) -> Option<String> {
    let assets = release["assets"].as_array()?;
    #[derive(PartialEq)]
    enum Os {
        Windows,
        Mac,
        Linux,
    }
    // "windowsscriptdeck", "win64", "macos", "osx"; not "macro"
    let os_of = |word: &str| match word {
        w if w.starts_with("windows") || ["win", "win32", "win64"].contains(&w) => Some(Os::Windows),
        w if w.starts_with("macos") || ["mac", "osx", "darwin"].contains(&w) => Some(Os::Mac),
        w if w.starts_with("linux") => Some(Os::Linux),
        _ => None,
    };
    let here = if windows { Os::Windows } else { Os::Mac };
    // 0: named for this system, 1: for no system, 2: only for others
    let rank = |name: &str| {
        let named: Vec<Os> = name.to_lowercase().split(|c: char| !c.is_ascii_alphanumeric()).filter_map(os_of).collect();
        if named.contains(&here) {
            0
        } else if named.is_empty() {
            1
        } else {
            2
        }
    };
    let url = |suffix: &str| {
        assets
            .iter()
            .filter_map(|a| Some((a["name"].as_str()?, a["browser_download_url"].as_str()?)))
            .filter(|(n, _)| n.to_lowercase().ends_with(suffix))
            .min_by_key(|(n, _)| rank(n))
            .map(|(_, u)| u.to_string())
    };
    url(".streamdeckplugin").or_else(|| url(".zip"))
}

/// Where a catalog plugin's file is: its own download address, else its latest GitHub release.
fn file_url(entry: &Entry) -> Result<String, String> {
    if let Some(url) = &entry.download_url {
        if url.starts_with("https://github.com/") || url.starts_with("https://raw.githubusercontent.com/") {
            return Ok(url.clone());
        }
        return Err("the plugin's download is not on GitHub".into());
    }
    let repo = github_repo(&entry.repository).ok_or("the plugin's repository is not on GitHub")?;
    if let Some(url) =
        super::share::download(&format!("https://api.github.com/repos/{repo}/releases/latest"), 1024 * 1024).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()).and_then(|r| pick_asset(&r))
    {
        return Ok(url);
    }
    // no "latest" when the author publishes pre-releases only (Flight Tracker, 2026-10-17)
    let releases: Value = serde_json::from_slice(&super::share::download(&format!("https://api.github.com/repos/{repo}/releases?per_page=10"), 4 * 1024 * 1024)?).map_err(|e| e.to_string())?;
    releases.as_array().into_iter().flatten().filter(|r| r["draft"].as_bool() != Some(true)).find_map(pick_asset).ok_or_else(|| format!("{} has no plugin file in its GitHub releases", entry.name))
}

/// The catalog (downloaded at most once an hour); an error when offline.
#[tauri::command]
pub async fn sd_catalog() -> Result<Vec<Entry>, String> {
    tauri::async_runtime::spawn_blocking(catalog).await.map_err(|e| e.to_string())?
}

/// Download a catalog plugin and install it like a `.streamDeckPlugin` file. Returns its id.
#[tauri::command]
pub async fn sd_catalog_install(app: tauri::AppHandle, id: String) -> Result<String, String> {
    use tauri::Emitter as _;
    let installed = tauri::async_runtime::spawn_blocking(move || {
        let entry = catalog()?.into_iter().find(|e| e.id == id).ok_or("not in the plugin catalog")?;
        if entry.elgato_only {
            return Err(format!("{} comes only through Elgato's Stream Deck app", entry.name));
        }
        let bytes = super::share::download(&file_url(&entry)?, MAX_PLUGIN)?;
        let id = super::plugins::install_sd(&bytes)?;
        // a plugin made for another system installs but cannot run here: say so and take it out
        if let Some((sd, dir)) = super::plugins::sd_of(&id)
            && let Err(e) = super::sdplugins::runs_here(&sd)
        {
            super::plugins::uninstall(&id);
            let _ = std::fs::remove_dir_all(dir);
            return Err(format!("{}: {e}", entry.name));
        }
        Ok(id)
    })
    .await
    .map_err(|e| e.to_string())??;
    let _ = app.emit("engine://catalog", ());
    Ok(installed)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Live: the real catalog, a plugin's release file and its folder inside (network).
    #[test]
    #[ignore]
    fn catalog_live() {
        let list = catalog().unwrap();
        assert!(list.len() > 100, "{} plugins", list.len());
        for id in ["me.amankhanna.oadiscord", "com.5e.hwinfo-reader"] {
            let entry = list.iter().find(|e| e.id == id).unwrap();
            let url = file_url(entry).unwrap();
            let bytes = super::super::share::download(&url, MAX_PLUGIN).unwrap();
            let root = super::super::sdplugins::install_root(&bytes).unwrap();
            println!("{id}: {url} -> {} KB, {root}", bytes.len() / 1024);
        }
    }

    #[test]
    fn reads_the_catalog_format() {
        let json = br#"{
            "me.amankhanna.oadiscord": {"name": "Discord", "author": "nekename", "repository": "https://github.com/OpenActionPlugins/discord", "description": "Control Discord"},
            "x.nameless": {"name": null, "repository": "https://github.com/a/b"},
            "x.nulls": {"name": "Nulls", "author": null, "description": null, "repository": null, "download_url": null},
            "com.5e.hwinfo-reader": {"name": "HWiNFO reader", "author": "5e", "repository": "https://github.com/5e/x", "description": "", "download_url": "https://github.com/5e/x/raw/refs/heads/main/a.streamDeckPlugin", "flags": {"ai_assisted": true}}
        }"#;
        let list = parse(json).unwrap();
        assert_eq!(list.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["Discord", "HWiNFO reader", "Nulls"]);
        assert!(list[1].ai_assisted && list[1].download_url.is_some());
        assert!(list[2].author.is_empty() && list[2].repository.is_empty());
        assert!(!list[0].ai_assisted && list[0].download_url.is_none());
        assert_eq!(list[0].tried.as_deref(), Some("Windows"), "Discord was tried");
        assert_eq!(list[2].tried, None);
    }

    #[test]
    fn joins_elgato_marketplace() {
        let catalog = br#"{
            "com.fredemmott.audiooutputswitch": {"name": "Audio Output Switcher", "author": "Fred Emmott", "repository": "https://github.com/fredemmott/StreamDeck-AudioSwitcher"},
            "com.x.timer": {"name": "Big Timer", "author": "X Dev", "repository": "https://github.com/x/timer"},
            "com.y.weather": {"name": "Weather", "author": "Someone", "repository": "https://github.com/y/weather"}
        }"#;
        let page = br#"{"count": 5, "results": [
            {"id": "e-1", "name": "Volume Controller", "org_name": "Elgato", "slug": "volume-controller-e-1", "description": "Ready to take **control**?\r\n\r\nMore.", "app_icon_cdn": "https://cdn/v.png", "variants": [{"is_free": true}]},
            {"id": "cf41cd3d-ef80-4122-bef2-7f4fb82fcaac", "name": "Audio Switcher", "org_name": "Fred Emmott", "slug": "audio-switcher"},
            {"id": "737415fa-1957-489d-a62e-1becec068b6e", "name": "Meld Studio", "org_name": "Meld Studio", "slug": "meld"},
            {"id": "e-4", "name": "Big Timer", "org_name": "X Dev", "slug": "big-timer"},
            {"id": "e-5", "name": "Weather", "org_name": "Elgato", "slug": "weather", "variants": [{"is_free": false}]}
        ]}"#;
        let mut list = parse(catalog).unwrap();
        merge_elgato(&mut list, parse_elgato(page).unwrap());
        let row = |id: &str| list.iter().find(|e| e.id == id).unwrap();
        assert_eq!(list[0].id, "elgato:e-1", "Elgato's most downloaded first");
        assert!(list[0].elgato_only && list[0].elgato.as_deref() == Some("https://marketplace.elgato.com/product/volume-controller-e-1"));
        assert_eq!(list[0].description, "Ready to take control?");
        let switcher = row("com.fredemmott.audiooutputswitch");
        assert_eq!((switcher.popular, switcher.elgato_only, switcher.name.as_str()), (Some(2), false, "Audio Output Switcher"), "matched by hand: the catalog's entry");
        let meld = row("co.meldstudio.streamdeck");
        assert_eq!((meld.repository.as_str(), meld.elgato_only), ("https://github.com/MeldStudio/streamdeck", false), "only on GitHub");
        assert_eq!(row("com.x.timer").popular, Some(4), "same name and author");
        assert!(row("elgato:e-5").elgato_only && row("elgato:e-5").description.starts_with("(paid)"), "same name, another author: not the catalog's Weather");
        assert_eq!(row("com.y.weather").popular, None);
        assert_eq!(list.last().unwrap().id, "com.y.weather", "the rest by name after Elgato's");
    }

    /// Live: Elgato Marketplace's list joins the real catalog (network).
    #[test]
    #[ignore]
    fn elgato_live() {
        let list = catalog().unwrap();
        let elgato = list.iter().filter(|e| e.popular.is_some()).count();
        let open = list.iter().filter(|e| e.popular.is_some_and(|p| p <= 100) && !e.elgato_only).count();
        println!("{} plugins, {elgato} from Elgato's list, {open} of its top 100 install here", list.len());
        assert!(elgato >= 250 && open >= 36, "{elgato} / {open}");
    }

    #[test]
    fn finds_the_repository_and_the_plugin_file() {
        assert_eq!(github_repo("https://github.com/OpenActionPlugins/discord").as_deref(), Some("OpenActionPlugins/discord"));
        assert_eq!(github_repo("https://github.com/a/b.git/").as_deref(), Some("a/b"));
        assert_eq!(github_repo("https://gitlab.com/a/b"), None);
        let release = serde_json::json!({"assets": [
            {"name": "source.tar.gz", "browser_download_url": "https://x/s"},
            {"name": "me.x.zip", "browser_download_url": "https://x/z"},
            {"name": "me.x.streamDeckPlugin", "browser_download_url": "https://x/p"}
        ]});
        assert_eq!(pick_asset(&release).as_deref(), Some("https://x/p"));
        assert_eq!(pick_asset(&serde_json::json!({"assets": [{"name": "me.x.zip", "browser_download_url": "https://x/z"}]})).as_deref(), Some("https://x/z"));
        assert_eq!(pick_asset(&serde_json::json!({"assets": []})), None);
    }

    #[test]
    fn picks_the_file_for_this_system() {
        let asset = |n: &str| serde_json::json!({"name": n, "browser_download_url": format!("https://x/{n}")});
        let release = |names: &[&str]| serde_json::json!({"assets": names.iter().map(|n| asset(n)).collect::<Vec<_>>()});
        let script = release(&["com.start-automating.scriptdeck.streamDeckPlugin", "com.start-automating.windowsscriptdeck.streamDeckPlugin"]);
        assert_eq!(pick_asset_for(&script, true).as_deref(), Some("https://x/com.start-automating.windowsscriptdeck.streamDeckPlugin"));
        assert_eq!(pick_asset_for(&script, false).as_deref(), Some("https://x/com.start-automating.scriptdeck.streamDeckPlugin"), "a neutral file before one for Windows only");
        let both = release(&["x-linux.streamDeckPlugin", "x-mac_windows.streamDeckPlugin"]);
        assert_eq!(pick_asset_for(&both, false).as_deref(), Some("https://x/x-mac_windows.streamDeckPlugin"));
        assert_eq!(pick_asset_for(&release(&["macropad.streamDeckPlugin", "x-osx.streamDeckPlugin"]), true).as_deref(), Some("https://x/macropad.streamDeckPlugin"), "macro is not mac");
    }
}
