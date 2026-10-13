//! Test profiles: one click in the "+" menu gives the owner a profile that exercises the features
//! waiting for a test on the hardware (`docs/ROADMAP.md` `[~]`, checklist in `private/TESTS.md`).
//! They fill the whole kit; the first key (or the dial) says what to try. Their pictures are our
//! own sample art (`resources/samples/`, made by `prototype/tools/gen_samples.py`), built into the
//! app and copied into the images folder when a test profile is added.

use serde_json::json;

use super::profile::{Binding, Look, Page, PanelContent, Profile};

const INFO: &str = "#3b2f12";
const KEY: &str = "#1f2a33";
const STATE: &str = "#2a1f33";
const TURN: &str = "#1f1d26";

/// Our own sample pictures, built into the binary (about 400 KB together).
const SAMPLES: [(&str, &[u8]); 11] = [
    ("aurora.gif", include_bytes!("../../resources/samples/aurora.gif")),
    ("fire.gif", include_bytes!("../../resources/samples/fire.gif")),
    ("loader.gif", include_bytes!("../../resources/samples/loader.gif")),
    ("bars.gif", include_bytes!("../../resources/samples/bars.gif")),
    ("ring.gif", include_bytes!("../../resources/samples/ring.gif")),
    ("bounce.gif", include_bytes!("../../resources/samples/bounce.gif")),
    ("dial-spin.gif", include_bytes!("../../resources/samples/dial-spin.gif")),
    ("violet.png", include_bytes!("../../resources/samples/violet.png")),
    ("sunset.png", include_bytes!("../../resources/samples/sunset.png")),
    ("sea.png", include_bytes!("../../resources/samples/sea.png")),
    ("panel.png", include_bytes!("../../resources/samples/panel.png")),
];

/// Copy a sample into the images folder and give the name a look uses (None in tests, where
/// there is no images folder).
fn sample(name: &str) -> Option<String> {
    let (_, bytes) = SAMPLES.iter().find(|(n, _)| *n == name)?;
    let ext = name.rsplit('.').next().unwrap_or("png");
    super::images::import_bytes(bytes, ext).ok()
}

fn b(action: &str, params: serde_json::Value, label: &str, icon: &str, bg: &str) -> Binding {
    Binding { action: action.into(), params, look: Look { label: Some(label.into()), icon: Some(icon.into()), bg: Some(bg.into()), ..Look::default() }, accel: false, look_on: None, swap_turns: false }
}

fn text(t: &str, label: &str) -> Binding {
    b("core.text", json!({"text": t}), label, "type", KEY)
}

fn put(p: &mut Profile, id: &str, binding: Binding) {
    p.controls.insert(id.into(), binding);
}

fn faders(p: &mut Profile) {
    for (i, (target, app, label)) in
        [("system", "", "System"), ("app", "chrome.exe", "Chrome"), ("app", "Spotify.exe", "Spotify"), ("app", "Discord.exe", "Discord"), ("front", "", "In front")].into_iter().enumerate()
    {
        put(p, &format!("faders.{i}"), b("core.volume", json!({"target": target, "app": app}), label, "volume-2", TURN));
    }
}

