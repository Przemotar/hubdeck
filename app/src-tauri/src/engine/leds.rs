//! What the knob, fader and base edge LEDs show for a profile: a color per LED and its effect
//! (static, breathing, strobe, blink, cycling, random, wave, water, rainbow; verified on the knobs,
//! PROTOCOL.md), plus the module's effect speed. Pure: the engine sends the result.

use std::collections::HashMap;

use mh_protocol::report::{LedMode, Rgb};

use super::profile::{LightSettings, Look, Profile};

/// A level LED never goes fully dark, so the control can still be found.
const MIN_LEVEL: f32 = 0.06;

/// Effect names as profiles store them (`look.ledEffect`, `edgeEffect`) and the UI offers them.
pub const EFFECTS: [(&str, &str); 9] = [
    ("static", "Static"),
    ("breathing", "Breathing"),
    ("strobe", "Strobe"),
    ("blink", "Fast blink"),
    ("cycling", "Color cycle"),
    ("random", "Random colors"),
    ("wave", "Wave"),
    ("water", "Shimmer"),
    ("rainbow", "Rainbow"),
];

pub fn parse_hex(hex: &str) -> Option<Rgb> {
    let h = hex.strip_prefix('#')?;
    if h.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

pub fn scale(rgb: Rgb, level: f32) -> Rgb {
    let k = level.clamp(MIN_LEVEL, 1.0);
    rgb.map(|c| (c as f32 * k).round() as u8)
}

/// An effect name -> the module's LED mode; anything unknown is static.
pub fn effect_mode(name: Option<&str>) -> LedMode {
    match name.unwrap_or("static") {
        "breathing" => LedMode::Breathing,
        "strobe" => LedMode::Strobe,
        "blink" => LedMode::Spring,
        "cycling" => LedMode::Cycling,
        "random" => LedMode::Random,
        "wave" => LedMode::Wave,
        "water" => LedMode::Water,
        "rainbow" => LedMode::Rainbow,
        "off" => LedMode::Off,
        _ => LedMode::Static,
    }
}

/// One LED: its color and effect.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Led {
    pub rgb: Rgb,
    pub mode: LedMode,
}

impl Led {
    pub fn solid(rgb: Rgb) -> Self {
        Self { rgb, mode: LedMode::Static }
    }
}

/// The LEDs of one module and the speed of its effects (one speed per module, 0..=100).
#[derive(Debug, Clone, PartialEq)]
pub struct Lights {
    pub leds: Vec<Led>,
    pub speed: Option<u8>,
}

impl Lights {
    pub fn solid(colors: Vec<Rgb>) -> Self {
        Self { leds: colors.into_iter().map(Led::solid).collect(), speed: None }
    }
}

/// A lighting effect set by the "Lighting effect" action, until the profile changes.
#[derive(Debug, Clone, PartialEq)]
pub struct Override {
    pub mode: LedMode,
    pub rgb: Rgb,
    pub speed: u8,
}

impl Override {
    pub fn lights(&self, count: usize) -> Lights {
        Lights { leds: vec![Led { rgb: self.rgb, mode: self.mode }; count], speed: Some(self.speed) }
    }
}

/// How long HubDeck's rainbow takes to go round once. The window uses the same number.
pub const RAINBOW_MS: u64 = 10_000;

/// HubDeck's rainbow color at `unix_ms` (milliseconds since 1970): the hue goes round once every
/// `RAINBOW_MS`, full saturation. The window works it out the same way from the same clock.
pub fn rainbow_rgb(unix_ms: u64) -> Rgb {
    let h = (unix_ms % RAINBOW_MS) as f32 / RAINBOW_MS as f32 * 6.0;
    let x = 1.0 - (h % 2.0 - 1.0).abs();
    let (r, g, b) = match h as u32 {
        0 => (1.0, x, 0.0),
        1 => (x, 1.0, 0.0),
        2 => (0.0, 1.0, x),
        3 => (0.0, x, 1.0),
        4 => (x, 0.0, 1.0),
        _ => (1.0, 0.0, x),
    };
    [r, g, b].map(|v: f32| (v * 255.0).round() as u8)
}

