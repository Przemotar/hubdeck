//! The screensaver (roadmap phase 12 G; the assistant's advice the owner passed on, 2026-10-12):
//! after some idle minutes a calm scene across all 15 keys, gone at the first touch, which only
//! wakes the deck. Mood when you are away, nothing moving while you work.

/// The scenes: the living deck's layers across all 15 keys, made for a deck nobody touches
/// (owner, 2026-10-26: "jakieś wow, takie bardzo wysokiej jakości wygaszacza"). Each with the colour
/// the dial and the lights take while it runs. The drawn effects (code rain, night sky, waves) and
/// the big clock are gone (owner, 2026-10-30: "cdoe rain, nioght sky, waves, big clock do usuniecia").
pub const SCENES: [(&str, &str); 8] =
    [("aurora", "#3ee6a8"), ("aquarium", "#2aa8e0"), ("lava", "#ff5a2a"), ("fire", "#ff8a1e"), ("snowfall", "#b8d4ff"), ("fireflies", "#d8e860"), ("neon", "#ff3aa8"), ("haunted", "#9a6ae0")];

/// Scenes kept for their season but not offered: not in "Next", not in the key's list (owner,
/// 2026-10-30: "snowy night schowaj - przywrócimy na zimę"). One already chosen keeps running.
pub const HIDDEN: [&str; 1] = ["snowfall"];

/// The screensavers taken out, and the scene a setting that still names one gets instead.
const GONE: [&str; 4] = ["rain", "stars", "waves", "clock"];
const INSTEAD: &str = "aurora";

/// A saved screensaver as it runs today: one taken out becomes the aurora.
pub fn current(id: &str) -> String {
    if GONE.contains(&id) { INSTEAD.to_string() } else { id.to_string() }
}

/// A preset as the screensaver (owner, 2026-10-30: "presety dorzucamy jako screensavery tez"):
/// `FROM_LOOK` is the profile's own look from Deck look, `preset:<id>` a preset whose layers come
/// with it (the window knows the presets).
pub const FROM_LOOK: &str = "preset";

/// The preset `id` names: Some("") for the profile's own look, Some(<id>) for a preset.
pub fn preset(id: &str) -> Option<&str> {
    if id == FROM_LOOK { Some("") } else { id.strip_prefix("preset:").filter(|p| !p.is_empty()) }
}

/// Whether `id` is a screensaver (a scene, a taken-out one that maps to a scene, or a preset).
pub fn known(id: &str) -> bool {
    let id = current(id);
    preset(&id).is_some() || SCENES.iter().any(|(s, _)| *s == id)
}

/// A preset's layers as a screensaver: the same picture without its answer to presses (the first
/// touch only wakes the deck).
pub fn still(fx: &super::profile::Effects) -> super::profile::Effects {
    let mut out = fx.clone();
    for l in &mut out.layers {
        l.react = Some(false);
    }
    out
}

/// The layers of scene `id`, drawn by the living deck's canvas on black keys; None for anything else.
pub fn scene(id: &str) -> Option<super::profile::Effects> {
    use super::profile::{Effects, Layer};
    let l = |kind: &str| Layer { kind: kind.into(), react: Some(false), ..Layer::default() };
    let layers = match id {
        // the northern lights over a starry night sky
        "aurora" => vec![Layer { density: Some(60), ..l("scene") }, Layer { strength: Some(85), ..l("aurora") }],
        // koi in clear water, the light of the surface on the bottom
        "aquarium" => vec![l("aquarium"), Layer { strength: Some(45), ..l("caustics") }],
        "lava" => vec![Layer { strength: Some(80), ..l("lava") }],
        // a fire along the bottom, embers rising out of it
        "fire" => vec![Layer { strength: Some(80), ..l("fire") }, Layer { density: Some(60), ..l("embers") }],
        "snowfall" => vec![Layer { tint: Some("#1c2a48".into()), density: Some(40), ..l("scene") }, Layer { density: Some(65), ..l("snow") }],
        "fireflies" => vec![Layer { tint: Some("#14301c".into()), density: Some(30), ..l("scene") }, Layer { density: Some(70), ..l("fireflies") }],
        "neon" => vec![Layer { strength: Some(80), ..l("neonrain") }, Layer { tint: Some("#7a3ad0".into()), strength: Some(35), density: Some(30), ..l("mist") }],
        // Halloween's haunted night without its presses
        "haunted" => vec![
            Layer { tint: Some("#6a3aa0".into()), density: Some(45), ..l("scene") },
            l("haunt"),
            Layer { tint: Some("#9a7ae0".into()), strength: Some(50), density: Some(30), wind: Some(15), ..l("mist") },
            l("bats"),
            l("ghost"),
        ],
        _ => return None,
    };
    Some(Effects { layers, ..Effects::default() })
}

/// The colour of `id` for the dial and the lights; None = the deck's light colour.
pub fn color(id: &str) -> Option<&'static str> {
    SCENES.iter().find(|(s, _)| *s == id).map(|(_, c)| *c).filter(|c| !c.is_empty())
}

/// Seconds since the app's clock started (the weather dial and the nudges move with it).
pub fn clock() -> f32 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_secs_f32()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_scene_is_known_layers() {
        for (id, _) in SCENES {
            assert!(known(id));
            let fx = scene(id).expect("a scene has layers");
            assert!(fx.active(), "{id} has layers");
            for l in fx.stack() {
                assert!(super::super::canvas::KINDS.contains(&l.kind.as_str()), "{id}: {} is a layer", l.kind);
            }
        }
        assert!(!known("nope") && scene("nope").is_none());
    }

    #[test]
    fn the_ones_taken_out_become_the_aurora() {
        for id in ["rain", "stars", "waves", "clock"] {
            assert!(known(id));
            assert_eq!(current(id), "aurora");
        }
        assert_eq!(current("lava"), "lava");
    }

    #[test]
    fn presets_are_screensavers_that_do_not_answer_presses() {
        assert!(known(FROM_LOOK) && known("preset:japan-koi") && !known("preset:"));
        assert_eq!(preset("preset:japan-koi"), Some("japan-koi"));
        assert_eq!(preset(FROM_LOOK), Some(""));
        assert_eq!(preset("aurora"), None);
        let fx = super::super::profile::Effects { layers: vec![super::super::profile::Layer { kind: "koi".into(), ..Default::default() }], ..Default::default() };
        assert!(still(&fx).layers.iter().all(|l| l.react == Some(false)));
    }
}