/// Taps, holds and double presses, multi-actions, toggles, pages, mute and push-to-talk, the timer
/// and the clock, typing, running a program.
pub fn keys_gestures() -> Profile {
    let mut p = Profile::new("Test: keys & gestures");
    put(&mut p, "keys.0", b("core.run", json!({"command": "notepad.exe", "args": "", "folder": "", "window": "normal"}), "1. Notepad", "terminal", INFO));
    put(&mut p, "keys.1", text("tap ", "Tap Hold Dbl"));
    put(&mut p, "keys.1:hold", text("HOLD ", "Hold"));
    put(&mut p, "keys.1:double", text("DOUBLE ", "Double"));
    put(
        &mut p,
        "keys.2",
        b(
            "core.multi",
            json!({"steps": [
                {"action": "core.text", "params": {"text": "one"}, "delay": 0},
                {"action": "core.text", "params": {"text": " two"}, "delay": 1000},
                {"action": "core.text", "params": {"text": " three"}, "delay": 1000},
                {"action": "core.hotkey", "params": {"keys": "Enter"}, "delay": 500}
            ]}),
            "Multi 1-2-3",
            "list-ordered",
            KEY,
        ),
    );
    put(
        &mut p,
        "keys.3",
        b(
            "core.toggle",
            json!({
                "first": [{"action": "core.text", "params": {"text": "ON "}, "delay": 0}, {"action": "core.text", "params": {"text": "(mic is muted) "}, "delay": 0, "when": "micMuted"}],
                "second": [{"action": "core.text", "params": {"text": "OFF "}, "delay": 0}]
            }),
            "Toggle",
            "refresh-cw",
            KEY,
        ),
    );
    put(&mut p, "keys.4", b("core.mic", json!({"mode": "toggle"}), "Mic", "mic", STATE));
    put(&mut p, "keys.5", b("core.mic", json!({"mode": "pushToTalk"}), "Push to talk", "mic", STATE));
    put(&mut p, "keys.6", b("core.timer", json!({"minutes": 0, "seconds": 10, "press": "startPause", "sound": "on"}), "Tea", "timer", KEY));
    put(&mut p, "keys.6:hold", b("core.timer", json!({"minutes": 0, "seconds": 10, "press": "reset", "sound": "on"}), "Reset", "timer", KEY));
    put(&mut p, "keys.7", b("core.clock", json!({"show": "timeDate", "hours": "24"}), "Clock", "clock", KEY));
    put(&mut p, "keys.8", b("core.page", json!({"page": "test-page-2"}), "Page 2", "folder", KEY));
    put(&mut p, "keys.9", text("Zażółć gęślą jaźń\n", "Polish text"));
    put(&mut p, "keys.10", b("core.hotkey", json!({"keys": "F13"}), "F13", "zap", KEY));
    put(&mut p, "keys.11", b("core.hotkey", json!({"keys": "W", "mode": "game", "hold": 30}), "Game W", "gamepad-2", KEY));
    put(&mut p, "keys.12", b("core.hotkey", json!({"keys": "Win+Shift+S"}), "Snip", "camera", KEY));
    put(&mut p, "keys.13", b("core.profile", json!({"which": "previous"}), "Prev profile", "skip-back", INFO));
    put(&mut p, "keys.14", b("core.profile", json!({"which": "next"}), "Next profile", "skip-forward", INFO));
    p.pages.push(Page { id: "test-page-2".into(), name: "Page 2".into() });
    put(&mut p, "test-page-2/keys.0", b("core.page", json!({"page": ""}), "Back", "arrow-left", INFO));
    put(&mut p, "test-page-2/keys.1", text("page two ", "Type"));
    put(&mut p, "dial", b("core.timer", json!({"minutes": 3, "seconds": 0, "press": "startPause", "sound": "on"}), "Turn = minutes", "timer", TURN));
    put(&mut p, "knobs.0", Binding { accel: true, ..b("core.volume", json!({"target": "system", "step": 2, "popup": "show"}), "Volume", "volume-2", TURN) });
    put(&mut p, "knobs.1", b("core.scroll", json!({"axis": "vertical", "amount": 3}), "Scroll", "arrow-up-down", TURN));
    put(&mut p, "knobs.2", b("core.deckBrightness", json!({"press": "darker", "step": 10}), "Brightness", "sun", TURN));
    put(&mut p, "rollers.0", b("core.scroll", json!({"axis": "vertical", "amount": 3}), "Scroll", "arrow-up-down", TURN));
    put(&mut p, "rollers.1", b("core.scroll", json!({"axis": "horizontal", "amount": 3}), "Scroll ↔", "arrow-left-right", TURN));
    faders(&mut p);
    p
}

