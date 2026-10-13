//! Showcase profiles: what a profile looks like when it is designed with care (owner's brief,
//! 2026-10-12: polished effects, pictures and animations, "top level"). Each one has one idea,
//! one accent color, consistent icons and lights that mean something. Pictures are our own art
//! (`resources/showcase/`, made by `prototype/tools/gen_showcase.py`), copied into the images
//! folder when the profile is added.

use serde_json::json;

use super::profile::{Binding, Look, Profile};

const ART: [(&str, &[u8]); 21] = [
    ("scene-start.png", include_bytes!("../../resources/showcase/scene-start.png")),
    ("scene-chat.png", include_bytes!("../../resources/showcase/scene-chat.png")),
    ("scene-game.png", include_bytes!("../../resources/showcase/scene-game.png")),
    ("scene-brb.png", include_bytes!("../../resources/showcase/scene-brb.png")),
    ("scene-end.png", include_bytes!("../../resources/showcase/scene-end.png")),
    ("equalizer.gif", include_bytes!("../../resources/showcase/equalizer.gif")),
    ("sky-0.png", include_bytes!("../../resources/showcase/sky-0.png")),
    ("sky-1.png", include_bytes!("../../resources/showcase/sky-1.png")),
    ("sky-2.png", include_bytes!("../../resources/showcase/sky-2.png")),
    ("sky-3.png", include_bytes!("../../resources/showcase/sky-3.png")),
    ("sky-4.png", include_bytes!("../../resources/showcase/sky-4.png")),
    ("sky-5.png", include_bytes!("../../resources/showcase/sky-5.png")),
    ("sky-6.png", include_bytes!("../../resources/showcase/sky-6.png")),
    ("sky-7.png", include_bytes!("../../resources/showcase/sky-7.png")),
    ("sky-8.png", include_bytes!("../../resources/showcase/sky-8.png")),
    ("sky-9.png", include_bytes!("../../resources/showcase/sky-9.png")),
    ("sky-10.png", include_bytes!("../../resources/showcase/sky-10.png")),
    ("sky-11.png", include_bytes!("../../resources/showcase/sky-11.png")),
    ("sky-12.png", include_bytes!("../../resources/showcase/sky-12.png")),
    ("sky-13.png", include_bytes!("../../resources/showcase/sky-13.png")),
    ("sky-14.png", include_bytes!("../../resources/showcase/sky-14.png")),
];

/// Copy a picture into the images folder; None in tests (no images folder there).
pub(super) fn art(name: &str) -> Option<String> {
    let (_, bytes) = ART.iter().find(|(n, _)| *n == name)?;
    super::images::import_bytes(bytes, name.rsplit('.').next().unwrap_or("png")).ok()
}

/// A key or control: action, params, label, icon, background, text color and style.
fn key(action: &str, params: serde_json::Value, label: &str, icon: &str, bg: &str, style: &str) -> Binding {
    Binding {
        action: action.into(),
        params,
        look: Look { label: Some(label.into()), icon: Some(icon.into()), bg: Some(bg.into()), fg: Some("#ffffff".into()), style: Some(style.into()), ..Look::default() },
        accel: false,
        look_on: None,
        swap_turns: false,
    }
}

/// A turn control with its LED color; the LED's brightness shows the value.
fn turn(action: &str, params: serde_json::Value, label: &str, icon: &str, led: &str) -> Binding {
    Binding {
        action: action.into(),
        params,
        look: Look { label: Some(label.into()), icon: Some(icon.into()), led: Some(led.into()), led_level: true, ..Look::default() },
        accel: true,
        look_on: None,
        swap_turns: false,
    }
}

fn with_image(mut b: Binding, image: &str) -> Binding {
    b.look.image = art(image);
    b
}

fn put(p: &mut Profile, id: &str, b: Binding) {
    p.controls.insert(id.into(), b);
}

fn volume(app: &str) -> serde_json::Value {
    if app.is_empty() { json!({"target": "system", "step": 2}) } else { json!({"target": "app", "app": app, "step": 2}) }
}

