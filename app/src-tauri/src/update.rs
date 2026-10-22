//! Updates through the Tauri updater: a signed `latest.json` on GitHub Releases. Under the user's
//! control: HubDeck only looks for a new version (at start when "Check for updates at start" is on,
//! or when asked) and shows its version and notes; it installs only when the user says so.
//!
//! Inactive until the signing key exists: while `plugins.updater.pubkey` in `tauri.conf.json` is
//! empty, no check reaches the network. How the key is made: `docs/KNOWLEDGE.md` > "Updates".

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_updater::UpdaterExt;

use crate::engine::Engine;

/// Event with an [`UpdateInfo`] when the check at start found a new version.
pub const AVAILABLE_EVENT: &str = "update://available";

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    /// The release notes (changelog) of that version.
    pub notes: String,
    pub date: Option<String>,
}

/// The public key the updates are signed with; empty = updates are not set up in this build.
fn pubkey(app: &AppHandle) -> String {
    app.config().plugins.0.get("updater").and_then(|u| u.get("pubkey")).and_then(|k| k.as_str()).unwrap_or("").trim().to_string()
}

/// Whether this build can update itself (a signing key is configured).
pub fn enabled(app: &AppHandle) -> bool {
    !pubkey(app).is_empty()
}

async fn find(app: &AppHandle) -> Result<Option<tauri_plugin_updater::Update>, String> {
    if !enabled(app) {
        return Ok(None);
    }
    app.updater().map_err(|e| e.to_string())?.check().await.map_err(|e| e.to_string())
}

/// Look for a newer version: its version and notes, or None (also when updates are not set up).
#[tauri::command]
pub async fn update_check(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    Ok(find(&app).await?.map(|u| UpdateInfo { version: u.version.clone(), notes: u.body.clone().unwrap_or_default(), date: u.date.map(|d| d.to_string()) }))
}

/// Download, verify and install the newer version, then restart. Only when the user asked.
#[tauri::command]
pub async fn update_install(app: AppHandle) -> Result<(), String> {
    let update = find(&app).await?.ok_or("no newer version")?;
    log::info!("installing HubDeck {}", update.version);
    // the installer ends this HubDeck (on Windows from inside download_and_install): the guard
    // must not start the old one again
    crate::guard::leaving();
    if let Err(e) = update.download_and_install(|_, _| {}, || {}).await {
        if let (Ok(data), Ok(logs)) = (app.path().app_data_dir(), app.path().app_log_dir()) {
            crate::guard::spawn(&data, &logs);
        }
        return Err(e.to_string());
    }
    app.restart()
}

/// Where the releases are listed (public, no sign-in).
const RELEASES: &str = "https://api.github.com/repos/Przemotar/hubdeck/releases?per_page=30";

/// `0.1.2` or `v0.1.2` as numbers to compare; anything after `-` (a pre-release) is not a version
/// to go back to.
fn version_of(tag: &str) -> Option<(u64, u64, u64)> {
    let v = tag.trim().trim_start_matches('v');
    if v.contains('-') {
        return None;
    }
    let mut parts = v.split('.').map(|p| p.parse::<u64>().ok());
    Some((parts.next()??, parts.next()??, parts.next()??))
}

/// The newest published release older than `current` in a GitHub releases list: (version, notes).
pub fn previous_release(releases: &serde_json::Value, current: &str) -> Option<(String, String)> {
    let now = version_of(current)?;
    releases
        .as_array()?
        .iter()
        .filter(|r| r["draft"] != true && r["prerelease"] != true)
        .filter_map(|r| {
            let tag = r["tag_name"].as_str()?;
            let v = version_of(tag)?;
            (v < now).then(|| (v, tag.trim_start_matches('v').to_string(), r["body"].as_str().unwrap_or("").to_string()))
        })
        .max_by_key(|(v, _, _)| *v)
        .map(|(_, version, notes)| (version, notes))
}

