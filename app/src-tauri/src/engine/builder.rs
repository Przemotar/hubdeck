//! Build the deck step by step (owner, 2026-10-13, layout A): one control at a time, a gallery of
//! options for it, each tried on the real deck before it is picked. This module gives the options
//! (real settings with a picture of the dial's screen) and the working draft; the window walks the
//! steps. Options come at once, without the model: built-in actions first (no setup), then the
//! Stream Deck plugins installed here.

use serde::Serialize;
use serde_json::{Value, json};
use tauri::State;

use super::profile::{Binding, Look, Profile};
use super::{Engine, actions, gallery, house, images, live, plugins, render};

/// One option for a control.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuilderOption {
    pub id: String,
    /// The theme it is shown under ("Music", "Work and focus"...).
    pub group: String,
    pub name: String,
    /// What turning and pressing do, in one line.
    pub does: String,
    /// Where it comes from and what it needs ("Built in, no setup", "Essentials for Spotify").
    pub source: String,
    /// It needs a setup of its own first (an account, a key).
    pub setup: bool,
    pub binding: Binding,
    /// The dial's screen as the deck draws it (dial options only).
    pub svg: Option<String>,
}

fn look(label: &str, icon: &str, hue: f32) -> Look {
    Look { label: Some(label.into()), icon: Some(icon.into()), bg: Some(house::BG.into()), fg: Some(house::LABEL.into()), icon_color: Some(house::icon_color(hue)), ..Look::default() }
}

fn binding(action: &str, params: Value, look: Look) -> Binding {
    Binding { action: action.into(), params, look, accel: false, look_on: None, swap_turns: false }
}

fn opt(id: &str, name: &str, does: &str, source: &str, binding: Binding, svg: Option<String>) -> BuilderOption {
    BuilderOption { id: id.into(), group: String::new(), name: name.into(), does: does.into(), source: source.into(), setup: false, binding, svg }
}

const MUSIC: &str = "Music";
const WORK: &str = "Work and focus";
const GLANCE: &str = "At a glance";
const CONTROL: &str = "One dial, many jobs";
const LOOKS: &str = "Looks";

/// The same option under a theme.
fn under(group: &str, mut o: BuilderOption) -> BuilderOption {
    o.group = group.into();
    o
}

/// The platform's command key in shortcuts.
fn cmd() -> &'static str {
    if cfg!(target_os = "macos") { "Cmd" } else { "Ctrl" }
}

fn redo() -> String {
    if cfg!(target_os = "macos") { "Cmd+Shift+Z".into() } else { "Ctrl+Y".into() }
}

const BUILT_IN: &str = "Built in, no setup";

/// A Stream Deck plugin's action installed here: (its action id, the plugin's name).
fn plugin_action(uuid: &str) -> Option<(String, String)> {
    plugins::actions().into_iter().find(|a| a.id.ends_with(&format!("/{uuid}"))).map(|a| (a.id.to_string(), a.group.to_string()))
}

/// What the deck shows for a sample song (the real cover when something plays now).
fn song_svg(look: &Look, title: &str, line: &str, ring: Option<f32>) -> String {
    match images::uri(images::COVER_REF) {
        Some(cover) => render::cover_dial_svg(&cover, title, line, ring, 0),
        None => render::now_playing_dial_svg(look, Some((title, line, true, None)), Some(0.4), 0),
    }
}