/// "Stream live": five scene cards across the top row, going live and recording in red, Twitch
/// markers and clips, the mixer on the faders; the base edge breathes red while live (the
/// "on air" reaction of the light settings).
pub fn stream_live() -> Profile {
    let mut p = Profile::new("Stream live");
    p.apps = vec!["obs64.exe".into(), "obs.exe".into()];
    p.color = Some("#e11d48".into());
    p.edge = Some("#7c3aed".into());
    p.edge_effect = Some("static".into());
    // (OBS scene name, short key label, art)
    let scenes = [("Starting soon", "Starting", "start"), ("Just chatting", "Chatting", "chat"), ("Gameplay", "Gameplay", "game"), ("Be right back", "BRB", "brb"), ("Ending", "Ending", "end")];
    for (i, (scene, label, art_name)) in scenes.iter().enumerate() {
        put(&mut p, &format!("keys.{i}"), with_image(key("obs.scene", json!({"scene": scene}), label, "monitor", "#141018", "flat"), &format!("scene-{art_name}.png")));
    }
    let live = "#3b0d18";
    put(&mut p, "keys.5", key("obs.stream", json!({}), "Go live", "radio", live, "glass"));
    put(&mut p, "keys.6", key("obs.record", json!({}), "Record", "circle-dot", live, "glass"));
    put(&mut p, "keys.7", key("obs.replay", json!({}), "Replay", "rocket", "#2a1a3d", "glass"));
    put(&mut p, "keys.8", key("obs.mute", json!({"input": "Mic/Aux", "mode": "toggle"}), "Mic", "mic", "#1d2440", "glass"));
    put(&mut p, "keys.9", key("obs.source", json!({"scene": "", "source": "Video Capture Device"}), "Camera", "video", "#1d2440", "glass"));
    put(&mut p, "keys.10", key("twitch.marker", json!({"description": "Highlight"}), "Marker", "star", "#24183a", "glass"));
    put(&mut p, "keys.11", key("twitch.clip", json!({}), "Clip", "camera", "#24183a", "glass"));
    put(&mut p, "keys.12", key("obs.screenshot", json!({}), "Shot", "camera", "#24183a", "glass"));
    put(&mut p, "keys.13", key("core.media", json!({"command": "playPause"}), "Music", "music", "#10261a", "glass"));
    put(&mut p, "keys.14", key("core.launch", json!({"target": "https://dashboard.twitch.tv"}), "Dashboard", "globe", "#10261a", "glass"));
    put(&mut p, "dial", turn("obs.volume", json!({"input": "Mic/Aux", "step": 1}), "Mic level", "mic", "#a78bfa"));
    for (i, (app, label, led)) in [("Spotify.exe", "Music", "#22c55e"), ("Discord.exe", "Discord", "#6366f1"), ("", "System", "#e5e7eb")].into_iter().enumerate() {
        put(&mut p, &format!("knobs.{i}"), turn("core.volume", volume(app), label, "volume-2", led));
    }
    let mixer = [("Mic/Aux", "Mic", "#a78bfa"), ("Desktop Audio", "Desktop", "#38bdf8")];
    for (i, (input, label, led)) in mixer.into_iter().enumerate() {
        put(&mut p, &format!("faders.{i}"), turn("obs.volume", json!({"input": input, "step": 1}), label, "volume-2", led));
    }
    for (i, (app, label, led)) in [("Spotify.exe", "Music", "#22c55e"), ("Discord.exe", "Discord", "#6366f1"), ("", "System", "#e5e7eb")].into_iter().enumerate() {
        put(&mut p, &format!("faders.{}", i + 2), turn("core.volume", volume(app), label, "volume-2", led));
    }
    put(&mut p, "rollers.0", turn("core.scroll", json!({"axis": "vertical", "amount": 3}), "Chat", "arrow-up-down", "#7c3aed"));
    put(&mut p, "rollers.1", turn("obs.volume", json!({"input": "Desktop Audio", "step": 1}), "Desktop", "volume-2", "#38bdf8"));
    p
}

