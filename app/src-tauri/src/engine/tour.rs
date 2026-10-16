//! The tour (owner, 2026-10-13: "show, then ask, then build"; `docs/ASSISTANT-TOUR.md`, after an
//! audit): HubDeck makes a home deck for this person at once (their modules, their system, what
//! they do most at the computer, their apps and games), shows it on the deck as a draft and walks
//! them through it part by part; each part can be kept or left out and has one or two looks. No
//! AI is needed. Nothing of theirs changes: the deck, the microphone and the volume come back on
//! Exit, and at the next start after a crash. While it runs, actions that would go live, record or
//! end something only say so (`tour_dry_run`).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::{AppHandle, Manager};

use super::Engine;
use super::profile::{Page, Profile};
use super::volume::{self, Target};

/// What the person does most at the computer (one tap; HubDeck suggests one from what it sees).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Focus {
    Games,
    Streaming,
    Calls,
    Music,
    #[default]
    Mixed,
}

/// What the tour found on this computer and deck.
#[derive(Debug, Clone, Default)]
pub struct Ctx {
    pub kinds: Vec<String>,
    pub mac: bool,
    pub polish: bool,
    pub focus: Focus,
    /// The apps used most: (program, name, path).
    pub apps: Vec<(String, String, Option<String>)>,
    /// Installed games: (name, what opens it).
    pub games: Vec<(String, String)>,
    /// The call app: `discord`, `teams` or `zoom`.
    pub call: Option<&'static str>,
    /// Spotify or Apple Music is here.
    pub music: Option<String>,
    pub obs: bool,
    pub browser: Option<String>,
}

impl Ctx {
    fn has(&self, kind: &str) -> bool {
        self.kinds.iter().any(|k| k == kind)
    }
    fn tr(&self, en: &str, pl: &str) -> String {
        if self.polish { pl.to_string() } else { en.to_string() }
    }
    /// "faders", "dialKnobs", "keys" or "none": which of the two common kits this is.
    pub fn kit(&self) -> &'static str {
        if self.has("faders") {
            "faders"
        } else if self.has("knobs") || self.has("dial") {
            "dialKnobs"
        } else if self.has("keys") {
            "keys"
        } else {
            "none"
        }
    }
}

/// One part of the deck the tour walks through.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Stop {
    pub id: &'static str,
    pub keep: bool,
    /// The look picked (0 = the first) and how many there are.
    pub look: usize,
    pub looks: usize,
    /// The controls this part puts on the deck (`keys.5`, `dial`...).
    pub controls: Vec<String>,
    /// The control the "Try it" task is about.
    pub try_control: Option<String>,
    /// "One control, many jobs": the gestures to try, in order (`keys.5:hold`, `knobs.2:double`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tasks: Vec<String>,
    /// "Your dial, many faces": the dial looks shown, by the gallery's ids; `look` picks one.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub faces: Vec<&'static str>,
}

/// The dial looks the tour offers (owner's pick, 2026-10-14: the dial is what stands out).
const DIAL_FACES: [&str; 8] = ["nowPlaying", "pet", "clock", "pomodoro", "weather", "timer", "cpu", "volume"];

/// The tour as the window sees it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub draft: String,
    pub focus: Focus,
    pub kit: &'static str,
    pub mac: bool,
    pub stops: Vec<Stop>,
    pub call: Option<&'static str>,
    pub music: Option<String>,
    pub games: usize,
    pub rollers: bool,
}

/// Before a tour: what HubDeck suggests and why (the window pre-selects it).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    pub focus: Focus,
    pub games: usize,
    pub obs: bool,
    pub call: Option<&'static str>,
    pub music: Option<String>,
    pub kit: &'static str,
    pub mac: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Choices {
    keep: HashMap<String, bool>,
    look: HashMap<String, usize>,
}

/// What was on before the tour, put back after it (also from `tour.json` after a crash).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Snapshot {
    draft: String,
    mic_muted: Option<bool>,
    volume: Option<f32>,
    muted: Option<bool>,
}

struct Tour {
    ctx: Ctx,
    choices: Choices,
    snapshot: Snapshot,
}

static TOUR: Mutex<Option<Tour>> = Mutex::new(None);

const LOOP: &[u8] = include_bytes!("../../resources/tour/loop.wav");

fn snapshot_file(engine: &Engine) -> PathBuf {
    engine.data_dir().join("tour.json")
}

/// Run audio calls on a thread of their own (Windows: the audio API needs its own setup there).
fn on_audio_thread<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    std::thread::spawn(move || {
        volume::init_thread();
        f()
    })
    .join()
    .ok()
}

fn take_snapshot(draft: &str) -> Snapshot {
    let draft = draft.to_string();
    on_audio_thread(move || Snapshot { draft, mic_muted: volume::is_muted(&Target::Mic).ok(), volume: volume::get(&Target::System).ok(), muted: volume::is_muted(&Target::System).ok() })
        .unwrap_or_default()
}

fn restore_audio(s: &Snapshot) {
    let s = s.clone();
    on_audio_thread(move || {
        if let Some(m) = s.mic_muted {
            let _ = volume::set_mute(&Target::Mic, m);
        }
        if let Some(v) = s.volume {
            let _ = volume::set(&Target::System, v);
        }
        if let Some(m) = s.muted {
            let _ = volume::set_mute(&Target::System, m);
        }
    });
    super::sound::send(super::sound::Cmd::StopAll);
}

// ---------- what is on this computer ----------

fn exe_like(name: &str, any: &[&str]) -> bool {
    let n = name.to_lowercase();
    any.iter().any(|a| n.contains(a))
}

