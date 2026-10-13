//! Ready-made profiles. Every preset fills all control kinds; whatever is attached uses its part.

use serde_json::json;

use super::profile::{Binding, Look, Profile};

fn bind(action: &str, params: serde_json::Value, label: &str, icon: &str, bg: &str) -> Binding {
    Binding { action: action.into(), params, look: Look { label: Some(label.into()), icon: Some(icon.into()), bg: Some(bg.into()), ..Look::default() }, accel: false, look_on: None, swap_turns: false }
}

const MEDIA: &str = "#1d2b3a";
const EDIT: &str = "#262333";
const WEB: &str = "#1f3326";
const FN: &str = "#2b2733";

/// "Everyday desk": media, editing shortcuts, per-app volume on the faders, scrolling on the
/// rollers, volume on the dial.
pub fn everyday_desk() -> Profile {
    let cmd = if cfg!(target_os = "macos") { "Cmd" } else { "Ctrl" };
    let screenshot = if cfg!(target_os = "macos") { "Cmd+Shift+4" } else { "PrintScreen" };
    let mut p = Profile::new("Everyday desk");
    let c = &mut p.controls;
    let keys = [
        bind("core.media", json!({"command": "previous"}), "Prev", "skip-back", MEDIA),
        bind("core.media", json!({"command": "playPause"}), "Play", "play", MEDIA),
        bind("core.media", json!({"command": "next"}), "Next", "skip-forward", MEDIA),
        bind("core.hotkey", json!({"keys": "VolumeMute"}), "Mute", "volume-x", MEDIA),
        bind("core.hotkey", json!({"keys": screenshot}), "Screenshot", "camera", MEDIA),
        bind("core.hotkey", json!({"keys": format!("{cmd}+C")}), "Copy", "copy", EDIT),
        bind("core.hotkey", json!({"keys": format!("{cmd}+V")}), "Paste", "clipboard-paste", EDIT),
        bind("core.hotkey", json!({"keys": format!("{cmd}+Z")}), "Undo", "undo-2", EDIT),
        bind("core.hotkey", json!({"keys": format!("{cmd}+F")}), "Find", "search", EDIT),
        bind("core.launch", json!({"target": "https://www.google.com"}), "Web", "globe", WEB),
        bind("core.hotkey", json!({"keys": "F13"}), "F13", "zap", FN),
        bind("core.hotkey", json!({"keys": "F14"}), "F14", "zap", FN),
        bind("core.hotkey", json!({"keys": "F15"}), "F15", "zap", FN),
        bind("core.hotkey", json!({"keys": "F16"}), "F16", "zap", FN),
        bind("core.hotkey", json!({"keys": "F17"}), "F17", "zap", FN),
    ];
    for (i, b) in keys.into_iter().enumerate() {
        c.insert(format!("keys.{i}"), b);
    }
    c.insert("dial".into(), bind("core.volume", json!({"target": "system", "step": 2}), "Volume", "volume-2", "#1f1d26"));
    c.insert("knobs.0".into(), bind("core.volume", json!({"target": "app", "app": "spotify.exe", "step": 3}), "Spotify", "music", "#1f1d26"));
    c.insert("knobs.1".into(), bind("core.media", json!({"command": "playPause"}), "Tracks", "play", "#1f1d26"));
    c.insert("knobs.2".into(), bind("core.scroll", json!({"axis": "horizontal", "amount": 3}), "Scroll ↔", "arrow-left-right", "#1f1d26"));
    let faders = [("system", "", "System"), ("app", "chrome.exe", "Chrome"), ("app", "Discord.exe", "Discord"), ("app", "Spotify.exe", "Spotify"), ("app", "firefox.exe", "Firefox")];
    for (i, (target, app, label)) in faders.into_iter().enumerate() {
        c.insert(format!("faders.{i}"), bind("core.volume", json!({"target": target, "app": app}), label, "volume-2", "#1f1d26"));
    }
    c.insert("rollers.0".into(), bind("core.scroll", json!({"axis": "vertical", "amount": 3}), "Scroll", "arrow-up-down", "#1f1d26"));
    c.insert("rollers.1".into(), bind("core.scroll", json!({"axis": "horizontal", "amount": 3}), "Scroll ↔", "arrow-left-right", "#1f1d26"));
    p
}

const BOARD: &str = "#1b2430";
const HOUSE: &str = "#1f2b24";

/// A "Live value" key: `params` say where from and how to show it, `label` is the caption.
fn live(params: serde_json::Value, label: &str, icon: &str, bg: &str) -> Binding {
    let mut params = params;
    for (k, v) in [("every", json!(60)), ("decimals", json!(-1)), ("below", json!("false")), ("max", json!(0))] {
        if params.get(k).is_none() {
            params[k] = v;
        }
    }
    let mut b = bind(super::live::ACTION, params, label, icon, bg);
    b.look.style = Some("glass".into());
    b
}