/// "Night focus": a quiet night sky across all keys with the controls drawn on it, a 25-minute
/// focus timer on the dial (its ring shows the time left), breaks, lo-fi music, a calm indigo
/// edge that breathes slowly.
pub fn night_focus() -> Profile {
    let mut p = Profile::new("Night focus");
    p.color = Some("#6366f1".into());
    p.edge = Some("#4f46e5".into());
    p.edge_effect = Some("breathing".into());
    p.edge_speed = Some(15);
    let keys: [(&str, serde_json::Value, &str, &str); 15] = [
        ("core.timer", json!({"minutes": 25, "seconds": 0, "press": "startPause", "sound": "on"}), "Focus 25", "timer"),
        ("core.timer", json!({"minutes": 5, "seconds": 0, "press": "startPause", "sound": "on"}), "Break 5", "moon"),
        ("core.launch", json!({"target": "spotify:playlist:37i9dQZF1DWWQRwui0ExPn"}), "Lo-fi", "music"),
        ("core.media", json!({"command": "playPause"}), "Play", "play"),
        ("core.media", json!({"command": "next"}), "Next", "skip-forward"),
        ("core.mic", json!({}), "Mic", "mic"),
        ("core.launch", json!({"target": "notepad"}), "Notes", "type"),
        ("core.hotkey", json!({"keys": "Win+Shift+S"}), "Snip", "camera"),
        ("core.clock", json!({"show": "time", "hours": "24"}), "Clock", "clock"),
        ("core.hotkey", json!({"keys": "Win+L"}), "Lock", "lock"),
        ("core.lighting", json!({"target": "all", "effect": "water", "color": "#6366f1", "speed": 20}), "Calm", "sparkles"),
        ("core.deckBrightness", json!({"press": "darker", "step": 20}), "Dim", "sun"),
        ("core.hotkey", json!({"keys": "Win+D"}), "Desktop", "monitor"),
        ("core.hotkey", json!({"keys": "Win+N"}), "Notices", "bell"),
        ("core.profile", json!({"which": "next"}), "Next", "layers"),
    ];
    for (i, (action, params, label, icon)) in keys.into_iter().enumerate() {
        put(&mut p, &format!("keys.{i}"), with_image(key(action, params, label, icon, "#0b0c1a", "flat"), &format!("sky-{i}.png")));
    }
    put(&mut p, "dial", turn("core.timer", json!({"minutes": 25, "seconds": 0, "press": "startPause", "sound": "on"}), "Focus", "timer", "#818cf8"));
    for (i, (app, label, led)) in [("Spotify.exe", "Music", "#818cf8"), ("", "System", "#c7d2fe"), ("chrome.exe", "Browser", "#6366f1")].into_iter().enumerate() {
        put(&mut p, &format!("knobs.{i}"), turn("core.volume", volume(app), label, "volume-2", led));
    }
    for (i, (app, label)) in [("", "System"), ("Spotify.exe", "Music"), ("chrome.exe", "Browser"), ("Discord.exe", "Chat"), ("", "In front")].into_iter().enumerate() {
        let params = if i == 4 { json!({"target": "front", "step": 2}) } else { volume(app) };
        put(&mut p, &format!("faders.{i}"), turn("core.volume", params, label, "volume-2", "#6366f1"));
    }
    put(&mut p, "rollers.0", turn("core.scroll", json!({"axis": "vertical", "amount": 3}), "Scroll", "arrow-up-down", "#6366f1"));
    put(&mut p, "rollers.1", turn("core.scroll", json!({"axis": "horizontal", "amount": 3}), "Sideways", "arrow-left-right", "#6366f1"));
    p
}