/// Look at this computer and deck (nothing leaves it).
pub fn context(app: &AppHandle, focus: Option<Focus>, polish: bool) -> Ctx {
    let kinds = super::api::module_kinds(app).attached;
    let apps = super::assistant::top_apps(&[], 12);
    let installed = super::installed::list(false);
    let games: Vec<(String, String)> = installed.games.iter().map(|g| (g.name.clone(), g.target.clone())).collect();
    let names: Vec<String> = apps.iter().map(|(exe, name, _)| format!("{exe} {name}")).chain(installed.apps.iter().map(|a| format!("{} {}", a.exe, a.name))).collect();
    let find = |any: &[&str]| names.iter().position(|n| exe_like(n, any));
    // the call app used most (apps come first in `names`, in the order of time in front)
    let call = [("discord", &["discord"][..]), ("teams", &["teams"][..]), ("zoom", &["zoom"][..])].iter().filter_map(|(id, any)| find(any).map(|at| (at, *id))).min().map(|(_, id)| id);
    let music = find(&["spotify"]).map(|_| "Spotify".to_string()).or_else(|| (cfg!(target_os = "macos") && find(&["music.app", " music"]).is_some()).then(|| "Music".to_string()));
    let obs = find(&["obs64", "obs studio", "obs.app", "streamlabs"]).is_some();
    let browser = apps.iter().find(|(exe, name, _)| exe_like(&format!("{exe} {name}"), &["chrome", "firefox", "msedge", "brave", "opera", "safari", "arc"])).map(|(exe, _, _)| exe.clone());
    let mut ctx = Ctx { kinds, mac: cfg!(target_os = "macos"), polish, focus: Focus::Mixed, apps, games, call, music, obs, browser };
    ctx.focus = focus.unwrap_or_else(|| suggest(&ctx));
    ctx
}

/// What HubDeck guesses the deck is mostly for, from what it sees (the window shows it as the
/// answer already picked, so a person who does not know yet just goes on; owner, 2026-10-13).
pub fn suggest(ctx: &Ctx) -> Focus {
    let used = |any: &[&str]| ctx.apps.iter().take(6).any(|(exe, name, _)| exe_like(&format!("{exe} {name}"), any));
    if ctx.obs && used(&["obs", "streamlabs"]) {
        Focus::Streaming
    } else if ctx.games.len() >= 3 && used(&["steam", "epic", "battle.net", "riot", "game"]) {
        Focus::Games
    } else if ctx.call.is_some() && used(&["discord", "teams", "zoom"]) {
        Focus::Calls
    } else if ctx.music.is_some() && used(&["spotify", "music"]) {
        Focus::Music
    } else if ctx.games.len() >= 3 {
        Focus::Games
    } else {
        Focus::Mixed
    }
}

// ---------- building the deck ----------

fn binding(action: &str, params: Value, label: &str, icon: &str) -> Value {
    json!({"action": action, "params": params, "look": {"label": label, "icon": icon}})
}

fn hotkey(ctx: &Ctx, win: &str, mac: &str, label: &str, icon: &str) -> Value {
    binding("core.hotkey", json!({"keys": if ctx.mac { mac } else { win }}), label, icon)
}

fn volume_of(target: &str, app: &str, label: &str, icon: &str) -> Value {
    binding("core.volume", json!({"target": target, "app": app, "step": 2}), label, icon)
}

/// App colors for "each app its color" (the knob and fader LEDs).
fn app_color(label: &str) -> &'static str {
    match label.to_lowercase().as_str() {
        l if l.contains("spotify") || l.contains("music") || l.contains("muzyk") => "#1db954",
        l if l.contains("discord") => "#5865f2",
        l if l.contains("teams") => "#6264a7",
        l if l.contains("zoom") => "#2d8cff",
        l if l.contains("game") || l.contains("gra") => "#f5a524",
        l if l.contains("system") => "#ededf2",
        _ => "#8b7cf6",
    }
}

/// Which parts this deck has, in the order of the tour.
fn stop_ids(ctx: &Ctx) -> Vec<&'static str> {
    let mut ids = vec!["volume", "mute", "music", "apps"];
    ids.push(match ctx.focus {
        Focus::Games if !ctx.games.is_empty() => "games",
        Focus::Streaming if ctx.obs => "streaming",
        Focus::Calls if ctx.apps.len() > 5 => "work",
        Focus::Music => "player",
        _ => "extras",
    });
    // put the part they said they do most right after the volume
    if let Some(last) = ids.pop() {
        ids.insert(1, last);
    }
    // press, hold and double press on a key, a knob and the dial; then the dial's other faces
    ids.push("gestures");
    if ctx.has("dial") {
        ids.push("dial");
    }
    ids.extend(["superAnimations", "superSaver", "superRainbow"]);
    ids
}

