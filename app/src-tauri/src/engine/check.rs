//! Import check: controls of a profile that point at something this computer does not have (a
//! program, file or script path, a picture, an audio device, a MIDI port). Shown as a short list
//! with a "check this" mark, so a profile made on another PC is fixed in a minute, not debugged.

use serde::Serialize;

use super::profile::Profile;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    /// Control id (`keys.3`, `p2/keys.0`, `dial`, `keys.1:hold`).
    pub control: String,
    pub message: String,
}

/// What this computer has; device lists are None when not looked up (nothing needs them).
pub struct Env<'a> {
    pub file_exists: &'a dyn Fn(&str) -> bool,
    pub image_exists: &'a dyn Fn(&str) -> bool,
    pub audio_devices: Option<Vec<String>>,
    pub midi_ports: Option<Vec<String>>,
    /// (scenes, audio inputs) of OBS while it is connected.
    pub obs: Option<(Vec<String>, Vec<String>)>,
    /// Home Assistant entities it does not know.
    pub ha_missing: Vec<String>,
    /// A bare name opens something here: a program on PATH, an installed app or game, an app seen
    /// in front ("notepad", "Visual Studio Code").
    pub name_opens: &'a dyn Fn(&str) -> bool,
}

/// A target that is a path (not a URL, not a bare program name found through PATH).
fn is_path(s: &str) -> bool {
    let s = s.trim().trim_matches('"');
    // shell:AppsFolder\<app id> opens a Microsoft Store app: a link, not a file (2026-10-19: every
    // Spotify key was "Not found on this PC")
    !s.contains("://") && !s.to_lowercase().starts_with("shell:") && (s.contains('\\') || s.contains('/'))
}

/// Does the profile use audio devices or MIDI ports (so they are worth listing)?
pub fn needs(p: &Profile) -> (bool, bool) {
    let any = |pre: &str| p.controls.values().any(|b| b.action.starts_with(pre));
    (p.controls.values().any(|b| b.action == "core.audioDevice" || (b.action == "core.sound" && !super::actions::param_str(&b.params, "device").is_empty())), any("midi."))
}

/// Does a bare name open something on this computer (as `core.launch` would, `exec.rs`): a
/// program on the PATH or in the registered App Paths (`notepad`, `chrome`), an installed app or
/// game by its name, an app seen in front.
pub fn name_opens(name: &str) -> bool {
    let n = name.trim();
    if super::installed::item_named(n).is_some() || super::apps::seen_named(n).is_some() {
        return true;
    }
    let exe = if n.to_lowercase().ends_with(".exe") { n.to_string() } else { format!("{n}.exe") };
    let on_path = std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(&exe).is_file() || d.join(n).is_file()));
    on_path || app_path_registered(&exe)
}

#[cfg(windows)]
fn app_path_registered(exe: &str) -> bool {
    use std::os::windows::process::CommandExt;
    ["HKLM", "HKCU"].iter().any(|hive| {
        std::process::Command::new("reg")
            .args(["query", &format!("{hive}\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\App Paths\\{exe}")])
            .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    })
}

#[cfg(not(windows))]
fn app_path_registered(_exe: &str) -> bool {
    false
}

