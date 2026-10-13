//! HubDeck's own key look (owner, 2026-10-12: option 3 "colored icon" of
//! `prototype/tools/gen_house_style.py`): a graphite key, the icon in its group's soft color, a
//! light label. Everything HubDeck makes (ready-made profiles, starters, suggestions, the demo)
//! uses it, so it sits calmly next to app icons and game covers. The user's own colors stay theirs.

use super::profile::{Look, Profile};

/// The key itself.
pub const BG: &str = "#1b1a20";
/// The label.
pub const LABEL: &str = "#e6e4ee";

/// Hues of the groups (degrees), so the same kind of thing has the same color everywhere.
pub mod hue {
    pub const MEDIA: f32 = 160.0;
    pub const SOUND: f32 = 190.0;
    pub const MIC: f32 = 350.0;
    pub const NAV: f32 = 250.0;
    pub const TOOL: f32 = 30.0;
    pub const LIVE: f32 = 200.0;
    pub const APP: f32 = 230.0;
    pub const SHOW: f32 = 270.0;
    pub const GAME: f32 = 10.0;
}

/// The soft icon color of a hue.
pub fn icon_color(hue: f32) -> String {
    hsl(hue, 0.75, 0.70)
}

fn hsl(h: f32, s: f32, l: f32) -> String {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = (h.rem_euclid(360.0)) / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let to = |v: f32| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    format!("#{:02x}{:02x}{:02x}", to(r), to(g), to(b))
}

/// The hue of a color, when it has one worth keeping (greys have none).
pub fn hue_of(hex: &str) -> Option<f32> {
    let [r, g, b] = super::leds::parse_hex(hex)?.map(|v| v as f32 / 255.0);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let d = max - min;
    let l = (max + min) / 2.0;
    let s = if d == 0.0 { 0.0 } else { d / (1.0 - (2.0 * l - 1.0).abs()) };
    if s < 0.12 || d < 0.03 {
        return None;
    }
    let h = if max == r {
        60.0 * (((g - b) / d) % 6.0)
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    Some(h.rem_euclid(360.0))
}

/// The group hue of an action, for keys that come without a tint of their own.
pub fn hue_for_action(action: &str, params: &serde_json::Value) -> Option<f32> {
    let target = super::actions::param_str(params, "target");
    Some(match action {
        "core.media" | "core.seek" | "core.nowPlaying" => hue::MEDIA,
        "core.mic" => hue::MIC,
        "core.volume" if target == "mic" => hue::MIC,
        "core.volume" | "core.audioDevice" | "core.sound" => hue::SOUND,
        "core.page" | "core.profile" => hue::NAV,
        "core.launch" => hue::APP,
        "core.game" | "core.games" => hue::GAME,
        "core.clock" | "core.timer" | "core.monitor" | "core.live" => hue::LIVE,
        "core.hotkey" | "core.text" | "core.multi" | "core.run" => hue::TOOL,
        "core.lighting" | "core.deckBrightness" | "core.brightness" => hue::SHOW,
        _ => return None,
    })
}

/// A look in the house style: the graphite key and light label, the icon colored by the look's
/// own tint (its old background) or by what the action does. Pictures keep their own look.
pub fn apply_look(look: &mut Look, action: &str, params: &serde_json::Value) {
    if look.image.is_some() {
        return;
    }
    let hue = look.bg.as_deref().and_then(hue_of).or_else(|| hue_for_action(action, params));
    look.bg = Some(BG.into());
    look.fg = Some(LABEL.into());
    look.style = None;
    look.icon_color = hue.map(icon_color);
}

/// Every control of a profile HubDeck made, in the house style (the second look of stateful
/// keys stays as it was: it has to stand out).
pub fn apply(profile: &mut Profile) {
    for b in profile.controls.values_mut() {
        let (action, params) = (b.action.clone(), b.params.clone());
        apply_look(&mut b.look, &action, &params);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn hues_colors_and_the_look() {
        assert_eq!(hue_of("#808080"), None, "greys have no hue");
        assert!((hue_of("#ff0000").unwrap() - 0.0).abs() < 0.5);
        assert!((hue_of("#1f2a33").unwrap() - 207.0).abs() < 3.0, "a dark tint keeps its hue");
        assert_eq!(icon_color(0.0), "#ec7979");
        let mut look = Look { label: Some("Play".into()), icon: Some("play".into()), bg: Some("#1d2b3a".into()), style: Some("glass".into()), ..Look::default() };
        apply_look(&mut look, "core.media", &json!({}));
        assert_eq!(look.bg.as_deref(), Some(BG));
        assert!(look.style.is_none() && look.icon_color.is_some());
        // no tint: the action decides
        let mut plain = Look::default();
        apply_look(&mut plain, "core.mic", &json!({}));
        assert_eq!(plain.icon_color, Some(icon_color(hue::MIC)));
        // a picture keeps its look
        let mut pic = Look { image: Some("a.png".into()), bg: Some("#ff0000".into()), ..Look::default() };
        apply_look(&mut pic, "core.launch", &json!({}));
        assert_eq!(pic.bg.as_deref(), Some("#ff0000"));
    }
}