/// Own pictures and animated GIFs on keys and the dial, colors, a custom "on" look, the base edge
/// and LED colors per knob and fader, with brightness that follows the value.
pub fn looks() -> Profile {
    let mut p = Profile::new("Test: looks");
    p.edge = Some("#ff6a00".into());
    p.edge_effect = Some("breathing".into());
    p.edge_speed = Some(40);
    let pic = |name: &str, label: &str, action: &str, params: serde_json::Value| {
        let mut x = b(action, params, label, "sparkles", KEY);
        x.look.image = sample(name);
        x
    };
    put(&mut p, "keys.0", b("core.text", json!({"text": ""}), "Look at keys, dial, LEDs", "sparkles", INFO));
    put(&mut p, "keys.1", pic("ring.gif", "", "core.media", json!({"command": "playPause"})));
    put(&mut p, "keys.2", pic("bounce.gif", "Bounce", "core.media", json!({"command": "next"})));
    put(&mut p, "keys.3", pic("violet.png", "", "core.media", json!({"command": "previous"})));
    put(&mut p, "keys.4", pic("sunset.png", "Sunset", "core.hotkey", json!({"keys": "F14"})));
    put(&mut p, "keys.5", pic("sea.png", "Sea", "core.hotkey", json!({"keys": "F15"})));
    for (i, (bg, fg, label)) in [("#7a1f24", "#ffffff", "Red"), ("#1f5a2c", "#d8ffd8", "Green"), ("#203a7a", "#ffe08a", "Blue"), ("#ffffff", "#111111", "Light")].into_iter().enumerate() {
        let mut x = b("core.hotkey", json!({"keys": "F16"}), label, "star", bg);
        x.look.fg = Some(fg.into());
        put(&mut p, &format!("keys.{}", 6 + i), x);
    }
    let mut mic = b("core.mic", json!({"mode": "toggle"}), "Mic", "mic", STATE);
    mic.look_on = Some(Look { label: Some("MUTED".into()), icon: Some("mic-off".into()), bg: Some("#b0301f".into()), ..Look::default() });
    put(&mut p, "keys.10", mic);
    put(&mut p, "keys.11", b("core.lighting", json!({"target": "all", "effect": "rainbow", "speed": 70}), "All rainbow", "lightbulb", KEY));
    put(&mut p, "keys.11:hold", b("core.lighting", json!({"target": "all", "effect": "profile"}), "Back", "lightbulb", KEY));
    put(&mut p, "keys.12", b("core.hotkey", json!({"keys": "F18"}), "Big", "type", KEY));
    put(&mut p, "keys.13", b("core.profile", json!({"which": "previous"}), "Prev profile", "skip-back", INFO));
    put(&mut p, "keys.14", b("core.profile", json!({"which": "next"}), "Next profile", "skip-forward", INFO));
    let mut dial = b("core.volume", json!({"target": "system", "step": 2}), "GIF dial", "volume-2", TURN);
    dial.look.image = sample("dial-spin.gif");
    put(&mut p, "dial", dial);
    for (i, (color, app, label)) in [("#ff2020", "Spotify.exe", "Spotify"), ("#20ff40", "chrome.exe", "Chrome"), ("#2080ff", "Discord.exe", "Discord")].into_iter().enumerate() {
        let mut k = b("core.volume", json!({"target": "app", "app": app, "step": 5}), label, "volume-2", TURN);
        k.look.led = Some(color.into());
        // knob 1: brightness follows the volume; knobs 2 and 3: hardware effects (one speed per module)
        match i {
            0 => k.look.led_level = true,
            1 => k.look.led_effect = Some("water".into()),
            _ => {
                k.look.led_effect = Some("strobe".into());
                k.look.led_speed = Some(60);
            }
        }
        put(&mut p, &format!("knobs.{i}"), k);
    }
    put(&mut p, "rollers.0", b("core.scroll", json!({"axis": "vertical", "amount": 3}), "Scroll", "arrow-up-down", TURN));
    put(&mut p, "rollers.1", b("core.scroll", json!({"axis": "horizontal", "amount": 3}), "Scroll ↔", "arrow-left-right", TURN));
    faders(&mut p);
    for (i, color) in ["#ff0000", "#ffaa00", "#00ff00", "#00aaff", "#aa00ff"].into_iter().enumerate() {
        if let Some(f) = p.controls.get_mut(&format!("faders.{i}")) {
            f.look.led = Some(color.into());
            f.look.led_level = true;
        }
    }
    p
}