/// The version before this one, to go back to after an update that broke something (owner's pick,
/// 2026-10-16: users of other decks roll back by hand after almost every update,
/// `docs/PAIN-POINTS.md` RD1). None when there is none or updates are not set up.
#[tauri::command]
pub async fn update_previous(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    if !enabled(&app) {
        return Ok(None);
    }
    let current = app.package_info().version.to_string();
    let list = tauri::async_runtime::spawn_blocking(|| -> Result<serde_json::Value, String> {
        let agent: ureq::Agent = crate::engine::calls::builder().timeout_global(Some(std::time::Duration::from_secs(15))).build().into();
        agent.get(RELEASES).header("User-Agent", "HubDeck").header("Accept", "application/vnd.github+json").call().map_err(|e| e.to_string())?.body_mut().read_json().map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(previous_release(&list, &current).map(|(version, notes)| UpdateInfo { version, notes, date: None }))
}

/// Go back to `version`: the profiles are backed up first, then that release's signed update is
/// downloaded, its signature checked like any update, installed, and HubDeck restarts.
#[tauri::command]
pub async fn update_rollback(app: AppHandle, version: String) -> Result<(), String> {
    if !enabled(&app) || version_of(&version).is_none() {
        return Err("no such version".into());
    }
    if let Ok(data) = app.path().app_data_dir() {
        crate::engine::backup::create(&data, &format!("before going back to {version}")).map_err(|e| e.to_string())?;
    }
    let url: tauri::Url = format!("https://github.com/Przemotar/hubdeck/releases/download/v{version}/latest.json").parse().map_err(|e| format!("{e}"))?;
    let wanted = version.clone();
    let update = app
        .updater_builder()
        .endpoints(vec![url])
        .map_err(|e| e.to_string())?
        // an older version is the point here: take exactly the one asked for
        .version_comparator(move |_, release| release.version.to_string() == wanted)
        .build()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("version {version} has no update file"))?;
    log::info!("going back to HubDeck {}", update.version);
    crate::guard::leaving();
    if let Err(e) = update.download_and_install(|_, _| {}, || {}).await {
        if let (Ok(data), Ok(logs)) = (app.path().app_data_dir(), app.path().app_log_dir()) {
            crate::guard::spawn(&data, &logs);
        }
        return Err(e.to_string());
    }
    app.restart()
}

/// (updates set up in this build, "Check for updates at start").
#[tauri::command]
pub fn update_settings_get(app: AppHandle, engine: State<'_, Engine>) -> (bool, bool) {
    (enabled(&app), engine.lock().store.settings.check_updates)
}

#[tauri::command]
pub fn update_settings_set(engine: State<'_, Engine>, check_at_start: bool) -> Result<(), String> {
    let mut s = engine.lock();
    s.store.settings.check_updates = check_at_start;
    s.store.save_settings().map_err(|e| e.to_string())
}

/// A newer version found by the last check, kept for the window and the tray: the window is
/// made only when it is opened, so the event at start reached nobody (owner, 2026-10-16: "nie
/// widzę info o update").
static AVAILABLE: std::sync::Mutex<Option<UpdateInfo>> = std::sync::Mutex::new(None);

/// How often HubDeck looks again while it runs: it can stay in the tray for days.
const CHECK_EVERY: std::time::Duration = std::time::Duration::from_secs(6 * 3600);

/// The newer version found, if any (the window asks when it opens).
#[tauri::command]
pub fn update_available() -> Option<UpdateInfo> {
    AVAILABLE.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// The newer version for the tray menu.
pub fn available_version() -> Option<String> {
    AVAILABLE.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map(|i| i.version.clone())
}

/// At start and then every few hours: look in the background, keep what was found, tell the window
/// and the tray; never installs anything.
pub fn check_at_start(app: &AppHandle) {
    let wanted = app.state::<Engine>().lock().store.settings.check_updates;
    if !wanted || !enabled(app) || cfg!(debug_assertions) {
        return;
    }
    let app = app.clone();
    std::thread::Builder::new()
        .name("update check".into())
        .spawn(move || {
            loop {
                if app.state::<Engine>().lock().store.settings.check_updates {
                    match tauri::async_runtime::block_on(update_check(app.clone())) {
                        Ok(Some(info)) => {
                            let new = AVAILABLE.lock().unwrap_or_else(|e| e.into_inner()).replace(info.clone()).as_ref() != Some(&info);
                            if new {
                                log::info!("HubDeck {} is available", info.version);
                                let _ = app.emit(AVAILABLE_EVENT, info);
                                crate::tray::refresh(&app);
                            }
                        }
                        Ok(None) => {}
                        Err(e) => log::info!("update check: {e}"),
                    }
                }
                std::thread::sleep(CHECK_EVERY);
            }
        })
        .ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_previous_release_is_the_newest_older_published_one() {
        // the GitHub releases list (tag_name, draft, prerelease, body)
        let list = serde_json::json!([
            {"tag_name": "v0.2.0", "draft": false, "prerelease": false, "body": "new"},
            {"tag_name": "v0.1.2", "draft": false, "prerelease": false, "body": "fixes"},
            {"tag_name": "v0.1.3-rc.1", "draft": false, "prerelease": true, "body": "rc"},
            {"tag_name": "v0.1.1", "draft": false, "prerelease": false, "body": "old"},
            {"tag_name": "v0.1.9", "draft": true, "prerelease": false, "body": "draft"}
        ]);
        assert_eq!(previous_release(&list, "0.2.0"), Some(("0.1.2".into(), "fixes".into())));
        assert_eq!(previous_release(&list, "0.1.1"), None);
        assert_eq!(previous_release(&serde_json::json!([]), "0.1.0"), None);
        assert_eq!(version_of("v0.10.2"), Some((0, 10, 2)));
        assert!(version_of("0.1.0-rc.6").is_none());
    }
}
