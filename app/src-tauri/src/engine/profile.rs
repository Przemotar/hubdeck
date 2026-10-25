//! Profiles: what every control does and how keys and the dial look. Bound to control kinds
//! (`keys.4`, `dial`, `faders.2`), not to serial numbers, so a profile works on any deck and can
//! be shared as one file.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const PROFILE_VERSION: u32 = 1;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Look {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Name of a built-in icon (see `icons_list`), or `emoji:` and an emoji drawn in the system's
    /// color emoji font.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Background, `#rrggbb`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bg: Option<String>,
    /// Icon and text, `#rrggbb`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fg: Option<String>,
    /// The icon in its own color, `#rrggbb` (the house look: a soft group color); None = `fg`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_color: Option<String>,
    /// LED of a knob or fader, `#rrggbb`; no color anywhere on a module keeps its own rainbow.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub led: Option<String>,
    /// The LED's brightness follows the value: the fader position, or the volume a knob sets.
    #[serde(default, skip_serializing_if = "is_false")]
    pub led_level: bool,
    /// LED effect of a knob or fader: `static` (default), `breathing`, `strobe`, `blink`, `cycling`,
    /// `random`, `wave`, `water`, `rainbow` (see `leds::EFFECTS`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub led_effect: Option<String>,
    /// Effect speed 0..=100; one speed per module (the last control that sets one).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub led_speed: Option<u8>,
    /// How a key's background is drawn: `flat` (default), `gradient`, `glass` (a highlight like the
    /// hardware caps), `outline` (a colored frame on black). Keys only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    /// The user's own picture (a name from `images::import`); replaces the icon. Animated GIFs move.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// How the picture is drawn: `icon` (centred with the label under it), `fill` (the whole key or
    /// dial, the label on a strip); None = by the picture (logos as icons, photos fill).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fit: Option<String>,
    /// Where a key's label stands, over the icon or picture as on Stream Deck ("Title alignment"):
    /// `top`, `middle`, `bottom`; None = under the icon (the house layout). Issue #10, owner's 1a.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_pos: Option<String>,
    /// A key label's size in key pixels (a key is 72, see `LABEL_SIZES`); None = fitted to the key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_size: Option<u8>,
    /// A key icon's size: `small`, `large`; None = as the label leaves room for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_size: Option<String>,
    /// What this control does and how to set it, shown in the window when it is picked or pressed
    /// (the demo's `demo:<id>` are translated by the window).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Drawn as a clock or timer face: the label's first line big, the line after a newline small.
    /// Set by the engine while drawing, never saved.
    #[serde(skip)]
    pub face: bool,
    /// A thin light frame for a moment: the press arrived (owner's pick 1a, 2026-10-16; users of
    /// other decks press again when nothing shows, `docs/PAIN-POINTS.md` RD2). Never saved.
    #[serde(skip)]
    pub pressed: bool,
    /// A red corner: the last press of this key failed; the window says why. Never saved.
    #[serde(skip)]
    pub failed: bool,
}

