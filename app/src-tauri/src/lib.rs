#[cfg(windows)]
mod assoc;
mod automation;
mod deck;
mod engine;
mod guard;
mod output;
mod placements;
mod system;
mod tray;
mod update;

use std::collections::BTreeMap;

use deck::{Deck, DeckSnapshot};
use engine::Engine;
use log::LevelFilter;
use placements::{Placement, Placements};
use tauri::{Manager, RunEvent, State};
use tauri_plugin_autostart::ManagerExt as _;

/// Passed by autostart: start with the window hidden, only the tray icon shows.
const HIDDEN_ARG: &str = "--hidden";

/// Current modules and orientation; the window calls this once, then follows `deck://event`.
#[tauri::command]
fn deck_snapshot(deck: State<'_, Deck>) -> DeckSnapshot {
    deck.snapshot()
}

/// Hand keys and dial back to their firmware (`true`) or take them again.
#[tauri::command]
fn deck_pause(app: tauri::AppHandle, deck: State<'_, Deck>, paused: bool) {
    deck.set_paused(paused);
    tray::refresh(&app);
}

/// Set how the base is turned (0, 90, 180, 270 degrees clockwise), or `None` to follow the sensor.
#[tauri::command]
fn base_turn_set(deck: State<'_, Deck>, engine: State<'_, Engine>, turn: Option<i32>) -> Result<(), String> {
    {
        let mut s = engine.lock();
        s.store.settings.base_turn = turn;
        s.store.save_settings().map_err(|e| e.to_string())?;
    }
    deck.set_turn_override(turn);
    Ok(())
}

/// Restart a module that stopped responding (Windows asks for administrator rights).
#[tauri::command]
async fn module_restart(kind: String) -> Result<(), String> {
    let kind = mh_protocol::ModuleKind::from_name(&kind).ok_or_else(|| format!("unknown module kind {kind}"))?;
    tauri::async_runtime::spawn_blocking(move || system::restart_module(kind)).await.map_err(|e| e.to_string())?
}

/// Saved module placements, keyed `"<kind>@<slot>"`.
#[tauri::command]
fn placements_get(placements: State<'_, Placements>) -> BTreeMap<String, Placement> {
    placements.all()
}

/// Save (or with `None`, forget) where a module in a slot sits.
#[tauri::command]
fn placement_set(placements: State<'_, Placements>, engine: State<'_, Engine>, key: String, placement: Option<Placement>) -> Result<(), String> {
    placements.set(key, placement).map_err(|e| e.to_string())?;
    // a module turned on the drawing turns its images on the hardware too
    let _ = engine.sender().send(engine::Msg::DeckChanged);
    Ok(())
}

/// `RUST_LOG` in its simplest forms: `debug`, or `hubdeck_lib=debug,mh_device=trace`.
fn log_levels() -> (LevelFilter, Vec<(String, LevelFilter)>) {
    let mut default = LevelFilter::Info;
    let mut per_module = Vec::new();
    for part in std::env::var("RUST_LOG").unwrap_or_default().split(',').map(str::trim).filter(|p| !p.is_empty()) {
        match part.split_once('=') {
            Some((module, level)) => {
                if let Ok(level) = level.parse() {
                    per_module.push((module.to_string(), level));
                }
            }
            None => {
                if let Ok(level) = part.parse() {
                    default = level;
                }
            }
        }
    }
    (default, per_module)
}

fn log_plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    use tauri_plugin_log::{RotationStrategy, Target, TargetKind, TimezoneStrategy};
    let (default, per_module) = log_levels();
    let mut builder = tauri_plugin_log::Builder::new()
        .clear_targets()
        .targets([
            Target::new(TargetKind::Stdout),
            match automation::get() {
                // a test copy must not write into the real app's log
                Some(a) => Target::new(TargetKind::Folder { path: a.data_dir.join("logs"), file_name: Some("hubdeck".into()) }),
                None => Target::new(TargetKind::LogDir { file_name: Some("hubdeck".into()) }),
            },
        ])
        .timezone_strategy(TimezoneStrategy::UseLocal)
        .rotation_strategy(RotationStrategy::KeepSome(5))
        .max_file_size(2_000_000)
        .level(default);
    for (module, level) in per_module {
        builder = builder.level_for(module, level);
    }
    builder.build()
}

/// A panic aborts the whole app (`panic = "abort"`) and its message went to a stderr nobody
/// reads: the 2026-10-15 night death left nothing in the log. Now it is the log's last line.
fn log_panics() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        log::error!("panic in thread {}: {info}\n{}", thread.name().unwrap_or("unnamed"), std::backtrace::Backtrace::force_capture());
        log::logger().flush();
        default(info);
    }));
}