/// Dial modes, volume with acceleration and the Windows popup, per-app volume, deck and monitor
/// brightness, next / previous profile by turning, scrolling.
pub fn turning() -> Profile {
    let mut p = Profile::new("Test: knobs, dial, rollers");
    put(
        &mut p,
        "dial",
        b(
            "core.modes",
            json!({"modes": [
                {"label": "Volume", "action": "core.volume", "params": {"target": "system", "step": 2, "popup": "show"}},
                {"label": "Scroll", "action": "core.scroll", "params": {"axis": "vertical", "amount": 3}},
                {"label": "Deck light", "action": "core.deckBrightness", "params": {"step": 5}},
                {"label": "Monitor", "action": "core.brightness", "params": {"monitor": "all", "step": 5}},
                {"label": "Profiles", "action": "core.profile", "params": {"which": "next"}}
            ]}),
            "Press = next mode",
            "layers",
            TURN,
        ),
    );
    put(&mut p, "knobs.0", Binding { accel: true, ..b("core.volume", json!({"target": "system", "step": 2, "popup": "show"}), "Fast = x3", "volume-2", TURN) });
    put(&mut p, "knobs.1", b("core.volume", json!({"target": "app", "app": "spotify.exe", "step": 3}), "Spotify", "music", TURN));
    put(&mut p, "knobs.2", b("core.brightness", json!({"monitor": "all", "step": 5}), "Monitor", "sun", TURN));
    put(&mut p, "rollers.0", b("core.scroll", json!({"axis": "vertical", "amount": 3}), "Scroll", "arrow-up-down", TURN));
    put(&mut p, "rollers.1", b("core.profile", json!({"which": "next"}), "Profiles", "layers", TURN));
    put(&mut p, "keys.0", b("core.text", json!({"text": ""}), "Turn the dial, knobs, rollers", "sparkles", INFO));
    put(&mut p, "keys.1", b("core.deckBrightness", json!({"press": "brighter", "step": 20}), "Brighter", "sun", KEY));
    put(&mut p, "keys.2", b("core.deckBrightness", json!({"press": "darker", "step": 20}), "Darker", "moon", KEY));
    put(&mut p, "keys.3", b("core.hotkey", json!({"keys": "VolumeUp"}), "Vol +", "volume-2", KEY));
    put(&mut p, "keys.4", b("core.hotkey", json!({"keys": "VolumeDown"}), "Vol -", "volume-x", KEY));
    put(&mut p, "keys.5", b("core.volume", json!({"target": "system"}), "Mute", "volume-x", STATE));
    put(&mut p, "keys.6", b("core.volume", json!({"target": "front"}), "Mute front app", "volume-x", STATE));
    put(&mut p, "keys.7", b("core.audioDevice", json!({"kind": "output", "device": ""}), "Next speakers", "headphones", KEY));
    put(&mut p, "keys.8", b("core.audioDevice", json!({"kind": "input", "device": ""}), "Next mic", "mic", KEY));
    put(&mut p, "keys.9", b("core.media", json!({"command": "playPause"}), "Play", "play", KEY));
    put(&mut p, "keys.10", b("core.timer", json!({"minutes": 1, "seconds": 0, "press": "startPause", "sound": "off"}), "Silent 1 min", "timer", KEY));
    put(&mut p, "keys.11", b("core.clock", json!({"show": "time", "hours": "12"}), "12 h", "clock", KEY));
    put(&mut p, "keys.12", b("core.clock", json!({"show": "date", "hours": "24"}), "Date", "calendar", KEY));
    put(&mut p, "keys.13", b("core.profile", json!({"which": "previous"}), "Prev profile", "skip-back", INFO));
    put(&mut p, "keys.14", b("core.profile", json!({"which": "next"}), "Next profile", "skip-forward", INFO));
    put(&mut p, "faders.0", b("core.brightness", json!({"monitor": "all"}), "Monitor", "sun", TURN));
    put(&mut p, "faders.1", b("core.deckBrightness", json!({}), "Deck light", "sun", TURN));
    for (i, (target, app, label)) in [("system", "", "System"), ("app", "chrome.exe", "Chrome"), ("app", "Spotify.exe", "Spotify")].into_iter().enumerate() {
        put(&mut p, &format!("faders.{}", i + 2), b("core.volume", json!({"target": target, "app": app}), label, "volume-2", TURN));
    }
    p
}

