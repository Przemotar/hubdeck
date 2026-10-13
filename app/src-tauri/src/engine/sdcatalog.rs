//! The catalog of open-source plugins for Stream Deck and OpenAction (the OpenAction Marketplace's
//! list, about 200 plugins), browsed in Settings > Plugins and installed with one click. The list
//! and the plugin files come from GitHub; nothing is fetched until the user opens the catalog.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;

const CATALOG: &str = "https://openactionapi.github.io/plugins/catalogue.json";
/// Plugin icons: `<ICONS><id>.png`.
const ICONS: &str = "https://openactionapi.github.io/plugins/icons/";
const KEEP_FOR: Duration = Duration::from_secs(3600);
const MAX_PLUGIN: u64 = 200 * 1024 * 1024;

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
                id,
            })
        })
        .collect();
    list.sort_by_key(|e| e.name.to_lowercase());
    Ok(list)
}

static CACHE: Mutex<Option<(Instant, Vec<Entry>)>> = Mutex::new(None);

fn catalog() -> Result<Vec<Entry>, String> {
    if let Some((at, list)) = CACHE.lock().unwrap_or_else(|e| e.into_inner()).as_ref()
        && at.elapsed() < KEEP_FOR
    {
        return Ok(list.clone());
    }
    let list = parse(&super::share::download(CATALOG, 4 * 1024 * 1024)?)?;
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

/// The plugin file to download from a release's assets: a `.streamDeckPlugin`, else a `.zip`.
pub fn pick_asset(release: &Value) -> Option<String> {
    let assets = release["assets"].as_array()?;
    let url = |suffix: &str| assets.iter().find(|a| a["name"].as_str().is_some_and(|n| n.to_lowercase().ends_with(suffix))).and_then(|a| a["browser_download_url"].as_str()).map(str::to_string);
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
    let release: Value = serde_json::from_slice(&super::share::download(&format!("https://api.github.com/repos/{repo}/releases/latest"), 1024 * 1024)?).map_err(|e| e.to_string())?;
    pick_asset(&release).ok_or_else(|| format!("{} has no plugin file in its latest release", entry.name))
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
}
