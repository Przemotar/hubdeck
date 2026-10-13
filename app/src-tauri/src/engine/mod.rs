//! The engine: active profile, actions, and what the keys and dial show. One thread owns the
//! executor and the renderer; the device thread feeds it input, commands feed it changes.

pub mod actions;
pub mod agents;
pub mod api;
mod appicon;
pub mod apps;
pub mod assistant;
pub mod audiodev;
pub mod auto;
pub mod backup;
mod brightness;
pub mod builder;
pub mod call;
pub mod check;
pub mod cli;
pub mod commands;
pub mod deck_mcp;
pub mod demo;
pub mod diagnostics;
pub mod elevated;
mod exec;
mod foreground;
pub mod gallery;
pub mod game_icons;
pub mod games;
mod gestures;
pub mod guard;
pub mod homeassistant;
pub(crate) mod hotkey;
pub mod house;
pub mod iconpacks;
mod icons;
mod idle;
pub mod images;
pub mod installed;
mod leds;
mod lhm;
pub mod live;
pub mod localize;
#[cfg(target_os = "macos")]
pub mod machw;
pub mod mcp;
mod media;
pub mod mhz;
pub mod midi;
pub mod monitor;
pub mod nudge;
pub mod obs;
pub mod plugins;
pub(crate) mod presets;
pub mod presses;
pub mod profile;
pub mod recorder;
pub mod render;
mod saver;
pub mod sdcatalog;
pub mod sdplugins;
pub mod secrets;
mod seek;
pub mod share;
pub mod shortcuts;
pub mod showcase;
pub mod sound;
pub mod spotify;
pub mod starter;
pub mod states;
pub mod streamlabs;
pub mod suggest;
pub mod taste;
mod test_presets;
mod timer;
pub mod tour;
pub mod twitch;
pub mod usage;
pub mod vjoy;
pub mod volume;

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, unbounded};
use mh_protocol::report;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::deck::Deck;
use crate::placements::Placements;
pub use exec::InputEvent;
use exec::{Executor, Feedback};
use gestures::{Bound, Gestures, accel_factor, fader_filter};
use mh_protocol::report::{LedMode, LedTarget, Rgb};
use profile::{Binding, Look, Profile, Store, control_id};
use render::Renderer;

pub const STATE_EVENT: &str = "engine://state";
pub const CALIBRATION_EVENT: &str = "engine://calibration";
pub const ERROR_EVENT: &str = "engine://error";
/// What the knob, fader and base LEDs show now, by module id (`leds::LedView`).
pub const LEDS_EVENT: &str = "engine://leds";
/// How often HubDeck's rainbow moves on (10 steps a second: smooth on the LEDs, light on USB).
const RAINBOW_STEP: Duration = Duration::from_millis(100);
/// How long the dial shows a value after it changed.
const FEEDBACK_FOR: Duration = Duration::from_millis(1500);
/// How often stateful keys check their state (mute can change outside the deck).
const STATE_POLL: Duration = Duration::from_millis(1000);
/// Minimum time between two dial images.
const DIAL_FRAME: Duration = Duration::from_millis(60);

/// Merge consecutive turns of the same control (`+1 +1 +1` -> `+3`), keeping order otherwise.
fn coalesce_turns(batch: &mut Vec<Msg>) {
    let mut out: Vec<Msg> = Vec::with_capacity(batch.len());
    for msg in batch.drain(..) {
        if let (Msg::Input { control, event: InputEvent::Turn(d) }, Some(Msg::Input { control: last_c, event: InputEvent::Turn(last_d) })) = (&msg, out.last_mut())
            && control == last_c
        {
            *last_d = last_d.saturating_add(*d);
            continue;
        }
        out.push(msg);
    }
    *batch = out;
}

const SLOT_TABLE_JSON: &str = include_str!("../../../src/slots.generated.json");

pub enum Msg {
    Input {
        control: String,
        event: InputEvent,
    },
    /// Modules, orientation or placements changed: redraw everything.
    DeckChanged,
    /// A profile or the active one changed: redraw everything.
    ProfilesChanged,
    /// The app in front (exe file name) and its window title.
    Foreground {
        exe: String,
        title: String,
    },
    /// A fader's position when it was attached (0..=100): feeds level LEDs, runs no action.
    Level {
        control: String,
        value: u8,
    },
    /// What is playing changed (None = nothing).
    NowPlaying(Option<media::Track>),
    /// A new system load sample (once a second).
    Metrics(monitor::Sample),
    /// An integration (OBS...) reports an on/off state, e.g. `obs:record`.
    IntegrationState {
        key: String,
        on: bool,
    },
    /// An integration reports a value, e.g. the live OBS scene under `obs:scene`.
    IntegrationValue {
        key: String,
        value: String,
    },
    /// An integration failed doing what a control asked for.
    IntegrationError {
        source: String,
        message: String,
    },
    /// An integration finished something worth showing on the dial ("Liked <song>").
    IntegrationFeedback(String),
    /// A question or a status for the deck changed: draw the keys and the dial again.
    DeckUi,
    /// Light the base edge in this color for a while (MCP `notify`).
    Notify {
        rgb: Rgb,
        seconds: u64,
    },
    /// A plugin (or a local API client) sent a message.
    FromPlugin {
        plugin: String,
        message: serde_json::Value,
    },
    /// A local API client asked for the engine state.
    ApiStateRequest(String),
    /// The session was locked or unlocked (Windows: Win+L, the lock screen after resume).
    #[cfg_attr(not(windows), allow(dead_code))]
    Locked(bool),
}

/// The looks plugins give the active profile's controls now (see `publish_looks`).
static LIVE_LOOKS: std::sync::LazyLock<std::sync::Mutex<(String, HashMap<String, Look>)>> = std::sync::LazyLock::new(Default::default);

/// `{profile, looks}`: the profile they are for, and the looks by control id.
#[tauri::command]
pub fn live_looks() -> serde_json::Value {
    let (profile, looks) = LIVE_LOOKS.lock().unwrap_or_else(|e| e.into_inner()).clone();
    serde_json::json!({"profile": profile, "looks": looks})
}

/// What a plugin or the local API put on a control: title, picture, on/off.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PluginLook {
    pub title: Option<String>,
    pub image: Option<String>,
    pub state: Option<bool>,
    /// What a Stream Deck plugin shows for an encoder (`setFeedback`): value 0..1, big text, title.
    pub feedback: Option<(Option<f32>, String, String)>,
    /// A photo its layout shows (a song's cover), drawn behind the dial's text.
    pub cover: Option<String>,
}

/// A profile on the deck for a try, and what was showing before: (active, home, the app that took over).
#[derive(Debug, Clone)]
pub struct Trying {
    pub profile: String,
    pub back: (Option<String>, Option<String>, Option<String>),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSummary {
    pub id: String,
    pub name: String,
    pub apps: Vec<String>,
    /// Shown as a tab in the top bar (`Profile::shows_in_bar`); never for a draft.
    pub in_bar: bool,
    /// A draft of the deck assistant.
    pub draft: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineState {
    pub profiles: Vec<ProfileSummary>,
    pub active: Option<String>,
    /// The home profile at start (setting `fallback`).
    pub fallback: Option<String>,
    /// The profile the user picked by hand; app profiles give back to it.
    pub home: Option<String>,
    /// The app whose own profile took over (`Photoshop.exe`), None while the home profile shows.
    pub auto_app: Option<String>,
    /// "Pin this profile": automatic switching is off until unpinned.
    pub pinned: bool,
    pub auto_switch: bool,
    /// Executable in front, e.g. `obs64.exe`.
    pub foreground: String,
    /// Key page shown on the deck; None = the main page.
    pub page: Option<String>,
    /// The first-run welcome was dismissed.
    pub onboarded: bool,
    /// The deck is in game mode.
    pub playing: bool,
    /// Profiles that made themselves and the user has not looked at yet.
    pub auto_made: Vec<String>,
    /// Names of the apps that connected themselves and the user has not looked at yet.
    pub apps_joined: Vec<String>,
    /// A call is on (the deck stays on its profile).
    pub in_call: bool,
    /// During a call the deck shows the call app's profile.
    pub call_switch: bool,
    /// Profiles make themselves for apps with a preset and for games.
    pub auto_profiles: bool,
    /// Less movement (Settings): None = not set yet (the window takes the system's setting).
    pub reduced_motion: Option<bool>,
    /// The draft shown on the deck for a try (`Engine::try_profile`).
    pub trying: Option<String>,
    /// Comparing drafts on the deck: (which one, of how many).
    pub comparing: Option<(usize, usize)>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineError {
    pub control: String,
    pub message: String,
}

/// The key panel calibration in progress: a draft geometry and the parameter the dial changes.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Calibration {
    pub geometry: render::PanelGeometry,
    pub selected: usize,
}

/// Side-by-side check of how keys are drawn: A = 15 separate 72x72 images at JPEG 85 (the first
/// version), B = one full-panel image at JPEG 98 scaled to the calibrated windows (current),
/// C = like B at JPEG 100.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, serde::Deserialize)]
pub enum CompareVariant {
    A,
    B,
    C,
}

pub(crate) struct Shared {
    pub store: Store,
    pub active: Option<String>,
    /// The profile the user picked by hand (or the home profile at start).
    pub home: Option<String>,
    /// The app whose own profile is showing now.
    pub auto_app: Option<String>,
    pub foreground: String,
    /// Key page of the active profile on the deck; None = the main page.
    pub page: Option<String>,
    pub calibration: Option<Calibration>,
    pub compare: Option<CompareVariant>,
    /// Game mode is on (the worker owns the arcade itself).
    pub playing: bool,
    /// Set by the window: start or stop game mode at the next tick.
    pub games_request: Option<bool>,
    /// What the LEDs of each module show now, for the window's drawing (None = its rainbow).
    pub leds: HashMap<String, Option<Vec<leds::LedView>>>,
    /// A question an assistant asks on the deck (MCP `ask_physical`): options on the keys.
    pub prompt: Option<Prompt>,
    /// A status an assistant shows on the dial (MCP `show_status`, `notify`).
    pub status: Option<DeckStatus>,
    /// Last value per control (0..=1), copied from the worker for `GET /api/controls`.
    pub levels: HashMap<String, f32>,
    /// A call is on and the deck shows its profile: apps in front switch nothing until it ends.
    pub call_hold: bool,
    /// A draft shown on the deck for a try: what to go back to afterwards.
    pub trying: Option<Trying>,
    /// Drafts compared on the deck (the dial flips between them) and which one shows.
    pub draft_compare: Option<(Vec<String>, usize)>,
    /// A screensaver shown on request for a moment (the tour's finale): its effect and until when.
    pub saver_preview: Option<(String, Instant)>,
    /// The tour is on (`tour.rs`): actions that would go live, record or leave a call only say so.
    pub tour_dry_run: bool,
    /// A control the tour points at on the hardware (`keys.5`, `knobs.2`, `dial`): it blinks (holds
    /// still with reduced motion) from the first instant until the second, or until it is touched.
    pub highlight: Option<(String, Instant, Instant)>,
}

/// A question on the deck: the options on the first keys, the question on the dial.
#[derive(Debug, Clone)]
pub struct Prompt {
    pub id: u64,
    pub question: String,
    pub options: Vec<String>,
    /// None while waiting; Some(Some(i)) = option i pressed, Some(None) = dismissed (dial) or timed out.
    pub answer: Option<Option<usize>>,
    pub until: Instant,
}

impl Prompt {
    pub fn waiting(&self) -> bool {
        self.answer.is_none() && Instant::now() < self.until
    }
}

/// Text on the dial put there by an assistant; `until` None = until cleared.
#[derive(Debug, Clone)]
pub struct DeckStatus {
    pub main: String,
    pub detail: Option<String>,
    pub until: Option<Instant>,
}

impl Shared {
    fn active_profile(&self) -> Option<&Profile> {
        self.active.as_ref().and_then(|id| self.store.profiles.get(id))
    }

    pub fn state(&self) -> EngineState {
        EngineState {
            profiles: self
                .store
                .profiles
                .values()
                .map(|p| {
                    let draft = self.store.settings.drafts.contains(&p.id);
                    ProfileSummary { id: p.id.clone(), name: p.name.clone(), apps: p.apps.clone(), in_bar: p.shows_in_bar() && !draft, draft }
                })
                .collect(),
            trying: self.trying.as_ref().map(|t| t.profile.clone()),
            comparing: self.draft_compare.as_ref().map(|(ids, i)| (i + 1, ids.len())),
            active: self.active.clone(),
            fallback: self.store.fallback_id(),
            home: self.home.clone(),
            auto_app: self.auto_app.clone(),
            pinned: self.store.settings.pinned,
            auto_switch: self.store.settings.auto_switch,
            foreground: self.foreground.clone(),
            page: self.page.clone(),
            onboarded: self.store.settings.onboarded,
            playing: self.playing,
            auto_made: self.store.settings.auto_made.clone(),
            apps_joined: self.store.settings.apps.iter().filter(|a| self.store.settings.apps_joined.contains(&a.id)).map(|a| a.name.clone()).collect(),
            in_call: self.call_hold,
            call_switch: self.store.settings.call_switch,
            auto_profiles: self.store.settings.auto_profiles,
            reduced_motion: self.store.settings.reduced_motion,
        }
    }
}

#[derive(Clone)]
pub struct Engine {
    data_dir: std::path::PathBuf,
    tx: Sender<Msg>,
    shared: Arc<Mutex<Shared>>,
    app: AppHandle,
}

impl Engine {
    pub fn start(app: AppHandle, deck: Deck, data_dir: &Path) -> Self {
        images::init(data_dir);
        #[cfg(target_os = "macos")]
        log::info!(
            "accessibility (keys and typing): {}",
            if cfg!(debug_assertions) {
                "dev build, follows the program that started it"
            } else if commands::accessibility_ok() {
                "allowed"
            } else {
                "not allowed yet"
            }
        );
        // Stream Deck plugins written as a web page run in a hidden window of their own
        let html_app = app.clone();
        sdplugins::set_html_launcher(Box::new(move |plugin, url| {
            let label: String = format!("sdplugin-{plugin}").chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '-' }).collect();
            let (app, url) = (html_app.clone(), url.to_string());
            // windows are made on the main thread
            let _ = html_app.run_on_main_thread(move || {
                if let Some(w) = app.get_webview_window(&label) {
                    let _ = w.destroy();
                }
                let Ok(parsed) = url.parse() else { return };
                if let Err(e) = tauri::WebviewWindowBuilder::new(&app, &label, tauri::WebviewUrl::External(parsed)).visible(false).title("HubDeck plugin").build() {
                    log::warn!("HTML plugin window: {e}");
                }
            });
        }));
        apps::init(data_dir);
        usage::init(data_dir);
        taste::init(data_dir);
        presses::init(data_dir);
        nudge::init(data_dir);
        installed::init(data_dir);
        backup::daily(data_dir);
        let mut store = Store::load(data_dir);
        render::set_app_icons(&store.settings.app_icons);
        guard::set_power(store.settings.assistant_power);
        demo::set_language(&store.settings.language);
        if store.profiles.is_empty() {
            let desk = presets::everyday_desk();
            store.settings.fallback = Some(desk.id.clone());
            if let Err(e) = store.save_profile(desk).and_then(|_| store.save_settings()) {
                log::error!("could not save the default profile: {e}");
            }
        }
        // HubDeck starts with the home profile at start; the user's pick replaces it until next time
        let active = store.fallback_id();
        let shared = Arc::new(Mutex::new(Shared {
            store,
            home: active.clone(),
            auto_app: None,
            active,
            foreground: String::new(),
            page: None,
            calibration: None,
            compare: None,
            playing: false,
            games_request: None,
            leds: HashMap::new(),
            prompt: None,
            status: None,
            levels: HashMap::new(),
            call_hold: false,
            trying: None,
            draft_compare: None,
            saver_preview: None,
            tour_dry_run: false,
            highlight: None,
        }));
        let (tx, rx) = unbounded();
        let engine = Self { data_dir: data_dir.to_path_buf(), tx: tx.clone(), shared: shared.clone(), app: app.clone() };

        let worker = Worker {
            app,
            deck,
            shared,
            executor: Executor::default(),
            renderer: Renderer::default(),
            sent: HashMap::new(),
            feedback: None,
            slot_table: parse_slot_table(),
            dial_dirty: true,
            gestures: Gestures::default(),
            scheduled: VecDeque::new(),
            levels: HashMap::new(),
            leds_sent: HashMap::new(),
            lighting: HashMap::new(),
            modes: HashMap::new(),
            gallery: 0,
            notify_until: None,
            rainbow_running: false,
            in_call: None,
            call_checked: Instant::now(),
            rainbow_last: Instant::now(),
            last_turn: HashMap::new(),
            frames: HashMap::new(),
            clock_shown: String::new(),
            timers: HashMap::new(),
            live_sig: String::new(),
            live_next: Instant::now(),
            live_dirty: false,
            states: HashMap::new(),
            last_state_check: Instant::now(),
            now_playing: None,
            samples: VecDeque::new(),
            integration: HashMap::new(),
            values: HashMap::new(),
            plugin_looks: HashMap::new(),
            fleet: agents::Fleet::default(),
            fleet_pulse: Instant::now(),
            saver_on: false,
            saver_frame: Instant::now(),
            fleet_sweep: Instant::now(),
            plugin_leds: HashMap::new(),
            plugin_panel: None,
            toggles: HashMap::new(),
            arcade: None,
            profiles_stamp: (0, None),
            last_folder_check: Instant::now(),
            data_dir: data_dir.to_path_buf(),
            blanked: false,
            locked: false,
            last_deck_input: Instant::now(),
            backlight_sent: None,
            last_blank_check: Instant::now(),
            highlight_shown: None,
        };
        std::thread::Builder::new().name("engine".into()).spawn(move || worker.run(rx)).expect("spawn engine thread");
        let media_tx = tx.clone();
        // a test copy is told the app in front (`/api/test/foreground`), the real one would mix in
        if crate::automation::get().is_none() {
            std::thread::Builder::new().name("foreground".into()).spawn(move || foreground::watch(tx)).expect("spawn foreground thread");
        }
        let monitor_tx = media_tx.clone();
        {
            let s = engine.shared.lock().unwrap_or_else(|e| e.into_inner());
            obs::start(media_tx.clone(), s.store.settings.obs_url.clone().unwrap_or_else(|| obs::DEFAULT_URL.into()), secrets::get("obs"));
            homeassistant::start(media_tx.clone(), s.store.settings.ha_url.clone().unwrap_or_default(), secrets::get("homeassistant"));
            twitch::start(media_tx.clone());
            live::init(media_tx.clone());
            // the user's own apps: HubDeck listens to their state (docs/APP-CONTRACT.md)
            states::init(media_tx.clone());
            states::connect_all(s.store.settings.apps.clone());
            sound::init(media_tx.clone());
            streamlabs::start(media_tx.clone(), s.store.settings.streamlabs_url.clone().unwrap_or_else(|| streamlabs::DEFAULT_URL.into()), secrets::get("streamlabs"));
            plugins::start(data_dir, media_tx.clone());
            spotify::start(media_tx.clone(), s.store.settings.spotify_client_id.clone().unwrap_or_default());
            let shared_for_api = engine.shared.clone();
            plugins::set_state_source(Box::new(move || serde_json::to_value(shared_for_api.lock().unwrap_or_else(|e| e.into_inner()).state()).unwrap_or_default()));
        }
        std::thread::Builder::new().name("media".into()).spawn(move || media::watch(media_tx)).expect("spawn media thread");
        std::thread::Builder::new().name("monitor".into()).spawn(move || monitor::watch(monitor_tx)).expect("spawn monitor thread");
        engine
    }