/// Now playing, system monitor, OBS, Streamlabs, Spotify, audio devices, Home Assistant, MIDI.
pub fn integrations() -> Profile {
    let mut p = Profile::new("Test: integrations");
    put(&mut p, "dial", b("core.nowPlaying", json!({"turn": "volume"}), "Play music", "music", TURN));
    put(&mut p, "keys.0", b("core.nowPlaying", json!({"turn": "tracks"}), "Now playing", "music", INFO));
    for (i, (metric, label)) in [("cpu", "CPU"), ("ram", "Memory"), ("gpu", "GPU"), ("gpuTemp", "GPU °C")].into_iter().enumerate() {
        put(&mut p, &format!("keys.{}", 1 + i), b("core.monitor", json!({"metric": metric}), label, "monitor", KEY));
    }
    put(&mut p, "keys.5", b("obs.record", json!({}), "OBS rec", "circle-dot", STATE));
    put(&mut p, "keys.6", b("obs.scene", json!({"scene": "Scene"}), "OBS Scene", "monitor", STATE));
    put(&mut p, "keys.7", b("obs.mute", json!({"input": "Mic/Aux", "mode": "pushToTalk"}), "OBS PTT", "mic", STATE));
    put(&mut p, "keys.8", b("obs.virtualCam", json!({}), "OBS cam", "video", STATE));
    put(&mut p, "keys.9", b("spotify.like", json!({}), "Like", "heart", STATE));
    put(&mut p, "keys.10", b("core.audioDevice", json!({"kind": "output", "device": ""}), "Speakers", "headphones", KEY));
    put(&mut p, "keys.11", b("ha.toggle", json!({"entity": "light.example"}), "HA light", "lightbulb", STATE));
    put(&mut p, "keys.12", b("streamlabs.record", json!({}), "SL rec", "circle-dot", STATE));
    put(&mut p, "keys.13", b("twitch.marker", json!({"description": "HubDeck test"}), "Twitch mark", "star", KEY));
    put(&mut p, "keys.14", b("core.profile", json!({"which": "next"}), "Next profile", "skip-forward", INFO));
    put(&mut p, "knobs.0", b("obs.volume", json!({"input": "Desktop Audio", "step": 1}), "OBS desk", "volume-2", TURN));
    put(&mut p, "knobs.1", b("midi.cc", json!({"port": "", "channel": 1, "cc": 1, "step": 2}), "MIDI CC1", "music", TURN));
    put(&mut p, "knobs.2", b("core.brightness", json!({"monitor": "all", "step": 5}), "Monitor", "sun", TURN));
    put(&mut p, "rollers.0", b("core.nowPlaying", json!({"turn": "tracks"}), "Tracks", "music", TURN));
    put(&mut p, "rollers.1", b("core.scroll", json!({"axis": "vertical", "amount": 3}), "Scroll", "arrow-up-down", TURN));
    faders(&mut p);
    p
}

