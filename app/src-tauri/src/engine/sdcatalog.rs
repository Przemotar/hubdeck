//! The catalog of free Stream Deck plugins that install here with one click, browsed in the Plugins
//! tab. It is one file built every day by `scripts/plugins/catalog.mjs` and published with the
//! project's site: Elgato Marketplace's most downloaded whose authors publish an open file, the
//! OpenAction Marketplace and GitHub repositories with a plugin file, with their stars, systems,
//! category and dial support. When that file cannot be read, the OpenAction list and Elgato's are
//! joined here as before. Plugins only Elgato's Stream Deck app installs are left out (owner,
//! 2026-10-18: "pokażmy te które są dostępne i tyle"). Nothing is fetched until the tab opens.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The catalog built by `scripts/plugins/catalog.mjs`; `HUBDECK_PLUGIN_CATALOG` (a file) replaces it
/// on a test copy.
const HOSTED: &str = "https://przemotar.github.io/hubdeck/plugins.json";
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

/// What the app and `scripts/plugins/catalog.mjs` share: the catalog plugins checked in HubDeck and
/// on which systems, the Elgato Marketplace plugins whose authors publish an open file, and those
/// that install but cannot run outside Elgato's app (left out, with why).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Shared {
    tried: std::collections::HashMap<String, String>,
    elgato_open: Vec<(String, String, Option<String>)>,
    #[serde(default)]
    wont_run: std::collections::HashMap<String, String>,
}

static SHARED: std::sync::LazyLock<Shared> = std::sync::LazyLock::new(|| serde_json::from_str(include_str!("sdcatalog.json")).expect("sdcatalog.json"));

