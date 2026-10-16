//! Tray icon: the app keeps running with the window closed. The menu opens the window, switches
//! profiles, pauses the deck, turns autostart on or off, opens the log folder and quits.

use std::sync::Mutex;

use tauri::menu::{CheckMenuItemBuilder, MenuBuilder, SubmenuBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_opener::OpenerExt as _;

use crate::deck::Deck;
use crate::engine::Engine;

const TRAY_ID: &str = "main";

/// What the menu shows; the menu is rebuilt only when this changes.
static SHOWN: Mutex<String> = Mutex::new(String::new());

#[cfg(target_os = "macos")]
const AUTOSTART_TEXT: &str = "Open at login";
#[cfg(not(target_os = "macos"))]
const AUTOSTART_TEXT: &str = "Start with Windows";

/// Show the settings window, creating it if it was closed (a closed window frees its web view,
/// ~300 MB, so HubDeck in the tray stays small).
pub fn show_main(app: &AppHandle) {
    let window = match app.get_webview_window("main") {
        Some(w) => Some(w),
        None => app.config().app.windows.iter().find(|w| w.label == "main").and_then(|cfg| match tauri::WebviewWindowBuilder::from_config(app, cfg).and_then(|b| b.build()) {
            Ok(w) => Some(w),
            Err(e) => {
                log::warn!("cannot open the window: {e}");
                None
            }
        }),
    };
    if let Some(w) = window {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let mut builder =
        TrayIconBuilder::with_id(TRAY_ID).tooltip("HubDeck").show_menu_on_left_click(false).on_menu_event(|app, event| on_menu(app, event.id().as_ref())).on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    refresh(app);
    Ok(())
}

fn on_menu(app: &AppHandle, id: &str) {
    match id {
        "open" => show_main(app),
        "games" => {
            if let Some(engine) = app.try_state::<Engine>() {
                engine.lock().games_request = Some(true);
            }
        }
        "pause" => {
            let deck = app.state::<Deck>();
            deck.set_paused(!deck.is_paused());
        }
        "autostart" => {
            let manager = app.autolaunch();
            let result = if manager.is_enabled().unwrap_or(false) { manager.disable() } else { manager.enable() };
            if let Err(e) = result {
                log::warn!("autostart: {e}");
            }
        }
        "diagnostics" => {
            if let Err(e) = save_diagnostics(app) {
                log::warn!("diagnostics: {e}");
            }
        }
        "logs" => {
            if let Ok(dir) = app.path().app_log_dir() {
                let _ = std::fs::create_dir_all(&dir);
                if let Err(e) = app.opener().open_path(dir.to_string_lossy(), None::<&str>) {
                    log::warn!("open log folder: {e}");
                }
            }
        }
        "pin" => {
            if let Some(engine) = app.try_state::<Engine>() {
                let pinned = engine.lock().store.settings.pinned;
                if let Err(e) = engine.set_pinned(!pinned) {
                    log::warn!("tray: {e}");
                }
            }
        }
        "quit" => app.exit(0),
        other => {
            if let Some(profile) = other.strip_prefix("profile:")
                && let Some(engine) = app.try_state::<Engine>()
                && let Err(e) = engine.activate(profile)
            {
                log::warn!("tray: {e}");
            }
        }
    }
    refresh(app);
}

/// "Save diagnostics" from the window (Settings > Help and feedback), the same as the tray item.
#[tauri::command]
pub fn diagnostics_save(app: AppHandle) -> Result<(), String> {
    save_diagnostics(&app)
}

/// The short diagnostics for the bug form (Report a problem).
#[tauri::command]
pub fn diagnostics_brief(app: AppHandle) -> Result<String, String> {
    let dir = app.path().app_log_dir().map_err(|e| e.to_string())?;
    Ok(crate::engine::diagnostics::brief(&dir.join("hubdeck.log"), &app.package_info().version.to_string(), &app.state::<Deck>().snapshot()))
}

/// Write the diagnostics file into the log folder and show it.
fn save_diagnostics(app: &AppHandle) -> Result<(), String> {
    let dir = app.path().app_log_dir().map_err(|e| e.to_string())?;
    let engine = app.try_state::<Engine>().ok_or("engine not running")?;
    let settings = {
        let s = engine.lock();
        // addresses, paths, keys and names of the user's: only whether they are set
        crate::engine::diagnostics::settings_view(&serde_json::to_value(&s.store.settings).unwrap_or_default())
    };
    let deck = app.state::<Deck>().snapshot();
    let path =
        crate::engine::diagnostics::write(&dir, &dir.join("hubdeck.log"), &app.package_info().version.to_string(), &deck, &settings, &crate::engine::plugins::list()).map_err(|e| e.to_string())?;
    if crate::automation::blocks_opening(&path.to_string_lossy()) {
        return Ok(());
    }
    app.opener().reveal_item_in_dir(&path).map_err(|e| e.to_string())
}

/// Rebuild the menu and tooltip if the profiles, the active one, pause or autostart changed.
pub fn refresh(app: &AppHandle) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else { return };
    let Some(engine) = app.try_state::<Engine>() else { return };
    let state = engine.lock().state();
    let paused = app.state::<Deck>().is_paused();
    let autostart = app.autolaunch().is_enabled().unwrap_or(false);
    let profiles: Vec<String> = state.profiles.iter().map(|p| format!("{}={}", p.id, p.name)).collect();
    let signature = format!("{profiles:?}|{:?}|{paused}|{autostart}|{}", state.active, state.pinned);
    {
        let mut shown = SHOWN.lock().unwrap_or_else(|e| e.into_inner());
        if *shown == signature {
            return;
        }
        *shown = signature;
    }
    let active_name = state.profiles.iter().find(|p| Some(&p.id) == state.active.as_ref()).map(|p| p.name.clone()).unwrap_or_default();
    let build = || -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
        let mut profiles = SubmenuBuilder::new(app, "Profile");
        for p in &state.profiles {
            let item = CheckMenuItemBuilder::with_id(format!("profile:{}", p.id), &p.name).checked(state.active.as_deref() == Some(p.id.as_str())).build(app)?;
            profiles = profiles.item(&item);
        }
        let pause = CheckMenuItemBuilder::with_id("pause", "Pause deck").checked(paused).build(app)?;
        let pin = CheckMenuItemBuilder::with_id("pin", "Pin this profile (no automatic switching)").checked(state.pinned).build(app)?;
        let autostart = CheckMenuItemBuilder::with_id("autostart", AUTOSTART_TEXT).checked(autostart).build(app)?;
        let mut menu = MenuBuilder::new(app).text("open", "Open HubDeck");
        if crate::engine::games::GAMES_ENABLED {
            menu = menu.text("games", "Play games on the deck");
        }
        menu.separator()
            .item(&profiles.build()?)
            .item(&pin)
            .item(&pause)
            .separator()
            .item(&autostart)
            .text("logs", "Open log folder")
            .text("diagnostics", "Save diagnostics for a bug report")
            .separator()
            .text("quit", "Quit")
            .build()
    };
    match build() {
        Ok(menu) => {
            let _ = tray.set_menu(Some(menu));
        }
        Err(e) => log::warn!("tray menu: {e}"),
    }
    let tooltip = match (paused, active_name.is_empty()) {
        (true, _) => "HubDeck · paused".to_string(),
        (false, true) => "HubDeck".to_string(),
        (false, false) => format!("HubDeck · {active_name}"),
    };
    let _ = tray.set_tooltip(Some(tooltip));
}