/// One picture across all 15 keys (the keys still act); switch it to the clock or the live graph in
/// the profile panel.
pub fn panel() -> Profile {
    let mut p = Profile::new("Test: full panel");
    p.panel = Some(match sample("panel.png") {
        Some(image) => PanelContent::Picture { image },
        None => PanelContent::Clock,
    });
    p.edge = Some("#2080ff".into());
    put(&mut p, "dial", b("core.text", json!({"text": ""}), "Keys = one picture", "sparkles", INFO));
    for i in 0..13 {
        put(&mut p, &format!("keys.{i}"), b("core.hotkey", json!({"keys": "F19"}), &format!("F19 #{i}"), "zap", KEY));
    }
    put(&mut p, "keys.13", b("core.profile", json!({"which": "previous"}), "Prev profile", "skip-back", INFO));
    put(&mut p, "keys.14", b("core.profile", json!({"which": "next"}), "Next profile", "skip-forward", INFO));
    put(&mut p, "knobs.0", b("core.volume", json!({"target": "system", "step": 2}), "Volume", "volume-2", TURN));
    put(&mut p, "knobs.1", b("core.scroll", json!({"axis": "vertical", "amount": 3}), "Scroll", "arrow-up-down", TURN));
    put(&mut p, "knobs.2", b("core.deckBrightness", json!({"step": 10}), "Brightness", "sun", TURN));
    put(&mut p, "rollers.0", b("core.scroll", json!({"axis": "vertical", "amount": 3}), "Scroll", "arrow-up-down", TURN));
    put(&mut p, "rollers.1", b("core.profile", json!({"which": "next"}), "Profiles", "layers", TURN));
    faders(&mut p);
    p
}

/// Programs worth a key, where Windows and their installers usually put them (`%VAR%` expanded;
/// a `*` matches the newest folder, e.g. Photoshop's version folder).
const KNOWN_APPS: [(&str, &[&str]); 18] = [
    ("Chrome", &[r"%ProgramFiles%\Google\Chrome\Application\chrome.exe", r"%ProgramFiles(x86)%\Google\Chrome\Application\chrome.exe", r"%LOCALAPPDATA%\Google\Chrome\Application\chrome.exe"]),
    ("Edge", &[r"%ProgramFiles(x86)%\Microsoft\Edge\Application\msedge.exe", r"%ProgramFiles%\Microsoft\Edge\Application\msedge.exe"]),
    ("Firefox", &[r"%ProgramFiles%\Mozilla Firefox\firefox.exe", r"%ProgramFiles(x86)%\Mozilla Firefox\firefox.exe"]),
    ("Spotify", &[r"%APPDATA%\Spotify\Spotify.exe"]),
    ("Discord", &[r"%LOCALAPPDATA%\Discord\app-*\Discord.exe"]),
    ("Steam", &[r"%ProgramFiles(x86)%\Steam\steam.exe", r"%ProgramFiles%\Steam\steam.exe"]),
    ("OBS", &[r"%ProgramFiles%\obs-studio\bin\64bit\obs64.exe"]),
    ("VS Code", &[r"%LOCALAPPDATA%\Programs\Microsoft VS Code\Code.exe", r"%ProgramFiles%\Microsoft VS Code\Code.exe"]),
    ("Photoshop", &[r"%ProgramFiles%\Adobe\Adobe Photoshop *\Photoshop.exe"]),
    ("Word", &[r"%ProgramFiles%\Microsoft Office\root\Office16\WINWORD.EXE"]),
    ("Excel", &[r"%ProgramFiles%\Microsoft Office\root\Office16\EXCEL.EXE"]),
    ("Telegram", &[r"%APPDATA%\Telegram Desktop\Telegram.exe"]),
    ("Afterburner", &[r"%ProgramFiles(x86)%\MSI Afterburner\MSIAfterburner.exe"]),
    ("Explorer", &[r"%WINDIR%\explorer.exe"]),
    ("Notepad", &[r"%WINDIR%\System32\notepad.exe"]),
    ("Calculator", &[r"%WINDIR%\System32\calc.exe"]),
    ("Paint", &[r"%WINDIR%\System32\mspaint.exe"]),
    ("Task Manager", &[r"%WINDIR%\System32\Taskmgr.exe"]),
];

