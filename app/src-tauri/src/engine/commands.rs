//! Tauri commands for the settings window: profiles, bindings, the action catalog, icons,
//! previews, export and import.

use serde::Serialize;
use tauri::State;

use super::actions::{self, ActionDef};
use super::profile::{Binding, Look, PROFILE_VERSION, Page, Profile};
use super::{Engine, EngineState, hotkey, icons, render};

type Res<T> = Result<T, String>;

fn io<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

#[tauri::command]
pub fn engine_state(engine: State<'_, Engine>) -> EngineState {
    engine.lock().state()
}

/// Draw every key and the dial again, e.g. after the identify marks of arrange mode.
#[tauri::command]
pub fn engine_redraw(engine: State<'_, Engine>) {
    let _ = engine.sender().send(super::Msg::DeckChanged);
}

/// Draw the keys with one of the comparison variants (`None` = normal drawing). A diagnostic for
/// dev builds; an installed build always draws variant B.
#[tauri::command]
pub fn render_compare(engine: State<'_, Engine>, variant: Option<super::CompareVariant>) {
    if !cfg!(debug_assertions) {
        return;
    }
    engine.lock().compare = variant;
    let _ = engine.sender().send(super::Msg::DeckChanged);
}

/// Start calibrating the key panel from the saved geometry.
#[tauri::command]
pub fn calibration_start(engine: State<'_, Engine>) -> super::Calibration {
    let cal = {
        let mut s = engine.lock();
        let cal = super::Calibration { geometry: s.store.settings.panel, selected: 0 };
        s.calibration = Some(cal.clone());
        cal
    };
    engine.changed();
    cal
}

/// Change the draft from the window's buttons.
#[tauri::command]
pub fn calibration_set(engine: State<'_, Engine>, geometry: super::render::PanelGeometry, selected: usize) -> Res<()> {
    {
        let mut s = engine.lock();
        let cal = s.calibration.as_mut().ok_or("not calibrating")?;
        cal.geometry = geometry;
        cal.selected = selected % super::render::GEOMETRY_PARAMS.len();
    }
    let payload = engine.lock().calibration.clone();
    let _ = tauri::Emitter::emit(&engine.app, super::CALIBRATION_EVENT, payload);
    engine.changed();
    Ok(())
}

/// Finish calibrating; `save` keeps the draft, otherwise the saved geometry stays.
#[tauri::command]
pub fn calibration_stop(engine: State<'_, Engine>, save: bool) -> Res<()> {
    {
        let mut s = engine.lock();
        if let Some(cal) = s.calibration.take()
            && save
        {
            s.store.settings.panel = cal.geometry;
            s.store.save_settings().map_err(io)?;
        }
    }
    let _ = tauri::Emitter::emit(&engine.app, super::CALIBRATION_EVENT, Option::<super::Calibration>::None);
    let _ = engine.sender().send(super::Msg::DeckChanged);
    engine.changed();
    Ok(())
}

#[tauri::command]
pub fn profile_get(engine: State<'_, Engine>, id: String) -> Res<Profile> {
    engine.lock().store.profiles.get(&id).cloned().ok_or_else(|| format!("no profile {id}"))
}

/// Replace a whole profile (name, apps, bindings).
#[tauri::command]
pub fn profile_save(engine: State<'_, Engine>, profile: Profile) -> Res<()> {
    if profile.name.trim().is_empty() {
        return Err("a profile needs a name".into());
    }
    engine.lock().store.save_profile(profile).map_err(io)?;
    engine.changed();
    Ok(())
}

/// New empty profile, or a copy of `from`.
#[tauri::command]
pub fn profile_create(engine: State<'_, Engine>, name: String, from: Option<String>) -> Res<Profile> {
    let mut profile = Profile::new(name.trim());
    if let Some(src) = from.and_then(|id| engine.lock().store.profiles.get(&id).cloned()) {
        profile.controls = src.controls;
        profile.pages = src.pages;
    }
    engine.lock().store.save_profile(profile.clone()).map_err(io)?;
    engine.changed();
    Ok(profile)
}

#[tauri::command]
pub fn profile_delete(engine: State<'_, Engine>, id: String) -> Res<()> {
    super::backup::create(engine.data_dir(), "before delete").map_err(io)?;
    {
        let mut s = engine.lock();
        if s.store.profiles.len() <= 1 {
            return Err("keep at least one profile".into());
        }
        s.store.delete_profile(&id).map_err(io)?;
        if s.active.as_deref() == Some(id.as_str()) {
            s.active = s.store.fallback_id();
        }
    }
    engine.changed();
    Ok(())
}