/// The module's own rainbow (None) and rainbow effects drawn by HubDeck instead: each such LED gets
/// the rainbow color of the moment as a static color, dimmed by `brightness`. Returns the lights
/// and which LEDs were replaced.
pub fn hub_rainbow(lights: Option<Lights>, count: usize, unix_ms: u64, brightness: u8) -> (Lights, Vec<bool>) {
    let rgb = rainbow_rgb(unix_ms).map(|c| (c as u32 * brightness.min(100) as u32 / 100) as u8);
    match lights {
        None => (Lights::solid(vec![rgb; count]), vec![true; count]),
        Some(mut l) => {
            let replaced: Vec<bool> = l.leds.iter().map(|led| led.mode == LedMode::Rainbow).collect();
            for (led, r) in l.leds.iter_mut().zip(&replaced) {
                if *r {
                    *led = Led::solid(rgb);
                }
            }
            (l, replaced)
        }
    }
}

/// One LED as the window draws it: `#rrggbb` and the effect name (`EFFECTS`, or `off`).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct LedView {
    pub color: String,
    pub effect: &'static str,
}

/// The effect name of a LED mode (the reverse of [`effect_mode`]).
pub fn effect_name(mode: LedMode) -> &'static str {
    match mode {
        LedMode::Off => "off",
        LedMode::Static => "static",
        LedMode::Breathing => "breathing",
        LedMode::Strobe => "strobe",
        LedMode::Spring => "blink",
        LedMode::Cycling => "cycling",
        LedMode::Random => "random",
        LedMode::Wave => "wave",
        LedMode::Water => "water",
        LedMode::Rainbow => "rainbow",
    }
}

/// What the window shows for a module's lights: one entry per LED.
pub fn view(lights: &Lights) -> Vec<LedView> {
    lights.leds.iter().map(|l| LedView { color: format!("#{:02x}{:02x}{:02x}", l.rgb[0], l.rgb[1], l.rgb[2]), effect: effect_name(l.mode) }).collect()
}

/// One LED per control of a `kind` module (`knobs` 3, `faders` 5). None = no control has a color
/// or an effect, so the module keeps its own rainbow. Controls without either stay dark.
/// "Brightness follows the value" works with the static effect only.
pub fn module_leds(profile: Option<&Profile>, kind: &str, count: usize, levels: &HashMap<String, f32>) -> Option<Lights> {
    let looks: Vec<Option<&Look>> = (0..count).map(|i| profile.and_then(|p| p.controls.get(&format!("{kind}.{i}"))).map(|b| &b.look)).collect();
    let lit = |l: &Look| l.led.as_deref().and_then(parse_hex).is_some() || l.led_effect.as_deref().is_some_and(|e| e != "static");
    if !looks.iter().any(|l| l.is_some_and(lit)) {
        return None;
    }
    let mut speed = None;
    let leds = looks
        .iter()
        .enumerate()
        .map(|(i, look)| {
            let Some(look) = look.filter(|l| lit(l)) else { return Led::solid([0, 0, 0]) };
            let mode = effect_mode(look.led_effect.as_deref());
            if let Some(s) = look.led_speed {
                speed = Some(s.min(100)); // the module has one speed: the last control that sets one
            }
            let rgb = look.led.as_deref().and_then(parse_hex).unwrap_or([255, 255, 255]);
            let rgb = if look.led_level && mode == LedMode::Static { scale(rgb, levels.get(&format!("{kind}.{i}")).copied().unwrap_or(1.0)) } else { rgb };
            Led { rgb, mode }
        })
        .collect();
    Some(Lights { leds, speed })
}

/// The base edge of a profile; None = the base's rainbow.
pub fn edge(profile: Option<&Profile>) -> Option<Lights> {
    let p = profile?;
    let rgb = p.edge.as_deref().and_then(parse_hex);
    let effect = p.edge_effect.as_deref().filter(|e| *e != "static");
    if rgb.is_none() && effect.is_none() {
        return None;
    }
    Some(Lights { leds: vec![Led { rgb: rgb.unwrap_or([255, 255, 255]), mode: effect_mode(effect) }], speed: p.edge_speed.map(|s| s.min(100)) })
}

/// What is going on now that the lights react to.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Live {
    pub mic_muted: bool,
    /// OBS or Streamlabs records or streams.
    pub on_air: bool,
    /// A Claude Code session needs the user.
    pub agent_waiting: bool,
    pub timer_done: bool,
    /// A call is on (`call.rs`).
    pub in_call: bool,
}

/// Colors for the "profile color" style when a profile has none: picked from its name.
const PROFILE_COLORS: [&str; 8] = ["#8b7cf6", "#3b82f6", "#10b981", "#f59e0b", "#ef4444", "#ec4899", "#06b6d4", "#84cc16"];