/// `%VAR%` -> its value; None when a variable is not set.
fn expand(pattern: &str) -> Option<String> {
    let mut out = String::new();
    let mut parts = pattern.split('%');
    out.push_str(parts.next()?);
    let mut var = true;
    for part in parts {
        if var {
            out.push_str(&std::env::var(part).ok()?);
        } else {
            out.push_str(part);
        }
        var = !var;
    }
    Some(out)
}

/// The first existing path of a pattern; one `*` folder takes the newest match.
fn find(pattern: &str) -> Option<std::path::PathBuf> {
    let path = std::path::PathBuf::from(expand(pattern)?);
    if !pattern.contains('*') {
        return path.is_file().then_some(path);
    }
    let text = path.to_string_lossy().to_string();
    let (before, after) = text.split_once('*')?;
    let (dir, prefix) = before.rsplit_once(['\\', '/'])?;
    let (rest_of_name, tail) = after.split_once(['\\', '/']).unwrap_or((after, ""));
    let mut found: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|e| {
            let n = e.file_name().to_string_lossy().to_string();
            n.starts_with(prefix) && n.ends_with(rest_of_name)
        })
        .map(|e| e.path().join(tail))
        .filter(|p| p.is_file())
        .collect();
    found.sort();
    found.pop()
}

/// An "Open app" key with the program's own icon.
fn app_key(name: &str, path: &str) -> Binding {
    let folder = std::path::Path::new(path).parent().map(|d| d.to_string_lossy().to_string()).unwrap_or_default();
    let mut k = b("core.run", json!({"command": path, "args": "", "folder": folder, "window": "normal"}), name, "app-window", KEY);
    k.look.image = super::appicon::png(path).ok().and_then(|png| super::images::import_bytes(&png, "png").ok());
    k
}

/// The apps on this PC (known places first, then the ones used recently), up to `max`.
fn apps_here(max: usize) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = KNOWN_APPS.iter().filter_map(|(name, patterns)| patterns.iter().find_map(|p| find(p)).map(|p| (name.to_string(), p.to_string_lossy().to_string()))).collect();
    for a in super::apps::list() {
        let taken = out.iter().any(|(_, p)| p.eq_ignore_ascii_case(&a.path));
        if !taken && !a.path.is_empty() && out.len() < max {
            out.push((a.exe.trim_end_matches(".exe").trim_end_matches(".EXE").to_string(), a.path));
        }
    }
    out.truncate(max);
    out
}

/// Real apps found on this PC as keys with their own icons, our animated art on the dial and
/// two keys, made when the profile is added.
pub fn real_apps() -> Profile {
    let mut p = Profile::new("Test: real apps");
    let apps = apps_here(12);
    for (i, (name, path)) in apps.iter().enumerate() {
        put(&mut p, &format!("keys.{i}"), app_key(name, path));
    }
    for i in apps.len()..12 {
        put(&mut p, &format!("keys.{i}"), b("core.text", json!({"text": ""}), "No app", "app-window", KEY));
    }
    let mut loading = b("core.timer", json!({"minutes": 0, "seconds": 30, "press": "startPause", "sound": "off"}), "", "timer", KEY);
    loading.look.image = sample("loader.gif");
    put(&mut p, "keys.12", loading);
    let mut music = b("core.media", json!({"command": "playPause"}), "", "music", KEY);
    music.look.image = sample("bars.gif");
    put(&mut p, "keys.13", music);
    put(&mut p, "keys.14", b("core.profile", json!({"which": "next"}), "Next profile", "skip-forward", INFO));
    let mut dial = b("core.volume", json!({"target": "system", "step": 2, "popup": "show"}), "", "volume-2", TURN);
    dial.look.image = sample("fire.gif");
    put(&mut p, "dial", dial);
    put(&mut p, "knobs.0", b("core.volume", json!({"target": "front", "step": 3}), "App in front", "volume-2", TURN));
    put(&mut p, "knobs.1", b("core.scroll", json!({"axis": "vertical", "amount": 3}), "Scroll", "arrow-up-down", TURN));
    put(&mut p, "knobs.2", b("core.deckBrightness", json!({"step": 10}), "Brightness", "sun", TURN));
    put(&mut p, "rollers.0", b("core.scroll", json!({"axis": "vertical", "amount": 3}), "Scroll", "arrow-up-down", TURN));
    put(&mut p, "rollers.1", b("core.profile", json!({"which": "next"}), "Profiles", "layers", TURN));
    faders(&mut p);
    p
}