/// The user picks a profile: it shows now and becomes home (app profiles give back to it).
#[tauri::command]
pub fn profile_activate(engine: State<'_, Engine>, id: String) -> Res<()> {
    engine.activate(&id)
}

/// Apps to link to a profile: open now, then seen in front recently, with their icons.
#[tauri::command]
pub async fn apps_list() -> Vec<super::apps::AppInfo> {
    tauri::async_runtime::spawn_blocking(super::apps::list).await.unwrap_or_default()
}

/// The deck's lights: style, brightness, reactions.
#[tauri::command]
pub fn lights_get(engine: State<'_, Engine>) -> super::profile::LightSettings {
    engine.lock().store.settings.lights.clone()
}

#[tauri::command]
pub fn lights_set(engine: State<'_, Engine>, lights: super::profile::LightSettings) -> Res<()> {
    {
        let mut s = engine.lock();
        s.store.settings.lights = super::profile::LightSettings { brightness: lights.brightness.min(100), ..lights };
        s.store.save_settings().map_err(io)?;
    }
    engine.changed();
    Ok(())
}

/// Is counting presses on (the Assistant tab's switch)?
#[tauri::command]
pub fn presses_counting(engine: State<'_, Engine>) -> bool {
    engine.lock().store.settings.count_presses
}

/// Count presses or not; turning it off forgets what was counted.
#[tauri::command]
pub fn presses_count_set(engine: State<'_, Engine>, on: bool) -> Res<()> {
    {
        let mut s = engine.lock();
        s.store.settings.count_presses = on;
        s.store.save_settings().map_err(io)?;
    }
    if !on {
        super::presses::forget();
    }
    Ok(())
}

/// Controls of a profile that point at something this computer does not have (after an import).
#[tauri::command]
pub async fn profile_check(engine: State<'_, Engine>, id: String) -> Res<Vec<super::check::Issue>> {
    let profile = engine.lock().store.profiles.get(&id).cloned().ok_or_else(|| format!("no profile {id}"))?;
    tauri::async_runtime::spawn_blocking(move || {
        let (audio, midi) = super::check::needs(&profile);
        let audio_devices = audio.then(|| {
            super::volume::init_thread();
            let mut names: Vec<String> = super::audiodev::list(false).unwrap_or_default().into_iter().map(|d| d.name).collect();
            names.extend(super::audiodev::list(true).unwrap_or_default().into_iter().map(|d| d.name));
            names
        });
        let env = super::check::Env {
            file_exists: &|p: &str| std::path::Path::new(p).exists(),
            image_exists: &super::images::exists,
            audio_devices,
            midi_ports: midi.then(super::midi::ports),
            obs: super::obs::known(),
            ha_missing: super::homeassistant::missing(),
            name_opens: &super::check::name_opens,
        };
        super::check::issues(&profile, &env)
    })
    .await
    .map_err(io)
}

/// Apps and games installed on this PC (cached; `refresh` scans again), with small icons.
#[tauri::command]
pub async fn installed_list(refresh: bool) -> super::installed::Installed {
    tauri::async_runtime::spawn_blocking(move || super::installed::list(refresh)).await.unwrap_or_default()
}

/// A key picture for an installed app or game: its cover from the launcher's cache on this PC,
/// else the program's icon. Returns the picture's name for the look.
#[tauri::command]
pub async fn installed_picture(item: super::installed::Item) -> Res<String> {
    tauri::async_runtime::spawn_blocking(move || super::installed::picture(&item)).await.map_err(io)?
}

/// "Pin this profile": no automatic switching until unpinned.
#[tauri::command]
pub fn profile_pin(engine: State<'_, Engine>, pinned: bool) -> Res<()> {
    engine.set_pinned(pinned)
}

#[tauri::command]
pub fn settings_set(engine: State<'_, Engine>, auto_switch: Option<bool>, fallback: Option<String>) -> Res<()> {
    {
        let mut s = engine.lock();
        if let Some(a) = auto_switch {
            s.store.settings.auto_switch = a;
        }
        if let Some(f) = fallback {
            if !s.store.profiles.contains_key(&f) {
                return Err(format!("no profile {f}"));
            }
            s.store.settings.fallback = Some(f);
        }
        s.store.save_settings().map_err(io)?;
    }
    engine.changed();
    Ok(())
}