/// A profile's color: its own, or one picked from its name (stable across starts).
pub fn profile_color(p: &Profile) -> Rgb {
    p.color.as_deref().and_then(parse_hex).unwrap_or_else(|| {
        let h = p.name.bytes().fold(5381u32, |h, b| h.wrapping_mul(33) ^ b as u32);
        parse_hex(PROFILE_COLORS[h as usize % PROFILE_COLORS.len()]).unwrap_or([255, 255, 255])
    })
}

/// A control bound to the microphone (mute, or its level).
fn is_mic(b: &super::profile::Binding) -> bool {
    b.action == "core.mic" || (b.action == "core.volume" && super::actions::param_str(&b.params, "target") == "mic")
}

/// The style's color for every LED; None = the modules' own rainbow.
fn style_rgb(profile: Option<&Profile>, cfg: &LightSettings) -> Option<Rgb> {
    match cfg.style.as_str() {
        "off" => Some([0, 0, 0]),
        "color" => Some(parse_hex(&cfg.color).unwrap_or([255, 255, 255])),
        "profile" => Some(profile.map(profile_color).unwrap_or([255, 255, 255])),
        _ => None,
    }
}

fn dim(lights: &mut Lights, brightness: u8) {
    if brightness < 100 {
        for l in &mut lights.leds {
            l.rgb = l.rgb.map(|c| (c as u32 * brightness as u32 / 100) as u8);
        }
    }
}

/// The LEDs of a knobs or faders module: the style, levels and the muted-mic reaction, with the
/// colors and effects set on single controls winning. None = the module's own rainbow.
pub fn lights(profile: Option<&Profile>, kind: &str, count: usize, levels: &HashMap<String, f32>, cfg: &LightSettings, live: &Live) -> Option<Lights> {
    let advanced = module_leds(profile, kind, count, levels);
    let style = style_rgb(profile, cfg);
    let binding = |i: usize| profile.and_then(|p| p.controls.get(&format!("{kind}.{i}")));
    let own = |i: usize| binding(i).is_some_and(|b| b.look.led.as_deref().and_then(parse_hex).is_some() || b.look.led_effect.as_deref().is_some_and(|e| e != "static"));
    let mic_red = |i: usize| cfg.react.mic.on && live.mic_muted && binding(i).is_some_and(is_mic);
    let mut any = false;
    let mut out = Lights { leds: Vec::with_capacity(count), speed: advanced.as_ref().and_then(|a| a.speed) };
    for i in 0..count {
        let led = if mic_red(i) {
            Led::solid(parse_hex(&cfg.react.mic.color).unwrap_or([255, 0, 0]))
        } else if own(i) {
            advanced.as_ref().map(|a| a.leds[i]).unwrap_or(Led::solid([0, 0, 0]))
        } else if let Some(rgb) = style {
            let level = levels.get(&format!("{kind}.{i}")).copied().filter(|_| cfg.react.levels && binding(i).is_some());
            Led::solid(level.map(|l| scale(rgb, l)).unwrap_or(rgb))
        } else {
            out.leds.push(Led { rgb: [255, 255, 255], mode: LedMode::Rainbow });
            continue;
        };
        any = true;
        out.leds.push(led);
    }
    if !any {
        return None; // all rainbow: the module's own
    }
    dim(&mut out, cfg.brightness);
    Some(out)
}