fn saver_minutes() -> u32 {
    5
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Binding {
    /// Action id, e.g. `core.launch`.
    pub action: String,
    #[serde(default)]
    pub params: serde_json::Value,
    #[serde(default)]
    pub look: Look,
    /// Knobs, rollers, the dial: turning fast counts each detent two or three times.
    #[serde(default, skip_serializing_if = "is_false")]
    pub accel: bool,
    /// How the key looks while its state is on (`lookOn` in JSON) (microphone or sound muted); None = a red version
    /// of `look` with a matching icon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub look_on: Option<Look>,
    /// A Stream Deck plugin's dial action: a plain turn is sent as a turn while pressed and the
    /// other way round (Spotify's Playback Control seeks on a pressed turn; the owner wanted it plain).
    #[serde(default, skip_serializing_if = "is_false")]
    pub swap_turns: bool,
}

impl Binding {
    /// The look to draw for `state` (None = the action has no state). An action nobody provides
    /// (its plugin is not installed) shows that, keeping its label.
    pub fn look_for(&self, state: Option<bool>) -> Look {
        // no action at all (a picture-only key, an imported action HubDeck cannot do): as set
        if self.action.is_empty() {
            return self.look.clone();
        }
        if super::actions::find(&self.action).is_none() {
            return Look { icon: Some("x".into()), bg: Some("#3a3a40".into()), fg: Some("#b8b8c0".into()), image: None, ..self.look.clone() };
        }
        if state != Some(true) {
            return self.look.clone();
        }
        self.look_on.clone().unwrap_or_else(|| {
            let icon = match (self.action.as_str(), self.look.icon.as_deref()) {
                ("core.mic", _) | (_, Some("mic")) => Some("mic-off".to_string()),
                (_, Some("video")) => Some("video-off".to_string()),
                (_, Some("volume-2")) => Some("volume-x".to_string()),
                (_, other) => other.map(str::to_string),
            };
            // a live scene or a shown source is good news: green; a mute or a recording: red
            let bg = match self.action.as_str() {
                "obs.scene" | "obs.source" | "obs.sceneCollection" | "obs.profile" | "obs.filter" | "obs.studioMode" | "streamlabs.scene" | "streamlabs.source" | "streamlabs.studioMode" => "#1f5a2c",
                "ha.toggle" => "#6b5413",
                "spotify.like" => "#1d6b3a",
                _ => "#7a1f24",
            };
            Look { bg: Some(bg.into()), icon, ..self.look.clone() }
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    #[serde(default = "default_version")]
    pub version: u32,
    pub id: String,
    pub name: String,
    /// Executable names that switch to this profile when in front, e.g. `obs64.exe`, `Photoshop`.
    #[serde(default)]
    pub apps: Vec<String>,
    /// Control id (`keys.0`..`keys.14`, `dial`, `knobs.0`.., `faders.0`.., `rollers.0`..) -> binding.
    /// A hold or double press has its own binding under `<id>:hold` / `<id>:double`; keys on an
    /// extra page live under `<page id>/keys.N` (see [`control_id`]).
    #[serde(default)]
    pub controls: BTreeMap<String, Binding>,
    /// Extra key pages, in order; the main page is not listed. Only keys change with the page.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pages: Vec<Page>,
    /// Color of the base's edge light while this profile is active; None = the base's rainbow.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edge: Option<String>,
    /// Effect of the base edge light (as `Look::led_effect`); None = static.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edge_effect: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edge_speed: Option<u8>,
    /// One picture or a clock across all 15 keys instead of separate key faces (keys still act).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub panel: Option<PanelContent>,
    /// The profile's color for the "profile color" light style; None = one picked from its name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Shown as a tab in the top bar; None = the default ([`Profile::shows_in_bar`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_bar: Option<bool>,
    /// Made by the deck assistant: its shortcuts outside the checked packs get a mark (`check.rs`).
    #[serde(default, skip_serializing_if = "is_false")]
    pub assistant: bool,
    /// A draft that is a new version of one of the user's own profiles (this id): "Use this"
    /// puts its controls into that profile instead of keeping a second one (owner's pick 1a).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_of: Option<String>,
    /// Switches in when its app comes to the front: None = yes (as always); Some(false) = a
    /// ready profile the user has not switched on ("Ready for your apps", owner's pick 1a).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub switch_in: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PanelContent {
    Picture {
        image: String,
    },
    Clock,
    /// A live graph of the last two minutes of one system metric.
    Graph {
        metric: super::monitor::Metric,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    pub id: String,
    pub name: String,
}

/// The id under which a control's binding is stored on `page` (None = main page). Only keys
/// belong to pages; knobs, faders, rollers and the dial are the same on every page.
pub fn control_id(page: Option<&str>, control: &str) -> String {
    match page {
        Some(p) if control.starts_with("keys.") => format!("{p}/{control}"),
        _ => control.to_string(),
    }
}

fn default_version() -> u32 {
    PROFILE_VERSION
}

impl Profile {
    pub fn new(name: &str) -> Self {
        Self {
            version: PROFILE_VERSION,
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            apps: Vec::new(),
            controls: BTreeMap::new(),
            pages: Vec::new(),
            edge: None,
            edge_effect: None,
            edge_speed: None,
            color: None,
            panel: None,
            in_bar: None,
            assistant: false,
            version_of: None,
            switch_in: None,
        }
    }

    /// Shown as a tab in the top bar: as the user set it, else profiles without apps are and app
    /// profiles are not (they show while they are active).
    pub fn shows_in_bar(&self) -> bool {
        self.in_bar.unwrap_or(self.apps.is_empty())
    }

    /// Does `exe` (file name of the app in front) belong to this profile?
    #[cfg(test)]
    pub fn matches_app(&self, exe: &str) -> bool {
        self.app_match(exe, "").is_some()
    }

    /// How well the app in front matches: 2 = an entry with a window title (`chrome.exe: YouTube`,
    /// `*: Zoom Meeting`) whose text is in the title, 1 = the app alone, None = no match.
    pub fn app_match(&self, exe: &str, title: &str) -> Option<u8> {
        let stem = app_stem(exe);
        let stem = stem.as_str();
        let title = title.to_lowercase();
        self.apps
            .iter()
            .filter_map(|entry| {
                let entry = entry.trim().to_lowercase();
                let (app, wanted) = match entry.split_once(':') {
                    Some((a, t)) => (a.trim().to_string(), Some(t.trim().to_string())),
                    None => (entry.clone(), None),
                };
                let a_stem = if app == "*" { "*".to_string() } else { app_stem(&app) };
                let a_stem = a_stem.as_str();
                let app_ok = a_stem == "*" || (!a_stem.is_empty() && a_stem == stem);
                match (app_ok, wanted) {
                    (true, Some(t)) if !t.is_empty() => title.contains(&t).then_some(2),
                    (true, _) if a_stem != "*" => Some(1),
                    _ => None,
                }
            })
            .max()
    }
}

/// One name for an app on Windows and macOS: lowercase, without `.exe` / `.app` and a year or
/// version at the end, and the names that differ between the systems mapped to one (a profile
/// made on Windows for `chrome.exe` takes over for `Google Chrome.app` on a Mac).
pub fn app_stem(name: &str) -> String {
    let lower = name.trim().to_lowercase();
    let file = lower.rsplit(['\\', '/']).next().unwrap_or(&lower);
    let mut stem = file.strip_suffix(".exe").or_else(|| file.strip_suffix(".app")).unwrap_or(file).trim().to_string();
    // "Adobe Photoshop 2025", "Unity 6000.0.1"
    while let Some((head, tail)) = stem.rsplit_once(' ') {
        if !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit() || c == '.') {
            stem = head.trim().to_string();
        } else {
            break;
        }
    }
    const SAME: &[(&str, &str)] = &[
        ("google chrome", "chrome"),
        ("visual studio code", "code"),
        ("microsoft edge", "msedge"),
        ("obs", "obs64"),
        ("zoom.us", "zoom"),
        ("microsoft teams", "ms-teams"),
        ("davinci resolve", "resolve"),
        ("adobe photoshop", "photoshop"),
        ("adobe illustrator", "illustrator"),
        ("adobe premiere pro", "adobe premiere pro"),
        ("adobe lightroom classic", "lightroom"),
        ("adobe after effects", "afterfx"),
        ("blender", "blender"),
        ("microsoft word", "winword"),
        ("microsoft excel", "excel"),
        ("microsoft powerpoint", "powerpnt"),
        ("microsoft outlook", "outlook"),
        ("firefox", "firefox"),
        ("discord", "discord"),
        ("slack", "slack"),
        ("spotify", "spotify"),
        ("notion", "notion"),
        ("figma", "figma"),
        ("terminal", "windowsterminal"),
        ("iterm2", "windowsterminal"),
        ("finder", "explorer"),
    ];
    SAME.iter().find(|(mac, _)| *mac == stem).map_or(stem, |(_, win)| win.to_string())
}

/// One reaction of the lights: on or off, and its color.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Reaction {
    pub on: bool,
    pub color: String,
}

fn reaction(color: &str) -> Reaction {
    Reaction { on: true, color: color.into() }
}

/// What the lights react to (all on by default).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Reactions {
    /// The microphone is muted: its knob (or fader) turns this color, else the base edge.
    pub mic: Reaction,
    /// OBS or Streamlabs records or streams: the edge breathes.
    pub live: Reaction,
    /// A Claude Code session needs the user: the edge pulses.
    pub agents: Reaction,
    /// A timer ran out: the edge flashes.
    pub timer: Reaction,
    /// A call is on (an app for calls records the microphone): the edge shows this color, the
    /// muted color while the microphone is muted.
    #[serde(default = "call_reaction")]
    pub call: Reaction,
    /// Volume knobs and faders show their level as brightness.
    pub levels: bool,
}

fn call_reaction() -> Reaction {
    reaction("#22c55e")
}

impl Default for Reactions {
    fn default() -> Self {
        Self { mic: reaction("#ff2020"), live: reaction("#ff2020"), agents: reaction("#ff8a00"), timer: reaction("#ffffff"), call: call_reaction(), levels: true }
    }
}

/// The deck's lights in one place: a style for every LED (`rainbow`, `color`, `profile` = each
/// profile's color, `off`), one brightness, and reactions. Colors and effects set on a single
/// control ("Advanced lights") win over the style.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct LightSettings {
    pub style: String,
    pub color: String,
    /// 0..=100
    pub brightness: u8,
    pub react: Reactions,
    /// The rainbow is drawn by HubDeck (one color for every LED, turning with the clock, so the
    /// window shows exactly what the deck does) instead of each module's own (owner, 2026-10-12).
    pub hub_rainbow: bool,
    /// Lights to the music, all of them: how dark between beats and from how loud a part they
    /// light up, 0..=100 each (`beat::PRESETS`).
    pub music_dim: u8,
    pub music_from: u8,
    /// `predict` (flashes on the beat the tempo predicts) or `react` (only on what was heard, the
    /// first way, kept to compare; owner, 2026-10-19).
    pub music_mode: String,
    /// How early a predicted beat flashes, 0..=300 ms (set with the metronome test).
    pub music_lead: u16,
    /// Snares and other hits flash whiter than the kick.
    pub music_white: bool,
    /// The dial shows a ring that flashes with the edge (off unless asked for).
    pub music_dial: bool,
    /// The knobs' and the faders' LEDs flash too, in their own colors, whenever the edge follows
    /// the music (owner, 2026-10-19: "z defaultu niech bedzie tylko swiecenie podstawy ale zeby
    /// mozna bylo dolaczyc jeszcze suwaki i knoby").
    pub music_knobs: bool,
    pub music_faders: bool,
}

impl Default for LightSettings {
    fn default() -> Self {
        // rainbow is what the modules do by themselves: nothing changes for existing setups
        Self {
            style: "rainbow".into(),
            color: "#8b7cf6".into(),
            brightness: 100,
            react: Reactions::default(),
            hub_rainbow: true,
            music_dim: super::beat::PRESETS[1].1,
            music_from: super::beat::PRESETS[1].2,
            music_mode: "predict".into(),
            music_lead: 20,
            music_white: true,
            music_dial: false,
            music_knobs: false,
            music_faders: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub active: Option<String>,
    /// The home profile at start: the one HubDeck begins with (the user's pick replaces it until
    /// the next start). Stored as `fallback` for older settings files.
    pub fallback: Option<String>,
    /// "Pin this profile": no automatic switching at all until unpinned.
    #[serde(default)]
    pub pinned: bool,
    #[serde(default = "yes")]
    pub auto_switch: bool,
    /// Key panel geometry from the calibration mode.
    #[serde(default)]
    pub panel: super::render::PanelGeometry,
    /// Clockwise degrees the base is turned, chosen by the user; None = follow the sensor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_turn: Option<i32>,
    /// Autostart was switched on once at the first start of an installed build; never again, so
    /// turning it off sticks.
    #[serde(default)]
    pub autostart_offered: bool,
    /// The first-run welcome was dismissed.
    #[serde(default)]
    pub onboarded: bool,
    /// obs-websocket address; None = `ws://127.0.0.1:4455`. The password is in the credential store.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub obs_url: Option<String>,
    /// Streamlabs Desktop remote-control address; None = `ws://127.0.0.1:59650/api/websocket`.
    /// The API token is in the credential store.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub streamlabs_url: Option<String>,
    /// Home Assistant address, e.g. `http://homeassistant.local:8123`; the token is in the credential store.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ha_url: Option<String>,
    /// Backlight of the key panel and the dial, 0..=100.
    #[serde(default = "full")]
    pub screen_brightness: u8,
    /// Turn screens and LEDs off after this many minutes without input; 0 = never.
    #[serde(default)]
    pub blank_after_minutes: u32,
    /// The screensaver's effect (`rain`, `stars`, `waves`; empty = none) and after how many idle
    /// minutes it starts (`saver.rs`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub saver: String,
    #[serde(default = "saver_minutes")]
    pub saver_after_minutes: u32,
    /// Turn screens and LEDs off while Windows is locked.
    #[serde(default = "yes")]
    pub blank_when_locked: bool,
    /// Where the profiles live, e.g. a OneDrive or Dropbox folder shared by two PCs; None =
    /// `<app data>/profiles`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profiles_dir: Option<String>,
    /// Look for a new HubDeck version at start (never installs without the user).
    #[serde(default = "yes")]
    pub check_updates: bool,
    /// The deck's lights: one style, one brightness, what they react to.
    #[serde(default)]
    pub lights: LightSettings,
    /// Client ID of the user's own Spotify app; the sign-in is in the credential store.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spotify_client_id: Option<String>,
    /// Profiles make themselves for apps with a preset and for games (`auto.rs`; owner, 2026-10-12).
    #[serde(default = "yes")]
    pub auto_profiles: bool,
    /// Programs that already got a profile by themselves (lowercase exe names): one the user deleted
    /// is not made again.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub offered: Vec<String>,
    /// Profiles made by themselves that the user has not looked at yet; the window lists them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub auto_made: Vec<String>,
    /// Drafts the deck assistant proposed: not in the bar, never switched to by an app, until
    /// the user picks one ("Use this") or throws it away.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub drafts: Vec<String>,
    /// The draft on the deck for a try, kept so a restart in the middle of a try can offer it back
    /// (owner, 2026-10-22: a restart took the assistant's tried changes off his deck, "wróciło mi
    /// do poprzedniej wersji").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trying: Option<String>,
    /// Profiles HubDeck made at once for the installed apps it knows ("Ready for your apps",
    /// owner's pick 1a, 2026-10-14); they switch in with their app only once switched on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ready: Vec<String>,
    /// The deck assistant's model, as a Claude Code alias (`opus`, `sonnet`...); None = what
    /// Claude Code uses by default (an alias follows its newest model after updates).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assistant_model: Option<String>,
    /// The deck assistant's effort (`low` .. `max`); None = Claude Code's own setting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assistant_effort: Option<String>,
    /// The deck assistant may read files, search, use the web and run commands, each command and
    /// web fetch asked in the window first (owner, 2026-10-12: "dajmy mu więcej mocy"); off by default.
    #[serde(default, skip_serializing_if = "is_false")]
    pub assistant_power: bool,
    /// The user's own apps HubDeck listens to (Settings > Connections > Your apps).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub apps: Vec<super::states::AppSource>,
    /// Apps that connected themselves (`POST /api/apps/announce`) and the user has not looked at.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub apps_joined: Vec<String>,
    /// Status addresses of apps the user disconnected: their announcements are refused until the
    /// user adds them again by hand.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub apps_refused: Vec<String>,
    /// Count how often each control is pressed (`presses.json`, on this computer), so the
    /// assistant can arrange keys by what is used; off until the user switches it on.
    #[serde(default, skip_serializing_if = "is_false")]
    pub count_presses: bool,
    /// How app icons and logos show on keys whose picture is on Auto: `icon` (like an icon, the
    /// name under it), `fill` (the whole key, no name), `fillLabel` (the whole key with the name).
    #[serde(default = "icon_mode")]
    pub app_icons: String,
    /// The font for text on the deck, a family on this computer (None = HubDeck's own).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deck_font: Option<String>,
    /// During a call the deck shows the profile linked to the call app and stays on it.
    #[serde(default = "yes")]
    pub call_switch: bool,
    /// The window's language (`en`, `pl`), for the words the deck itself shows.
    #[serde(default = "english")]
    pub language: String,
    /// Less movement (the tour audit's photosensitivity point): the tour's highlights hold still,
    /// pictures on the deck do not animate, the screensaver changes once a minute, the window's
    /// animations stop. None = never set (the window takes the system's setting at its first start).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reduced_motion: Option<bool>,
}

fn english() -> String {
    "en".into()
}

fn icon_mode() -> String {
    "fillLabel".into()
}

/// The folder the profiles are read from and written to (settings' choice or the default).
pub fn profiles_dir(data_dir: &Path, settings: &Settings) -> PathBuf {
    settings.profiles_dir.as_deref().map(str::trim).filter(|d| !d.is_empty()).map(PathBuf::from).unwrap_or_else(|| data_dir.join("profiles"))
}

/// The folder the profiles are in, read from `settings.json` (for backups made before the store).
pub fn profiles_dir_of(data_dir: &Path) -> PathBuf {
    let settings: Settings = std::fs::read(data_dir.join("settings.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    profiles_dir(data_dir, &settings)
}

/// A cheap fingerprint of a profiles folder: changes when a file is added, removed or rewritten.
pub fn folder_stamp(dir: &Path) -> (usize, Option<std::time::SystemTime>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return (0, None) };
    let mut n = 0;
    let mut newest = None;
    for e in entries.flatten().filter(|e| e.path().extension().is_some_and(|x| x == "json")) {
        n += 1;
        let t = e.metadata().and_then(|m| m.modified()).ok();
        if t > newest {
            newest = t;
        }
    }
    (n, newest)
}

fn full() -> u8 {
    100
}

fn yes() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            active: None,
            fallback: None,
            pinned: false,
            auto_switch: true,
            panel: Default::default(),
            base_turn: None,
            autostart_offered: false,
            onboarded: false,
            obs_url: None,
            ha_url: None,
            streamlabs_url: None,
            screen_brightness: 100,
            blank_after_minutes: 0,
            saver: String::new(),
            saver_after_minutes: saver_minutes(),
            blank_when_locked: true,
            profiles_dir: None,
            spotify_client_id: None,
            check_updates: true,
            lights: LightSettings::default(),
            auto_profiles: true,
            offered: Vec::new(),
            auto_made: Vec::new(),
            apps_joined: Vec::new(),
            apps_refused: Vec::new(),
            drafts: Vec::new(),
            trying: None,
            ready: Vec::new(),
            assistant_model: None,
            assistant_effort: None,
            assistant_power: false,
            apps: Vec::new(),
            count_presses: false,
            app_icons: icon_mode(),
            deck_font: None,
            language: english(),
            call_switch: true,
            reduced_motion: None,
        }
    }
}

/// Write `bytes` next to `path` and rename over it: a crash never leaves half a file.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(tmp, path)
}

