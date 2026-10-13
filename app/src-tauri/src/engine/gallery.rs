//! The dial's looks, as a gallery (owner, 2026-10-12): on the deck, a dial set to "Dial gallery"
//! shows one look after another as it turns, live where it can (the time, what is playing, the
//! computer's load), with a caption at the top; in the window the same looks are previews with
//! captions, and a click puts one on the dial. Each look is a real dial setting.

use serde::Serialize;
use serde_json::json;

use super::house;
use super::profile::{Binding, Look};
use super::render;

/// The "Dial gallery" action: turn or press for the next look.
pub const ACTION: &str = "core.dialGallery";

/// One look of the dial.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Face {
    pub id: &'static str,
    /// Caption in English and Polish (the deck shows the app's language).
    pub caption: &'static str,
    pub caption_pl: &'static str,
    pub binding: Binding,
}

impl Face {
    pub fn caption_in(&self, language: &str) -> &'static str {
        if language == "pl" { self.caption_pl } else { self.caption }
    }
}

fn look(label: &str, icon: &str, hue: f32) -> Look {
    Look { label: Some(label.into()), icon: Some(icon.into()), bg: Some(house::BG.into()), fg: Some(house::LABEL.into()), icon_color: Some(house::icon_color(hue)), ..Look::default() }
}

fn face(id: &'static str, caption: &'static str, caption_pl: &'static str, action: &str, params: serde_json::Value, look: Look) -> Face {
    Face { id, caption, caption_pl, binding: Binding { action: action.into(), params, look, accel: false, look_on: None, swap_turns: false } }
}

/// The looks, in the order the gallery shows them.
pub fn faces() -> Vec<Face> {
    vec![
        face("volume", "Volume", "Głośność", "core.volume", json!({"target": "system", "step": 2}), look("Volume", "volume-2", house::hue::SOUND)),
        face("clock", "Clock", "Zegar", super::actions::CLOCK, json!({"show": "timeDate", "hours": "24"}), look("", "clock", house::hue::LIVE)),
        face("nowPlaying", "Now playing", "Teraz grane", super::actions::NOW_PLAYING, json!({}), look("Now playing", "music", house::hue::MEDIA)),
        face("cpu", "Computer load", "Obciążenie komputera", super::actions::MONITOR, json!({"metric": "cpu"}), look("CPU", "zap", house::hue::LIVE)),
        face("gpuTemp", "GPU temperature", "Temperatura GPU", super::actions::MONITOR, json!({"metric": "gpuTemp"}), look("GPU", "monitor", house::hue::LIVE)),
        face("timer", "Timer", "Minutnik", super::actions::TIMER, json!({"minutes": 5, "seconds": 0, "press": "startPause", "sound": "on"}), look("Timer", "timer", house::hue::LIVE)),
        face("mic", "Mic level", "Poziom mikrofonu", "core.volume", json!({"target": "mic", "step": 2}), look("Mic", "mic", house::hue::MIC)),
        face("scroll", "Scroll and zoom", "Przewijanie i zoom", "core.scroll", json!({"axis": "vertical", "amount": 3}), look("Scroll", "arrow-up-down", house::hue::TOOL)),
        face("brightness", "Monitor brightness", "Jasność monitora", "core.brightness", json!({"monitor": "all", "step": 5}), look("Brightness", "sun", house::hue::SHOW)),
        face("profile", "Profiles", "Profile", "core.profile", json!({"which": "next"}), look("Profiles", "layers", house::hue::NAV)),
        face("aurora", "Animation", "Animacja", "core.volume", json!({"target": "system", "step": 2}), picture("dial-aurora.gif", "Volume")),
        face("orbit", "Animation", "Animacja", "core.media", json!({"command": "playPause"}), picture("dial-orbit.gif", "")),
        face("sweep", "Animation", "Animacja", "core.scroll", json!({"axis": "vertical", "amount": 3}), picture("dial-sweep.gif", "")),
    ]
}

/// A look with one of the demo's animations (an icon in tests, where there is no images folder).
fn picture(file: &str, label: &str) -> Look {
    let mut l = look(label, "sparkles", house::hue::SHOW);
    l.image = super::demo::animation(file);
    if label.is_empty() {
        l.label = None;
    }
    l
}

/// The look shown at `index` (any number: it wraps).
pub fn at(index: i64) -> Face {
    let all = faces();
    let n = all.len() as i64;
    all.into_iter().nth(index.rem_euclid(n) as usize).expect("in range")
}

/// A preview of each look with sample values (the window's gallery), drawn like the deck draws it.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    #[serde(flatten)]
    pub face: Face,
    pub svg: String,
}

pub fn previews() -> Vec<Preview> {
    let now = chrono::Local::now();
    faces()
        .into_iter()
        .map(|f| {
            let l = &f.binding.look;
            let svg = match f.id {
                "volume" => render::dial_svg(l, Some((Some(0.72), "Volume")), 0),
                "clock" => render::time_dial_svg(l, &now.format("%H:%M").to_string(), Some(&now.format("%a %-d %b").to_string()), None, 0),
                "nowPlaying" => render::now_playing_dial_svg(l, Some(("Midnight City", "M83", true, None)), Some(0.4), 0),
                "cpu" => render::dial_svg(l, Some((Some(0.37), "CPU")), 0),
                "gpuTemp" => render::dial_svg(l, Some((Some(0.54), "GPU temp")), 0),
                "timer" => render::time_dial_svg(l, "4:32", Some("Timer"), Some(0.9), 0),
                "mic" => render::dial_svg(l, Some((Some(0.8), "Mic")), 0),
                "aurora" | "orbit" | "sweep" => render::dial_svg(&Look { image: l.image.as_ref().map(|i| format!("{i}#0")), ..l.clone() }, None, 0),
                _ => render::dial_svg(l, None, 0),
            };
            Preview { face: f, svg }
        })
        .collect()
}

/// The window's dial gallery.
#[tauri::command]
pub fn dial_gallery() -> Vec<Preview> {
    previews()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_look_is_a_real_dial_setting_and_draws() {
        let catalog = super::super::actions::catalog();
        for f in faces() {
            let def = catalog.iter().find(|a| a.id == f.binding.action).unwrap_or_else(|| panic!("{}: unknown action {}", f.id, f.binding.action));
            assert!(def.inputs.contains(&super::super::actions::InputKind::Turn) || def.inputs.contains(&super::super::actions::InputKind::Press), "{}", f.id);
            assert!(!f.caption_pl.is_empty());
        }
        let p = previews();
        assert_eq!(p.len(), faces().len());
        assert!(p.iter().all(|p| p.svg.starts_with("<svg")));
        assert_eq!(at(-1).id, faces().last().unwrap().id, "turning back from the first wraps around");
    }
}