/// Identifier the app had before it was named HubDeck; its data folder sits next to ours.
const OLD_IDENTIFIER: &str = "dev.masterhub.app";

/// First start after the rename: copy profiles, settings and placements from the old folder.
/// Autostart is offered again, because the old entry starts the old program.
fn migrate_old_data(data_dir: &std::path::Path) {
    let Some(old) = data_dir.parent().map(|p| p.join(OLD_IDENTIFIER)) else { return };
    if data_dir.exists() || !old.is_dir() {
        return;
    }
    let result = (|| -> std::io::Result<()> {
        for name in ["settings.json", "placements.json"] {
            if old.join(name).is_file() {
                std::fs::create_dir_all(data_dir)?;
                std::fs::copy(old.join(name), data_dir.join(name))?;
            }
        }
        if let Ok(entries) = std::fs::read_dir(old.join("profiles")) {
            std::fs::create_dir_all(data_dir.join("profiles"))?;
            for e in entries.flatten().filter(|e| e.path().is_file()) {
                std::fs::copy(e.path(), data_dir.join("profiles").join(e.file_name()))?;
            }
        }
        let settings = data_dir.join("settings.json");
        if let Some(mut value) = std::fs::read(&settings).ok().and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok()) {
            if let Some(o) = value.as_object_mut() {
                o.remove("autostartOffered");
            }
            engine::profile::write_atomic(&settings, &serde_json::to_vec_pretty(&value)?)?;
        }
        Ok(())
    })();
    match result {
        Ok(()) => log::info!("copied the app data from {}", old.display()),
        Err(e) => log::warn!("could not copy the app data from {}: {e}", old.display()),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// `hubdeck --claude-hook`: called by Claude Code hooks; reports the session state and exits.
pub fn claude_hook() {
    engine::agents::run_hook();
}

/// `hubdeck --mcp`: an MCP server on stdin/stdout for the user's AI assistant (talks to the
/// running app over its local HTTP API); runs until the client closes stdin.
pub fn mcp() {
    engine::mcp::run();
}

/// `hubdeck --mcp-deck`: the deck tools for an assistant (notify, status, questions on the keys).
pub fn mcp_deck() {
    engine::mcp::run_deck();
}

/// `hubdeck --mcp-assistant`: the tools of HubDeck's own deck assistant (drafts only).
pub fn mcp_assistant() {
    engine::mcp::run_assistant();
}

/// `hubdeck --guard <data dir> <log dir>`: hands the deck back and restarts HubDeck if it ends
/// without quitting (`guard.rs`).
pub fn guard(args: &[String]) {
    guard::run(args);
}

/// One HubDeck on the real deck. The single-instance plugin lets a second start run on when it
/// finds the first one's lock but not its message window (it only exits after handing the start
/// over to that window), and on 2026-10-14 four copies ran on the deck at once, each with its own
/// engine (flickering screensaver, the owner's "rozpierdol"). This lock is a file in the user's
/// temp folder, held for the life of the process (the system lets it go when the process ends,
/// also when it is killed); a copy that quits for a restart has 10 s to let it go.
fn take_the_deck_lock() {
    static HELD: std::sync::OnceLock<std::fs::File> = std::sync::OnceLock::new();
    let path = std::env::temp_dir().join("hubdeck-deck.lock");
    let file = match std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(&path) {
        Ok(f) => f,
        Err(e) => {
            log::warn!("deck lock {}: {e}; going on without it", path.display());
            return;
        }
    };
    let waited = std::time::Instant::now();
    loop {
        match file.try_lock() {
            Ok(()) => break,
            Err(std::fs::TryLockError::WouldBlock) if waited.elapsed() < std::time::Duration::from_secs(10) => std::thread::sleep(std::time::Duration::from_millis(250)),
            Err(std::fs::TryLockError::WouldBlock) => {
                log::warn!("another HubDeck has the deck (lock {}); this one quits", path.display());
                std::process::exit(0);
            }
            Err(std::fs::TryLockError::Error(e)) => {
                log::warn!("deck lock {}: {e}; going on without it", path.display());
                return;
            }
        }
    }
    if waited.elapsed() > std::time::Duration::from_millis(300) {
        log::info!("deck lock taken after {:?} (the copy before was quitting)", waited.elapsed());
    }
    let _ = HELD.set(file);
}

/// Said in the log at every start (a user's log tells whether HubDeck starts with the system).
/// When it is on, the entry is written again: an entry left by a copy of HubDeck in another
/// folder (a build of one's own, an older install) starts that program or nothing, and the tray
/// still shows it ticked (issue #10). Off stays off, also when turned off in the system's list.
fn autostart_check(app: &tauri::AppHandle) {
    let manager = app.autolaunch();
    match manager.is_enabled() {
        Ok(true) => match manager.enable() {
            Ok(()) => log::info!("autostart: on ({})", std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default()),
            Err(e) => log::warn!("autostart: on, the entry not renewed: {e}"),
        },
        Ok(false) => log::info!("autostart: off"),
        Err(e) => log::warn!("autostart: {e}"),
    }
}

/// `hubdeck --catalog-check [N]`: the N most popular catalog plugins downloaded, unpacked and read
/// as Install would, nothing installed (the release gate's stage "catalog"); the exit code.
pub fn catalog_check(args: &[String]) -> i32 {
    engine::sdcatalog::check(args.first().and_then(|n| n.parse().ok()).unwrap_or(20))
}

/// `hubdeck cli ...`: one command against the running app; the exit code for the shell.
pub fn cli(args: &[String]) -> i32 {
    engine::cli::run(args)
}

pub fn run() {
    let testing = automation::get();
    if let Some(a) = testing {
        automation::prepare_webview(a);
    }
    let mut builder = tauri::Builder::default();
    if testing.is_none() {
        // first: a second start only shows the window of the running one
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            tray::show_main(app);
            // a double click on a plugin file while HubDeck runs
            engine::sdlocal::opened(app, engine::sdlocal::files_in(&args));
        }));
    }
    let app = builder
        .plugin(log_plugin())
        // a test copy does not open links clicked in the page (they would take the focus)
        .plugin(tauri_plugin_opener::Builder::new().open_js_links_on_click(testing.is_none()).build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::Builder::new().arg(HIDDEN_ARG).build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(Deck::default())
        .setup(move |app| {
            let _ = engine::exec::APP.set(app.handle().clone());
            log::info!("HubDeck {} starting", app.package_info().version);
            log_panics();
            if testing.is_none() || automation::real_deck() {
                take_the_deck_lock();
            }
            let data_dir = match testing {
                Some(a) => a.data_dir.clone(),
                None => app.path().app_data_dir()?,
            };
            if testing.is_none() {
                migrate_old_data(&data_dir);
                if let Ok(log_dir) = app.path().app_log_dir() {
                    guard::spawn(&data_dir, &log_dir);
                }
            }
            app.manage(Placements::load(data_dir.join("placements.json")));
            let deck = app.state::<Deck>().inner().clone();
            let engine = Engine::start(app.handle().clone(), deck.clone(), &data_dir);
            deck.set_engine(engine.sender());
            let (turn, offer_autostart) = {
                let s = engine.lock();
                (s.store.settings.base_turn, !s.store.settings.autostart_offered)
            };
            deck.set_turn_override(turn);
            // an installed build starts with the system once by default; the tray turns it off
            if offer_autostart && !cfg!(debug_assertions) && testing.is_none() {
                if let Err(e) = app.autolaunch().enable() {
                    log::warn!("autostart: {e}");
                }
                let mut s = engine.lock();
                s.store.settings.autostart_offered = true;
                let _ = s.store.save_settings();
            }
            app.manage(engine);
            // after manage: the registry write took long enough for another thread to ask for the
            // engine before it was managed (a panic at start, 2026-10-25 11:23); off the start path
            if !offer_autostart && !cfg!(debug_assertions) && testing.is_none() {
                let handle = app.handle().clone();
                std::thread::spawn(move || autostart_check(&handle));
            }
            // the assistant routes of the local HTTP API (`hubdeck --mcp` talks to them)
            engine::api::install(app.handle().clone());
            engine::tour::recover(app.handle());
            engine::auto::at_start(app.handle());
            if automation::real_deck() {
                // the film copy: its own data, the real deck, the window on screen (or off it, to
                // check the window against the hardware without taking the owner's screen)
                deck.spawn(app.handle().clone());
                if std::env::var_os("HUBDECK_OFFSCREEN").is_some() {
                    automation::open_window(app.handle());
                } else {
                    tray::show_main(app.handle());
                }
                return Ok(());
            }
            if testing.is_some() {
                // a test copy: virtual modules, no tray, no update check, the window off-screen
                deck.spawn_virtual(app.handle().clone(), automation::virtual_modules());
                // before the window, which takes them when it starts
                engine::sdlocal::opened(app.handle(), engine::sdlocal::files_in(&std::env::args().collect::<Vec<_>>()));
                automation::open_window(app.handle());
                return Ok(());
            }
            deck.spawn(app.handle().clone());
            system::watch_power(deck);
            tray::create(app.handle())?;
            update::check_at_start(app.handle());
            // started by a double click on a plugin file
            engine::sdlocal::opened(app.handle(), engine::sdlocal::files_in(&std::env::args().collect::<Vec<_>>()));
            #[cfg(windows)]
            assoc::register();
            if !std::env::args().any(|a| a == HIDDEN_ARG) {
                tray::show_main(app.handle());
            }
            Ok(())
        })
        // closing the window hides it and frees its web view 10 min later (tray::hide_main); the
        // deck keeps working, the tray opens it again, Quit is in the tray menu
        .invoke_handler(tauri::generate_handler![
            engine::commands::window_log,
            engine::commands::names_list,
            deck_snapshot,
            deck_pause,
            base_turn_set,
            module_restart,
            placements_get,
            placement_set,
            output::draw_jpeg,
            output::fill_screen,
            output::set_leds,
            engine::commands::engine_state,
            engine::commands::engine_redraw,
            engine::commands::timer_stop,
            engine::commands::render_compare,
            engine::commands::calibration_start,
            engine::commands::calibration_set,
            engine::commands::calibration_stop,
            engine::commands::profile_get,
            engine::commands::profile_save,
            engine::commands::profile_create,
            engine::commands::profile_delete,
            engine::commands::profile_activate,
            engine::commands::settings_set,
            engine::commands::binding_set,
            engine::commands::bindings_set,
            engine::commands::actions_list,
            engine::commands::leds_get,
            engine::commands::claude_status,
            engine::commands::claude_approving,
            engine::commands::claude_connect,
            engine::mcp::mcp_info,
            engine::mcp::mcp_connect_desktop,
            engine::mcp::mcp_connect_code,
            engine::live::live_secret_set,
            engine::live::live_secret_saved,
            engine::live::weather_search,
            engine::commands::icons_list,
            engine::commands::preview_svg,
            engine::commands::image_import,
            engine::commands::app_icon,
            engine::commands::presets_list,
            engine::commands::preset_add,
            engine::commands::onboarding_done,
            engine::commands::integrations_get,
            engine::commands::integrations_set,
            engine::commands::homeassistant_set,
            engine::commands::streamlabs_set,
            engine::share::gallery_list,
            engine::sdcatalog::sd_catalog,
            engine::sdcatalog::sd_catalog_install,
            engine::sdlocal::sd_stream_deck,
            engine::sdlocal::sd_import,
            engine::sdlocal::sd_peek,
            engine::sdlocal::sd_opened,
            engine::sdlocal::sd_drop_file,
            engine::tour::tour_suggest,
            engine::nudge::tour_done,
            engine::tour::tour_start,
            engine::tour::tour_set,
            engine::tour::tour_sound,
            engine::tour::tour_saver,
            engine::tour::tour_highlight,
            engine::tour::tour_finish,
            engine::tour::tour_exit,
            engine::starter::profile_for_app,
            engine::auto::auto_seen,
            guard::crash_report,
            engine::mirror_listen,
            engine::mirror_unlisten,
            engine::saver_shown,
            engine::saver_end,
            engine::commands::page_add_way,
            guard::crash_seen,
            engine::commands::profile_switch_in,
            engine::states::apps_seen,
            engine::suggest::suggestions,
            engine::gallery::dial_gallery,
            engine::commands::language_set,
            engine::commands::call_switch_set,
            tray::autostart_get,
            tray::autostart_set,
            engine::commands::reduced_motion_set,
            engine::mcp::ai_prompt,
            engine::mcp::ai_build,
            engine::suggest::profile_fill,
            engine::auto::auto_set,
            engine::iconpacks::iconpacks_list,
            engine::iconpacks::iconpack_icons,
            engine::iconpacks::iconpack_import,
            engine::iconpacks::iconpack_use,
            engine::iconpacks::iconpack_remove,
            engine::share::gallery_add,
            tray::diagnostics_save,
            tray::diagnostics_brief,
            update::update_check,
            update::update_available,
            update::update_install,
            update::update_previous,
            update::update_rollback,
            update::update_settings_get,
            update::update_settings_set,
            engine::commands::spotify_status,
            engine::commands::music_test,
            engine::commands::spotify_connect,
            engine::commands::spotify_disconnect,
            engine::commands::profile_pin,
            engine::commands::profile_check,
            engine::commands::installed_list,
            engine::commands::installed_picture,
            engine::commands::lights_get,
            engine::commands::lights_set,
            engine::commands::apps_list,
            engine::commands::macro_record_start,
            engine::commands::macro_record_stop,
            engine::commands::twitch_status,
            engine::commands::twitch_connect,
            engine::commands::twitch_disconnect,
            engine::commands::midi_ports,
            engine::commands::audio_devices,
            engine::commands::plugins_list,
            engine::commands::plugin_uninstall,
            engine::commands::plugin_uses,
            engine::commands::plugin_stop,
            engine::commands::plugin_start,
            engine::commands::sd_install,
            engine::builder::builder_options,
            engine::builder::builder_draft,
            engine::commands::accessibility_ok,
            engine::commands::accessibility_ask,
            engine::live_looks,
            engine::commands::sd_inspector,
            engine::commands::games_list,
            engine::commands::backups_list,
            engine::commands::profiles_dir_get,
            engine::commands::profiles_dir_set,
            engine::commands::screens_get,
            engine::commands::screens_set,
            engine::commands::deck_fonts,
            engine::commands::image_thumb,
            engine::commands::pet_preview,
            engine::languages::languages_list,
            engine::languages::language_file,
            engine::languages::language_template,
            engine::languages::languages_open_folder,
            engine::commands::backup_now,
            engine::commands::backup_restore,
            engine::commands::games_play,
            engine::commands::wheel_get,
            engine::commands::wheel_set,
            engine::commands::plugins_open_folder,
            engine::commands::open_link,
            engine::commands::app_icons,
            engine::assistant::assistant_scan,
            engine::assistant::assistant_info,
            engine::assistant::sketches_make,
            engine::assistant::compare_drafts,
            engine::assistant::assistant_set,
            engine::assistant::assistant_send,
            engine::assistant::assistant_stop,
            engine::assistant::assistant_reset,
            engine::assistant::assistant_tour,
            engine::states::apps_state,
            engine::commands::presses_counting,
            engine::commands::presses_count_set,
            engine::states::app_connect,
            engine::states::app_remove,
            engine::states::app_contract_prompt,
            engine::assistant::assistant_permit,
            engine::assistant::assistant_power_set,
            engine::assistant::assistant_sketch_app,
            engine::nudge::nudge_get,
            engine::nudge::nudge_act,
            engine::nudge::nudge_enable,
            engine::nudge::nudge_enabled,
            engine::taste::taste_get,
            engine::taste::taste_set,
            engine::taste::taste_forget,
            engine::assistant::profile_try,
            engine::assistant::draft_settle,
            engine::commands::page_set,
            engine::commands::page_create,
            engine::commands::page_rename,
            engine::commands::page_delete,
            engine::commands::profile_export,
            engine::commands::profile_import,
            engine::commands::profile_adapt
        ])
        .build(tauri::generate_context!())
        .expect("error while building the app");

    app.run(|handle, event| {
        // the main window's X hides it (tray::hide_main); other windows (plugins) close as usual
        if let RunEvent::WindowEvent { label, event: tauri::WindowEvent::CloseRequested { api, .. }, .. } = &event
            && label == "main"
        {
            api.prevent_close();
            tray::hide_main(handle);
        }
        // the last window closing is not quitting: only Quit in the tray (app.exit) ends HubDeck
        if let RunEvent::ExitRequested { api, code: None, .. } = &event {
            api.prevent_exit();
        }
        // macOS: a click on the Dock icon brings the window back after it was closed with its X
        #[cfg(target_os = "macos")]
        if let RunEvent::Reopen { .. } = &event {
            tray::show_main(handle);
        }
        // macOS: a plugin file opened with HubDeck (double click, Open with, dropped on the Dock icon)
        #[cfg(target_os = "macos")]
        if let RunEvent::Opened { urls } = &event {
            let files: Vec<String> = urls.iter().filter_map(|u| u.to_file_path().ok()).map(|p| p.to_string_lossy().into_owned()).collect();
            engine::sdlocal::opened(handle, engine::sdlocal::files_in(&files));
            tray::show_main(handle);
        }
        if let RunEvent::Exit = event {
            // the deck thread drops the hub, which hands keys and dial back to hardware mode
            handle.state::<Deck>().stop();
            deck::mark_released(handle);
            engine::plugins::stop();
            engine::usage::flush();
            engine::presses::flush();
            // macOS: apps whose volume HubDeck set play straight again
            #[cfg(target_os = "macos")]
            engine::volume::release();
            guard::leaving();
            log::info!("HubDeck stopped");
        }
    });
}