/// Options for the dial (it has a screen: each comes with a picture of it), by theme: what plays,
/// work and focus, things at a glance, one dial for several jobs, looks. The owner asked for more
/// ambition than a list of single actions (2026-10-13).
pub fn dial_options() -> Vec<BuilderOption> {
    let faces = gallery::previews();
    let face_svg = |id: &str| faces.iter().find(|p| p.face.id == id).map(|p| p.svg.clone());
    let mut out = Vec::new();

    // music: a plugin made for this when installed (Spotify's playback dial with the cover), then built-in
    if let Some((id, plugin)) = plugin_action("com.ntanis.essentials-for-spotify.playback-control-dial") {
        let mut b = binding(&id, json!({}), Look { image: Some(images::COVER_REF.into()), ..Look::default() });
        b.swap_turns = true;
        out.push(under(
            MUSIC,
            BuilderOption {
                setup: true,
                ..opt(
                    "spotifySong",
                    "Song with its cover",
                    "Turn: move in the song · hold and turn: songs · press: play",
                    &plugin,
                    b.clone(),
                    Some(song_svg(&b.look, "Song title", "1:08 / 3:59", Some(0.3))),
                )
            },
        ));
    }
    let np = look("Now playing", "music", house::hue::MEDIA);
    out.push(under(
        MUSIC,
        opt(
            "nowPlayingSeek",
            "What's playing",
            "Turn: move in the song · press: play or pause",
            if cfg!(target_os = "macos") { "Built in: Spotify and Music" } else { BUILT_IN },
            binding(actions::NOW_PLAYING, json!({"turn": "seek"}), np.clone()),
            Some(song_svg(&np, "Song title", "Artist", None)),
        ),
    ));
    out.push(under(
        MUSIC,
        opt(
            "nowPlayingTracks",
            "What's playing, turn for songs",
            "Turn: next or previous song · press: play or pause",
            BUILT_IN,
            binding(actions::NOW_PLAYING, json!({"turn": "tracks"}), np.clone()),
            Some(song_svg(&np, "Song title", "Artist", None)),
        ),
    ));
    if let Some((id, plugin)) = plugin_action("com.ntanis.essentials-for-spotify.my-playlists-dial") {
        let l = look("Playlists", "music", house::hue::MEDIA);
        out.push(under(
            MUSIC,
            BuilderOption {
                setup: true,
                ..opt(
                    "spotifyPlaylists",
                    "Your playlists",
                    "Turn: browse · press: play the one shown",
                    &plugin,
                    binding(&id, json!({}), l.clone()),
                    Some(render::time_dial_svg(&Look { icon: None, ..l }, "Chill Mix", Some("12 / 48"), None, 0)),
                )
            },
        ));
    }

    // work and focus
    let agents = look("Agents", "bot", house::hue::LIVE);
    out.push(under(
        WORK,
        opt(
            "agents",
            "Your Claude Code sessions",
            "Shows how many wait for you · turn: go through them · press: open it",
            "Built in, Claude Code hooks",
            binding(actions::AGENTS, json!({}), agents.clone()),
            Some(render::time_dial_svg(&Look { icon: None, ..agents }, "2", Some("waiting for you"), None, 0)),
        ),
    ));
    let history = look("History", "undo-2", house::hue::TOOL);
    out.push(under(
        WORK,
        opt(
            "history",
            "Undo and redo by turning",
            "Turn back: undo · turn on: redo, in any app, step by step through your changes",
            BUILT_IN,
            binding("core.hotkey", json!({"clockwise": redo(), "counterClockwise": format!("{}+Z", cmd())}), history.clone()),
            Some(render::time_dial_svg(&Look { icon: None, ..history }, "↶  ↷", Some("Undo · redo"), None, 0)),
        ),
    ));
    let focus = look("Focus", "timer", house::hue::LIVE);
    out.push(under(
        WORK,
        opt(
            "focus",
            "Focus timer",
            "25 minutes with a ring that empties · press: start or pause · turn: change the minutes",
            BUILT_IN,
            binding(actions::TIMER, json!({"minutes": 25, "seconds": 0, "press": "startPause", "sound": "on"}), focus.clone()),
            Some(render::time_dial_svg(&Look { icon: None, ..focus }, "18:42", Some("Focus"), Some(0.75), 0)),
        ),
    ));
    let meeting = look("Next", "calendar", house::hue::LIVE);
    out.push(under(
        WORK,
        BuilderOption {
            setup: true,
            ..opt(
                "meeting",
                "Next meeting",
                "Counts down to your next event",
                "Built in, your calendar's link (ICS)",
                binding(live::ACTION, json!({"source": "calendar", "url": ""}), meeting.clone()),
                Some(render::time_dial_svg(&Look { icon: None, ..meeting }, "in 18 min", Some("Standup"), None, 0)),
            )
        },
    ));
    let zoom = look("Zoom", "zoom-in", house::hue::TOOL);
    out.push(under(
        WORK,
        opt(
            "zoom",
            "Zoom in any app",
            "Turn: zoom in and out (design apps, browsers, documents)",
            BUILT_IN,
            binding("core.hotkey", json!({"clockwise": format!("{}+=", cmd()), "counterClockwise": format!("{}+-", cmd())}), zoom.clone()),
            Some(render::dial_svg(&zoom, Some((Some(0.6), "Zoom")), 0)),
        ),
    ));

    // at a glance
    out.push(under(
        GLANCE,
        opt(
            "clock",
            "Clock and date",
            "Shows the time and the day",
            BUILT_IN,
            binding(actions::CLOCK, json!({"show": "timeDate", "hours": "24"}), look("", "clock", house::hue::LIVE)),
            face_svg("clock"),
        ),
    ));
    let weather = look("Weather", "sun", house::hue::LIVE);
    out.push(under(
        GLANCE,
        BuilderOption {
            setup: true,
            ..opt(
                "weather",
                "Weather outside",
                "The temperature now where you are",
                "Built in, pick your city",
                binding(live::ACTION, json!({"source": "weather", "place": ""}), weather.clone()),
                Some(render::time_dial_svg(&Look { icon: None, ..weather }, "14°", Some("Warsaw, cloudy"), None, 0)),
            )
        },
    ));
    out.push(under(
        GLANCE,
        opt(
            "cpu",
            "Computer load",
            "CPU load as a ring · press: open the activity monitor",
            BUILT_IN,
            binding(actions::MONITOR, json!({"metric": "cpu"}), look("CPU", "zap", house::hue::LIVE)),
            face_svg("cpu"),
        ),
    ));

    // one dial, many jobs
    let modes = look("Modes", "layers", house::hue::NAV);
    out.push(under(
        CONTROL,
        opt(
            "modes",
            "Volume, scroll and zoom in one",
            "Turn: the current job · press: the next job, its name shows on the dial",
            BUILT_IN,
            binding(
                actions::MODES,
                json!({"modes": [
                    {"label": "Volume", "action": "core.volume", "params": {"target": "system", "step": 2}},
                    {"label": "Scroll", "action": "core.scroll", "params": {"axis": "vertical", "amount": 3}},
                    {"label": "Zoom", "action": "core.hotkey", "params": {"clockwise": format!("{}+=", cmd()), "counterClockwise": format!("{}+-", cmd())}},
                ]}),
                modes.clone(),
            ),
            Some(render::time_dial_svg(&Look { icon: None, ..modes }, "Volume", Some("press: Scroll · Zoom"), None, 0)),
        ),
    ));
    out.push(under(
        CONTROL,
        opt(
            "volume",
            "Volume, big",
            "Turn: system volume · press: mute",
            BUILT_IN,
            binding("core.volume", json!({"target": "system", "step": 2}), look("Volume", "volume-2", house::hue::SOUND)),
            face_svg("volume"),
        ),
    ));
    out.push(under(
        CONTROL,
        opt(
            "profiles",
            "Switch profiles",
            "Turn: next or previous profile, its name on the dial",
            BUILT_IN,
            binding("core.profile", json!({"which": "next"}), look("Profiles", "layers", house::hue::NAV)),
            face_svg("profile"),
        ),
    ));

    // looks: the animations, still a real dial underneath
    for (id, name) in [("aurora", "Aurora, turn for volume"), ("orbit", "Orbit, press to play")] {
        if let Some(f) = faces.iter().find(|p| p.face.id == id) {
            out.push(under(LOOKS, opt(id, name, "An animated picture that keeps moving; the dial still works", BUILT_IN, f.face.binding.clone(), Some(f.svg.clone()))));
        }
    }
    out
}