pub fn issues(p: &Profile, env: &Env) -> Vec<Issue> {
    let mut out = Vec::new();
    for (control, b) in &p.controls {
        let param = |k: &str| super::actions::param_str(&b.params, k).trim().trim_matches('"').to_string();
        let mut flag = |message: String| out.push(Issue { control: control.clone(), message });
        // a bare name a key opens (not a path, not a URL or shell: link) must name something here
        // (session 6: sketches named apps that did not open)
        let opens = match b.action.as_str() {
            "core.launch" => param("target"),
            "core.game" => param("game"),
            _ => String::new(),
        };
        let bare_name = !opens.is_empty() && !is_path(&opens) && !opens.contains(':');
        match b.action.as_str() {
            "core.launch" if is_path(&param("target")) && !(env.file_exists)(&param("target")) => flag(format!("Not found on this PC: {}", param("target"))),
            "core.launch" | "core.game" if bare_name && !(env.name_opens)(&opens) => flag(format!("Not found on this PC: {opens}")),
            "core.run" if is_path(&param("command")) && !(env.file_exists)(&param("command")) => flag(format!("Program or script not found: {}", param("command"))),
            "core.sound" if !param("file").is_empty() && !(env.file_exists)(&param("file")) => flag(format!("Sound file not found: {}", param("file"))),
            _ => {}
        }
        let device = param("device");
        if (b.action == "core.audioDevice" || b.action == "core.sound")
            && !device.is_empty()
            && let Some(list) = &env.audio_devices
            && !list.iter().any(|d| d.eq_ignore_ascii_case(&device))
        {
            flag(format!("No audio device named {device}"));
        }
        let port = param("port");
        if b.action.starts_with("midi.")
            && !port.is_empty()
            && let Some(list) = &env.midi_ports
            && !list.iter().any(|d| d.eq_ignore_ascii_case(&port))
        {
            flag(format!("No MIDI port named {port}"));
        }
        if let Some((scenes, inputs)) = &env.obs {
            let scene = param("scene");
            if b.action == "obs.scene" && !scene.is_empty() && !scenes.contains(&scene) {
                flag(format!("OBS has no scene named {scene}"));
            }
            let input = param("input");
            if (b.action == "obs.mute" || b.action == "obs.volume") && !input.is_empty() && !inputs.contains(&input) {
                flag(format!("OBS has no audio source named {input}"));
            }
        }
        let entity = param("entity");
        if b.action.starts_with("ha.") && !entity.is_empty() && env.ha_missing.contains(&entity) {
            flag(format!("Home Assistant does not know {entity}"));
        }
        if let Some(image) = b.look.image.as_deref().filter(|i| !i.is_empty() && *i != super::images::COVER_REF)
            && !(env.image_exists)(image)
        {
            flag("Its picture is missing".into());
        }
        // a shortcut the assistant chose that is not in a checked pack for the profile's apps
        if p.assistant {
            for combo in super::guard::unchecked_shortcuts(&b.action, &b.params, &p.apps) {
                flag(format!("Shortcut not checked for this app: {combo}"));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::profile::{Binding, Look};
    use serde_json::json;

    fn bind(action: &str, params: serde_json::Value) -> Binding {
        Binding { action: action.into(), params, look: Look::default(), accel: false, look_on: None, swap_turns: false }
    }

    #[test]
    fn finds_what_this_pc_lacks() {
        let mut p = Profile::new("Imported");
        p.controls.insert("keys.0".into(), bind("core.launch", json!({"target": r"C:\Program Files\Nope\nope.exe"})));
        p.controls.insert("keys.1".into(), bind("core.launch", json!({"target": "https://example.com"})));
        p.controls.insert("keys.2".into(), bind("core.run", json!({"command": "notepad.exe"})));
        p.controls.insert("keys.3".into(), bind("core.audioDevice", json!({"kind": "output", "device": "Old Headset"})));
        p.controls.insert("knobs.0".into(), bind("midi.cc", json!({"port": "loopMIDI Port", "cc": 1})));
        let mut pic = bind("core.media", json!({}));
        pic.look.image = Some("gone.png".into());
        p.controls.insert("keys.4".into(), pic);
        let env = Env {
            file_exists: &|_| false,
            image_exists: &|_| false,
            audio_devices: Some(vec!["Speakers (Realtek)".into()]),
            midi_ports: Some(vec!["loopMIDI Port".into()]),
            obs: Some((vec!["Gaming".into()], vec!["Mic/Aux".into()])),
            ha_missing: vec!["light.gone".into()],
            name_opens: &|n| n == "notepad",
        };
        p.controls.insert("keys.5".into(), bind("obs.scene", json!({"scene": "Gaming"})));
        p.controls.insert("keys.6".into(), bind("obs.scene", json!({"scene": "Old scene"})));
        p.controls.insert("keys.7".into(), bind("obs.mute", json!({"input": "Mic/Aux"})));
        p.controls.insert("keys.8".into(), bind("ha.toggle", json!({"entity": "light.gone"})));
        p.controls.insert("keys.9".into(), bind("core.launch", json!({"target": "notepad"})));
        p.controls.insert("keys.10".into(), bind("core.launch", json!({"target": "Habit Tracker v2"})));
        p.controls.insert("keys.11".into(), bind("core.game", json!({"game": "steam://rungameid/1"})));
        p.controls.insert("keys.12".into(), bind("core.game", json!({"game": "Path of Exile 2"})));
        // a Microsoft Store app opens through its shell: link
        p.controls.insert("keys.13".into(), bind("core.launch", json!({"target": r"shell:AppsFolder\SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify"})));
        let all = issues(&p, &env);
        let found: Vec<&str> = all.iter().map(|i| i.control.as_str()).collect();
        assert_eq!(found, ["keys.0", "keys.10", "keys.12", "keys.3", "keys.4", "keys.6", "keys.8"], "URLs, names that open, existing ports, scenes and inputs are fine");
        assert_eq!(needs(&p), (true, true));
    }
}
