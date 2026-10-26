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
pub mod beat;
mod brightness;
pub mod builder;
pub mod call;
pub mod calls;
pub mod check;
pub mod cli;
pub mod commands;
pub mod deck_mcp;
pub mod demo;
pub mod diagnostics;
pub mod elevated;
pub(crate) mod exec;
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
pub mod languages;
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
pub mod names;
pub mod nudge;
pub mod obs;
mod pet;
pub mod plugins;
pub(crate) mod presets;
pub mod presses;
pub mod profile;
pub mod recorder;
pub mod render;
mod saver;
pub mod sdcatalog;
pub mod sdlocal;
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

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
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
/// How long a pressed key shows its frame: long enough to see, short enough not to be in the way.
const PRESS_FLASH: Duration = Duration::from_millis(160);
/// How often HubDeck's rainbow moves on (10 steps a second: smooth on the LEDs, light on USB).
const RAINBOW_STEP: Duration = Duration::from_millis(leds::RAINBOW_TICK_MS);

/// How often lights that follow the music are set; one still going out waits for the next step
/// (the LED reports take a few ms each).
const MUSIC_STEP: Duration = Duration::from_millis(20);
/// How long the dial shows a value after it changed.
const FEEDBACK_FOR: Duration = Duration::from_millis(1500);
/// How often stateful keys check their state (mute can change outside the deck).
const STATE_POLL: Duration = Duration::from_millis(1000);
/// Minimum time between two dial images.
/// The dial takes ~32 pictures a second (`examples/framerate.rs`); a frame draws in ~4 ms (release).
const DIAL_FRAME: Duration = Duration::from_millis(36);
/// A moving key panel is drawn at most this often (20 a second; the GIFs run at 14-25).
const KEYS_MOVING_EVERY: Duration = Duration::from_millis(50);
/// JPEG quality of the key panel while it moves (GIFs, the screensaver); 98 at rest.
const MOVING_QUALITY: u8 = 80;
/// Mochi's resting frames kept as JPEG: one loop and a little (420 frames, ~6 KB each).
const PET_FRAMES_KEPT: usize = 480;

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
    /// The user clicked the deck in the window, or End: the screensaver ends as at a touch of the
    /// deck (owner's pick 1a, 2026-10-18).
    WindowTouched,
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
    /// Where in the song: (seconds played, seconds long, playing), every second from the player.
    Progress(Option<(f32, f32, bool)>),
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
    /// The window's Stop on a timer that ran out (profile, control).
    StopTimer {
        profile: String,
        control: String,
    },
    /// The session was locked or unlocked (Windows: Win+L, the lock screen after resume).
    #[cfg_attr(not(windows), allow(dead_code))]
    Locked(bool),
}

/// The looks plugins give the active profile's controls now (see `publish_looks`).
/// The dial's last picture as the hardware got it, for the window's drawing of the deck (owner,
/// 2026-10-15: "w apce na dialu nie pokazuje dokładnie tego co jest pokazywane na urządzeniu").
static DIAL_SHOWN: std::sync::LazyLock<std::sync::Mutex<Option<serde_json::Value>>> = std::sync::LazyLock::new(Default::default);
/// At most this often to the window (the hardware gets every frame).
const DIAL_MIRROR_EVERY: Duration = Duration::from_millis(125);

/// The window plays GIF keys itself (src/components/DeckView.tsx, the same switch there): smooth,
/// WebView2 440-600 MB instead of ~320 with the deck's pictures (window stage, 2026-10-16); off
/// until the owner picks.
const WINDOW_PLAYS_GIFS: bool = false;

/// How often the key panel goes to the window while only animations move on it.
const KEYS_MIRROR_SLOW: Duration = Duration::from_secs(1);

/// The screensaver on the keys now (`{effect, color}`), for the window to draw it itself.
static SAVER_SHOWN: std::sync::Mutex<Option<serde_json::Value>> = std::sync::Mutex::new(None);