/// "Spotify studio": the track on the dial with its cover, like it with a heart that stays red,
/// add it to a playlist, an animated equalizer on the app key; Spotify green throughout.
pub fn spotify_studio() -> Profile {
    let mut p = Profile::new("Spotify studio");
    p.apps = vec!["Spotify.exe".into()];
    p.color = Some("#1db954".into());
    p.edge = Some("#1db954".into());
    p.edge_effect = Some("static".into());
    let g = "#0d2a19";
    let keys: [(&str, serde_json::Value, &str, &str, &str); 15] = [
        ("core.media", json!({"command": "previous"}), "Back", "skip-back", g),
        ("core.media", json!({"command": "playPause"}), "Play", "play", "#14532d"),
        ("core.media", json!({"command": "next"}), "Next", "skip-forward", g),
        ("spotify.like", json!({}), "Like", "heart", "#3b1220"),
        ("spotify.add", json!({"playlist": ""}), "Save to", "list-checks", g),
        ("core.launch", json!({"target": "spotify:"}), "Spotify", "music", "#06140c"),
        ("core.hotkey", json!({"keys": "Ctrl+S"}), "Shuffle", "refresh-cw", g),
        ("core.hotkey", json!({"keys": "Ctrl+R"}), "Repeat", "refresh-cw", g),
        ("core.hotkey", json!({"keys": "Ctrl+L"}), "Search", "search", g),
        ("spotify.remove", json!({}), "Remove", "x", "#2a1414"),
        (super::actions::NOW_PLAYING, json!({}), "Now", "music", "#06140c"),
        ("core.volume", json!({"target": "app", "app": "Spotify.exe"}), "Mute", "volume-x", g),
        ("core.launch", json!({"target": "spotify:collection:tracks"}), "Liked", "heart", g),
        ("core.launch", json!({"target": "spotify:"}), "Library", "layers", g),
        ("core.profile", json!({"which": "next"}), "Next set", "layers", "#1f1d26"),
    ];
    for (i, (action, params, label, icon, bg)) in keys.into_iter().enumerate() {
        let b = key(action, params, label, icon, bg, "glass");
        put(&mut p, &format!("keys.{i}"), if i == 5 { with_image(b, "equalizer.gif") } else { b });
    }
    put(&mut p, "dial", turn(super::actions::NOW_PLAYING, json!({}), "Now playing", "music", "#1db954"));
    for (i, (app, label, led)) in [("Spotify.exe", "Spotify", "#1db954"), ("", "System", "#e5e7eb"), ("chrome.exe", "Browser", "#86efac")].into_iter().enumerate() {
        put(&mut p, &format!("knobs.{i}"), turn("core.volume", volume(app), label, "volume-2", led));
    }
    for (i, (app, label)) in [("Spotify.exe", "Spotify"), ("", "System"), ("chrome.exe", "Browser"), ("Discord.exe", "Discord"), ("", "In front")].into_iter().enumerate() {
        let params = if i == 4 { json!({"target": "front", "step": 2}) } else { volume(app) };
        put(&mut p, &format!("faders.{i}"), turn("core.volume", params, label, "volume-2", "#1db954"));
    }
    put(&mut p, "rollers.0", turn("core.seek", json!({"player": "spotify"}), "Seek", "skip-forward", "#1db954"));
    put(&mut p, "rollers.1", turn("core.volume", volume("Spotify.exe"), "Volume", "volume-2", "#1db954"));
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::actions;

    #[test]
    fn showcase_profiles_use_real_actions_and_fill_the_kit() {
        for p in [stream_live(), night_focus(), spotify_studio()] {
            assert_eq!(p.controls.keys().filter(|k| k.starts_with("keys.")).count(), 15, "{}", p.name);
            assert!(p.controls.contains_key("dial") && p.controls.contains_key("knobs.2") && p.controls.contains_key("faders.4"), "{}", p.name);
            for (control, b) in &p.controls {
                assert!(actions::find(&b.action).is_some(), "{}: {control} uses unknown action {}", p.name, b.action);
            }
        }
        assert!(ART.iter().all(|(_, bytes)| !bytes.is_empty()));
    }
}
