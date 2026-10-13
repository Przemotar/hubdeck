//! Suggestions for one control (owner, 2026-10-12: "with so many options, how does a user know
//! what to set?"): what the profile's app preset puts on that kind of control, then the starter for
//! its kind of app (game, browser, editor...), then generally useful ones; never one the profile
//! already has. "Fill empty keys" puts the key suggestions on the free keys of a page.

use serde_json::{Value, json};
use tauri::State;

use super::Engine;
use super::profile::{Binding, Look, Profile};
use super::starter::{self, AppKind};

/// How many suggestions a control shows.
const MAX: usize = 8;

/// The kind of control an id is for: `keys`, `dial`, `knobs`, `faders`, `rollers` (page and
/// gesture parts dropped: `p2/keys.3:hold` -> `keys`).
fn kind_of_control(control: &str) -> &str {
    let base = control.rsplit('/').next().unwrap_or(control);
    let base = base.split(':').next().unwrap_or(base);
    base.split('.').next().unwrap_or(base)
}

/// The plain controls of one kind in a profile, in their order (`keys.2` before `keys.10`).
fn controls_of(p: &Profile, kind: &str) -> Vec<Binding> {
    let mut found: Vec<(usize, Binding)> = p
        .controls
        .iter()
        .filter(|(id, _)| !id.contains('/') && !id.contains(':') && kind_of_control(id) == kind)
        .map(|(id, b)| (id.rsplit('.').next().and_then(|n| n.parse().ok()).unwrap_or(0), b.clone()))
        .collect();
    found.sort_by_key(|(i, _)| *i);
    found.into_iter().map(|(_, b)| b).collect()
}

fn bind(action: &str, params: Value, label: &str, icon: &str) -> Binding {
    Binding { action: action.into(), params, look: Look { label: Some(label.into()), icon: Some(icon.into()), ..Look::default() }, accel: false, look_on: None, swap_turns: false }
}

/// An action with its default inputs (from the catalog) and the given look.
fn with_defaults(action: &str, label: &str, icon: &str) -> Option<Binding> {
    let def = super::actions::catalog().into_iter().find(|a| a.id == action)?;
    let params: serde_json::Map<String, Value> = def.params.iter().map(|p| (p.key.to_string(), p.default.clone())).collect();
    Some(bind(action, Value::Object(params), label, icon))
}

fn volume(target: &str, app: &str, label: &str, icon: &str) -> Binding {
    bind("core.volume", json!({"target": target, "app": app, "step": 2}), label, icon)
}

/// Useful on any profile, by kind of control.
fn general(kind: &str) -> Vec<Binding> {
    let media = |command: &str, label: &str, icon: &str| bind("core.media", json!({ "command": command }), label, icon);
    let hotkey = |keys: &str, label: &str, icon: &str| bind("core.hotkey", json!({ "keys": keys }), label, icon);
    let mut out = match kind {
        "keys" => vec![
            media("playPause", "Play", "play"),
            media("next", "Next", "skip-forward"),
            media("previous", "Prev", "skip-back"),
            bind("core.mic", json!({"mode": "toggle"}), "Mic", "mic"),
            hotkey("VolumeMute", "Mute", "volume-x"),
            hotkey("Win+Shift+S", "Snip", "camera"),
            hotkey("Win+D", "Desktop", "monitor"),
            hotkey("Win+L", "Lock", "lock"),
        ],
        "dial" => vec![volume("system", "", "Volume", "volume-2"), volume("front", "", "App volume", "volume-2")],
        "knobs" => vec![
            volume("system", "", "System", "volume-2"),
            volume("front", "", "App", "volume-2"),
            volume("mic", "", "Mic level", "mic"),
            volume("app", "Discord.exe", "Discord", "headphones"),
            volume("app", "Spotify.exe", "Music", "music"),
        ],
        "faders" => vec![
            volume("system", "", "System", "volume-2"),
            volume("front", "", "App", "volume-2"),
            volume("app", "Discord.exe", "Discord", "headphones"),
            volume("app", "Spotify.exe", "Music", "music"),
            volume("app", "chrome.exe", "Browser", "globe"),
            volume("mic", "", "Mic level", "mic"),
        ],
        "rollers" => vec![
            bind("core.scroll", json!({"axis": "vertical", "amount": 3}), "Scroll", "arrow-up-down"),
            bind("core.scroll", json!({"axis": "horizontal", "amount": 3}), "Scroll ↔", "arrow-left-right"),
            bind("core.hotkey", json!({"keys": "", "clockwise": "Ctrl+=", "counterClockwise": "Ctrl+-"}), "Zoom", "zoom-in"),
            volume("system", "", "Volume", "volume-2"),
        ],
        _ => Vec::new(),
    };
    let extra: &[(&str, &str, &str)] = match kind {
        "keys" => &[(super::actions::CLOCK, "Clock", "clock"), (super::actions::TIMER, "Timer", "timer")],
        "dial" => &[(super::actions::NOW_PLAYING, "Now playing", "music"), (super::actions::CLOCK, "Clock", "clock"), ("core.brightness", "Brightness", "sun")],
        "knobs" => &[("core.brightness", "Brightness", "sun")],
        _ => &[],
    };
    out.extend(extra.iter().filter_map(|(a, l, i)| with_defaults(a, l, i)));
    if kind == "dial" {
        out.push(bind("core.scroll", json!({"axis": "vertical", "amount": 3}), "Scroll", "arrow-up-down"));
    }
    out
}