/// Set (or with `None`, clear) what one control does in a profile.
#[tauri::command]
pub fn binding_set(engine: State<'_, Engine>, profile: String, control: String, binding: Option<Binding>) -> Res<()> {
    if let Some(b) = &binding {
        actions::find(&b.action).ok_or_else(|| format!("unknown action {}", b.action))?;
    }
    {
        let mut s = engine.lock();
        let mut p = s.store.profiles.get(&profile).cloned().ok_or_else(|| format!("no profile {profile}"))?;
        match binding {
            Some(b) => p.controls.insert(control, b),
            None => p.controls.remove(&control),
        };
        s.store.save_profile(p).map_err(io)?;
    }
    engine.changed();
    Ok(())
}

#[derive(Serialize)]
pub struct PresetSummary {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    /// A test profile: the "+" menu shows these under their own heading.
    test: bool,
    /// The group it is listed in (`presets::group`).
    group: &'static str,
}

#[tauri::command]
pub fn presets_list() -> Vec<PresetSummary> {
    super::presets::catalog().into_iter().map(|p| PresetSummary { id: p.id, name: p.name, description: p.description, test: p.test, group: super::presets::group(p.id) }).collect()
}

/// Add a preset as a new profile (a fresh copy; the user's profiles are never overwritten).
#[tauri::command]
/// `kinds`: the module kinds the user has (attached and owned); None = the preset as it is.
pub fn preset_add(engine: State<'_, Engine>, id: String, kinds: Option<Vec<String>>) -> Res<Profile> {
    let preset = super::presets::catalog().into_iter().find(|p| p.id == id).ok_or_else(|| format!("no preset {id}"))?;
    let profile = match kinds {
        Some(k) => super::presets::adapt(preset.make(), &k),
        None => preset.make(),
    };
    engine.lock().store.save_profile(profile.clone()).map_err(io)?;
    engine.changed();
    Ok(profile)
}

#[derive(Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Screens {
    brightness: u8,
    blank_after_minutes: u32,
    blank_when_locked: bool,
    /// How app icons show on keys (Settings > Screens).
    #[serde(default)]
    app_icons: String,
    /// The screensaver's effect (empty = none) and its idle minutes.
    #[serde(default)]
    saver: String,
    #[serde(default)]
    saver_after_minutes: u32,
}

#[tauri::command]
pub fn screens_get(engine: State<'_, Engine>) -> Screens {
    let s = &engine.lock().store.settings;
    Screens {
        brightness: s.screen_brightness,
        blank_after_minutes: s.blank_after_minutes,
        blank_when_locked: s.blank_when_locked,
        app_icons: s.app_icons.clone(),
        saver: s.saver.clone(),
        saver_after_minutes: s.saver_after_minutes,
    }
}

/// Screen brightness and when screens turn off; applied within half a second.
#[tauri::command]
pub fn screens_set(engine: State<'_, Engine>, screens: Screens) -> Res<()> {
    let icons_changed = {
        let mut s = engine.lock();
        s.store.settings.screen_brightness = screens.brightness.clamp(5, 100);
        s.store.settings.blank_after_minutes = screens.blank_after_minutes.min(240);
        s.store.settings.blank_when_locked = screens.blank_when_locked;
        s.store.settings.saver = Some(screens.saver.clone()).filter(|e| super::saver::EFFECTS.contains(&e.as_str())).unwrap_or_default();
        if screens.saver_after_minutes > 0 {
            s.store.settings.saver_after_minutes = screens.saver_after_minutes.min(240);
        }
        let changed = !screens.app_icons.is_empty() && screens.app_icons != s.store.settings.app_icons;
        if changed {
            s.store.settings.app_icons = screens.app_icons.clone();
            super::render::set_app_icons(&screens.app_icons);
        }
        s.store.save_settings().map_err(io)?;
        changed
    };
    if icons_changed {
        engine.changed(); // redraw the keys
    }
    Ok(())
}

/// Where the profiles live now.
#[tauri::command]
pub fn profiles_dir_get(engine: State<'_, Engine>) -> (String, bool) {
    let s = engine.lock();
    (s.store.dir().to_string_lossy().to_string(), s.store.settings.profiles_dir.is_some())
}

/// Keep the profiles in another folder (None = back to the app's own). Profiles there are used;
/// the current ones are copied in when they are not there yet.
#[tauri::command]
pub fn profiles_dir_set(engine: State<'_, Engine>, dir: Option<String>) -> Res<()> {
    let data = engine.data_dir().to_path_buf();
    super::backup::create(&data, "before folder change").map_err(io)?;
    let (from, mut settings) = {
        let s = engine.lock();
        (s.store.dir().to_path_buf(), s.store.settings.clone())
    };
    settings.profiles_dir = dir.map(|d| d.trim().to_string()).filter(|d| !d.is_empty());
    let to = super::profile::profiles_dir(&data, &settings);
    std::fs::create_dir_all(&to).map_err(io)?;
    if let Ok(entries) = std::fs::read_dir(&from) {
        for e in entries.flatten().filter(|e| e.path().extension().is_some_and(|x| x == "json")) {
            let target = to.join(e.file_name());
            if !target.exists() {
                std::fs::copy(e.path(), target).map_err(io)?;
            }
        }
    }
    {
        let mut s = engine.lock();
        s.store.settings.profiles_dir = settings.profiles_dir.clone();
        s.store.save_settings().map_err(io)?;
    }
    engine.reload();
    Ok(())
}