/// The deck from what is kept, and the parts with the controls each one put on it.
fn build(ctx: &Ctx, choices: &Choices, id: &str) -> (Profile, Vec<Stop>) {
    // the screensaver starts off (owner, 2026-10-16: "zostawiłbym defaultowo bez wygaszacza"; it
    // stays one switch away here and in Settings > Lights and screens); every other part starts on
    let keep = |stop: &str| choices.keep.get(stop).copied().unwrap_or(stop != "superSaver");
    let look = |stop: &str| choices.look.get(stop).copied().unwrap_or(0);
    let mut controls = serde_json::Map::new();
    let mut stops = Vec::new();
    // keys only: keys 13 and 14 are the volume, so the last row's parts stop before them
    let keys_only = !(ctx.has("faders") || ctx.has("knobs") || ctx.has("dial"));
    let mut put = |stop: &mut Stop, control: &str, b: Value| {
        let n: usize = control.strip_prefix("keys.").and_then(|n| n.parse().ok()).unwrap_or(0);
        if keys_only && n >= 13 && stop.id != "volume" {
            return;
        }
        stop.controls.push(control.to_string());
        if keep(stop.id) {
            controls.insert(control.to_string(), b);
        }
    };
    let call_exe = match ctx.call {
        Some("discord") => "Discord.exe",
        Some("teams") => "ms-teams.exe",
        Some("zoom") => "Zoom.exe",
        _ => "",
    };
    let call_name = match ctx.call {
        Some("discord") => "Discord",
        Some("teams") => "Teams",
        Some("zoom") => "Zoom",
        _ => "",
    };
    let dial_music = ctx.has("dial") && keep("music");
    // the Fun page is always on the deck the tour makes (owner, 2026-10-14): the animations,
    // Mochi on the dial, the screensavers
    let fun_page = true;
    for sid in stop_ids(ctx) {
        let mut stop = Stop { id: sid, keep: keep(sid), look: look(sid), looks: 1, controls: Vec::new(), try_control: None, tasks: Vec::new(), faces: Vec::new() };
        match sid {
            "volume" => {
                stop.looks = 2;
                let colored = look(sid) == 1;
                let paint = |mut b: Value, colored: bool| {
                    if colored {
                        let label = b["look"]["label"].as_str().unwrap_or_default().to_string();
                        b["look"]["led"] = json!(app_color(&label));
                    }
                    b["look"]["ledLevel"] = json!(true);
                    b
                };
                // what the controls set: per app on Windows, the system and the player on macOS
                // macOS 14.2+ sets each app's volume too (process taps; it asks once for System Audio Recording)
                let mut list: Vec<Value> = Vec::new();
                {
                    let front = if ctx.focus == Focus::Games { ctx.tr("Game", "Gra") } else { ctx.tr("App in front", "Na wierzchu") };
                    list.push(volume_of("front", "", &front, "app-window"));
                    if !call_exe.is_empty() {
                        list.push(volume_of("app", call_exe, call_name, "headphones"));
                    }
                    if ctx.music.as_deref() == Some("Spotify") {
                        list.push(volume_of("app", "Spotify.exe", &ctx.tr("Music", "Muzyka"), "music"));
                    } else if ctx.music.as_deref() == Some("Music") {
                        list.push(volume_of("app", "Music", &ctx.tr("Music", "Muzyka"), "music"));
                    }
                    if let Some(b) = &ctx.browser {
                        list.push(volume_of("app", b, &ctx.tr("Browser", "Przeglądarka"), "globe"));
                    }
                    list.push(volume_of("others", "", &ctx.tr("Other apps", "Reszta"), "layers"));
                }
                let system = volume_of("system", "", &ctx.tr("System", "System"), "volume-2");
                if ctx.has("faders") {
                    // the last fader is the whole system: the one the tour's sound follows
                    let mut five: Vec<Value> = list.iter().filter(|b| b["params"]["target"] != "system").take(4).cloned().collect();
                    five.push(system.clone());
                    let n = five.len();
                    for (i, b) in five.into_iter().enumerate() {
                        put(&mut stop, &format!("faders.{i}"), paint(b, colored));
                    }
                    stop.try_control = Some(format!("faders.{}", n - 1));
                } else if ctx.has("knobs") {
                    let mut three: Vec<Value> = list.iter().filter(|b| b["params"]["target"] != "system").take(2).cloned().collect();
                    three.push(system.clone());
                    for (i, b) in three.into_iter().enumerate() {
                        put(&mut stop, &format!("knobs.{i}"), paint(b, colored));
                    }
                    stop.try_control = Some("knobs.2".into());
                }
                if ctx.has("dial") && !dial_music {
                    let target = if ctx.has("knobs") || ctx.has("faders") {
                        volume_of("system", "", &ctx.tr("System", "System"), "volume-2")
                    } else {
                        volume_of("front", "", &ctx.tr("App in front", "Na wierzchu"), "app-window")
                    };
                    put(&mut stop, "dial", target);
                    stop.try_control.get_or_insert_with(|| "dial".into());
                }
                if !(ctx.has("faders") || ctx.has("knobs") || ctx.has("dial")) {
                    // keys only: quieter and louder on two keys
                    put(&mut stop, "keys.13", hotkey(ctx, "VolumeDown", "VolumeDown", &ctx.tr("Quieter", "Ciszej"), "volume-1"));
                    put(&mut stop, "keys.14", hotkey(ctx, "VolumeUp", "VolumeUp", &ctx.tr("Louder", "Głośniej"), "volume-2"));
                    stop.try_control = Some("keys.14".into());
                }
            }
            "mute" => {
                put(&mut stop, "keys.5", binding("core.mic", json!({}), &ctx.tr("Mic", "Mikrofon"), "mic"));
                stop.try_control = Some("keys.5".into());
                match ctx.call {
                    Some("discord") => {
                        put(&mut stop, "keys.6", hotkey(ctx, "Ctrl+Shift+M", "Cmd+Shift+M", &ctx.tr("Discord mute", "Discord mute"), "mic-off"));
                        put(&mut stop, "keys.7", hotkey(ctx, "Ctrl+Shift+D", "Cmd+Shift+D", &ctx.tr("Deafen", "Wycisz dźwięk"), "headphones"));
                    }
                    Some("teams") => {
                        put(&mut stop, "keys.6", hotkey(ctx, "Ctrl+Shift+M", "Cmd+Shift+M", &ctx.tr("Teams mute", "Teams mute"), "mic-off"));
                        put(&mut stop, "keys.7", hotkey(ctx, "Ctrl+Shift+O", "Cmd+Shift+O", &ctx.tr("Camera", "Kamera"), "video-off"));
                    }
                    Some("zoom") => {
                        put(&mut stop, "keys.6", hotkey(ctx, "Alt+A", "Cmd+Shift+A", &ctx.tr("Zoom mute", "Zoom mute"), "mic-off"));
                        put(&mut stop, "keys.7", hotkey(ctx, "Alt+V", "Cmd+Shift+V", &ctx.tr("Camera", "Kamera"), "video-off"));
                    }
                    _ => {
                        put(&mut stop, "keys.6", binding("core.volume", json!({"target": "system"}), &ctx.tr("Sound off", "Bez dźwięku"), "volume-x"));
                    }
                }
            }
            "music" => {
                if ctx.has("dial") {
                    stop.looks = 2;
                    let turn = if look(sid) == 1 { "seek" } else { "volume" };
                    // for the user's music app (owner, 2026-10-14: a press paused a video in Firefox
                    // instead of starting Spotify)
                    let player = ctx.music.as_deref().filter(|_| !ctx.mac).unwrap_or("");
                    put(&mut stop, "dial", binding("core.nowPlaying", json!({"turn": turn, "player": player}), &ctx.tr("Now playing", "Teraz gra"), "music"));
                    put(&mut stop, "keys.8", binding("core.media", json!({"command": "next"}), &ctx.tr("Next", "Następny"), "skip-forward"));
                    stop.try_control = Some("dial".into());
                } else {
                    put(&mut stop, "keys.8", binding("core.nowPlaying", json!({"turn": "volume"}), &ctx.tr("Now playing", "Teraz gra"), "music"));
                    if !fun_page {
                        put(&mut stop, "keys.9", binding("core.media", json!({"command": "next"}), &ctx.tr("Next", "Następny"), "skip-forward"));
                    }
                    stop.try_control = Some("keys.8".into());
                }
            }
            "apps" => {
                for (i, (exe, name, path)) in ctx.apps.iter().take(5).enumerate() {
                    let label: String = name.chars().take(12).collect();
                    put(&mut stop, &format!("keys.{i}"), binding("core.launch", json!({"target": path.clone().unwrap_or_else(|| exe.clone())}), &label, "app-window"));
                }
                stop.try_control = stop.controls.first().cloned();
                if ctx.has("rollers") {
                    put(&mut stop, "rollers.0", binding("core.scroll", json!({"axis": "vertical", "amount": 3}), &ctx.tr("Scroll", "Przewijanie"), "arrow-up-down"));
                    put(
                        &mut stop,
                        "rollers.1",
                        binding(
                            "core.hotkey",
                            json!({"clockwise": if ctx.mac { "Cmd+=" } else { "Ctrl+=" }, "counterClockwise": if ctx.mac { "Cmd+-" } else { "Ctrl+-" }}),
                            &ctx.tr("Zoom", "Zoom"),
                            "zoom-in",
                        ),
                    );
                }
            }
            "games" => {
                for (i, (name, target)) in ctx.games.iter().take(4).enumerate() {
                    let label: String = name.chars().take(12).collect();
                    put(&mut stop, &format!("keys.{}", 10 + i), binding("core.game", json!({"game": target}), &label, "gamepad-2"));
                }
                put(&mut stop, "keys.14", hotkey(ctx, "Win+Alt+G", "Cmd+Shift+5", &ctx.tr("Clip", "Klip"), "film"));
                stop.try_control = stop.controls.last().cloned();
            }
            "streaming" => {
                for (i, (action, en, pl, icon)) in [
                    ("obs.record", "Record", "Nagrywaj", "circle"),
                    ("obs.stream", "Go live", "Na żywo", "radio"),
                    ("obs.replay", "Replay", "Powtórka", "rewind"),
                    ("obs.screenshot", "Screenshot", "Zrzut", "camera"),
                    ("obs.virtualCam", "Virtual cam", "Kamera wirt.", "video"),
                ]
                .into_iter()
                .enumerate()
                {
                    put(&mut stop, &format!("keys.{}", 10 + i), binding(action, json!({}), &ctx.tr(en, pl), icon));
                }
                stop.try_control = Some("keys.11".into());
            }
            "work" => {
                for (i, (exe, name, path)) in ctx.apps.iter().skip(5).take(5).enumerate() {
                    let label: String = name.chars().take(12).collect();
                    put(&mut stop, &format!("keys.{}", 10 + i), binding("core.launch", json!({"target": path.clone().unwrap_or_else(|| exe.clone())}), &label, "app-window"));
                }
                stop.try_control = Some("keys.10".into());
            }
            "player" => {
                put(&mut stop, "keys.10", binding("core.media", json!({"command": "previous"}), &ctx.tr("Previous", "Poprzedni"), "skip-back"));
                put(&mut stop, "keys.11", binding("core.media", json!({"command": "playPause"}), &ctx.tr("Play", "Graj"), "play"));
                put(&mut stop, "keys.12", binding("core.media", json!({"command": "next"}), &ctx.tr("Next", "Następny"), "skip-forward"));
                put(&mut stop, "keys.13", binding("core.volume", json!({"target": "system"}), &ctx.tr("Sound off", "Bez dźwięku"), "volume-x"));
                put(&mut stop, "keys.14", binding(super::actions::CLOCK, json!({}), &ctx.tr("Clock", "Zegar"), "clock"));
                stop.try_control = Some("keys.11".into());
            }
            "extras" => {
                put(&mut stop, "keys.10", binding(super::actions::CLOCK, json!({}), &ctx.tr("Clock", "Zegar"), "clock"));
                put(&mut stop, "keys.11", binding(super::actions::TIMER, json!({"minutes": 25}), &ctx.tr("Focus 25", "Skupienie 25"), "timer"));
                put(&mut stop, "keys.12", hotkey(ctx, "Win+Shift+S", "Cmd+Shift+4", &ctx.tr("Screenshot", "Zrzut"), "camera"));
                put(&mut stop, "keys.13", binding(super::actions::MONITOR, json!({"metric": "cpu"}), &ctx.tr("CPU", "Procesor"), "cpu"));
                put(&mut stop, "keys.14", hotkey(ctx, "Win+L", "Ctrl+Cmd+Q", &ctx.tr("Lock", "Zablokuj"), "lock"));
                stop.try_control = Some("keys.11".into());
            }
            "superAnimations" => {
                // always kept: the way to the Fun page
                stop.keep = true;
                stop.controls.push("keys.9".into());
                stop.try_control = Some("keys.9".into());
            }
            "gestures" => {
                // the Mic key: a hold turns all sound off
                put(&mut stop, "keys.5:hold", binding("core.volume", json!({"target": "system"}), &ctx.tr("Sound off", "Bez dźwięku"), "volume-x"));
                stop.tasks.push("keys.5:hold".into());
                // the System knob (or a key without knobs): a double press switches speakers and headphones
                let (control, task) = if ctx.has("knobs") { ("knobs.2:double", "knobs.2:double") } else { ("keys.6:double", "keys.6:double") };
                put(&mut stop, control, binding("core.audioDevice", json!({"kind": "output", "device": ""}), &ctx.tr("Speakers / headphones", "Głośniki / słuchawki"), "headphones"));
                stop.tasks.push(task.into());
                // the dial: a hold opens the music app
                if ctx.has("dial") {
                    let b = match ctx.music.as_deref() {
                        Some(app) => binding("core.launch", json!({"target": app}), &format!("{} {app}", ctx.tr("Open", "Otwórz")), "music"),
                        None => binding("core.media", json!({"command": "next"}), &ctx.tr("Next", "Następny"), "skip-forward"),
                    };
                    put(&mut stop, "dial:hold", b);
                    stop.tasks.push("dial:hold".into());
                }
                stop.try_control = stop.tasks.first().map(|t| t.split(':').next().unwrap_or(t).to_string());
            }
            "dial" => {
                stop.faces = DIAL_FACES.to_vec();
                stop.looks = DIAL_FACES.len();
                // the first is what the dial is now (the music); the others come from the dial gallery
                let face = super::gallery::faces().into_iter().find(|f| f.id == DIAL_FACES[look(sid).min(DIAL_FACES.len() - 1)]);
                if look(sid) > 0
                    && let Some(face) = &face
                {
                    let mut b = serde_json::to_value(&face.binding).unwrap_or_default();
                    b["look"]["label"] = json!(face.caption_in(if ctx.polish { "pl" } else { "en" }));
                    put(&mut stop, "dial", b);
                }
                // each look brings its own double press and hold (none: the one from before goes)
                if let Some(face) = &face {
                    for gesture in ["dial:double", "dial:hold"] {
                        let b = face.extra.iter().find(|(g, _)| *g == gesture).map(|(_, b)| serde_json::to_value(b).unwrap_or_default()).unwrap_or(Value::Null);
                        put(&mut stop, gesture, b);
                    }
                }
                // the first look leaves the dial as the parts before made it (the music, or the volume)
                stop.try_control = Some("dial".into());
            }
            "superSaver" | "superRainbow" => {}
            _ => {}
        }
        stops.push(stop);
    }
    // the Fun page: HubDeck's own animations to look through, Mochi on the dial and off again,
    // the screensavers one after another, and the way back
    if fun_page {
        controls.insert("keys.9".into(), binding("core.page", json!({"page": "fun"}), &ctx.tr("Fun", "Zabawa"), "sparkles"));
        let mut tools: Vec<(usize, Value)> = vec![
            (11, binding(super::actions::SAVER, json!({"effect": ""}), &ctx.tr("Screensaver", "Wygaszacz"), "image")),
            (14, binding("core.page", json!({"page": ""}), &ctx.tr("Back", "Wróć"), "arrow-left")),
        ];
        if ctx.has("dial") {
            tools.push((10, binding(super::pet::ACTION, json!({"name": "Mochi", "press": "dial"}), "Mochi", "heart")));
        }
        let mut gifs = [
            "aurora.gif",
            "fire.gif",
            "rain.gif",
            "equalizer.gif",
            "orbit.gif",
            "lava.gif",
            "heartbeat.gif",
            "bars.gif",
            "ping.gif",
            "scope.gif",
            "rainbow.gif",
            "dots.gif",
            "typing.gif",
            "spinner.gif",
        ]
        .into_iter();
        for i in 0..15 {
            if let Some((_, b)) = tools.iter().find(|(k, _)| *k == i) {
                controls.insert(format!("fun/keys.{i}"), b.clone());
            } else if let Some(name) = gifs.next() {
                // a look to watch, nothing to run
                controls.insert(format!("fun/keys.{i}"), json!({"action": "", "look": {"label": "", "image": super::demo::animation(name), "fit": "fill"}}));
            }
        }
    }
    let kit = match ctx.kit() {
        "faders" => ctx.tr("faders", "suwaki"),
        "dialKnobs" => ctx.tr("dial and knobs", "dial i gałki"),
        _ => ctx.tr("keys", "klawisze"),
    };
    let mut p: Profile = serde_json::from_value(json!({
        "version": super::profile::PROFILE_VERSION,
        "id": id,
        "name": format!("{} · {kit}", ctx.tr("My deck", "Mój deck")),
        "apps": [],
        "controls": controls.into_iter().filter(|(_, b)| !b.is_null()).collect::<serde_json::Map<_, _>>(),
    }))
    .unwrap_or_else(|_| Profile::new("My deck"));
    if fun_page {
        p.pages = vec![Page { id: "fun".into(), name: ctx.tr("Fun", "Zabawa") }];
    }
    // the rainbow edge, or one calm color
    p.edge = if keep("superRainbow") { None } else { Some("#8B7CF6".into()) };
    super::assistant::with_real_apps(&mut p);
    super::house::apply(&mut p);
    (p, stops)
}