/// The same job: the same action with the same inputs.
fn same_job(a: &Binding, b: &Binding) -> bool {
    a.action == b.action && a.params == b.params
}

/// Suggestions for `control` of `profile`, best first, none the profile already has.
pub fn for_control(profile: &Profile, control: &str) -> Vec<Binding> {
    let kind = kind_of_control(control);
    let mut out: Vec<Binding> = Vec::new();
    for app in &profile.apps {
        let exe = app.split(':').next().unwrap_or(app).trim();
        if exe.is_empty() {
            continue;
        }
        if let Some(preset) = starter::preset_for(exe) {
            out.extend(controls_of(&preset, kind));
        }
        let app_kind = if starter::is_game(exe, "") { AppKind::Game } else { starter::kind_of(exe) };
        let name = exe.trim_end_matches(".exe").trim_end_matches(".EXE");
        out.extend(controls_of(&starter::starter(app_kind, exe, name, exe, None), kind));
    }
    out.extend(general(kind).into_iter().map(|mut b| {
        let (action, params) = (b.action.clone(), b.params.clone());
        super::house::apply_look(&mut b.look, &action, &params);
        b
    }));
    let used: Vec<&Binding> = profile.controls.values().collect();
    let mut picked: Vec<Binding> = Vec::new();
    for b in out {
        if b.action.is_empty() || used.iter().any(|u| same_job(u, &b)) || picked.iter().any(|p| same_job(p, &b)) {
            continue;
        }
        picked.push(b);
        if picked.len() == MAX {
            break;
        }
    }
    // in the window's language, like the profiles HubDeck makes
    picked.into_iter().map(super::localize::binding).collect()
}

/// Suggestions for one control of a stored profile.
#[tauri::command]
pub fn suggestions(engine: State<'_, Engine>, profile: String, control: String) -> Result<Vec<Binding>, String> {
    let p = engine.lock().store.profiles.get(&profile).cloned().ok_or_else(|| format!("no profile {profile}"))?;
    Ok(for_control(&p, &control))
}

/// Put suggestions on the free keys of a page (`page`: None = the main page), in key order;
/// returns how many keys were filled.
pub fn fill_keys(profile: &mut Profile, page: Option<&str>) -> usize {
    let id = |i: usize| match page {
        Some(pg) => format!("{pg}/keys.{i}"),
        None => format!("keys.{i}"),
    };
    let free: Vec<usize> = (0..15).filter(|i| !profile.controls.contains_key(&id(*i))).collect();
    let mut filled = 0;
    for i in free {
        let Some(b) = for_control(profile, &id(i)).into_iter().next() else { break };
        profile.controls.insert(id(i), b);
        filled += 1;
    }
    filled
}

/// "Fill empty keys" in the profile panel.
#[tauri::command]
pub fn profile_fill(engine: State<'_, Engine>, profile: String, page: Option<String>) -> Result<usize, String> {
    let filled = {
        let mut s = engine.lock();
        let mut p = s.store.profiles.get(&profile).cloned().ok_or_else(|| format!("no profile {profile}"))?;
        let filled = fill_keys(&mut p, page.as_deref().filter(|p| !p.is_empty()));
        if filled > 0 {
            s.store.save_profile(p).map_err(|e| e.to_string())?;
        }
        filled
    };
    if filled > 0 {
        engine.changed();
    }
    Ok(filled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::render;

    #[test]
    fn control_kinds_from_ids() {
        assert_eq!(kind_of_control("keys.3"), "keys");
        assert_eq!(kind_of_control("p2/keys.3:hold"), "keys");
        assert_eq!(kind_of_control("dial"), "dial");
        assert_eq!(kind_of_control("faders.4"), "faders");
    }

    #[test]
    fn suggestions_come_from_the_app_then_general_and_skip_what_is_there() {
        let mut p = Profile::new("Chrome");
        p.apps = vec!["chrome.exe".into()];
        let keys = for_control(&p, "keys.5");
        assert!(!keys.is_empty() && keys.len() <= MAX);
        // a browser gets browser keys first
        assert!(keys.iter().take(4).any(|b| b.look.label.as_deref() == Some("New tab")), "{:?}", keys.iter().map(|b| b.look.label.clone()).collect::<Vec<_>>());
        // what the profile has is not suggested again
        p.controls.insert("keys.0".into(), keys[0].clone());
        assert!(!for_control(&p, "keys.5").iter().any(|b| same_job(b, &keys[0])));
        // every kind of control gets some, all drawable
        for control in ["keys.0", "dial", "knobs.1", "faders.2", "rollers.0"] {
            let s = for_control(&Profile::new("Empty"), control);
            assert!(!s.is_empty(), "{control}");
            for b in &s {
                if let Some(icon) = &b.look.icon {
                    assert!(render::icon_svg(icon).is_some(), "{control}: no icon {icon}");
                }
            }
        }
    }

    #[test]
    fn fill_puts_suggestions_on_free_keys_only() {
        let mut p = Profile::new("Desk");
        p.controls.insert("keys.0".into(), bind("core.media", json!({"command": "stop"}), "Stop", "square"));
        let filled = fill_keys(&mut p, None);
        assert!(filled > 0);
        assert_eq!(p.controls["keys.0"].look.label.as_deref(), Some("Stop"), "a set key stays");
        // no two keys do the same
        let keys: Vec<&Binding> = (0..15).filter_map(|i| p.controls.get(&format!("keys.{i}"))).collect();
        for (i, a) in keys.iter().enumerate() {
            assert!(!keys[i + 1..].iter().any(|b| same_job(a, b)));
        }
    }
}