/// The base edge: reactions first (a session needs you, on air, a timer ran out, the mic is muted
/// and no knob shows it), then the profile's own edge color or effect, then the style.
pub fn edge_lights(profile: Option<&Profile>, cfg: &LightSettings, live: &Live) -> Option<Lights> {
    let react = |r: &super::profile::Reaction, mode: LedMode, speed: u8| Lights { leds: vec![Led { rgb: parse_hex(&r.color).unwrap_or([255, 255, 255]), mode }], speed: Some(speed) };
    let mic_shown_on_a_knob = profile.is_some_and(|p| p.controls.iter().any(|(id, b)| (id.starts_with("knobs.") || id.starts_with("faders.")) && is_mic(b)));
    let mut out = if cfg.react.agents.on && live.agent_waiting {
        react(&cfg.react.agents, LedMode::Breathing, 85)
    } else if cfg.react.live.on && live.on_air {
        react(&cfg.react.live, LedMode::Breathing, 40)
    } else if cfg.react.timer.on && live.timer_done {
        react(&cfg.react.timer, LedMode::Strobe, 70)
    } else if cfg.react.call.on && live.in_call {
        // in a call the edge says whether you can be heard
        if live.mic_muted { react(&cfg.react.mic, LedMode::Static, 50) } else { react(&cfg.react.call, LedMode::Static, 50) }
    } else if cfg.react.mic.on && live.mic_muted && !mic_shown_on_a_knob {
        react(&cfg.react.mic, LedMode::Static, 50)
    } else if let Some(own) = edge(profile) {
        own
    } else {
        Lights::solid(vec![style_rgb(profile, cfg)?])
    };
    dim(&mut out, cfg.brightness);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::profile::Binding;

    fn with_look(p: &mut Profile, control: &str, look: Look) {
        p.controls.insert(control.into(), Binding { action: "core.volume".into(), params: serde_json::json!({}), look, accel: false, look_on: None, swap_turns: false });
    }

    fn with_led(p: &mut Profile, control: &str, led: &str, level: bool) {
        with_look(p, control, Look { led: Some(led.into()), led_level: level, ..Look::default() });
    }

    fn colors(l: &Lights) -> Vec<Rgb> {
        l.leds.iter().map(|l| l.rgb).collect()
    }

    #[test]
    fn the_window_gets_hex_colors_and_effect_names() {
        for (name, _) in EFFECTS {
            assert_eq!(effect_name(effect_mode(Some(name))), name);
        }
        let v = view(&Lights { leds: vec![Led { rgb: [255, 8, 0], mode: LedMode::Breathing }, Led::solid([0, 0, 0])], speed: None });
        assert_eq!(v, vec![LedView { color: "#ff0800".into(), effect: "breathing" }, LedView { color: "#000000".into(), effect: "static" }]);
    }

    #[test]
    fn hubdecks_rainbow_turns_with_the_clock() {
        assert_eq!(rainbow_rgb(0), [255, 0, 0]);
        assert_eq!(rainbow_rgb(RAINBOW_MS / 3), [0, 255, 0]);
        assert_eq!(rainbow_rgb(RAINBOW_MS), [255, 0, 0], "once round");
        // the module's own rainbow: every LED the same color
        let (l, replaced) = hub_rainbow(None, 3, 0, 100);
        assert_eq!(l.leds, vec![Led::solid([255, 0, 0]); 3]);
        assert!(replaced.iter().all(|r| *r));
        // only rainbow LEDs are replaced; brightness dims them
        let mixed = Lights { leds: vec![Led { rgb: [1, 2, 3], mode: LedMode::Rainbow }, Led::solid([9, 9, 9])], speed: None };
        let (l, replaced) = hub_rainbow(Some(mixed), 2, 0, 50);
        assert_eq!(l.leds[0], Led::solid([127, 0, 0]));
        assert_eq!(l.leds[1], Led::solid([9, 9, 9]));
        assert_eq!(replaced, vec![true, false]);
    }

    #[test]
    fn no_colors_keep_the_rainbow() {
        let p = Profile::new("x");
        assert_eq!(module_leds(Some(&p), "faders", 5, &HashMap::new()), None);
        assert_eq!(module_leds(None, "knobs", 3, &HashMap::new()), None);
        assert_eq!(edge(Some(&p)), None);
    }

    #[test]
    fn colors_levels_and_dark_controls() {
        let mut p = Profile::new("x");
        with_led(&mut p, "faders.0", "#ff8000", false);
        with_led(&mut p, "faders.2", "#ffffff", true);
        let levels = HashMap::from([("faders.2".to_string(), 0.5)]);
        let leds = module_leds(Some(&p), "faders", 5, &levels).unwrap();
        assert_eq!(colors(&leds), vec![[255, 128, 0], [0, 0, 0], [128, 128, 128], [0, 0, 0], [0, 0, 0]]);
        assert!(leds.leds.iter().all(|l| l.mode == LedMode::Static));
        // at zero the LED stays faintly on
        let low = module_leds(Some(&p), "faders", 5, &HashMap::from([("faders.2".to_string(), 0.0)])).unwrap();
        assert_eq!(low.leds[2].rgb, [15, 15, 15]);
    }

    #[test]
    fn effects_speed_and_level_only_when_static() {
        let mut p = Profile::new("x");
        with_look(&mut p, "knobs.0", Look { led: Some("#ff0000".into()), led_effect: Some("breathing".into()), led_level: true, led_speed: Some(30), ..Look::default() });
        with_look(&mut p, "knobs.1", Look { led_effect: Some("rainbow".into()), led_speed: Some(250), ..Look::default() });
        let l = module_leds(Some(&p), "knobs", 3, &HashMap::from([("knobs.0".to_string(), 0.1)])).unwrap();
        assert_eq!(l.leds[0], Led { rgb: [255, 0, 0], mode: LedMode::Breathing }, "no level scaling while breathing");
        assert_eq!(l.leds[1].mode, LedMode::Rainbow, "an effect without a color lights up too");
        assert_eq!(l.leds[2], Led::solid([0, 0, 0]));
        assert_eq!(l.speed, Some(100), "the last speed, at most 100");
        let mut q = Profile::new("y");
        q.edge_effect = Some("strobe".into());
        q.edge = Some("#00ff00".into());
        q.edge_speed = Some(70);
        assert_eq!(edge(Some(&q)), Some(Lights { leds: vec![Led { rgb: [0, 255, 0], mode: LedMode::Strobe }], speed: Some(70) }));
        assert_eq!(effect_mode(Some("blink")), LedMode::Spring);
        assert_eq!(effect_mode(Some("nonsense")), LedMode::Static);
    }

    #[test]
    fn style_reactions_and_advanced_colors() {
        let cfg = LightSettings::default();
        let live = Live::default();
        let mut p = Profile::new("Desk");
        // the default style is the modules' own rainbow: nothing to send
        assert_eq!(lights(Some(&p), "knobs", 3, &HashMap::new(), &cfg, &live), None);
        assert_eq!(edge_lights(Some(&p), &cfg, &live), None);
        // one color for the whole deck, levels as brightness
        let blue = LightSettings { style: "color".into(), color: "#0000ff".into(), ..LightSettings::default() };
        with_led(&mut p, "knobs.2", "#ff8000", false); // an advanced color wins
        p.controls
            .insert("knobs.0".into(), Binding { action: "core.volume".into(), params: serde_json::json!({"target": "mic"}), look: Look::default(), accel: false, look_on: None, swap_turns: false });
        let levels = HashMap::from([("knobs.0".to_string(), 0.5)]);
        let l = lights(Some(&p), "knobs", 3, &levels, &blue, &live).unwrap();
        assert_eq!(colors(&l), vec![[0, 0, 128], [0, 0, 255], [255, 128, 0]]);
        // the muted mic turns its knob red; the edge stays as it is
        let muted = Live { mic_muted: true, ..Live::default() };
        assert_eq!(lights(Some(&p), "knobs", 3, &levels, &blue, &muted).unwrap().leds[0].rgb, [255, 32, 32]);
        assert_eq!(edge_lights(Some(&p), &blue, &muted).unwrap().leds[0].rgb, [0, 0, 255]);
        // on air beats the style, a waiting agent beats on air
        let busy = Live { on_air: true, agent_waiting: true, ..Live::default() };
        assert_eq!(edge_lights(Some(&p), &blue, &busy).unwrap().leds[0].rgb, [255, 138, 0]);
        let air = Live { on_air: true, ..Live::default() };
        assert_eq!(edge_lights(Some(&p), &blue, &air).unwrap().leds[0].mode, LedMode::Breathing);
        // switched off, a reaction does nothing
        let mut quiet = blue.clone();
        quiet.react.live.on = false;
        assert_eq!(edge_lights(Some(&p), &quiet, &air).unwrap().leds[0], Led::solid([0, 0, 255]));
        // brightness dims every LED; profile colors come from the name unless set
        let half = LightSettings { brightness: 50, ..blue.clone() };
        assert_eq!(edge_lights(Some(&p), &half, &live).unwrap().leds[0].rgb, [0, 0, 127]);
        let by_profile = LightSettings { style: "profile".into(), ..LightSettings::default() };
        assert_eq!(edge_lights(Some(&p), &by_profile, &live).unwrap().leds[0].rgb, profile_color(&p));
        p.color = Some("#123456".into());
        assert_eq!(profile_color(&p), [0x12, 0x34, 0x56]);
    }

    #[test]
    fn hex_parsing() {
        assert_eq!(parse_hex("#0a141e"), Some([10, 20, 30]));
        assert_eq!(parse_hex("0a141e"), None);
        assert_eq!(parse_hex("#fff"), None);
    }
}