/// Profiles in `<data>/profiles/<id>.json` plus `<data>/settings.json`.
pub struct Store {
    dir: PathBuf,
    settings_path: PathBuf,
    pub profiles: BTreeMap<String, Profile>,
    pub settings: Settings,
}

impl Store {
    pub fn load(data_dir: &Path) -> Self {
        let settings_path = data_dir.join("settings.json");
        let settings: Settings = std::fs::read(&settings_path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        let dir = profiles_dir(data_dir, &settings);
        let profiles = Self::read_profiles(&dir);
        Self { dir, settings_path, profiles, settings }
    }

    /// The folder profiles are in.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Read the profiles again (another PC changed the shared folder).
    pub fn reload_profiles(&mut self) {
        self.profiles = Self::read_profiles(&self.dir);
    }

    fn read_profiles(dir: &Path) -> BTreeMap<String, Profile> {
        let mut profiles = BTreeMap::new();
        if let Ok(entries) = std::fs::read_dir(dir) {
            for e in entries.flatten() {
                let path = e.path();
                if path.extension().and_then(|x| x.to_str()) != Some("json") {
                    continue;
                }
                match std::fs::read(&path).map_err(|e| e.to_string()).and_then(|b| serde_json::from_slice::<Profile>(&b).map_err(|e| e.to_string())) {
                    Ok(p) => {
                        profiles.insert(p.id.clone(), p);
                    }
                    // a broken file is skipped, never fatal; it stays on disk for the user to fix
                    Err(err) => log::warn!("skipping profile {}: {err}", path.display()),
                }
            }
        }
        profiles
    }

    pub fn save_profile(&mut self, profile: Profile) -> std::io::Result<()> {
        let json = serde_json::to_vec_pretty(&profile)?;
        write_atomic(&self.dir.join(format!("{}.json", safe_file_name(&profile.id))), &json)?;
        self.profiles.insert(profile.id.clone(), profile);
        Ok(())
    }

    pub fn delete_profile(&mut self, id: &str) -> std::io::Result<()> {
        self.profiles.remove(id);
        let path = self.dir.join(format!("{}.json", safe_file_name(id)));
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        if self.settings.active.as_deref() == Some(id) {
            self.settings.active = None;
        }
        self.settings.drafts.retain(|d| d != id);
        self.settings.ready.retain(|d| d != id);
        // home moves to the next profile and stays there (owner, 2026-10-23: the deleted home
        // left a draft as home in the top bar; "home przeskakuje automatycznie na kolejny profil")
        if self.settings.fallback.as_deref() == Some(id) {
            self.settings.fallback = self.next_home();
        }
        self.save_settings()
    }

    pub fn save_settings(&self) -> std::io::Result<()> {
        write_atomic(&self.settings_path, &serde_json::to_vec_pretty(&self.settings)?)
    }

    /// Fallback profile id, repaired if it points nowhere.
    pub fn fallback_id(&self) -> Option<String> {
        self.settings.fallback.clone().filter(|id| self.profiles.contains_key(id)).or_else(|| self.next_home())
    }

    /// The profile that becomes home when none is set: the first of the user's own profiles as
    /// the profile list shows them (no app, not a draft of the assistant, not a ready-made one),
    /// then any that is not a draft, then any.
    fn next_home(&self) -> Option<String> {
        let draft = |p: &&Profile| self.settings.drafts.contains(&p.id);
        let ready = |p: &&Profile| self.settings.ready.contains(&p.id);
        let all = || self.profiles.values();
        all()
            .find(|p| p.apps.is_empty() && !draft(p) && !ready(p))
            .or_else(|| all().find(|p| p.apps.is_empty() && !draft(p)))
            .or_else(|| all().find(|p| !draft(p)))
            .or_else(|| all().next())
            .map(|p| p.id.clone())
    }
}

/// Profile ids are UUIDs we generate, but imported files could carry anything.
fn safe_file_name(id: &str) -> String {
    id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn app_names_match_across_systems() {
        assert_eq!(super::app_stem("Google Chrome.app"), super::app_stem("chrome.exe"));
        assert_eq!(super::app_stem("Visual Studio Code.app"), "code");
        assert_eq!(super::app_stem("Adobe Photoshop 2025.app"), "photoshop");
        assert_eq!(super::app_stem("Spotify.app"), super::app_stem("Spotify.exe"));
        assert_eq!(super::app_stem(r"C:\\Tools\\obs64.exe"), "obs64");
        assert_eq!(super::app_stem("OBS.app"), "obs64");
        assert_eq!(super::app_stem("Unity 6000.0.1"), "unity");
    }

    use super::*;

    #[test]
    fn profiles_with_apps_stay_out_of_the_bar_unless_pinned() {
        let mut p = Profile::new("Desk");
        assert!(p.shows_in_bar());
        p.apps.push("obs64.exe".into());
        assert!(!p.shows_in_bar());
        p.in_bar = Some(true);
        assert!(p.shows_in_bar());
        let old: Profile = serde_json::from_str(r#"{"id":"x","name":"Old","apps":["chrome.exe"]}"#).unwrap();
        assert!(old.in_bar.is_none() && !old.shows_in_bar());
        assert!(!serde_json::to_string(&Profile::new("n")).unwrap().contains("inBar"));
    }

    #[test]
    fn app_matching_ignores_case_and_exe_suffix() {
        let mut p = Profile::new("OBS");
        p.apps = vec!["obs64.exe".into(), "Photoshop".into()];
        assert!(p.matches_app("OBS64.EXE"));
        assert!(p.matches_app("obs64"));
        assert!(p.matches_app("Photoshop.exe"));
        assert!(!p.matches_app("obs32.exe"));
        let mut yt = Profile::new("YouTube");
        yt.apps = vec!["chrome.exe: YouTube".into(), "*: Zoom Meeting".into()];
        assert_eq!(yt.app_match("chrome.exe", "Cats - YouTube - Google Chrome"), Some(2));
        assert_eq!(yt.app_match("chrome.exe", "Gmail"), None, "a title rule needs the title");
        assert_eq!(yt.app_match("Zoom.exe", "Zoom Meeting"), Some(2));
        assert_eq!(yt.app_match("notepad.exe", ""), None, "* alone matches nothing");
    }

    #[test]
    fn stateful_look_turns_red_with_a_matching_icon() {
        let b = Binding {
            action: "core.mic".into(),
            params: serde_json::json!({}),
            look: Look { label: Some("Mic".into()), icon: Some("mic".into()), ..Look::default() },
            accel: false,
            look_on: None,
            swap_turns: false,
        };
        assert_eq!(b.look_for(Some(false)), b.look);
        let on = b.look_for(Some(true));
        assert_eq!(on.icon.as_deref(), Some("mic-off"));
        assert_eq!(on.label.as_deref(), Some("Mic"));
        assert_ne!(on.bg, b.look.bg);
        let custom = Binding { look_on: Some(Look { label: Some("MUTED".into()), ..Look::default() }), ..b };
        assert_eq!(custom.look_for(Some(true)).label.as_deref(), Some("MUTED"));
    }

    #[test]
    fn store_round_trip_skips_broken_files() {
        let dir = std::env::temp_dir().join(format!("mh-store-{}", std::process::id()));
        let mut store = Store::load(&dir);
        let mut p = Profile::new("Desk");
        p.controls.insert(
            "keys.0".into(),
            Binding { action: "core.media".into(), params: serde_json::json!({"command": "playPause"}), look: Look::default(), accel: false, look_on: None, swap_turns: false },
        );
        store.save_profile(p.clone()).unwrap();
        std::fs::write(dir.join("profiles").join("broken.json"), b"{").unwrap();
        let again = Store::load(&dir);
        assert_eq!(again.profiles.len(), 1);
        assert_eq!(again.profiles[&p.id], p);
        assert_eq!(safe_file_name("../evil"), "___evil");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn deleting_home_moves_it_to_the_next_own_profile() {
        let dir = std::env::temp_dir().join(format!("mh-home-{}", std::process::id()));
        let mut store = Store::load(&dir);
        for (id, apps) in [("a-draft", vec![]), ("b-app", vec!["obs64.exe".to_string()]), ("c-home", vec![]), ("d-ready", vec![]), ("e-own", vec![])] {
            let mut p = Profile::new(id);
            p.id = id.into();
            p.apps = apps;
            store.save_profile(p).unwrap();
        }
        store.settings.drafts.push("a-draft".into());
        store.settings.ready.push("d-ready".into());
        store.settings.fallback = Some("c-home".into());
        store.delete_profile("c-home").unwrap();
        assert_eq!(store.settings.fallback.as_deref(), Some("e-own"), "not the draft, the app's or the ready-made profile");
        assert_eq!(Store::load(&dir).settings.fallback.as_deref(), Some("e-own"), "saved");
        store.delete_profile("e-own").unwrap();
        assert_eq!(store.fallback_id().as_deref(), Some("d-ready"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