/// Options for a knob (no screen: what it changes shows on the dial for a moment).
pub fn knob_options() -> Vec<BuilderOption> {
    let mut out = vec![
        under(
            CONTROL,
            opt("volume", "System volume", "Turn: volume · press: mute", BUILT_IN, binding("core.volume", json!({"target": "system", "step": 2}), look("System", "volume-2", house::hue::SOUND)), None),
        ),
        under(
            MUSIC,
            opt(
                "tracks",
                "Songs",
                "Turn: next or previous song · press: play or pause",
                BUILT_IN,
                binding(actions::NOW_PLAYING, json!({"turn": "tracks"}), look("Songs", "music", house::hue::MEDIA)),
                None,
            ),
        ),
        under(
            MUSIC,
            opt(
                "seek",
                "Move in the song",
                "Turn: five seconds a step · press: play or pause",
                BUILT_IN,
                binding(actions::NOW_PLAYING, json!({"turn": "seek"}), look("Seek", "music", house::hue::MEDIA)),
                None,
            ),
        ),
        under(
            WORK,
            opt(
                "history",
                "Undo and redo",
                "Turn back: undo · turn on: redo, in any app",
                BUILT_IN,
                binding("core.hotkey", json!({"clockwise": redo(), "counterClockwise": format!("{}+Z", cmd())}), look("History", "undo-2", house::hue::TOOL)),
                None,
            ),
        ),
        under(
            WORK,
            opt(
                "zoom",
                "Zoom",
                "Turn: zoom in and out in any app",
                BUILT_IN,
                binding("core.hotkey", json!({"clockwise": format!("{}+=", cmd()), "counterClockwise": format!("{}+-", cmd())}), look("Zoom", "zoom-in", house::hue::TOOL)),
                None,
            ),
        ),
        under(
            CONTROL,
            opt(
                "mic",
                "Microphone level",
                "Turn: input level · press: mute the microphone",
                BUILT_IN,
                binding("core.volume", json!({"target": "mic", "step": 2}), look("Mic", "mic", house::hue::MIC)),
                None,
            ),
        ),
        under(
            CONTROL,
            opt(
                "scroll",
                "Scroll",
                "Turn: scroll the window under the pointer",
                BUILT_IN,
                binding("core.scroll", json!({"axis": "vertical", "amount": 3}), look("Scroll", "arrow-up-down", house::hue::TOOL)),
                None,
            ),
        ),
        under(
            CONTROL,
            opt("profiles", "Switch profiles", "Turn: next or previous profile", BUILT_IN, binding("core.profile", json!({"which": "next"}), look("Profiles", "layers", house::hue::NAV)), None),
        ),
    ];
    if let Some((id, plugin)) = plugin_action("com.ntanis.essentials-for-spotify.volume-control-dial") {
        out.insert(
            1,
            under(
                MUSIC,
                BuilderOption {
                    setup: true,
                    ..opt("spotifyVolume", "Spotify volume", "Turn: Spotify's own volume · press: mute", &plugin, binding(&id, json!({}), look("Spotify", "volume-2", house::hue::MEDIA)), None)
                },
            ),
        );
    }
    if let Some((id, plugin)) = plugin_action("com.ntanis.essentials-for-spotify.my-playlists-dial") {
        out.push(under(
            MUSIC,
            BuilderOption {
                setup: true,
                ..opt(
                    "spotifyPlaylists",
                    "Your playlists",
                    "Turn: browse (the dial shows the name) · press: play",
                    &plugin,
                    binding(&id, json!({}), look("Playlists", "music", house::hue::MEDIA)),
                    None,
                )
            },
        ));
    }
    out
}