/// "Status board": the time, the weather, the next meeting, the computer's load, a website,
/// a build, the home (Home Assistant) and a price, each refreshing by itself and turning orange
/// or red past its thresholds. Fields marked "set up" wait for the user's own place, calendar,
/// addresses and sensors (`docs/STATUS-BOARD.md`).
pub fn status_board() -> Profile {
    let mut p = Profile::new("Status board");
    let disk = if cfg!(windows) { r#"powershell -NoProfile -Command "[math]::Round((Get-PSDrive C).Free/1GB)""# } else { "df -g / | awk 'NR==2 {print $4}'" };
    put_keys(
        &mut p,
        vec![
            bind(super::actions::CLOCK, json!({"show": "timeDate", "hours": "24"}), "Time", "clock", BOARD),
            live(json!({"source": "weather", "place": "", "units": "metric"}), "", "sun", BOARD),
            live(json!({"source": "calendar", "url": "", "every": 300, "warn": "", "alert": ""}), "", "calendar", BOARD),
            bind(super::actions::MONITOR, json!({"metric": "cpu"}), "CPU", "monitor", BOARD),
            bind(super::actions::MONITOR, json!({"metric": "ram"}), "Memory", "layers", BOARD),
            live(json!({"source": "url", "url": "", "path": "", "open": ""}), "Website", "globe", WEB),
            live(json!({"source": "url", "url": "", "path": "workflow_runs.0.conclusion", "every": 120}), "Build", "git-branch", WEB),
            live(
                json!({"source": "url", "url": "https://api.coingecko.com/api/v3/simple/price?ids=bitcoin&vs_currencies=usd", "path": "bitcoin.usd", "prefix": "$", "decimals": 0, "every": 300, "open": "https://www.coingecko.com/en/coins/bitcoin"}),
                "Bitcoin",
                "star",
                WEB,
            ),
            live(json!({"source": "command", "command": disk, "suffix": " GB", "warn": "50", "alert": "20", "below": "true", "every": 600}), "Disk free", "folder", BOARD),
            bind(super::actions::MONITOR, json!({"metric": "gpu"}), "GPU", "monitor", BOARD),
            live(json!({"source": "ha", "entity": "sensor.living_room_temperature", "decimals": 1, "suffix": "°", "warn": "26", "alert": "29"}), "Inside", "home", HOUSE),
            live(json!({"source": "ha", "entity": "sensor.phone_battery_level", "suffix": "%", "warn": "30", "alert": "15", "below": "true", "max": 100}), "Phone", "zap", HOUSE),
            bind("ha.toggle", json!({"entity": "light.desk"}), "Desk light", "lightbulb", HOUSE),
            bind("core.launch", json!({"target": "http://homeassistant.local:8123"}), "Home", "home", HOUSE),
            bind(super::actions::NOW_PLAYING, json!({"turn": "volume"}), "Playing", "music", MEDIA),
        ],
    );
    p.controls.insert("dial".into(), bind(super::actions::MONITOR, json!({"metric": "cpu"}), "CPU", "monitor", "#1f1d26"));
    p.controls.insert("knobs.0".into(), bind("core.volume", json!({"target": "system", "step": 2}), "Volume", "volume-2", "#1f1d26"));
    p.controls.insert("knobs.1".into(), bind("ha.brightness", json!({"entity": "light.desk", "step": 5}), "Desk light", "lightbulb", "#1f1d26"));
    p.controls.insert("knobs.2".into(), bind("core.brightness", json!({"monitor": "all", "step": 5}), "Screen", "sun", "#1f1d26"));
    volume_faders(&mut p, [("system", "", "System"), ("app", "Spotify.exe", "Music"), ("app", "chrome.exe", "Browser"), ("app", "Discord.exe", "Discord"), ("mic", "", "Mic")]);
    p.controls.insert("rollers.0".into(), bind("core.scroll", json!({"axis": "vertical", "amount": 3}), "Scroll", "arrow-up-down", "#1f1d26"));
    p.controls.insert("rollers.1".into(), bind("core.scroll", json!({"axis": "horizontal", "amount": 3}), "Scroll ↔", "arrow-left-right", "#1f1d26"));
    p
}

/// "Claude Code": twelve session keys (blue working, orange asking, green your turn), all
/// sessions on the dial, the rest as on the everyday desk.
pub fn claude_code() -> Profile {
    let screenshot = if cfg!(target_os = "macos") { "Cmd+Shift+4" } else { "PrintScreen" };
    let mut p = everyday_desk();
    p.name = "Claude Code".into();
    for i in 0..12 {
        p.controls.insert(format!("keys.{i}"), bind(super::actions::AGENT, json!({"slot": i + 1}), &format!("Agent {}", i + 1), "bot", "#1d2433"));
    }
    p.controls.insert("keys.12".into(), bind("core.media", json!({"command": "playPause"}), "Play", "play", MEDIA));
    p.controls.insert("keys.13".into(), bind("core.hotkey", json!({"keys": "VolumeMute"}), "Mute", "volume-x", MEDIA));
    p.controls.insert("keys.14".into(), bind("core.hotkey", json!({"keys": screenshot}), "Screenshot", "camera", MEDIA));
    p.controls.insert("dial".into(), bind(super::actions::AGENTS, json!({}), "Sessions", "bot", "#1f1d26"));
    p
}

const MEET: &str = "#2a1f33";
const DANGER: &str = "#3a1f22";
const TOOL: &str = "#1f2a33";
const MARK: &str = "#33301f";

fn hotkey(keys: &str, label: &str, icon: &str, bg: &str) -> Binding {
    bind("core.hotkey", json!({"keys": keys}), label, icon, bg)
}

/// A knob, roller or the dial that sends one combination per direction.
fn turn(cw: &str, ccw: &str, label: &str, icon: &str) -> Binding {
    bind("core.hotkey", json!({"clockwise": cw, "counterClockwise": ccw}), label, icon, "#1f1d26")
}

fn volume_faders(p: &mut Profile, apps: [(&str, &str, &str); 5]) {
    for (i, (target, app, label)) in apps.into_iter().enumerate() {
        p.controls.insert(format!("faders.{i}"), bind("core.volume", json!({"target": target, "app": app}), label, "volume-2", "#1f1d26"));
    }
}

fn put_keys(p: &mut Profile, keys: Vec<Binding>) {
    for (i, b) in keys.into_iter().enumerate().take(15) {
        p.controls.insert(format!("keys.{i}"), b);
    }
}

/// "Discord & meetings": mute, camera, leave and hand-raise for Discord, Teams, Zoom and Google
/// Meet with each app's default shortcuts; their volumes on the faders.
pub fn meetings() -> Profile {
    let mut p = Profile::new("Discord & meetings");
    p.apps = vec!["Discord.exe".into(), "ms-teams.exe".into(), "Teams.exe".into(), "Zoom.exe".into()];
    put_keys(
        &mut p,
        vec![
            hotkey("Ctrl+Shift+M", "Discord mute", "mic-off", MEET),
            hotkey("Ctrl+Shift+D", "Discord deafen", "headphones", MEET),
            hotkey("Ctrl+Shift+M", "Teams mute", "mic-off", MEET),
            hotkey("Ctrl+Shift+O", "Teams camera", "video-off", MEET),
            hotkey("Ctrl+Shift+H", "Teams leave", "x", DANGER),
            hotkey("Alt+A", "Zoom mute", "mic-off", MEET),
            hotkey("Alt+V", "Zoom camera", "video-off", MEET),
            hotkey("Alt+Y", "Zoom hand", "star", MEET),
            hotkey("Alt+Q", "Zoom leave", "x", DANGER),
            hotkey("Ctrl+Shift+K", "Teams hand", "star", MEET),
            hotkey("Ctrl+D", "Meet mic", "mic-off", MEET),
            hotkey("Ctrl+E", "Meet camera", "video-off", MEET),
            bind("core.hotkey", json!({"keys": "VolumeMute"}), "Mute all", "volume-x", DANGER),
            bind("core.media", json!({"command": "playPause"}), "Music", "music", MEDIA),
            bind("core.launch", json!({"target": "https://meet.google.com"}), "New Meet", "video", WEB),
        ],
    );
    volume_faders(&mut p, [("system", "", "System"), ("app", "Discord.exe", "Discord"), ("app", "ms-teams.exe", "Teams"), ("app", "Zoom.exe", "Zoom"), ("app", "chrome.exe", "Chrome")]);
    p.controls.insert("dial".into(), bind("core.volume", json!({"target": "system", "step": 2}), "Volume", "volume-2", "#1f1d26"));
    p
}

/// "Photoshop": tools on the keys, brush size on the dial, zoom and history on the knobs.
/// Adobe's default shortcuts (Windows).
pub fn photoshop() -> Profile {
    let mut p = Profile::new("Photoshop");
    p.apps = vec!["Photoshop.exe".into()];
    put_keys(
        &mut p,
        vec![
            hotkey("V", "Move", "mouse-pointer-click", TOOL),
            hotkey("M", "Marquee", "square", TOOL),
            hotkey("L", "Lasso", "circle-dot", TOOL),
            hotkey("W", "Quick select", "sparkles", TOOL),
            hotkey("C", "Crop", "crop", TOOL),
            hotkey("B", "Brush", "lightbulb", TOOL),
            hotkey("E", "Eraser", "x", TOOL),
            hotkey("S", "Clone", "layers", TOOL),
            hotkey("J", "Heal", "heart", TOOL),
            hotkey("T", "Text", "type", TOOL),
            hotkey("Ctrl+Shift+N", "New layer", "layers", EDIT),
            hotkey("Ctrl+J", "Duplicate", "layers", EDIT),
            hotkey("Ctrl+E", "Merge down", "layers", EDIT),
            hotkey("D", "Default colors", "circle-dot", EDIT),
            hotkey("X", "Swap colors", "refresh-cw", EDIT),
        ],
    );
    p.controls.insert("dial".into(), turn("]", "[", "Brush size", "circle-dot"));
    p.controls.insert("knobs.0".into(), turn("Ctrl+=", "Ctrl+-", "Zoom", "zoom-in"));
    p.controls.insert("knobs.1".into(), turn("Ctrl+Shift+Z", "Ctrl+Z", "History", "refresh-cw"));
    p.controls.insert("knobs.2".into(), turn("Shift+]", "Shift+[", "Hardness", "sun"));
    p.controls.insert("rollers.0".into(), bind("core.scroll", json!({"axis": "vertical", "amount": 3}), "Scroll", "arrow-up-down", "#1f1d26"));
    p.controls.insert("rollers.1".into(), bind("core.scroll", json!({"axis": "horizontal", "amount": 3}), "Scroll ↔", "arrow-left-right", "#1f1d26"));
    p
}

/// "Illustrator": tools and arranging on the keys, zoom on the dial, history, brush size and
/// stacking order on the knobs. Adobe's default shortcuts (Windows).
pub fn illustrator() -> Profile {
    let mut p = Profile::new("Illustrator");
    p.apps = vec!["Illustrator.exe".into()];
    put_keys(
        &mut p,
        vec![
            hotkey("V", "Selection", "mouse-pointer-click", TOOL),
            hotkey("A", "Direct select", "mouse-pointer-click", TOOL),
            hotkey("P", "Pen", "git-branch", TOOL),
            hotkey("T", "Type", "type", TOOL),
            hotkey("M", "Rectangle", "square", TOOL),
            hotkey("L", "Ellipse", "circle-dot", TOOL),
            hotkey("B", "Paintbrush", "lightbulb", TOOL),
            hotkey("Shift+E", "Eraser", "x", TOOL),
            hotkey("I", "Eyedropper", "sparkles", TOOL),
            hotkey("Shift+O", "Artboard", "layers", TOOL),
            hotkey("Ctrl+G", "Group", "layers", EDIT),
            hotkey("Ctrl+Shift+G", "Ungroup", "layers", EDIT),
            hotkey("Ctrl+F", "Paste front", "check", EDIT),
            hotkey("Ctrl+B", "Paste back", "check", EDIT),
            hotkey("Ctrl+Shift+V", "Paste in place", "check", EDIT),
        ],
    );
    p.controls.insert("dial".into(), turn("Ctrl+=", "Ctrl+-", "Zoom", "zoom-in"));
    p.controls.insert("knobs.0".into(), turn("Ctrl+Shift+Z", "Ctrl+Z", "History", "refresh-cw"));
    p.controls.insert("knobs.1".into(), turn("]", "[", "Brush size", "circle-dot"));
    p.controls.insert("knobs.2".into(), turn("Ctrl+]", "Ctrl+[", "Arrange", "layers"));
    p.controls.insert("rollers.0".into(), bind("core.scroll", json!({"axis": "vertical", "amount": 3}), "Scroll", "arrow-up-down", "#1f1d26"));
    p.controls.insert("rollers.1".into(), bind("core.scroll", json!({"axis": "horizontal", "amount": 3}), "Scroll ↔", "arrow-left-right", "#1f1d26"));
    p
}

/// "My games": the games installed on this PC (Steam, Epic, GOG), a key each with its cover from
/// the launcher's cache (stays on this PC), built when the preset is added.
pub fn my_games() -> Profile {
    let mut p = Profile::new("My games");
    let games = super::installed::games();
    for (i, g) in games.iter().take(14).enumerate() {
        let mut k = bind("core.game", json!({"game": g.target}), &g.name, "gamepad-2", FN);
        k.look.image = super::installed::picture(g).ok();
        p.controls.insert(format!("keys.{i}"), k);
    }
    for i in games.len().min(14)..14 {
        p.controls.insert(format!("keys.{i}"), bind("core.launch", json!({"target": "steam://open/games"}), "Steam", "gamepad-2", FN));
    }
    p.controls.insert("keys.14".into(), bind("core.profile", json!({"which": "next"}), "Next profile", "skip-forward", FN));
    p.controls.insert("dial".into(), bind("core.volume", json!({"target": "system", "step": 2}), "Volume", "volume-2", "#1f1d26"));
    p.controls.insert("knobs.0".into(), bind("core.volume", json!({"target": "front", "step": 3}), "Game volume", "volume-2", "#1f1d26"));
    p.controls.insert("rollers.0".into(), bind("core.scroll", json!({"axis": "vertical", "amount": 1}), "Scroll", "arrow-up-down", "#1f1d26"));
    volume_faders(&mut p, [("system", "", "System"), ("front", "", "Game"), ("app", "Discord.exe", "Discord"), ("app", "Spotify.exe", "Spotify"), ("app", "steamwebhelper.exe", "Steam")]);
    p
}

/// Default Windows shortcuts of one video editor for its jog / shuttle panel; sources and what to
/// check in `docs/PRESETS.md` ("Video editing panels").
pub struct Editor {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    /// Executables the profile switches in with.
    pub apps: &'static [&'static str],
    /// The first four keys: (shortcut, label, icon) of its edit tools (select, razor, split, delete).
    pub tools: [(&'static str, &'static str, &'static str); 4],
    /// Play / pause (the dial's press).
    pub play: &'static str,
    /// Shuttle reverse, stop, forward (J K L: each press of J or L plays faster); "" = none, the
    /// dial then jumps with `jump`.
    pub shuttle: [&'static str; 3],
    /// One frame back / forward (roller 1).
    pub frame: [&'static str; 2],
    /// A bigger jump back / forward (knob 1): 5 frames or a second, depending on the editor.
    pub jump: [&'static str; 2],
    /// Timeline zoom out / in (roller 2).
    pub zoom: [&'static str; 2],
    /// Previous / next edit point (knob 2 and two keys).
    pub edit: [&'static str; 2],
    /// Mark in / out.
    pub mark: [&'static str; 2],
    pub marker: &'static str,
    /// Undo / redo (redo on a hold of the undo key).
    pub undo: [&'static str; 2],
    pub save: &'static str,
    /// Export (on a hold of the save key); "" = no default shortcut.
    pub export: &'static str,
}

pub const EDITORS: [Editor; 5] = [
    Editor {
        id: "premiere",
        name: "Premiere Pro",
        description: "Jog and shuttle: the dial shuttles (J/L, faster as you turn faster), a press plays; frames and timeline zoom on the rollers; cut, mark and play on the keys.",
        apps: &["Adobe Premiere Pro.exe"],
        tools: [("V", "Select", "mouse-pointer-click"), ("C", "Razor", "x"), ("Ctrl+K", "Add edit", "git-branch"), ("Shift+Delete", "Ripple delete", "x")],
        play: "Space",
        shuttle: ["J", "K", "L"],
        frame: ["Left", "Right"],
        jump: ["Shift+Left", "Shift+Right"],
        zoom: ["-", "="],
        edit: ["Up", "Down"],
        mark: ["I", "O"],
        marker: "M",
        undo: ["Ctrl+Z", "Ctrl+Shift+Z"],
        save: "Ctrl+S",
        export: "Ctrl+M",
    },
    Editor {
        id: "resolve",
        name: "DaVinci Resolve",
        description: "Jog and shuttle for the Edit page: the dial shuttles (J/L), a press plays; frames and timeline zoom on the rollers; blade, ripple delete, marks on the keys.",
        apps: &["Resolve.exe"],
        tools: [("A", "Select", "mouse-pointer-click"), ("B", "Blade", "x"), ("Ctrl+B", "Split", "git-branch"), ("Delete", "Ripple delete", "x")],
        play: "Space",
        shuttle: ["J", "K", "L"],
        frame: ["Left", "Right"],
        jump: ["Shift+Left", "Shift+Right"],
        zoom: ["Ctrl+-", "Ctrl+="],
        edit: ["Up", "Down"],
        mark: ["I", "O"],
        marker: "M",
        undo: ["Ctrl+Z", "Ctrl+Shift+Z"],
        save: "Ctrl+S",
        export: "",
    },
    Editor {
        id: "vegas",
        name: "VEGAS Pro",
        description: "Jog and shuttle: the dial shuttles (J/L), a press plays or pauses; frames and timeline zoom on the rollers; split, auto ripple, marks on the keys.",
        apps: &["vegas230.exe", "vegas220.exe", "vegas210.exe", "vegas200.exe"],
        tools: [("Ctrl+D", "Edit tool", "mouse-pointer-click"), ("Ctrl+L", "Auto ripple", "refresh-cw"), ("S", "Split", "git-branch"), ("Delete", "Delete", "trash-2")],
        play: "Enter",
        shuttle: ["J", "K", "L"],
        frame: ["Alt+Left", "Alt+Right"],
        jump: ["PageUp", "PageDown"],
        zoom: ["Down", "Up"],
        edit: ["Ctrl+Alt+Left", "Ctrl+Alt+Right"],
        mark: ["I", "O"],
        marker: "M",
        undo: ["Ctrl+Z", "Ctrl+Shift+Z"],
        save: "Ctrl+S",
        export: "",
    },
    Editor {
        id: "capcut",
        name: "CapCut",
        description: "CapCut for Windows: the dial jumps through the video (faster as you turn faster), a press plays; frames and timeline zoom on the rollers; split, marks on the keys.",
        apps: &["CapCut.exe"],
        tools: [("A", "Select", "mouse-pointer-click"), ("B", "Split mode", "x"), ("Ctrl+B", "Split", "git-branch"), ("Delete", "Delete", "trash-2")],
        play: "Space",
        shuttle: ["", "", ""],
        frame: ["Left", "Right"],
        jump: ["Shift+Left", "Shift+Right"],
        zoom: ["Ctrl+-", "Ctrl+="],
        edit: ["Up", "Down"],
        mark: ["I", "O"],
        marker: "M",
        undo: ["Ctrl+Z", "Ctrl+Shift+Z"],
        save: "Ctrl+S",
        export: "Ctrl+E",
    },
    Editor {
        id: "shotcut",
        name: "Shotcut",
        description: "Jog and shuttle: the dial shuttles (J/L), a press plays; frames and timeline zoom on the rollers; split, ripple delete, lift, marks on the keys.",
        apps: &["shotcut.exe"],
        tools: [("S", "Split", "git-branch"), ("Shift+S", "Split all", "layers"), ("X", "Ripple delete", "x"), ("Z", "Lift", "square")],
        play: "Space",
        shuttle: ["J", "K", "L"],
        frame: ["Left", "Right"],
        jump: ["PageUp", "PageDown"],
        zoom: ["-", "="],
        edit: ["Alt+Left", "Alt+Right"],
        mark: ["I", "O"],
        marker: "M",
        undo: ["Ctrl+Z", "Ctrl+Shift+Z"],
        save: "Ctrl+S",
        export: "Ctrl+E",
    },
];

/// A jog / shuttle panel for one editor: the dial shuttles with acceleration (turning faster
/// sends more presses of L or J, so it plays faster) and its press plays or pauses; roller 1 steps
/// frame by frame, roller 2 zooms the timeline; knobs jump and go from edit to edit; the keys cut,
/// mark and play.
pub fn editor_panel(e: &Editor) -> Profile {
    let mut p = Profile::new(e.name);
    p.apps = e.apps.iter().map(|a| a.to_string()).collect();
    let [t0, t1, t2, t3] = e.tools;
    // no shuttle (CapCut): redo on its own key instead of stop
    let (stop, stop_label) = if e.shuttle[1].is_empty() { (e.undo[1], "Redo") } else { (e.shuttle[1], "Stop") };
    let (rev, fwd) = if e.shuttle[0].is_empty() { (e.jump[0], e.jump[1]) } else { (e.shuttle[0], e.shuttle[2]) };
    put_keys(
        &mut p,
        vec![
            hotkey(t0.0, t0.1, t0.2, TOOL),
            hotkey(t1.0, t1.1, t1.2, TOOL),
            hotkey(t2.0, t2.1, t2.2, TOOL),
            hotkey(t3.0, t3.1, t3.2, DANGER),
            hotkey(e.undo[0], "Undo", "undo-2", EDIT),
            hotkey(e.mark[0], "Mark in", "skip-back", MARK),
            hotkey(e.mark[1], "Mark out", "skip-forward", MARK),
            hotkey(e.marker, "Marker", "star", MARK),
            hotkey(e.edit[0], "Prev edit", "skip-back", MARK),
            hotkey(e.edit[1], "Next edit", "skip-forward", MARK),
            hotkey(rev, "Reverse", "skip-back", MEDIA),
            hotkey(stop, stop_label, "pause", MEDIA),
            hotkey(e.play, "Play", "play", MEDIA),
            hotkey(fwd, "Forward", "skip-forward", MEDIA),
            hotkey(e.save, "Save", "save", EDIT),
        ],
    );
    p.controls.insert("keys.4:hold".into(), hotkey(e.undo[1], "Redo", "redo-2", EDIT));
    if !e.export.is_empty() {
        p.controls.insert("keys.14:hold".into(), hotkey(e.export, "Export", "rocket", EDIT));
    }
    let mut dial = bind("core.hotkey", json!({"keys": e.play, "clockwise": fwd, "counterClockwise": rev}), "Shuttle", "play-circle", "#1f1d26");
    dial.accel = true;
    p.controls.insert("dial".into(), dial);
    p.controls.insert("rollers.0".into(), turn(e.frame[1], e.frame[0], "Frame", "arrow-left-right"));
    p.controls.insert("rollers.1".into(), turn(e.zoom[1], e.zoom[0], "Zoom", "zoom-in"));
    p.controls.insert("knobs.0".into(), turn(e.jump[1], e.jump[0], "Jump", "skip-forward"));
    p.controls.insert("knobs.1".into(), turn(e.edit[1], e.edit[0], "Edit points", "arrow-left-right"));
    p.controls.insert("knobs.2".into(), bind("core.volume", json!({"target": "system", "step": 2}), "Volume", "volume-2", "#1f1d26"));
    volume_faders(&mut p, [("system", "", "System"), ("app", e.apps[0], e.name), ("app", "chrome.exe", "Chrome"), ("app", "Spotify.exe", "Spotify"), ("app", "Discord.exe", "Discord")]);
    p
}

fn editor(id: &str) -> Profile {
    editor_panel(EDITORS.iter().find(|e| e.id == id).expect("a listed editor"))
}

pub fn premiere() -> Profile {
    editor("premiere")
}
pub fn resolve() -> Profile {
    editor("resolve")
}
pub fn vegas() -> Profile {
    editor("vegas")
}
pub fn capcut() -> Profile {
    editor("capcut")
}
pub fn shotcut() -> Profile {
    editor("shotcut")
}

/// "Lightroom": culling and rating on the keys, Develop sliders on the knobs (pick a slider, then
/// change it), photos on the rollers. Lightroom Classic defaults.
pub fn lightroom() -> Profile {
    let mut p = Profile::new("Lightroom");
    p.apps = vec!["Lightroom.exe".into()];
    put_keys(
        &mut p,
        vec![
            hotkey("P", "Pick", "check", MARK),
            hotkey("X", "Reject", "x", DANGER),
            hotkey("U", "Unflag", "circle-dot", MARK),
            hotkey("\\", "Before/after", "layers", TOOL),
            hotkey("G", "Grid", "square", TOOL),
            hotkey("1", "1 star", "star", MARK),
            hotkey("2", "2 stars", "star", MARK),
            hotkey("3", "3 stars", "star", MARK),
            hotkey("4", "4 stars", "star", MARK),
            hotkey("5", "5 stars", "star", MARK),
            hotkey("D", "Develop", "sun", TOOL),
            hotkey("E", "Loupe", "search", TOOL),
            hotkey("Ctrl+Z", "Undo", "undo-2", EDIT),
            hotkey("Ctrl+Shift+C", "Copy settings", "layers", EDIT),
            hotkey("Ctrl+Shift+E", "Export", "rocket", EDIT),
        ],
    );
    p.controls.insert("knobs.0".into(), turn(".", ",", "Slider", "list-ordered"));
    p.controls.insert("knobs.1".into(), turn("=", "-", "Adjust", "sun"));
    p.controls.insert("knobs.2".into(), turn("Ctrl+=", "Ctrl+-", "Zoom", "zoom-in"));
    p.controls.insert("dial".into(), turn("=", "-", "Adjust", "sun"));
    p.controls.insert("rollers.0".into(), turn("Right", "Left", "Photos", "arrow-left-right"));
    p.controls.insert("rollers.1".into(), bind("core.scroll", json!({"axis": "vertical", "amount": 3}), "Scroll", "arrow-up-down", "#1f1d26"));
    p
}

/// "Sim panel": the deck as one vJoy joystick (Windows). Keys = buttons 1-15, faders = throttle,
/// mixture, prop, flaps and trim (axes Z, RX, RY, Slider, Dial), knobs = axes X, Y and RZ, the dial
/// fine-tunes the trim (the same axis as fader 5), rollers tap buttons 16-19. vJoy device 1 needs
/// 19 buttons and all eight axes (`docs/SIM-PANEL.md`); the sim binds them.
pub fn sim_panel() -> Profile {
    let mut p = Profile::new("Sim panel");
    // switch in with the sim in front
    p.apps = ["FlightSimulator.exe", "FlightSimulator2024.exe", "DCS.exe", "X-Plane.exe", "iRacingSim64DX11.exe", "StarCitizen.exe"].iter().map(|a| a.to_string()).collect();
    let rows = ["#1f2a33", "#2a1f33", "#33301f"];
    for i in 0..15 {
        let mut k = bind("vjoy.button", json!({"device": 1, "button": i + 1}), &format!("B{}", i + 1), "game:joystick", rows[i / 5]);
        k.look.style = Some("glass".into());
        p.controls.insert(format!("keys.{i}"), k);
    }
    let axis = |axis: &str, step: f64, press: &str, label: &str, icon: &str| bind("vjoy.axis", json!({"device": 1, "axis": axis, "step": step, "press": press}), label, icon, "#1f1d26");
    for (i, (ax, label, icon)) in
        [("z", "Throttle", "game:jet-fighter"), ("rx", "Mixture", "game:water-drop"), ("ry", "Prop", "game:cycle"), ("slider", "Flaps", "game:expand"), ("dial", "Trim", "game:compass")]
            .into_iter()
            .enumerate()
    {
        p.controls.insert(format!("faders.{i}"), axis(ax, 2.0, "zero", label, icon));
    }
    for (i, ax) in ["x", "y", "rz"].into_iter().enumerate() {
        p.controls.insert(format!("knobs.{i}"), axis(ax, 2.0, "center", &format!("Axis {}", ax.to_uppercase()), "gamepad-2"));
    }
    p.controls.insert("dial".into(), axis("dial", 0.5, "center", "Trim", "game:compass"));
    for (i, (up, down)) in [(16, 17), (18, 19)].into_iter().enumerate() {
        let label = format!("B{up} / B{down}");
        p.controls.insert(format!("rollers.{i}"), bind("vjoy.button", json!({"device": 1, "button": 0, "clockwise": up, "counterClockwise": down}), &label, "arrow-up-down", "#1f1d26"));
    }
    p
}

/// "Game action bar": number and ability keys, camera zoom on a knob, item cycle on a roller.
pub fn game_bar() -> Profile {
    let mut p = Profile::new("Game action bar");
    let mut keys: Vec<Binding> = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"].iter().map(|k| hotkey(k, k, "gamepad-2", FN)).collect();
    keys.extend([("Q", "zap"), ("E", "zap"), ("R", "refresh-cw"), ("F", "star"), ("Tab", "list-checks")].iter().map(|(k, i)| hotkey(k, k, i, TOOL)));
    put_keys(&mut p, keys);
    p.controls.insert("knobs.0".into(), bind("core.scroll", json!({"axis": "vertical", "amount": 1}), "Zoom", "zoom-in", "#1f1d26"));
    p.controls.insert("rollers.0".into(), bind("core.scroll", json!({"axis": "vertical", "amount": 1}), "Cycle", "arrow-up-down", "#1f1d26"));
    p.controls.insert("dial".into(), bind("core.volume", json!({"target": "system", "step": 2}), "Volume", "volume-2", "#1f1d26"));
    volume_faders(&mut p, [("system", "", "System"), ("app", "Discord.exe", "Discord"), ("app", "Spotify.exe", "Spotify"), ("app", "chrome.exe", "Chrome"), ("app", "steam.exe", "Steam")]);
    p
}

/// "OBS streaming": scenes, record, stream, replay and mic mute on the keys (they light up with the
/// real state), the OBS mixer on the faders, the mic level on the dial. Scene names are OBS's
/// defaults; rename them in the inspector to match yours.
pub fn obs_streaming() -> Profile {
    let mut p = Profile::new("OBS streaming");
    p.apps = vec!["obs64.exe".into(), "obs.exe".into()];
    let scene = |n: usize| bind("obs.scene", json!({"scene": format!("Scene {n}")}), &format!("Scene {n}"), "monitor", TOOL);
    put_keys(
        &mut p,
        vec![
            scene(1),
            scene(2),
            scene(3),
            scene(4),
            scene(5),
            bind("obs.record", json!({}), "Record", "circle-dot", DANGER),
            bind("obs.stream", json!({}), "Stream", "radio", DANGER),
            bind("obs.replay", json!({}), "Replay", "rocket", MARK),
            bind("obs.mute", json!({"input": "Mic/Aux"}), "Mic", "mic", MEET),
            bind("obs.mute", json!({"input": "Desktop Audio"}), "Desktop", "volume-2", MEET),
            bind("obs.source", json!({"source": "Video Capture Device", "scene": ""}), "Camera", "video", TOOL),
            bind("core.mic", json!({}), "System mic", "mic", MEET),
            bind("core.media", json!({"command": "playPause"}), "Music", "music", MEDIA),
            bind("core.launch", json!({"target": "https://dashboard.twitch.tv"}), "Dashboard", "globe", WEB),
            bind("core.monitor", json!({"metric": "cpu"}), "CPU", "monitor", FN),
        ],
    );
    for (i, (input, label)) in [("Desktop Audio", "Desktop"), ("Mic/Aux", "Mic")].into_iter().enumerate() {
        p.controls.insert(format!("faders.{i}"), bind("obs.volume", json!({"input": input, "step": 1}), label, "volume-2", "#1f1d26"));
    }
    for (i, (target, app, label)) in [("app", "Discord.exe", "Discord"), ("app", "Spotify.exe", "Spotify"), ("system", "", "System")].into_iter().enumerate() {
        p.controls.insert(format!("faders.{}", i + 2), bind("core.volume", json!({"target": target, "app": app}), label, "volume-2", "#1f1d26"));
    }
    p.controls.insert("dial".into(), bind("obs.volume", json!({"input": "Mic/Aux", "step": 1}), "Mic level", "mic", "#1f1d26"));
    p.controls.insert("rollers.0".into(), bind("core.scroll", json!({"axis": "vertical", "amount": 3}), "Scroll", "arrow-up-down", "#1f1d26"));
    p
}

/// Kinds of modules a deck can have.
pub const KINDS: [&str; 5] = ["keys", "dial", "knobs", "faders", "rollers"];

/// Actions whose job is a volume: they may fall back to a pair of keys.
fn is_volume(action: &str) -> bool {
    matches!(action, "core.volume" | "obs.volume" | "streamlabs.volume")
}

/// What a volume binding controls, to put each one on the keys only once.
fn volume_target(b: &Binding) -> String {
    let p = &b.params;
    let s = |k: &str| super::actions::param_str(p, k).trim().to_lowercase();
    format!("{}|{}|{}|{}", b.action, s("target"), s("app").trim_end_matches(".exe"), s("input"))
}

fn as_mode(b: &Binding) -> serde_json::Value {
    let label = b.look.label.clone().unwrap_or_else(|| super::actions::find(&b.action).map(|a| a.name.to_string()).unwrap_or_default());
    json!({"label": label, "action": b.action, "params": b.params})
}

/// Put `b` on a turnable control: alone if it is free, else as one more mode of what is there.
/// Nothing changes when the control already does it (adapting twice adds nothing); returns
/// whether it was added.
fn merge_turn(p: &mut Profile, control: &str, b: &Binding) -> bool {
    let same = |action: &serde_json::Value, params: &serde_json::Value| action.as_str() == Some(b.action.as_str()) && *params == b.params;
    match p.controls.get_mut(control) {
        None => {
            p.controls.insert(control.into(), b.clone());
        }
        Some(existing) if existing.action == b.action && existing.params == b.params => return false,
        Some(existing) if existing.action == super::actions::MODES => {
            let Some(list) = existing.params.get_mut("modes").and_then(|m| m.as_array_mut()) else { return false };
            if list.iter().any(|m| same(&m["action"], &m["params"])) {
                return false;
            }
            list.push(as_mode(b));
        }
        Some(existing) => {
            let modes = vec![as_mode(existing), as_mode(b)];
            existing.params = json!({"modes": modes});
            existing.action = super::actions::MODES.into();
        }
    }
    true
}

/// One function [`adapt_noted`] copied to a control the user has, for the note after an import.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Moved {
    /// What it does: the binding's label, or the action's name.
    pub label: String,
    /// Control it is bound to in the profile (`dial`, `faders.1`...); that binding stays.
    pub from: String,
    /// Controls that do it now (`knobs.0`, or two keys such as `controls/keys.1`, `controls/keys.2`).
    pub to: Vec<String>,
}

fn label_of(b: &Binding) -> String {
    b.look.label.clone().filter(|l| !l.trim().is_empty()).unwrap_or_else(|| super::actions::find(&b.action).map(|a| a.name.to_string()).unwrap_or_else(|| b.action.clone()))
}

/// A key that acts like one detent of a turn (`turnOnPress`): "Volume +" and "Volume -".
fn turn_key(b: &Binding, up: bool) -> Binding {
    let name = b.look.label.clone().unwrap_or_else(|| "Volume".into());
    let sign = if up { "+" } else { "-" };
    let icon = if up { "volume-2" } else { "volume-x" };
    // the system volume as the keyboard's own volume keys: Windows shows its volume popup
    if b.action == "core.volume" && super::actions::param_str(&b.params, "target") == "system" {
        return bind("core.hotkey", json!({"keys": if up { "VolumeUp" } else { "VolumeDown" }}), &format!("{name} {sign}"), icon, MEDIA);
    }
    let mut params = b.params.clone();
    params["turnOnPress"] = json!(if up { 1 } else { -1 });
    bind(&b.action, params, &format!("{name} {sign}"), icon, MEDIA)
}

/// Fit a preset to the modules the user owns (attached or not): every binding stays, so a module
/// attached later works at once, and the important turn functions of the kinds the user does not
/// own are copied to the next kind that is there: faders and rollers -> knobs -> the dial, the dial
/// -> knob 1, knobs -> the dial; a volume with nowhere to turn becomes a pair of keys ("+" and "-"),
/// on free keys or on an extra "Controls" page. Several functions on one knob or the dial become
/// its modes (a press goes to the next).
pub fn adapt(p: Profile, owned: &[String]) -> Profile {
    adapt_noted(p, owned).0
}

/// [`adapt`], also telling what was copied where. Adapting an adapted profile again changes
/// nothing, so it is safe on any profile (imported, made by hand, adapted before).
pub fn adapt_noted(mut p: Profile, owned: &[String]) -> (Profile, Vec<Moved>) {
    let has = |k: &str| owned.iter().any(|o| o == k);
    let mut moved = Vec::new();
    if KINDS.iter().all(|k| has(k)) {
        return (p, moved);
    }
    let turnable = |b: &Binding| super::actions::find(&b.action).is_some_and(|a| a.inputs.contains(&super::actions::InputKind::Turn));
    // functions of kinds that are not there, in order of importance
    let mut lost: Vec<(&str, String, Binding)> = Vec::new();
    for (kind, ids) in [
        ("dial", vec!["dial".to_string()]),
        ("faders", (0..5).map(|i| format!("faders.{i}")).collect()),
        ("knobs", (0..3).map(|i| format!("knobs.{i}")).collect()),
        ("rollers", (0..2).map(|i| format!("rollers.{i}")).collect()),
    ] {
        if has(kind) {
            continue;
        }
        for id in ids {
            if let Some(b) = p.controls.get(&id).filter(|b| turnable(b)) {
                lost.push((kind, id, b.clone()));
            }
        }
    }
    // volumes already on a control the user has need no keys
    let mut on_keys: std::collections::HashSet<String> =
        p.controls.iter().filter(|(id, b)| is_volume(&b.action) && KINDS.iter().any(|k| has(k) && *k != "keys" && id.split('.').next() == Some(k))).map(|(_, b)| volume_target(b)).collect();
    let mut knob_turn = 0usize;
    let mut pairs: Vec<(String, Binding)> = Vec::new();
    for (kind, from, b) in lost {
        let target = match kind {
            "dial" if has("knobs") => Some("knobs.0".to_string()),
            "faders" | "rollers" if has("knobs") => {
                knob_turn += 1;
                Some(format!("knobs.{}", (knob_turn - 1) % 3))
            }
            "faders" | "rollers" | "knobs" if has("dial") => Some("dial".to_string()),
            _ => None,
        };
        match target {
            Some(t) => {
                if is_volume(&b.action) {
                    on_keys.insert(volume_target(&b));
                }
                if merge_turn(&mut p, &t, &b) {
                    moved.push(Moved { label: label_of(&b), from, to: vec![t] });
                }
            }
            None if is_volume(&b.action) && has("keys") && on_keys.insert(volume_target(&b)) => {
                // a "+" key made by an earlier adapt (or by hand) is there already
                let plus = turn_key(&b, true);
                if !p.controls.values().any(|k| k.action == plus.action && k.params == plus.params) {
                    pairs.push((from, b));
                }
            }
            None => {}
        }
    }
    if !pairs.is_empty() {
        let placed = put_pairs(&mut p, pairs.iter().map(|(_, b)| b.clone()).collect());
        for ((from, b), to) in pairs.into_iter().zip(placed) {
            moved.push(Moved { label: label_of(&b), from, to });
        }
    }
    (p, moved)
}

/// "+" and "-" keys for each volume: on free main keys, or on a "Controls" page reached from the
/// last main key (whose own function moves to that page). Returns the two keys of each volume, in
/// order (empty for a volume that found no room).
fn put_pairs(p: &mut Profile, pairs: Vec<Binding>) -> Vec<Vec<String>> {
    let keys: Vec<Binding> = pairs.iter().flat_map(|b| [turn_key(b, true), turn_key(b, false)]).collect();
    let mut placed: Vec<Vec<String>> = vec![Vec::new(); pairs.len()];
    let free: Vec<usize> = (0..15).filter(|i| !p.controls.contains_key(&format!("keys.{i}"))).collect();
    if free.len() >= keys.len() {
        for (n, (slot, b)) in free.into_iter().zip(keys).enumerate() {
            p.controls.insert(format!("keys.{slot}"), b);
            placed[n / 2].push(format!("keys.{slot}"));
        }
        return placed;
    }
    let page = "controls";
    let slots: Vec<usize> = if p.pages.iter().any(|pg| pg.id == page) {
        // made by an earlier adapt: its free keys
        (0..14).filter(|i| !p.controls.contains_key(&format!("{page}/keys.{i}"))).collect()
    } else {
        p.pages.push(super::profile::Page { id: page.into(), name: "Controls".into() });
        let mut slots: Vec<usize> = (0..14).collect();
        if let Some(last) = p.controls.remove("keys.14") {
            p.controls.insert(format!("{page}/keys.0"), last);
            slots.remove(0);
        }
        p.controls.insert("keys.14".into(), bind("core.page", json!({"page": page}), "Controls", "layers", FN));
        p.controls.insert(format!("{page}/keys.14"), bind("core.page", json!({"page": ""}), "Back", "arrow-left", FN));
        slots
    };
    // whole pairs only: "+" without its "-" would be odd
    let room = slots.len() / 2 * 2;
    for (n, (slot, b)) in slots.into_iter().zip(keys.into_iter().take(room)).enumerate() {
        p.controls.insert(format!("{page}/keys.{slot}"), b);
        placed[n / 2].push(format!("{page}/keys.{slot}"));
    }
    placed
}

// ---------------------------------------------------------------------------------------------
// Game presets: the game's default PC key bindings sent "for games" (hardware scan codes held a
// moment), game icons, the game's executable for automatic switching, and the volumes of the game,
// Discord and the music on the dial, knobs and faders. Sources and notes: docs/PRESETS.md.

const COMBAT: &str = "#3a1f22";
const MOVE: &str = "#1f2a33";
const MENU: &str = "#2a1f33";
const ITEM: &str = "#33301f";

/// A key that sends `keys` the way games read them.
pub(super) fn game_key(keys: &str, label: &str, icon: &str, bg: &str) -> Binding {
    let mut b = bind("core.hotkey", json!({"keys": keys, "mode": "game", "hold": 30}), label, icon, bg);
    b.look.style = Some("glass".into());
    b
}

/// A game profile: 15 keys `(keys, label, game icon, color)`, the volumes of the game (`exe`),
/// Discord and the music on the dial, knobs and faders; switches in while one of `apps` is in front.
pub(super) fn game_preset(name: &str, apps: &[&str], exe: &str, keys: &[(&str, &str, &str, &str)]) -> Profile {
    let mut p = Profile::new(name);
    p.apps = apps.iter().map(|a| a.to_string()).collect();
    put_keys(&mut p, keys.iter().map(|(k, label, icon, bg)| game_key(k, label, &format!("game:{icon}"), bg)).collect());
    p.controls.insert("dial".into(), bind("core.volume", json!({"target": "app", "app": exe, "step": 2}), "Game", "volume-2", "#1f1d26"));
    p.controls.insert("knobs.0".into(), bind("core.volume", json!({"target": "app", "app": "Discord.exe", "step": 3}), "Discord", "headphones", "#1f1d26"));
    p.controls.insert("knobs.1".into(), bind("core.volume", json!({"target": "app", "app": "Spotify.exe", "step": 3}), "Music", "music", "#1f1d26"));
    p.controls.insert("knobs.2".into(), bind("core.volume", json!({"target": "system", "step": 2}), "System", "volume-2", "#1f1d26"));
    volume_faders(&mut p, [("system", "", "System"), ("app", exe, "Game"), ("app", "Discord.exe", "Discord"), ("app", "Spotify.exe", "Music"), ("app", "chrome.exe", "Browser")]);
    p.controls.insert("rollers.0".into(), bind("core.scroll", json!({"axis": "vertical", "amount": 1}), "Wheel", "arrow-up-down", "#1f1d26"));
    p
}

pub fn diablo4() -> Profile {
    game_preset(
        "Diablo IV",
        &["Diablo IV.exe"],
        "Diablo IV.exe",
        &[
            ("1", "Skill 1", "crossed-swords", COMBAT),
            ("2", "Skill 2", "fireball", COMBAT),
            ("3", "Skill 3", "lightning-arc", COMBAT),
            ("4", "Skill 4", "magic-swirl", COMBAT),
            ("Q", "Potion", "health-potion", ITEM),
            ("Space", "Evade", "dodge", MOVE),
            ("Z", "Mount", "horse-head", MOVE),
            ("T", "Portal", "magic-portal", MOVE),
            ("E", "Emote", "talk", MENU),
            ("Tab", "Map", "treasure-map", MENU),
            ("I", "Inventory", "backpack", MENU),
            ("A", "Skills", "skills", MENU),
            ("C", "Character", "visored-helm", MENU),
            ("J", "Journal", "scroll-unfurled", MENU),
            ("M", "World map", "compass", MENU),
        ],
    )
}

pub fn baldurs_gate3() -> Profile {
    game_preset(
        "Baldur's Gate 3",
        &["bg3.exe", "bg3_dx11.exe"],
        "bg3.exe",
        &[
            ("Space", "End turn", "hourglass", COMBAT),
            ("Shift+Space", "Turn-based", "sword-clash", COMBAT),
            ("V", "Shove", "punch", COMBAT),
            ("X", "Throw", "thrown-knife", COMBAT),
            ("Z", "Jump", "jump-across", MOVE),
            ("C", "Sneak", "ninja-mask", MOVE),
            ("Alt", "Highlight", "eye-target", MOVE),
            ("I", "Inventory", "backpack", MENU),
            ("N", "Character", "visored-helm", MENU),
            ("K", "Spellbook", "spell-book", MENU),
            ("J", "Journal", "book-cover", MENU),
            ("M", "Map", "treasure-map", MENU),
            ("H", "Alchemy", "standing-potion", ITEM),
            ("F5", "Quicksave", "locked-chest", ITEM),
            ("F8", "Quickload", "open-chest", ITEM),
        ],
    )
}

pub fn counter_strike2() -> Profile {
    game_preset(
        "Counter-Strike 2",
        &["cs2.exe"],
        "cs2.exe",
        &[
            ("1", "Primary", "ak47", COMBAT),
            ("2", "Pistol", "pistol-gun", COMBAT),
            ("3", "Knife", "bowie-knife", COMBAT),
            ("4", "Grenades", "grenade", COMBAT),
            ("5", "Bomb", "dynamite", COMBAT),
            ("R", "Reload", "bullets", ITEM),
            ("F", "Inspect", "eye-target", ITEM),
            ("G", "Drop", "knapsack", ITEM),
            ("E", "Use", "hand", ITEM),
            ("B", "Buy", "two-coins", MENU),
            ("Tab", "Scores", "laurels-trophy", MENU),
            ("Z", "Radio", "radio-tower", MENU),
            ("U", "Team chat", "chat-bubble", MENU),
            ("Y", "All chat", "talk", MENU),
            ("T", "Spray", "water-drop", MENU),
        ],
    )
}

pub fn valorant() -> Profile {
    game_preset(
        "Valorant",
        &["VALORANT-Win64-Shipping.exe"],
        "VALORANT-Win64-Shipping.exe",
        &[
            ("C", "Ability C", "magic-swirl", COMBAT),
            ("Q", "Ability Q", "fireball", COMBAT),
            ("E", "Ability E", "lightning-arc", COMBAT),
            ("X", "Ultimate", "power-lightning", COMBAT),
            ("1", "Primary", "rifle", COMBAT),
            ("2", "Pistol", "revolver", COMBAT),
            ("3", "Knife", "thrown-knife", COMBAT),
            ("4", "Spike", "dynamite", COMBAT),
            ("R", "Reload", "bullets", ITEM),
            ("F", "Use", "hand", ITEM),
            ("Z", "Ping", "targeted", MENU),
            ("M", "Map", "treasure-map", MENU),
            ("Tab", "Scores", "laurels-trophy", MENU),
            ("Enter", "Chat", "chat-bubble", MENU),
            ("Space", "Jump", "jump-across", MOVE),
        ],
    )
}

pub fn fortnite() -> Profile {
    game_preset(
        "Fortnite",
        &["FortniteClient-Win64-Shipping.exe"],
        "FortniteClient-Win64-Shipping.exe",
        &[
            ("F1", "Wall", "brick-wall", MOVE),
            ("F2", "Floor", "stone-block", MOVE),
            ("F3", "Stairs", "ladder", MOVE),
            ("F4", "Roof", "tower-flag", MOVE),
            ("F5", "Trap", "trap-mask", MOVE),
            ("G", "Edit", "hammer-nails", MOVE),
            ("1", "Slot 1", "ak47", COMBAT),
            ("2", "Slot 2", "pistol-gun", COMBAT),
            ("3", "Slot 3", "winchester-rifle", COMBAT),
            ("4", "Slot 4", "grenade", COMBAT),
            ("5", "Slot 5", "health-potion", COMBAT),
            ("M", "Map", "treasure-map", MENU),
            ("Tab", "Inventory", "backpack", MENU),
            ("B", "Emote", "talk", MENU),
            ("H", "Repair", "anvil", ITEM),
        ],
    )
}

pub fn minecraft() -> Profile {
    game_preset(
        "Minecraft",
        // Java Edition runs in javaw.exe; the window title tells Minecraft from other Java apps
        &["javaw.exe: Minecraft", "Minecraft.Windows.exe"],
        "javaw.exe",
        &[
            ("1", "Slot 1", "mining", ITEM),
            ("2", "Slot 2", "broadsword", ITEM),
            ("3", "Slot 3", "bow-arrow", ITEM),
            ("4", "Slot 4", "bread", ITEM),
            ("E", "Inventory", "backpack", MENU),
            ("Q", "Drop", "swap-bag", ITEM),
            ("F", "Swap hands", "cycle", ITEM),
            ("T", "Chat", "chat-bubble", MENU),
            ("Tab", "Players", "village", MENU),
            ("L", "Advancements", "trophy", MENU),
            ("F5", "Camera", "eye-target", MOVE),
            ("F2", "Screenshot", "photo-camera", MOVE),
            ("F3", "Debug", "cog", MOVE),
            ("F11", "Fullscreen", "expand", MOVE),
            ("Space", "Jump", "jump-across", MOVE),
        ],
    )
}

pub fn league_of_legends() -> Profile {
    game_preset(
        "League of Legends",
        &["League of Legends.exe"],
        "League of Legends.exe",
        &[
            ("Q", "Q", "fireball", COMBAT),
            ("W", "W", "magic-shield", COMBAT),
            ("E", "E", "lightning-arc", COMBAT),
            ("R", "Ultimate", "power-lightning", COMBAT),
            ("D", "Summoner D", "teleport", COMBAT),
            ("F", "Summoner F", "healing", COMBAT),
            ("1", "Item 1", "health-potion", ITEM),
            ("2", "Item 2", "magic-potion", ITEM),
            ("3", "Item 3", "gem-pendant", ITEM),
            ("4", "Trinket", "eye-target", ITEM),
            ("B", "Recall", "magic-portal", MOVE),
            ("P", "Shop", "two-coins", MENU),
            ("Tab", "Scores", "laurels-trophy", MENU),
            ("Y", "Camera lock", "padlock", MOVE),
            ("Space", "Center", "targeted", MOVE),
        ],
    )
}

/// The group a preset is listed in (the "+" menu shows one line per group, owner's pick 1A):
/// `start`, `streaming`, `photo`, `video`, `games`, `tools`, `showcase`, `test`.
pub fn group(id: &str) -> &'static str {
    match id {
        "demo" | "everyday" => "start",
        "obs" | "meetings" | "showcase-stream" => "streaming",
        "photoshop" | "illustrator" | "lightroom" => "photo",
        "premiere" | "resolve" | "vegas" | "capcut" | "shotcut" => "video",
        "games" | "game" | "sim" => "games",
        "claude" | "status" => "tools",
        id if id.starts_with("showcase") => "showcase",
        id if id.starts_with("test-") => "test",
        id if games().iter().any(|g| g.id == id) => "games",
        _ => "tools",
    }
}

/// The game presets, listed after the others.
pub fn games() -> Vec<PresetInfo> {
    let game = |id: &'static str, name: &'static str, description: &'static str, build: fn() -> Profile| PresetInfo { id, name, description, build, test: false };
    vec![
        game("diablo4", "Diablo IV", "Skills 1-4, potion, evade, mount, portal, map, inventory and panels; game, Discord and music volumes.", diablo4),
        game("bg3", "Baldur's Gate 3", "End turn, turn-based mode, shove, throw, jump, sneak, the panels, quicksave and quickload.", baldurs_gate3),
        game("cs2", "Counter-Strike 2", "Weapon slots, reload, inspect, drop, use, buy menu, scores, radio and chats.", counter_strike2),
        game("valorant", "Valorant", "Abilities and ultimate, weapon slots, spike, reload, use, ping, map and scores.", valorant),
        game("fortnite", "Fortnite", "Building pieces (F1-F5), edit, item slots 1-5, map, inventory, emote and repair.", fortnite),
        game("minecraft", "Minecraft", "Hotbar slots, inventory, drop, swap hands, chat, players, advancements, camera, screenshot, debug.", minecraft),
        game("lol", "League of Legends", "Q W E R, summoner spells, items and trinket, recall, shop, scores, camera lock and center.", league_of_legends),
    ]
}

pub struct PresetInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub build: fn() -> Profile,
    /// A test profile for trying features on the hardware (its own group in the "+" menu).
    pub test: bool,
}

impl PresetInfo {
    /// The profile as the user gets it: built, in the house look (showcase and test profiles keep
    /// their own art).
    pub fn make(&self) -> Profile {
        let mut p = (self.build)();
        // the showcase and the demo bring their own looks
        if !self.test && !self.id.starts_with("showcase") && self.id != "demo" {
            super::house::apply(&mut p);
        }
        // names and labels in the window's language (test profiles stay English)
        if self.test { p } else { super::localize::profile(p) }
    }
}

/// The catalog entry of a video editor's panel.
fn editor_info(id: &'static str, build: fn() -> Profile) -> PresetInfo {
    let e = EDITORS.iter().find(|e| e.id == id).expect("a listed editor");
    PresetInfo { id, name: e.name, description: e.description, build, test: false }
}

/// Every preset, in the order the catalog (`docs/PRESETS.md`) ranks them.
pub fn catalog() -> Vec<PresetInfo> {
    vec![
        PresetInfo {
            id: "demo",
            name: "Discover HubDeck",
            description: "A tour on the deck: seven tabs of what HubDeck can do (the bottom-right key goes on), the dial as a gallery of its looks; press a key and the window says what it is and how to set it.",
            build: super::demo::discover,
            test: false,
        },
        PresetInfo { id: "everyday", name: "Everyday desk", description: "Media, copy and paste, app volumes on the faders, scrolling on the rollers.", build: everyday_desk, test: false },
        PresetInfo {
            id: "obs",
            name: "OBS streaming",
            description: "Scenes, record, stream, replay and mic mute that light up with the real state; the OBS mixer on the faders.",
            build: obs_streaming,
            test: false,
        },
        PresetInfo { id: "meetings", name: "Discord & meetings", description: "Mute, camera, leave and raise hand for Discord, Teams, Zoom and Google Meet.", build: meetings, test: false },
        PresetInfo { id: "photoshop", name: "Photoshop", description: "Tools on the keys, brush size on the dial, zoom and history on the knobs.", build: photoshop, test: false },
        PresetInfo {
            id: "illustrator",
            name: "Illustrator",
            description: "Tools, group and paste in front / back on the keys, zoom on the dial, history and arranging on the knobs.",
            build: illustrator,
            test: false,
        },
        PresetInfo { id: "lightroom", name: "Lightroom", description: "Pick, reject and stars on the keys; Develop sliders on the knobs; photos on a roller.", build: lightroom, test: false },
        editor_info("premiere", premiere),
        editor_info("resolve", resolve),
        editor_info("vegas", vegas),
        editor_info("capcut", capcut),
        editor_info("shotcut", shotcut),
        PresetInfo {
            id: "claude",
            name: "Claude Code",
            description: "A key per Claude Code session showing whether it works, asks or waits for you; all sessions on the dial.",
            build: claude_code,
            test: false,
        },
        PresetInfo { id: "game", name: "Game action bar", description: "Number and ability keys, zoom and item cycle, game chat and music volumes.", build: game_bar, test: false },
        PresetInfo {
            id: "sim",
            name: "Sim panel",
            description: "The deck as a joystick for flight and racing sims (Windows, needs vJoy): 15 buttons, throttle, mixture, prop, flaps and trim on the faders, axes on the knobs. Bind them in your sim's control settings.",
            build: sim_panel,
            test: false,
        },
        PresetInfo {
            id: "status",
            name: "Status board",
            description: "Time, weather, your next meeting, computer load, a website, a build, home sensors and a price on the keys, refreshing by themselves and turning orange or red past their limits. Set up your place, calendar and addresses on the keys that ask for it.",
            build: status_board,
            test: false,
        },
        PresetInfo { id: "games", name: "My games", description: "The games installed on this PC with their covers: a key starts one.", build: my_games, test: false },
        PresetInfo {
            id: "showcase-stream",
            name: "Stream live",
            description: "Scene cards, going live and recording in red, markers and clips, the stream mixer on the faders.",
            build: super::showcase::stream_live,
            test: false,
        },
        PresetInfo {
            id: "showcase-focus",
            name: "Night focus",
            description: "A night sky across the keys, a 25-minute focus timer on the dial, lo-fi music, a calm breathing edge.",
            build: super::showcase::night_focus,
            test: false,
        },
        PresetInfo {
            id: "showcase-spotify",
            name: "Spotify studio",
            description: "The track and its cover on the dial, like and save, an animated equalizer, Spotify green throughout.",
            build: super::showcase::spotify_studio,
            test: false,
        },
        PresetInfo {
            id: "test-keys",
            name: "Test: keys & gestures",
            description: "Tap, hold, double press, multi-action, toggle, pages, mute and push-to-talk, timer, clock, typing.",
            build: super::test_presets::keys_gestures,
            test: true,
        },
        PresetInfo {
            id: "test-looks",
            name: "Test: looks",
            description: "Own pictures and animated GIFs on keys and the dial, colors, edge and LED colors.",
            build: super::test_presets::looks,
            test: true,
        },
        PresetInfo {
            id: "test-turning",
            name: "Test: knobs, dial, rollers",
            description: "Dial modes, fast turning, app volume, deck and monitor brightness, profiles by turning.",
            build: super::test_presets::turning,
            test: true,
        },
        PresetInfo {
            id: "test-integrations",
            name: "Test: integrations",
            description: "Now playing, system monitor, OBS, Streamlabs, Spotify, audio devices, Home Assistant, Twitch.",
            build: super::test_presets::integrations,
            test: true,
        },
        PresetInfo {
            id: "test-real-apps",
            name: "Test: real apps",
            description: "Apps found on this PC as keys with their real icons, animated art on the dial and keys.",
            build: super::test_presets::real_apps,
            test: true,
        },
        PresetInfo { id: "test-aurora", name: "Test: aurora", description: "An animated aurora across all 15 keys, fire on the dial, LED effects.", build: super::test_presets::aurora, test: true },
        PresetInfo { id: "test-panel", name: "Test: full panel", description: "One picture across all 15 keys; the keys still act.", build: super::test_presets::panel, test: true },
    ]
    .into_iter()
    .chain(games())
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{actions, render};

    #[test]
    fn game_presets_use_real_icons_and_game_keys() {
        let named = |k: &str| {
            k.len() == 1 && k.chars().all(|c| c.is_ascii_alphanumeric())
                || ["Space", "Tab", "Enter", "Shift", "Alt", "Ctrl"].contains(&k)
                || (k.starts_with('F') && k[1..].parse::<u8>().is_ok_and(|n| (1..=24).contains(&n)))
        };
        for info in games() {
            let p = (info.build)();
            assert!(!p.apps.is_empty(), "{}", info.id);
            for i in 0..15 {
                let b = &p.controls[&format!("keys.{i}")];
                assert_eq!(b.params["mode"], "game", "{} key {i}", info.id);
                let keys = b.params["keys"].as_str().unwrap();
                assert!(keys.split('+').all(named), "{} key {i}: {keys} cannot be sent to games", info.id);
                let icon = b.look.icon.as_deref().unwrap();
                assert!(render::icon_svg(icon).is_some(), "{} key {i}: no icon {icon}", info.id);
            }
            assert_eq!(p.controls["dial"].action, "core.volume");
        }
        assert_eq!(catalog().iter().filter(|p| ["diablo4", "lol"].contains(&p.id)).count(), 2);
    }

    #[test]
    fn every_preset_has_a_group_that_fits() {
        for info in catalog() {
            let g = group(info.id);
            assert_eq!(g == "test", info.test, "{}", info.id);
        }
        assert_eq!(group("diablo4"), "games");
        assert_eq!(group("resolve"), "video");
        assert_eq!(group("showcase-focus"), "showcase");
        assert_eq!(group("showcase-stream"), "streaming");
    }

    #[test]
    fn every_icon_a_preset_uses_exists() {
        // a name that is not in the set draws the key without an icon
        for info in catalog() {
            let p = (info.build)();
            for (control, b) in &p.controls {
                for look in std::iter::once(&b.look).chain(b.look_on.iter()) {
                    if let Some(icon) = look.icon.as_deref() {
                        assert!(render::icon_svg(icon).is_some(), "{} {control}: no icon {icon}", info.id);
                    }
                }
            }
        }
    }

    /// Writes `app/src/demo/fixture.json`: the real action catalog, icons, presets as profiles and
    /// the engine's own pictures of their keys and dial, for the browser demo (`?demo`). Run after
    /// changing actions, icons, presets or drawing:
    /// `cargo test -p hubdeck dump_demo_fixture -- --ignored`.
    #[test]
    #[ignore]
    fn dump_demo_fixture() {
        use super::super::commands;
        let mut profiles = Vec::new();
        let mut previews = serde_json::Map::new();
        for (i, preset) in catalog().iter().enumerate() {
            let mut p = (preset.build)();
            p.id = format!("demo-{}", preset.id);
            if i == 0 {
                p.edge = Some("#8b5cf6".into());
            }
            for (control, b) in &p.controls {
                let svg = if control == "dial" {
                    render::dial_svg(&b.look, None, 0)
                } else if control.contains("keys.") {
                    render::key_svg(&b.look, 0)
                } else {
                    continue;
                };
                let screen = if control == "dial" { "dial" } else { "key" };
                // keys sorted (a serde_json map), like the profiles in the file; the demo sorts too
                previews.insert(format!("{screen}|{}", serde_json::to_value(&b.look).unwrap()), svg.into());
            }
            profiles.push(p);
        }
        let fixture = serde_json::json!({
            "catalog": commands::actions_list(),
            "icons": commands::icons_list(),
            "presets": commands::presets_list(),
            "profiles": profiles,
            "previews": previews,
        });
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/demo/fixture.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, serde_json::to_string(&fixture).unwrap()).unwrap();
        println!("wrote {} ({} KB)", path.display(), std::fs::metadata(&path).unwrap().len() / 1024);
    }

    fn kinds(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn full_kit_keeps_presets_as_they_are() {
        for preset in catalog() {
            // one build only: whether sample pictures get stored depends on other tests' timing
            let fresh = (preset.build)();
            let p = adapt(fresh.clone(), &kinds(&KINDS));
            assert_eq!(p, fresh, "{}", preset.id);
        }
    }

    #[test]
    fn basic_kit_puts_dial_knob_and_roller_jobs_on_keys_or_nowhere() {
        // keys and faders: the dial's system volume is already on a fader, Spotify too
        let p = adapt(everyday_desk(), &kinds(&["keys", "faders"]));
        assert!(p.pages.is_empty(), "no volume is missing: every one is on a fader");
        assert_eq!(p.controls, everyday_desk().controls, "nothing lost, nothing added");
        // OBS: the dial's mic level is on fader 2 already
        let obs = adapt(obs_streaming(), &kinds(&["keys", "faders"]));
        assert!(obs.pages.is_empty());
        // Photoshop has no faders: brush size and zoom have nowhere to turn, they are not volumes
        let ps = adapt(photoshop(), &kinds(&["keys", "faders"]));
        assert_eq!(ps.controls, photoshop().controls);
    }

    #[test]
    fn keys_only_get_volume_pairs_on_a_controls_page() {
        let p = adapt(everyday_desk(), &kinds(&["keys"]));
        assert_eq!(p.pages.iter().map(|pg| pg.id.as_str()).collect::<Vec<_>>(), ["controls"]);
        assert_eq!(p.controls["keys.14"].action, "core.page");
        assert_eq!(p.controls["controls/keys.0"], everyday_desk().controls["keys.14"], "the last key moved to the page");
        assert_eq!(p.controls["controls/keys.14"].params["page"], "");
        assert_eq!(p.controls["controls/keys.1"].params["keys"], "VolumeUp", "system volume as the volume keys");
        assert_eq!(p.controls["controls/keys.2"].params["keys"], "VolumeDown");
        // system (dial and fader 1, once), Spotify (knob and fader, once), Chrome, Discord, Firefox
        let pairs = (1..14).filter(|i| p.controls.contains_key(&format!("controls/keys.{i}"))).count();
        assert_eq!(pairs, 10);
        let chrome = p.controls.values().find(|b| b.look.label.as_deref() == Some("Chrome +")).unwrap();
        assert_eq!((chrome.action.as_str(), chrome.params["turnOnPress"].as_i64()), ("core.volume", Some(1)));
        assert!(p.controls.contains_key("faders.0") && p.controls.contains_key("dial"), "the other kinds keep their bindings");
    }

    #[test]
    fn knobs_take_the_dial_and_the_faders_as_modes() {
        let p = adapt(everyday_desk(), &kinds(&["keys", "knobs"]));
        let knob0 = &p.controls["knobs.0"];
        assert_eq!(knob0.action, actions::MODES);
        let labels: Vec<&str> = knob0.params["modes"].as_array().unwrap().iter().map(|m| m["label"].as_str().unwrap()).collect();
        assert_eq!(&labels[..3], ["Spotify", "Volume", "System"], "own job first, then the dial, then fader 1");
        assert!(p.pages.is_empty(), "every volume found a knob");
        let dial_only = adapt(photoshop(), &kinds(&["keys", "dial"]));
        let modes = dial_only.controls["dial"].params["modes"].as_array().unwrap().len();
        assert_eq!(modes, 1 + 3 + 2, "the dial's brush size, three knobs, two rollers");
    }

    #[test]
    fn adapting_notes_what_moved_and_twice_changes_nothing() {
        let (p, moved) = adapt_noted(everyday_desk(), &kinds(&["keys", "knobs"]));
        let dial = moved.iter().find(|m| m.from == "dial").expect("the dial's job is noted");
        assert_eq!(dial.to, ["knobs.0"]);
        assert_eq!(dial.label, everyday_desk().controls["dial"].look.label.clone().unwrap());
        assert!(moved.iter().all(|m| !m.to.is_empty() && !m.from.starts_with("keys")));
        assert_eq!(p.controls["dial"], everyday_desk().controls["dial"], "the original binding stays");
        for owned in [&["keys", "knobs"][..], &["keys"], &["keys", "dial"], &["keys", "faders"]] {
            let (once, _) = adapt_noted(everyday_desk(), &kinds(owned));
            let (twice, again) = adapt_noted(once.clone(), &kinds(owned));
            assert_eq!(twice, once, "{owned:?}");
            assert!(again.is_empty(), "{owned:?}: nothing new to note");
        }
        let (_, keys_only) = adapt_noted(everyday_desk(), &kinds(&["keys"]));
        let pair = keys_only.iter().find(|m| m.to.len() == 2).expect("a volume as a pair of keys");
        assert!(pair.to.iter().all(|k| k.starts_with("controls/keys.")));
        assert!(adapt_noted(everyday_desk(), &kinds(&KINDS)).1.is_empty());
    }

    #[test]
    fn video_panels_shuttle_on_the_dial_and_step_on_the_rollers() {
        for e in &EDITORS {
            let p = editor_panel(e);
            let dial = &p.controls["dial"];
            assert!(dial.accel, "{}: the shuttle speeds up with fast turns", e.name);
            assert_eq!(dial.params["keys"], e.play, "{}: a press plays", e.name);
            let cw = dial.params["clockwise"].as_str().unwrap();
            assert_eq!(cw, if e.shuttle[2].is_empty() { e.jump[1] } else { e.shuttle[2] });
            assert_eq!(p.controls["rollers.0"].params["clockwise"], e.frame[1], "{}: frame by frame", e.name);
            assert_eq!(p.controls["rollers.1"].params["clockwise"], e.zoom[1], "{}: zoom in", e.name);
            assert!(p.controls.contains_key("keys.4:hold"), "{}: redo on a hold of undo", e.name);
            // "Profile for an app" picks the editor's own panel
            for app in e.apps {
                assert_eq!(super::super::starter::preset_for(app).map(|p| p.name), Some(e.name.to_string()), "{app}");
            }
        }
        assert_eq!(catalog().iter().filter(|p| EDITORS.iter().any(|e| e.id == p.id)).count(), EDITORS.len());
    }

    #[test]
    fn sim_panel_is_one_joystick() {
        let p = sim_panel();
        let buttons: Vec<i64> = (0..15).map(|i| p.controls[&format!("keys.{i}")].params["button"].as_i64().unwrap()).collect();
        assert_eq!(buttons, (1..=15).collect::<Vec<_>>());
        assert!(p.controls.values().all(|b| b.action.starts_with("vjoy.") && b.params["device"] == 1));
        let axes: std::collections::HashSet<&str> = p.controls.values().filter(|b| b.action == "vjoy.axis").map(|b| b.params["axis"].as_str().unwrap()).collect();
        assert_eq!(axes.len(), crate::engine::vjoy::AXES.len(), "every axis used once (the trim on a fader and the dial)");
        assert!(axes.iter().all(|a| crate::engine::vjoy::AXES.iter().any(|x| x.0 == *a)));
        assert!(p.controls.values().all(|b| actions::while_held(&b.action, &b.params) == (b.action == "vjoy.button")));
    }

    #[test]
    fn presets_use_only_known_actions_icons_and_keys() {
        let mut ids = std::collections::HashSet::new();
        for preset in catalog() {
            assert!(ids.insert(preset.id), "unique id {}", preset.id);
            let p = (preset.build)();
            assert_eq!(p.name, preset.name);
            assert_eq!(p.controls.keys().filter(|k| k.starts_with("keys.") && !k.contains(':')).count(), 15, "{}", p.name);
            for (control, b) in &p.controls {
                // an empty action only shows something (the demo's Looks tab)
                assert!(b.action.is_empty() || actions::find(&b.action).is_some(), "{}/{control}: unknown action {}", p.name, b.action);
                if let Some(icon) = b.look.icon.as_deref() {
                    assert!(render::icon_svg(icon).is_some(), "{}/{control}: unknown icon {icon}", p.name);
                } else {
                    assert!(b.look.image.is_some() || b.look.label.is_some() || b.action.is_empty(), "{}/{control}: shows nothing", p.name);
                }
                for key in ["keys", "clockwise", "counterClockwise"] {
                    if let Some(combo) = b.params.get(key).and_then(|v| v.as_str()).filter(|c| !c.is_empty()) {
                        assert!(crate::engine::hotkey::parse(combo).is_some(), "{}/{control}: hotkey {combo:?} does not parse", p.name);
                    }
                }
            }
        }
    }
}