    pub fn sender(&self) -> Sender<Msg> {
        self.tx.clone()
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, Shared> {
        self.shared.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Tell the engine thread to redraw and the window about the new state.
    /// A question or a status for the deck changed.
    pub fn deck_ui(&self) {
        let _ = self.tx.send(Msg::DeckUi);
    }

    /// Light the base edge for a while.
    pub fn notify_light(&self, rgb: Rgb, seconds: u64) {
        let _ = self.tx.send(Msg::Notify { rgb, seconds });
    }

    /// Test copy: this app is in front now, as if the user switched to it (seen at `path`).
    pub fn test_foreground(&self, exe: String, path: &str, title: String) {
        if !path.is_empty() {
            apps::seen(&exe, path, &title);
        }
        let _ = self.tx.send(Msg::Foreground { exe, title });
    }

    /// Look at the app in front again (a profile for it may have just been made).
    pub fn refresh_foreground(&self) {
        let exe = self.lock().foreground.clone();
        if !exe.is_empty() {
            let _ = self.tx.send(Msg::Foreground { exe, title: String::new() });
        }
    }

    pub fn changed(&self) {
        let _ = self.tx.send(Msg::ProfilesChanged);
        let state = self.lock().state();
        publish_state(&self.app, state);
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Read profiles and settings again from disk (after a restore).
    pub fn reload(&self) {
        {
            let mut s = self.lock();
            let store = Store::load(&self.data_dir);
            let active = s.active.clone().filter(|id| store.profiles.contains_key(id)).or_else(|| store.fallback_id());
            let home = s.home.clone().filter(|id| store.profiles.contains_key(id)).or_else(|| active.clone());
            s.store = store;
            s.active = active;
            s.home = home;
            s.page = None;
        }
        self.changed();
    }

    /// Show a key page of the active profile on the deck (None = the main page).
    pub fn set_page(&self, page: Option<String>) -> Result<(), String> {
        {
            let mut s = self.lock();
            if let Some(p) = &page
                && !s.active_profile().is_some_and(|prof| prof.pages.iter().any(|x| &x.id == p))
            {
                return Err(format!("no page {p}"));
            }
            s.page = page;
        }
        self.changed();
        Ok(())
    }

    /// Make a profile active now (and after a restart). Auto-switch may change it again when
    /// another app comes to the front.
    pub fn activate(&self, id: &str) -> Result<(), String> {
        {
            let mut s = self.lock();
            if !s.store.profiles.contains_key(id) {
                return Err(format!("no profile {id}"));
            }
            if s.active.as_deref() != Some(id) {
                s.page = None;
            }
            // picked by hand: this is home now, app profiles give back to it
            s.active = Some(id.to_string());
            s.home = Some(id.to_string());
            s.auto_app = None;
            s.store.settings.active = Some(id.to_string());
            s.store.save_settings().map_err(|e| e.to_string())?;
        }
        self.changed();
        Ok(())
    }

    /// Show `id` on the deck for a try (a draft, while choosing), without making it home or
    /// saving it as the active one; apps do not switch meanwhile. None: back to what was there.
    pub fn try_profile(&self, id: Option<&str>) -> Result<(), String> {
        {
            let mut s = self.lock();
            match id {
                Some(id) => {
                    if !s.store.profiles.contains_key(id) {
                        return Err(format!("no profile {id}"));
                    }
                    let back = match s.trying.take() {
                        Some(t) => t.back,
                        None => (s.active.clone(), s.home.clone(), s.auto_app.clone()),
                    };
                    s.trying = Some(Trying { profile: id.to_string(), back });
                    s.active = Some(id.to_string());
                    s.auto_app = None;
                    s.page = None;
                }
                None => {
                    // the end of a try ends a comparison too, and its note on the dial
                    if s.draft_compare.take().is_some() {
                        s.status = None;
                    }
                    let Some(t) = s.trying.take() else { return Ok(()) };
                    let (active, home, auto_app) = t.back;
                    s.active = active.filter(|a| s.store.profiles.contains_key(a)).or_else(|| s.store.fallback_id());
                    s.home = home;
                    s.auto_app = auto_app;
                    s.page = None;
                }
            }
        }
        self.changed();
        Ok(())
    }

    /// Compare `ids` on the deck: the first shows, turning the dial flips to the next or the one
    /// before, the dial says which ("2/3 · name"). Empty: stop and go back.
    pub fn compare(&self, ids: Vec<String>, at: usize) -> Result<(), String> {
        if ids.is_empty() {
            return self.try_profile(None);
        }
        // from the one the window shows (owner's pick C: the window and the deck show the same)
        let at = at.min(ids.len() - 1);
        self.lock().draft_compare = Some((ids, at));
        self.show_compared()
    }

    /// Show the compared draft that is current, with its place and name on the dial.
    fn show_compared(&self) -> Result<(), String> {
        let Some((ids, i)) = self.lock().draft_compare.clone() else { return Ok(()) };
        let id = ids[i].clone();
        self.try_profile(Some(&id))?;
        let mut s = self.lock();
        let name = s.store.profiles.get(&id).map(|p| p.name.clone()).unwrap_or_default();
        s.draft_compare = Some((ids.clone(), i));
        s.status = Some(DeckStatus { main: format!("{}/{}", i + 1, ids.len()), detail: Some(name), until: Some(Instant::now() + Duration::from_secs(4)) });
        drop(s);
        self.deck_ui();
        Ok(())
    }

    /// The user picked a draft ("Use this": it becomes a normal profile) or threw it away.
    /// The user's own apps and what they report now (the API, the assistant).
    pub fn app_states(&self) -> serde_json::Value {
        let apps = self.lock().store.settings.apps.clone();
        serde_json::json!({"apps": apps, "states": states::snapshot()})
    }

    /// Connect an app by the address of its manifest or status (docs/APP-CONTRACT.md).
    pub fn connect_app(&self, url: &str) -> Result<states::AppSource, String> {
        let taken: Vec<String> = self.lock().store.settings.apps.iter().map(|a| a.id.clone()).collect();
        let app = states::discover(url, &taken)?;
        let apps = {
            let mut s = self.lock();
            // added by hand: its announcements are welcome again
            s.store.settings.apps_refused.retain(|r| *r != app.status);
            s.store.settings.apps.retain(|a| a.status != app.status);
            s.store.settings.apps.push(app.clone());
            s.store.save_settings().map_err(|e| e.to_string())?;
            s.store.settings.apps.clone()
        };
        states::connect_all(apps);
        self.changed();
        Ok(app)
    }

    /// An app announced itself (`POST /api/apps/announce`, owner 2026-10-13: "dlaczego muszę to
    /// robić ręcznie?"): connect it without a click and say so in the window; an app already
    /// connected is refreshed, one the user disconnected is refused.
    pub fn announce_app(&self, url: &str) -> Result<serde_json::Value, String> {
        let known: Vec<states::AppSource> = self.lock().store.settings.apps.clone();
        let taken: Vec<String> = known.iter().map(|a| a.id.clone()).collect();
        let mut app = states::discover(url, &taken)?;
        if self.lock().store.settings.apps_refused.contains(&app.status) {
            return Err("the user disconnected this app; they can add it again in Settings > Connections > Your apps".into());
        }
        let old = known.iter().find(|a| a.status == app.status);
        if let Some(old) = old {
            app.id = old.id.clone();
            if *old == app {
                return Ok(serde_json::json!({"ok": true, "id": app.id, "new": false}));
            }
        }
        let new = old.is_none();
        let apps = {
            let mut s = self.lock();
            s.store.settings.apps.retain(|a| a.status != app.status);
            s.store.settings.apps.push(app.clone());
            if new {
                s.store.settings.apps_joined.push(app.id.clone());
            }
            s.store.save_settings().map_err(|e| e.to_string())?;
            s.store.settings.apps.clone()
        };
        log::info!("app {} announced itself ({}), {}", app.name, app.status, if new { "connected" } else { "updated" });
        states::connect_all(apps);
        self.changed();
        Ok(serde_json::json!({"ok": true, "id": app.id, "new": new}))
    }

    pub fn remove_app(&self, id: &str) -> Result<(), String> {
        let apps = {
            let mut s = self.lock();
            let status = s.store.settings.apps.iter().find(|a| a.id == id).map(|a| a.status.clone());
            let Some(status) = status else {
                return Err(format!("no app {id}"));
            };
            s.store.settings.apps.retain(|a| a.id != id);
            s.store.settings.apps_joined.retain(|a| a != id);
            if !s.store.settings.apps_refused.contains(&status) {
                s.store.settings.apps_refused.push(status);
            }
            s.store.save_settings().map_err(|e| e.to_string())?;
            s.store.settings.apps.clone()
        };
        states::connect_all(apps);
        self.changed();
        Ok(())
    }

    pub fn settle_draft(&self, id: &str, keep: bool) -> Result<(), String> {
        let trying_it = self.lock().trying.as_ref().is_some_and(|t| t.profile == id);
        if trying_it {
            self.try_profile(None)?;
        }
        {
            let mut s = self.lock();
            s.store.settings.drafts.retain(|d| d != id);
            s.store.save_settings().map_err(|e| e.to_string())?;
            if !keep {
                s.store.delete_profile(id).map_err(|e| e.to_string())?;
            }
        }
        self.changed();
        if keep {
            self.refresh_foreground();
        }
        Ok(())
    }

    /// "Pin this profile": stop all automatic switching (true) or allow it again.
    pub fn set_pinned(&self, pinned: bool) -> Result<(), String> {
        {
            let mut s = self.lock();
            s.store.settings.pinned = pinned;
            if pinned {
                // what shows now stays; it is the home profile from here on
                s.home = s.active.clone();
                s.auto_app = None;
            }
            s.store.save_settings().map_err(|e| e.to_string())?;
        }
        self.changed();
        Ok(())
    }
}

/// The window and the tray menu follow the engine state.
fn publish_state(app: &AppHandle, state: EngineState) {
    let _ = app.emit(STATE_EVENT, state);
    crate::tray::refresh(app);
}

/// The profile that lists `exe` (a rule with a window title beats one for the app alone).
fn profile_for_app(store: &Store, exe: &str, title: &str) -> Option<String> {
    // a draft never takes over: it is only a proposal until the user picks it
    store.profiles.values().filter(|p| !store.settings.drafts.contains(&p.id)).filter_map(|p| p.app_match(exe, title).map(|rank| (rank, p))).max_by_key(|(rank, _)| *rank).map(|(_, p)| p.id.clone())
}

/// What to show when `exe` comes to the front: its own profile (taking over, `Some(exe)`), or
/// the home profile again when an app's profile was showing and this app has none (`None`).
/// Apps without a profile never switch anything; nothing switches while pinned or auto-switch is
/// off. None = stay.
fn switch_target(store: &Store, home: Option<&str>, active: Option<&str>, exe: &str, title: &str) -> Option<(String, Option<String>)> {
    if !store.settings.auto_switch || store.settings.pinned {
        return None;
    }
    match profile_for_app(store, exe, title) {
        Some(id) => Some((id, Some(exe.to_string()))),
        None => home.filter(|h| Some(*h) != active && store.profiles.contains_key(*h)).map(|h| (h.to_string(), None)),
    }
}

/// The frame of an animation that shows at `now`, from the one showing (`frame`, until `ends`):
/// frames follow the GIF's own clock, so the ones that were due while drawing and sending took
/// longer are skipped and the animation keeps the pace the window shows. After a long stall
/// (sleep, a busy PC) it goes on from `now`.
fn next_frame(delays: &[Duration], (mut frame, mut ends): (usize, Instant), now: Instant) -> (usize, Instant) {
    for _ in 0..delays.len() {
        if now < ends {
            return (frame, ends);
        }
        frame = (frame + 1) % delays.len();
        ends += delays[frame];
    }
    if now < ends { (frame, ends) } else { (frame, now + delays[frame]) }
}

/// When each step of a multi-action runs: every delay counts from the step before it.
fn schedule_steps(start: Instant, params: &serde_json::Value) -> Vec<(Instant, Binding, String)> {
    schedule_steps_list(start, actions::steps(params))
}

fn schedule_steps_list(start: Instant, steps: Vec<actions::Step>) -> Vec<(Instant, Binding, String)> {
    let mut due = start;
    steps
        .into_iter()
        .map(|step| {
            due += Duration::from_millis(step.delay);
            (due, Binding { action: step.action, params: step.params, look: Look::default(), accel: false, look_on: None, swap_turns: false }, step.when)
        })
        .collect()
}

/// Reports that turn the LEDs of a module from `old` (what was sent last; None = its own rainbow)
/// into `new`: changed colors first, then effects (one for the whole module when they are all the
/// same), then the module's effect speed.
fn leds_reports(kind: mh_protocol::ModuleKind, new: &leds::Lights, old: Option<&leds::Lights>) -> Vec<mh_protocol::report::Report> {
    let mut out = Vec::new();
    let base = kind == mh_protocol::ModuleKind::Base;
    let target = |i: usize| if base { LedTarget::All } else { LedTarget::Index(i as u8) };
    for (i, led) in new.leds.iter().enumerate() {
        if old.and_then(|o| o.leds.get(i)).map(|l| l.rgb) != Some(led.rgb) {
            out.extend(report::led_color(kind, target(i), led.rgb, false));
        }
    }
    let modes_changed = old.is_none_or(|o| o.leds.iter().map(|l| l.mode).ne(new.leds.iter().map(|l| l.mode)));
    if modes_changed && let Some(first) = new.leds.first() {
        if base || new.leds.iter().all(|l| l.mode == first.mode) {
            out.extend(report::led_mode(kind, LedTarget::All, first.mode, false));
        } else {
            for (i, led) in new.leds.iter().enumerate() {
                out.extend(report::led_mode(kind, target(i), led.mode, false));
            }
        }
    }
    if let Some(speed) = new.speed
        && old.is_none_or(|o| o.speed != Some(speed))
    {
        out.extend(report::led_speed(kind, speed));
    }
    out
}

/// What a clock or timer control shows.
#[derive(Debug, Default)]
struct LiveFace {
    main: String,
    /// The date under a clock, the label under a timer.
    second: Option<String>,
    /// A timer's time left, 0..=1.
    ring: Option<f32>,
    /// A timer that ran out.
    done: bool,
}

/// How often clocks and timers on keys and the dial are looked at.
const LIVE_EVERY: Duration = Duration::from_millis(250);

/// Local time as the panel clock shows it.
/// The color the tour points with on the hardware.
const HIGHLIGHT: &str = "#ffffff";

/// Is `input` (a control as the deck reports it, maybe on a page) the highlighted `control`?
fn highlight_matches(control: &str, input: &str) -> bool {
    input == control || input.rsplit('/').next() == Some(control)
}

fn clock_now() -> String {
    chrono::Local::now().format("%H:%M").to_string()
}

/// `kind@slot` -> rotation, from the generated table.
fn parse_slot_table() -> HashMap<String, i32> {
    let value: serde_json::Value = serde_json::from_str(SLOT_TABLE_JSON).unwrap_or_default();
    value.as_object().map(|o| o.iter().filter_map(|(k, v)| Some((k.clone(), v.get("rotation")?.as_i64()? as i32))).collect()).unwrap_or_default()
}

struct Worker {
    app: AppHandle,
    deck: Deck,
    shared: Arc<Mutex<Shared>>,
    executor: Executor,
    renderer: Renderer,
    /// Last SVG sent per screen, so unchanged keys are not re-sent.
    sent: HashMap<String, String>,
    feedback: Option<(Feedback, Instant)>,
    slot_table: HashMap<String, i32>,
    dial_dirty: bool,
    gestures: Gestures,
    /// Steps of running multi-actions, in the order they are due, with their condition.
    scheduled: VecDeque<(Instant, Binding, String)>,
    /// Last value per control (0..=1): fader positions, volumes set by knobs. Drives level LEDs.
    levels: HashMap<String, f32>,
    /// What each module's LEDs were last set to (None = rainbow), so only changes are sent.
    leds_sent: HashMap<String, Option<leds::Lights>>,
    /// Effects set by the "Lighting effect" action per module kind (`knobs`, `faders`, `base`),
    /// until the profile changes.
    lighting: HashMap<String, leds::Override>,
    /// Current mode index of each control bound to the modes action.
    modes: HashMap<String, usize>,
    /// The look the dial gallery shows (`gallery::at`).
    gallery: i64,
    /// The base edge lights up for a `notify` until then.
    notify_until: Option<Instant>,
    /// The app of the call that is on (it records the microphone), and when that was last looked at.
    in_call: Option<String>,
    call_checked: Instant,
    /// HubDeck draws a rainbow now: the LEDs get the next color every `RAINBOW_STEP`.
    rainbow_running: bool,
    rainbow_last: Instant,
    /// When each control was last turned, for acceleration.
    last_turn: HashMap<String, Instant>,
    /// Animated pictures on screen: name -> (frame shown, when the next one is due).
    frames: HashMap<String, (usize, Instant)>,
    /// The time the panel clock shows, so it redraws once a minute.
    clock_shown: String,
    /// Countdown timers per control (`keys.3`, `dial`; gestures of a key share its timer).
    timers: HashMap<String, timer::Timer>,
    /// The texts of the clocks and timers on screen, so they redraw only when one changes.
    live_sig: String,
    /// When to look at the clocks and timers again; `live_dirty` = now (a timer was pressed).
    live_next: Instant,
    live_dirty: bool,
    /// On/off state of stateful keys (`mic`, `volume:system`, `volume:app:spotify.exe`).
    states: HashMap<String, bool>,
    last_state_check: Instant,
    /// Title, artist, playing; the cover lives in `images` under `cover`.
    now_playing: Option<(String, String, bool)>,
    /// The last two minutes of system load, oldest first.
    samples: VecDeque<monitor::Sample>,
    /// On/off states reported by integrations (`obs:record`, `obs:mute:Mic/Aux`...).
    integration: HashMap<String, bool>,
    /// Values reported by integrations (`obs:scene` = the live scene).
    values: HashMap<String, String>,
    /// Titles, pictures and states set by plugins and the local API, per control id.
    plugin_looks: HashMap<String, PluginLook>,
    /// Claude Code sessions reported by the hooks.
    fleet: agents::Fleet,
    /// When the fleet keys last blinked, and when stale sessions were last dropped.
    fleet_pulse: Instant,
    /// The screensaver runs (idle minutes passed; the first touch ends it).
    saver_on: bool,
    saver_frame: Instant,
    fleet_sweep: Instant,
    /// LED colors set by plugins and the local API: `knobs.1`, `faders.0`, `base`.
    plugin_leds: HashMap<String, Rgb>,
    /// A picture for the whole key panel set by a plugin or the API (data URI).
    plugin_panel: Option<String>,
    /// Toggles that ran their first steps last (the next press runs the second).
    toggles: HashMap<String, bool>,
    /// Game mode: the deck belongs to the arcade.
    arcade: Option<games::Arcade>,
    /// Fingerprint of the profiles folder, to notice changes from another PC.
    profiles_stamp: (usize, Option<std::time::SystemTime>),
    last_folder_check: Instant,
    data_dir: std::path::PathBuf,
    /// Screens and LEDs are off (idle or locked).
    blanked: bool,
    /// The session is locked (from the system, not from the app in front).
    locked: bool,
    last_deck_input: Instant,
    /// The backlight last sent to keys and dial (None = send again, e.g. after a replug).
    backlight_sent: Option<u8>,
    last_blank_check: Instant,
    /// The tour's highlight as last drawn: the control and whether it is lit now.
    highlight_shown: Option<(String, bool)>,
}

impl Worker {
    fn lock(&self) -> MutexGuard<'_, Shared> {
        self.shared.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn run(mut self, rx: Receiver<Msg>) {
        volume::init_thread();
        let mut last_dial = Instant::now() - DIAL_FRAME;
        loop {
            let tick = if self.arcade.is_some() { Duration::from_millis(25) } else { Duration::from_millis(40) };
            // wake for the next frame of an animation on screen, not up to a whole tick later
            let now = Instant::now();
            let wait = self.frames.values().map(|(_, ends)| ends.saturating_duration_since(now)).min().map_or(tick, |d| d.clamp(Duration::from_millis(10), tick));
            let first = match rx.recv_timeout(wait) {
                Ok(m) => Some(m),
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => None,
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => return,
            };
            // take everything that queued up while the last batch ran (a dial image takes about
            // a quarter of a second over USB) and merge turns of the same control into one step,
            // so a fast spin never builds a backlog that replays old values
            let mut batch: Vec<Msg> = first.into_iter().chain(rx.try_iter()).collect();
            coalesce_turns(&mut batch);
            for msg in batch {
                match msg {
                    Msg::Input { control, event } => self.on_input(&control, event),
                    Msg::DeckChanged => {
                        if let Some(a) = self.arcade.as_mut() {
                            a.set_modules(self.deck.snapshot().modules.iter().map(|m| m.kind.to_string()).collect());
                        }
                        self.sent.clear(); // modules may have been re-attached with blank screens
                        self.leds_sent.clear();
                        self.backlight_sent = None;
                        self.redraw_keys();
                        self.update_leds();
                        self.dial_dirty = true;
                    }
                    Msg::ProfilesChanged => {
                        self.want_integrations();
                        self.redraw_keys();
                        self.update_leds();
                        self.dial_dirty = true;
                    }
                    Msg::Foreground { exe, title } => self.on_foreground(exe, title),
                    Msg::DeckUi => {
                        self.redraw_keys();
                        self.dial_dirty = true;
                    }
                    Msg::Notify { rgb, seconds } => {
                        self.notify_until = Some(Instant::now() + Duration::from_secs(seconds.clamp(1, 120)));
                        self.set_lighting(exec::Lighting { target: "base".into(), effect: Some(leds::Override { mode: LedMode::Breathing, rgb, speed: 85 }) });
                    }
                    Msg::Locked(locked) => {
                        if locked {
                            usage::switched(""); // the time in front stops while locked
                        }
                        if locked != self.locked {
                            self.locked = locked;
                            log::info!("session {}", if locked { "locked" } else { "unlocked" });
                            self.check_blank();
                        }
                    }
                    Msg::NowPlaying(track) => self.on_now_playing(track),
                    Msg::Metrics(sample) => self.on_metrics(sample),
                    Msg::IntegrationState { key, on } => {
                        if self.integration.insert(key, on) != Some(on) {
                            self.redraw_keys();
                            self.update_leds();
                            self.dial_dirty = true;
                        }
                    }
                    Msg::IntegrationValue { key, value } => {
                        if self.values.get(&key) != Some(&value) {
                            let app_state = key.starts_with(states::PREFIX);
                            self.values.insert(key, value);
                            self.redraw_keys();
                            self.dial_dirty = true;
                            // an app's item waiting for the user pulses the edge
                            if app_state {
                                self.update_leds();
                            }
                        }
                    }
                    Msg::FromPlugin { plugin, message } => self.on_plugin(&plugin, message),
                    Msg::ApiStateRequest(who) => {
                        let state = self.lock().state();
                        plugins::send_to(&who, serde_json::json!({"event": "state", "state": state}));
                    }
                    Msg::IntegrationFeedback(caption) => {
                        self.feedback = Some((Feedback { value: None, caption }, Instant::now()));
                        self.dial_dirty = true;
                    }
                    Msg::IntegrationError { source, message } => {
                        log::warn!("{source}: {message}");
                        let _ = self.app.emit(ERROR_EVENT, EngineError { control: source, message });
                    }
                    Msg::Level { control, value } => {
                        self.levels.insert(control, value as f32 / 100.0);
                        self.update_leds();
                    }
                }
            }
            let now = Instant::now();
            let games_request = self.lock().games_request.take();
            if let Some(start) = games_request {
                self.set_playing(start);
            }
            if let Some(arcade) = self.arcade.as_mut() {
                arcade.tick(now);
                if arcade.wants_to_leave() {
                    self.set_playing(false);
                } else {
                    self.redraw_keys();
                    self.update_leds();
                    self.dial_dirty = true;
                }
            }
            for (control, gesture) in self.gestures.tick(now) {
                self.fire(&format!("{control}{}", gesture.suffix()), InputEvent::Press(true));
            }
            while self.scheduled.front().is_some_and(|(due, _, _)| *due <= now) {
                let (_, step, when) = self.scheduled.pop_front().expect("checked");
                if self.condition_holds(&when) {
                    self.run_binding("multi-action", &step, InputEvent::Press(true));
                }
            }
            self.expire_deck_ui(now);
            if now.duration_since(self.call_checked) >= call::EVERY {
                self.call_checked = now;
                self.check_call();
            }
            if self.rainbow_running && now.duration_since(self.rainbow_last) >= RAINBOW_STEP {
                self.rainbow_last = now;
                self.update_leds();
            }
            self.animate(now);
            if now.duration_since(self.last_folder_check) >= Duration::from_secs(4) {
                self.last_folder_check = now;
                self.check_profiles_folder();
            }
            if now.duration_since(self.last_blank_check) >= Duration::from_millis(500) {
                self.last_blank_check = now;
                self.check_blank();
            }
            if now.duration_since(self.last_state_check) >= STATE_POLL {
                self.last_state_check = now;
                if self.refresh_states() {
                    self.redraw_keys();
                    self.update_leds();
                    self.dial_dirty = true;
                }
            }
            if self.feedback.as_ref().is_some_and(|(_, at)| at.elapsed() > FEEDBACK_FOR) {
                self.feedback = None;
                self.dial_dirty = true;
            }
            // only the newest dial state is drawn, at most every DIAL_FRAME
            // games animate the dial at up to 30 fps
            let frame = if self.arcade.is_some() { Duration::from_millis(33) } else { DIAL_FRAME };
            if self.dial_dirty && last_dial.elapsed() >= frame {
                self.dial_dirty = false;
                last_dial = Instant::now();
                self.redraw_dial();
            }
        }
    }

    fn on_input(&mut self, control: &str, event: InputEvent) {
        self.last_deck_input = Instant::now();
        if self.blanked {
            // the first touch only wakes the deck; it does not run an action
            self.blanked = false;
            self.backlight_sent = None;
            self.apply_screens();
            return;
        }
        if self.saver_on {
            // the first touch only ends the screensaver (a preview too)
            self.lock().saver_preview = None;
            self.saver_on = false;
            self.redraw_keys();
            return;
        }
        if self.lock().calibration.is_some() {
            self.on_calibration_input(control, event);
            return;
        }
        if self.arcade.is_some() {
            self.on_game_input(control, event);
            return;
        }
        // the control the tour points at was found: it stops blinking
        {
            let mut s = self.lock();
            if s.highlight.as_ref().is_some_and(|(c, _, _)| highlight_matches(c, control)) {
                s.highlight = None;
            }
        }
        // what is touched while a draft is tried (for the assistant), and, if the user allowed
        // it, how often each control is used
        if matches!(event, InputEvent::Press(true) | InputEvent::Turn(_)) {
            let (profile, trying, counting) = {
                let s = self.lock();
                let trying = s.trying.as_ref().map(|t| t.profile.clone());
                (trying.clone().or_else(|| s.active.clone()).unwrap_or_default(), trying, s.store.settings.count_presses)
            };
            presses::record(&profile, control, trying.as_deref(), counting);
        }
        // comparing drafts: the dial flips between them (its own job waits until one is picked)
        let comparing = self.lock().draft_compare.as_ref().map(|(ids, i)| (ids.len(), *i));
        if let Some((n, i)) = comparing
            && control == "dial"
        {
            if let InputEvent::Turn(d) = event {
                let next = (i as i64 + d.signum() as i64).rem_euclid(n as i64) as usize;
                if let Some(c) = self.lock().draft_compare.as_mut() {
                    c.1 = next;
                }
                let engine = self.app.state::<Engine>();
                if let Err(e) = engine.show_compared() {
                    log::warn!("compare: {e}");
                }
            }
            return;
        }
        // a question on the deck: a key with an option answers it, a dial press dismisses it
        let asking = self.lock().prompt.as_ref().filter(|p| p.waiting()).map(|p| p.options.len());
        if let Some(n) = asking
            && (control.starts_with("keys.") || control == "dial")
        {
            if let InputEvent::Press(true) = event {
                let choice = control.strip_prefix("keys.").and_then(|i| i.parse::<usize>().ok());
                let answer = match choice {
                    Some(i) if i < n => Some(Some(i)),
                    Some(_) => None, // a key without an option
                    None => Some(None),
                };
                if let Some(answer) = answer {
                    if let Some(p) = self.lock().prompt.as_mut() {
                        p.answer = Some(answer);
                    }
                    self.redraw_keys();
                    self.dial_dirty = true;
                }
            }
            return;
        }
        let id = {
            let s = self.lock();
            control_id(s.page.as_deref(), control)
        };
        // scripts on the local API hear every control
        let (kind, value) = match event {
            InputEvent::Press(down) => (if down { "down" } else { "up" }, serde_json::Value::Null),
            InputEvent::Turn(d) => ("turn", serde_json::json!(d)),
            InputEvent::Slide(v) => ("slide", serde_json::json!(v)),
        };
        plugins::send_to("api", serde_json::json!({"event": "input", "control": id, "kind": kind, "value": value}));
        match event {
            // held notes and plugin actions also hear the release
            InputEvent::Press(false) if self.lock().active_profile().and_then(|p| p.controls.get(&id)).is_some_and(|b| b.action == "midi.note" || plugins::is_plugin_action(&b.action)) => {
                self.gestures.press(&id, false, Instant::now(), Bound::default());
                self.fire(&id, InputEvent::Press(false));
            }
            // push-to-talk / push-to-mute act at once on both edges; hold and double press do not apply
            InputEvent::Press(down) if self.lock().active_profile().and_then(|p| p.controls.get(&id)).is_some_and(|b| actions::while_held(&b.action, &b.params)) => {
                self.gestures.press(&id, down, Instant::now(), Bound::default());
                self.fire(&id, InputEvent::Press(down));
            }
            InputEvent::Press(down) => {
                let bound =
                    self.lock().active_profile().map(|p| Bound { hold: p.controls.contains_key(&format!("{id}:hold")), double: p.controls.contains_key(&format!("{id}:double")) }).unwrap_or_default();
                if let Some(gesture) = self.gestures.press(&id, down, Instant::now(), bound) {
                    self.fire(&format!("{id}{}", gesture.suffix()), InputEvent::Press(true));
                }
            }
            InputEvent::Turn(d) => {
                let accel = self.lock().active_profile().and_then(|p| p.controls.get(&id)).is_some_and(|b| b.accel);
                let now = Instant::now();
                let factor = match self.last_turn.insert(id.clone(), now) {
                    Some(last) if accel => accel_factor(now.duration_since(last)),
                    _ => 1,
                };
                self.fire(&id, InputEvent::Turn(d.saturating_mul(factor)));
            }
            InputEvent::Slide(raw) => {
                let last = self.levels.get(&id).map(|l| (l * 100.0).round() as u8);
                let Some(v) = fader_filter(last, raw) else { return };
                self.levels.insert(id.clone(), v as f32 / 100.0);
                self.fire(&id, InputEvent::Slide(v));
                self.update_leds();
            }
        }
    }

    /// Run the binding stored under `id` (a control id, maybe with a gesture suffix).
    fn fire(&mut self, id: &str, event: InputEvent) {
        let Some(binding) = self.lock().active_profile().and_then(|p| p.controls.get(id).cloned()) else { return };
        if binding.action == actions::GAMES {
            if matches!(event, InputEvent::Press(true)) {
                self.set_playing(true);
            }
            return;
        }
        if binding.action == actions::TOGGLE {
            if matches!(event, InputEvent::Press(true)) {
                let on = self.toggles.get(id).copied().unwrap_or(false);
                let list = actions::steps_at(&binding.params, if on { "second" } else { "first" });
                self.scheduled.extend(schedule_steps_list(Instant::now(), list));
                self.scheduled.make_contiguous().sort_by_key(|(due, _, _)| *due);
                self.toggles.insert(id.to_string(), !on);
                self.redraw_keys();
                self.dial_dirty = true;
            }
            return;
        }
        if binding.action == actions::MULTI {
            // queued, not slept: the engine keeps drawing and reacting while the steps wait
            self.scheduled.extend(schedule_steps(Instant::now(), &binding.params));
            self.scheduled.make_contiguous().sort_by_key(|(due, _, _)| *due);
            return;
        }
        if plugins::is_plugin_action(&binding.action) {
            plugins::input(&binding.action, id, &binding.params, event, binding.swap_turns);
            // a Stream Deck two-state action flips on key-up, as Stream Deck does
            if event == InputEvent::Press(false)
                && let Some(on) = sdplugins::toggle_after_key_up(&binding.action, id)
            {
                self.plugin_looks.entry(id.to_string()).or_default().state = Some(on);
                self.redraw_keys();
            }
            return;
        }
        if binding.action == gallery::ACTION {
            match event {
                InputEvent::Turn(d) => self.gallery += i64::from(d.signum()),
                InputEvent::Press(true) => self.gallery += 1,
                _ => return,
            }
            self.dial_dirty = true;
            return;
        }
        if binding.action == actions::MODES {
            let modes = actions::modes(&binding.params);
            if modes.is_empty() {
                return;
            }
            let current = self.modes.get(id).copied().unwrap_or(0) % modes.len();
            match event {
                InputEvent::Press(true) => {
                    let next = (current + 1) % modes.len();
                    self.modes.insert(id.to_string(), next);
                    self.feedback = Some((Feedback { value: None, caption: modes[next].label.clone() }, Instant::now()));
                    self.dial_dirty = true;
                }
                InputEvent::Turn(_) | InputEvent::Slide(_) => {
                    let m = &modes[current];
                    let step = Binding { action: m.action.clone(), params: m.params.clone(), look: Look::default(), accel: false, look_on: None, swap_turns: false };
                    self.run_binding(id, &step, event);
                }
                InputEvent::Press(false) => {}
            }
            return;
        }
        self.run_binding(id, &binding, event);
    }

    fn run_binding(&mut self, id: &str, binding: &Binding, event: InputEvent) {
        // the tour: going live, recording or ending something only says so on the dial
        let dry = {
            let s = self.lock();
            s.tour_dry_run.then(|| tour::dry_run_says(&binding.action, s.store.settings.language.starts_with("pl"))).flatten()
        };
        if let Some(says) = dry {
            self.feedback = Some((Feedback { value: None, caption: says.to_string() }, Instant::now()));
            self.dial_dirty = true;
            return;
        }
        if binding.action == actions::AGENT || binding.action == actions::AGENTS {
            self.agent_input(binding, event);
            return;
        }
        let (foreground, mapped) = {
            let s = self.lock();
            // apps another control sets: "every other app" leaves them alone
            let mapped = s
                .active_profile()
                .map(|p| {
                    p.controls.values().filter(|b| b.action == "core.volume" && actions::param_str(&b.params, "target") == "app").map(|b| actions::param_str(&b.params, "app").to_string()).collect()
                })
                .unwrap_or_default();
            (s.foreground.clone(), mapped)
        };
        self.executor.foreground = foreground;
        self.executor.mapped = mapped;
        match self.executor.run(binding, event) {
            Ok(out) => {
                if let Some(page) = out.page {
                    self.show_page((!page.is_empty()).then_some(page));
                }
                if let Some(profile) = out.switch_profile {
                    self.choose(&profile);
                }
                if let Some(step) = out.profile_step {
                    let target = {
                        let s = self.lock();
                        let mut list: Vec<(String, String)> = s.store.profiles.values().map(|p| (p.name.to_lowercase(), p.id.clone())).collect();
                        list.sort();
                        let ordered: Vec<String> = list.into_iter().map(|(_, id)| id).collect();
                        exec::step_profile(&ordered, s.active.as_deref(), step)
                    };
                    if let Some(id) = target {
                        self.choose(&id);
                        let name = self.lock().store.profiles.get(&id).map(|p| p.name.clone()).unwrap_or_default();
                        self.feedback = Some((Feedback { value: None, caption: name }, Instant::now()));
                        self.dial_dirty = true;
                    }
                }
                if let Some(cmd) = out.lighting {
                    self.set_lighting(cmd);
                }
                if let Some(cmd) = out.timer {
                    let control = id.split(':').next().unwrap_or(id).to_string();
                    let total = timer::total_of(&binding.params);
                    let now = Instant::now();
                    let t = self.timers.entry(control).or_insert_with(|| timer::Timer::new(total));
                    if !t.running() && t.total != total && !matches!(cmd, timer::TimerCmd::Adjust(_)) && t.remaining(now) == t.total {
                        // its length was changed in the editor since
                        *t = timer::Timer::new(total);
                    }
                    t.apply(cmd, now);
                    self.live_sig.clear();
                    self.live_dirty = true;
                    self.update_leds();
                }
                if let Some(change) = out.deck_brightness {
                    let level = {
                        let mut s = self.lock();
                        let level = change.apply(s.store.settings.screen_brightness);
                        s.store.settings.screen_brightness = level;
                        if let Err(e) = s.store.save_settings() {
                            log::warn!("saving the brightness: {e}");
                        }
                        level
                    };
                    self.apply_screens();
                    self.feedback = Some((Feedback { value: Some(level as f32 / 100.0), caption: "Brightness".into() }, Instant::now()));
                    self.dial_dirty = true;
                }
                // a mute changed: show it on the key now, not at the next poll
                if Self::state_key(binding).is_some() && matches!(event, InputEvent::Press(_)) && self.refresh_states() {
                    self.redraw_keys();
                }
                if let Some(fb) = out.feedback {
                    if let Some(v) = fb.value {
                        // a knob that set a volume shows it on its level LED
                        let control = id.split(':').next().unwrap_or(id);
                        self.levels.insert(control.to_string(), v);
                        self.update_leds();
                    }
                    self.feedback = Some((fb, Instant::now()));
                    self.dial_dirty = true;
                }
            }
            Err(message) => {
                log::warn!("{id}: {message}");
                let _ = self.app.emit(ERROR_EVENT, EngineError { control: id.into(), message });
            }
        }
    }

    fn show_page(&mut self, page: Option<String>) {
        {
            let mut s = self.lock();
            let exists = page.as_ref().is_none_or(|p| s.active_profile().is_some_and(|prof| prof.pages.iter().any(|x| &x.id == p)));
            if !exists || s.page == page {
                return;
            }
            s.page = page;
        }
        self.gestures.clear();
        self.redraw_keys();
        self.report_visible();
        let state = self.lock().state();
        publish_state(&self.app, state);
    }

    /// While calibrating, the dial (or knob 1) adjusts the geometry: turn changes the selected
    /// parameter by one pixel per detent, press selects the next parameter. Other input is ignored.
    fn on_calibration_input(&mut self, control: &str, event: InputEvent) {
        let changed = {
            let mut s = self.lock();
            let Some(cal) = s.calibration.as_mut() else { return };
            match (control, event) {
                ("dial" | "knobs.0", InputEvent::Turn(d)) => {
                    cal.geometry.adjust(cal.selected, d as f32);
                    true
                }
                ("dial" | "knobs.0", InputEvent::Press(true)) => {
                    cal.selected = (cal.selected + 1) % render::GEOMETRY_PARAMS.len();
                    true
                }
                _ => false,
            }
        };
        if changed {
            let payload = self.lock().calibration.clone();
            let _ = self.app.emit(CALIBRATION_EVENT, payload);
            self.redraw_keys();
            self.dial_dirty = true;
        }
    }

    fn on_foreground(&mut self, exe: String, title: String) {
        // the lock screen switches no profile; the screens follow `Msg::Locked`, because on the lock
        // screen other windows come to the front for a moment and made the screens flicker
        if self.locked || idle::is_lock_screen(&exe) {
            usage::switched("");
            return;
        }
        usage::switched(&exe);
        if self.lock().calibration.is_some() {
            return; // stay put while calibrating
        }
        let (target, same_app) = {
            let mut s = self.lock();
            let same_app = s.foreground == exe;
            s.foreground = exe.clone();
            // during a call the deck stays on the call's profile
            // during a call or while a draft is tried the deck stays where it is
            let target = if s.call_hold || s.trying.is_some() { None } else { switch_target(&s.store, s.home.as_deref(), s.active.as_deref(), &exe, &title) };
            (target, same_app)
        };
        if !same_app && target.is_none() {
            auto::on_foreground(&self.app, &exe, &title);
        }
        // only the title changed and it changes nothing: stay quiet (some apps retitle every second)
        if same_app && target.as_ref().map(|(id, _)| id.as_str()) == self.lock().active.as_deref() {
            return;
        }
        if let Some((_, app)) = &target {
            self.lock().auto_app = app.clone();
        }
        match target.map(|(id, _)| id) {
            Some(id) if self.lock().active.as_deref() != Some(id.as_str()) => {
                let t = Instant::now();
                self.activate(&id);
                let name = self.lock().store.profiles.get(&id).map(|p| p.name.clone()).unwrap_or_default();
                log::info!("foreground {exe} -> profile \"{name}\" drawn in {:?}", t.elapsed());
            }
            _ => {
                let state = self.lock().state();
                publish_state(&self.app, state);
            }
        }
    }

    /// The user picked a profile (on the deck, by a plugin): it becomes home, then shows.
    fn choose(&mut self, id: &str) {
        {
            let mut s = self.lock();
            if !s.store.profiles.contains_key(id) {
                return;
            }
            s.home = Some(id.to_string());
            s.auto_app = None;
        }
        self.activate(id);
    }

    fn activate(&mut self, id: &str) {
        let changed = {
            let mut s = self.lock();
            if !s.store.profiles.contains_key(id) {
                return;
            }
            let changed = s.active.as_deref() != Some(id);
            if changed {
                s.page = None;
            }
            s.active = Some(id.to_string());
            changed
        };
        // effects from the "Lighting effect" action last until the profile changes
        if changed {
            self.lighting.clear();
        }
        self.gestures.clear();
        self.redraw();
        self.update_leds();
        self.want_integrations();
        let state = self.lock().state();
        publish_state(&self.app, state);
    }

    /// Clockwise degrees to turn a module's image so it reads upright on the desk: undo the
    /// module's own rotation and the base's turn (the same rule the settings window uses).
    fn content_rotation(&self, kind: &str, slot: Option<&str>, base_turn: i32, user: &BTreeMap<String, crate::placements::Placement>) -> i32 {
        let rotation = slot.map(|s| format!("{kind}@{s}")).and_then(|key| user.get(&key).map(|p| p.rotation as i32).or_else(|| self.slot_table.get(&key).copied())).unwrap_or(0);
        (-(rotation + base_turn)).rem_euclid(360)
    }

    fn redraw(&mut self) {
        self.redraw_keys();
        self.dial_dirty = true;
    }

    fn redraw_keys(&mut self) {
        self.publish_looks();
        let snapshot = self.deck.snapshot();
        let user = self.app.state::<Placements>().all();
        let (profile, page) = {
            let s = self.lock();
            (s.active_profile().cloned(), s.page.clone())
        };
        let (geometry, calibrating, compare) = {
            let s = self.lock();
            match &s.calibration {
                Some(c) => (c.geometry, s.compare.is_none(), s.compare),
                None => (s.store.settings.panel, false, s.compare),
            }
        };
        for m in snapshot.modules.iter().filter(|m| m.kind == "keys") {
            let Some(module) = self.deck.module(&m.id) else { continue };
            let rotation = self.content_rotation(m.kind, m.slot.as_deref(), snapshot.turn, &user);
            let mut looks: Vec<Look> = (0..15)
                .map(|i| {
                    let mut look = profile
                        .as_ref()
                        .and_then(|p| p.controls.get(&control_id(page.as_deref(), &format!("keys.{i}"))))
                        .map(|b| {
                            let id = control_id(page.as_deref(), &format!("keys.{i}"));
                            let state = self
                                .plugin_looks
                                .get(&id)
                                .and_then(|p| p.state)
                                .or_else(|| (b.action == actions::TOGGLE).then(|| self.toggles.get(&id).copied().unwrap_or(false)))
                                .or_else(|| self.state_of(b));
                            // a Stream Deck action's state picks its picture, it is not HubDeck's red "on"
                            let shown_state = if sdplugins::is_sd_action(&b.action) { None } else { state };
                            let look = self.plugin_look(&id, self.now_playing_look(b, b.look_for(shown_state)));
                            let look = self.sd_look(&id, b, look);
                            if b.action == actions::AGENT {
                                let slot = (actions::param_f64(&b.params, "slot", 1.0).max(1.0) as usize) - 1;
                                return self.fleet.key_look(slot, &look, Instant::now(), self.fleet_sweep);
                            }
                            if b.action == live::ACTION {
                                return live::key_look(&b.params, look, live::value_key(&b.params).and_then(|k| self.values.get(&k)).map(String::as_str));
                            }
                            if b.action == states::ACTION {
                                let blink = (Instant::now().duration_since(self.fleet_sweep).as_millis() / agents::PULSE.as_millis()) % 2 == 1;
                                return states::key_look(&b.params, look, &self.values, blink);
                            }
                            match (self.monitor_value(b), self.live_face(&id, b, Instant::now())) {
                                // a monitor key: the metric and its value instead of the label
                                (Some((metric, value)), _) => {
                                    Look { label: Some(format!("{} {}", metric.name(), value.map(|v| metric.format(v)).unwrap_or_else(|| "–".into()))), icon: None, ..look }
                                }
                                // a clock or timer: its text, big; a timer that ran out turns red
                                (None, Some(face)) => {
                                    let base = if face.done { b.look_for(Some(true)) } else { look };
                                    let label = match face.second {
                                        Some(s) => format!("{}\n{s}", face.main),
                                        None => face.main,
                                    };
                                    Look { label: Some(label), icon: None, image: None, face: true, ..base }
                                }
                                (None, None) => look,
                            }
                        })
                        .unwrap_or(Look { bg: Some("#000000".into()), ..Look::default() });
                    self.current_frame(&mut look);
                    look
                })
                .collect();
            // a question: its options on the first keys, the other keys dark
            if let Some(prompt) = self.lock().prompt.clone().filter(Prompt::waiting) {
                looks = (0..15)
                    .map(|i| match prompt.options.get(i) {
                        Some(option) => Look { label: Some(option.clone()), bg: Some("#2b2540".into()), fg: Some("#ffffff".into()), ..Look::default() },
                        None => Look { bg: Some("#000000".into()), ..Look::default() },
                    })
                    .collect();
            }
            // the tour points at a key: it turns white (the label and icon stay readable)
            if let Some(look) = self.lit_highlight().and_then(|c| c.strip_prefix("keys.")?.parse::<usize>().ok()).and_then(|i| looks.get_mut(i)) {
                *look = Look { bg: Some(HIGHLIGHT.into()), fg: Some("#111018".into()), icon_color: None, style: None, image: None, ..look.clone() };
            }
            let game_screen = self.arcade.as_ref().map(|a| a.screen(Instant::now()));
            // the screensaver: one frame of its effect, in the deck's light color
            let game_panel = game_screen.as_ref().and_then(|g| g.panel.clone()).or_else(|| {
                self.saver_on
                    .then(|| {
                        let s = self.lock();
                        let effect = s.saver_preview.as_ref().filter(|(_, until)| *until > Instant::now()).map_or(s.store.settings.saver.as_str(), |(e, _)| e.as_str());
                        saver::body(effect, saver::clock(), &s.store.settings.lights.color)
                    })
                    .flatten()
            });
            let looks = match game_screen {
                Some(g) => g.keys,
                None => looks,
            };
            // a plugin's panel picture wins over the profile's
            let plugin_panel = self.plugin_panel.clone().filter(|_| self.arcade.is_none()).map(|image| profile::PanelContent::Picture { image });
            let panel_content = plugin_panel.or_else(|| profile.as_ref().filter(|_| self.arcade.is_none()).and_then(|p| p.panel.clone())).map(|c| match c {
                profile::PanelContent::Picture { image } => {
                    let mut look = Look { image: Some(image), ..Look::default() };
                    self.current_frame(&mut look);
                    profile::PanelContent::Picture { image: look.image.unwrap_or_default() }
                }
                other => other,
            });
            if compare == Some(CompareVariant::A) {
                // the first version: one 72x72 image per key
                for (i, look) in looks.iter().enumerate() {
                    let svg = render::key_svg(look, rotation);
                    if let Err(e) = self.renderer.jpeg_with(&svg, Some(85)).and_then(|jpeg| module.send_all(&report::key_image(i as u8, &jpeg, module.output_len)).map_err(|e| e.to_string())) {
                        log::warn!("draw key {i}: {e}");
                    }
                }
                self.sent.remove(&m.id);
                continue;
            }
            let quality = (compare == Some(CompareVariant::C)).then_some(100);
            // one full-panel image: ~11 ms over USB and all keys change at once
            let svg = match (&panel_content, calibrating) {
                (_, true) => render::calibration_svg(&geometry),
                _ if game_panel.is_some() => render::panel_body_svg(game_panel.as_deref().unwrap_or(""), rotation),
                (Some(content), false) => {
                    self.clock_shown = clock_now();
                    let history: Vec<f32> = match content {
                        profile::PanelContent::Graph { metric } => self.samples.iter().filter_map(|s| s.get(*metric)).collect(),
                        _ => Vec::new(),
                    };
                    render::panel_content_svg(content, rotation, &self.clock_shown, &history)
                }
                (None, false) => render::panel_svg(&looks, rotation, &geometry),
            };
            let svg = if quality.is_some() { format!("{svg}<!--q100-->") } else { svg };
            if self.sent.get(&m.id) == Some(&svg) {
                continue;
            }
            let send_started = Instant::now();
            let result = self.renderer.jpeg_with(&svg, quality).and_then(|jpeg| {
                let t = Instant::now();
                let r = module.send_all(&report::panel_image(&jpeg, module.output_len)).map_err(|e| e.to_string());
                log::debug!("panel: {} B sent in {:?}", jpeg.len(), t.elapsed());
                r
            });
            log::debug!("panel total {:?}", send_started.elapsed());
            match result {
                Ok(()) => {
                    self.sent.insert(m.id.clone(), svg);
                }
                Err(e) => log::warn!("draw keys: {e}"),
            }
        }
    }

    fn on_now_playing(&mut self, track: Option<media::Track>) {
        match track {
            Some(t) => {
                log::info!(
                    "now playing: {} - {} ({}, cover {})",
                    t.artist,
                    t.title,
                    if t.playing { "playing" } else { "paused" },
                    t.cover.as_ref().map_or("none".to_string(), |(m, b)| format!("{m} {} B", b.len()))
                );
                let same = self.now_playing.as_ref().is_some_and(|(title, artist, _)| *title == t.title && *artist == t.artist);
                // a play/pause change of the same track comes without the cover: keep the one we have
                if !same || t.cover.is_some() {
                    images::set_cover(t.cover.as_ref().map(|(m, b)| (m.as_str(), b.as_slice())));
                }
                self.now_playing = Some((t.title, t.artist, t.playing));
            }
            None => {
                images::set_cover(None);
                self.now_playing = None;
            }
        }
        let shown = {
            let s = self.lock();
            let page = s.page.clone();
            s.active_profile().is_some_and(|p| {
                p.controls.get("dial").is_some_and(|b| b.action == actions::NOW_PLAYING)
                    || (0..15).any(|i| p.controls.get(&control_id(page.as_deref(), &format!("keys.{i}"))).is_some_and(|b| b.action == actions::NOW_PLAYING))
            })
        };
        if shown {
            self.redraw_keys();
            self.dial_dirty = true;
        }
    }

    fn on_metrics(&mut self, sample: monitor::Sample) {
        self.samples.push_back(sample);
        while self.samples.len() > monitor::HISTORY {
            self.samples.pop_front();
        }
        let (keys, dial) = {
            let s = self.lock();
            let page = s.page.clone();
            match s.active_profile() {
                Some(p) => (
                    matches!(p.panel, Some(profile::PanelContent::Graph { .. }))
                        || (0..15).any(|i| p.controls.get(&control_id(page.as_deref(), &format!("keys.{i}"))).is_some_and(|b| b.action == actions::MONITOR)),
                    p.controls.get("dial").is_some_and(|b| b.action == actions::MONITOR),
                ),
                None => (false, false),
            }
        };
        if keys {
            self.redraw_keys();
        }
        if dial {
            self.dial_dirty = true;
        }
    }

    /// The metric a "System monitor" binding shows, with its latest value.
    fn monitor_value(&self, binding: &Binding) -> Option<(monitor::Metric, Option<f32>)> {
        (binding.action == actions::MONITOR).then(|| {
            let metric = monitor::Metric::from_param(actions::param_str(&binding.params, "metric"));
            (metric, self.samples.back().and_then(|s| s.get(metric)))
        })
    }

    /// What a clock or timer shows now; None for other actions.
    fn live_face(&self, control: &str, binding: &Binding, now: Instant) -> Option<LiveFace> {
        let p = &binding.params;
        match binding.action.as_str() {
            actions::CLOCK => {
                let (main, second) = timer::clock_text(chrono::Local::now().naive_local(), actions::param_str(p, "show"), actions::param_str(p, "hours"));
                Some(LiveFace { main, second, ring: None, done: false })
            }
            actions::TIMER => {
                let fresh = timer::Timer::new(timer::total_of(p));
                let t = self.timers.get(control).unwrap_or(&fresh);
                let caption = binding.look.label.clone().filter(|l| !l.trim().is_empty());
                Some(LiveFace { main: t.label(now), second: caption, ring: Some(t.fraction(now)), done: t.finished(now) && t.running() })
            }
            _ => None,
        }
    }

    /// A key bound to "Now playing" shows the cover and the title.
    fn now_playing_look(&self, binding: &Binding, look: Look) -> Look {
        match (&self.now_playing, binding.action == actions::NOW_PLAYING) {
            (Some((title, _, _)), true) => Look { image: images::uri(images::COVER_REF).map(|_| images::COVER_REF.to_string()), label: Some(title.clone()), icon: Some("music".into()), ..look },
            _ => look,
        }
    }

    fn on_plugin(&mut self, plugin: &str, m: serde_json::Value) {
        let context = m["context"].as_str().or(m["control"].as_str()).unwrap_or("").to_string();
        let mut look = self.plugin_looks.get(&context).cloned().unwrap_or_default();
        match m["event"].as_str().unwrap_or("") {
            "setTitle" => look.title = m["title"].as_str().map(str::to_string),
            "flash" => {
                // Stream Deck's showOk / showAlert: a short sign on the key would need its own timer
                log::debug!("plugin {plugin}: {} on {context}", if m["ok"] == true { "ok" } else { "alert" });
                return;
            }
            "openUrl" => {
                if let Some(url) = m["url"].as_str().filter(|u| u.starts_with("https://") || u.starts_with("http://"))
                    && !crate::automation::blocks_opening(url)
                {
                    let _ = tauri_plugin_opener::open_url(url, None::<&str>);
                }
                return;
            }
            "sdSettings" => {
                // a Stream Deck plugin or its settings page saved the control's settings
                let settings = m["settings"].clone();
                let saved = {
                    let mut s = self.lock();
                    let active = s.active.clone();
                    match active.and_then(|id| s.store.profiles.get(&id).cloned()) {
                        Some(mut p) => match p.controls.get_mut(&context) {
                            Some(b) if b.params != settings => {
                                b.params = settings;
                                s.store.save_profile(p).is_ok()
                            }
                            _ => false,
                        },
                        None => false,
                    }
                };
                if saved {
                    let state = self.lock().state();
                    publish_state(&self.app, state);
                }
                return;
            }
            "sdFeedback" => {
                // a layout's items by their keys: the big text, the title, a bar (docs/STREAMDECK-PLUGINS.md)
                let (value, title, bar) = sdplugins::feedback_parts(&m["feedback"]);
                if let Some(cover) = sdplugins::feedback_cover(&m["feedback"]) {
                    look.cover = Some(cover);
                } else if m["feedback"].get("icon").is_some() {
                    // the layout's picture went back to a glyph: no cover any more
                    look.cover = None;
                }
                let old = look.feedback.clone().unwrap_or_default();
                look.feedback = Some((bar.or(old.0), value.unwrap_or(old.1), title.unwrap_or(old.2)));
                // knobs have no screen: what they set shows on the dial, but only while they are turned
                // or pressed (a song's clock ticking on a knob must not take the dial)
                if context != "dial" && !context.ends_with("/dial") && sdplugins::touched_lately(&context) {
                    let (v, main, t) = look.feedback.clone().unwrap_or_default();
                    // the user's own label for the knob names it on the dial ("Spotify 45%")
                    let own = self.lock().active_profile().and_then(|p| p.controls.get(&context)).and_then(|b| b.look.label.clone()).filter(|l| !l.trim().is_empty());
                    let t = own.unwrap_or(t);
                    let caption = [t, main].into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(" ");
                    self.feedback = Some((Feedback { value: v, caption }, Instant::now()));
                }
                self.dial_dirty = true;
            }
            "setImage" => look.image = m["image"].as_str().filter(|i| images::safe_data_uri(i)).map(str::to_string),
            "setState" => look.state = m["state"].as_bool(),
            "showFeedback" => {
                let caption = m["caption"].as_str().unwrap_or("").to_string();
                self.feedback = Some((Feedback { value: m["value"].as_f64().map(|v| v as f32), caption }, Instant::now()));
                self.dial_dirty = true;
                return;
            }
            "switchProfile" => {
                if let Some(id) = m["profile"].as_str() {
                    let id = self.lock().store.profiles.values().find(|p| p.id == id || p.name == id).map(|p| p.id.clone());
                    match id {
                        Some(id) => self.choose(&id),
                        None => plugins::send_to(plugin, serde_json::json!({"event": "error", "message": "no such profile"})),
                    }
                }
                return;
            }
            "log" => {
                log::info!("plugin {plugin}: {}", m["message"].as_str().unwrap_or(""));
                return;
            }
            "setLed" => {
                let color = m["color"].as_str().and_then(leds::parse_hex);
                match color {
                    Some(c) => self.plugin_leds.insert(context.clone(), c),
                    None => self.plugin_leds.remove(&context),
                };
                self.update_leds();
                return;
            }
            "setPanelImage" => {
                self.plugin_panel = m["image"].as_str().filter(|i| images::safe_data_uri(i)).map(str::to_string);
                self.redraw_keys();
                return;
            }
            "agent" => {
                if self.fleet.report(&m, Instant::now()) {
                    self.redraw_keys();
                    self.update_leds();
                    self.dial_dirty = true;
                }
                return;
            }
            "playGames" => {
                self.set_playing(m["on"].as_bool().unwrap_or(true));
                return;
            }
            other => {
                plugins::send_to(plugin, serde_json::json!({"event": "error", "message": format!("unknown event {other:?}")}));
                return;
            }
        }
        if context.is_empty() {
            return;
        }
        if self.plugin_looks.get(&context) != Some(&look) {
            self.plugin_looks.insert(context, look);
            self.redraw_keys();
            self.dial_dirty = true;
        }
    }

    /// A control's look with what a plugin or the local API set on it.
    fn plugin_look(&self, id: &str, look: Look) -> Look {
        match self.plugin_looks.get(id) {
            Some(p) => Look { label: p.title.clone().or(look.label), image: p.image.clone().or(look.image), ..look },
            None => look,
        }
    }

    /// What plugins show on the active profile's controls, for the window's drawing of the deck
    /// (it draws saved looks; these change while HubDeck runs). Sent when it changes.
    fn publish_looks(&mut self) {
        let profile = self.lock().active_profile().cloned();
        let mut now: HashMap<String, Look> = HashMap::new();
        let id = profile.as_ref().map(|p| p.id.clone()).unwrap_or_default();
        if let Some(p) = profile {
            for (id, b) in p.controls.iter().filter(|(_, b)| plugins::is_plugin_action(&b.action)) {
                let look = self.sd_look(id, b, self.plugin_look(id, b.look_for(None)));
                now.insert(id.clone(), look);
            }
        }
        let mut sent = LIVE_LOOKS.lock().unwrap_or_else(|e| e.into_inner());
        if sent.0 != id || sent.1 != now {
            *sent = (id.clone(), now.clone());
            let _ = self.app.emit("engine://looks", serde_json::json!({"profile": id, "looks": now}));
        }
    }

    /// A Stream Deck action's look: what its plugin set, else its manifest's picture and title for
    /// the current state (the whole key, as on a Stream Deck).
    fn sd_look(&self, id: &str, binding: &Binding, look: Look) -> Look {
        let Some((image, title)) = sdplugins::state_look(&binding.action, self.plugin_looks.get(id).and_then(|p| p.state)) else { return look };
        let set = self.plugin_looks.get(id);
        let image = set.and_then(|p| p.image.clone()).or(look.image.clone()).or(image);
        let label = set.and_then(|p| p.title.clone()).or(look.label.clone()).or(title);
        let fit = if image.is_some() && look.fit.is_none() { Some("fill".to_string()) } else { look.fit.clone() };
        Look { image, label, fit, icon: None, ..look }
    }

    /// Tell plugins which of their controls are on screen now.
    fn report_visible(&mut self) {
        let (profile, page) = {
            let s = self.lock();
            (s.active_profile().cloned(), s.page.clone())
        };
        let mut now = HashMap::new();
        if let Some(p) = profile {
            for (id, b) in &p.controls {
                let base = id.split(':').next().unwrap_or(id);
                let on_page = match base.split_once('/') {
                    Some((pg, _)) => page.as_deref() == Some(pg),
                    None => !(base.starts_with("keys.") && page.is_some()),
                };
                if let (true, Some((plugin, action))) = (on_page, b.action.split_once('/')) {
                    now.insert(id.clone(), (plugin.to_string(), action.to_string(), b.params.clone()));
                }
            }
        }
        // what a plugin set on a control that now shows something else is gone with it
        for gone in plugins::set_visible(now) {
            self.plugin_looks.remove(&gone);
        }
    }

    /// Start or end game mode. Ending gives the deck back to the profile.
    fn set_playing(&mut self, on: bool) {
        if on == self.arcade.is_some() || (on && !games::GAMES_ENABLED) {
            return;
        }
        self.arcade = on.then(|| {
            let mut a = games::Arcade::new(&self.data_dir);
            a.set_modules(self.deck.snapshot().modules.iter().map(|m| m.kind.to_string()).collect());
            a
        });
        self.lock().playing = on;
        self.gestures.clear();
        self.sent.clear();
        self.leds_sent.clear();
        self.redraw_keys();
        self.update_leds();
        self.dial_dirty = true;
        let state = self.lock().state();
        publish_state(&self.app, state);
        log::info!("game mode {}", if on { "on" } else { "off" });
    }

    fn on_game_input(&mut self, control: &str, event: InputEvent) {
        let now = Instant::now();
        let Some(arcade) = self.arcade.as_mut() else { return };
        let index = |c: &str| c.rsplit('.').next().and_then(|n| n.parse::<usize>().ok()).unwrap_or(0);
        match (control.split('.').next().unwrap_or(""), event) {
            ("dial", InputEvent::Press(down)) => arcade.dial_press(down, now),
            ("dial", InputEvent::Turn(d)) => arcade.input(games::Input::Dial(d), now),
            ("keys", InputEvent::Press(down)) => arcade.input(games::Input::Key { index: index(control), down }, now),
            ("knobs", InputEvent::Turn(d)) => arcade.input(games::Input::Knob(index(control), d), now),
            ("knobs", InputEvent::Press(true)) => arcade.input(games::Input::DialClick, now),
            ("rollers", InputEvent::Turn(d)) => arcade.input(games::Input::Roller(index(control), d), now),
            ("faders", InputEvent::Slide(v)) => arcade.input(games::Input::Fader(index(control), v), now),
            _ => {}
        }
        self.redraw_keys();
        self.dial_dirty = true;
    }

    /// Keep the OBS connection open while the active profile has OBS controls.
    fn want_integrations(&mut self) {
        self.report_visible();
        let profile = self.lock().active_profile().cloned();
        let uses_obs = profile.as_ref().is_some_and(|p| p.controls.values().any(|b| b.action.starts_with("obs.")));
        obs::want(uses_obs);
        streamlabs::want(profile.as_ref().is_some_and(|p| p.controls.values().any(|b| b.action.starts_with("streamlabs."))));
        spotify::watch(profile.as_ref().is_some_and(|p| p.controls.values().any(|b| b.action == "spotify.like")));
        // CPU temperature and non-NVIDIA GPUs come from LibreHardwareMonitor: ask it only when shown
        let lhm_metric = |m: &str| matches!(m, "cpuTemp" | "gpu" | "gpuTemp");
        monitor::want_lhm(profile.as_ref().is_some_and(|p| {
            p.controls.values().any(|b| b.action == actions::MONITOR && lhm_metric(actions::param_str(&b.params, "metric")))
                || matches!(p.panel, Some(profile::PanelContent::Graph { metric: monitor::Metric::CpuTemp | monitor::Metric::Gpu | monitor::Metric::GpuTemp }))
        }));
        // Home Assistant entities whose state a key shows
        let mut entities: Vec<String> = profile
            .iter()
            .flat_map(|p| p.controls.values())
            .filter(|b| b.action == "ha.toggle")
            .map(|b| actions::param_str(&b.params, "entity").trim().to_string())
            .filter(|e| e.contains('.'))
            .collect();
        entities.sort();
        entities.dedup();
        homeassistant::watch(entities);
        // live values on screen (a worker starts the first time one is shown)
        let (page, ha_url) = {
            let s = self.lock();
            (s.page.clone(), s.store.settings.ha_url.clone().unwrap_or_default())
        };
        live::show(profile.as_ref(), page.as_deref(), &ha_url);
    }

    /// A step's condition, checked when the step is due (unknown conditions hold).
    fn condition_holds(&self, when: &str) -> bool {
        let obs = |k: &str| self.integration.get(k).copied().unwrap_or(false);
        match when {
            "" => true,
            "micMuted" => volume::is_muted(&volume::Target::Mic).unwrap_or(false),
            "micOn" => !volume::is_muted(&volume::Target::Mic).unwrap_or(false),
            "soundMuted" => volume::is_muted(&volume::Target::System).unwrap_or(false),
            "soundOn" => !volume::is_muted(&volume::Target::System).unwrap_or(false),
            "obsRecording" => obs("obs:record"),
            "obsNotRecording" => !obs("obs:record"),
            "obsStreaming" => obs("obs:stream"),
            "obsNotStreaming" => !obs("obs:stream"),
            other => {
                log::warn!("unknown step condition {other:?}");
                true
            }
        }
    }

    /// The state source of a binding, if its action has one.
    fn state_key(binding: &Binding) -> Option<String> {
        match binding.action.as_str() {
            "core.mic" => Some("mic".into()),
            "core.audioDevice" => {
                let p = &binding.params;
                let device = actions::param_str(p, "device").trim().to_lowercase();
                (!device.is_empty()).then(|| format!("audio:{}:{}", actions::param_str(p, "kind"), device))
            }
            "core.volume" => match binding.params.get("target").and_then(|t| t.as_str()) {
                Some("app") => binding.params.get("app").and_then(|a| a.as_str()).filter(|a| !a.trim().is_empty()).map(|a| format!("volume:app:{}", a.trim())),
                Some("mic") => Some("mic".into()),
                _ => Some("volume:system".into()),
            },
            _ => None,
        }
    }

    /// Ask the system for the state of every stateful key on screen; true when one changed.
    fn refresh_states(&mut self) -> bool {
        let (profile, page) = {
            let s = self.lock();
            (s.active_profile().cloned(), s.page.clone())
        };
        let Some(profile) = profile else { return false };
        let mut wanted: Vec<String> =
            (0..15).filter_map(|i| profile.controls.get(&control_id(page.as_deref(), &format!("keys.{i}")))).chain(profile.controls.get("dial")).filter_map(Self::state_key).collect();
        wanted.sort();
        wanted.dedup();
        let mut changed = false;
        // which audio devices are the defaults now (one call per kind)
        let mut defaults: HashMap<bool, Vec<String>> = HashMap::new();
        for key in &wanted {
            if let Some(rest) = key.strip_prefix("audio:") {
                let (kind, name) = rest.split_once(':').unwrap_or(("output", rest));
                let input = kind == "input";
                let list = defaults.entry(input).or_insert_with(|| audiodev::list(input).unwrap_or_default().into_iter().filter(|d| d.default).map(|d| d.name.to_lowercase()).collect());
                let on = list.iter().any(|n| n == name || n.contains(name));
                if self.states.insert(key.clone(), on) != Some(on) {
                    changed = true;
                }
            }
        }
        for key in wanted.iter().filter(|k| !k.starts_with("audio:")) {
            let target = match key.as_str() {
                "mic" => volume::Target::Mic,
                "volume:system" => volume::Target::System,
                k => volume::Target::App(k.trim_start_matches("volume:app:").to_string()),
            };
            // an app that plays nothing has no audio session: treat it as not muted
            let on = volume::is_muted(&target).unwrap_or(false);
            if self.states.insert(key.clone(), on) != Some(on) {
                changed = true;
            }
        }
        self.states.retain(|k, _| wanted.contains(k));
        changed
    }

    fn state_of(&self, binding: &Binding) -> Option<bool> {
        let p = &binding.params;
        let text = |k: &str| actions::param_str(p, k).trim().to_string();
        match binding.action.as_str() {
            // a day mode is on while its profile shows
            actions::DAY_MODE => (!text("profile").is_empty()).then(|| self.lock().active.as_deref() == Some(text("profile").as_str())),
            "obs.record" => Some(self.integration.get("obs:record").copied().unwrap_or(false)),
            "obs.stream" => Some(self.integration.get("obs:stream").copied().unwrap_or(false)),
            "obs.mute" => Some(self.integration.get(&format!("obs:mute:{}", text("input"))).copied().unwrap_or(false)),
            "obs.scene" => Some(self.values.get("obs:scene").is_some_and(|live| *live == text("scene"))),
            "obs.source" => self.integration.get(&format!("obs:source:{}/{}", text("scene"), text("source"))).copied(),
            "twitch.chatMode" => self.integration.get(&format!("twitch:mode:{}", text("mode"))).copied(),
            "twitch.shield" => self.integration.get("twitch:shield").copied(),
            "streamlabs.record" => Some(self.integration.get("sl:record").copied().unwrap_or(false)),
            "streamlabs.stream" => Some(self.integration.get("sl:stream").copied().unwrap_or(false)),
            "streamlabs.replayBuffer" => Some(self.integration.get("sl:replayBuffer").copied().unwrap_or(false)),
            "streamlabs.studioMode" => Some(self.integration.get("sl:studio").copied().unwrap_or(false)),
            "streamlabs.mute" => Some(self.integration.get(&format!("sl:mute:{}", text("input"))).copied().unwrap_or(false)),
            "streamlabs.scene" => Some(self.values.get("sl:scene").is_some_and(|live| *live == text("scene"))),
            "streamlabs.source" => self.integration.get(&format!("sl:source:{}/{}", text("scene"), text("source"))).copied(),
            "obs.recordPause" => Some(self.integration.get("obs:recordPaused").copied().unwrap_or(false)),
            "obs.virtualCam" => Some(self.integration.get("obs:virtualCam").copied().unwrap_or(false)),
            "obs.replayBuffer" => Some(self.integration.get("obs:replayBuffer").copied().unwrap_or(false)),
            "obs.studioMode" => Some(self.integration.get("obs:studio").copied().unwrap_or(false)),
            "obs.sceneCollection" => Some(self.values.get("obs:collection").is_some_and(|v| *v == text("collection"))),
            "obs.profile" => Some(self.values.get("obs:profile").is_some_and(|v| *v == text("profile"))),
            "obs.filter" => self.integration.get(&format!("obs:filter:{}/{}", text("source"), text("filter"))).copied(),
            "ha.toggle" => Some(self.integration.get(&format!("ha:{}", text("entity"))).copied().unwrap_or(false)),
            "spotify.like" => Some(self.integration.get("spotify:liked").copied().unwrap_or(false)),
            "core.audioDevice" if !text("device").is_empty() => Some(self.states.get(&format!("audio:{}:{}", text("kind"), text("device").to_lowercase())).copied().unwrap_or(false)),
            a if plugins::is_plugin_action(a) => None,
            _ => Self::state_key(binding).map(|k| self.states.get(&k).copied().unwrap_or(false)),
        }
    }

    /// Another PC (or a sync tool) changed the profiles folder: read the profiles again.
    fn check_profiles_folder(&mut self) {
        let dir = self.lock().store.dir().to_path_buf();
        let stamp = profile::folder_stamp(&dir);
        if self.profiles_stamp == (0, None) {
            self.profiles_stamp = stamp;
            return;
        }
        if stamp == self.profiles_stamp {
            return;
        }
        self.profiles_stamp = stamp;
        {
            let mut s = self.lock();
            s.store.reload_profiles();
            if s.active.as_ref().is_none_or(|id| !s.store.profiles.contains_key(id)) {
                s.active = s.store.fallback_id();
                s.page = None;
            }
        }
        log::info!("profiles folder changed: profiles read again");
        self.redraw_keys();
        self.update_leds();
        self.dial_dirty = true;
        let state = self.lock().state();
        publish_state(&self.app, state);
    }

    /// Turn screens and LEDs off or on as the lock screen and idle time ask.
    fn check_blank(&mut self) {
        let settings = self.lock().store.settings.clone();
        let deck_idle = self.last_deck_input.elapsed();
        let idle = idle::system_idle().map_or(deck_idle, |sys| sys.min(deck_idle));
        let blank = self.arcade.is_none() && idle::should_blank(self.locked, settings.blank_when_locked, idle, settings.blank_after_minutes);
        // the screensaver: after its idle minutes, unless the screens are off or a game runs
        let preview = self.lock().saver_preview.as_ref().is_some_and(|(_, until)| *until > Instant::now());
        let saver = !blank
            && self.arcade.is_none()
            && (preview || (!settings.saver.is_empty() && self.lock().trying.is_none() && deck_idle >= Duration::from_secs(u64::from(settings.saver_after_minutes.max(1)) * 60)));
        if saver != self.saver_on {
            self.saver_on = saver;
            log::info!("screensaver {}", if saver { "on" } else { "off" });
            self.redraw_keys();
        }
        if blank != self.blanked {
            self.blanked = blank;
            log::info!("screens {}", if blank { "off" } else { "on" });
            self.leds_sent.clear();
            self.update_leds();
        }
        self.apply_screens();
    }

    /// Send the backlight the settings ask for (0 while blanked), only when it changed.
    fn apply_screens(&mut self) {
        let level = if self.blanked { 0 } else { self.lock().store.settings.screen_brightness.min(100) };
        if self.backlight_sent == Some(level) {
            return;
        }
        let mut ok = true;
        for m in self.deck.snapshot().modules.iter().filter(|m| m.kind == "keys" || m.kind == "dial") {
            let Some(module) = self.deck.module(&m.id) else { continue };
            if let Some(r) = mh_protocol::report::backlight(module.kind(), level)
                && let Err(e) = module.send(&r)
            {
                log::warn!("backlight {}: {e}", m.id);
                ok = false;
            }
        }
        if ok {
            self.backlight_sent = Some(level);
            if !self.blanked {
                self.leds_sent.clear();
                self.update_leds();
            }
        }
    }

    /// A press on a session key brings that session's terminal forward; the sessions dial turns
    /// through them and a press brings the one shown forward.
    fn agent_input(&mut self, binding: &Binding, event: InputEvent) {
        let project = match (binding.action.as_str(), event) {
            (actions::AGENTS, InputEvent::Turn(d)) => {
                self.fleet.scroll(d as i32);
                self.dial_dirty = true;
                return;
            }
            (actions::AGENTS, InputEvent::Press(true)) => self.fleet.selected_session().map(|s| s.project.clone()),
            (_, InputEvent::Press(true)) => {
                let slot = (actions::param_f64(&binding.params, "slot", 1.0).max(1.0) as usize) - 1;
                self.fleet.by_slot(slot).map(|s| s.project.clone())
            }
            _ => return,
        };
        let caption = match project {
            Some(p) if agents::focus_project(&p) => return,
            Some(p) => format!("No window named {p}"),
            None => "No session here".to_string(),
        };
        self.feedback = Some((Feedback { value: None, caption }, Instant::now()));
        self.dial_dirty = true;
    }

    /// Advance animated GIFs whose next frame is due, and the panel clock once a minute.
    fn animate(&mut self, now: Instant) {
        self.update_highlight(now);
        // sessions waiting for a permission blink; silent ones are dropped after hours
        // sessions and app items waiting for the user pulse on their keys
        if (self.fleet.pulsing() || states::any_waiting(&self.values)) && now.duration_since(self.fleet_pulse) >= agents::PULSE {
            self.fleet_pulse = now;
            self.redraw_keys();
        }
        let still = self.lock().store.settings.reduced_motion == Some(true);
        // with reduced motion the screensaver changes once a minute
        let saver_every = if still { Duration::from_secs(60) } else { Duration::from_millis(saver::FRAME_MS) };
        if self.saver_on && now.duration_since(self.saver_frame) >= saver_every {
            self.saver_frame = now;
            self.redraw_keys();
        }
        if now.duration_since(self.fleet_sweep) >= Duration::from_secs(60) && self.fleet.forget_stale(now) {
            self.redraw_keys();
            self.dial_dirty = true;
        }
        let running_timer = self.timers.values().any(|t| t.running());
        let (profile, page) = {
            let s = self.lock();
            // nothing that moves (pictures, clocks, timers): no copy of the profile every tick
            let moving = s
                .active_profile()
                .is_some_and(|p| p.panel.is_some() || p.controls.values().any(|b| b.look.image.is_some() || b.action == actions::CLOCK || b.action == actions::TIMER || b.action == gallery::ACTION));
            if !moving && !running_timer {
                return;
            }
            (s.active_profile().cloned(), s.page.clone())
        };
        let Some(profile) = profile else { return };
        // the pictures on screen now: the panel picture, or the keys of this page, and the dial
        let mut names: Vec<String> = match &profile.panel {
            Some(profile::PanelContent::Picture { image }) => vec![image.clone()],
            Some(profile::PanelContent::Clock | profile::PanelContent::Graph { .. }) => Vec::new(),
            None => (0..15).filter_map(|i| profile.controls.get(&control_id(page.as_deref(), &format!("keys.{i}")))?.look.image.clone()).collect(),
        };
        let dial_image = profile.controls.get("dial").and_then(|b| b.look.image.clone());
        names.extend(dial_image.clone());
        self.frames.retain(|n, _| names.contains(n));
        let (mut keys_due, mut dial_due) = (false, false);
        // with reduced motion animated pictures stay on their first frame
        for name in names.into_iter().filter(|_| !still) {
            let Some(delays) = images::animation(&name) else { continue };
            let entry = self.frames.entry(name.clone()).or_insert((0, now + delays[0]));
            if now >= entry.1 {
                *entry = next_frame(&delays, *entry, now);
                if dial_image.as_deref() == Some(name.as_str()) {
                    dial_due = true;
                }
                keys_due = true;
            }
        }
        if matches!(profile.panel, Some(profile::PanelContent::Clock)) && self.clock_shown != clock_now() {
            keys_due = true;
        }
        // clocks and timers on keys and the dial: redraw when a shown text changes; looked at four
        // times a second (their texts change once a second), not on every engine tick
        if !self.live_dirty && now < self.live_next {
            if keys_due {
                self.redraw_keys();
            }
            if dial_due {
                self.dial_dirty = true;
            }
            return;
        }
        self.live_dirty = false;
        self.live_next = now + LIVE_EVERY;
        // the dial gallery may be on a live look (time, load, track): look again
        if profile.controls.get("dial").is_some_and(|b| b.action == gallery::ACTION) {
            dial_due = true;
        }
        let mut sig = String::new();
        let mut dial_live = false;
        let mut finished: Vec<String> = Vec::new();
        for (i, control) in (0..15).map(|i| (Some(i), control_id(page.as_deref(), &format!("keys.{i}")))).chain(std::iter::once((None, "dial".to_string()))) {
            let Some(b) = profile.controls.get(&control) else { continue };
            if let Some(face) = self.live_face(&control, b, now) {
                sig.push_str(&control);
                sig.push('=');
                sig.push_str(&face.main);
                sig.push(';');
                dial_live |= i.is_none();
                if b.action == actions::TIMER && self.timers.get(&control).is_some_and(|t| t.finished(now) && t.running() && !t.rang) {
                    finished.push(control.clone());
                }
            }
        }
        // a timer on another page rings too
        for (control, t) in &self.timers {
            if t.running() && t.finished(now) && !t.rang && !finished.contains(control) && profile.controls.get(control).is_some_and(|b| b.action == actions::TIMER) {
                finished.push(control.clone());
            }
        }
        for control in finished {
            let sound = profile.controls.get(&control).is_none_or(|b| actions::param_str(&b.params, "sound") != "off");
            if let Some(t) = self.timers.get_mut(&control) {
                t.rang = true;
            }
            self.update_leds();
            if sound {
                timer::ring();
            }
        }
        if sig != self.live_sig {
            self.live_sig = sig;
            keys_due = true;
            dial_due |= dial_live;
        }
        if keys_due {
            self.redraw_keys();
        }
        if dial_due {
            self.dial_dirty = true;
        }
    }

    /// The image reference to draw for a look: an animated GIF's current frame (`name#3`).
    fn current_frame(&self, look: &mut Look) {
        if let Some(name) = look.image.clone()
            && let Some((frame, _)) = self.frames.get(&name)
        {
            look.image = Some(format!("{name}#{frame}"));
        }
    }

    /// Knob and fader LEDs from the profile (colors, levels) and the base edge light. Only
    /// changes are sent; a module whose controls have no color keeps its own rainbow.
    fn update_leds(&mut self) {
        // the values the local API reads (`GET /api/controls`)
        self.lock().levels = self.levels.clone();
        let snapshot = self.deck.snapshot();
        let (profile, light_cfg) = {
            let s = self.lock();
            (s.active_profile().cloned(), s.store.settings.lights.clone())
        };
        let on = |k: &str| self.integration.get(k).copied().unwrap_or(false);
        let live = leds::Live {
            mic_muted: self.states.get("mic").copied().unwrap_or(false),
            on_air: on("obs:record") || on("obs:stream") || on("sl:record") || on("sl:stream"),
            agent_waiting: self.fleet.pulsing() || states::any_waiting(&self.values),
            timer_done: self.timers.values().any(|t| t.running() && t.finished(Instant::now())),
            in_call: self.in_call.is_some(),
        };
        let game = self.arcade.as_ref().map(|a| a.screen(Instant::now()));
        let blanked = self.blanked;
        let mut shown = Vec::new();
        let mut rainbow_now = false;
        for m in &snapshot.modules {
            let count = match m.kind {
                "knobs" => 3,
                "faders" => 5,
                "base" => 1,
                _ => continue,
            };
            let wanted: Option<leds::Lights> = match (m.kind, &game) {
                (_, _) if blanked => Some(leds::Lights::solid(vec![[0, 0, 0]; count])),
                ("knobs", Some(g)) => g.knob_leds.clone().map(leds::Lights::solid),
                ("faders", Some(g)) => g.fader_leds.clone().map(leds::Lights::solid),
                (kind, None) if self.lighting.contains_key(kind) => self.lighting.get(kind).map(|o| o.lights(count)),
                ("knobs", None) => leds::lights(profile.as_ref(), "knobs", 3, &self.levels, &light_cfg, &live),
                ("faders", None) => leds::lights(profile.as_ref(), "faders", 5, &self.levels, &light_cfg, &live),
                ("base", _) => leds::edge_lights(profile.as_ref(), &light_cfg, &live),
                _ => continue,
            };
            // LEDs set by plugins or the API go on top (not in games or while the screens are off)
            let wanted = if game.is_none() && !blanked {
                let ids: Vec<String> = if m.kind == "base" { vec!["base".into()] } else { (0..count).map(|i| format!("{}.{i}", m.kind)).collect() };
                if ids.iter().any(|id| self.plugin_leds.contains_key(id)) {
                    let mut lights = wanted.clone().unwrap_or_else(|| leds::Lights::solid(vec![[0, 0, 0]; count]));
                    for (i, id) in ids.iter().enumerate() {
                        if let (Some(c), Some(slot)) = (self.plugin_leds.get(id), lights.leds.get_mut(i)) {
                            *slot = leds::Led::solid(*c);
                        }
                    }
                    Some(lights)
                } else {
                    wanted
                }
            } else {
                wanted
            };
            // the tour points at a knob or fader: its LED blinks white
            let wanted = match self.highlight_shown.as_ref().filter(|_| game.is_none() && !blanked).and_then(|(c, lit)| Some((c.strip_prefix(m.kind)?.strip_prefix('.')?.parse::<usize>().ok()?, *lit)))
            {
                Some((i, lit)) if i < count => {
                    let mut lights = wanted.unwrap_or_else(|| leds::Lights::solid(vec![[0, 0, 0]; count]));
                    if let Some(led) = lights.leds.get_mut(i) {
                        *led = leds::Led::solid(if lit { [255, 255, 255] } else { [0, 0, 0] });
                    }
                    Some(lights)
                }
                _ => wanted,
            };
            // HubDeck's own rainbow: the color of this moment, the same the window works out
            let (wanted, replaced) =
                if light_cfg.hub_rainbow && !blanked && game.is_none() && (wanted.is_none() || wanted.as_ref().is_some_and(|l| l.leds.iter().any(|led| led.mode == LedMode::Rainbow))) {
                    let unix_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
                    let (lights, replaced) = leds::hub_rainbow(wanted, count, unix_ms, light_cfg.brightness);
                    rainbow_now = true;
                    (Some(lights), replaced)
                } else {
                    (wanted, Vec::new())
                };
            let before = self.leds_sent.get(&m.id).cloned();
            if before.as_ref() == Some(&wanted) {
                continue;
            }
            let Some(module) = self.deck.module(&m.id) else { continue };
            let kind = module.kind();
            let mut reports = Vec::new();
            match &wanted {
                None => reports.extend(report::led_mode(kind, LedTarget::All, LedMode::Rainbow, false)),
                Some(lights) => reports.extend(leds_reports(kind, lights, before.flatten().as_ref())),
            }
            match module.send_all(&reports) {
                Ok(()) => {
                    let view = wanted.as_ref().map(|l| {
                        let mut v = leds::view(l);
                        for (led, r) in v.iter_mut().zip(&replaced) {
                            if *r {
                                *led = leds::LedView { color: String::new(), effect: "hubRainbow" };
                            }
                        }
                        v
                    });
                    self.leds_sent.insert(m.id.clone(), wanted);
                    shown.push((m.id.clone(), view));
                }
                Err(e) => log::warn!("leds {}: {e}", m.id),
            }
        }
        self.rainbow_running = rainbow_now;
        // the window hears only real changes (HubDeck's rainbow turning is not one: it follows the clock)
        let all = {
            let mut s = self.lock();
            let changed = shown.iter().any(|(id, view)| s.leds.get(id) != Some(view));
            s.leds.extend(shown);
            changed.then(|| s.leds.clone())
        };
        if let Some(all) = all {
            let _ = self.app.emit(LEDS_EVENT, all);
        }
    }

    /// A call started or ended: the edge shows the microphone, and while it lasts the deck shows the
    /// call app's profile and stays on it.
    fn check_call(&mut self) {
        let now = call::call_app(&volume::mic_users());
        if now == self.in_call {
            return;
        }
        let started = now.is_some() && self.in_call.is_none();
        log::info!("call {}", if let Some(app) = &now { format!("on in {app}") } else { "over".to_string() });
        self.in_call = now.clone();
        let target = match (&now, started) {
            (Some(app), true) => {
                let s = self.lock();
                s.store.settings.call_switch.then(|| profile_for_app(&s.store, app, "")).flatten()
            }
            _ => None,
        };
        // a new call with a profile holds it; the same call goes on holding; no call, no hold
        let hold = target.is_some() || (now.is_some() && self.lock().call_hold);
        self.lock().call_hold = hold;
        if let Some(id) = target
            && self.lock().active.as_deref() != Some(id.as_str())
        {
            self.activate(&id);
        }
        if now.is_none() {
            // the call is over: back to what the app in front asks for
            let exe = self.lock().foreground.clone();
            self.lock().foreground.clear();
            self.on_foreground(exe, String::new());
        }
        self.update_leds();
    }

    /// Questions and statuses that ran out, and the end of a `notify` light.
    /// The control the tour points at, if it is lit at this moment.
    fn lit_highlight(&self) -> Option<&str> {
        self.highlight_shown.as_ref().filter(|(_, lit)| *lit).map(|(c, _)| c.as_str())
    }

    /// The tour's highlight: blink it (or hold it still with reduced motion), redraw what changed.
    fn update_highlight(&mut self, now: Instant) {
        let want = {
            let mut s = self.lock();
            if s.highlight.as_ref().is_some_and(|(_, _, until)| now >= *until) {
                s.highlight = None;
            }
            let still = s.store.settings.reduced_motion == Some(true);
            s.highlight.as_ref().map(|(c, from, _)| (c.clone(), still || (now.duration_since(*from).as_millis() / agents::PULSE.as_millis()).is_multiple_of(2)))
        };
        if want == self.highlight_shown {
            return;
        }
        let touched: Vec<String> = [&want, &self.highlight_shown].into_iter().flatten().map(|(c, _)| c.clone()).collect();
        self.highlight_shown = want;
        if touched.iter().any(|c| c.starts_with("keys.")) {
            self.redraw_keys();
        }
        if touched.iter().any(|c| c.starts_with("knobs.") || c.starts_with("faders.")) {
            self.update_leds();
        }
        if touched.iter().any(|c| c == "dial") {
            self.dial_dirty = true;
        }
    }

    fn expire_deck_ui(&mut self, now: Instant) {
        if self.notify_until.is_some_and(|t| now >= t) {
            self.notify_until = None;
            self.set_lighting(exec::Lighting { target: "base".into(), effect: None });
        }
        let changed = {
            let mut s = self.lock();
            let mut changed = false;
            if let Some(p) = s.prompt.as_mut()
                && p.answer.is_none()
                && now >= p.until
            {
                p.answer = Some(None);
                changed = true;
            }
            if s.status.as_ref().is_some_and(|st| st.until.is_some_and(|t| now >= t)) {
                s.status = None;
                changed = true;
            }
            changed
        };
        if changed {
            self.redraw_keys();
            self.dial_dirty = true;
        }
    }

    /// The "Lighting effect" action: an effect on a module kind (or all) until the profile changes;
    /// `None` = back to the profile's own lights.
    fn set_lighting(&mut self, cmd: exec::Lighting) {
        let kinds: Vec<&str> = match cmd.target.as_str() {
            "all" => vec!["knobs", "faders", "base"],
            k => vec![k],
        };
        for k in kinds {
            match &cmd.effect {
                Some(o) => self.lighting.insert(k.to_string(), o.clone()),
                None => self.lighting.remove(k),
            };
        }
        self.update_leds();
    }

    fn redraw_dial(&mut self) {
        let snapshot = self.deck.snapshot();
        let user = self.app.state::<Placements>().all();
        let profile = self.lock().active_profile().cloned();
        for m in snapshot.modules.iter().filter(|m| m.kind == "dial") {
            let Some(module) = self.deck.module(&m.id) else { continue };
            let rotation = self.content_rotation(m.kind, m.slot.as_deref(), snapshot.turn, &user);
            let stored = profile.as_ref().and_then(|p| p.controls.get("dial"));
            // the dial gallery draws the look it is on, as that look's own setting would
            let shown = stored.filter(|b| b.action == gallery::ACTION).map(|_| gallery::at(self.gallery));
            let binding = shown.as_ref().map(|f| &f.binding).or(stored);
            let mut look = self.plugin_look("dial", binding.map(|b| b.look_for(self.state_of(b))).unwrap_or_default());
            if let Some(b) = binding {
                look = self.sd_look("dial", b, look);
            }
            // a Stream Deck encoder action on the dial: its feedback stays on the screen
            let sd_face = binding.filter(|b| plugins::is_plugin_action(&b.action)).and_then(|_| self.plugin_looks.get("dial")).and_then(|p| p.feedback.clone());
            // the plugin's own picture, or the dial's look set to the cover of what plays now
            let sd_cover = binding.filter(|b| plugins::is_plugin_action(&b.action)).and_then(|b| {
                self.plugin_looks.get("dial").and_then(|p| p.cover.clone()).or_else(|| (b.look.image.as_deref() == Some(images::COVER_REF)).then(|| images::uri(images::COVER_REF)).flatten())
            });
            self.current_frame(&mut look);
            // with modes, the dial names the current one
            if let Some(b) = binding.filter(|b| b.action == actions::MODES) {
                let modes = actions::modes(&b.params);
                if !modes.is_empty() {
                    look.label = Some(modes[self.modes.get("dial").copied().unwrap_or(0) % modes.len()].label.clone());
                    look.icon = None;
                }
            }
            if look.label.is_none() && look.icon.is_none() {
                look.label = profile.as_ref().map(|p| p.name.clone());
            }
            let value = self.feedback.as_ref().map(|(f, _)| (f.value, f.caption.as_str()));
            let calibration = self.lock().calibration.clone();
            let game = self.arcade.as_ref().map(|a| a.screen(Instant::now()).dial);
            let svg = match (calibration, game) {
                (None, Some(body)) => render::dial_body_svg(&body, rotation),
                (Some(c), _) => {
                    let (_, name) = render::GEOMETRY_PARAMS[c.selected];
                    render::calibration_dial_svg(name, c.geometry.get(c.selected), c.selected + 1, render::GEOMETRY_PARAMS.len(), rotation)
                }
                (None, None) if value.is_none() && sd_face.is_some() => {
                    let (v, main, title) = sd_face.clone().expect("checked");
                    let main = if main.is_empty() { look.label.clone().unwrap_or_default() } else { main };
                    match &sd_cover {
                        // a song's cover behind its title and time, the bar as a ring
                        Some(cover) => render::cover_dial_svg(cover, &title, &main, v, rotation),
                        None => render::time_dial_svg(&Look { icon: None, ..look.clone() }, &main, Some(title.as_str()).filter(|t| !t.is_empty()), v, rotation),
                    }
                }
                (None, None) if value.is_none() && binding.is_some_and(|b| b.action == actions::MONITOR) => {
                    let (metric, v) = binding.and_then(|b| self.monitor_value(b)).expect("monitor binding");
                    // temperatures fill the ring up to 100 °C
                    render::dial_svg(&look, Some((v.map(|v| v / 100.0), metric.name())), rotation)
                }
                (None, None) if value.is_none() && binding.is_some_and(|b| b.action == actions::CLOCK || b.action == actions::TIMER) => {
                    let b = binding.expect("checked");
                    let face = self.live_face("dial", b, Instant::now()).unwrap_or_default();
                    let look = if face.done { b.look_for(Some(true)) } else { look };
                    render::time_dial_svg(&look, &face.main, face.second.as_deref(), face.ring, rotation)
                }
                (None, None) if value.is_none() && binding.is_some_and(|b| b.action == live::ACTION) => {
                    let b = binding.expect("checked");
                    let f = live::face(&b.params, &look, live::value_key(&b.params).and_then(|k| self.values.get(&k)).map(String::as_str));
                    render::time_dial_svg(&Look { bg: f.bg.clone().or(look.bg.clone()), ..look.clone() }, &f.main, f.second.as_deref(), f.ring, rotation)
                }
                (None, None) if value.is_none() && binding.is_some_and(|b| b.action == states::ACTION) => {
                    let b = binding.expect("checked");
                    let (main, second, color, _) = states::face(&b.params, &look, &self.values);
                    render::time_dial_svg(&Look { icon: None, image: None, fg: Some(color.to_string()), ..look.clone() }, &main, second.as_deref(), None, rotation)
                }
                (None, None) if value.is_none() && binding.is_some_and(|b| b.action == actions::AGENTS) => {
                    let (main, second) = self.fleet.dial_text();
                    render::time_dial_svg(&Look { icon: None, image: None, ..look.clone() }, &main, Some(&second), None, rotation)
                }
                (None, None) if value.is_none() && binding.is_some_and(|b| b.action == actions::NOW_PLAYING) => {
                    let cover = images::uri(images::COVER_REF);
                    let track = self.now_playing.as_ref().map(|(t, a, p)| (t.as_str(), a.as_str(), *p, cover.as_deref()));
                    render::now_playing_dial_svg(&look, track, rotation)
                }
                (None, None) => render::dial_svg(&look, value, rotation),
            };
            // a question or a status from an assistant takes the dial
            let ui = {
                let s = self.lock();
                let question = s.prompt.as_ref().filter(|p| p.waiting()).map(|p| p.question.clone());
                (question, s.status.clone())
            };
            let house_look = Look { bg: Some(house::BG.into()), fg: Some(house::LABEL.into()), ..Look::default() };
            let svg = match ui {
                (Some(question), _) => render::with_caption(&render::dial_svg(&Look { label: Some(question), ..house_look }, None, rotation), "?", rotation),
                (None, Some(status)) => render::time_dial_svg(&house_look, &status.main, status.detail.as_deref(), None, rotation),
                _ => svg,
            };
            let svg = match &shown {
                Some(face) => {
                    let language = self.lock().store.settings.language.clone();
                    render::with_caption(&svg, face.caption_in(&language), rotation)
                }
                None => svg,
            };
            // the tour points at the dial: a white ring around its edge
            let svg = if self.lit_highlight() == Some("dial") { render::with_ring(&svg, HIGHLIGHT) } else { svg };
            if self.sent.get(&m.id) == Some(&svg) {
                continue;
            }
            match self.renderer.jpeg(&svg).and_then(|jpeg| module.send_all(&report::dial_image(&jpeg, module.output_len)).map_err(|e| e.to_string())) {
                Ok(()) => {
                    self.sent.insert(m.id.clone(), svg);
                }
                Err(e) => log::warn!("draw dial: {e}"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tour_highlight_is_found_on_any_page() {
        assert!(highlight_matches("keys.9", "keys.9"));
        assert!(highlight_matches("keys.9", "fun/keys.9"));
        assert!(!highlight_matches("keys.9", "keys.1"));
        assert!(highlight_matches("dial", "dial"));
        let ring = render::with_ring(&render::dial_svg(&Look::default(), None, 0), HIGHLIGHT);
        assert!(ring.contains(r##"stroke="#ffffff""##) && ring.ends_with("</svg>"));
    }

    #[test]
    fn animations_keep_their_pace_by_skipping_frames() {
        let ms = Duration::from_millis;
        let delays = [ms(20); 5];
        let t0 = Instant::now();
        // not due yet: stays
        assert_eq!(next_frame(&delays, (0, t0 + ms(20)), t0 + ms(10)), (0, t0 + ms(20)));
        // drawing took 70 ms: frames 1 and 2 are skipped, frame 3 shows until 80 ms
        assert_eq!(next_frame(&delays, (0, t0 + ms(20)), t0 + ms(70)), (3, t0 + ms(80)));
        // wraps around
        assert_eq!(next_frame(&delays, (4, t0 + ms(100)), t0 + ms(105)), (0, t0 + ms(120)));
        // a long stall: goes on from now
        assert_eq!(next_frame(&delays, (0, t0 + ms(20)), t0 + ms(5000)), (0, t0 + ms(5020)));
    }

    #[test]
    fn led_reports_send_colors_then_effects_then_speed() {
        use leds::{Led, Lights};
        use mh_protocol::ModuleKind;
        let red = Led { rgb: [255, 0, 0], mode: LedMode::Breathing };
        let lights = Lights { leds: vec![red; 3], speed: Some(40) };
        let bytes: Vec<Vec<u8>> = leds_reports(ModuleKind::Knobs, &lights, None).iter().map(|r| r.bytes().to_vec()).collect();
        assert_eq!(bytes.len(), 5, "three colors, one effect for the module, the speed");
        assert_eq!(bytes[3], vec![0x02, 0x2C, 2, 0xFF, LedMode::Breathing as u8, 0]);
        assert_eq!(bytes[4], vec![0x02, 0x30, 40]);
        // nothing changed: nothing sent; one knob's effect changed: only its mode
        assert!(leds_reports(ModuleKind::Knobs, &lights, Some(&lights)).is_empty());
        let mut mixed = lights.clone();
        mixed.leds[1].mode = LedMode::Rainbow;
        let bytes: Vec<Vec<u8>> = leds_reports(ModuleKind::Knobs, &mixed, Some(&lights)).iter().map(|r| r.bytes().to_vec()).collect();
        assert_eq!(bytes.len(), 3, "a mode per knob when they differ");
        assert_eq!(bytes[1], vec![0x02, 0x2C, 2, 1, LedMode::Rainbow as u8, 0]);
    }

    #[test]
    fn turns_of_the_same_control_merge() {
        let turn = |c: &str, d| Msg::Input { control: c.into(), event: InputEvent::Turn(d) };
        let mut batch = vec![turn("dial", 1), turn("dial", 1), turn("dial", -1), turn("knobs.0", 1), turn("dial", 1), Msg::ProfilesChanged, turn("dial", 1)];
        coalesce_turns(&mut batch);
        let summary: Vec<String> = batch
            .iter()
            .map(|m| match m {
                Msg::Input { control, event: InputEvent::Turn(d) } => format!("{control}{d:+}"),
                _ => "other".into(),
            })
            .collect();
        assert_eq!(summary, ["dial+1", "knobs.0+1", "dial+1", "other", "dial+1"]);
    }

    #[test]
    fn app_profiles_take_over_and_give_back_to_home() {
        let dir = std::env::temp_dir().join(format!("hubdeck-home-{}", std::process::id()));
        let mut store = Store::load(&dir);
        for (id, apps) in [("desk", vec![]), ("test", vec![]), ("ps", vec!["Photoshop.exe".to_string()])] {
            let mut p = Profile::new(id);
            p.id = id.into();
            p.apps = apps;
            store.profiles.insert(id.into(), p);
        }
        let go = |s: &Store, home: &str, active: &str, exe: &str| switch_target(s, Some(home), Some(active), exe, "");
        // Photoshop in front takes over from the home profile
        assert_eq!(go(&store, "desk", "desk", "Photoshop.exe"), Some(("ps".into(), Some("Photoshop.exe".into()))));
        // another app without a profile gives back to home
        assert_eq!(go(&store, "desk", "ps", "chrome.exe"), Some(("desk".into(), None)));
        // a hand-picked profile stays while apps without a profile come and go
        assert_eq!(go(&store, "test", "test", "chrome.exe"), None);
        assert_eq!(go(&store, "test", "test", "explorer.exe"), None);
        // ... and Photoshop still takes over, then gives back to the hand-picked one
        assert_eq!(go(&store, "test", "ps", "chrome.exe"), Some(("test".into(), None)));
        // picking Photoshop's profile by hand makes it home: it stays outside Photoshop too
        assert_eq!(go(&store, "ps", "ps", "chrome.exe"), None);
        // pinned: nothing switches
        store.settings.pinned = true;
        assert_eq!(go(&store, "desk", "desk", "Photoshop.exe"), None);
        store.settings.pinned = false;
        store.settings.auto_switch = false;
        assert_eq!(go(&store, "desk", "desk", "Photoshop.exe"), None, "auto-switch off");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn foreground_app_picks_its_profile_or_the_fallback() {
        let dir = std::env::temp_dir().join(format!("hubdeck-route-{}", std::process::id()));
        let mut store = Store::load(&dir);
        let mut desk = Profile::new("Desk");
        desk.id = "desk".into();
        let mut obs = Profile::new("OBS");
        obs.id = "obs".into();
        obs.apps = vec!["obs64.exe".into()];
        store.profiles.insert(desk.id.clone(), desk);
        store.profiles.insert(obs.id.clone(), obs);
        store.settings.fallback = Some("desk".into());
        assert_eq!(profile_for_app(&store, "OBS64.EXE", "").as_deref(), Some("obs"));
        assert_eq!(profile_for_app(&store, "notepad.exe", ""), None, "apps without a profile pick nothing");
        let mut chat = Profile::new("OBS chat");
        chat.id = "chat".into();
        chat.apps = vec!["obs64.exe: Chat".into()];
        store.profiles.insert(chat.id.clone(), chat);
        assert_eq!(profile_for_app(&store, "obs64.exe", "Chat window").as_deref(), Some("chat"), "the title rule wins");
        assert_eq!(profile_for_app(&store, "obs64.exe", "Studio").as_deref(), Some("obs"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn multi_action_delays_add_up() {
        let t = Instant::now();
        let params = serde_json::json!({"steps": [
            {"action": "core.media", "params": {"command": "next"}, "delay": 0},
            {"action": "core.text", "params": {"text": "a"}, "delay": 250},
            {"action": "core.text", "params": {"text": "b"}, "delay": 100}
        ]});
        let at: Vec<u128> = schedule_steps(t, &params).iter().map(|(due, _, _)| due.duration_since(t).as_millis()).collect();
        assert_eq!(at, [0, 250, 350]);
    }

    #[test]
    fn page_ids_only_move_keys() {
        assert_eq!(control_id(Some("p1"), "keys.3"), "p1/keys.3");
        assert_eq!(control_id(Some("p1"), "dial"), "dial");
        assert_eq!(control_id(Some("p1"), "knobs.0"), "knobs.0");
        assert_eq!(control_id(None, "keys.3"), "keys.3");
    }

    #[test]
    fn calibration_adjusts_and_clamps() {
        let mut g = render::PanelGeometry::default();
        g.adjust(2, 3.0);
        assert_eq!(g.dx, 103.0);
        g.adjust(4, 1000.0);
        assert_eq!(g.w, 200.0, "clamped");
        assert_eq!(g.key_rect(6), (103.0, 99.0, 200.0, 75.0), "row 2, column 2");
    }

    #[test]
    fn slot_table_is_embedded() {
        let table = parse_slot_table();
        assert!(table.len() > 80);
        assert_eq!(table.get("keys@2-3"), Some(&90));
    }
}