// ---------- the tour's life ----------

fn view_of(t: &Tour, draft: &str, stops: Vec<Stop>) -> View {
    View {
        draft: draft.to_string(),
        focus: t.ctx.focus,
        kit: t.ctx.kit(),
        mac: t.ctx.mac,
        stops,
        call: t.ctx.call,
        music: t.ctx.music.clone(),
        games: t.ctx.games.len(),
        rollers: t.ctx.has("rollers"),
    }
}

fn save_and_show(engine: &Engine, p: Profile) -> Result<(), String> {
    let id = p.id.clone();
    {
        let mut s = engine.lock();
        s.store.save_profile(p).map_err(|e| e.to_string())?;
        if !s.store.settings.drafts.contains(&id) {
            s.store.settings.drafts.push(id.clone());
            s.store.save_settings().map_err(|e| e.to_string())?;
        }
    }
    engine.try_profile(Some(&id))
}

/// What HubDeck would suggest, before anything is made (the first screen of the tour).
#[tauri::command]
pub async fn tour_suggest(app: AppHandle, language: String) -> Result<Suggestion, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = context(&app, None, language.starts_with("pl"));
        Suggestion { focus: ctx.focus, games: ctx.games.len(), obs: ctx.obs, call: ctx.call, music: ctx.music.clone(), kit: ctx.kit(), mac: ctx.mac }
    })
    .await
    .map_err(|e| e.to_string())
}