/// The systems a catalog plugin was checked on in HubDeck ("Windows", "Windows, macOS").
fn tried(id: &str) -> Option<String> {
    SHARED.tried.get(id).cloned()
}

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
    /// Where the catalog has it from: "elgato", "openaction" or "github".
    pub source: String,
    /// Its GitHub repository's stars and last change (YYYY-MM-DD), when known.
    pub stars: Option<u32>,
    pub updated: Option<String>,
    /// The systems its manifest or Elgato names ("Windows", "macOS"); empty = not known.
    pub systems: Vec<String>,
    /// One of the tab's categories ("Audio", "Music", "Streaming"...).
    pub category: Option<String>,
    /// It has actions for the dial and the knobs (Stream Deck's encoders).
    pub dial: bool,
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
                tried: tried(&id),
                elgato: None,
                popular: None,
                source: "openaction".into(),
                stars: None,
                updated: None,
                systems: Vec::new(),
                category: None,
                dial: false,
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
/// (`elgatoOpen` in `sdcatalog.json`) or by the same name and author marks its catalog entry (or
/// comes in with its GitHub repository); the others come only through Elgato's app and stay out.
fn merge_elgato(list: &mut Vec<Entry>, elgato: Vec<ElgatoRaw>) {
    for (rank, e) in elgato.into_iter().enumerate() {
        let popular = Some(rank as u32 + 1);
        let page = e.slug.as_deref().map(|s| format!("{ELGATO_PAGE}{s}"));
        let author = e.org_name.clone().unwrap_or_default();
        let open = SHARED.elgato_open.iter().find(|(id, ..)| *id == e.id);
        let at = match open {
            Some((_, plugin, _)) => list.iter().position(|c| c.id == *plugin),
            None => list.iter().position(|c| c.popular.is_none() && !author.is_empty() && norm(&c.name) == norm(&e.name) && norm(&c.author) == norm(&author)),
        };
        if let Some(at) = at {
            list[at].elgato = page;
            list[at].popular = popular;
            list[at].source = "elgato".into();
            continue;
        }
        let Some((_, plugin, Some(repo))) = open else { continue };
        if e.variants.iter().any(|v| v["is_free"].as_bool() == Some(false)) {
            continue;
        }
        list.push(Entry {
            tried: tried(plugin),
            id: plugin.clone(),
            name: e.name,
            author,
            description: first_paragraph(e.description.as_deref().unwrap_or("")),
            repository: format!("https://github.com/{repo}"),
            icon: e.app_icon_cdn.unwrap_or_default(),
            download_url: None,
            ai_assisted: false,
            elgato: page,
            popular,
            source: "elgato".into(),
            stars: None,
            updated: None,
            systems: Vec::new(),
            category: None,
            dial: false,
        });
    }
    // Elgato's most downloaded first, then the rest by name
    list.sort_by(|a, b| a.popular.unwrap_or(u32::MAX).cmp(&b.popular.unwrap_or(u32::MAX)).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
}

/// The catalog file built by `scripts/plugins/catalog.mjs` (`{ version, plugins: [...] }`).
fn parse_hosted(bytes: &[u8]) -> Result<Vec<Entry>, String> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Raw {
        id: String,
        name: String,
        #[serde(default)]
        author: String,
        #[serde(default)]
        description: String,
        #[serde(default)]
        repository: String,
        #[serde(default)]
        icon: String,
        #[serde(default)]
        download_url: Option<String>,
        #[serde(default)]
        ai_assisted: bool,
        #[serde(default)]
        elgato: Option<String>,
        #[serde(default)]
        popular: Option<u32>,
        #[serde(default)]
        source: String,
        #[serde(default)]
        stars: Option<u32>,
        #[serde(default)]
        updated: Option<String>,
        #[serde(default)]
        systems: Vec<String>,
        #[serde(default)]
        category: Option<String>,
        #[serde(default)]
        dial: bool,
    }
    #[derive(Deserialize)]
    struct File {
        version: u32,
        plugins: Vec<Value>,
    }
    let file: File = serde_json::from_slice(bytes).map_err(|e| format!("the plugin catalog is not readable: {e}"))?;
    if file.version != 1 {
        return Err(format!("the plugin catalog is a newer kind ({})", file.version));
    }
    // one odd entry does not cost the others
    Ok(file
        .plugins
        .into_iter()
        .filter_map(|v| serde_json::from_value::<Raw>(v).ok())
        .filter(|r| !r.id.is_empty() && !r.name.trim().is_empty())
        .map(|r| Entry {
            // the app's own list of tried plugins is newer than a file built yesterday
            tried: tried(&r.id),
            id: r.id,
            name: r.name,
            author: r.author,
            description: r.description,
            repository: r.repository,
            icon: r.icon,
            download_url: r.download_url.filter(|u| !u.is_empty()),
            ai_assisted: r.ai_assisted,
            elgato: r.elgato,
            popular: r.popular,
            source: r.source,
            stars: r.stars,
            updated: r.updated,
            systems: r.systems,
            category: r.category,
            dial: r.dial,
        })
        .collect())
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
    let hosted = match std::env::var_os("HUBDECK_PLUGIN_CATALOG") {
        Some(file) => std::fs::read(file).map_err(|e| e.to_string()),
        None => super::share::download(HOSTED, 8 * 1024 * 1024),
    };
    let list = match hosted.and_then(|b| parse_hosted(&b)) {
        Ok(list) if !list.is_empty() => list,
        failed => {
            if let Err(e) = failed {
                log::warn!("plugin catalog: {e}; the OpenAction and Elgato lists instead");
            }
            let elgato = std::thread::spawn(elgato_top);
            let mut list = parse(&super::share::download(CATALOG, 4 * 1024 * 1024)?)?;
            merge_elgato(&mut list, elgato.join().unwrap_or_default());
            list
        }
    };
    let list = runnable(list);
    *CACHE.lock().unwrap_or_else(|e| e.into_inner()) = Some((Instant::now(), list.clone()));
    Ok(list)
}