/// An animated aurora across all 15 keys (the keys still act), fire on the dial.
pub fn aurora() -> Profile {
    let mut p = Profile::new("Test: aurora");
    p.panel = sample("aurora.gif").map(|image| PanelContent::Picture { image }).or(Some(PanelContent::Clock));
    p.edge = Some("#40ffa0".into());
    p.edge_effect = Some("wave".into());
    p.edge_speed = Some(30);
    for i in 0..13 {
        put(&mut p, &format!("keys.{i}"), b("core.hotkey", json!({"keys": "F20"}), &format!("F20 #{i}"), "zap", KEY));
    }
    put(&mut p, "keys.13", b("core.profile", json!({"which": "previous"}), "Prev profile", "skip-back", INFO));
    put(&mut p, "keys.14", b("core.profile", json!({"which": "next"}), "Next profile", "skip-forward", INFO));
    let mut dial = b("core.volume", json!({"target": "system", "step": 2}), "", "volume-2", TURN);
    dial.look.image = sample("fire.gif");
    put(&mut p, "dial", dial);
    for (i, effect) in ["wave", "water", "breathing"].into_iter().enumerate() {
        let mut k = b("core.volume", json!({"target": "system", "step": 2}), "Volume", "volume-2", TURN);
        k.look.led = Some("#40ffa0".into());
        k.look.led_effect = Some(effect.into());
        put(&mut p, &format!("knobs.{i}"), k);
    }
    put(&mut p, "rollers.0", b("core.scroll", json!({"axis": "vertical", "amount": 3}), "Scroll", "arrow-up-down", TURN));
    put(&mut p, "rollers.1", b("core.profile", json!({"which": "next"}), "Profiles", "layers", TURN));
    faders(&mut p);
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_are_readable_pictures() {
        for (name, bytes) in SAMPLES {
            let img = image::load_from_memory(bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(img.width() >= 72, "{name} is big enough for a key");
        }
        assert!(SAMPLES.iter().map(|(_, b)| b.len()).sum::<usize>() < 450 * 1024, "small enough to build in");
    }

    #[test]
    fn test_profiles_fill_the_kit_and_say_what_to_try() {
        for p in [keys_gestures(), looks(), turning(), integrations(), panel(), real_apps(), aurora()] {
            for id in ["dial", "knobs.0", "knobs.1", "knobs.2", "rollers.0", "rollers.1", "faders.0", "faders.4", "keys.0", "keys.14"] {
                assert!(p.controls.contains_key(id), "{}: {id}", p.name);
            }
            assert!(p.name.starts_with("Test: "));
        }
        assert_eq!(expand("%HUBDECK_SURELY_UNSET%\\x"), None);
        #[cfg(windows)]
        {
            assert!(find(r"%WINDIR%\System32\notepad.exe").is_some());
            assert!(apps_here(12).iter().any(|(n, _)| n == "Notepad"), "Windows' own apps are found");
        }
        let k = keys_gestures();
        assert!(k.controls.contains_key("keys.1:hold") && k.controls.contains_key("keys.1:double"));
        assert!(k.pages.iter().any(|pg| k.controls.contains_key(&format!("{}/keys.0", pg.id))));
    }
}