/// The options for a kind of control (`dial`, `knobs`).
#[tauri::command]
pub fn builder_options(kind: String) -> Vec<BuilderOption> {
    match kind.as_str() {
        "dial" => dial_options(),
        "knobs" => knob_options(),
        _ => Vec::new(),
    }
}

/// The working draft: a copy of the profile on the deck now (or empty), kept out of the bar and
/// of switching until it is saved.
#[tauri::command]
pub fn builder_draft(engine: State<'_, Engine>, from: Option<String>, name: String) -> Result<Profile, String> {
    let mut profile = Profile::new(name.trim());
    if let Some(src) = from.and_then(|id| engine.lock().store.profiles.get(&id).cloned()) {
        profile.controls = src.controls;
        profile.pages = src.pages;
    }
    {
        let mut s = engine.lock();
        s.store.save_profile(profile.clone()).map_err(|e| e.to_string())?;
        s.store.settings.drafts.push(profile.id.clone());
        s.store.save_settings().map_err(|e| e.to_string())?;
    }
    engine.changed();
    Ok(profile)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_are_real_settings() {
        let catalog = actions::catalog();
        for o in dial_options().into_iter().chain(knob_options()) {
            assert!(catalog.iter().any(|a| a.id == o.binding.action) || plugins::is_plugin_action(&o.binding.action), "{}: unknown action {}", o.id, o.binding.action);
            assert!(!o.does.is_empty() && !o.source.is_empty());
        }
        assert!(dial_options().iter().all(|o| o.svg.as_deref().is_some_and(|s| s.starts_with("<svg"))), "every dial option has a picture");
    }
}