/// Only plugins that run here (owner, 2026-10-18: "pokażmy te które są dostępne i tyle"); the
/// hosted catalog leaves them out too, the app's own lists may not.
fn runnable(mut list: Vec<Entry>) -> Vec<Entry> {
    list.retain(|e| !SHARED.wont_run.contains_key(&e.id));
    list
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
        // a repository its author removed answers 404 (YouTube Ticker, 2026-10-18): say that, not
        // "http status: 404"
        let gone = |e: String| {
            if e.contains("404") {
                format!("{}: its author removed or moved its files on GitHub, so it cannot be installed", entry.name)
            } else if e.contains("timeout") {
                format!("{}: GitHub did not answer in time ({e}); try again", entry.name)
            } else {
                e
            }
        };
        let bytes = file_url(&entry).and_then(|url| super::share::download_big(&url, MAX_PLUGIN)).map_err(gone)?;
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

/// What Install would get for one catalog plugin on this system, without installing it: its file
/// downloaded the same way, unpacked into `scratch` and its manifest read. The size and program.
fn check_one(entry: &Entry, scratch: &std::path::Path) -> Result<String, String> {
    let bytes = file_url(entry).and_then(|url| super::share::download_big(&url, MAX_PLUGIN))?;
    std::fs::create_dir_all(scratch).map_err(|e| e.to_string())?;
    let id = super::sdplugins::install(&bytes, scratch)?;
    let dir = scratch.join(format!("{id}.sdPlugin"));
    let v: Value = std::fs::read(dir.join("manifest.json")).ok().and_then(|b| serde_json::from_slice(super::sdplugins::without_bom(&b)).ok()).ok_or("no manifest")?;
    let sd = super::sdplugins::read(&v, &dir)?;
    super::sdplugins::runs_here(&sd)?;
    // Windows starts `name` as `name.exe` (Audio Mixer, Color Picker, 2026-10-23)
    let program = |rel: &str| super::sdplugins::inside(&dir, rel).filter(|p| p.is_file());
    program(&sd.code_path).or_else(|| program(&format!("{}.exe", sd.code_path)).filter(|_| cfg!(windows))).ok_or_else(|| format!("its program {} is not in the file", sd.code_path))?;
    Ok(format!("{} KB, {}{}", bytes.len() / 1024, sd.code_path, if sd.node { " (Node.js)" } else { "" }))
}

/// `hubdeck --catalog-check [N]`: the N most popular catalog plugins (Elgato's order, then stars)
/// checked as above, a few at a time; a line each and the exit code 1 when one fails. The release
/// gate's stage "catalog" on both systems (owner's pick 1a, 2026-10-23: on the Mac VTube Studio
/// would not install, the catalog had been tried on Windows only; it runs beside the soak, as he
/// wants no longer gates).
pub fn check(n: usize) -> i32 {
    let mut list = match catalog() {
        Ok(list) => list,
        Err(e) => {
            println!("FAIL catalog: {e}");
            return 1;
        }
    };
    list.sort_by_key(|e| (e.popular.unwrap_or(u32::MAX), std::cmp::Reverse(e.stars.unwrap_or(0))));
    list.truncate(n);
    let scratch = std::env::temp_dir().join("hubdeck-catalog-check");
    let _ = std::fs::remove_dir_all(&scratch);
    let next = std::sync::atomic::AtomicUsize::new(0);
    let failed = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..4 {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(entry) = list.get(i) else { break };
                    match check_one(entry, &scratch.join(i.to_string())) {
                        Ok(what) => println!("ok   {} ({}): {what}", entry.name, entry.id),
                        Err(e) => {
                            failed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            println!("FAIL {} ({}): {e}", entry.name, entry.id);
                        }
                    }
                }
            });
        }
    });
    let _ = std::fs::remove_dir_all(&scratch);
    let failed = failed.into_inner();
    println!("catalog: {} of {} install and run here", list.len() - failed, list.len());
    i32::from(failed > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A plugin that installs but cannot run here is not offered (Stocks, 2026-10-18).
    #[test]
    fn plugins_that_will_not_run_are_left_out() {
        let hosted = br#"{"version":1,"plugins":[{"id":"com.exension.stocks","name":"Stocks","source":"github","repository":"https://github.com/a/b"},{"id":"com.jk.weather","name":"Weather","source":"github","repository":"https://github.com/c/d"}]}"#;
        let list = runnable(parse_hosted(hosted).unwrap());
        assert_eq!(list.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), ["com.jk.weather"]);
    }

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

    /// Live: the biggest plugin we know (VTube Studio, 59 MB) downloads whole (network; it failed
    /// on the owner's Mac within the old 5 s for the whole download, 2026-10-23).
    /// Live: the release gate's catalog check on this system (network).
    #[test]
    #[ignore]
    fn catalog_check_live() {
        assert_eq!(check(20), 0);
    }

    #[test]
    #[ignore]
    fn big_plugin_downloads() {
        let entry = catalog().unwrap().into_iter().find(|e| e.id == "dev.cazzar.streamdeck.vtubestudio").unwrap();
        let bytes = super::super::share::download_big(&file_url(&entry).unwrap(), MAX_PLUGIN).unwrap();
        assert!(bytes.len() > 50 * 1024 * 1024, "{} bytes", bytes.len());
        assert_eq!(super::super::sdplugins::install_root(&bytes).unwrap(), "dev.cazzar.streamdeck.vtubestudio");
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
        assert!(!list.iter().any(|e| e.name == "Volume Controller"), "only in Elgato's app: left out");
        let switcher = row("com.fredemmott.audiooutputswitch");
        assert_eq!((switcher.popular, switcher.source.as_str(), switcher.name.as_str()), (Some(2), "elgato", "Audio Output Switcher"), "matched by hand: the catalog's entry");
        assert_eq!(list[0].id, "com.fredemmott.audiooutputswitch", "Elgato's most downloaded first");
        let meld = row("co.meldstudio.streamdeck");
        assert_eq!((meld.repository.as_str(), meld.source.as_str()), ("https://github.com/MeldStudio/streamdeck", "elgato"), "only on GitHub");
        assert_eq!(row("com.x.timer").popular, Some(4), "same name and author");
        assert_eq!(list.iter().filter(|e| e.name == "Weather").count(), 1, "same name, another author and paid: not added");
        assert_eq!((row("com.y.weather").popular, row("com.y.weather").source.as_str()), (None, "openaction"));
        assert_eq!(list.last().unwrap().id, "com.y.weather", "the rest by name after Elgato's");
    }

    #[test]
    fn reads_the_hosted_catalog() {
        let json = br#"{"version": 1, "plugins": [
            {"id": "me.amankhanna.oadiscord", "name": "Discord", "author": "nekename", "repository": "https://github.com/OpenActionPlugins/discord", "source": "openaction", "stars": 40, "updated": "2026-09-01", "systems": ["Windows", "macOS"], "category": "Communication", "dial": true, "icon": "https://x/i.png"},
            {"id": "com.x.new", "name": "New", "source": "github", "downloadUrl": "https://github.com/x/new/raw/a.streamDeckPlugin", "aiAssisted": true, "extra": [1]},
            {"id": "", "name": "No id"},
            {"name": "No id at all"},
            {"id": "x.blank", "name": "  "}
        ]}"#;
        let list = parse_hosted(json).unwrap();
        assert_eq!(list.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), ["me.amankhanna.oadiscord", "com.x.new"], "odd entries left out, the rest kept");
        let d = &list[0];
        assert_eq!((d.stars, d.updated.as_deref(), d.systems.len(), d.category.as_deref(), d.dial), (Some(40), Some("2026-09-01"), 2, Some("Communication"), true));
        assert_eq!(d.tried.as_deref(), Some("Windows"), "tried comes from the app's own list");
        assert!(list[1].ai_assisted && list[1].download_url.is_some() && list[1].systems.is_empty() && !list[1].dial);
        assert!(parse_hosted(br#"{"version": 2, "plugins": []}"#).is_err(), "a newer kind is not guessed at");
        assert!(parse_hosted(b"<html>").is_err());
    }

    /// Live: Elgato Marketplace's list joins the real catalog (network).
    #[test]
    #[ignore]
    fn elgato_live() {
        let list = catalog().unwrap();
        let elgato = list.iter().filter(|e| e.popular.is_some()).count();
        let open = list.iter().filter(|e| e.popular.is_some_and(|p| p <= 100)).count();
        println!("{} plugins, {elgato} from Elgato's list, {open} of its top 100 install here", list.len());
        assert!(elgato >= 36 && open >= 30, "{elgato} / {open}");
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
