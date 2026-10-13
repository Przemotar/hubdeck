//! Built-in actions and their parameter schemas. The settings window builds each action's form
//! from these definitions; plugins will describe their actions the same way.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum InputKind {
    /// Keys, and the push of knobs and the dial.
    Press,
    /// Knobs, rollers and the dial (relative steps).
    Turn,
    /// Faders (absolute 0..=100).
    Slide,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ParamKind {
    Text {
        multiline: bool,
    },
    /// A file, folder, app or URL to open.
    Target,
    /// A key combination like `Ctrl+Shift+F13`.
    Hotkey,
    Select {
        options: Vec<(String, String)>,
    },
    Number {
        min: f64,
        max: f64,
        step: f64,
    },
    /// One of the user's profiles.
    Profile,
    /// An executable name, e.g. `spotify.exe`.
    App,
    /// A key page of the current profile; empty = the main page.
    Page,
    /// A list of `{action, params, delay}` steps (multi-action).
    Steps,
    /// A list of `{label, action, params}` modes (dial modes).
    Modes,
    /// A MIDI output port by name.
    MidiPort,
    /// An audio device by name (outputs and inputs are offered).
    AudioDevice,
    /// A game installed on this PC (Steam, Epic, GOG): its launch link.
    Game,
    /// A token kept in the system's credential store; the param holds only its name.
    Secret,
    /// A place for the weather, `Name|lat|lon`, picked with a city search.
    Place,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParamDef {
    pub key: &'static str,
    pub label: &'static str,
    #[serde(flatten)]
    pub kind: ParamKind,
    pub default: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<&'static str>,
    /// Show only when another param has one of these values, e.g. `("target", ["app"])`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub when: Option<(&'static str, Vec<&'static str>)>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionDef {
    pub id: &'static str,
    pub name: &'static str,
    pub group: &'static str,
    pub description: &'static str,
    pub inputs: Vec<InputKind>,
    pub params: Vec<ParamDef>,
    /// Suggested icon for a new key.
    pub icon: &'static str,
}

fn p(key: &'static str, label: &'static str, kind: ParamKind, default: Value) -> ParamDef {
    ParamDef { key, label, kind, default, help: None, when: None }
}

fn options(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs.iter().map(|(v, l)| (v.to_string(), l.to_string())).collect()
}

pub fn catalog() -> Vec<ActionDef> {
    use InputKind::*;
    vec![
        ActionDef {
            id: "core.launch",
            name: "Open app, file or website",
            group: "System",
            description: "Opens a program, a document, a folder or a URL.",
            inputs: vec![Press],
            params: vec![ParamDef { help: Some("A path like C:\\Tools\\app.exe, a folder, or a URL like https://example.com"), ..p("target", "What to open", ParamKind::Target, json!("")) }],
            icon: "app-window",
        },
        ActionDef {
            id: "core.game",
            name: "Open game",
            group: "System",
            description: "Starts a game installed on this PC through its launcher (Steam, Epic, GOG). Picking one puts its cover on the key.",
            inputs: vec![Press],
            params: vec![ParamDef { help: Some("Pick from the games found on this PC, or paste a launcher link like steam://rungameid/730."), ..p("game", "Game", ParamKind::Game, json!("")) }],
            icon: "gamepad-2",
        },
        ActionDef {
            id: "core.hotkey",
            name: "Hotkey",
            group: "System",
            description: "Sends a key combination. On a knob, roller or the dial: one combination per direction.",
            inputs: vec![Press, Turn],
            params: vec![
                ParamDef { help: Some("Press: on keys and when a knob or the dial is pushed."), ..p("keys", "Keys", ParamKind::Hotkey, json!("")) },
                p("clockwise", "Turn clockwise / up", ParamKind::Hotkey, json!("")),
                p("counterClockwise", "Turn counter-clockwise / down", ParamKind::Hotkey, json!("")),
                ParamDef {
                    help: Some(
                        "For games: hardware scan codes, held a moment, so games that read the keyboard directly see the press. A game running as administrator needs HubDeck run as administrator too.",
                    ),
                    ..p("mode", "Send as", ParamKind::Select { options: options(&[("normal", "Normal keys"), ("game", "For games")]) }, json!("normal"))
                },
                ParamDef { when: Some(("mode", vec!["game"])), ..p("hold", "Hold (ms)", ParamKind::Number { min: 1.0, max: 500.0, step: 5.0 }, json!(30)) },
            ],
            icon: "keyboard",
        },
        ActionDef {
            id: "core.media",
            name: "Media control",
            group: "Media",
            description: "Play/pause, next and previous track for whatever is playing.",
            inputs: vec![Press, Turn],
            params: vec![p(
                "command",
                "Command",
                ParamKind::Select { options: options(&[("playPause", "Play / pause"), ("next", "Next track"), ("previous", "Previous track"), ("stop", "Stop")]) },
                json!("playPause"),
            )],
            icon: "play",
        },
        ActionDef {
            id: "core.volume",
            name: "Volume",
            group: "Audio",
            description: "System volume or one app's volume. Faders set it, knobs and the dial turn it, a press mutes.",
            inputs: vec![Press, Turn, Slide],
            params: vec![
                p(
                    "target",
                    "Volume of",
                    ParamKind::Select {
                        options: options(&[
                            ("system", "The whole system"),
                            ("app", "One app"),
                            ("front", "The app in front"),
                            ("others", "Every other app (not on another control)"),
                            ("mic", "The microphone (input level)"),
                        ]),
                    },
                    json!("system"),
                ),
                ParamDef {
                    help: Some(
                        "The program, e.g. spotify.exe, chrome.exe, Discord.exe (on a Mac the app's name works too: Spotify, Google Chrome). macOS: 14.2 or later, asks once for System Audio Recording.",
                    ),
                    when: Some(("target", vec!["app"])),
                    ..p("app", "App", ParamKind::App, json!(""))
                },
                p("step", "Step per detent (%)", ParamKind::Number { min: 1.0, max: 20.0, step: 1.0 }, json!(2)),
                ParamDef {
                    help: Some("The volume window Windows shows for the volume keys of a keyboard."),
                    when: Some(("target", vec!["system"])),
                    ..p("popup", "Windows volume popup", ParamKind::Select { options: options(&[("show", "Show"), ("hide", "Do not show")]) }, json!("show"))
                },
            ],
            icon: "volume-2",
        },
        ActionDef {
            id: "core.seek",
            name: "Seek in a player",
            group: "Media",
            description: "Jumps forward or back in a video or song with the shortcut of the player you pick. Knobs, rollers and the dial: turn to seek.",
            inputs: vec![Press, Turn],
            params: vec![
                p("player", "Player", ParamKind::Select { options: options(&super::seek::PLAYERS.iter().map(|(id, name, _, _)| (*id, *name)).collect::<Vec<_>>()) }, json!("vlc")),
                p("press", "A press goes", ParamKind::Select { options: options(&[("forward", "Forward"), ("back", "Back")]) }, json!("forward")),
            ],
            icon: "skip-forward",
        },
        ActionDef {
            id: "core.sound",
            name: "Play sound",
            group: "Audio",
            description: "Plays a sound file (WAV, MP3, FLAC, OGG): a small soundboard. A press on a playing sound stops it, starts it over, or adds another copy; or the key stops every sound.",
            inputs: vec![Press],
            params: vec![
                p(
                    "mode",
                    "A press",
                    ParamKind::Select { options: options(&[("playStop", "Plays / stops"), ("restart", "Starts over"), ("overlap", "Plays one more"), ("stopAll", "Stops all sounds")]) },
                    json!("playStop"),
                ),
                ParamDef { when: Some(("mode", vec!["playStop", "restart", "overlap"])), ..p("file", "Sound file", ParamKind::Target, json!("")) },
                ParamDef { when: Some(("mode", vec!["playStop", "restart", "overlap"])), ..p("volume", "Volume (%)", ParamKind::Number { min: 0.0, max: 200.0, step: 5.0 }, json!(80)) },
                ParamDef {
                    help: Some("Leave empty for the default speakers; pick a device to play there, e.g. a virtual cable into a call or a stream."),
                    when: Some(("mode", vec!["playStop", "restart", "overlap"])),
                    ..p("device", "Play on", ParamKind::AudioDevice, json!(""))
                },
            ],
            icon: "music",
        },
        ActionDef {
            id: "core.mic",
            name: "Microphone mute",
            group: "Audio",
            description: "Mutes and unmutes the default microphone for every app. The key shows whether it is muted.",
            inputs: vec![Press],
            params: vec![press_mode()],
            icon: "mic",
        },
        ActionDef {
            id: NOW_PLAYING,
            name: "Now playing",
            group: "Media",
            description: "Shows the cover, title and artist of what is playing (Windows: any player; macOS: Spotify and Apple Music). A press plays or pauses; turning changes the volume, skips tracks or moves in the song.",
            inputs: vec![Press, Turn],
            params: vec![p("turn", "Turning", ParamKind::Select { options: options(&[("volume", "Changes the volume"), ("tracks", "Skips tracks"), ("seek", "Moves in the song")]) }, json!("volume"))],
            icon: "music",
        },
        ActionDef {
            id: MONITOR,
            name: "System monitor",
            group: "System",
            description: "Shows CPU load or temperature, memory, or GPU load, temperature or memory on a key or the dial, updated every second. A press opens Task Manager (Activity Monitor on a Mac).",
            inputs: vec![Press],
            params: vec![ParamDef {
                help: Some(
                    "CPU temperature, and AMD or Intel GPUs, come from LibreHardwareMonitor: run it (Options > Run on Windows startup) and they show; without it the key shows a dash. NVIDIA GPUs work without it.",
                ),
                ..p(
                    "metric",
                    "Show",
                    ParamKind::Select {
                        options: options(&[
                            ("cpu", "CPU load"),
                            ("cpuTemp", "CPU temperature"),
                            ("ram", "Memory used"),
                            ("gpu", "GPU load"),
                            ("gpuTemp", "GPU temperature"),
                            ("vram", "GPU memory used"),
                        ]),
                    },
                    json!("cpu"),
                )
            }],
            icon: "monitor",
        },
        ActionDef {
            id: super::live::ACTION,
            name: "Live value",
            group: "System",
            description: "Shows a value that refreshes by itself on a key or the dial: from a web address (JSON or text), a Home Assistant sensor, a command, the weather or the next calendar event. Thresholds color it; with a maximum the dial draws a ring. A press opens a page, or refreshes.",
            inputs: vec![Press],
            params: vec![
                p(
                    "source",
                    "From",
                    ParamKind::Select {
                        options: options(&[("url", "A web address"), ("weather", "The weather"), ("calendar", "Next calendar event"), ("ha", "Home Assistant"), ("command", "A command")]),
                    },
                    json!("url"),
                ),
                ParamDef {
                    when: Some(("source", vec!["url", "calendar"])),
                    help: Some(
                        "A web address that returns JSON or text, e.g. https://api.example.com/status; for a calendar its secret ICS address (Google: Settings > the calendar > Secret address in iCal format; Outlook: Publish calendar > ICS).",
                    ),
                    ..p("url", "Address", ParamKind::Text { multiline: false }, json!(""))
                },
                ParamDef {
                    when: Some(("source", vec!["url"])),
                    help: Some("Where the value is in the JSON, e.g. data.items.0.price; empty for a plain-text answer."),
                    ..p("path", "Value at", ParamKind::Text { multiline: false }, json!(""))
                },
                ParamDef {
                    when: Some(("source", vec!["url"])),
                    help: Some("Sent as Authorization: Bearer; kept in the system's credential store, never in the profile."),
                    ..p("secret", "Token", ParamKind::Secret, json!(""))
                },
                ParamDef {
                    when: Some(("source", vec!["ha"])),
                    help: Some("The entity whose state to show, e.g. sensor.living_room_temperature."),
                    ..p("entity", "Entity", ParamKind::Text { multiline: false }, json!(""))
                },
                ParamDef {
                    when: Some(("source", vec!["command"])),
                    help: Some("Its output's first line is shown. It runs only after you allow it with a press on the key (a shared profile cannot run commands on its own)."),
                    ..p("command", "Command", ParamKind::Text { multiline: false }, json!(""))
                },
                ParamDef {
                    when: Some(("source", vec!["weather"])),
                    help: Some("Search for a city; the weather comes from Open-Meteo (free, no account)."),
                    ..p("place", "Place", ParamKind::Place, json!(""))
                },
                ParamDef {
                    when: Some(("source", vec!["weather"])), ..p("units", "Temperature in", ParamKind::Select { options: options(&[("metric", "°C"), ("imperial", "°F")]) }, json!("metric"))
                },
                ParamDef { when: Some(("source", vec!["url", "ha", "command", "calendar"])), ..p("every", "Refresh every (s)", ParamKind::Number { min: 5.0, max: 86_400.0, step: 5.0 }, json!(60)) },
                p("prefix", "Before the value", ParamKind::Text { multiline: false }, json!("")),
                p("suffix", "After the value", ParamKind::Text { multiline: false }, json!("")),
                ParamDef { help: Some("-1 = as it comes."), ..p("decimals", "Decimals", ParamKind::Number { min: -1.0, max: 6.0, step: 1.0 }, json!(-1)) },
                ParamDef { help: Some("The key turns orange from this value on; empty = never."), ..p("warn", "Warning at", ParamKind::Text { multiline: false }, json!("")) },
                ParamDef { help: Some("The key turns red from this value on; empty = never."), ..p("alert", "Alert at", ParamKind::Text { multiline: false }, json!("")) },
                p("below", "Worse when", ParamKind::Select { options: options(&[("false", "Higher (load, temperature)"), ("true", "Lower (battery, free space)")]) }, json!("false")),
                ParamDef { help: Some("With a maximum the dial draws the value as a ring; 0 = no ring."), ..p("max", "Maximum", ParamKind::Number { min: 0.0, max: 1e12, step: 1.0 }, json!(0)) },
                ParamDef { help: Some("A web page or file to open on a press; empty = a press refreshes."), ..p("open", "Press opens", ParamKind::Target, json!("")) },
            ],
            icon: "zap",
        },
        ActionDef {
            id: "core.brightness",
            name: "Monitor brightness",
            group: "System",
            description: "The monitor's own brightness over DDC/CI: knobs and the dial turn it, faders set it. Most desktop monitors support it; laptop screens do not.",
            inputs: vec![Turn, Slide],
            params: vec![
                p("monitor", "Monitor", ParamKind::Select { options: options(&[("all", "All monitors"), ("1", "Monitor 1"), ("2", "Monitor 2"), ("3", "Monitor 3")]) }, json!("all")),
                p("step", "Step per detent (%)", ParamKind::Number { min: 1.0, max: 20.0, step: 1.0 }, json!(5)),
            ],
            icon: "sun",
        },
        ActionDef {
            id: "obs.scene",
            name: "Scene",
            group: "OBS",
            description: "Switches OBS to a scene. The key lights up while that scene is live.",
            inputs: vec![Press],
            params: vec![ParamDef { help: Some("Exactly as named in OBS."), ..p("scene", "Scene", ParamKind::Text { multiline: false }, json!("")) }],
            icon: "monitor",
        },
        ActionDef {
            id: "obs.record",
            name: "Record",
            group: "OBS",
            description: "Starts or stops recording. The key turns red while recording.",
            inputs: vec![Press],
            params: vec![],
            icon: "circle-dot",
        },
        ActionDef { id: "obs.stream", name: "Stream", group: "OBS", description: "Starts or stops streaming. The key turns red while live.", inputs: vec![Press], params: vec![], icon: "radio" },
        ActionDef {
            id: "obs.replay",
            name: "Save replay",
            group: "OBS",
            description: "Saves the replay buffer (start the replay buffer in OBS first).",
            inputs: vec![Press],
            params: vec![],
            icon: "rocket",
        },
        ActionDef {
            id: "obs.mute",
            name: "Mute source",
            group: "OBS",
            description: "Mutes or unmutes an audio source, e.g. the microphone. The key shows when it is muted.",
            inputs: vec![Press],
            params: vec![
                ParamDef { help: Some("The audio source as named in the OBS mixer, e.g. Mic/Aux."), ..p("input", "Audio source", ParamKind::Text { multiline: false }, json!("Mic/Aux")) },
                press_mode(),
            ],
            icon: "mic",
        },
        ActionDef {
            id: "obs.volume",
            name: "Source volume",
            group: "OBS",
            description: "The OBS mixer volume of a source: faders set it, knobs and the dial turn it.",
            inputs: vec![Turn, Slide],
            params: vec![
                ParamDef { help: Some("The audio source as named in the OBS mixer."), ..p("input", "Audio source", ParamKind::Text { multiline: false }, json!("Desktop Audio")) },
                p("step", "Step per detent (dB)", ParamKind::Number { min: 0.5, max: 6.0, step: 0.5 }, json!(1)),
            ],
            icon: "volume-2",
        },
        ActionDef {
            id: "obs.source",
            name: "Show / hide source",
            group: "OBS",
            description: "Shows or hides a source in a scene, e.g. a camera or an overlay.",
            inputs: vec![Press],
            params: vec![
                p("source", "Source", ParamKind::Text { multiline: false }, json!("")),
                ParamDef { help: Some("Leave empty for the scene that is live."), ..p("scene", "In scene", ParamKind::Text { multiline: false }, json!("")) },
            ],
            icon: "video",
        },
        ActionDef {
            id: "obs.recordPause",
            name: "Pause recording",
            group: "OBS",
            description: "Pauses or resumes a running recording. The key turns red while paused.",
            inputs: vec![Press],
            params: vec![],
            icon: "pause",
        },
        ActionDef {
            id: "obs.virtualCam",
            name: "Virtual camera",
            group: "OBS",
            description: "Starts or stops the OBS virtual camera. The key turns red while it runs.",
            inputs: vec![Press],
            params: vec![],
            icon: "video",
        },
        ActionDef {
            id: "obs.replayBuffer",
            name: "Replay buffer on / off",
            group: "OBS",
            description: "Starts or stops the replay buffer (needed before Save replay). The key turns red while it runs.",
            inputs: vec![Press],
            params: vec![],
            icon: "rocket",
        },
        ActionDef {
            id: "obs.studioMode",
            name: "Studio mode",
            group: "OBS",
            description: "Turns studio mode (preview and program side by side) on or off. The key lights up while it is on.",
            inputs: vec![Press],
            params: vec![],
            icon: "monitor",
        },
        ActionDef {
            id: "obs.transition",
            name: "Transition",
            group: "OBS",
            description: "In studio mode: sends the preview scene to program.",
            inputs: vec![Press],
            params: vec![ParamDef {
                help: Some("Leave empty for the transition chosen in OBS, or name one, e.g. Fade."),
                ..p("transition", "Transition", ParamKind::Text { multiline: false }, json!(""))
            }],
            icon: "skip-forward",
        },
        ActionDef {
            id: "obs.sceneCollection",
            name: "Scene collection",
            group: "OBS",
            description: "Switches OBS to another scene collection. The key lights up while it is the current one.",
            inputs: vec![Press],
            params: vec![ParamDef { help: Some("Exactly as named in OBS (Scene Collection menu)."), ..p("collection", "Scene collection", ParamKind::Text { multiline: false }, json!("")) }],
            icon: "layers",
        },
        ActionDef {
            id: "obs.profile",
            name: "OBS profile",
            group: "OBS",
            description: "Switches OBS to another of its profiles (output and stream settings). The key lights up while it is the current one.",
            inputs: vec![Press],
            params: vec![ParamDef { help: Some("Exactly as named in OBS (Profile menu)."), ..p("profile", "OBS profile", ParamKind::Text { multiline: false }, json!("")) }],
            icon: "settings",
        },
        ActionDef {
            id: "obs.filter",
            name: "Filter on / off",
            group: "OBS",
            description: "Turns a filter of a source or scene on or off. The key lights up while the filter is on.",
            inputs: vec![Press],
            params: vec![
                ParamDef { help: Some("The source or scene that has the filter."), ..p("source", "Source", ParamKind::Text { multiline: false }, json!("")) },
                p("filter", "Filter", ParamKind::Text { multiline: false }, json!("")),
            ],
            icon: "sparkles",
        },
        ActionDef {
            id: "obs.screenshot",
            name: "Screenshot",
            group: "OBS",
            description: "Saves a PNG of a source or of the live scene.",
            inputs: vec![Press],
            params: vec![
                ParamDef { help: Some("Leave empty for the scene that is live."), ..p("source", "Source", ParamKind::Text { multiline: false }, json!("")) },
                ParamDef { help: Some("Leave empty for the OBS recording folder."), ..p("folder", "Folder", ParamKind::Text { multiline: false }, json!("")) },
            ],
            icon: "camera",
        },
        ActionDef {
            id: "streamlabs.scene",
            name: "Scene",
            group: "Streamlabs",
            description: "Switches Streamlabs Desktop to a scene. The key lights up while that scene is live.",
            inputs: vec![Press],
            params: vec![ParamDef { help: Some("Exactly as named in Streamlabs."), ..p("scene", "Scene", ParamKind::Text { multiline: false }, json!("")) }],
            icon: "monitor",
        },
        ActionDef {
            id: "streamlabs.record",
            name: "Record",
            group: "Streamlabs",
            description: "Starts or stops recording. The key turns red while recording.",
            inputs: vec![Press],
            params: vec![],
            icon: "circle-dot",
        },
        ActionDef {
            id: "streamlabs.stream",
            name: "Stream",
            group: "Streamlabs",
            description: "Goes live or ends the stream. The key turns red while live.",
            inputs: vec![Press],
            params: vec![],
            icon: "radio",
        },
        ActionDef {
            id: "streamlabs.replay",
            name: "Save replay",
            group: "Streamlabs",
            description: "Saves the replay buffer (it has to be running).",
            inputs: vec![Press],
            params: vec![],
            icon: "rocket",
        },
        ActionDef {
            id: "streamlabs.replayBuffer",
            name: "Replay buffer on / off",
            group: "Streamlabs",
            description: "Starts or stops the replay buffer. The key turns red while it runs.",
            inputs: vec![Press],
            params: vec![],
            icon: "rocket",
        },
        ActionDef {
            id: "streamlabs.studioMode",
            name: "Studio mode",
            group: "Streamlabs",
            description: "Turns studio mode on or off. The key lights up while it is on.",
            inputs: vec![Press],
            params: vec![],
            icon: "monitor",
        },
        ActionDef {
            id: "streamlabs.transition",
            name: "Transition",
            group: "Streamlabs",
            description: "In studio mode: sends the preview scene to the stream.",
            inputs: vec![Press],
            params: vec![],
            icon: "skip-forward",
        },
        ActionDef {
            id: "streamlabs.source",
            name: "Show / hide source",
            group: "Streamlabs",
            description: "Shows or hides a source in a scene, e.g. a camera or an overlay.",
            inputs: vec![Press],
            params: vec![
                p("source", "Source", ParamKind::Text { multiline: false }, json!("")),
                ParamDef { help: Some("Leave empty for the scene that is live."), ..p("scene", "In scene", ParamKind::Text { multiline: false }, json!("")) },
            ],
            icon: "video",
        },
        ActionDef {
            id: "streamlabs.mute",
            name: "Mute source",
            group: "Streamlabs",
            description: "Mutes or unmutes an audio source of the Streamlabs mixer. The key shows when it is muted.",
            inputs: vec![Press],
            params: vec![ParamDef { help: Some("As named in the Streamlabs mixer, e.g. Mic/Aux."), ..p("input", "Audio source", ParamKind::Text { multiline: false }, json!("Mic/Aux")) }],
            icon: "mic",
        },
        ActionDef {
            id: "streamlabs.volume",
            name: "Source volume",
            group: "Streamlabs",
            description: "The Streamlabs mixer volume of a source: faders set it, knobs and the dial turn it.",
            inputs: vec![Turn, Slide],
            params: vec![
                ParamDef { help: Some("As named in the Streamlabs mixer."), ..p("input", "Audio source", ParamKind::Text { multiline: false }, json!("Desktop Audio")) },
                p("step", "Step per detent (%)", ParamKind::Number { min: 1.0, max: 20.0, step: 1.0 }, json!(2)),
            ],
            icon: "volume-2",
        },
        ActionDef {
            id: "twitch.chat",
            name: "Chat message",
            group: "Twitch",
            description: "Sends a message to your Twitch chat.",
            inputs: vec![Press],
            params: vec![ParamDef { help: Some("Commands that start with / do not work here."), ..p("message", "Message", ParamKind::Text { multiline: false }, json!("")) }],
            icon: "message-square",
        },
        ActionDef { id: "twitch.clearChat", name: "Clear chat", group: "Twitch", description: "Removes all messages from your chat.", inputs: vec![Press], params: vec![], icon: "x" },
        ActionDef {
            id: "twitch.chatMode",
            name: "Chat mode",
            group: "Twitch",
            description: "Turns a chat mode on or off: followers only, subscribers only, emotes only or slow mode. The key lights up while it is on (known after the first press).",
            inputs: vec![Press],
            params: vec![
                p(
                    "mode",
                    "Mode",
                    ParamKind::Select { options: options(&[("followers", "Followers only"), ("subscribers", "Subscribers only"), ("emote", "Emotes only"), ("slow", "Slow mode")]) },
                    json!("followers"),
                ),
                ParamDef {
                    help: Some("Followers only: how long someone must follow, in minutes (0 = any follower). Slow mode: seconds between messages (3-120)."),
                    when: Some(("mode", vec!["followers", "slow"])),
                    ..p("duration", "Duration", ParamKind::Number { min: 0.0, max: 129600.0, step: 1.0 }, json!(10))
                },
            ],
            icon: "message-square",
        },
        ActionDef {
            id: "twitch.shield",
            name: "Shield Mode",
            group: "Twitch",
            description: "Turns Shield Mode on or off. The key lights up while it is on (known after the first press).",
            inputs: vec![Press],
            params: vec![],
            icon: "lock",
        },
        ActionDef {
            id: "twitch.ad",
            name: "Run an ad",
            group: "Twitch",
            description: "Starts a commercial break (Twitch Partners and Affiliates, while live).",
            inputs: vec![Press],
            params: vec![p(
                "length",
                "Length",
                ParamKind::Select { options: options(&[("30", "30 s"), ("60", "1 min"), ("90", "1 min 30 s"), ("120", "2 min"), ("150", "2 min 30 s"), ("180", "3 min")]) },
                json!("30"),
            )],
            icon: "bell",
        },
        ActionDef {
            id: "twitch.title",
            name: "Stream title and game",
            group: "Twitch",
            description: "Sets the stream title, the game (category) or both.",
            inputs: vec![Press],
            params: vec![
                ParamDef { help: Some("Leave empty to keep the current title."), ..p("title", "Title", ParamKind::Text { multiline: false }, json!("")) },
                ParamDef { help: Some("As named on Twitch, e.g. Just Chatting. Leave empty to keep it."), ..p("game", "Game", ParamKind::Text { multiline: false }, json!("")) },
            ],
            icon: "type",
        },
        ActionDef {
            id: "twitch.marker",
            name: "Stream marker",
            group: "Twitch",
            description: "Marks this moment of the live stream, to find it later in the video.",
            inputs: vec![Press],
            params: vec![p("description", "Description", ParamKind::Text { multiline: false }, json!(""))],
            icon: "star",
        },
        ActionDef { id: "twitch.clip", name: "Create clip", group: "Twitch", description: "Clips the last moments of your live stream.", inputs: vec![Press], params: vec![], icon: "camera" },
        ActionDef {
            id: "twitch.openClip",
            name: "Open last clip",
            group: "Twitch",
            description: "Opens the clip made last with HubDeck, or your channel's clips page.",
            inputs: vec![Press],
            params: vec![],
            icon: "globe",
        },
        ActionDef {
            id: "spotify.like",
            name: "Like / unlike",
            group: "Spotify",
            description: "Adds the playing track to your Liked Songs, or takes it out. The key turns green while the track is liked.",
            inputs: vec![Press],
            params: vec![],
            icon: "heart",
        },
        ActionDef {
            id: "spotify.remove",
            name: "Remove from this playlist",
            group: "Spotify",
            description: "Takes the playing track out of the playlist it plays from (your own playlists), then skips to the next one (Premium).",
            inputs: vec![Press],
            params: vec![],
            icon: "x",
        },
        ActionDef {
            id: "spotify.add",
            name: "Add to playlist",
            group: "Spotify",
            description: "Adds the playing track to a playlist of yours.",
            inputs: vec![Press],
            params: vec![ParamDef { help: Some("The playlist's link (Share > Copy link) or id."), ..p("playlist", "Playlist", ParamKind::Text { multiline: false }, json!("")) }],
            icon: "list-checks",
        },
        ActionDef {
            id: "ha.toggle",
            name: "Toggle device",
            group: "Home Assistant",
            description: "Turns a light, switch, fan, cover or media player on or off. The key lights up while it is on.",
            inputs: vec![Press],
            params: vec![ParamDef { help: Some("The entity id, e.g. light.desk or switch.kettle."), ..p("entity", "Entity", ParamKind::Text { multiline: false }, json!("")) }],
            icon: "lightbulb",
        },
        ActionDef {
            id: "ha.activate",
            name: "Scene or script",
            group: "Home Assistant",
            description: "Runs a scene or a script.",
            inputs: vec![Press],
            params: vec![ParamDef { help: Some("e.g. scene.movie_night or script.good_night."), ..p("entity", "Scene or script", ParamKind::Text { multiline: false }, json!("")) }],
            icon: "sparkles",
        },
        ActionDef {
            id: "ha.brightness",
            name: "Light brightness",
            group: "Home Assistant",
            description: "A light's brightness: faders set it, knobs and the dial turn it, a knob press toggles the light.",
            inputs: vec![Turn, Slide, Press],
            params: vec![
                ParamDef { help: Some("e.g. light.desk."), ..p("entity", "Light", ParamKind::Text { multiline: false }, json!("")) },
                p("step", "Step per detent (%)", ParamKind::Number { min: 1.0, max: 25.0, step: 1.0 }, json!(5)),
            ],
            icon: "sun",
        },
        ActionDef {
            id: "ha.service",
            name: "Call a service",
            group: "Home Assistant",
            description: "Calls any Home Assistant service with data, e.g. climate.set_temperature.",
            inputs: vec![Press],
            params: vec![
                p("service", "Service", ParamKind::Text { multiline: false }, json!("")),
                ParamDef { help: Some(r#"JSON, e.g. {"entity_id": "climate.living_room", "temperature": 21}"#), ..p("data", "Data", ParamKind::Text { multiline: true }, json!("{}")) },
            ],
            icon: "home",
        },
        ActionDef {
            id: "midi.cc",
            name: "MIDI control change",
            group: "MIDI",
            description: "Faders send their position, knobs and the dial turn the value, keys send the value below on press. For any DAW or synth.",
            inputs: vec![Slide, Turn, Press],
            params: vec![
                p("port", "MIDI output", ParamKind::MidiPort, json!("")),
                p("channel", "Channel", ParamKind::Number { min: 1.0, max: 16.0, step: 1.0 }, json!(1)),
                p("cc", "Controller (CC)", ParamKind::Number { min: 0.0, max: 127.0, step: 1.0 }, json!(1)),
                p("step", "Step per detent", ParamKind::Number { min: 1.0, max: 16.0, step: 1.0 }, json!(2)),
                ParamDef { help: Some("Keys only: the value a press sends."), ..p("value", "Value on press", ParamKind::Number { min: 0.0, max: 127.0, step: 1.0 }, json!(127)) },
            ],
            icon: "music",
        },
        ActionDef {
            id: "midi.note",
            name: "MIDI note",
            group: "MIDI",
            description: "Plays a note while the key is held down.",
            inputs: vec![Press],
            params: vec![
                p("port", "MIDI output", ParamKind::MidiPort, json!("")),
                p("channel", "Channel", ParamKind::Number { min: 1.0, max: 16.0, step: 1.0 }, json!(1)),
                ParamDef { help: Some("60 = middle C."), ..p("note", "Note", ParamKind::Number { min: 0.0, max: 127.0, step: 1.0 }, json!(60)) },
                p("velocity", "Velocity", ParamKind::Number { min: 1.0, max: 127.0, step: 1.0 }, json!(100)),
            ],
            icon: "music",
        },
        ActionDef {
            id: "core.run",
            name: "Run command",
            group: "System",
            description: "Runs a program, a PowerShell or batch script, or a shell command, with arguments, without a console window if you like.",
            inputs: vec![Press],
            params: vec![
                ParamDef { help: Some("A program, or a .ps1 / .bat / .cmd / .sh script."), ..p("command", "Program or script", ParamKind::Target, json!("")) },
                ParamDef { help: Some(r#"Separated by spaces; put "quotes" around one with spaces."#), ..p("args", "Arguments", ParamKind::Text { multiline: false }, json!("")) },
                p("folder", "Start in folder", ParamKind::Text { multiline: false }, json!("")),
                p("window", "Window", ParamKind::Select { options: options(&[("hidden", "Hidden"), ("normal", "Shown")]) }, json!("hidden")),
            ],
            icon: "terminal",
        },
        // an app's state on a key or the dial (docs/APP-CONTRACT.md)
        ActionDef {
            id: super::states::ACTION,
            name: "App state",
            group: "System",
            description: "Shows the state of one of your connected apps (an agent waiting, a build running): its color and icon change with it. On the dial, app.* counts what waits for you.",
            inputs: vec![Press],
            params: vec![
                ParamDef {
                    help: Some("app.item, like launcher.1; launcher.* counts the items (Settings > Connections > Your apps)"),
                    ..p("name", "State", ParamKind::Text { multiline: false }, json!(""))
                },
                ParamDef {
                    help: Some("Called on a press ({id} = the item), like http://127.0.0.1:4317/agents/{id}/focus"),
                    ..p("url", "Address to call", ParamKind::Text { multiline: false }, json!(""))
                },
                p("method", "Method", ParamKind::Select { options: options(&[("POST", "POST"), ("GET", "GET"), ("PUT", "PUT")]) }, json!("POST")),
                ParamDef { help: Some("Or a link the app registered, opened on a press"), ..p("link", "Link to open", ParamKind::Text { multiline: false }, json!("")) },
            ],
            icon: "circle-dot",
        },
        // a key that talks to an app's own API (owner, 2026-10-12: integrate my own apps)
        ActionDef {
            id: HTTP,
            name: "HTTP request",
            group: "System",
            description: "Calls a web address or an app's local API (GET, or POST with a body). The key says whether it worked.",
            inputs: vec![Press],
            params: vec![
                p("method", "Method", ParamKind::Select { options: options(&[("GET", "GET"), ("POST", "POST"), ("PUT", "PUT"), ("PATCH", "PATCH"), ("DELETE", "DELETE")]) }, json!("POST")),
                ParamDef { help: Some("Like http://127.0.0.1:7777/agents/1/focus"), ..p("url", "Address", ParamKind::Text { multiline: false }, json!("")) },
                ParamDef {
                    help: Some("JSON or text; sent as JSON when it starts with { or ["),
                    when: Some(("method", vec!["POST", "PUT", "PATCH"])),
                    ..p("body", "Body", ParamKind::Text { multiline: true }, json!(""))
                },
                ParamDef { help: Some("One per line, like Authorization: Bearer abc"), ..p("headers", "Headers", ParamKind::Text { multiline: true }, json!("")) },
            ],
            icon: "send",
        },
        ActionDef {
            id: "core.audioDevice",
            name: "Audio device",
            group: "Audio",
            description: "Makes a speaker, headset or microphone the default for every app. Turning goes through them in turn. The key lights up while its device is the default.",
            inputs: vec![Press, Turn],
            params: vec![
                p("kind", "Kind", ParamKind::Select { options: options(&[("output", "Speakers / headphones"), ("input", "Microphone")]) }, json!("output")),
                ParamDef { help: Some("Leave empty to go to the next device on each press."), ..p("device", "Device", ParamKind::AudioDevice, json!("")) },
            ],
            icon: "headphones",
        },
        ActionDef {
            id: DAY_MODE,
            name: "Day mode",
            group: "HubDeck",
            description: "One key for a whole setup: Work, Game, Meeting, Recording... It switches the profile, makes your speakers or headset and your microphone the default, and lights the base edge in the mode's color, all at once. The key lights up while its profile shows.",
            inputs: vec![Press],
            params: vec![
                ParamDef { help: Some("Empty: the profile stays."), ..p("profile", "Profile", ParamKind::Profile, json!("")) },
                ParamDef { help: Some("Empty: the speakers stay as they are."), ..p("output", "Speakers / headphones", ParamKind::AudioDevice, json!("")) },
                ParamDef { help: Some("Empty: the microphone stays as it is."), ..p("input", "Microphone", ParamKind::AudioDevice, json!("")) },
                ParamDef { help: Some("#rrggbb; empty: the edge keeps the profile's light."), ..p("color", "Edge color", ParamKind::Text { multiline: false }, json!("")) },
            ],
            icon: "sun",
        },
        ActionDef {
            id: TOGGLE,
            name: "Toggle",
            group: "HubDeck",
            description: "Runs the first steps on one press and the second on the next, e.g. start and stop. The key shows which one comes next.",
            inputs: vec![Press],
            params: vec![p("first", "First press", ParamKind::Steps, json!([])), p("second", "Second press", ParamKind::Steps, json!([]))],
            icon: "refresh-cw",
        },
        ActionDef {
            id: "core.scroll",
            name: "Scroll",
            group: "System",
            description: "Scrolls the window under the mouse pointer.",
            inputs: vec![Turn],
            params: vec![
                p("axis", "Direction", ParamKind::Select { options: options(&[("vertical", "Up / down"), ("horizontal", "Left / right")]) }, json!("vertical")),
                p("amount", "Lines per detent", ParamKind::Number { min: 1.0, max: 10.0, step: 1.0 }, json!(3)),
            ],
            icon: "arrow-up-down",
        },
        ActionDef {
            id: "core.text",
            name: "Type text",
            group: "System",
            description: "Types a snippet of text, e.g. an email address or a signature.",
            inputs: vec![Press],
            params: vec![p("text", "Text", ParamKind::Text { multiline: true }, json!(""))],
            icon: "type",
        },
        ActionDef {
            id: MULTI,
            name: "Multi-action",
            group: "HubDeck",
            description: "Runs several actions one after another, with a pause before each if you want one.",
            inputs: vec![Press],
            params: vec![ParamDef { help: Some("Each step waits its delay, then runs. Steps cannot contain another multi-action."), ..p("steps", "Steps", ParamKind::Steps, json!([])) }],
            icon: "list-ordered",
        },
        ActionDef {
            id: MODES,
            name: "Modes",
            group: "HubDeck",
            description: "Several jobs on one dial or knob: turning does the current one, a press switches to the next and shows its name.",
            inputs: vec![Press, Turn],
            params: vec![ParamDef { help: Some("Each mode turns something: volume, scroll, a hotkey per direction, media."), ..p("modes", "Modes", ParamKind::Modes, json!([])) }],
            icon: "layers",
        },
        ActionDef {
            id: super::gallery::ACTION,
            name: "Dial gallery",
            group: "HubDeck",
            description: "Shows what the dial can look like: turn it to go through the looks (volume, clock, now playing, computer load, timer...), live where it can be, each with its name at the top.",
            inputs: vec![Press, Turn],
            params: vec![],
            icon: "circle-dot",
        },
        ActionDef {
            id: GAMES,
            name: "Games",
            group: "HubDeck",
            description: "Turns the deck into a small console: every key is a game. Hold the dial to go back.",
            inputs: vec![Press],
            params: vec![],
            icon: "gamepad-2",
        },
        ActionDef {
            id: AGENT,
            name: "Session",
            group: "Claude Code",
            description: "One Claude Code session per key, in the order they started: blue while it works, orange blinking when it needs your permission, green when it is your turn. A press brings its terminal to the front. Connect Claude Code in Settings first.",
            inputs: vec![Press],
            params: vec![p("slot", "Session number", ParamKind::Number { min: 1.0, max: 15.0, step: 1.0 }, json!(1))],
            icon: "bot",
        },
        ActionDef {
            id: AGENTS,
            name: "All sessions",
            group: "Claude Code",
            description: "The dial (or a knob) goes through your Claude Code sessions: turn to pick one, the screen tells what it does, press to bring it to the front.",
            inputs: vec![Press, Turn],
            params: vec![],
            icon: "bot",
        },
        ActionDef {
            id: "core.page",
            name: "Go to page",
            group: "HubDeck",
            description: "Shows another page of keys, like a folder. Put a key back to the main page on that page.",
            inputs: vec![Press],
            params: vec![p("page", "Page", ParamKind::Page, json!(""))],
            icon: "folder",
        },
        ActionDef {
            id: "core.profile",
            name: "Switch profile",
            group: "HubDeck",
            description: "Changes the active profile: a chosen one, or the next / previous (profiles in name order). Turning a knob or the dial steps through them.",
            inputs: vec![Press, Turn],
            params: vec![
                p("which", "Go to", ParamKind::Select { options: options(&[("named", "A chosen profile"), ("next", "The next profile"), ("previous", "The previous profile")]) }, json!("named")),
                ParamDef { when: Some(("which", vec!["named"])), ..p("profile", "Profile", ParamKind::Profile, json!("")) },
            ],
            icon: "layers",
        },
        ActionDef {
            id: CLOCK,
            name: "Clock",
            group: "HubDeck",
            description: "Shows the time, the date or both on a key or the dial.",
            inputs: vec![Press],
            params: vec![
                p("show", "Show", ParamKind::Select { options: options(&[("time", "Time"), ("timeDate", "Time and date"), ("date", "Date")]) }, json!("time")),
                p("hours", "Hours", ParamKind::Select { options: options(&[("24", "24-hour"), ("12", "12-hour")]) }, json!("24")),
            ],
            icon: "clock",
        },
        ActionDef {
            id: TIMER,
            name: "Timer",
            group: "HubDeck",
            description: "A countdown on a key or the dial. A press starts, pauses and resumes it; once it ran out, a press sets it back. On a knob or the dial, turning sets the minutes while it is stopped.",
            inputs: vec![Press, Turn],
            params: vec![
                p("minutes", "Minutes", ParamKind::Number { min: 0.0, max: 5999.0, step: 1.0 }, json!(5)),
                p("seconds", "Seconds", ParamKind::Number { min: 0.0, max: 59.0, step: 1.0 }, json!(0)),
                ParamDef {
                    help: Some("Put a Timer that resets on the hold or double press of the same key to have both."),
                    ..p("press", "A press", ParamKind::Select { options: options(&[("startPause", "Starts / pauses"), ("reset", "Resets")]) }, json!("startPause"))
                },
                p("sound", "Sound at the end", ParamKind::Select { options: options(&[("on", "On"), ("off", "Off")]) }, json!("on")),
            ],
            icon: "timer",
        },
        ActionDef {
            id: "core.lighting",
            name: "Lighting effect",
            group: "HubDeck",
            description: "Sets an LED effect on the knobs, the faders, the base edge or all of them until the profile changes; \"The profile's own\" sets them back. Works in multi-actions and toggles too, e.g. the base breathing red while OBS records.",
            inputs: vec![Press],
            params: vec![
                p("target", "On", ParamKind::Select { options: options(&[("all", "Everything"), ("knobs", "Knobs"), ("faders", "Faders"), ("base", "Base edge")]) }, json!("all")),
                p(
                    "effect",
                    "Effect",
                    ParamKind::Select { options: options(&[("profile", "The profile's own")].into_iter().chain(super::leds::EFFECTS).chain([("off", "Off")]).collect::<Vec<_>>()) },
                    json!("breathing"),
                ),
                ParamDef { when: Some(("effect", vec!["static", "breathing", "strobe", "blink", "water"])), ..p("color", "Color", ParamKind::Text { multiline: false }, json!("#ff2020")) },
                ParamDef {
                    when: Some(("effect", vec!["breathing", "strobe", "blink", "cycling", "random", "wave", "water", "rainbow"])),
                    ..p("speed", "Speed (%)", ParamKind::Number { min: 0.0, max: 100.0, step: 5.0 }, json!(50))
                },
            ],
            icon: "lightbulb",
        },
        ActionDef {
            id: "core.deckBrightness",
            name: "Deck brightness",
            group: "HubDeck",
            description: "Brightness of the key panel and the dial screen: faders set it, knobs and the dial turn it, a key makes it brighter or darker.",
            inputs: vec![Press, Turn, Slide],
            params: vec![
                p("press", "A press makes it", ParamKind::Select { options: options(&[("brighter", "Brighter"), ("darker", "Darker")]) }, json!("brighter")),
                p("step", "Step (%)", ParamKind::Number { min: 1.0, max: 50.0, step: 1.0 }, json!(10)),
            ],
            icon: "sun",
        },
        ActionDef {
            id: "vjoy.button",
            name: "Joystick button",
            group: "Joystick",
            description: "Presses a button of a virtual joystick (vJoy) while the key is held, for sims and games: bind it in the game's control settings. On a knob, roller or the dial each step taps a button. Windows only; needs vJoy.",
            inputs: vec![Press, Turn],
            params: vec![
                joystick_device(),
                ParamDef { help: Some("Held down as long as the key or the push of a knob or the dial."), ..p("button", "Button", ParamKind::Number { min: 1.0, max: 128.0, step: 1.0 }, json!(1)) },
                ParamDef {
                    help: Some("Knobs, rollers and the dial: tapped once per step; 0 = none."),
                    ..p("clockwise", "Turn clockwise / up taps", ParamKind::Number { min: 0.0, max: 128.0, step: 1.0 }, json!(0))
                },
                p("counterClockwise", "Turn counter-clockwise / down taps", ParamKind::Number { min: 0.0, max: 128.0, step: 1.0 }, json!(0)),
            ],
            icon: "gamepad-2",
        },
        ActionDef {
            id: "vjoy.axis",
            name: "Joystick axis",
            group: "Joystick",
            description: "Moves an axis of a virtual joystick (vJoy): a fader sets it (throttle, mixture, flaps), knobs, rollers and the dial move it step by step (trim), a press centers it. Bind it in the sim. Windows only; needs vJoy.",
            inputs: vec![Slide, Turn, Press],
            params: vec![
                joystick_device(),
                p("axis", "Axis", ParamKind::Select { options: options(&super::vjoy::AXES.iter().map(|(v, l, _)| (*v, *l)).collect::<Vec<_>>()) }, json!("x")),
                ParamDef { help: Some("Knobs, rollers and the dial: how far one step moves it."), ..p("step", "Step per detent (%)", ParamKind::Number { min: 0.5, max: 25.0, step: 0.5 }, json!(2)) },
                p(
                    "press",
                    "A press",
                    ParamKind::Select { options: options(&[("center", "Centers it"), ("zero", "Sets it to the minimum"), ("max", "Sets it to the maximum"), ("nothing", "Does nothing")]) },
                    json!("center"),
                ),
            ],
            icon: "gamepad-2",
        },
        ActionDef {
            id: "vjoy.hat",
            name: "Joystick hat",
            group: "Joystick",
            description: "Points a hat switch (POV) of a virtual joystick (vJoy) while the key is held, e.g. to look around. Windows only; needs vJoy.",
            inputs: vec![Press],
            params: vec![
                joystick_device(),
                p("hat", "Hat", ParamKind::Number { min: 1.0, max: 4.0, step: 1.0 }, json!(1)),
                ParamDef {
                    help: Some("Diagonals need a continuous hat (Configure vJoy > POV hat switch: Continuous)."),
                    ..p("direction", "Direction", ParamKind::Select { options: options(&super::vjoy::HAT_DIRECTIONS) }, json!("up"))
                },
            ],
            icon: "gamepad-2",
        },
    ]
}

/// The vJoy device param of the joystick actions.
fn joystick_device() -> ParamDef {
    ParamDef {
        help: Some("vJoy device 1-16 as set up in Configure vJoy. Install vJoy first: https://github.com/BrunnerInnovation/vJoy/releases"),
        ..p("device", "vJoy device", ParamKind::Number { min: 1.0, max: 16.0, step: 1.0 }, json!(1))
    }
}

/// Id of the system monitor action; the engine draws the live value.
pub const MONITOR: &str = "core.monitor";

/// The "Press" choice of a mute action: toggle, or act only while the key is held.
fn press_mode() -> ParamDef {
    ParamDef {
        help: Some("Push to talk: muted, on while you hold the key. Push to mute: on, muted while you hold it. While held, the key ignores its hold and double press."),
        ..p(
            "mode",
            "Press",
            ParamKind::Select { options: options(&[("toggle", "Toggles mute"), ("pushToTalk", "Push to talk (hold to speak)"), ("pushToMute", "Push to mute (hold to mute)")]) },
            json!("toggle"),
        )
    }
}

/// Does this binding act on the release too (push-to-talk, push-to-mute, joystick buttons and
/// hats)? Such a press acts at once, without waiting to tell a tap from a hold or double press.
pub fn while_held(action: &str, params: &Value) -> bool {
    matches!(action, "vjoy.button" | "vjoy.hat") || matches!(action, "core.mic" | "obs.mute") && matches!(param_str(params, "mode"), "pushToTalk" | "pushToMute")
}

/// Push modes: whether the target should be muted while the key is `down`; None = toggle mode.
pub fn push_mute(params: &Value, down: bool) -> Option<bool> {
    match param_str(params, "mode") {
        "pushToTalk" => Some(!down),
        "pushToMute" => Some(down),
        _ => None,
    }
}

/// Id of the clock action; the engine draws the time.
pub const CLOCK: &str = "core.clock";

/// Id of the countdown timer; the engine keeps its time and draws it.
pub const TIMER: &str = "core.timer";

/// Id of the now playing action; the engine draws the cover on its key or the dial.
pub const NOW_PLAYING: &str = "core.nowPlaying";
/// One Claude Code session on a key (`agents.rs`).
pub const AGENT: &str = "claude.session";
/// Every Claude Code session on the dial.
pub const AGENTS: &str = "claude.sessions";

/// Id of the action that starts game mode.
pub const GAMES: &str = "core.games";

/// Id of the toggle: two step lists taking turns.
pub const TOGGLE: &str = "core.toggle";

pub const DAY_MODE: &str = "core.dayMode";

/// Calls a web address or an app's local API.
pub const HTTP: &str = "core.http";
/// Id of the multi-action; the engine runs its steps itself, so delays never block it.
pub const MULTI: &str = "core.multi";

/// Longest delay before one step of a multi-action.
pub const MAX_STEP_DELAY_MS: u64 = 10_000;

/// One step of a multi-action.
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct Step {
    pub action: String,
    #[serde(default)]
    pub params: Value,
    /// Milliseconds to wait before this step.
    #[serde(default)]
    pub delay: u64,
    /// Run only when this is true at that moment: `micMuted`, `micOn`, `soundMuted`, `soundOn`,
    /// `obsRecording`, `obsNotRecording`, `obsStreaming`, `obsNotStreaming`; empty = always.
    #[serde(default)]
    pub when: String,
}

/// Id of the modes action; the engine keeps the current mode of each control.
pub const MODES: &str = "core.modes";

/// One mode of a dial or knob.
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct Mode {
    #[serde(default)]
    pub label: String,
    pub action: String,
    #[serde(default)]
    pub params: Value,
}

/// The usable modes: actions that react to turning, no nesting.
pub fn modes(params: &Value) -> Vec<Mode> {
    params
        .get("modes")
        .and_then(|s| serde_json::from_value::<Vec<Mode>>(s.clone()).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(|m| m.action != MODES && m.action != MULTI && find(&m.action).is_some_and(|a| a.inputs.contains(&InputKind::Turn)))
        .collect()
}

/// The runnable steps of a multi-action: unknown actions and nested multi-actions are dropped.
pub fn steps(params: &Value) -> Vec<Step> {
    steps_at(params, "steps")
}

/// The steps under `key` (multi-action `steps`, toggle `first` / `second`).
pub fn steps_at(params: &Value, key: &str) -> Vec<Step> {
    params
        .get(key)
        .and_then(|s| serde_json::from_value::<Vec<Step>>(s.clone()).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(|s| s.action != MULTI && s.action != TOGGLE && find(&s.action).is_some_and(|a| a.inputs.contains(&InputKind::Press)))
        .map(|s| Step { delay: s.delay.min(MAX_STEP_DELAY_MS), ..s })
        .collect()
}

/// Built-in actions followed by the actions of installed plugins.
pub fn all() -> Vec<ActionDef> {
    let mut list = catalog();
    list.extend(super::plugins::actions());
    list
}

pub fn find(id: &str) -> Option<ActionDef> {
    all().into_iter().find(|a| a.id == id)
}

/// Read a string param, falling back to the definition's default.
pub fn param_str<'a>(params: &'a Value, key: &str) -> &'a str {
    params.get(key).and_then(Value::as_str).unwrap_or("")
}

pub fn param_f64(params: &Value, key: &str, default: f64) -> f64 {
    params.get(key).and_then(|v| v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok()))).unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_consistent() {
        let all = catalog();
        let mut ids: Vec<_> = all.iter().map(|a| a.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), all.len(), "action ids are unique");
        for a in &all {
            assert!(!a.inputs.is_empty(), "{} accepts some input", a.id);
            assert!(super::super::render::icon_svg(a.icon).is_some(), "{}: icon {} is built in", a.id, a.icon);
            for p in &a.params {
                if let Some((other, _)) = &p.when {
                    assert!(a.params.iter().any(|q| q.key == *other), "{}.{} depends on a param that exists", a.id, p.key);
                }
            }
        }
        // the schema is what the UI receives
        let json = serde_json::to_value(&all).unwrap();
        assert_eq!(json[0]["params"][0]["type"], "target");
    }

    #[test]
    fn multi_action_steps_are_cleaned() {
        let params = json!({"steps": [
            {"action": "core.media", "params": {"command": "next"}},
            {"action": "core.multi", "params": {}},
            {"action": "core.scroll", "params": {}, "delay": 100},
            {"action": "nope"},
            {"action": "core.text", "params": {"text": "hi"}, "delay": 999999}
        ]});
        let s = steps(&params);
        assert_eq!(s.iter().map(|s| s.action.as_str()).collect::<Vec<_>>(), ["core.media", "core.text"], "no nesting, no turn-only or unknown actions");
        assert_eq!(s[1].delay, MAX_STEP_DELAY_MS);
        assert!(steps(&json!({})).is_empty());
    }

    #[test]
    fn modes_keep_only_turnable_actions() {
        let params = json!({"modes": [
            {"label": "Vol", "action": "core.volume", "params": {}},
            {"label": "Open", "action": "core.launch", "params": {}},
            {"label": "Nest", "action": "core.modes", "params": {}},
            {"label": "Scroll", "action": "core.scroll"}
        ]});
        assert_eq!(modes(&params).iter().map(|m| m.label.as_str()).collect::<Vec<_>>(), ["Vol", "Scroll"]);
    }

    #[test]
    fn push_modes_mute_by_key_position() {
        let ptt = json!({"mode": "pushToTalk"});
        assert_eq!(push_mute(&ptt, true), Some(false), "held: speak");
        assert_eq!(push_mute(&ptt, false), Some(true), "released: muted");
        assert_eq!(push_mute(&json!({"mode": "pushToMute"}), true), Some(true));
        assert_eq!(push_mute(&json!({}), true), None);
        assert!(while_held("obs.mute", &ptt));
        assert!(!while_held("core.mic", &json!({"mode": "toggle"})));
        assert!(!while_held("core.volume", &ptt));
        assert!(while_held("vjoy.button", &json!({})), "joystick buttons are never delayed");
    }

    #[test]
    fn numeric_params_accept_strings() {
        assert_eq!(param_f64(&json!({"step": "5"}), "step", 2.0), 5.0);
        assert_eq!(param_f64(&json!({}), "step", 2.0), 2.0);
    }
}