#[tauri::command]
pub fn saver_shown() -> Option<serde_json::Value> {
    SAVER_SHOWN.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// The deck was clicked in the window, or End: the screensaver ends, as at a touch of the deck.
#[tauri::command]
pub fn saver_end(engine: tauri::State<'_, Engine>) {
    let _ = engine.sender().send(Msg::WindowTouched);
}

/// The key panel's last picture, the same way (keys with live values showed only their icon).
static KEYS_SHOWN: std::sync::LazyLock<std::sync::Mutex<Option<serde_json::Value>>> = std::sync::LazyLock::new(Default::default);

/// The window's pages that take the pictures: (`"keys"` or `"dial"`, its channel). Through
/// channels, not events: Tauri's events "are not designed for high throughput" (its docs), and as
/// events the pictures grew WebView2's page by ~19 MB a minute while the deck moved (release gate,
/// 2026-10-16); with the window on a page without the deck it stayed flat.
type MirrorChannel = tauri::ipc::Channel<serde_json::Value>;
static MIRRORS: std::sync::LazyLock<std::sync::Mutex<Vec<(String, MirrorChannel)>>> = std::sync::LazyLock::new(Default::default);
/// At most this many channels a kind: a page that reloads never says goodbye.
const MIRRORS_EACH: usize = 4;

/// A page takes the key panel's (`which` "keys": `{profile, page, module, jpeg, rects}`) or the
/// dial's (`{profile, module, jpeg}`, JPEG in base64) pictures through `channel`; answers the
/// last one.
#[tauri::command]
pub fn mirror_listen(which: String, channel: MirrorChannel) -> Option<serde_json::Value> {
    let mut all = MIRRORS.lock().unwrap_or_else(|e| e.into_inner());
    while all.iter().filter(|(w, _)| *w == which).count() >= MIRRORS_EACH {
        let oldest = all.iter().position(|(w, _)| *w == which).expect("counted");
        all.remove(oldest);
    }
    all.push((which.clone(), channel));
    let shown = if which == "dial" { &DIAL_SHOWN } else { &KEYS_SHOWN };
    shown.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// The page no longer shows the deck.
#[tauri::command]
pub fn mirror_unlisten(id: u32) {
    MIRRORS.lock().unwrap_or_else(|e| e.into_inner()).retain(|(_, c)| c.id() != id);
}

fn send_mirror(which: &str, frame: &serde_json::Value) {
    MIRRORS.lock().unwrap_or_else(|e| e.into_inner()).retain(|(w, c)| w != which || c.send(frame.clone()).is_ok());
}

static LIVE_LOOKS: std::sync::LazyLock<std::sync::Mutex<(String, HashMap<String, Look>)>> = std::sync::LazyLock::new(Default::default);

/// `{profile, looks}`: the profile they are for, and the looks by control id.
#[tauri::command]
pub fn live_looks() -> serde_json::Value {
    let (profile, looks) = LIVE_LOOKS.lock().unwrap_or_else(|e| e.into_inner()).clone();
    serde_json::json!({"profile": profile, "looks": looks})
}

/// What a plugin or the local API put on a control: title, picture, state.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PluginLook {
    pub title: Option<String>,
    pub image: Option<String>,
    /// State index. Stream Deck `setState` uses the action's `States` (0, 1, 2, ...). Hubdeck's own
    /// on/off is 0 or 1, and 1 is the "on" look.
    pub state: Option<u8>,
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
    /// Made at once for an installed app ("Ready for your apps").
    pub ready: bool,
    /// Switches in when its app comes to the front (off for a ready profile until switched on).
    pub switch_in: bool,
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
    /// A draft that was on the deck for a try when HubDeck stopped (the window offers it back).
    pub interrupted_try: Option<String>,
    /// Comparing drafts on the deck: (which one, of how many).
    pub comparing: Option<(usize, usize)>,
    /// Timers that ran out (the window names them and can stop them).
    pub ringing: Vec<Ringing>,
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
    /// What really went out to each module's LEDs last (#rrggbb at the moment's brightness, None =
    /// its own rainbow) and how many times since the start: `GET /api/leds`, so a check can see a
    /// light to the music change without looking at the deck.
    pub leds_out: HashMap<String, (Option<Vec<String>>, u64)>,
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
    /// Timers that ran out and wait for a press (or the window's Stop).
    pub ringing: Vec<Ringing>,
}

/// A timer that ran out, wherever it is (another page or profile), for the window.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ringing {
    pub profile: String,
    pub profile_name: String,
    /// `keys.3`, `page-2/keys.3` or `dial`.
    pub control: String,
    /// The key's own caption, if it has one.
    pub label: Option<String>,
    /// The key page it is on; None = the main page (or the dial).
    pub page: Option<String>,
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
                    ProfileSummary {
                        id: p.id.clone(),
                        name: p.name.clone(),
                        apps: p.apps.clone(),
                        in_bar: p.shows_in_bar() && !draft,
                        draft,
                        ready: self.store.settings.ready.contains(&p.id),
                        switch_in: p.switch_in != Some(false),
                    }
                })
                .collect(),
            trying: self.trying.as_ref().map(|t| t.profile.clone()),
            interrupted_try: self.store.settings.trying.clone().filter(|id| self.trying.is_none() && self.store.settings.drafts.contains(id) && self.store.profiles.contains_key(id)),
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
            ringing: self.ringing.clone(),
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
        render::set_deck_font(store.settings.deck_font.as_deref());
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
            leds_out: HashMap::new(),
            prompt: None,
            status: None,
            levels: HashMap::new(),
            call_hold: false,
            trying: None,
            draft_compare: None,
            saver_preview: None,
            tour_dry_run: false,
            highlight: None,
            ringing: Vec::new(),
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
            music_running: false,
            music_glow: None,
            music_edge: false,
            music_sent: HashMap::new(),
            leds_shown: HashMap::new(),
            in_call: None,
            call_checked: Instant::now(),
            rainbow_last: Instant::now(),
            last_turn: HashMap::new(),
            frames: HashMap::new(),
            clock_shown: String::new(),
            timers: HashMap::new(),
            clocks: HashMap::new(),
            pomodoros: HashMap::new(),
            weather_view: HashMap::new(),
            pet: pet::Pet::load(data_dir),
            pet_frame: Instant::now(),
            pet_jpegs: HashMap::new(),
            dial_mirror_at: Instant::now(),
            keys_mirror_at: Instant::now(),
            dial_mirror_due: false,
            keys_mirror_due: false,
            keys_mirror_slow: false,
            saver_published: None,
            monitor_pick: HashMap::new(),
            live_sig: String::new(),
            live_next: Instant::now(),
            live_dirty: false,
            states: HashMap::new(),
            last_state_check: Instant::now(),
            now_playing: None,
            progress: None,
            progress_drawn: None,
            seek_to: None,
            seek_sent: None,
            samples: VecDeque::new(),
            integration: HashMap::new(),
            values: HashMap::new(),
            plugin_looks: HashMap::new(),
            fleet: agents::Fleet::default(),
            fleet_pulse: Instant::now(),
            saver_on: false,
            saver_frame: Instant::now(),
            saver_next: 0,
            keys_waiting: false,
            keys_moved: Instant::now(),
            perf: Perf::new(),
            fleet_sweep: Instant::now(),
            plugin_leds: HashMap::new(),
            plugin_panel: None,
            toggles: load_toggles(data_dir),
            arcade: None,
            profiles_stamp: (0, None),
            last_folder_check: Instant::now(),
            data_dir: data_dir.to_path_buf(),
            blanked: false,
            locked: false,
            last_deck_input: Instant::now(),
            backlight_sent: HashMap::new(),
            last_blank_check: Instant::now(),
            highlight_shown: None,
            press_flash: None,
            failed_keys: HashMap::new(),
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
            // the start profile's plugin controls and OBS: until now only a profile switch told them
            // (#5: with Auto-switch off, Discord keys did nothing after a restart)
            let _ = media_tx.send(Msg::ProfilesChanged);
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
    /// Stop a timer that ran out (the window's Stop).
    pub fn stop_timer(&self, profile: String, control: String) {
        let _ = self.tx.send(Msg::StopTimer { profile, control });
    }

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
            // a profile picked by hand during a try ends the try: the bar went on saying "Trying"
            // and a ready-made profile tried was never thrown away (2026-10-20)
            if s.trying.take().is_some() {
                s.draft_compare = None;
            }
            // picked by hand: app profiles give back to it; HubDeck still starts on the home profile,
            // set only by "Set as home" (owner, 2026-10-22: a click made the profile home)
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
                    if s.store.settings.trying.as_deref() != Some(id) {
                        s.store.settings.trying = Some(id.to_string());
                        if let Err(e) = s.store.save_settings() {
                            log::warn!("settings: {e}");
                        }
                    }
                    s.active = Some(id.to_string());
                    s.auto_app = None;
                    s.page = None;
                }
                None => {
                    // the end of a try ends a comparison too, and its note on the dial
                    if s.draft_compare.take().is_some() {
                        s.status = None;
                    }
                    if s.store.settings.trying.take().is_some()
                        && let Err(e) = s.store.save_settings()
                    {
                        log::warn!("settings: {e}");
                    }
                    let Some(t) = s.trying.take() else {
                        drop(s);
                        self.changed();
                        return Ok(());
                    };
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

    /// "Use this" on a draft (owner's pick 1a, 2026-10-14: a kept draft was only tried, so the
    /// deck went back to the old profile at the next press). A new version of one of the user's
    /// own profiles goes into that profile after a backup (its id, name and apps stay); any
    /// other draft becomes a profile of its own. Either way it is on the deck now, as home.
    /// Returns the kept profile's id and, when it replaced one, that profile's name.
    pub fn use_draft(&self, id: &str) -> Result<(String, Option<String>), String> {
        let target = {
            let s = self.lock();
            let draft = s.store.profiles.get(id).ok_or_else(|| format!("no profile {id}"))?;
            if !s.store.settings.drafts.iter().any(|d| d == id) {
                return Err(format!("{id} is not a draft"));
            }
            version_target(&s.store, draft)
        };
        // the try ends first: what showed before it must not come back
        self.try_profile(None)?;
        let Some(target) = target else {
            self.settle_draft(id, true)?;
            self.activate(id)?;
            return Ok((id.to_string(), None));
        };
        backup::create(&self.data_dir, "before Use this replaced a profile").map_err(|e| e.to_string())?;
        let name = put_version(&mut self.lock().store, id, &target)?;
        self.activate(&target)?;
        Ok((target, Some(name)))
    }

    /// "Switch in with the app" of a ready profile: on links it to its app, off keeps it unlinked.
    pub fn set_switch_in(&self, id: &str, on: bool) -> Result<(), String> {
        {
            let mut s = self.lock();
            let mut p = s.store.profiles.get(id).cloned().ok_or_else(|| format!("no profile {id}"))?;
            p.switch_in = if on { None } else { Some(false) };
            s.store.save_profile(p).map_err(|e| e.to_string())?;
        }
        self.changed();
        if on {
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
                // what shows now stays; the start profile does not change ("Set as home" sets it;
                // owner, 2026-10-22)
                s.home = s.active.clone();
                s.auto_app = None;
                s.store.settings.active = s.active.clone();
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
    // a draft never takes over: it is only a proposal until the user picks it; nor does a ready
    // profile the user has not switched on
    store
        .profiles
        .values()
        .filter(|p| !store.settings.drafts.contains(&p.id) && p.switch_in != Some(false))
        .filter_map(|p| p.app_match(exe, title).map(|rank| (rank, p)))
        .max_by_key(|(rank, _)| *rank)
        .map(|(_, p)| p.id.clone())
}

/// The user's own profile a draft is a new version of: the one it names (`version_of`), else for
/// the assistant's drafts one with the same name ("My deck · dial and knobs", with or without
/// " · sketch"), when exactly one matches.
fn version_target(store: &Store, draft: &Profile) -> Option<String> {
    let own = |id: &str| id != draft.id && store.profiles.contains_key(id) && !store.settings.drafts.iter().any(|d| d == id);
    if let Some(v) = draft.version_of.as_deref().filter(|v| own(v)) {
        return Some(v.to_string());
    }
    if !draft.assistant {
        return None;
    }
    let base = draft.name.trim().trim_end_matches("· sketch").trim().to_lowercase();
    let mut same = store.profiles.values().filter(|p| own(&p.id) && p.name.trim().to_lowercase() == base);
    match (same.next(), same.next()) {
        (Some(p), None) => Some(p.id.clone()),
        _ => None,
    }
}

/// The draft `id` goes into the user's profile `target`: its controls, pages and lights; the
/// profile keeps its id, name and apps; the draft is gone. Returns the profile's name.
fn put_version(store: &mut Store, id: &str, target: &str) -> Result<String, String> {
    let draft = store.profiles.get(id).cloned().ok_or_else(|| format!("no profile {id}"))?;
    let mut p = store.profiles.get(target).cloned().ok_or_else(|| format!("no profile {target}"))?;
    p.controls = draft.controls;
    p.pages = draft.pages;
    p.edge = draft.edge;
    p.edge_effect = draft.edge_effect;
    p.edge_speed = draft.edge_speed;
    p.panel = draft.panel;
    p.color = draft.color.or(p.color);
    p.assistant |= draft.assistant;
    let name = p.name.clone();
    store.save_profile(p).map_err(|e| e.to_string())?;
    store.settings.drafts.retain(|d| d != id);
    store.save_settings().map_err(|e| e.to_string())?;
    store.delete_profile(id).map_err(|e| e.to_string())?;
    Ok(name)
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

/// The key panel and the dial get their pictures from threads of their own (2026-10-14: drawn
/// and sent one after the other on the engine thread, both stayed at 10.6 a second on the
/// busiest deck, the engine 77 % busy). A picture is whole, so only the newest waiting one is sent;
/// a module handed back takes nothing (`Module::send_all` after `release` does nothing).
#[derive(Clone, Copy)]
enum Screen {
    Keys,
    Dial,
}

type Picture = (String, Arc<mh_device::Module>, Vec<report::Report>);

struct PictureSender {
    slot: Arc<(Mutex<Option<Picture>>, std::sync::Condvar)>,
    /// A picture waits or is going out now.
    busy: Arc<std::sync::atomic::AtomicBool>,
}

impl PictureSender {
    fn send(&self, id: String, module: Arc<mh_device::Module>, reports: Vec<report::Report>) {
        let (lock, wake) = &*self.slot;
        self.busy.store(true, std::sync::atomic::Ordering::SeqCst);
        *lock.lock().unwrap_or_else(|e| e.into_inner()) = Some((id, module, reports));
        wake.notify_one();
    }

    fn busy(&self) -> bool {
        self.busy.load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// Pictures sent and the time they took, per screen: (count, microseconds), read by `Perf`.
static PICTURES_SENT: [(std::sync::atomic::AtomicU64, std::sync::atomic::AtomicU64); 2] =
    [(std::sync::atomic::AtomicU64::new(0), std::sync::atomic::AtomicU64::new(0)), (std::sync::atomic::AtomicU64::new(0), std::sync::atomic::AtomicU64::new(0))];

fn picture_failed() -> &'static Mutex<HashSet<String>> {
    static FAILED: std::sync::OnceLock<Mutex<HashSet<String>>> = std::sync::OnceLock::new();
    FAILED.get_or_init(Default::default)
}

/// Modules whose last picture failed (drawn and sent again).
fn picture_failures() -> Vec<String> {
    picture_failed().lock().unwrap_or_else(|e| e.into_inner()).drain().collect()
}

/// A test copy draws its virtual keys and dial for the window's mirror and sends them nowhere, so
/// the window can be measured with the deck's pictures coming in (owner's pick 2a, 2026-10-16:
/// the window as a stage of the release gate, on the test copy).
fn mirror_only() -> bool {
    crate::automation::get().is_some()
}

fn picture_sender(screen: Screen) -> &'static PictureSender {
    static SENDERS: [std::sync::OnceLock<PictureSender>; 2] = [std::sync::OnceLock::new(), std::sync::OnceLock::new()];
    let i = screen as usize;
    SENDERS[i].get_or_init(|| {
        let slot: Arc<(Mutex<Option<Picture>>, std::sync::Condvar)> = Arc::new((Mutex::new(None), std::sync::Condvar::new()));
        let mine = slot.clone();
        let busy = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let busy_flag = busy.clone();
        let name = if i == 0 { "keys pictures" } else { "dial pictures" };
        let spawned = std::thread::Builder::new().name(name.into()).spawn(move || {
            let (lock, wake) = &*mine;
            loop {
                let next = {
                    let mut waiting = lock.lock().unwrap_or_else(|e| e.into_inner());
                    while waiting.is_none() {
                        waiting = wake.wait(waiting).unwrap_or_else(|e| e.into_inner());
                    }
                    waiting.take()
                };
                let Some((id, module, reports)) = next else { continue };
                let started = Instant::now();
                match module.send_all(&reports) {
                    Ok(()) => {
                        let (n, us) = &PICTURES_SENT[i];
                        n.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        us.fetch_add(started.elapsed().as_micros() as u64, std::sync::atomic::Ordering::Relaxed);
                    }
                    Err(e) => {
                        log::warn!("{name} {id}: {e}");
                        picture_failed().lock().unwrap_or_else(|e| e.into_inner()).insert(id);
                    }
                }
                // free again unless a newer picture came while this one went out
                let waiting = lock.lock().unwrap_or_else(|e| e.into_inner());
                if waiting.is_none() {
                    busy_flag.store(false, std::sync::atomic::Ordering::SeqCst);
                }
            }
        });
        if let Err(e) = spawned {
            log::warn!("{name} thread: {e}");
        }
        PictureSender { slot, busy }
    })
}

/// What the deck really got while things moved: pictures of the key panel and the dial, LED
/// batches and how busy the engine thread was, written to the log every 10 s (owner, 2026-10-14:
/// "jakiś stress test ... co tam zamula"). Quiet while nothing moves.
struct Perf {
    since: Instant,
    panel: u32,
    panel_time: Duration,
    dial: u32,
    dial_time: Duration,
    leds: u32,
    busy: Duration,
}

impl Perf {
    fn new() -> Self {
        Self { since: Instant::now(), panel: 0, panel_time: Duration::ZERO, dial: 0, dial_time: Duration::ZERO, leds: 0, busy: Duration::ZERO }
    }

    fn note(&mut self) {
        let secs = self.since.elapsed().as_secs_f32();
        if secs < 10.0 {
            return;
        }
        calls::flush();
        let sent = |i: usize| {
            let (n, us) = &PICTURES_SENT[i];
            (n.swap(0, std::sync::atomic::Ordering::Relaxed), us.swap(0, std::sync::atomic::Ordering::Relaxed))
        };
        let (keys_sent, keys_us) = sent(0);
        let (dial_sent, dial_us) = sent(1);
        if self.panel + self.dial >= 30 {
            let ms = |d: Duration, n: u32| if n == 0 { 0.0 } else { d.as_secs_f32() * 1000.0 / n as f32 };
            let send_ms = |us: u64, n: u64| if n == 0 { 0.0 } else { us as f32 / 1000.0 / n as f32 };
            log::info!(
                "deck: keys {:.1}/s sent of {:.1} drawn ({:.1} ms to draw, {:.1} to send), dial {:.1}/s of {:.1} ({:.1} ms, {:.1}), LEDs {:.1}/s, engine busy {:.0} %",
                keys_sent as f32 / secs,
                self.panel as f32 / secs,
                ms(self.panel_time, self.panel),
                send_ms(keys_us, keys_sent),
                dial_sent as f32 / secs,
                self.dial as f32 / secs,
                ms(self.dial_time, self.dial),
                send_ms(dial_us, dial_sent),
                self.leds as f32 / secs,
                self.busy.as_secs_f32() * 100.0 / secs
            );
        }
        *self = Self::new();
    }
}

/// The knobs', faders' and base's LED reports go out on a thread of their own (2026-10-14: HubDeck's
/// rainbow, sent every 100 ms on the engine thread, held the key panel back: 9 pictures a second
/// on the Fun page instead of 15). In order, nothing dropped: each batch is the change from the one
/// before. Those modules are not the keys or the dial, so their reports go alongside the pictures.
struct LedSender {
    tx: crossbeam_channel::Sender<(String, Arc<mh_device::Module>, Vec<report::Report>)>,
    failed: Arc<Mutex<HashSet<String>>>,
    /// Batches given and not sent yet, by module: lights to the music wait instead of queueing,
    /// so the deck never shows a beat from the past (owner, 2026-10-19: "mam wrazenie ze jest duzy
    /// delay"); by module, or the knobs always found the edge's batch waiting and never changed
    /// (owner: "galki nie mrugaja").
    pending: Arc<Mutex<HashMap<String, usize>>>,
}

impl LedSender {
    fn send(&self, batch: (String, Arc<mh_device::Module>, Vec<report::Report>)) {
        let id = batch.0.clone();
        *self.pending.lock().unwrap_or_else(|e| e.into_inner()).entry(id.clone()).or_default() += 1;
        if self.tx.send(batch).is_err() {
            done_with(&self.pending, &id);
        }
    }

    /// A batch of this module is still going out.
    fn busy(&self, id: &str) -> bool {
        self.pending.lock().unwrap_or_else(|e| e.into_inner()).get(id).is_some_and(|n| *n > 0)
    }
}

fn done_with(pending: &Mutex<HashMap<String, usize>>, id: &str) {
    if let Some(n) = pending.lock().unwrap_or_else(|e| e.into_inner()).get_mut(id) {
        *n = n.saturating_sub(1);
    }
}

fn led_sender() -> &'static LedSender {
    static SENDER: std::sync::OnceLock<LedSender> = std::sync::OnceLock::new();
    SENDER.get_or_init(|| {
        let (tx, rx) = crossbeam_channel::unbounded::<(String, Arc<mh_device::Module>, Vec<report::Report>)>();
        let failed = Arc::new(Mutex::new(HashSet::new()));
        let failures = failed.clone();
        let pending: Arc<Mutex<HashMap<String, usize>>> = Arc::new(Mutex::new(HashMap::new()));
        let left = pending.clone();
        let spawned = std::thread::Builder::new().name("leds".into()).spawn(move || {
            // how fast the LEDs really go out, every 10 s while they change (the engine counts
            // what it gives, not what the deck took)
            let (mut since, mut sent, mut busy) = (Instant::now(), 0u32, Duration::ZERO);
            for (id, module, reports) in rx {
                let started = Instant::now();
                if let Err(e) = module.send_all(&reports) {
                    log::warn!("leds {id}: {e}");
                    failures.lock().unwrap_or_else(|e| e.into_inner()).insert(id.clone());
                }
                busy += started.elapsed();
                sent += 1;
                done_with(&left, &id);
                if since.elapsed() >= Duration::from_secs(10) {
                    log::info!("leds: {:.1}/s sent, {:.1} ms each", sent as f32 / since.elapsed().as_secs_f32(), busy.as_secs_f32() * 1000.0 / sent.max(1) as f32);
                    (since, sent, busy) = (Instant::now(), 0, Duration::ZERO);
                }
            }
        });
        if let Err(e) = spawned {
            log::warn!("LED thread: {e}");
        }
        LedSender { tx, failed, pending }
    })
}

/// Modules whose LED reports failed since the last look.
fn led_failures() -> Vec<String> {
    led_sender().failed.lock().unwrap_or_else(|e| e.into_inner()).drain().collect()
}

/// When each step of a multi-action runs: every delay counts from the step before it.
fn schedule_steps(start: Instant, params: &serde_json::Value) -> Vec<(Instant, Binding, String)> {
    schedule_steps_list(start, actions::steps(params))
}

/// The steps of a toggle's first press as bindings (their state tells which side is on).
fn toggle_first(binding: &Binding) -> Vec<Binding> {
    actions::steps_at(&binding.params, "first").into_iter().map(|s| Binding { action: s.action, params: s.params, look: Look::default(), accel: false, look_on: None, swap_turns: false }).collect()
}

/// A toggle's side is kept per profile: the same key on two profiles is two toggles.
fn toggle_key(profile: Option<&str>, control: &str) -> String {
    format!("{}/{control}", profile.unwrap_or(""))
}

/// On when every first step with a readable state is on; nothing readable: the remembered side.
fn toggle_side(read: &[bool], remembered: Option<bool>) -> bool {
    if read.is_empty() { remembered.unwrap_or(false) } else { read.iter().all(|on| *on) }
}

const TOGGLES_FILE: &str = "toggles.json";

fn load_toggles(data_dir: &Path) -> HashMap<String, bool> {
    std::fs::read(data_dir.join(TOGGLES_FILE)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// Written on a press of a toggle (a few bytes, not on a timer).
fn save_toggles(data_dir: &Path, toggles: &HashMap<String, bool>) {
    if let Ok(json) = serde_json::to_vec_pretty(toggles)
        && let Err(e) = profile::write_atomic(&data_dir.join(TOGGLES_FILE), &json)
    {
        log::warn!("{TOGGLES_FILE}: {e}");
    }
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
    let changed = |i: usize, led: &leds::Led| old.and_then(|o| o.leds.get(i)).map(|l| l.rgb) != Some(led.rgb);
    // every LED of the module in one color (lights to the music, a style): one report for all of
    // them, not one each (2026-10-19: 5 faders and 3 knobs to the music took 19.5 % of a core)
    let one_color = new.leds.first().filter(|first| new.leds.len() > 1 && new.leds.iter().all(|l| l.rgb == first.rgb));
    if let (false, Some(first)) = (base, one_color) {
        if new.leds.iter().enumerate().any(|(i, l)| changed(i, l)) {
            out.extend(report::led_color(kind, LedTarget::All, first.rgb, false));
        }
    } else {
        for (i, led) in new.leds.iter().enumerate() {
            if changed(i, led) {
                out.extend(report::led_color(kind, target(i), led.rgb, false));
            }
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
    /// A light follows the music now (`beat.rs`): the LEDs are set every [`MUSIC_STEP`].
    music_running: bool,
    /// The edge's music color and brightness of the last step, for the dial's ring.
    music_glow: Option<(String, f32)>,
    /// The edge follows the music (knobs and faders join it by their switch).
    music_edge: bool,
    /// When each module last got a music brightness.
    music_sent: HashMap<String, Instant>,
    /// The LEDs of each module as set before the music's brightness (to tell a new color from a
    /// new brightness).
    leds_shown: HashMap<String, Option<leds::Lights>>,
    /// When each control was last turned, for acceleration.
    last_turn: HashMap<String, Instant>,
    /// Animated pictures on screen: name -> (frame shown, when the next one is due).
    frames: HashMap<String, (usize, Instant)>,
    /// The time the panel clock shows, so it redraws once a minute.
    clock_shown: String,
    /// Countdown timers per profile and control (`keys.3`, `dial`; gestures of a key share its
    /// timer): the same key on another profile is another timer.
    timers: HashMap<(String, String), timer::Timer>,
    /// A clock's view, hours and stopwatch on a control (its gestures), by control.
    clocks: HashMap<String, timer::ClockState>,
    /// A pomodoro's phase and rounds, by control.
    pomodoros: HashMap<String, timer::Pomodoro>,
    /// What a weather scene shows (the hours ahead, the days), by control.
    weather_view: HashMap<String, render::WeatherView>,
    /// Mochi, the dial pet (one per computer, remembered in pet.json).
    pet: pet::Pet,
    /// When Mochi was last drawn while resting.
    pet_frame: Instant,
    /// Mochi's resting frames as sent, by the hash of their picture: its rest is a loop
    /// (`pet::LOOP`), so after one round the dial only sends (at most `PET_FRAMES_KEPT`).
    pet_jpegs: HashMap<u64, Vec<u8>>,
    /// When the dial's picture last went to the window ([`DIAL_MIRROR_EVERY`]).
    dial_mirror_at: Instant,
    /// When the key panel's picture last went to the window.
    keys_mirror_at: Instant,
    /// A newer dial picture waits for the window ([`Worker::send_mirrors`]).
    dial_mirror_due: bool,
    /// A newer key panel picture waits for the window.
    keys_mirror_due: bool,
    /// Only animations move on the keys (GIFs, the screensaver), which the window plays itself:
    /// the key panel goes to it once a second, for what else changes (clocks, values).
    keys_mirror_slow: bool,
    /// The screensaver the window was last told about (effect, color).
    saver_published: Option<(String, String)>,
    /// The metric a system monitor was turned to, by control.
    monitor_pick: HashMap<String, monitor::Metric>,
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
    /// Where in the song and when that was read; the share drawn on the dial last (per mille).
    progress: Option<(f32, f32, bool, Instant)>,
    progress_drawn: Option<u16>,
    /// Scrubbing with the dial: where the song should go and when the dial last turned; the player
    /// is told once the turning pauses, and its own readings wait until then.
    seek_to: Option<(f32, Instant)>,
    seek_sent: Option<Instant>,
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
    /// The next effect a "Screensaver: Next" key shows.
    saver_next: usize,
    /// A moving key panel has a frame not drawn yet (USB was busy or it came too soon).
    keys_waiting: bool,
    keys_moved: Instant,
    /// What the deck really gets while things move, noted in the log every 10 s.
    perf: Perf,
    fleet_sweep: Instant,
    /// LED colors set by plugins and the local API: `knobs.1`, `faders.0`, `base`.
    plugin_leds: HashMap<String, Rgb>,
    /// A picture for the whole key panel set by a plugin or the API (data URI).
    plugin_panel: Option<String>,
    /// Toggles that ran their first steps last (the next press runs the second), by
    /// `profile/control`; kept in `toggles.json` so a toggle shows the same side after a restart.
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
    /// The backlight last sent to each keys and dial module, by module id (a module that left is
    /// forgotten, so it gets it again when it comes back).
    backlight_sent: HashMap<String, u8>,
    last_blank_check: Instant,
    /// The tour's highlight as last drawn: the control and whether it is lit now.
    highlight_shown: Option<(String, bool)>,
    /// A key just pressed (index, since when): its frame shows the press arrived.
    press_flash: Option<(usize, Instant)>,
    /// Keys whose last press failed, by `profile|control id` without the gesture, and why.
    failed_keys: HashMap<String, String>,
}

impl Worker {
    fn lock(&self) -> MutexGuard<'_, Shared> {
        self.shared.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn run(mut self, rx: Receiver<Msg>) {
        volume::init_thread();
        let mut last_dial = Instant::now() - DIAL_FRAME;
        let mut busy_from: Option<Instant> = None;
        loop {
            if let Some(b) = busy_from.take() {
                self.perf.busy += b.elapsed();
            }
            self.perf.note();
            let failed = picture_failures();
            if !failed.is_empty() {
                for id in &failed {
                    self.sent.remove(id);
                }
                self.redraw_keys();
                self.dial_dirty = true;
            }
            let tick = if self.arcade.is_some() { Duration::from_millis(25) } else { Duration::from_millis(40) };
            // wake for the next frame of an animation on screen, not up to a whole tick later
            let now = Instant::now();
            let wait = self.frames.values().map(|(_, ends)| ends.saturating_duration_since(now)).min().map_or(tick, |d| d.clamp(Duration::from_millis(10), tick));
            // a key frame that waits (the last one still going out, or too soon): wake when it may
            // go, not at the next tick; on the 40 ms grid a 36 ms send and the 50 ms gap meant a
            // frame every 80 ms, 12.5-14.5 a second (owner's pick, 2026-10-16: GIFs want 15)
            // lights to the music: the next step on time, not at the next tick
            let wait = if self.music_running { wait.min(MUSIC_STEP) } else { wait };
            // the screensaver's next frame on time too: on the tick it ran at 11 a second (2026-10-19)
            let wait = if self.saver_on { wait.min((self.saver_frame + Duration::from_millis(saver::FRAME_MS)).saturating_duration_since(now).max(Duration::from_millis(2))) } else { wait };
            let wait = if self.keys_waiting {
                let gap = (self.keys_moved + KEYS_MOVING_EVERY).saturating_duration_since(now);
                let poll = if picture_sender(Screen::Keys).busy() { Duration::from_millis(4) } else { gap };
                wait.min(gap.max(poll)).max(Duration::from_millis(2))
            } else {
                wait
            };
            let first = rx.recv_timeout(wait);
            busy_from = Some(Instant::now());
            let first = match first {
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
                        // only modules that came back get their backlight again: each set is a feature
                        // report the keys and dial sometimes never answer on macOS (2026-10-14: a turn of
                        // the base resent it and the keys timed out)
                        let present: HashSet<String> = self.deck.snapshot().modules.iter().map(|m| m.id.clone()).collect();
                        self.backlight_sent.retain(|id, _| present.contains(id));
                        self.redraw_keys();
                        self.update_leds();
                        self.dial_dirty = true;
                    }
                    Msg::WindowTouched => {
                        self.last_deck_input = Instant::now();
                        if self.saver_on {
                            self.lock().saver_preview = None;
                            self.saver_on = false;
                            log::info!("screensaver off (the window)");
                            self.redraw_keys();
                        }
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
                    Msg::StopTimer { profile, control } => self.stop_timer(profile, control),
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
                    Msg::Progress(p) => self.on_progress(p),
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
            if (self.rainbow_running && now.duration_since(self.rainbow_last) >= RAINBOW_STEP) || (self.music_running && now.duration_since(self.rainbow_last) >= MUSIC_STEP) {
                self.rainbow_last = now;
                self.update_leds();
            }
            self.send_seek(now);
            self.animate(now);
            self.send_mirrors();
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
        // the rollers did nothing on the owner's Mac (2026-10-14): every turn shows in the log for now
        if control.starts_with("rollers") {
            let s = self.lock();
            log::info!("input {control} {event:?} (profile {:?}, bound: {})", s.active, s.active_profile().is_some_and(|p| p.controls.contains_key(control)));
        }
        if self.blanked {
            // the first touch only wakes the deck; it does not run an action
            self.blanked = false;
            log::info!("screens on (a touch on the deck)");
            self.apply_screens();
            self.redraw_keys();
            return;
        }
        if self.saver_on {
            // the release of the key that asked for a preview is not a touch that ends it
            if matches!(event, InputEvent::Press(false)) {
                return;
            }
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
        // a key shows at once that its press arrived (owner's pick 1a, 2026-10-16)
        if let (Some(index), InputEvent::Press(true)) = (control.strip_prefix("keys.").and_then(|i| i.parse::<usize>().ok()), event) {
            self.press_flash = Some((index, Instant::now()));
            self.redraw_keys();
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
        // the tour ticks gestures off as they happen: a hold or a double press anywhere ("One control,
        // many jobs"), and the dial's turn and press too ("Your dial, many faces")
        if self.lock().tour_dry_run {
            let name = id.rsplit('/').next().unwrap_or(id);
            let tick = if name.ends_with(":hold") || name.ends_with(":double") {
                Some(name.to_string())
            } else if name == "dial" {
                match event {
                    InputEvent::Press(true) => Some("dial:press".to_string()),
                    InputEvent::Turn(_) => Some("dial:turn".to_string()),
                    _ => None,
                }
            } else {
                None
            };
            if let Some(tick) = tick {
                let _ = self.app.emit("tour://gesture", tick);
            }
        }
        let Some(binding) = self.lock().active_profile().and_then(|p| p.controls.get(id).cloned()) else { return };
        if binding.action == actions::GAMES {
            if matches!(event, InputEvent::Press(true)) {
                self.set_playing(true);
            }
            return;
        }
        if binding.action == actions::TOGGLE {
            if matches!(event, InputEvent::Press(true)) {
                let key = toggle_key(self.lock().active.as_deref(), id);
                let on = self.toggle_on(&key, &binding);
                let list = actions::steps_at(&binding.params, if on { "second" } else { "first" });
                self.scheduled.extend(schedule_steps_list(Instant::now(), list));
                self.scheduled.make_contiguous().sort_by_key(|(due, _, _)| *due);
                self.toggles.insert(key, !on);
                save_toggles(&self.data_dir, &self.toggles);
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
                && let Some(state) = sdplugins::toggle_after_key_up(&binding.action, id)
            {
                self.plugin_looks.entry(id.to_string()).or_default().state = Some(state);
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
        // scrubbing a song with the dial: the ring moves at every detent, the player is told once
        // the turning pauses (owner: it jumped every few seconds instead of moving smoothly)
        if binding.action == actions::NOW_PLAYING
            && actions::param_str(&binding.params, "turn") == "seek"
            && let InputEvent::Turn(d) = event
            && let Some(share) = self.progress_share()
            && let Some((_, of, playing, _)) = self.progress
        {
            // two seconds a detent; a fast spin (several detents at once) goes three times as far
            let step = if d.unsigned_abs() >= 3 { 6.0 } else { 2.0 };
            let at = (share * of + f32::from(d) * step).clamp(0.0, (of - 1.0).max(0.0));
            self.progress = Some((at, of, playing, Instant::now()));
            self.seek_to = Some((at, Instant::now()));
            self.dial_dirty = true;
            return;
        }
        self.executor.foreground = foreground;
        self.executor.mapped = mapped;
        // a key's red corner: set when its press fails, gone when one works (the gesture's suffix
        // is not part of the key's place)
        let active = self.lock().active.clone().unwrap_or_default();
        let key_place = format!("{active}|{}", id.split(':').next().unwrap_or(id));
        let result = self.executor.run(binding, event);
        let was_failed = self.failed_keys.contains_key(&key_place);
        match &result {
            Ok(_) => {
                self.failed_keys.remove(&key_place);
            }
            Err(message) if key_place.rsplit(['/', '|']).next().is_some_and(|c| c.starts_with("keys.")) => {
                self.failed_keys.insert(key_place.clone(), message.clone());
            }
            Err(_) => {}
        }
        if was_failed != self.failed_keys.contains_key(&key_place) {
            self.redraw_keys();
        }
        match result {
            Ok(out) => {
                if let Some(page) = out.page {
                    let page = if page == exec::NEXT_PAGE || page == exec::PREVIOUS_PAGE { self.page_beside(page == exec::NEXT_PAGE) } else { (!page.is_empty()).then_some(page) };
                    self.show_page(page);
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
                    let profile_id = self.lock().active.clone().unwrap_or_default();
                    let t = self.timers.entry((profile_id, control)).or_insert_with(|| timer::Timer::new(total));
                    if !t.running() && t.total != total && !matches!(cmd, timer::TimerCmd::Adjust(_)) && t.remaining(now) == t.total {
                        // its length was changed in the editor since
                        *t = timer::Timer::new(total);
                    }
                    t.apply(cmd, now);
                    self.live_sig.clear();
                    self.live_dirty = true;
                    self.update_leds();
                }
                if let Some(cmd) = out.clock {
                    let control = id.split(':').next().unwrap_or(id).to_string();
                    // the base control's own setting (a gesture's binding may carry other params)
                    let own = self.lock().active_profile().and_then(|p| p.controls.get(&control).cloned()).unwrap_or_else(|| binding.clone());
                    let (view, h12) = (actions::param_str(&own.params, "show").to_string(), actions::param_str(&own.params, "hours") == "12");
                    self.clocks.entry(control).or_default().apply(cmd, Instant::now(), &view, h12);
                    self.live_sig.clear();
                    self.live_dirty = true;
                }
                if let Some(cmd) = out.pomodoro {
                    let control = id.split(':').next().unwrap_or(id).to_string();
                    let own = self.lock().active_profile().and_then(|p| p.controls.get(&control).cloned()).unwrap_or_else(|| binding.clone());
                    self.pomodoros.entry(control).or_insert_with(|| timer::pomodoro_of(&own.params)).apply(cmd, Instant::now());
                    self.live_sig.clear();
                    self.live_dirty = true;
                }
                if let Some(cmd) = out.pet {
                    self.pet.apply(cmd, Instant::now());
                    self.dial_dirty = true;
                }
                if out.pet_dial {
                    self.toggle_pet_dial(id.split(':').next().unwrap_or(id));
                }
                if let Some(effect) = out.saver {
                    let effect = if saver::EFFECTS.contains(&effect.as_str()) {
                        effect
                    } else {
                        let next = saver::EFFECTS[self.saver_next % saver::EFFECTS.len()];
                        self.saver_next += 1;
                        next.to_string()
                    };
                    self.lock().saver_preview = Some((effect, Instant::now() + Duration::from_secs(12)));
                }
                if let Some(cmd) = out.weather {
                    let control = id.split(':').next().unwrap_or(id).to_string();
                    let v = self.weather_view.entry(control).or_default();
                    match cmd {
                        exec::WeatherCmd::Hours(d) => {
                            v.mode = 0;
                            v.hour = (v.hour as i32 + d).clamp(0, 23) as usize;
                        }
                        exec::WeatherCmd::View => {
                            v.mode = (v.mode + 1) % 4;
                            v.hour = 0;
                        }
                        exec::WeatherCmd::Forecast => {
                            let fc = live::value_key(&binding.params).and_then(|k| self.values.get(&k)).and_then(|r| serde_json::from_str::<live::Reading>(r).ok()).and_then(|r| r.forecast);
                            let url = fc.map_or_else(|| "https://www.meteoblue.com/".to_string(), |f| format!("https://www.meteoblue.com/en/weather/week/{:.3}N{:.3}E", f.lat, f.lon));
                            if let Err(e) = crate::system::shell_open(&url) {
                                log::warn!("weather forecast: {e}");
                            }
                        }
                    }
                    self.dial_dirty = true;
                }
                if let Some(step) = out.monitor_step {
                    let control = id.split(':').next().unwrap_or(id).to_string();
                    let own = self.lock().active_profile().and_then(|p| p.controls.get(&control).cloned()).unwrap_or_else(|| binding.clone());
                    let now = self.monitor_value(&control, &own).map(|(m, _)| m).unwrap_or(monitor::Metric::Cpu);
                    let at = monitor::Metric::CYCLE.iter().position(|m| *m == now).unwrap_or(0) as i32;
                    let next = monitor::Metric::CYCLE[(at + step).rem_euclid(monitor::Metric::CYCLE.len() as i32) as usize];
                    self.monitor_pick.insert(control, next);
                    self.feedback = Some((Feedback { value: None, caption: next.name().to_string() }, Instant::now()));
                    self.redraw_keys();
                    self.dial_dirty = true;
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

    /// The page after (or before) the one shown, the main page counting as the first; it wraps,
    /// so one key steps through all of them.
    fn page_beside(&self, next: bool) -> Option<String> {
        let s = self.lock();
        let mut all: Vec<Option<String>> = vec![None];
        all.extend(s.active_profile().map(|p| p.pages.iter().map(|x| Some(x.id.clone())).collect::<Vec<_>>()).unwrap_or_default());
        let at = all.iter().position(|x| *x == s.page).unwrap_or(0);
        let n = all.len();
        all[if next { (at + 1) % n } else { (at + n - 1) % n }].clone()
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
        self.watch_live();
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

    /// The user picked a profile (on the deck, by a plugin): app profiles give back to it, then it
    /// shows. The start profile stays as it is ("Set as home" sets it; owner, 2026-10-22).
    fn choose(&mut self, id: &str) {
        {
            let mut s = self.lock();
            if !s.store.profiles.contains_key(id) {
                return;
            }
            s.home = Some(id.to_string());
            s.auto_app = None;
            if s.store.settings.active.as_deref() != Some(id) {
                s.store.settings.active = Some(id.to_string());
                if let Err(e) = s.store.save_settings() {
                    log::warn!("cannot save the picked profile: {e}");
                }
            }
        }
        let name = self.lock().store.profiles.get(id).map(|p| p.name.clone()).unwrap_or_default();
        log::info!("picked on the deck: profile \"{name}\"");
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
        self.publish_saver();
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
            // a test copy's virtual keys have no module: their picture is drawn for the window only
            let module = self.deck.module(&m.id);
            if module.is_none() && !mirror_only() {
                continue;
            }
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
                                .map(|n| n == 1)
                                .or_else(|| (b.action == actions::TOGGLE).then(|| self.toggle_on(&toggle_key(profile.as_ref().map(|p| p.id.as_str()), &id), b)))
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
                            match (self.monitor_value(&id, b), self.live_face(&id, b, Instant::now())) {
                                // a monitor key: the metric and its value instead of the label
                                (Some((metric, value)), _) => {
                                    // the value big, the metric's name under it, like a clock face (the dial draws it the same way)
                                    Look { label: Some(format!("{}\n{}", value.map(|v| metric.format(v)).unwrap_or_else(|| "–".into()), metric.name())), icon: None, face: true, ..look }
                                }
                                // a clock or timer: its text, big; a timer that ran out turns red
                                (None, Some(face)) => {
                                    // and blinks, twice a second, until a press (owner, 2026-10-16: it
                                    // should say more strongly that it ran out)
                                    let base = if face.done && Self::blink_on() { b.look_for(Some(true)) } else { look };
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
            // a press just arrived, and the keys whose last press failed
            if let Some((i, _)) = self.press_flash
                && let Some(look) = looks.get_mut(i)
            {
                look.pressed = true;
            }
            for (i, look) in looks.iter_mut().enumerate() {
                let place = format!("{}|{}", profile.as_ref().map_or("", |p| p.id.as_str()), control_id(page.as_deref(), &format!("keys.{i}")));
                look.failed = self.failed_keys.contains_key(&place);
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
            // screens off: black keys (the backlight is never set to 0, see apply_screens)
            let looks = if self.blanked { vec![Look { bg: Some("#000000".into()), ..Look::default() }; 15] } else { looks };
            let game_panel = game_panel.filter(|_| !self.blanked);
            // a plugin's panel picture wins over the profile's
            let plugin_panel = self.plugin_panel.clone().filter(|_| self.arcade.is_none()).map(|image| profile::PanelContent::Picture { image });
            let panel_content = plugin_panel.or_else(|| profile.as_ref().filter(|_| self.arcade.is_none()).and_then(|p| p.panel.clone())).filter(|_| !self.blanked).map(|c| match c {
                profile::PanelContent::Picture { image } => {
                    let mut look = Look { image: Some(image), ..Look::default() };
                    self.current_frame(&mut look);
                    profile::PanelContent::Picture { image: look.image.unwrap_or_default() }
                }
                other => other,
            });
            if let (Some(CompareVariant::A), Some(module)) = (compare, &module) {
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
            // moving pictures (GIF frames, the screensaver) go lighter: a panel at quality 98 was
            // ~60 KB and took ~60 ms to send on Windows, so the Fun page ran at 7 pictures a second
            // (measured 2026-10-14); at rest the keys keep the full quality
            let moving = self.saver_on || looks.iter().any(|l| l.image.as_deref().is_some_and(|i| i.contains('#')));
            // the window plays GIFs itself; the screensaver it still takes from here (drawn in the
            // window it grew WebView2 by 45 MB a minute, 2026-10-16)
            self.keys_mirror_slow = WINDOW_PLAYS_GIFS && moving && !self.saver_on;
            let quality = (compare == Some(CompareVariant::C)).then_some(100).or(moving.then_some(MOVING_QUALITY));
            // one full-panel image: ~11 ms over USB and all keys change at once
            let mut tiles: Option<Vec<render::Tile>> = None;
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
                // the keys as tiles: a key that did not change is not drawn again
                (None, false) => {
                    let t = render::panel_tiles(&looks, rotation, &geometry);
                    let sig = t.iter().map(|t| t.svg.as_str()).collect::<String>();
                    tiles = Some(t);
                    sig
                }
            };
            let svg = if quality.is_some() { format!("{svg}<!--q100-->") } else { svg };
            if self.sent.get(&m.id) == Some(&svg) {
                continue;
            }
            let drawing = Instant::now();
            let jpeg = match &tiles {
                Some(t) => self.renderer.panel_jpeg(t, quality),
                None => self.renderer.jpeg_with(&svg, quality),
            };
            self.perf.panel += 1;
            self.perf.panel_time += drawing.elapsed();
            match jpeg {
                // sent on the keys' own thread, so the dial and the next frame do not wait for USB
                Ok(jpeg) => {
                    if let Some(module) = &module {
                        picture_sender(Screen::Keys).send(m.id.clone(), module.clone(), report::panel_image(&jpeg, module.output_len));
                    }
                    self.sent.insert(m.id.clone(), svg);
                    self.mirror_keys(profile.as_ref().map(|p| p.id.as_str()), page.as_deref(), &m.id, &jpeg, &geometry);
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
                if !same {
                    spotify::track_changed();
                }
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

    /// The song moved on: the dial's ring is drawn again when it moved by a percent or more.
    fn on_progress(&mut self, p: Option<(f32, f32, bool)>) {
        // while scrubbing (and a moment after), the player's own reading is older than the dial
        if self.seek_to.is_some() || self.seek_sent.is_some_and(|t| t.elapsed() < Duration::from_millis(1500)) {
            return;
        }
        self.progress = p.map(|(at, of, playing)| (at, of, playing, Instant::now()));
        if self.progress_share().map(|v| (v * 1000.0) as u16).is_some_and(|v| self.progress_drawn.is_none_or(|d| v.abs_diff(d) >= 10)) || (self.progress.is_none() && self.progress_drawn.is_some()) {
            let on_dial = self.lock().active_profile().is_some_and(|p| p.controls.get("dial").is_some_and(|b| b.action == actions::NOW_PLAYING));
            if on_dial {
                self.dial_dirty = true;
            }
        }
    }

    /// Tell the player where to go once the dial has stood still for a moment.
    fn send_seek(&mut self, now: Instant) {
        if let Some((at, turned)) = self.seek_to
            && now.duration_since(turned) >= Duration::from_millis(180)
        {
            self.seek_to = None;
            self.seek_sent = Some(now);
            std::thread::spawn(move || {
                if let Err(e) = media::seek_to(f64::from(at)) {
                    log::warn!("seek: {e}");
                }
            });
        }
    }

    /// How much of the song has played now (0..=1), from the last reading.
    fn progress_share(&self) -> Option<f32> {
        let (at, of, playing, read) = self.progress?;
        let at = if playing { at + read.elapsed().as_secs_f32() } else { at };
        Some((at / of).clamp(0.0, 1.0))
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
    fn monitor_value(&self, control: &str, binding: &Binding) -> Option<(monitor::Metric, Option<f32>)> {
        (binding.action == actions::MONITOR).then(|| {
            let metric = self.monitor_pick.get(control).copied().unwrap_or_else(|| monitor::Metric::from_param(actions::param_str(&binding.params, "metric")));
            (metric, self.samples.back().and_then(|s| s.get(metric)))
        })
    }

    /// What a clock or timer shows now; None for other actions.
    fn live_face(&self, control: &str, binding: &Binding, now: Instant) -> Option<LiveFace> {
        let p = &binding.params;
        match binding.action.as_str() {
            actions::CLOCK => {
                let state = self.clocks.get(control.split(':').next().unwrap_or(control));
                // a stopwatch that runs or was stopped shows instead of the time
                if let Some(d) = state.and_then(|c| c.stopwatch(now)) {
                    let polish = self.lock().store.settings.language.starts_with("pl");
                    let caption = match (state.is_some_and(timer::ClockState::running), polish) {
                        (true, false) => "Stopwatch",
                        (true, true) => "Stoper",
                        (false, false) => "Stopwatch · stopped",
                        (false, true) => "Stoper · zatrzymany",
                    };
                    return Some(LiveFace { main: timer::stopwatch_text(d), second: Some(caption.to_string()), ring: None, done: false });
                }
                let show = state.and_then(|c| c.view).map_or(actions::param_str(p, "show"), |v| timer::CLOCK_VIEWS[v]);
                let hours = match state.and_then(|c| c.h12) {
                    Some(true) => "12",
                    Some(false) => "24",
                    None => actions::param_str(p, "hours"),
                };
                let (main, second) = timer::clock_text(chrono::Local::now().naive_local(), show, hours);
                Some(LiveFace { main, second, ring: None, done: false })
            }
            actions::POMODORO => {
                let fresh = timer::pomodoro_of(p);
                let pomo = self.pomodoros.get(control.split(':').next().unwrap_or(control)).unwrap_or(&fresh);
                let polish = self.lock().store.settings.language.starts_with("pl");
                Some(LiveFace { main: pomo.timer.label(now), second: Some(pomo.phase(polish).to_string()), ring: Some(pomo.timer.fraction(now)), done: false })
            }
            actions::TIMER => {
                let fresh = timer::Timer::new(timer::total_of(p));
                let active = self.lock().active.clone().unwrap_or_default();
                let t = self.timers.get(&(active, control.to_string())).unwrap_or(&fresh);
                let caption = binding.look.label.clone().filter(|l| !l.trim().is_empty());
                // not started yet: no ring, so a press shows at once (owner, 2026-10-14: "25:00" stayed the
                // same for the first second and the press felt a second late)
                let fresh_now = !t.running() && t.remaining(now) == t.total;
                Some(LiveFace { main: t.label(now), second: caption, ring: (!fresh_now).then(|| t.fraction(now)), done: t.finished(now) && t.running() })
            }
            _ => None,
        }
    }

    /// The half second a ringing timer shows its alarm look.
    fn blink_on() -> bool {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(true, |d| (d.as_millis() / 500) % 2 == 0)
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
            "setState" => {
                // a number is a state index; true/false is Hubdeck's own on/off, stored as 1 and 0
                look.state = m["state"].as_u64().map(|n| u8::try_from(n).unwrap_or(u8::MAX)).or_else(|| m["state"].as_bool().map(u8::from));
            }
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
        let index = self.plugin_looks.get(id).and_then(|p| p.state);
        let Some((image, title)) = sdplugins::state_look(&binding.action, index) else { return look };
        let set = self.plugin_looks.get(id);
        let image = set.and_then(|p| p.image.clone()).or(look.image.clone()).or(image);
        // a plugin that draws its own title (ShowTitle false) has the label in its picture already
        let label = if sdplugins::shows_title(&binding.action) { set.and_then(|p| p.title.clone()).or(look.label.clone()).or(title) } else { None };
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
        let mut titles = HashMap::new();
        if let Some(p) = profile {
            for (id, b) in &p.controls {
                let base = id.split(':').next().unwrap_or(id);
                let on_page = match base.split_once('/') {
                    Some((pg, _)) => page.as_deref() == Some(pg),
                    None => !(base.starts_with("keys.") && page.is_some()),
                };
                if let (true, Some((plugin, action))) = (on_page, b.action.split_once('/')) {
                    now.insert(id.clone(), (plugin.to_string(), action.to_string(), b.params.clone()));
                    if sdplugins::is_sd_action(&b.action) {
                        titles.insert(id.clone(), (plugin.to_string(), action.to_string(), b.params.clone(), sdplugins::title_of(&b.action, b.look.label.as_deref())));
                    }
                }
            }
        }
        // what a plugin set on a control that now shows something else is gone with it
        for gone in plugins::set_visible(now) {
            self.plugin_looks.remove(&gone);
        }
        plugins::set_titles(titles);
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
        self.watch_live();
    }

    /// Fetch the live values on screen: the shown page's keys and the dial (a worker starts the
    /// first time one is shown). A page opened from the deck needs it too, or its values stay "…"
    /// (8bitcanvascode, Discussions #3, 2026-10-19).
    fn watch_live(&self) {
        let (profile, page, ha_url) = {
            let s = self.lock();
            (s.active_profile().cloned(), s.page.clone(), s.store.settings.ha_url.clone().unwrap_or_default())
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
        // a toggle's first steps too: their state tells which side the toggle is on
        let mut wanted: Vec<String> = (0..15)
            .filter_map(|i| profile.controls.get(&control_id(page.as_deref(), &format!("keys.{i}"))))
            .chain(profile.controls.get("dial"))
            .flat_map(|b| std::iter::once(b.clone()).chain(if b.action == actions::TOGGLE { toggle_first(b) } else { Vec::new() }))
            .filter_map(|b| Self::state_key(&b))
            .collect();
        wanted.sort();
        wanted.dedup();
        let mut changed = false;
        // which audio devices are the defaults now (one call per kind)
        let mut defaults: HashMap<bool, Vec<audiodev::Device>> = HashMap::new();
        for key in &wanted {
            if let Some(rest) = key.strip_prefix("audio:") {
                let (kind, name) = rest.split_once(':').unwrap_or(("output", rest));
                let input = kind == "input";
                let list = defaults.entry(input).or_insert_with(|| audiodev::list(input).unwrap_or_default());
                let on = audiodev::find(list, name).is_some_and(|d| d.default);
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

    /// Whether a toggle ran its first steps last (the next press runs the second). Read from the
    /// system when its first steps have a state HubDeck can read (an audio device, mute, a light,
    /// OBS recording), so it follows a change made elsewhere or a restart of the PC; else the side
    /// it remembers.
    fn toggle_on(&self, key: &str, binding: &Binding) -> bool {
        let read: Vec<bool> = toggle_first(binding).iter().filter_map(|b| self.state_of(b)).collect();
        toggle_side(&read, self.toggles.get(key).copied())
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
        self.want_integrations();
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
            self.redraw_keys();
            self.dial_dirty = true;
        }
        self.apply_screens();
    }

    /// Send the backlight the settings ask for to each keys and dial module that does not have it
    /// yet (a new module, a changed setting).
    fn apply_screens(&mut self) {
        // screens off draw black images: brightness 0 wedged the dial, then the keys, at screen locks
        // on the owner's Mac (2026-10-14; they took no command after it until reseated)
        let level = self.lock().store.settings.screen_brightness.clamp(1, 100);
        let mut sent_any = false;
        for m in self.deck.snapshot().modules.iter().filter(|m| m.kind == "keys" || m.kind == "dial") {
            if self.backlight_sent.get(&m.id) == Some(&level) {
                continue;
            }
            let Some(module) = self.deck.module(&m.id) else { continue };
            if let Some(r) = mh_protocol::report::backlight(module.kind(), level)
                && let Err(e) = module.send(&r)
            {
                // tried once: a module that does not answer must not hold up every tick for its
                // timeout (5 s each); it gets the level again when it comes back
                log::warn!("backlight {}: {e}", m.id);
            }
            self.backlight_sent.insert(m.id.clone(), level);
            sent_any = true;
        }
        if sent_any && !self.blanked {
            self.leds_sent.clear();
            self.update_leds();
        }
    }

    /// A press on a session key says on the dial which session it is and what it waits for; the
    /// sessions dial turns through them. It no longer pulls the terminal forward: that took a fake
    /// Alt press past Windows' focus lock (owner, 2026-10-16: no workarounds).
    fn agent_input(&mut self, binding: &Binding, event: InputEvent) {
        let session = match (binding.action.as_str(), event) {
            (actions::AGENTS, InputEvent::Turn(d)) => {
                self.fleet.scroll(d as i32);
                self.dial_dirty = true;
                return;
            }
            (actions::AGENTS, InputEvent::Press(true)) => self.fleet.selected_session().map(|s| (s.project.clone(), s.state, s.tool.clone())),
            (_, InputEvent::Press(true)) => {
                let slot = (actions::param_f64(&binding.params, "slot", 1.0).max(1.0) as usize) - 1;
                self.fleet.by_slot(slot).map(|s| (s.project.clone(), s.state, s.tool.clone()))
            }
            _ => return,
        };
        let caption = match session {
            Some((project, state, Some(tool))) if state == agents::State::Permission => format!("{project}: {} ({tool})", state.word()),
            Some((project, state, _)) => format!("{project}: {}", state.word()),
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
            // a steady beat: a frame that came late does not push the next ones back (13 a second)
            self.saver_frame = if now.duration_since(self.saver_frame) < saver_every * 2 { self.saver_frame + saver_every } else { now };
            self.redraw_keys();
        }
        if now.duration_since(self.fleet_sweep) >= Duration::from_secs(60) && self.fleet.forget_stale(now) {
            self.redraw_keys();
            self.dial_dirty = true;
        }
        let running_timer = self.timers.values().any(|t| t.running()) || self.pomodoros.values().any(|p| p.timer.running());
        let (profile, page) = {
            let s = self.lock();
            // nothing that moves (pictures, clocks, timers): no copy of the profile every tick
            let moving = s.active_profile().is_some_and(|p| {
                p.panel.is_some()
                    || p.controls.values().any(|b| {
                        b.look.image.is_some()
                            || b.action == actions::CLOCK
                            || b.action == actions::TIMER
                            || b.action == actions::POMODORO
                            || b.action == gallery::ACTION
                            || b.action == pet::ACTION
                            || (b.action == live::ACTION && actions::param_str(&b.params, "dial") == "scene")
                    })
            });
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
        // moving keys: at most KEYS_MOVING_EVERY apart and never while the last picture is still
        // going out (2026-10-14: 31.8 drawn a second, 21 sent, a third of the drawing thrown away)
        if keys_due || self.keys_waiting {
            if picture_sender(Screen::Keys).busy() || now.duration_since(self.keys_moved) < KEYS_MOVING_EVERY {
                keys_due = false;
                self.keys_waiting = true;
            } else {
                keys_due = true;
                self.keys_waiting = false;
                self.keys_moved = now;
            }
        }
        // Mochi: a dial frame every tick, ~25 a second (owner, 2026-10-14: 12 was too few, without
        // doubling the CPU): the moments are drawn live, the resting loop once (`pet_jpegs`)
        if profile.controls.get("dial").is_some_and(|b| b.action == pet::ACTION) {
            let moving = self.pet.moment.is_some_and(|(_, at)| now.duration_since(at) < Duration::from_millis(2400));
            if moving || now.duration_since(self.pet_frame) >= Duration::from_millis(30) {
                self.pet_frame = now;
                dial_due = true;
            }
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
        // so may a weather scene (its sky moves)
        if profile.controls.get("dial").is_some_and(|b| b.action == gallery::ACTION || b.action == pet::ACTION || (b.action == live::ACTION && actions::param_str(&b.params, "dial") == "scene")) {
            dial_due = true;
        }
        let mut sig = String::new();
        let mut dial_live = false;
        let mut finished: Vec<(String, String)> = Vec::new();
        for (i, control) in (0..15).map(|i| (Some(i), control_id(page.as_deref(), &format!("keys.{i}")))).chain(std::iter::once((None, "dial".to_string()))) {
            let Some(b) = profile.controls.get(&control) else { continue };
            if let Some(face) = self.live_face(&control, b, now) {
                sig.push_str(&control);
                sig.push('=');
                sig.push_str(&face.main);
                if face.done {
                    // a timer that ran out blinks: each half second is a new picture
                    sig.push(if Self::blink_on() { '+' } else { '-' });
                }
                sig.push(';');
                dial_live |= i.is_none();
                let key = (profile.id.clone(), control.clone());
                if b.action == actions::TIMER && self.timers.get(&key).is_some_and(|t| t.finished(now) && t.running() && !t.rang) {
                    finished.push(key);
                }
            }
        }
        // a timer on another page or profile rings too
        for (key, t) in &self.timers {
            if t.running() && t.finished(now) && !t.rang && !finished.contains(key) {
                finished.push(key.clone());
            }
        }
        // a pomodoro whose phase ran out rings and waits on the next phase
        let ended: Vec<String> = self.pomodoros.iter().filter(|(_, p)| p.timer.running() && p.timer.finished(now)).map(|(c, _)| c.clone()).collect();
        for control in ended {
            if let Some(p) = self.pomodoros.get_mut(&control) {
                p.advance();
            }
            self.live_sig.clear();
            if profile.controls.get(&control).is_none_or(|b| actions::param_str(&b.params, "sound") != "off") {
                timer::ring();
            }
        }
        for key in finished {
            let binding = self.lock().store.profiles.get(&key.0).and_then(|p| p.controls.get(&key.1).cloned());
            // the key no longer holds a timer (changed in the editor): it stays quiet
            let Some(binding) = binding.filter(|b| b.action == actions::TIMER) else {
                self.timers.remove(&key);
                continue;
            };
            if let Some(t) = self.timers.get_mut(&key) {
                t.rang = true;
            }
            self.update_leds();
            if actions::param_str(&binding.params, "sound") != "off" {
                timer::ring();
            }
        }
        self.publish_ringing(now);
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

    /// The timers that ran out, for the window (which one rings, with Stop): written to the shared
    /// state and sent to the window only when the list changes.
    fn publish_ringing(&mut self, now: Instant) {
        let mut ringing: Vec<Ringing> = {
            let s = self.lock();
            self.timers
                .iter()
                .filter(|(_, t)| t.running() && t.finished(now))
                .filter_map(|((pid, control), _)| {
                    let p = s.store.profiles.get(pid)?;
                    let b = p.controls.get(control).filter(|b| b.action == actions::TIMER)?;
                    Some(Ringing {
                        profile: pid.clone(),
                        profile_name: p.name.clone(),
                        control: control.clone(),
                        label: b.look.label.clone().filter(|l| !l.trim().is_empty()),
                        page: control.split_once('/').map(|(page, _)| page.to_string()),
                    })
                })
                .collect()
        };
        ringing.sort_by(|a, b| (&a.profile, &a.control).cmp(&(&b.profile, &b.control)));
        let state = {
            let mut s = self.lock();
            if s.ringing == ringing {
                return;
            }
            s.ringing = ringing;
            s.state()
        };
        publish_state(&self.app, state);
    }

    /// The window's Stop on a timer that ran out: back to its full time, not running.
    fn stop_timer(&mut self, profile: String, control: String) {
        if let Some(t) = self.timers.get_mut(&(profile, control)) {
            t.apply(timer::TimerCmd::Reset, Instant::now());
        }
        self.live_sig.clear();
        self.live_dirty = true;
        self.update_leds();
        self.publish_ringing(Instant::now());
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
        // a module whose LED reports failed on the LED thread gets all of them again
        for id in led_failures() {
            self.leds_sent.remove(&id);
        }
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
            // the edge only for a timer that ran out where it cannot be seen (another page or
            // profile): one on the deck blinks itself (owner's pick 1a, 2026-10-16)
            timer_done: {
                let (active, page) = {
                    let s = self.lock();
                    (s.active.clone().unwrap_or_default(), s.page.clone())
                };
                let seen = |(p, c): &(String, String)| *p == active && (c == "dial" || (0..15).any(|i| control_id(page.as_deref(), &format!("keys.{i}")) == *c));
                self.timers.iter().any(|(k, t)| t.running() && t.finished(Instant::now()) && !seen(k))
            },
            in_call: self.in_call.is_some(),
        };
        let game = self.arcade.as_ref().map(|a| a.screen(Instant::now()));
        let blanked = self.blanked;
        let mut shown = Vec::new();
        let mut out = Vec::new();
        let mut rainbow_now = false;
        let mut music_now = false;
        // lights that follow the music flash only while something plays (the sound heard is the
        // whole computer's, so a call alone does not make them flash)
        let playing = self.now_playing.as_ref().is_some_and(|(_, _, playing)| *playing) || beat::testing(Instant::now());
        let now_glow = Instant::now();
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
                // the metronome test: the edge follows it whatever it shows otherwise
                ("base", _) if beat::testing(Instant::now()) => Some(leds::Lights { leds: vec![leds::Led::music(profile.as_ref().map(leds::profile_color).unwrap_or([255, 255, 255]))], speed: None }),
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
            // knobs and faders join the edge's music by their switch: every lit LED flashes in its
            // own color (the edge's state is the last module's step: base comes first)
            let joins = match m.kind {
                "knobs" => light_cfg.music_knobs,
                "faders" => light_cfg.music_faders,
                _ => false,
            };
            let mut wanted = wanted;
            if joins
                && self.music_edge
                && let Some(lights) = wanted.as_mut()
            {
                for led in lights.leds.iter_mut().filter(|l| l.rgb != [0, 0, 0] && l.mode == LedMode::Static) {
                    led.pulse = true;
                }
            }
            if m.kind == "base" {
                self.music_edge = wanted.as_ref().is_some_and(|l| l.leds.iter().any(|led| led.pulse));
            }
            // lights that follow the music: their color at the brightness of this moment; the
            // window is told the color as it is, with the effect's name
            let music_view: Vec<bool> = wanted.as_ref().map(|l| l.leds.iter().map(|led| led.pulse).collect()).unwrap_or_default();
            let as_set = wanted.clone();
            if music_view.iter().any(|p| *p) && !blanked && game.is_none() {
                music_now = true;
                // one tuning for every light to the music (Settings > Lights)
                let tuning = beat::Tuning {
                    dim: light_cfg.music_dim.min(100) as f32 / 100.0,
                    from: light_cfg.music_from.min(100) as f32 / 100.0,
                    predict: light_cfg.music_mode != "react",
                    lead_ms: light_cfg.music_lead.min(300) as u64,
                    white_hits: light_cfg.music_white,
                };
                let glow = if playing { beat::glow(now_glow, &tuning) } else { None };
                if m.kind == "base"
                    && let Some(base) = wanted.as_ref().and_then(|l| l.leds.iter().find(|l| l.pulse))
                {
                    let ring = glow.map_or(0.35, |g| g.level);
                    let color = format!("#{:02x}{:02x}{:02x}", base.rgb[0], base.rgb[1], base.rgb[2]);
                    // only with the dial's ring on: a redraw that shows nothing new cost 16 dial
                    // pictures a second (20 % of a core, 2026-10-19)
                    if light_cfg.music_dial && self.music_glow.as_ref().is_none_or(|(c, l)| *c != color || ((l * 10.0).round() - (ring * 10.0).round()).abs() >= 1.0) {
                        self.dial_dirty = true;
                    }
                    self.music_glow = Some((color, ring));
                }
                // in twelfths: the eye sees no steps, and a fading flash need not go out every step
                let glow = glow.map(|g| beat::Glow { level: (g.level * 12.0).round() / 12.0, white: (g.white * 4.0).round() / 4.0 });
                if let (Some(g), Some(lights)) = (glow, wanted.as_mut()) {
                    for led in lights.leds.iter_mut().filter(|l| l.pulse) {
                        led.rgb = led.rgb.map(|c| ((c as f32 + (255.0 - c as f32) * g.white) * g.level).round().min(255.0) as u8);
                    }
                }
            }
            let before = self.leds_sent.get(&m.id).cloned();
            if before.as_ref() == Some(&wanted) {
                continue;
            }
            // only the music's brightness changed and the last LEDs are still going out, or knobs
            // and faders had one 40 ms ago (their firmware takes fewer: the faders dropped off the
            // bus at 50 a second, 2026-10-19): this moment is skipped, a later step sends its own
            let only_brightness = music_view.iter().any(|p| *p) && before.is_some() && self.leds_shown.get(&m.id) == Some(&as_set);
            let too_soon = m.kind != "base" && self.music_sent.get(&m.id).is_some_and(|at| at.elapsed() < Duration::from_millis(40));
            if only_brightness && (led_sender().busy(&m.id) || too_soon) {
                continue;
            }
            if only_brightness {
                self.music_sent.insert(m.id.clone(), Instant::now());
            }
            let Some(module) = self.deck.module(&m.id) else { continue };
            let kind = module.kind();
            let mut reports = Vec::new();
            match &wanted {
                None => reports.extend(report::led_mode(kind, LedTarget::All, LedMode::Rainbow, false)),
                Some(lights) => reports.extend(leds_reports(kind, lights, before.flatten().as_ref())),
            }
            // sent on the LED thread, in order (the reports are the change from what went before)
            led_sender().send((m.id.clone(), module, reports));
            self.perf.leds += 1;
            let view = as_set.as_ref().map(|l| {
                let mut v = leds::view(l);
                for (led, r) in v.iter_mut().zip(&replaced) {
                    if *r {
                        *led = leds::LedView { color: String::new(), effect: "hubRainbow" };
                    }
                }
                for (led, music) in v.iter_mut().zip(&music_view) {
                    if *music {
                        led.effect = "music";
                    }
                }
                v
            });
            out.push((m.id.clone(), wanted.as_ref().map(|l| l.leds.iter().map(|led| format!("#{:02x}{:02x}{:02x}", led.rgb[0], led.rgb[1], led.rgb[2])).collect())));
            self.leds_sent.insert(m.id.clone(), wanted);
            self.leds_shown.insert(m.id.clone(), as_set);
            shown.push((m.id.clone(), view));
        }
        self.rainbow_running = rainbow_now;
        self.music_running = music_now;
        beat::want(music_now);
        // the window hears only real changes (HubDeck's rainbow turning is not one: it follows the clock)
        let all = {
            let mut s = self.lock();
            for (id, colors) in out {
                let entry = s.leds_out.entry(id).or_insert((None, 0));
                *entry = (colors, entry.1 + 1);
            }
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
        if self.press_flash.is_some_and(|(_, at)| now.duration_since(at) >= PRESS_FLASH) {
            self.press_flash = None;
            self.redraw_keys();
        }
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

    /// A key puts Mochi on the dial of the profile on screen, or takes it off and gives the dial
    /// back what it had (kept in `pet.json` by profile; owner, 2026-10-14: a Mochi switch on the
    /// Fun page).
    fn toggle_pet_dial(&mut self, from: &str) {
        const GESTURES: [&str; 3] = ["dial", "dial:double", "dial:hold"];
        let profile = {
            let s = self.lock();
            s.active.clone().and_then(|id| s.store.profiles.get(&id).cloned())
        };
        let Some(mut p) = profile else { return };
        if p.controls.get("dial").is_some_and(|b| b.action == pet::ACTION) {
            // its name and body set on the dial come back with it
            self.pet.needs.mochi = pet::kept_bindings(&p.controls, &GESTURES);
            for g in GESTURES {
                p.controls.remove(g);
            }
            if let Some(serde_json::Value::Object(before)) = self.pet.needs.dials.remove(&p.id) {
                for (g, b) in before {
                    if let Ok(b) = serde_json::from_value(b) {
                        p.controls.insert(g, b);
                    }
                }
            }
        } else {
            let Some(face) = gallery::faces().into_iter().find(|f| f.id == "pet") else { return };
            let kept: serde_json::Map<String, serde_json::Value> = GESTURES.iter().filter_map(|g| p.controls.get(*g).map(|b| (g.to_string(), serde_json::json!(b)))).collect();
            self.pet.needs.dials.insert(p.id.clone(), serde_json::Value::Object(kept));
            for g in GESTURES {
                p.controls.remove(g);
            }
            // Mochi as the user set it on the dial the last time, else the gallery's
            let mochi = self.pet.needs.mochi.clone();
            let kept = |g: &str| mochi.as_ref().and_then(|m| m.get(g)).and_then(|b| serde_json::from_value::<Binding>(b.clone()).ok());
            // the key that brought it: its body picture is Mochi's body on the dial
            let mut binding = kept("dial").unwrap_or_else(|| face.binding.clone());
            if let Some(params) = binding.params.as_object_mut() {
                params.insert(pet::FROM_PARAM.into(), serde_json::json!(from));
            }
            p.controls.insert("dial".into(), binding);
            // it comes onto the dial happy and full (owner's 2a, 2026-10-16), not as it was left
            self.pet.needs.joy = 1.0;
            self.pet.needs.fullness = 1.0;
            self.pet.needs.asleep = false;
            for (g, b) in face.extra {
                let b = kept(g).unwrap_or(b);
                p.controls.insert(g.to_string(), b);
            }
        }
        self.pet.save();
        let saved = self.lock().store.save_profile(p).is_ok();
        if saved {
            let state = self.lock().state();
            publish_state(&self.app, state);
        }
        self.dial_dirty = true;
    }

    /// The dial's picture to the window too, so its drawing shows what the hardware shows: kept
    /// for when the window opens, sent while it is open, at most every [`DIAL_MIRROR_EVERY`].
    fn mirror_dial(&mut self, profile: Option<&str>, module: &str, jpeg: &[u8]) {
        use base64::Engine as _;
        let frame = serde_json::json!({"profile": profile, "module": module, "jpeg": base64::engine::general_purpose::STANDARD.encode(jpeg)});
        *DIAL_SHOWN.lock().unwrap_or_else(|e| e.into_inner()) = Some(frame);
        self.dial_mirror_due = true;
        self.send_mirrors();
    }

    /// The key panel's picture to the window, with where each key's window sits in it, so the
    /// drawing shows what the keys show (live values, covers, clocks); like [`Self::mirror_dial`].
    fn mirror_keys(&mut self, profile: Option<&str>, page: Option<&str>, module: &str, jpeg: &[u8], geometry: &render::PanelGeometry) {
        use base64::Engine as _;
        let rects: Vec<[f32; 4]> = (0..15).map(|i| geometry.key_rect(i)).map(|(x, y, w, h)| [x, y, w, h]).collect();
        let frame = serde_json::json!({"profile": profile, "page": page, "module": module, "jpeg": base64::engine::general_purpose::STANDARD.encode(jpeg), "rects": rects});
        *KEYS_SHOWN.lock().unwrap_or_else(|e| e.into_inner()) = Some(frame);
        self.keys_mirror_due = true;
        self.send_mirrors();
    }

    /// The newest dial and key panel pictures to an open window, each at most every
    /// [`DIAL_MIRROR_EVERY`]. A frame that comes sooner waits and goes on the engine's next turn:
    /// dropping it left the window on the screensaver's last frame once the keys stood still
    /// (owner, 2026-10-15).
    /// Tells the window when the screensaver starts, changes or ends, so it draws it itself.
    fn publish_saver(&mut self) {
        let shown = (self.saver_on && !self.blanked).then(|| {
            let s = self.lock();
            let effect = s.saver_preview.as_ref().filter(|(_, until)| *until > Instant::now()).map_or(s.store.settings.saver.clone(), |(e, _)| e.clone());
            (effect, s.store.settings.lights.color.clone())
        });
        if shown != self.saver_published {
            let value = shown.as_ref().map(|(effect, color)| serde_json::json!({"effect": effect, "color": color}));
            *SAVER_SHOWN.lock().unwrap_or_else(|e| e.into_inner()) = value.clone();
            let _ = self.app.emit("engine://saver", value);
            self.saver_published = shown;
        }
    }

    fn send_mirrors(&mut self) {
        if !self.dial_mirror_due && !self.keys_mirror_due {
            return;
        }
        // a window hidden by its X counts as closed (`is_visible` stayed true for it on the test copy)
        let open = !crate::tray::main_hidden() && self.app.get_webview_window("main").is_some_and(|w| w.is_visible().unwrap_or(false));
        if self.dial_mirror_due && self.dial_mirror_at.elapsed() >= DIAL_MIRROR_EVERY {
            self.dial_mirror_due = false;
            self.dial_mirror_at = Instant::now();
            if open && let Some(frame) = DIAL_SHOWN.lock().unwrap_or_else(|e| e.into_inner()).clone() {
                send_mirror("dial", &frame);
            }
        }
        let keys_every = if self.keys_mirror_slow { KEYS_MIRROR_SLOW } else { DIAL_MIRROR_EVERY };
        if self.keys_mirror_due && self.keys_mirror_at.elapsed() >= keys_every {
            self.keys_mirror_due = false;
            self.keys_mirror_at = Instant::now();
            if open && let Some(frame) = KEYS_SHOWN.lock().unwrap_or_else(|e| e.into_inner()).clone() {
                send_mirror("keys", &frame);
            }
        }
    }

    fn redraw_dial(&mut self) {
        let snapshot = self.deck.snapshot();
        let user = self.app.state::<Placements>().all();
        let profile = self.lock().active_profile().cloned();
        for m in snapshot.modules.iter().filter(|m| m.kind == "dial") {
            let module = self.deck.module(&m.id);
            if module.is_none() && !mirror_only() {
                continue;
            }
            let rotation = self.content_rotation(m.kind, m.slot.as_deref(), snapshot.turn, &user);
            let stored = profile.as_ref().and_then(|p| p.controls.get("dial"));
            // the dial's Now playing names its music app: its song shows and its player is steered
            media::prefer(stored.filter(|b| b.action == actions::NOW_PLAYING).map_or("", |b| actions::param_str(&b.params, "player")));
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
            let mut pet_rest = false;
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
                    let (metric, v) = binding.and_then(|b| self.monitor_value("dial", b)).expect("monitor binding");
                    // temperatures fill the ring up to 100 °C
                    render::dial_svg(&look, Some((v.map(|v| v / 100.0), metric.name())), rotation)
                }
                (None, None) if value.is_none() && binding.is_some_and(|b| b.action == pet::ACTION) => {
                    let mood = self.pet.mood(self.locked, chrono::Timelike::hour(&chrono::Local::now()));
                    pet_rest = self.pet.moment.is_none_or(|(_, at)| at.elapsed() >= Duration::from_millis(2400));
                    // Mochi is one pet: a body picked on the key that brings it to the dial is its
                    // body there too (owner, 2026-10-18: set on the Fun page's Mochi key, the dial
                    // stayed purple)
                    let body = binding
                        .and_then(|b| pet::body_of(&b.params))
                        .or_else(|| profile.as_ref().and_then(|p| pet::body_on(&p.controls, binding.map(|b| actions::param_str(&b.params, pet::FROM_PARAM)))));
                    pet::dial_svg_with(&self.pet, mood, pet::loop_clock(), Instant::now(), rotation, body.as_ref())
                }
                (None, None) if value.is_none() && binding.is_some_and(|b| b.action == actions::POMODORO) => {
                    let b = binding.expect("checked");
                    let fresh = timer::pomodoro_of(&b.params);
                    let pomo = self.pomodoros.get("dial").unwrap_or(&fresh);
                    let polish = self.lock().store.settings.language.starts_with("pl");
                    render::pomodoro_dial_svg(&look, pomo, Instant::now(), pomo.phase(polish), rotation)
                }
                (None, None) if value.is_none() && binding.is_some_and(|b| b.action == actions::CLOCK || b.action == actions::TIMER) => {
                    let b = binding.expect("checked");
                    let face = self.live_face("dial", b, Instant::now()).unwrap_or_default();
                    let look = if face.done { b.look_for(Some(true)) } else { look };
                    render::time_dial_svg(&look, &face.main, face.second.as_deref(), face.ring, rotation)
                }
                (None, None) if value.is_none() && binding.is_some_and(|b| b.action == live::ACTION && actions::param_str(&b.params, "dial") == "scene") => {
                    let b = binding.expect("checked");
                    let reading = live::value_key(&b.params).and_then(|k| self.values.get(&k)).and_then(|r| serde_json::from_str::<live::Reading>(r).ok());
                    let polish = self.lock().store.settings.language.starts_with("pl");
                    let view = self.weather_view.get("dial").copied().unwrap_or_default();
                    render::weather_dial_svg(&look, reading.as_ref().and_then(|r| r.forecast.as_ref()), reading.as_ref().and_then(|r| r.num), view, saver::clock(), polish, rotation)
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
                    let share = self.progress_share();
                    self.progress_drawn = share.map(|v| (v * 1000.0) as u16);
                    render::now_playing_dial_svg(&look, track, share, rotation)
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
                (Some(question), _) => render::question_dial_svg(&question, rotation),
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
            // lights to the music: a ring that flashes with the edge
            let ring = self.music_glow.clone().filter(|_| self.music_running && self.lock().store.settings.lights.music_dial);
            let svg = match ring {
                Some((color, level)) => render::with_music_ring(&svg, &color, level),
                None => svg,
            };
            // screens off: the dial shows black (its backlight is never set to 0, see apply_screens)
            let svg = if self.blanked { render::dial_body_svg(r##"<rect width="240" height="240" fill="#000"/>"##, 0) } else { svg };
            if self.sent.get(&m.id) == Some(&svg) {
                continue;
            }
            let dial_started = Instant::now();
            let jpeg = if pet_rest {
                let key = {
                    use std::hash::{Hash, Hasher};
                    let mut h = std::collections::hash_map::DefaultHasher::new();
                    svg.hash(&mut h);
                    h.finish()
                };
                match self.pet_jpegs.get(&key) {
                    Some(jpeg) => Ok(jpeg.clone()),
                    None => self.renderer.jpeg(&svg).inspect(|jpeg| {
                        // a new mood, name or turn makes another loop: start over rather than grow
                        if self.pet_jpegs.len() >= PET_FRAMES_KEPT {
                            self.pet_jpegs.clear();
                        }
                        self.pet_jpegs.insert(key, jpeg.clone());
                    }),
                }
            } else {
                self.renderer.jpeg(&svg)
            };
            self.perf.dial += 1;
            self.perf.dial_time += dial_started.elapsed();
            match jpeg {
                Ok(jpeg) => {
                    if let Some(module) = &module {
                        picture_sender(Screen::Dial).send(m.id.clone(), module.clone(), report::dial_image(&jpeg, module.output_len));
                    }
                    self.sent.insert(m.id.clone(), svg);
                    self.mirror_dial(profile.as_ref().map(|p| p.id.as_str()), &m.id, &jpeg);
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
    fn a_toggle_follows_the_system_and_else_remembers_its_side() {
        // issue #10: headphones made the default before a restart, the key still said speakers
        assert!(toggle_side(&[true], Some(false)));
        assert!(!toggle_side(&[false], Some(true)));
        assert!(!toggle_side(&[true, false], None));
        assert!(toggle_side(&[], Some(true)));
        assert!(!toggle_side(&[], None));
        let b = Binding {
            action: actions::TOGGLE.into(),
            params: serde_json::json!({
                "first": [{"action": "core.audioDevice", "params": {"kind": "output", "device": "Headphones"}}, {"action": "core.text", "params": {"text": "x"}}],
                "second": [{"action": "core.audioDevice", "params": {"kind": "output", "device": "Speakers"}}]
            }),
            look: Look::default(),
            accel: false,
            look_on: None,
            swap_turns: false,
        };
        let first = toggle_first(&b);
        assert_eq!(first.len(), 2);
        // only the audio device has a state to read; the snippet is left out
        assert_eq!(first.iter().filter_map(Worker::state_key).collect::<Vec<_>>(), vec!["audio:output:headphones".to_string()]);
        assert_ne!(toggle_key(Some("a"), "keys.3"), toggle_key(Some("b"), "keys.3"));
        let dir = std::env::temp_dir().join(format!("hubdeck-toggles-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        save_toggles(&dir, &HashMap::from([(toggle_key(Some("a"), "keys.3"), true)]));
        assert_eq!(load_toggles(&dir).get("a/keys.3"), Some(&true));
        let _ = std::fs::remove_dir_all(&dir);
    }

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
        let red = Led::new([255, 0, 0], LedMode::Breathing);
        let lights = Lights { leds: vec![red; 3], speed: Some(40) };
        let bytes: Vec<Vec<u8>> = leds_reports(ModuleKind::Knobs, &lights, None).iter().map(|r| r.bytes().to_vec()).collect();
        assert_eq!(bytes.len(), 3, "one color for the three knobs, one effect for the module, the speed");
        assert_eq!(bytes[0][..4], [0x02, 0x2D, 2, 0xFF]);
        assert_eq!(bytes[1], vec![0x02, 0x2C, 2, 0xFF, LedMode::Breathing as u8, 0]);
        assert_eq!(bytes[2], vec![0x02, 0x30, 40]);
        // different colors: one each
        let mut colors = lights.clone();
        colors.leds[2].rgb = [0, 0, 255];
        let each: Vec<Vec<u8>> = leds_reports(ModuleKind::Knobs, &colors, Some(&lights)).iter().map(|r| r.bytes().to_vec()).collect();
        assert_eq!(each.len(), 1, "colors differ: only the changed knob's, by its own index");
        assert_eq!(each[0][..4], [0x02, 0x2D, 2, 2]);
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
        // a ready profile the user has not switched on stays out of switching
        store.profiles.get_mut("ps").unwrap().switch_in = Some(false);
        assert_eq!(go(&store, "desk", "desk", "Photoshop.exe"), None, "Switch in with the app is off");
        store.profiles.get_mut("ps").unwrap().switch_in = None;
        // pinned: nothing switches
        store.settings.pinned = true;
        assert_eq!(go(&store, "desk", "desk", "Photoshop.exe"), None);
        store.settings.pinned = false;
        store.settings.auto_switch = false;
        assert_eq!(go(&store, "desk", "desk", "Photoshop.exe"), None, "auto-switch off");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn use_this_puts_a_new_version_into_the_users_deck() {
        let dir = std::env::temp_dir().join(format!("hubdeck-version-{}", std::process::id()));
        let mut store = Store::load(&dir);
        let key = || Binding { action: "core.media".into(), params: serde_json::json!({}), look: Look::default(), accel: false, look_on: None, swap_turns: false };
        let mut mine = Profile::new("My deck · dial and knobs");
        mine.id = "mine".into();
        mine.apps = vec!["Spotify.exe".into()];
        mine.controls.insert("keys.0".into(), key());
        store.save_profile(mine).unwrap();
        // the assistant's version of it, linked by id
        let mut draft = Profile::new("Knobs for Spotify");
        draft.id = "d1".into();
        draft.assistant = true;
        draft.version_of = Some("mine".into());
        draft.controls.insert("knobs.1".into(), key());
        draft.edge = Some("#79ecc6".into());
        store.save_profile(draft.clone()).unwrap();
        store.settings.drafts.push("d1".into());
        assert_eq!(version_target(&store, &draft).as_deref(), Some("mine"));
        // a draft is never the profile another draft replaces; a new deck replaces nothing
        let mut other = Profile::new("Streaming");
        other.id = "d2".into();
        other.assistant = true;
        other.version_of = Some("d1".into());
        assert_eq!(version_target(&store, &other), None);
        // without the link, the same name (with or without " · sketch") says which one
        let mut named = Profile::new("My deck · dial and knobs · sketch");
        named.assistant = true;
        assert_eq!(version_target(&store, &named).as_deref(), Some("mine"));
        named.assistant = false;
        assert_eq!(version_target(&store, &named), None, "only the assistant's drafts by name");
        assert_eq!(put_version(&mut store, "d1", "mine").unwrap(), "My deck · dial and knobs");
        let p = &store.profiles["mine"];
        assert_eq!((p.apps.clone(), p.edge.as_deref()), (vec!["Spotify.exe".to_string()], Some("#79ecc6")), "its apps stay, the draft's lights come");
        assert!(p.controls.contains_key("knobs.1") && !p.controls.contains_key("keys.0"), "the draft's controls replace the old ones");
        assert!(!store.profiles.contains_key("d1") && store.settings.drafts.is_empty(), "the draft is gone");
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
