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

/// At start: look once in the background and tell the window; never installs anything.
pub fn check_at_start(app: &AppHandle) {
    let wanted = app.state::<Engine>().lock().store.settings.check_updates;
    if !wanted || !enabled(app) || cfg!(debug_assertions) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        match update_check(app.clone()).await {
            Ok(Some(info)) => {
                log::info!("HubDeck {} is available", info.version);
                let _ = app.emit(AVAILABLE_EVENT, info);
            }
            Ok(None) => {}
            Err(e) => log::info!("update check: {e}"),
        }
    });
}