#[tauri::command]
pub fn backups_list(engine: State<'_, Engine>) -> Vec<super::backup::BackupInfo> {
    super::backup::list(engine.data_dir())
}

#[tauri::command]
pub fn backup_now(engine: State<'_, Engine>) -> Res<String> {
    super::backup::create(engine.data_dir(), "by hand").map_err(io)
}

/// Put a backup back (the current state is backed up first) and reload the profiles.
#[tauri::command]
pub fn backup_restore(engine: State<'_, Engine>, id: String) -> Res<()> {
    super::backup::restore(engine.data_dir(), &id)?;
    engine.reload();
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameSummary {
    id: &'static str,
    name: &'static str,
    about: &'static str,
    icon: &'static str,
    best: Option<u32>,
}

#[tauri::command]
pub fn games_list(app: tauri::AppHandle) -> Vec<GameSummary> {
    use tauri::Manager as _;
    let scores: std::collections::HashMap<String, u32> =
        app.path().app_data_dir().ok().and_then(|d| std::fs::read(d.join("games.json")).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    super::games::catalog().into_iter().map(|e| GameSummary { id: e.id, name: e.name, about: e.about, icon: e.icon, best: scores.get(e.id).copied() }).collect()
}

/// The wheel's fields.
#[tauri::command]
pub fn wheel_get(engine: State<'_, Engine>) -> Vec<String> {
    super::games::load_settings(engine.data_dir());
    super::games::wheel_choices()
}

/// Set the wheel's fields (2..=12; fewer means the defaults); returns what is used.
#[tauri::command]
pub fn wheel_set(engine: State<'_, Engine>, choices: Vec<String>) -> Res<Vec<String>> {
    let used = super::games::set_wheel_choices(choices);
    super::games::save_settings(engine.data_dir()).map_err(io)?;
    Ok(used)
}

/// Start (`true`) or end game mode on the deck.
#[tauri::command]
pub fn games_play(engine: State<'_, Engine>, on: bool) {
    engine.lock().games_request = Some(on);
}

#[tauri::command]
pub fn plugins_list() -> Vec<super::plugins::PluginInfo> {
    super::plugins::list()
}

/// Install a Stream Deck plugin file (`.streamDeckPlugin`) and start it; the window re-reads the
/// action library. Returns the plugin's id.
#[tauri::command]
pub async fn sd_install(app: tauri::AppHandle, path: String) -> Res<String> {
    use tauri::Emitter as _;
    let id = tauri::async_runtime::spawn_blocking(move || std::fs::read(&path).map_err(|e| e.to_string()).and_then(|b| super::plugins::install_sd(&b))).await.map_err(|e| e.to_string())??;
    let _ = app.emit("engine://catalog", ());
    Ok(id)
}

/// The address of a Stream Deck action's settings page for a control (None: it has none).
#[tauri::command]
pub fn sd_inspector(control: String, action: String, settings: serde_json::Value) -> Option<String> {
    let (plugin, _) = action.split_once('/')?;
    let (sd, dir) = super::plugins::sd_of(plugin)?;
    super::sdplugins::inspector_url(plugin, &dir, &action, &control, &settings, &sd)
}

/// A web or mail link from the window, in the system's browser or mail app.
#[tauri::command]
pub fn open_link(url: String) -> Res<()> {
    if !(url.starts_with("https://") || url.starts_with("mailto:")) {
        return Err(format!("not a web or mail link: {url}"));
    }
    if crate::automation::blocks_opening(&url) {
        return Ok(());
    }
    tauri_plugin_opener::open_url(&url, None::<&str>).map_err(io)
}

/// Open the folder plugins are installed into.
#[tauri::command]
pub fn plugins_open_folder(app: tauri::AppHandle) -> Res<()> {
    use tauri_plugin_opener::OpenerExt as _;
    let dir = super::plugins::folder().ok_or("plugins are not running")?;
    if crate::automation::blocks_opening(&dir.to_string_lossy()) {
        return Ok(());
    }
    app.opener().open_path(dir.to_string_lossy(), None::<&str>).map_err(io)
}

/// Active audio devices: (outputs, inputs).
#[tauri::command]
pub async fn audio_devices() -> (Vec<super::audiodev::Device>, Vec<super::audiodev::Device>) {
    tauri::async_runtime::spawn_blocking(|| {
        super::volume::init_thread();
        (super::audiodev::list(false).unwrap_or_default(), super::audiodev::list(true).unwrap_or_default())
    })
    .await
    .unwrap_or_default()
}

/// MIDI output ports by name (loopMIDI, IAC driver, hardware synths...).
#[tauri::command]
pub async fn midi_ports() -> Vec<String> {
    tauri::async_runtime::spawn_blocking(super::midi::ports).await.unwrap_or_default()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Integrations {
    obs_url: String,
    obs_password_set: bool,
    ha_url: String,
    ha_token_set: bool,
    streamlabs_url: String,
    streamlabs_token_set: bool,
}

#[tauri::command]
pub fn integrations_get(engine: State<'_, Engine>) -> Integrations {
    let s = engine.lock();
    Integrations {
        obs_url: s.store.settings.obs_url.clone().unwrap_or_else(|| super::obs::DEFAULT_URL.into()),
        obs_password_set: super::secrets::get("obs").is_some(),
        ha_url: s.store.settings.ha_url.clone().unwrap_or_default(),
        ha_token_set: super::secrets::get("homeassistant").is_some(),
        streamlabs_url: s.store.settings.streamlabs_url.clone().unwrap_or_else(|| super::streamlabs::DEFAULT_URL.into()),
        streamlabs_token_set: super::secrets::get("streamlabs").is_some(),
    }
}

/// Streamlabs Desktop: its remote-control address and, if given, the API token (empty = remove).
#[tauri::command]
pub fn streamlabs_set(engine: State<'_, Engine>, url: String, token: Option<String>) -> Res<()> {
    let url = url.trim();
    let url = if url.is_empty() { super::streamlabs::DEFAULT_URL } else { url };
    if !(url.starts_with("ws://") || url.starts_with("wss://")) {
        return Err(format!("the Streamlabs address starts with ws://, e.g. {}", super::streamlabs::DEFAULT_URL));
    }
    if let Some(t) = &token {
        super::secrets::set("streamlabs", t.trim())?;
    }
    {
        let mut s = engine.lock();
        s.store.settings.streamlabs_url = (url != super::streamlabs::DEFAULT_URL).then(|| url.to_string());
        s.store.save_settings().map_err(io)?;
    }
    super::streamlabs::configure(url.to_string(), super::secrets::get("streamlabs"));
    Ok(())
}

/// Spotify: (Client ID, signed in).
#[tauri::command]
pub fn spotify_status(engine: State<'_, Engine>) -> (String, bool) {
    (engine.lock().store.settings.spotify_client_id.clone().unwrap_or_default(), super::secrets::get("spotify").is_some())
}

/// Sign in to Spotify in the browser with the user's own app (PKCE); waits up to three minutes.
#[tauri::command]
pub async fn spotify_connect(engine: State<'_, Engine>, client_id: String) -> Res<()> {
    {
        let mut s = engine.lock();
        s.store.settings.spotify_client_id = Some(client_id.trim().to_string());
        s.store.save_settings().map_err(io)?;
    }
    tauri::async_runtime::spawn_blocking(move || {
        super::spotify::connect(&client_id, |url| {
            let _ = tauri_plugin_opener::open_url(url, None::<&str>);
        })
    })
    .await
    .map_err(io)?
}

/// Start recording the user's keys for a multi-action (Windows; stops by itself after 2 minutes).
#[tauri::command]
pub fn macro_record_start() -> Res<()> {
    super::recorder::start()
}

/// Stop recording: the keys as multi-action steps (Hotkey steps with the pauses in between).
#[tauri::command]
pub fn macro_record_stop() -> Vec<serde_json::Value> {
    super::recorder::stop()
}

/// Twitch: (this build can sign in, signed in).
#[tauri::command]
pub fn twitch_status() -> (bool, bool) {
    super::twitch::status()
}

/// Sign in to Twitch with the device code flow; waits up to five minutes for the approval.
#[tauri::command]
pub async fn twitch_connect() -> Res<()> {
    tauri::async_runtime::spawn_blocking(|| {
        super::twitch::connect(|url| {
            let _ = tauri_plugin_opener::open_url(url, None::<&str>);
        })
    })
    .await
    .map_err(io)?
}

#[tauri::command]
pub fn twitch_disconnect() -> Res<()> {
    super::twitch::disconnect()
}

#[tauri::command]
pub fn spotify_disconnect() -> Res<()> {
    super::secrets::set("spotify", "")
}

/// Home Assistant address and, if given, its long-lived token (empty = remove it).
#[tauri::command]
pub fn homeassistant_set(engine: State<'_, Engine>, url: String, token: Option<String>) -> Res<()> {
    let url = url.trim().trim_end_matches('/');
    if !(url.is_empty() || url.starts_with("http://") || url.starts_with("https://")) {
        return Err("the Home Assistant address starts with http:// or https://, e.g. http://homeassistant.local:8123".into());
    }
    if let Some(t) = &token {
        super::secrets::set("homeassistant", t.trim())?;
    }
    {
        let mut s = engine.lock();
        s.store.settings.ha_url = (!url.is_empty()).then(|| url.to_string());
        s.store.save_settings().map_err(io)?;
    }
    super::homeassistant::configure(url.to_string(), super::secrets::get("homeassistant"));
    Ok(())
}

/// Change the OBS address and, if given, its password (empty = no password). The password goes to
/// the system's credential store.
#[tauri::command]
pub fn integrations_set(engine: State<'_, Engine>, obs_url: String, obs_password: Option<String>) -> Res<()> {
    let url = obs_url.trim();
    if !(url.starts_with("ws://") || url.starts_with("wss://")) {
        return Err("the OBS address starts with ws://, e.g. ws://127.0.0.1:4455".into());
    }
    if let Some(pw) = &obs_password {
        super::secrets::set("obs", pw)?;
    }
    {
        let mut s = engine.lock();
        s.store.settings.obs_url = (url != super::obs::DEFAULT_URL).then(|| url.to_string());
        s.store.save_settings().map_err(io)?;
    }
    super::obs::configure(url.to_string(), super::secrets::get("obs"));
    Ok(())
}

/// The first-run welcome was dismissed.
#[tauri::command]
pub fn onboarding_done(engine: State<'_, Engine>) -> Res<()> {
    {
        let mut s = engine.lock();
        s.store.settings.onboarded = true;
        s.store.save_settings().map_err(io)?;
    }
    engine.changed();
    Ok(())
}

/// The icon of a program as a picture for a key; returns the image name.
#[tauri::command]
pub async fn app_icon(path: String) -> Res<String> {
    tauri::async_runtime::spawn_blocking(move || super::appicon::png(&path).and_then(|png| super::images::import_bytes(&png, "png"))).await.map_err(io)?
}

/// Icons of the programs profiles are linked to, also ones not seen in front lately (by exe).
#[tauri::command]
pub async fn app_icons(exes: Vec<String>) -> std::collections::HashMap<String, Option<String>> {
    tauri::async_runtime::spawn_blocking(move || exes.into_iter().map(|e| (e.clone(), super::apps::icon_for_exe(&e))).collect()).await.unwrap_or_default()
}

/// Copy a picture into the app's images folder; returns the name to put in a look.
#[tauri::command]
pub async fn image_import(path: String) -> Res<String> {
    tauri::async_runtime::spawn_blocking(move || super::images::import(std::path::Path::new(&path))).await.map_err(io)?
}

/// Show a key page on the deck (`None` = the main page).
#[tauri::command]
pub fn page_set(engine: State<'_, Engine>, page: Option<String>) -> Res<()> {
    engine.set_page(page)
}

/// Add a key page to a profile; returns it.
#[tauri::command]
pub fn page_create(engine: State<'_, Engine>, profile: String, name: String) -> Res<Page> {
    let page = Page { id: format!("p{}", &uuid::Uuid::new_v4().simple().to_string()[..8]), name: name.trim().to_string() };
    if page.name.is_empty() {
        return Err("a page needs a name".into());
    }
    {
        let mut s = engine.lock();
        let mut p = s.store.profiles.get(&profile).cloned().ok_or_else(|| format!("no profile {profile}"))?;
        p.pages.push(page.clone());
        s.store.save_profile(p).map_err(io)?;
    }
    engine.changed();
    Ok(page)
}

#[tauri::command]
pub fn page_rename(engine: State<'_, Engine>, profile: String, page: String, name: String) -> Res<()> {
    let name = name.trim();
    if name.is_empty() {
        return Err("a page needs a name".into());
    }
    {
        let mut s = engine.lock();
        let mut p = s.store.profiles.get(&profile).cloned().ok_or_else(|| format!("no profile {profile}"))?;
        p.pages.iter_mut().find(|x| x.id == page).ok_or_else(|| format!("no page {page}"))?.name = name.to_string();
        s.store.save_profile(p).map_err(io)?;
    }
    engine.changed();
    Ok(())
}

/// Remove a page and its keys; keys elsewhere that went to it now do nothing.
#[tauri::command]
pub fn page_delete(engine: State<'_, Engine>, profile: String, page: String) -> Res<()> {
    {
        let mut s = engine.lock();
        let mut p = s.store.profiles.get(&profile).cloned().ok_or_else(|| format!("no profile {profile}"))?;
        p.pages.retain(|x| x.id != page);
        let prefix = format!("{page}/");
        p.controls.retain(|id, _| !id.starts_with(&prefix));
        s.store.save_profile(p).map_err(io)?;
        if s.page.as_deref() == Some(page.as_str()) {
            s.page = None;
        }
    }
    engine.changed();
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    actions: Vec<ActionDef>,
    special_keys: Vec<&'static str>,
}

#[tauri::command]
pub fn actions_list() -> Catalog {
    let actions = actions::all().into_iter().filter(|a| super::games::GAMES_ENABLED || a.id != actions::GAMES).collect();
    Catalog { actions, special_keys: hotkey::SPECIAL_KEYS.to_vec() }
}

#[derive(Serialize)]
pub struct Icon {
    name: &'static str,
    svg: &'static str,
}

#[tauri::command]
pub fn icons_list() -> Vec<Icon> {
    icons::ICONS.iter().map(|(name, svg)| Icon { name, svg }).collect()
}

/// The exact SVG the hardware gets (unrotated), for the live preview.
#[tauri::command]
pub fn preview_svg(screen: String, look: Look) -> String {
    match screen.as_str() {
        "dial" => render::dial_svg(&look, None, 0),
        _ => render::key_svg(&look, 0),
    }
}

#[tauri::command]
pub fn profile_export(engine: State<'_, Engine>, id: String, path: String) -> Res<()> {
    // a .hubdeck package carries the pictures too; plain JSON only the settings
    if path.to_lowercase().ends_with(&format!(".{}", super::share::EXT)) {
        return super::share::export_package(&engine, &id, std::path::Path::new(&path));
    }
    let profile = engine.lock().store.profiles.get(&id).cloned().ok_or_else(|| format!("no profile {id}"))?;
    std::fs::write(path, serde_json::to_vec_pretty(&profile).map_err(io)?).map_err(io)
}

/// An imported profile and what was copied to fit the user's modules (`moved` in JSON, next to
/// the profile's own fields).
#[derive(Serialize)]
pub struct Imported {
    #[serde(flatten)]
    pub profile: Profile,
    pub moved: Vec<super::presets::Moved>,
}

/// Import a profile file as a new profile (fresh id, so it never overwrites one), adapted to the
/// user's modules (the original bindings stay).
#[tauri::command]
pub fn profile_import(app: tauri::AppHandle, engine: State<'_, Engine>, path: String) -> Res<Imported> {
    super::backup::create(engine.data_dir(), "before import").map_err(io)?;
    let bytes = std::fs::read(&path).map_err(io)?;
    let adapt = |p: Profile| super::api::adapt_to_user(&app, p);
    // a HubDeck package (.hubdeck): the profile with its pictures
    if super::share::is_package(&bytes) {
        return super::share::import_package(&engine, &bytes, &adapt);
    }
    // a profile exported by the official MasterHUB app: a zip (starts with "PK")
    if path.to_lowercase().ends_with(".mhz") || bytes.starts_with(b"PK\x03\x04") {
        let file_name = std::path::Path::new(&path).file_name().and_then(|n| n.to_str()).unwrap_or("MasterHUB.mhz").to_string();
        let profiles = super::mhz::import(&bytes, &file_name, &mut |data, ext| super::images::import_bytes(data, ext).map_err(|e| log::warn!("mhz picture: {e}")).ok())?;
        let mut adapted: Vec<(Profile, Vec<super::presets::Moved>)> = profiles.into_iter().map(adapt).collect();
        if adapted.is_empty() {
            return Err("the .mhz file holds no profile".into());
        }
        let (first, moved) = adapted.remove(0);
        {
            let mut shared = engine.lock();
            for p in std::iter::once(first.clone()).chain(adapted.into_iter().map(|(p, _)| p)) {
                shared.store.save_profile(p).map_err(io)?;
            }
        }
        engine.changed();
        // the caller activates the returned (first) profile
        return Ok(Imported { profile: first, moved });
    }
    let mut profile: Profile = serde_json::from_slice(&bytes).map_err(|e| format!("not a HubDeck profile: {e}"))?;
    if profile.version > PROFILE_VERSION {
        return Err(format!("this profile was made by a newer version (format {})", profile.version));
    }
    // actions of plugins that are not installed are kept: they work again once the plugin is there
    profile.id = uuid::Uuid::new_v4().to_string();
    profile.version = PROFILE_VERSION;
    let (profile, moved) = adapt(profile);
    engine.lock().store.save_profile(profile.clone()).map_err(io)?;
    engine.changed();
    Ok(Imported { profile, moved })
}

/// "Adapt to my modules": copy the turn jobs of the module kinds the user does not have to the
/// ones they have (the original bindings stay); returns what moved, empty when nothing had to.
#[tauri::command]
pub fn profile_adapt(app: tauri::AppHandle, engine: State<'_, Engine>, id: String) -> Res<Vec<super::presets::Moved>> {
    let profile = engine.lock().store.profiles.get(&id).cloned().ok_or_else(|| format!("no profile {id}"))?;
    let (profile, moved) = super::api::adapt_to_user(&app, profile);
    if !moved.is_empty() {
        engine.lock().store.save_profile(profile).map_err(io)?;
        engine.changed();
    }
    Ok(moved)
}

/// What the knob, fader and base LEDs show now, by module id (None = the module's rainbow);
/// changes come as `engine://leds`.
#[tauri::command]
pub fn leds_get(engine: State<'_, Engine>) -> std::collections::HashMap<String, Option<Vec<super::leds::LedView>>> {
    engine.lock().leds.clone()
}

/// Settings: during a call the deck shows the call app's profile (on) or switches nothing.
#[tauri::command]
pub fn call_switch_set(engine: State<'_, Engine>, on: bool) -> Res<()> {
    {
        let mut s = engine.lock();
        s.store.settings.call_switch = on;
        s.store.save_settings().map_err(io)?;
    }
    engine.changed();
    Ok(())
}

/// The window's language, for the words the deck shows (`en` or `pl`).
#[tauri::command]
pub fn language_set(engine: State<'_, Engine>, language: String) -> Res<()> {
    let language = if language == "pl" { "pl" } else { "en" };
    let changed = {
        let mut s = engine.lock();
        let changed = s.store.settings.language != language;
        super::demo::set_language(language);
        if changed {
            s.store.settings.language = language.into();
            s.store.save_settings().map_err(io)?;
        }
        changed
    };
    if changed {
        engine.changed();
    }
    Ok(())
}

/// Whether Claude Code reports to HubDeck (HubDeck's hooks are in `~/.claude/settings.json`).
#[tauri::command]
pub fn claude_status() -> bool {
    super::agents::connected()
}

/// Connect (add HubDeck's hooks to the user's Claude Code settings) or disconnect (remove them).
/// The previous settings are kept as `settings.json.hubdeck-backup`.
#[tauri::command]
pub fn claude_connect(on: bool, approve: Option<bool>) -> Res<bool> {
    super::agents::set_connected_with(on, approve.unwrap_or_else(super::agents::approving))
}

/// Whether permission prompts of safe tools are answered on the deck.
#[tauri::command]
pub fn claude_approving() -> bool {
    super::agents::approving()
}

/// macOS: may HubDeck send keys and type (Accessibility)? Elsewhere always yes.
#[tauri::command]
pub fn accessibility_ok() -> bool {
    // the dev app gets its permissions from the program that started it (a terminal): it cannot be
    // allowed by itself, so it does not ask (the owner kept seeing the banner after allowing)
    cfg!(debug_assertions) || accessibility::trusted(false)
}

/// macOS: ask for Accessibility (the system's own dialog, which leads to System Settings).
#[tauri::command]
pub fn accessibility_ask() -> bool {
    accessibility::trusted(true)
}

#[cfg(target_os = "macos")]
mod accessibility {
    use objc2_foundation::{NSDictionary, NSNumber, NSString};

    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn AXIsProcessTrustedWithOptions(options: *const std::ffi::c_void) -> bool;
    }

    /// `prompt`: show the system's dialog when not trusted yet.
    pub fn trusted(prompt: bool) -> bool {
        let key = NSString::from_str("AXTrustedCheckOptionPrompt");
        let value = NSNumber::new_bool(prompt);
        let options = NSDictionary::from_slices(&[&*key], &[&*value]);
        // SAFETY: a CFDictionary (toll-free bridged NSDictionary) we own for the call.
        unsafe { AXIsProcessTrustedWithOptions((&*options as *const NSDictionary<NSString, NSNumber>).cast()) }
    }
}

#[cfg(not(target_os = "macos"))]
mod accessibility {
    pub fn trusted(_prompt: bool) -> bool {
        true
    }
}