/// Make the deck for `focus` and show it on the deck; a tour that runs is ended first.
#[tauri::command]
pub async fn tour_start(app: AppHandle, focus: Focus, language: String) -> Result<View, String> {
    tauri::async_runtime::spawn_blocking(move || {
        end(&app, false)?;
        let engine = app.state::<Engine>();
        let ctx = context(&app, Some(focus), language.starts_with("pl"));
        let draft = uuid::Uuid::new_v4().to_string();
        let snapshot = take_snapshot(&draft);
        let _ = std::fs::write(snapshot_file(&engine), serde_json::to_vec(&snapshot).unwrap_or_default());
        let choices = Choices::default();
        let (p, stops) = build(&ctx, &choices, &draft);
        save_and_show(&engine, p)?;
        engine.lock().tour_dry_run = true;
        let t = Tour { ctx, choices, snapshot };
        let view = view_of(&t, &draft, stops);
        *TOUR.lock().unwrap_or_else(|e| e.into_inner()) = Some(t);
        Ok(view)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Keep or leave out a part, or pick its look; the deck follows at once.
#[tauri::command]
pub async fn tour_set(app: AppHandle, stop: String, keep: Option<bool>, look: Option<usize>) -> Result<View, String> {
    // off the window's thread: building and drawing the deck takes a moment
    tauri::async_runtime::spawn_blocking(move || set_now(&app, stop, keep, look)).await.map_err(|e| e.to_string())?
}

fn set_now(app: &AppHandle, stop: String, keep: Option<bool>, look: Option<usize>) -> Result<View, String> {
    let engine = app.state::<Engine>();
    let mut guard = TOUR.lock().unwrap_or_else(|e| e.into_inner());
    let t = guard.as_mut().ok_or("no tour runs")?;
    if let Some(k) = keep {
        t.choices.keep.insert(stop.clone(), k);
    }
    if let Some(l) = look {
        t.choices.look.insert(stop, l);
    }
    let draft = t.snapshot.draft.clone();
    let (p, stops) = build(&t.ctx, &t.choices, &draft);
    save_and_show(&engine, p)?;
    Ok(view_of(t, &draft, stops))
}

/// The tour's own sound (the volume stop): something to hear when nothing else plays.
#[tauri::command]
pub fn tour_sound(app: AppHandle, on: bool) -> Result<(), String> {
    if !on {
        super::sound::send(super::sound::Cmd::StopAll);
        return Ok(());
    }
    let engine = app.state::<Engine>();
    let file = engine.data_dir().join("tour-sound.wav");
    if !file.exists() {
        std::fs::write(&file, LOOP).map_err(|e| e.to_string())?;
    }
    super::sound::send(super::sound::Cmd::Play { file: file.to_string_lossy().into_owned(), volume: 0.8, device: String::new(), mode: super::sound::Mode::Restart });
    Ok(())
}

/// Show a screensaver on the deck for a few seconds (the finale).
#[tauri::command]
pub fn tour_saver(app: AppHandle, effect: String) -> Result<(), String> {
    let effect = if super::saver::EFFECTS.contains(&effect.as_str()) { effect } else { "rain".into() };
    app.state::<Engine>().lock().saver_preview = Some((effect, Instant::now() + Duration::from_secs(8)));
    Ok(())
}

/// How long a control blinks on the hardware when the tour points at it (or until it is touched).
const HIGHLIGHT_FOR: Duration = Duration::from_secs(10);

/// Point at a control on the hardware (`keys.5`, `knobs.2`, `faders.4`, `dial`): it blinks for a
/// few seconds or until it is touched; None stops it.
#[tauri::command]
pub fn tour_highlight(app: AppHandle, control: Option<String>) {
    let now = Instant::now();
    app.state::<Engine>().lock().highlight = control.map(|c| (c, now, now + HIGHLIGHT_FOR));
}

/// The end: keep the deck (and make it home) or throw it away. What was turned on the way (the
/// microphone, the volume) comes back either way.
#[tauri::command]
pub fn tour_finish(app: AppHandle, keep: bool, home: bool) -> Result<Option<String>, String> {
    let (draft, saver) = {
        let guard = TOUR.lock().unwrap_or_else(|e| e.into_inner());
        let t = guard.as_ref().ok_or("no tour runs")?;
        (t.snapshot.draft.clone(), t.choices.keep.get("superSaver").copied().unwrap_or(false))
    };
    let engine = app.state::<Engine>();
    if keep {
        engine.settle_draft(&draft, true)?;
        super::nudge::tour_kept(&draft);
        super::presses::forget_profile(&draft);
        if home {
            engine.activate(&draft)?;
        }
        if saver {
            let mut s = engine.lock();
            if s.store.settings.saver.is_empty() {
                s.store.settings.saver = "rain".into();
                s.store.save_settings().map_err(|e| e.to_string())?;
            }
        }
    }
    end(&app, keep)?;
    Ok(keep.then_some(draft))
}

/// Exit at any time: the deck as it was, the draft gone.
#[tauri::command]
pub fn tour_exit(app: AppHandle) -> Result<(), String> {
    end(&app, false)
}

/// End a running tour: the audio back, no more dry run; the draft is thrown away unless `kept`.
fn end(app: &AppHandle, kept: bool) -> Result<(), String> {
    let Some(t) = TOUR.lock().unwrap_or_else(|e| e.into_inner()).take() else { return Ok(()) };
    let engine = app.state::<Engine>();
    {
        let mut s = engine.lock();
        s.tour_dry_run = false;
        s.saver_preview = None;
        s.highlight = None;
    }
    if !kept {
        let _ = engine.try_profile(None);
        let _ = engine.settle_draft(&t.snapshot.draft, false);
    }
    restore_audio(&t.snapshot);
    let _ = std::fs::remove_file(snapshot_file(&engine));
    Ok(())
}

/// At start: a tour that did not end (a crash, a kill) is undone: its draft goes, the
/// microphone and the volume come back as they were before it.
pub fn recover(app: &AppHandle) {
    let engine = app.state::<Engine>();
    let file = snapshot_file(&engine);
    let Some(snapshot) = std::fs::read(&file).ok().and_then(|b| serde_json::from_slice::<Snapshot>(&b).ok()) else { return };
    log::info!("an unfinished tour: its draft goes, the audio comes back");
    let _ = engine.settle_draft(&snapshot.draft, false);
    restore_audio(&snapshot);
    let _ = std::fs::remove_file(file);
}

/// Actions that go live, record or end something: during the tour they only say what they would do.
pub fn dry_run_says(action: &str, polish: bool) -> Option<&'static str> {
    Some(match (action, polish) {
        ("obs.stream" | "streamlabs.stream", false) => "Would go live",
        ("obs.stream" | "streamlabs.stream", true) => "Tu: na żywo",
        ("obs.record" | "streamlabs.record" | "obs.recordPause", false) => "Would record",
        ("obs.record" | "streamlabs.record" | "obs.recordPause", true) => "Tu: nagrywanie",
        ("core.audioDevice", false) => "Would switch speakers and headphones",
        ("core.audioDevice", true) => "Tu: głośniki ↔ słuchawki",
        ("twitch.ad" | "twitch.clearChat" | "twitch.shield" | "twitch.title" | "twitch.chatMode" | "core.run" | "core.http", false) => "Not in the tour",
        ("twitch.ad" | "twitch.clearChat" | "twitch.shield" | "twitch.title" | "twitch.chatMode" | "core.run" | "core.http", true) => "Nie w pokazie",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(kinds: &[&str], focus: Focus) -> Ctx {
        Ctx {
            kinds: kinds.iter().map(|k| k.to_string()).collect(),
            mac: false,
            polish: false,
            focus,
            apps: (0..8).map(|i| (format!("app{i}.exe"), format!("App {i}"), None)).collect(),
            games: vec![("Helldivers 2".into(), "steam://rungameid/553850".into())],
            call: Some("discord"),
            music: Some("Spotify".into()),
            obs: true,
            browser: Some("chrome.exe".into()),
        }
    }

    fn controls(p: &Profile) -> Vec<String> {
        let mut c: Vec<String> = p.controls.keys().cloned().collect();
        c.sort();
        c
    }

    #[test]
    fn the_faders_kit_gets_the_volumes_on_faders_and_the_system_last() {
        let (p, stops) = build(&ctx(&["keys", "faders", "rollers"], Focus::Games), &Choices::default(), "t1");
        let c = controls(&p);
        assert!(c.iter().any(|x| x == "faders.4") && !c.iter().any(|x| x.starts_with("knobs")));
        assert_eq!(p.controls["faders.4"].params["target"], "system");
        let volume = stops.iter().find(|s| s.id == "volume").unwrap();
        assert_eq!(volume.try_control.as_deref(), Some("faders.4"));
        // what they do most comes right after the volume
        assert_eq!(stops[1].id, "games");
        assert!(c.iter().any(|x| x == "rollers.0"));
    }

    #[test]
    fn the_dial_kit_gets_knobs_and_the_music_on_the_dial() {
        let (p, _) = build(&ctx(&["keys", "dial", "knobs"], Focus::Mixed), &Choices::default(), "t2");
        assert_eq!(p.controls["dial"].action, "core.nowPlaying");
        assert_eq!(p.controls["knobs.2"].params["target"], "system");
        assert_eq!(p.controls["keys.5"].action, "core.mic");
    }

    #[test]
    fn a_part_left_out_leaves_the_deck_and_a_look_changes_it() {
        let mut ch = Choices::default();
        ch.keep.insert("music".into(), false);
        ch.look.insert("volume".into(), 1);
        let (p, stops) = build(&ctx(&["keys", "dial", "knobs"], Focus::Mixed), &ch, "t3");
        // without the music part the dial sets the volume
        assert_eq!(p.controls["dial"].action, "core.volume");
        assert!(p.controls["knobs.0"].look.led.is_some());
        assert!(!stops.iter().find(|s| s.id == "music").unwrap().keep);
    }

    #[test]
    fn keys_only_still_gets_a_volume_and_the_finale_a_fun_page() {
        let (p, _) = build(&ctx(&["keys"], Focus::Mixed), &Choices::default(), "t4");
        assert!(p.controls.contains_key("keys.14"));
        assert!(p.controls.contains_key("fun/keys.14"));
        assert_eq!(p.pages.len(), 1);
        assert!(!p.controls.contains_key("fun/keys.10") || p.controls["fun/keys.10"].action.is_empty(), "no dial: no Mochi key");
    }

    #[test]
    fn the_screensaver_starts_off_and_the_rest_on() {
        let (_, stops) = build(&ctx(&["keys", "dial", "knobs"], Focus::Mixed), &Choices::default(), "t9");
        assert!(stops.iter().find(|s| s.id == "superSaver").is_some_and(|s| !s.keep));
        assert!(stops.iter().find(|s| s.id == "superRainbow").is_some_and(|s| s.keep));
    }

    #[test]
    fn the_fun_page_is_always_there_with_mochi_and_the_screensavers() {
        // even when its part was not kept (owner, 2026-10-14)
        let mut ch = Choices::default();
        ch.keep.insert("superAnimations".into(), false);
        let (p, stops) = build(&ctx(&["keys", "dial", "knobs"], Focus::Mixed), &ch, "t7");
        assert_eq!(p.pages.len(), 1);
        assert_eq!(p.controls["keys.9"].action, "core.page");
        assert!(stops.iter().find(|s| s.id == "superAnimations").is_some_and(|s| s.keep));
        assert_eq!(p.controls["fun/keys.10"].action, super::super::pet::ACTION);
        assert_eq!(super::super::actions::param_str(&p.controls["fun/keys.10"].params, "press"), "dial");
        assert_eq!(p.controls["fun/keys.11"].action, super::super::actions::SAVER);
        assert_eq!(p.controls["fun/keys.14"].action, "core.page");
        // the animations are to watch: a press runs nothing
        assert!(p.controls["fun/keys.0"].action.is_empty());
    }

    #[test]
    fn many_jobs_on_a_key_a_knob_and_the_dial_and_the_dial_s_faces() {
        let mut ch = Choices::default();
        let (p, stops) = build(&ctx(&["keys", "dial", "knobs"], Focus::Mixed), &ch, "t5");
        let gestures = stops.iter().find(|s| s.id == "gestures").unwrap();
        assert_eq!(gestures.tasks, vec!["keys.5:hold", "knobs.2:double", "dial:hold"]);
        assert!(p.controls.contains_key("keys.5:hold") && p.controls.contains_key("knobs.2:double") && p.controls.contains_key("dial:hold"));
        assert_eq!(stops.iter().find(|s| s.id == "dial").unwrap().faces.len(), 8);
        // another face: the dial shows it, with its own hold and double press
        ch.look.insert("dial".into(), 2);
        let (p, _) = build(&ctx(&["keys", "dial", "knobs"], Focus::Mixed), &ch, "t5");
        assert_eq!(p.controls["dial"].action, super::super::actions::CLOCK);
        assert_eq!(super::super::actions::param_str(&p.controls["dial:hold"].params, "press"), "stopwatchReset");
        assert_eq!(super::super::actions::param_str(&p.controls["dial:double"].params, "press"), "hours");
        // no dial: no dial part, and the knob task stays
        let (_, stops) = build(&ctx(&["keys", "faders"], Focus::Mixed), &Choices::default(), "t6");
        assert!(!stops.iter().any(|s| s.id == "dial"));
        assert_eq!(stops.iter().find(|s| s.id == "gestures").unwrap().tasks, vec!["keys.5:hold", "keys.6:double"]);
    }

    #[test]
    fn going_live_is_only_said_during_the_tour() {
        assert!(dry_run_says("obs.stream", false).is_some());
        assert!(dry_run_says("core.mic", false).is_none());
    }
}
