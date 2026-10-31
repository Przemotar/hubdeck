//! Key and dial images. Each image is an SVG built from the binding's look; the same SVG is the
//! live preview in the settings window, and resvg rasterises it for the hardware, so the preview
//! and the device always match.

use std::io::Cursor;

use resvg::{tiny_skia, usvg};

use super::game_icons::GAME_ICONS;
use super::icons::ICONS;
use super::profile::Look;

const FONT: &[u8] = include_bytes!("../../assets/fonts/IBMPlexSans-SemiBold.ttf");
pub const FONT_FAMILY: &str = "IBM Plex Sans";

/// The fonts images are drawn with: HubDeck's own and the computer's, loaded once (memory-mapped,
/// so the system's fonts cost address space, not memory). A letter HubDeck's font lacks (Japanese,
/// Chinese, Korean...) comes from a system font that has it (Discussions #3, 2026-10-18).
static FONTS: std::sync::LazyLock<std::sync::Arc<usvg::fontdb::Database>> = std::sync::LazyLock::new(|| {
    let started = std::time::Instant::now();
    let mut db = usvg::fontdb::Database::new();
    db.load_font_data(FONT.to_vec());
    db.load_system_fonts();
    log::info!("fonts: {} faces in {:.0?}", db.len(), started.elapsed());
    std::sync::Arc::new(db)
});

/// Settings > Screens: the font for text on the deck (None = HubDeck's own).
#[cfg(not(test))]
static DECK_FONT: std::sync::RwLock<Option<String>> = std::sync::RwLock::new(None);
// one per test thread: a test that picks a font must not change the text of tests drawing beside
// it (a test failed one run in two, 2026-10-18)
#[cfg(test)]
thread_local! {
    static DECK_FONT: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// Use `family` for text on the deck; one this computer does not have falls back to HubDeck's own.
pub fn set_deck_font(family: Option<&str>) {
    let family = family.map(str::trim).filter(|f| !f.is_empty() && *f != FONT_FAMILY && deck_fonts().iter().any(|x| x == f)).map(str::to_string);
    #[cfg(not(test))]
    {
        *DECK_FONT.write().unwrap_or_else(|e| e.into_inner()) = family;
    }
    #[cfg(test)]
    DECK_FONT.with(|f| *f.borrow_mut() = family);
}

thread_local! {
    /// The font of the key being drawn now (`Look::font`), over the deck's font.
    static KEY_FONT: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// Whether this computer has the font family `name`.
pub fn has_font(name: &str) -> bool {
    name == FONT_FAMILY || FONTS.faces().any(|f| f.families.iter().any(|(n, _)| n == name))
}

/// The key's own font while a key is drawn; back to the deck's when dropped.
struct KeyFont;

impl KeyFont {
    fn set(family: Option<&str>) -> Self {
        let family = family.map(str::trim).filter(|f| !f.is_empty() && has_font(f)).map(str::to_string);
        KEY_FONT.with(|k| *k.borrow_mut() = family);
        KeyFont
    }
}

impl Drop for KeyFont {
    fn drop(&mut self) {
        KEY_FONT.with(|k| *k.borrow_mut() = None);
    }
}

/// The font family text on the deck is drawn in, ready for an SVG attribute.
pub fn font_family() -> String {
    if let Some(f) = KEY_FONT.with(|k| k.borrow().clone()) {
        return escape(&f);
    }
    #[cfg(not(test))]
    let chosen = DECK_FONT.read().unwrap_or_else(|e| e.into_inner()).clone();
    #[cfg(test)]
    let chosen = DECK_FONT.with(|f| f.borrow().clone());
    match chosen.as_deref() {
        Some(f) => escape(f),
        None => FONT_FAMILY.to_string(),
    }
}

/// The font families on this computer, by name (for the list in Settings > Screens).
pub fn deck_fonts() -> Vec<String> {
    let mut names: Vec<String> = FONTS.faces().filter_map(|f| f.families.first().map(|(name, _)| name.clone())).filter(|n| !n.starts_with('@') && !n.trim().is_empty()).collect();
    names.sort_by_key(|n| n.to_lowercase());
    names.dedup();
    names
}
pub const DEFAULT_BG: &str = "#1f1d26";
pub const DEFAULT_FG: &str = "#f0eef6";
pub const KEY_PX: u32 = 72;
pub const DIAL_PX: u32 = 240;

/// Game icons are named `game:<name>` in looks.
pub const GAME_PREFIX: &str = "game:";

pub fn icon_svg(name: &str) -> Option<&'static str> {
    match name.strip_prefix(GAME_PREFIX) {
        Some(game) => GAME_ICONS.iter().find(|(n, _)| *n == game).map(|(_, svg)| *svg),
        None => ICONS.iter().find(|(n, _)| *n == name).map(|(_, svg)| *svg),
    }
}

/// The drawing inside an icon's `<svg>` element (paths, circles...).
fn icon_body(name: &str) -> Option<&'static str> {
    let svg = icon_svg(name)?;
    let start = svg.find("<svg")?;
    let open_end = start + svg[start..].find('>')? + 1;
    let close = svg.rfind("</svg>")?;
    Some(&svg[open_end..close])
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Accept only `#rgb` / `#rrggbb` so user input cannot inject markup.
fn color(value: Option<&String>, fallback: &str) -> String {
    match value {
        Some(v) if v.len() <= 7 && v.starts_with('#') && v[1..].chars().all(|c| c.is_ascii_hexdigit()) && matches!(v.len(), 4 | 7) => v.clone(),
        _ => fallback.to_string(),
    }
}

/// `#rgb` / `#rrggbb` as (r, g, b); None for anything else.
fn rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let h = hex.strip_prefix('#')?;
    let v = |s: &str| u8::from_str_radix(s, 16).ok();
    match h.len() {
        3 => Some((v(&h[0..1].repeat(2))?, v(&h[1..2].repeat(2))?, v(&h[2..3].repeat(2))?)),
        6 => Some((v(&h[0..2])?, v(&h[2..4])?, v(&h[4..6])?)),
        _ => None,
    }
}

/// `a` moved `t` (0..=1) of the way toward `b`.
fn mix(a: &str, b: &str, t: f32) -> String {
    let (Some(x), Some(y)) = (rgb(a), rgb(b)) else { return a.to_string() };
    let m = |p: u8, q: u8| (p as f32 + (q as f32 - p as f32) * t).round() as u8;
    format!("#{:02x}{:02x}{:02x}", m(x.0, y.0), m(x.1, y.1), m(x.2, y.2))
}

/// Black behind an outline key.
const OUTLINE_FILL: &str = "#0c0b10";

/// The gradient's ends for a CSS angle (0 toward the top, 90 toward the right), in the key's box.
fn gradient_ends(angle: u16) -> (f32, f32, f32, f32) {
    let (sin, cos) = (angle as f32).to_radians().sin_cos();
    let r = |v: f32| (v * 1000.0).round() / 1000.0;
    (r(0.5 - 0.5 * sin), r(0.5 + 0.5 * cos), r(0.5 + 0.5 * sin), r(0.5 - 0.5 * cos))
}

const GLASS: &str = r##"<defs><linearGradient id="kglass" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#fff" stop-opacity="0.34"/><stop offset="0.46" stop-color="#fff" stop-opacity="0.07"/><stop offset="0.5" stop-color="#fff" stop-opacity="0"/><stop offset="1" stop-color="#000" stop-opacity="0.22"/></linearGradient></defs>"##;
const MATTE: &str = r##"<defs><radialGradient id="kmatte" cx="0.5" cy="0.42" r="0.75"><stop offset="0.55" stop-color="#000" stop-opacity="0"/><stop offset="1" stop-color="#000" stop-opacity="0.38"/></radialGradient></defs>"##;

/// A key's background for its style, and what goes over the content (the glass or matte finish).
fn key_background(look: &Look, s: f32, bg: &str) -> (String, String) {
    let glass = || {
        format!(
            r##"{GLASS}<rect width="{s}" height="{s}" fill="url(#kglass)"/><rect x="1" y="1" width="{w}" height="{w}" rx="8" fill="none" stroke="#fff" stroke-opacity="0.22" stroke-width="1.5"/>"##,
            w = s - 2.0
        )
    };
    let finish = match look.gloss.as_deref() {
        Some("glass") => glass(),
        Some("matte") => format!(r#"{MATTE}<rect width="{s}" height="{s}" fill="url(#kmatte)"/>"#),
        _ => String::new(),
    };
    match look.style.as_deref() {
        Some("gradient") => {
            let (top, bottom) = match look.bg2.as_ref().map(|c| color(Some(c), "")).filter(|c| !c.is_empty()) {
                Some(end) => (bg.to_string(), end),
                None => (mix(bg, "#ffffff", 0.22), mix(bg, "#000000", 0.38)),
            };
            let angle = look.angle.unwrap_or(180) % 360;
            let (x1, y1, x2, y2) = gradient_ends(angle);
            let id = format!("kg{}{}{angle}", &top[1..], &bottom[1..]);
            (
                format!(
                    r#"<defs><linearGradient id="{id}" x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}"><stop offset="0" stop-color="{top}"/><stop offset="1" stop-color="{bottom}"/></linearGradient></defs><rect width="{s}" height="{s}" fill="url(#{id})"/>"#
                ),
                finish,
            )
        }
        // the old style: a flat key with the glass finish
        Some("glass") => (format!(r#"<rect width="{s}" height="{s}" fill="{bg}"/>"#), if finish.is_empty() { glass() } else { finish }),
        Some("outline") => (
            format!(r#"<rect width="{s}" height="{s}" fill="{OUTLINE_FILL}"/><rect x="3.5" y="3.5" width="{w}" height="{w}" rx="10" fill="none" stroke="{bg}" stroke-width="3"/>"#, w = s - 7.0),
            finish,
        ),
        _ => (format!(r#"<rect width="{s}" height="{s}" fill="{bg}"/>"#), finish),
    }
}

/// The clear edge of a label placed at the top or the bottom, key px (a frame takes 2 more).
const LABEL_EDGE: f32 = 7.0;
/// How far a label's letters reach above its baseline, and its tails below it, in em (IBM Plex
/// Sans SemiBold, measured on "Play" and "Lubię to": `label_margins`).
const CAP: f32 = 0.70;
const TAIL: f32 = 0.24;

/// The repeat of the noise pattern, px.
const NOISE_CELL: f32 = 24.0;

/// The patterns a key's background can carry (`Look::pattern`).
pub const PATTERNS: &[&str] = &["stripes", "dots", "grid", "noise", "waves", "hex"];

/// A key's built-in motion (`Look::motion`).
pub const MOTIONS: &[&str] = &["slide", "pulse", "shine", "glow", "sweep", "sparkle", "flow", "film", "glitch", "flicker", "sparks"];

/// Where a motion is in its cycle of `steps` sixteenths of a second, 0..1. The engine gives `t` in
/// whole sixteenths (`Engine::current_frame`), so the same point of a cycle draws the same picture.
fn cycle(t: f32, steps: u32) -> f32 {
    ((t * 16.0).round() as u32 % steps) as f32 / steps as f32
}

/// How far a sliding pattern has moved at `t` seconds: a whole pixel every quarter second (4 px a
/// second, so it drifts, never rushes), round its repeat (8 px stripes, 9 px dots and grid, 24 px
/// noise): few different pictures, each kept in the tile cache, and no blur of half pixels.
fn slide_offset(pattern: &str, t: f32) -> f32 {
    let repeat = match pattern {
        "stripes" => 8.0,
        "noise" => NOISE_CELL,
        "waves" => 18.0,
        "hex" => 15.0,
        _ => 9.0,
    };
    ((t * 4.0).floor() % repeat).max(0.0)
}

/// A pattern over a key's background in `fg`, `opacity` 0..=1, moved `offset` px right, cut to the
/// key (the stripes run past its edge, and the panel's tiles show a little around each key);
/// empty for an unknown one.
fn key_pattern(name: &str, s: f32, fg: &str, opacity: f32, offset: f32) -> String {
    let drawn = pattern_body(name, s, fg, opacity);
    if drawn.is_empty() {
        return drawn;
    }
    // moving: every pattern repeats and reaches one repeat past the left edge, so a shift right
    // never shows a gap
    let moved = if offset > 0.0 { format!(r#"<g transform="translate({offset} 0)">{drawn}</g>"#) } else { drawn };
    // strongest at the edges and fading to nothing behind the icon, so the icon and the label
    // stay clean (owner, 2026-10-25: the patterns looked cheap, flat over the whole key)
    format!(
        r##"<svg width="{s}" height="{s}" viewBox="0 0 {s} {s}"><defs><radialGradient id="kfadeg" cx="0.5" cy="0.45" r="0.62"><stop offset="0.3" stop-color="#000"/><stop offset="1" stop-color="#fff"/></radialGradient><mask id="kfade"><rect width="{s}" height="{s}" fill="url(#kfadeg)"/></mask></defs><g mask="url(#kfade)">{moved}</g></svg>"##
    )
}

/// 0 -> 1 -> 0 over a cycle, smoothly (a breath).
fn breath(p: f32) -> f32 {
    0.5 - 0.5 * (p * std::f32::consts::TAU).cos()
}

/// 0 -> 1 with a soft start and end.
fn ease(p: f32) -> f32 {
    0.5 - 0.5 * (p.clamp(0.0, 1.0) * std::f32::consts::PI).cos()
}

/// A key's built-in motion as what goes under its content and what goes over it, in the key's
/// accent (its icon color): only these layers are drawn again each frame, the label never (owner,
/// 2026-10-25: "te animacje ... mają być topowej jakości" and as light as can be).
fn key_motion(look: &Look, s: f32, accent: &str) -> (String, String) {
    if look.t <= 0.0 {
        return (String::new(), String::new());
    }
    let c = s / 2.0;
    let t = look.t;
    let clip = |inner: String| format!(r#"<svg width="{s}" height="{s}" viewBox="0 0 {s} {s}">{inner}</svg>"#);
    match look.motion.as_deref() {
        // the key breathes: a little lighter, with its edge warming in the accent (2.5 s)
        Some("pulse") => {
            let b = breath(cycle(t, 40));
            let (light, edge) = (0.10 * b, 0.55 * b);
            (
                format!(
                    r##"<defs><radialGradient id="kpulse" cx="0.5" cy="0.5" r="0.72"><stop offset="0.55" stop-color="{accent}" stop-opacity="0"/><stop offset="1" stop-color="{accent}" stop-opacity="{edge:.3}"/></radialGradient></defs><rect width="{s}" height="{s}" fill="#fff" fill-opacity="{light:.3}"/><rect width="{s}" height="{s}" fill="url(#kpulse)"/>"##
                ),
                String::new(),
            )
        }
        // a soft light behind the icon in its own color, rising and settling (3 s)
        Some("glow") => {
            let b = breath(cycle(t, 48));
            let (core, r) = (0.18 + 0.42 * b, 0.42 + 0.12 * b);
            (
                format!(
                    r##"<defs><radialGradient id="kglow" cx="0.5" cy="0.44" r="{r:.3}"><stop offset="0" stop-color="{accent}" stop-opacity="{core:.3}"/><stop offset="0.55" stop-color="{accent}" stop-opacity="{half:.3}"/><stop offset="1" stop-color="{accent}" stop-opacity="0"/></radialGradient></defs><rect width="{s}" height="{s}" fill="url(#kglow)"/>"##,
                    half = core * 0.35
                ),
                String::new(),
            )
        }
        // a glint across the key: a bright core in a soft band, eased, then a rest (every 3 s)
        Some("shine") => {
            let p = cycle(t, 48) / 0.42;
            if p >= 1.0 {
                return (String::new(), String::new());
            }
            let x = -34.0 + ease(p) * (s + 68.0);
            (
                String::new(),
                clip(format!(
                    r##"<defs><linearGradient id="kshine" x1="0" y1="0" x2="1" y2="0"><stop offset="0" stop-color="#fff" stop-opacity="0"/><stop offset="0.38" stop-color="#fff" stop-opacity="0.10"/><stop offset="0.5" stop-color="#fff" stop-opacity="0.42"/><stop offset="0.62" stop-color="#fff" stop-opacity="0.10"/><stop offset="1" stop-color="#fff" stop-opacity="0"/></linearGradient></defs><g transform="rotate(24 {c} {c})"><rect x="{x:.1}" y="-30" width="30" height="{h}" fill="url(#kshine)"/><rect x="{x2:.1}" y="-30" width="3" height="{h}" fill="#fff" fill-opacity="0.12"/></g>"##,
                    h = s + 60.0,
                    x2 = x + 36.0
                )),
            )
        }
        // a light running round the key's edge, with a soft trail (every 3 s)
        Some("sweep") => {
            // inside the key's edge (a stroke on the edge itself is half cut off)
            let (inset, rx) = (3.5, 9.0);
            let w = s - 2.0 * inset;
            let perimeter = 4.0 * w - (8.0 - 2.0 * std::f32::consts::PI) * rx;
            let at = -cycle(t, 48) * perimeter;
            // the head in white over a glow in the accent, and a trail of three fading pieces
            let dash = |len: f32, back: f32, color: &str, opacity: f32, width: f32| {
                format!(
                    r##"<rect x="{inset}" y="{inset}" width="{w}" height="{w}" rx="{rx}" fill="none" stroke="{color}" stroke-opacity="{opacity}" stroke-width="{width}" stroke-dasharray="{len} {gap:.1}" stroke-dashoffset="{off:.1}" stroke-linecap="round"/>"##,
                    gap = perimeter - len,
                    off = at + back
                )
            };
            let trail: String = [(0.42, 16.0), (0.24, 30.0), (0.11, 44.0)].iter().map(|(o, back)| dash(14.0, *back, accent, *o, 2.4)).collect();
            (String::new(), trail + &dash(22.0, 2.0, accent, 0.5, 5.0) + &dash(16.0, 0.0, "#ffffff", 0.95, 2.0))
        }
        // four small stars twinkling in turn (2 s)
        Some("sparkle") => {
            let at = cycle(t, 32);
            let stars: String = [(13.0, 14.0), (59.0, 17.0), (56.0, 57.0), (15.0, 55.0)]
                .iter()
                .enumerate()
                .map(|(k, (x, y))| {
                    let v = breath((at + k as f32 * 0.25) % 1.0).powi(2);
                    let r = 1.5 + 3.5 * v;
                    format!(
                        r##"<path d="M{x} {a}Q{x} {y} {b} {y}Q{x} {y} {x} {d}Q{x} {y} {e} {y}Q{x} {y} {x} {a}Z" fill="#fff" fill-opacity="{o:.2}"/>"##,
                        a = y - r,
                        b = x + r,
                        d = y + r,
                        e = x - r,
                        o = 0.15 + 0.8 * v
                    )
                })
                .collect();
            (String::new(), stars)
        }
        // the background's colors flow round (every 6 s)
        Some("flow") => {
            let bg = color(look.bg.as_ref(), DEFAULT_BG);
            let (a, b) = match look.bg2.as_ref().map(|c| color(Some(c), "")).filter(|c| !c.is_empty()) {
                Some(end) => (bg.clone(), end),
                None => (mix(&bg, "#ffffff", 0.25), mix(&bg, "#000000", 0.35)),
            };
            let angle = (cycle(t, 96) * 360.0).round() as u16 % 360;
            let (x1, y1, x2, y2) = gradient_ends(angle);
            (
                format!(
                    r#"<defs><linearGradient id="kflow" x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}"><stop offset="0" stop-color="{a}"/><stop offset="1" stop-color="{b}"/></linearGradient></defs><rect width="{s}" height="{s}" fill="url(#kflow)"/>"#
                ),
                String::new(),
            )
        }
        // an old projector: the light flickers, a scratch and dust jump from frame to frame
        // (a 1.5 s loop of 24 pictures, so the tile cache keeps them)
        Some("film") => {
            let n = (t * 16.0).round() as u32 % 24;
            let light = (rand(n, 1) - 0.5) * 0.14;
            let (tone, o) = if light > 0.0 { ("#fff", light) } else { ("#000", -light) };
            let mut over = format!(r#"<rect width="{s}" height="{s}" fill="{tone}" fill-opacity="{o:.3}"/>"#);
            if rand(n, 2) < 0.6 {
                let x = 6.0 + rand(n, 3) * (s - 12.0);
                over += &format!(r##"<path d="M{x:.1} 0Q{bend:.1} 36 {x:.1} {s}" stroke="#000" stroke-opacity="0.3" stroke-width="0.7" fill="none"/>"##, bend = x + (rand(n, 4) - 0.5) * 4.0);
            }
            for k in 0..3 {
                let (x, y, r) = (rand(n, 10 + k) * s, rand(n, 20 + k) * s, 0.5 + rand(n, 30 + k) * 0.9);
                over += &format!(r##"<circle cx="{x:.1}" cy="{y:.1}" r="{r:.2}" fill="#000" fill-opacity="0.5"/>"##);
            }
            (String::new(), over)
        }
        // a broken signal: a bright bar rolls down the screen; every 2.5 s a burst of torn
        // colored bands for a quarter second
        Some("glitch") => {
            let roll = cycle(t, 64) * (s + 20.0) - 10.0;
            let mut over = format!(
                r##"<defs><linearGradient id="kroll" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#fff" stop-opacity="0"/><stop offset="0.5" stop-color="#fff" stop-opacity="0.07"/><stop offset="1" stop-color="#fff" stop-opacity="0"/></linearGradient></defs><rect y="{y:.1}" width="{s}" height="12" fill="url(#kroll)"/>"##,
                y = roll - 6.0
            );
            let step = (t * 16.0).round() as u32 % 40;
            if step < 4 {
                for k in 0..3u32 {
                    let y = rand(step, 40 + k) * (s - 6.0);
                    let h = 1.5 + rand(step, 50 + k) * 4.0;
                    let x = (rand(step, 60 + k) - 0.5) * 16.0;
                    let tint = ["#ff2bd6", "#21f5ff", accent][k as usize];
                    over += &format!(r#"<rect x="{x:.1}" y="{y:.1}" width="{s}" height="{h:.1}" fill="{tint}" fill-opacity="0.45"/>"#);
                }
                over += &format!(r##"<rect y="{y:.1}" width="{s}" height="2" fill="#000" fill-opacity="0.6"/>"##, y = rand(step, 70) * s);
            }
            (String::new(), clip(over))
        }
        // a neon tube that hums and now and then stutters (every 4 s)
        Some("flicker") => {
            let step = (t * 16.0).round() as u32 % 64;
            let dark = match step {
                5 | 8 => 0.5,
                6 => 0.25,
                _ => 0.03 * breath(cycle(t, 16)),
            };
            (String::new(), format!(r##"<rect width="{s}" height="{s}" fill="#000" fill-opacity="{dark:.3}"/>"##))
        }
        // sparks rising from the bottom and fading (3 s)
        Some("sparks") => {
            let at = cycle(t, 48);
            let hot = mix(accent, "#fff3c4", 0.5);
            let sparks: String = (0..9u32)
                .map(|k| {
                    let p = (at + rand(0, 80 + k)) % 1.0;
                    let x = 6.0 + rand(0, 90 + k) * (s - 12.0) + (p * 7.0 + k as f32).sin() * 3.0;
                    let y = s + 2.0 - p * (s + 6.0);
                    let o = (1.0 - p) * 0.9;
                    format!(r#"<path d="M{x:.1} {y:.1}l{dx:.1} 2.6" stroke="{hot}" stroke-opacity="{o:.2}" stroke-width="1.1" stroke-linecap="round"/>"#, dx = (rand(0, 100 + k) - 0.5) * 1.6)
                })
                .collect();
            (String::new(), clip(sparks))
        }
        _ => (String::new(), String::new()),
    }
}

/// A fixed number 0..1 for picture `n` and slot `k`: the same frame always draws the same.
fn rand(n: u32, k: u32) -> f32 {
    let mut x = n.wrapping_mul(0x9e37_79b9) ^ k.wrapping_mul(0x85eb_ca6b) ^ 0x27d4_eb2f;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2c1b_3c6d);
    x ^= x >> 12;
    (x % 10_000) as f32 / 10_000.0
}

fn pattern_body(name: &str, s: f32, fg: &str, opacity: f32) -> String {
    let body = match name {
        // diagonal stripes, 8 px apart
        "stripes" => (-1..20).map(|i| format!("M{} 0l-{s} {s}", i as f32 * 8.0)).collect::<String>(),
        "grid" => (-1..9).map(|i| format!("M{v} 0V{s}M-9 {v}H{s}", v = i as f32 * 9.0)).collect::<String>(),
        "dots" | "noise" | "hex" => String::new(),
        // soft waves, 9 px apart, one wave every 18 px across
        "waves" => (-1..9).map(|i| format!("M-18 {}{}", i as f32 * 9.0 + 4.5, "q4.5 -3.2 9 0t9 0".repeat(6))).collect::<String>(),
        _ => return String::new(),
    };
    match name {
        "stripes" => format!(r#"<path d="{body}" stroke="{fg}" stroke-opacity="{opacity}" stroke-width="3"/>"#),
        "grid" => format!(r#"<path d="{body}" stroke="{fg}" stroke-opacity="{opacity}" stroke-width="1"/>"#),
        "waves" => format!(r#"<path d="{body}" fill="none" stroke="{fg}" stroke-opacity="{opacity}" stroke-width="1.6" stroke-linecap="round"/>"#),
        // a honeycomb of flat-top cells, 5 px from centre to corner (repeats every 15 px across)
        "hex" => {
            let r = 5.0_f32;
            let d: String = (-2..12)
                .flat_map(|col| (-1..10).map(move |row| (col, row)))
                .map(|(col, row)| {
                    let (cx, cy) = (col as f32 * 1.5 * r, row as f32 * 3f32.sqrt() * r + if col % 2 != 0 { 3f32.sqrt() * r / 2.0 } else { 0.0 });
                    let pts: Vec<String> = (0..6)
                        .map(|k| {
                            let a = (k as f32 * 60.0).to_radians();
                            format!("{:.1} {:.1}", cx + r * a.cos(), cy + r * a.sin())
                        })
                        .collect();
                    format!("M{}Z", pts.join("L"))
                })
                .collect();
            format!(r#"<path d="{d}" fill="none" stroke="{fg}" stroke-opacity="{opacity}" stroke-width="1.1"/>"#)
        }
        "dots" => {
            let d: String =
                (0..8).flat_map(|r| (-1..8).map(move |c| (r, c))).map(|(r, c)| format!("M{} {}h0", c as f32 * 9.0 + 4.5 + if r % 2 == 1 { 4.5 } else { 0.0 }, r as f32 * 9.0 + 4.5)).collect();
            format!(r#"<path d="{d}" stroke="{fg}" stroke-opacity="{opacity}" stroke-width="3" stroke-linecap="round"/>"#)
        }
        // the same speckles every time (a fixed sequence), so a key never flickers between frames
        _ => {
            let mut x: u32 = 0x9e37_79b9;
            let mut next = || {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                (x % 1000) as f32 / 1000.0
            };
            // 30 specks in a 24 px cell, repeated over the key (and a cell past its left edge for
            // the slide): it reads as noise, and moving it repeats every 24 px (24 pictures)
            let cell: Vec<(f32, f32)> = (0..30).map(|_| (next() * NOISE_CELL, next() * NOISE_CELL)).collect();
            let d: String = (-1..4)
                .flat_map(|cx| (0..3).map(move |cy| (cx as f32 * NOISE_CELL, cy as f32 * NOISE_CELL)))
                .flat_map(|(ox, oy)| cell.iter().map(move |(x, y)| format!("M{:.1} {:.1}h1.4v1.4h-1.4z", ox + x, oy + y)))
                .collect();
            format!(r#"<path d="{d}" fill="{fg}" fill-opacity="{opacity}"/>"#)
        }
    }
}

/// What the icon editor adds on top of a key: a frame in the ring color and the badge.
fn key_extras(look: &Look, s: f32) -> String {
    let mut out = String::new();
    if let Some(ring) = look.ring.as_ref().map(|c| color(Some(c), "")).filter(|c| !c.is_empty()) {
        // the frame and a thin echo of it inside, for depth
        out += &format!(
            r#"<rect x="2" y="2" width="{w}" height="{w}" rx="9" fill="none" stroke="{ring}" stroke-width="3"/><rect x="5.5" y="5.5" width="{w2}" height="{w2}" rx="6.5" fill="none" stroke="{ring}" stroke-opacity="0.35" stroke-width="1"/>"#,
            w = s - 4.0,
            w2 = s - 11.0
        );
    }
    if let Some((one, w)) = badge(look) {
        let fill = color(look.badge_color.as_ref(), "#e5484d");
        let (x, y) = (s - 4.0 - w, 4.0);
        // a dark edge parts it from any background, a light top gives it a little body
        out += &format!(
            r##"<rect x="{x}" y="{y}" width="{w}" height="18" rx="9" fill="{fill}" stroke="#000" stroke-opacity="0.35" stroke-width="1.5"/><rect x="{x1}" y="{y1}" width="{w1}" height="7" rx="3.5" fill="#fff" fill-opacity="0.18"/>"##,
            x1 = x + 3.0,
            y1 = y + 2.0,
            w1 = w - 6.0
        );
        out += &text(x + w / 2.0, y + 13.0, BADGE_PX, "#ffffff", &one);
    }
    out
}

const BADGE_PX: f32 = 10.5;

/// A key's badge text (at most 3 characters) and the width of its pill.
fn badge(look: &Look) -> Option<(String, f32)> {
    let one: String = look.badge.as_deref().map(str::trim).filter(|b| !b.is_empty())?.chars().take(3).collect();
    let w = (text_width(&one, BADGE_PX) + 8.0).max(18.0);
    Some((one, w))
}

/// A key label drawn by `draw` (in the color it is given) with the look's text effect: `shadow` a
/// soft dark copy under it, `outline` a dark edge; with none, the dark edge only where `edge` asks
/// for it (a label over an icon or a picture).
fn label_effect(shift: (i8, i8), effect: Option<&str>, edge: bool, fg: &str, draw: impl Fn(&str) -> String) -> String {
    const EDGE: &str = r##" stroke="#000" stroke-opacity="0.55" stroke-width="2.4" stroke-linejoin="round" paint-order="stroke""##;
    let drawn = label_drawn(effect, edge, fg, draw, EDGE);
    if shift == (0, 0) { drawn } else { format!(r#"<g transform="translate({} {})">{drawn}</g>"#, shift.0, shift.1) }
}

fn label_drawn(effect: Option<&str>, edge: bool, fg: &str, draw: impl Fn(&str) -> String, edge_style: &str) -> String {
    match effect {
        Some("shadow") => format!(r#"<g opacity="0.5" transform="translate(0.6 1.1)">{}</g>{}"#, draw("#000000"), draw(fg)),
        Some("outline") => format!(r##"<g stroke="#000" stroke-opacity="0.85" stroke-width="2.8" stroke-linejoin="round" paint-order="stroke">{}</g>"##, draw(fg)),
        None if edge => format!("<g{edge_style}>{}</g>", draw(fg)),
        _ => draw(fg),
    }
}

fn icon(name: &str, x: f32, y: f32, size: f32, fg: &str) -> String {
    // game icons are filled shapes on a 512 grid; the others are 24 px line drawings
    if name.starts_with(GAME_PREFIX) {
        return match icon_body(name) {
            Some(body) => format!(r#"<g transform="translate({x} {y}) scale({s})" fill="{fg}">{body}</g>"#, s = size / 512.0),
            None => String::new(),
        };
    }
    match icon_body(name) {
        Some(body) => format!(r#"<g transform="translate({x} {y}) scale({s})" fill="none" stroke="{fg}" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{body}</g>"#, s = size / 24.0),
        None => String::new(),
    }
}

/// A caption at the top of a dial image (the dial gallery), turned like the rest of the image.
pub fn with_caption(svg: &str, caption: &str, rotation: i32) -> String {
    let c = DIAL_PX as f32 / 2.0;
    let (rows, px) = fit(caption, 17.0, 12.0, 2.0 * dial_chord(40.0) - 30.0, false);
    let w = text_width(&rows[0], px) + 24.0;
    let pill = format!(
        r##"<g transform="rotate({rotation} {c} {c})"><rect x="{x}" y="22" width="{w}" height="{h}" rx="{r}" fill="#000" fill-opacity="0.6"/>{t}</g>"##,
        x = c - w / 2.0,
        h = px + 12.0,
        r = (px + 12.0) / 2.0,
        t = text(c, 22.0 + 6.0 + px * 0.82, px, "#e6e4ee", &rows[0])
    );
    match svg.rfind("</svg>") {
        Some(end) => format!("{}{pill}</svg>", &svg[..end]),
        None => svg.to_string(),
    }
}

/// A ring around the dial's edge on top of what it shows (the tour points at the dial with it).
pub fn with_ring(svg: &str, color: &str) -> String {
    let c = DIAL_PX as f32 / 2.0;
    let ring = format!(r##"<circle cx="{c}" cy="{c}" r="{r}" fill="none" stroke="{color}" stroke-width="12"/>"##, r = c - 6.0);
    match svg.rfind("</svg>") {
        Some(end) => format!("{}{ring}</svg>", &svg[..end]),
        None => svg.to_string(),
    }
}

/// Lights to the music on the dial (owner, 2026-10-19: "A diale tez moglyby swiecic … tak samo
/// jak podstawa"): a ring round its edge in `color`, as bright as the edge's flash (`level`
/// 0..1, in tenths so a steady glow draws nothing new).
pub fn with_music_ring(svg: &str, color: &str, level: f32) -> String {
    let level = (level.clamp(0.0, 1.0) * 10.0).round() / 10.0;
    if level <= 0.0 {
        return svg.to_string();
    }
    let c = DIAL_PX as f32 / 2.0;
    let ring = format!(r##"<circle cx="{c}" cy="{c}" r="{r}" fill="none" stroke="{color}" stroke-width="10" opacity="{level}"/>"##, r = c - 5.0);
    match svg.rfind("</svg>") {
        Some(end) => format!("{}{ring}</svg>", &svg[..end]),
        None => svg.to_string(),
    }
}

/// Settings > Screens: how app icons on Auto show (`icon`, `fill`, `fillLabel`).
static APP_ICONS: std::sync::Mutex<&'static str> = std::sync::Mutex::new("fillLabel");

pub fn set_app_icons(mode: &str) {
    *APP_ICONS.lock().unwrap_or_else(|e| e.into_inner()) = match mode {
        "fill" => "fill",
        "fillLabel" => "fillLabel",
        _ => "icon",
    };
}

fn app_icons() -> &'static str {
    *APP_ICONS.lock().unwrap_or_else(|e| e.into_inner())
}

/// Whether the look's picture stands like an icon (chosen, or a logo on Auto while app icons show
/// as icons).
fn picture_as_icon(look: &Look) -> bool {
    match look.fit.as_deref() {
        Some("icon") => true,
        Some("fill") => false,
        _ => app_icons() == "icon" && look.image.as_deref().is_some_and(super::images::is_icon),
    }
}

/// The label drawn over a picture: none for a logo on Auto when app icons fill the key without
/// their name.
fn picture_label<'a>(look: &Look, label: &'a str) -> &'a str {
    if look.fit.is_none() && app_icons() == "fill" && look.image.as_deref().is_some_and(super::images::is_icon) { "" } else { label }
}

/// An app's icon or a logo in an icon's place.
fn logo_image(uri: &str, x: f32, y: f32, size: f32) -> String {
    format!(r#"<image href="{uri}" x="{x}" y="{y}" width="{size}" height="{size}" preserveAspectRatio="xMidYMid meet"/>"#)
}

fn text(x: f32, y: f32, size: f32, fg: &str, content: &str) -> String {
    let family = font_family();
    format!(r#"<text x="{x}" y="{y}" font-family="{family}" font-weight="600" font-size="{size}" fill="{fg}" text-anchor="middle">{}</text>"#, escape(content))
}

/// Width of `content` in px at `size`, measured with the font the images are drawn in (the shaped
/// text's bounding box; cached per text).
fn text_width(content: &str, size: f32) -> f32 {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static OPTIONS: OnceLock<usvg::Options<'static>> = OnceLock::new();
    static WIDTHS: OnceLock<Mutex<HashMap<String, f32>>> = OnceLock::new();
    if content.trim().is_empty() {
        return 0.0;
    }
    let widths = WIDTHS.get_or_init(Default::default);
    let family = font_family();
    // per font: another font has other widths
    let key = format!("{family}\u{0}{content}");
    if let Some(w) = widths.lock().unwrap_or_else(|e| e.into_inner()).get(&key) {
        return w * size / 100.0;
    }
    let options = OPTIONS.get_or_init(|| Renderer::default().options);
    let svg =
        format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="20000" height="200"><text x="10" y="150" font-family="{family}" font-weight="600" font-size="100">{}</text></svg>"#, escape(content));
    // if shaping fails, a fair guess for this font: about 0.58 em a character
    let w = usvg::Tree::from_str(&svg, options).map(|t| t.root().abs_bounding_box().width()).unwrap_or(content.chars().count() as f32 * 58.0);
    let mut cache = widths.lock().unwrap_or_else(|e| e.into_inner());
    if cache.len() > 2000 {
        cache.clear(); // live values change; keep the cache small
    }
    cache.insert(key, w);
    w * size / 100.0
}

/// `content` cut to fit `max_w` at `size`, ending in "…".
fn ellipsize(content: &str, size: f32, max_w: f32) -> String {
    if text_width(content, size) <= max_w {
        return content.to_string();
    }
    let chars: Vec<char> = content.chars().collect();
    for n in (1..chars.len()).rev() {
        let cut = format!("{}…", chars[..n].iter().collect::<String>().trim_end());
        if text_width(&cut, size) <= max_w {
            return cut;
        }
    }
    "…".into()
}

/// How a label fits a width: one line at `size` when it fits; else smaller, down to `min`; or (when
/// `two_lines` and it reads bigger) two lines split between words; else cut with "…" at `min`.
/// Returns the lines and their font size.
fn fit(label: &str, size: f32, min: f32, max_w: f32, two_lines: bool) -> (Vec<String>, f32) {
    let w = text_width(label, size);
    if w <= max_w {
        return (vec![label.to_string()], size);
    }
    let one = size * max_w / w;
    let words: Vec<&str> = label.split_whitespace().collect();
    if two_lines && words.len() > 1 {
        // the split whose longer line is shortest
        let longer = |(a, b): &(String, String)| text_width(a, size).max(text_width(b, size));
        let split = (1..words.len()).map(|i| (words[..i].join(" "), words[i..].join(" "))).min_by(|x, y| longer(x).total_cmp(&longer(y))).expect("two words or more");
        let two = (size * max_w / longer(&split)).min(size);
        if two >= min && two > one * 1.15 {
            return (vec![split.0, split.1], two);
        }
        if one < min {
            let two = two.max(min);
            return (vec![ellipsize(&split.0, two, max_w), ellipsize(&split.1, two, max_w)], two);
        }
    }
    if one >= min { (vec![label.to_string()], one) } else { (vec![ellipsize(label, min, max_w)], min) }
}

/// Lines of text centred on `x`; the first baseline at `y`, the next ones `gap` em below.
fn lines(x: f32, y: f32, size: f32, gap: f32, fg: &str, rows: &[String]) -> String {
    rows.iter().enumerate().map(|(i, l)| text(x, y + i as f32 * size * gap, size, fg, l)).collect()
}

/// Half the width of the round dial screen at height `y` (text near its top or bottom has less room).
fn dial_chord(y: f32) -> f32 {
    let r = DIAL_PX as f32 / 2.0;
    let dy = (y - r).abs().min(r);
    (r * r - dy * dy).sqrt()
}

/// A label on its own: bigger when short, smaller or over two lines when long, cut with "…" when
/// even that does not fit. `max_w`: the width it may take.
fn centered_label(label: &str, w: f32, h: f32, max_w: f32, fg: &str) -> String {
    let start = match label.chars().count() {
        0..=3 => w * 0.36,
        4..=6 => w * 0.25,
        _ => w * 0.2,
    };
    let (rows, size) = fit(label, start, w * 0.12, max_w, true);
    let gap = 1.1;
    // the block of lines centred on the middle (cap height about 0.7 em)
    let y = h / 2.0 + size * 0.35 - (rows.len() - 1) as f32 * size * gap / 2.0;
    lines(w / 2.0, y, size, gap, fg, &rows)
}

/// The 480x272 panel behind the 15 keys: key `i` shows the 72x72 square at
/// (`(i % 5) * 102`, `(i / 5) * 100`). Measured on 2026-10-09 by putting a labelled grid on the
/// panel and photographing the keys; the gaps between keys are hidden by the key frames.
pub const PANEL_W: u32 = 480;
pub const PANEL_H: u32 = 272;
/// Where each key's window sits on the panel. Defaults: calibrated on the owner's hardware on
/// 2026-10-09 with the double-border mode (a photo of a labelled grid gave 102/100/72x72 first).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelGeometry {
    /// Left and top edge of the first key's window.
    pub x0: f32,
    pub y0: f32,
    /// Distance from one key's left (top) edge to the next one's.
    pub dx: f32,
    pub dy: f32,
    /// Size of a key's visible window.
    pub w: f32,
    pub h: f32,
}

impl Default for PanelGeometry {
    fn default() -> Self {
        Self { x0: 0.0, y0: 0.0, dx: 100.0, dy: 99.0, w: 79.0, h: 75.0 }
    }
}

/// Calibration parameters in the order the dial steps through them: (field, name).
pub const GEOMETRY_PARAMS: [(&str, &str); 6] = [("x0", "Left edge"), ("y0", "Top edge"), ("dx", "Column spacing"), ("dy", "Row spacing"), ("w", "Key width"), ("h", "Key height")];

impl PanelGeometry {
    pub fn get(&self, i: usize) -> f32 {
        [self.x0, self.y0, self.dx, self.dy, self.w, self.h][i % 6]
    }

    pub fn adjust(&mut self, i: usize, by: f32) {
        let f = match i % 6 {
            0 => &mut self.x0,
            1 => &mut self.y0,
            2 => &mut self.dx,
            3 => &mut self.dy,
            4 => &mut self.w,
            _ => &mut self.h,
        };
        *f = (*f + by).clamp(-40.0, 200.0);
    }

    /// The visible window of key `index` (row by row, 5 per row): x, y, width, height.
    pub fn key_rect(&self, index: usize) -> (f32, f32, f32, f32) {
        (self.x0 + (index % 5) as f32 * self.dx, self.y0 + (index / 5) as f32 * self.dy, self.w, self.h)
    }
}

#[cfg(test)]
pub fn key_origin(index: usize) -> (f32, f32) {
    let (x, y, _, _) = PanelGeometry::default().key_rect(index);
    (x, y)
}

/// A picture filling a `size` square (cropped to fill, clipped to the square), with the label on a
/// dark strip at the bottom.
/// A picture filling the screen, with the label at the bottom on its own shadow; `round` (the dial):
/// the label sits higher, where the round screen is wide enough.
fn picture(uri: &str, size: f32, label: &str, fg: &str, label_px: f32, round: bool) -> String {
    let mut out = format!(r#"<svg width="{size}" height="{size}" viewBox="0 0 {size} {size}"><image href="{uri}" width="{size}" height="{size}" preserveAspectRatio="xMidYMid slice"/></svg>"#);
    if !label.is_empty() {
        let strip = label_px * 1.7;
        let top = if round { size * 0.8 - strip } else { size - strip };
        let baseline = top + strip * 0.7;
        let max_w = if round { 2.0 * dial_chord(baseline) - 16.0 } else { size - 6.0 };
        let (rows, px) = fit(label, label_px, label_px * 0.72, max_w, false);
        let label = text(size / 2.0, baseline - (label_px - px) * 0.35, px, fg, &rows[0]);
        // the label's own shadow, no strip across the picture (owner, 2026-10-30, "2 b": the grey
        // strip cut the logos; https://claude.ai/artifact/49hkreHQu3U55R4iKmEtHG variant C)
        out += r##"<defs><filter id="ts" x="-30%" y="-80%" width="160%" height="260%"><feDropShadow dx="0" dy="0.8" stdDeviation="1.2" flood-color="#000" flood-opacity="0.95"/></filter></defs>"##;
        out += &format!(r#"<g filter="url(#ts)">{label}</g>"#);
    }
    out
}

/// What sits over a key's drawing: a light frame while a press is acknowledged, a red corner
/// with an "!" after a failed press. Inside the turned group, so the corner is top right as read.
fn key_marks(look: &Look, s: f32) -> String {
    let mut out = String::new();
    if look.failed {
        out += &format!(
            r##"<path d="M{a} 0H{s}V{b}Z" fill="#f0506e"/><rect x="{x}" y="4" width="2.6" height="8" rx="1.3" fill="#fff"/><circle cx="{cx}" cy="15" r="1.5" fill="#fff"/>"##,
            a = s - 24.0,
            b = 24.0,
            x = s - 8.3,
            cx = s - 7.0
        );
    }
    if look.pressed {
        // in the key's text color, so it shows on a light key too
        let fg = color(look.fg.as_ref(), DEFAULT_FG);
        out += &format!(r##"<rect x="1.5" y="1.5" width="{w}" height="{w}" rx="10" fill="none" stroke="{fg}" stroke-opacity="0.9" stroke-width="3"/>"##, w = s - 3.0);
    }
    out
}

/// One key's drawing in a 72x72 box, turned `rotation` degrees clockwise about its centre.
fn key_body(look: &Look, rotation: i32) -> String {
    key_layers(look, rotation).concat()
}

/// A key's drawing in three layers, each turned `rotation` degrees about the key's centre: what
/// is under its content (background, pattern, pulse), the content (icon, label, picture), and
/// what is over it (finish, shine, frame, badge, the press and failure marks). A key with motion
/// is drawn layer by layer, so a new frame draws only its moving layer and the label's text,
/// the costly part, once (`panel_tiles`).
fn key_layers(look: &Look, rotation: i32) -> [String; 3] {
    let s = KEY_PX as f32;
    let (under, body, over) = key_parts(look);
    let over = over + &key_marks(look, s);
    let turn = |x: String| if x.is_empty() { x } else { format!(r#"<g transform="rotate({rotation} {c} {c})">{x}</g>"#, c = s / 2.0) };
    [turn(under), turn(body), turn(over)]
}

fn key_parts(look: &Look) -> (String, String, String) {
    let Some(skin) = look.skin.as_deref().filter(|k| SKINS.contains(k)) else {
        return key_parts_plain(look);
    };
    let accent = color(look.icon_color.as_ref(), &color(look.fg.as_ref(), DEFAULT_FG));
    // a picture filling the key keeps its label light on its dark strip, and no tube glow
    let filled = look.image.is_some() && !picture_as_icon(look);
    let mut worn = skinned(look, skin, &accent);
    if filled {
        worn.fg = look.fg.clone();
    }
    let (under, body, over) = key_parts_plain(&worn);
    let (lower, filter, upper) = skin_layers(skin, look, KEY_PX as f32, &accent);
    let filter = if filled && skin == "neon" { "{}".to_string() } else { filter };
    (under + &lower, if body.is_empty() { body } else { filter.replace("{}", &body) }, upper + &over)
}

/// The styles a key can wear whole (`Look::skin`).
pub const SKINS: &[&str] = &["film", "glitch", "neon", "glass", "pumpkin"];

/// The look a skin draws its key from: its own colors in place of the key's, with the key's
/// color kept as a tint (film's paper, glitch's and neon's dark) and its accent for the light.
fn skinned(look: &Look, skin: &str, accent: &str) -> Look {
    let bg = color(look.bg.as_ref(), DEFAULT_BG);
    let plain = Look { style: None, gloss: None, pattern: None, bg2: None, ..look.clone() };
    match skin {
        "film" => Look { bg: Some(mix(&bg, "#ece4d2", 0.84)), fg: Some(FILM_INK.into()), icon_color: Some(FILM_INK.into()), label_effect: None, ..plain },
        "glitch" => Look { bg: Some(mix(&bg, "#090a12", 0.82)), fg: Some("#f4f6ff".into()), icon_color: Some(mix(accent, "#ffffff", 0.25)), label_effect: None, ..plain },
        // Halloween: the icon and the label lit from inside like a carved pumpkin, on a dark key
        "pumpkin" => Look { bg: Some(mix(&bg, "#0b0710", 0.9)), fg: Some("#ffd27a".into()), icon_color: Some("#ffe3a3".into()), label_effect: None, ..plain },
        "neon" => Look {
            bg: Some(mix(&bg, "#06060b", 0.88)),
            fg: Some(mix(accent, "#ffffff", 0.55)),
            icon_color: Some(mix(accent, "#ffffff", 0.7)),
            label_effect: None,
            // labels stay inside the tube
            inset: 3.0,
            ..plain
        },
        // glass keeps the key's own color: it is the glass's tint
        _ => Look { label_effect: None, ..plain },
    }
}

const FILM_INK: &str = "#16130f";

/// A skin's layers round the key's content: what goes under it (light, texture), a filter its
/// icon and label are drawn through (`{}` stands for them), and what goes over it.
fn skin_layers(skin: &str, look: &Look, s: f32, accent: &str) -> (String, String, String) {
    let tag = &accent[1..];
    let grain = |id: &str, freq: f32, alpha: f32, seed: u32| {
        format!(
            r#"<filter id="{id}" x="0" y="0" width="100%" height="100%"><feTurbulence type="fractalNoise" baseFrequency="{freq}" numOctaves="2" seed="{seed}" stitchTiles="stitch"/><feColorMatrix type="saturate" values="0"/><feComponentTransfer><feFuncA type="table" tableValues="0 {alpha}"/></feComponentTransfer></filter>"#
        )
    };
    match skin {
        // an old film frame (as Matinee's pack): a spotlight on paper, black corners, grain,
        // scratches and dust; the icon and the label in worn ink
        "film" => (
            format!(
                r##"<defs><radialGradient id="kfilml" cx="0.5" cy="0.44" r="0.74"><stop offset="0" stop-color="#fffaf0" stop-opacity="0.6"/><stop offset="0.5" stop-color="#fffaf0" stop-opacity="0"/><stop offset="0.78" stop-color="#000" stop-opacity="0.32"/><stop offset="1" stop-color="#000" stop-opacity="0.85"/></radialGradient>{g}</defs><rect width="{s}" height="{s}" fill="url(#kfilml)"/><rect width="{s}" height="{s}" filter="url(#kfilmg)" opacity="0.55"/><path d="M14 0Q15.5 30 13 72M57 0Q55.8 40 58.5 72" stroke="#000" stroke-opacity="0.16" stroke-width="0.6" fill="none"/><path d="M9 21h1.2v1.2h-1.2zM62 48h1.4v1h-1.4zM47 9h1v1.6h-1zM20 63h1.6v1.2h-1.6z" fill="#000" fill-opacity="0.45"/>"##,
                g = grain("kfilmg", 1.15, 0.6, 7)
            ),
            r#"<defs><filter id="kfilmi" x="-10%" y="-10%" width="120%" height="120%"><feColorMatrix type="saturate" values="0" result="m"/><feTurbulence type="fractalNoise" baseFrequency="0.35" numOctaves="1" seed="3" result="n"/><feDisplacementMap in="m" in2="n" scale="0.7" xChannelSelector="R" yChannelSelector="G"/></filter></defs><g filter="url(#kfilmi)">{}</g>"#.into(),
            String::new(),
        ),
        // a CRT screen gone wrong (as the Glitch pack): the icon and the label split into red and
        // cyan, a soft phosphor glow, scan lines and a curved glass over it
        "glitch" => (
            format!(
                r##"<defs><radialGradient id="kgl{tag}" cx="0.5" cy="0.42" r="0.6"><stop offset="0" stop-color="{accent}" stop-opacity="0.22"/><stop offset="1" stop-color="{accent}" stop-opacity="0"/></radialGradient></defs><rect width="{s}" height="{s}" fill="url(#kgl{tag})"/>"##
            ),
            r#"<defs><filter id="kgli" x="-15%" y="-15%" width="130%" height="130%"><feColorMatrix in="SourceGraphic" type="matrix" values="1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0" result="r"/><feOffset in="r" dx="-1.3" result="r2"/><feColorMatrix in="SourceGraphic" type="matrix" values="0 0 0 0 0 0 1 0 0 0 0 0 1 0 0 0 0 0 1 0" result="c"/><feOffset in="c" dx="1.3" result="c2"/><feBlend in="r2" in2="c2" mode="screen" result="rc"/><feGaussianBlur in="SourceGraphic" stdDeviation="2.4" result="b"/><feComponentTransfer in="b" result="glow"><feFuncA type="linear" slope="0.7"/></feComponentTransfer><feMerge><feMergeNode in="glow"/><feMergeNode in="rc"/></feMerge></filter></defs><g filter="url(#kgli)">{}</g>"#.into(),
            format!(
                r##"<defs><pattern id="kscan" width="4" height="3" patternUnits="userSpaceOnUse"><rect width="4" height="1.2" fill="#000" fill-opacity="0.32"/></pattern><radialGradient id="kcrt" cx="0.5" cy="0.5" r="0.72"><stop offset="0.62" stop-color="#000" stop-opacity="0"/><stop offset="1" stop-color="#000" stop-opacity="0.7"/></radialGradient><radialGradient id="kcrtg" cx="0.5" cy="0.08" r="0.55" gradientTransform="translate(0.5 0.08) scale(1 0.45) translate(-0.5 -0.08)"><stop offset="0" stop-color="#fff" stop-opacity="0.16"/><stop offset="1" stop-color="#fff" stop-opacity="0"/></radialGradient></defs><rect width="{s}" height="{s}" fill="url(#kscan)"/><rect width="{s}" height="{s}" fill="url(#kcrt)"/><rect width="{s}" height="{s}" fill="url(#kcrtg)"/>"##
            ),
        ),
        // a carved pumpkin: candle light rising from below, the dark of the shell at the edges,
        // and the icon and the label glowing orange from inside with a hot yellow core
        "pumpkin" => (
            r##"<defs><radialGradient id="kpumpl" cx="0.5" cy="0.95" r="0.85"><stop offset="0" stop-color="#ff7a1a" stop-opacity="0.38"/><stop offset="0.55" stop-color="#b8360a" stop-opacity="0.12"/><stop offset="1" stop-color="#000" stop-opacity="0"/></radialGradient><radialGradient id="kpumpv" cx="0.5" cy="0.5" r="0.72"><stop offset="0.6" stop-color="#000" stop-opacity="0"/><stop offset="1" stop-color="#000" stop-opacity="0.6"/></radialGradient></defs><rect width="72" height="72" fill="url(#kpumpl)"/><rect width="72" height="72" fill="url(#kpumpv)"/>"##.into(),
            r##"<defs><filter id="kpumpi" x="-30%" y="-30%" width="160%" height="160%"><feGaussianBlur in="SourceAlpha" stdDeviation="3.2" result="b1"/><feFlood flood-color="#ff6a00" flood-opacity="0.95"/><feComposite in2="b1" operator="in" result="g1"/><feGaussianBlur in="SourceAlpha" stdDeviation="1.0" result="b2"/><feFlood flood-color="#ffae3b"/><feComposite in2="b2" operator="in" result="g2"/><feMerge><feMergeNode in="g1"/><feMergeNode in="g1"/><feMergeNode in="g2"/><feMergeNode in="SourceGraphic"/></feMerge></filter></defs><g filter="url(#kpumpi)">{}</g>"##.into(),
            String::new(),
        ),
        // a neon sign on a dark wall: a tube round the key, the icon and the label glowing in the
        // key's accent, the light falling on the wall behind them
        "neon" => {
            let tube = mix(accent, "#ffffff", 0.35);
            (
                format!(
                    r##"<defs><radialGradient id="kneonw{tag}" cx="0.5" cy="0.42" r="0.62"><stop offset="0" stop-color="{accent}" stop-opacity="0.3"/><stop offset="1" stop-color="{accent}" stop-opacity="0"/></radialGradient><filter id="kneont{tag}" x="-20%" y="-20%" width="140%" height="140%"><feGaussianBlur in="SourceGraphic" stdDeviation="2.2" result="b"/><feMerge><feMergeNode in="b"/><feMergeNode in="SourceGraphic"/></feMerge></filter>{g}</defs><rect width="{s}" height="{s}" filter="url(#kneong)" opacity="0.5"/><rect width="{s}" height="{s}" fill="url(#kneonw{tag})"/><rect x="3" y="3" width="{w}" height="{w}" rx="10" fill="none" stroke="{tube}" stroke-opacity="0.85" stroke-width="1.5" filter="url(#kneont{tag})"/>"##,
                    w = s - 6.0,
                    g = grain("kneong", 1.4, 0.35, 5)
                ),
                format!(
                    r#"<defs><filter id="kneoni{tag}" x="-30%" y="-30%" width="160%" height="160%"><feGaussianBlur in="SourceAlpha" stdDeviation="3.6" result="b1"/><feFlood flood-color="{accent}" flood-opacity="0.9"/><feComposite in2="b1" operator="in" result="g1"/><feGaussianBlur in="SourceAlpha" stdDeviation="1.1" result="b2"/><feFlood flood-color="{accent}"/><feComposite in2="b2" operator="in" result="g2"/><feMerge><feMergeNode in="g1"/><feMergeNode in="g1"/><feMergeNode in="g2"/><feMergeNode in="SourceGraphic"/></feMerge></filter></defs><g filter="url(#kneoni{tag})">{{}}</g>"#
                ),
                String::new(),
            )
        }
        // a glass keycap: the key's color lit from below, a bevel catching the light at the top
        // and a curved reflection over the upper half; the icon and the label stand out by a shadow
        _ => {
            let bg = color(look.bg.as_ref(), DEFAULT_BG);
            let (top, bottom) = (mix(&bg, "#ffffff", 0.2), mix(&bg, "#000000", 0.45));
            let id = format!("kglass{}", &bg[1..]);
            (
                format!(
                    r##"<defs><linearGradient id="{id}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="{top}"/><stop offset="1" stop-color="{bottom}"/></linearGradient><radialGradient id="{id}u" cx="0.5" cy="1.05" r="0.6"><stop offset="0" stop-color="{accent}" stop-opacity="0.45"/><stop offset="1" stop-color="{accent}" stop-opacity="0"/></radialGradient></defs><rect width="{s}" height="{s}" fill="url(#{id})"/><rect width="{s}" height="{s}" fill="url(#{id}u)"/>"##
                ),
                r##"<defs><filter id="kglassi" x="-20%" y="-20%" width="140%" height="140%"><feDropShadow dx="0" dy="1.6" stdDeviation="1.4" flood-color="#000" flood-opacity="0.55"/></filter></defs><g filter="url(#kglassi)">{}</g>"##.into(),
                format!(
                    r##"<defs><linearGradient id="kglassr" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#fff" stop-opacity="0.3"/><stop offset="1" stop-color="#fff" stop-opacity="0.02"/></linearGradient><linearGradient id="kglassb" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#fff" stop-opacity="0.6"/><stop offset="0.35" stop-color="#fff" stop-opacity="0.08"/><stop offset="0.75" stop-color="#000" stop-opacity="0.05"/><stop offset="1" stop-color="#000" stop-opacity="0.45"/></linearGradient></defs><path d="M3 12Q3 3 12 3H60Q69 3 69 12V24Q36 33 3 24Z" fill="url(#kglassr)"/><rect x="1.5" y="1.5" width="{w}" height="{w}" rx="10" fill="none" stroke="url(#kglassb)" stroke-width="1.6"/>"##,
                    w = s - 3.0
                ),
            )
        }
    }
}

fn key_parts_plain(look: &Look) -> (String, String, String) {
    let _font = KeyFont::set(look.font.as_deref());
    let s = KEY_PX as f32;
    // the width a label may take (less inside a skin's frame)
    let w = s - 2.0 * look.inset;
    let shift = (look.label_dx.clamp(-36, 36), look.label_dy.clamp(-36, 36));
    let bg = color(look.bg.as_ref(), DEFAULT_BG);
    let fg = color(look.fg.as_ref(), DEFAULT_FG);
    let label = look.label.as_deref().unwrap_or("").trim();
    let (under, over) = key_background(look, s, &bg);
    let under = match look.pattern.as_deref() {
        Some(p) => {
            let offset = if look.motion.as_deref() == Some("slide") && look.t > 0.0 { slide_offset(p, look.t) } else { 0.0 };
            under + &key_pattern(p, s, &fg, look.pattern_opacity.map_or(0.18, |o| o.clamp(5, 100) as f32 / 100.0), offset)
        }
        None => under,
    };
    let (pulse, shine) = key_motion(look, s, &color(look.icon_color.as_ref(), &fg));
    let under = under + &pulse;
    let over = over + &shine + &key_extras(look, s);
    let effect = look.label_effect.as_deref();
    let ic = color(look.icon_color.as_ref(), &fg);
    // an app's icon or a logo stands where an icon would; other pictures fill the key
    let logo = look.image.as_deref().filter(|_| picture_as_icon(look)).and_then(super::images::uri);
    if logo.is_none()
        && let Some(uri) = look.image.as_deref().and_then(super::images::filled_uri)
    {
        return (under, picture(&uri, s, picture_label(look, label), &fg, 11.5, false), over);
    }
    if look.face {
        let (main, second) = label.split_once('\n').map(|(a, b)| (a, Some(b))).unwrap_or((label, None));
        let start = match main.chars().count() {
            0..=4 => 26.0,
            5 => 23.0,
            6..=7 => 17.0,
            _ => 14.0,
        };
        let (main_rows, size) = fit(main, start, 9.0, w - 6.0, false);
        let y = if second.is_some() { s / 2.0 + size * 0.1 } else { s / 2.0 + size * 0.35 };
        let mut body = text(s / 2.0, y, size, &fg, &main_rows[0]);
        if let Some(second) = second {
            let (rows, px) = fit(second, 11.0, 8.0, w - 6.0, false);
            body += &text(s / 2.0, y + 19.0, px, &fg, &rows[0]);
        }
        return (under, body, over);
    }
    let emoji = look.icon.as_deref().and_then(|i| i.strip_prefix(EMOJI_PREFIX)).filter(|e| !e.trim().is_empty());
    let icon_name = look.icon.as_deref().filter(|n| icon_svg(n).is_some());
    let scale = match look.icon_size.as_deref() {
        Some("small") => 0.7,
        Some("large") => 1.3,
        _ => 1.0,
    };
    // the icon's square, made smaller or bigger around its own centre and kept on the key
    let mark = |x: f32, y: f32, size: f32| {
        let new = (size * scale).min(s - 4.0);
        let (x, y) = ((x + (size - new) / 2.0).clamp(2.0, s - 2.0 - new), (y + (size - new) / 2.0).clamp(2.0, s - 2.0 - new));
        // moved by hand in the full editor
        let (x, y) = (x + look.icon_dx.clamp(-36, 36) as f32, y + look.icon_dy.clamp(-36, 36) as f32);
        match (&logo, emoji, icon_name) {
            (Some(uri), _, _) => logo_image(uri, x, y, new),
            (None, Some(e), _) => emoji_mark(e, x, y, new),
            (None, None, Some(i)) => icon(i, x, y, new, &ic),
            _ => String::new(),
        }
    };
    let has_mark = logo.is_some() || emoji.is_some() || icon_name.is_some();
    let size = look.label_size.map(|px| (px as f32).clamp(6.0, 30.0));
    let body = match (look.label_pos.as_deref(), has_mark, label.is_empty()) {
        // placed as on Stream Deck: the icon in the middle, the label over it at the top, middle
        // or bottom, with a dark edge so it reads on any icon
        (Some(pos), _, false) => {
            // a label at the top keeps clear of the badge at the top right (centred, so both sides)
            let room = match (pos, badge(look)) {
                ("top", Some((_, badge_w))) => s - 2.0 * (badge_w + 6.0),
                _ => w - 6.0,
            };
            let (rows, px) = fit(label, size.unwrap_or(11.5), size.map_or(7.5, |z| z.min(7.5)), room, true);
            let gap = 1.1;
            let block = (rows.len() - 1) as f32 * px * gap;
            // the same clear edge at the top and the bottom, from the letters' tops to their tails
            // (owner, 2026-10-25: Bottom sat too low and Top too high; measured 1-6 px, on the frame)
            let edge = if look.ring.is_some() { LABEL_EDGE + 2.0 } else { LABEL_EDGE };
            let y = match pos {
                "top" => edge + px * CAP,
                "middle" => s / 2.0 + px * 0.35 - block / 2.0,
                _ => s - edge - px * TAIL - block,
            };
            mark(16.0, 16.0, 40.0) + &label_effect(shift, effect, has_mark, &fg, |c| lines(s / 2.0, y, px, gap, c, &rows))
        }
        (_, true, true) => mark(16.0, 16.0, 40.0),
        (_, true, false) => match fit(label, size.unwrap_or(11.5), size.map_or(9.5, |z| z.min(9.5)), w - 6.0, true) {
            (rows, px) if rows.len() == 1 => mark(21.0, 8.0, 30.0) + &label_effect(shift, effect, false, &fg, |c| text(s / 2.0, 61.0, px, c, &rows[0])),
            // a long label: a smaller icon above two lines
            (rows, px) => mark(25.0, 6.0, 22.0) + &label_effect(shift, effect, false, &fg, |c| lines(s / 2.0, 47.0, if size.is_some() { px } else { px.min(10.5) }, 1.12, c, &rows)),
        },
        (_, false, false) => match size {
            Some(z) => {
                let (rows, px) = fit(label, z, z.min(7.5), w - 8.0, true);
                label_effect(shift, effect, false, &fg, |c| lines(s / 2.0, s / 2.0 + px * 0.35 - (rows.len() - 1) as f32 * px * 1.1 / 2.0, px, 1.1, c, &rows))
            }
            None => label_effect(shift, effect, false, &fg, |c| centered_label(label, s, s, w - 8.0, c)),
        },
        (_, false, true) => String::new(),
    };
    (under, body, over)
}

/// `Look::icon` of an emoji: `emoji:🎵`.
pub const EMOJI_PREFIX: &str = "emoji:";

/// An emoji in the system's color emoji font, filling the icon's square.
fn emoji_mark(emoji: &str, x: f32, y: f32, size: f32) -> String {
    // one emoji (a flag or a family is several characters): at most 8 characters of it
    let one: String = emoji.trim().chars().take(8).collect();
    format!(
        r#"<text x="{}" y="{}" font-size="{}" text-anchor="middle" font-family="Segoe UI Emoji, Apple Color Emoji, Noto Color Emoji">{}</text>"#,
        x + size / 2.0,
        y + size * 0.86,
        size * 0.92,
        escape(&one)
    )
}

/// `rotation`: clockwise degrees applied to the content so it reads upright on the deck.
/// A key's drawing at `size` px square (drawn at 72 and scaled), upright: to save or share it.
pub fn key_svg_sized(look: &Look, size: u32) -> String {
    let s = KEY_PX as f32;
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="{size}" height="{size}" viewBox="0 0 {s} {s}">{}</svg>"#, key_body(look, 0))
}

pub fn key_svg(look: &Look, rotation: i32) -> String {
    let s = KEY_PX as f32;
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="{s}" height="{s}" viewBox="0 0 {s} {s}">{}</svg>"#, key_body(look, rotation))
}

fn panel_open() -> String {
    format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="{PANEL_W}" height="{PANEL_H}" viewBox="0 0 {PANEL_W} {PANEL_H}"><rect width="{PANEL_W}" height="{PANEL_H}" fill="#000"/>"##)
}

/// All 15 keys as one panel image: one transfer, and every key changes at the same moment. The
/// engine draws it from tiles now ([`panel_tiles`]); this whole-panel SVG is their reference.
#[cfg(test)]
pub fn panel_svg(looks: &[Look], rotation: i32, geometry: &PanelGeometry) -> String {
    let mut out = panel_open();
    // the background bleeds half-way into the hidden gaps, so a pixel of misalignment between
    // the panel and a key's window never shows a black edge
    let bleed_x = ((geometry.dx - geometry.w) / 2.0).max(0.0);
    let bleed_y = ((geometry.dy - geometry.h) / 2.0).max(0.0);
    for (i, look) in looks.iter().enumerate().take(15) {
        let (x, y, w, h) = geometry.key_rect(i);
        let bg = if look.style.as_deref() == Some("outline") { OUTLINE_FILL.to_string() } else { color(look.bg.as_ref(), DEFAULT_BG) };
        out += &format!(r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{bg}"/>"#, x - bleed_x, y - bleed_y, w + 2.0 * bleed_x, h + 2.0 * bleed_y);
        // the 72x72 drawing is scaled to fill the window (keeping its proportions) and centred
        let scale = (w.min(h) / KEY_PX as f32).max(0.1);
        let side = KEY_PX as f32 * scale;
        let (cx, cy) = (x + (w - side) / 2.0, y + (h - side) / 2.0);
        out += &format!(r#"<g transform="translate({cx} {cy}) scale({scale})">{}</g>"#, key_body(look, rotation));
    }
    out + "</svg>"
}

/// A screen's pixels as JPEG. The pixmap is opaque (every screen has a background), so its
/// premultiplied RGBA is the plain colour and the alpha byte is simply left out by the encoder.
pub fn encode_jpeg(pixmap: &tiny_skia::Pixmap, quality: u8) -> Result<Vec<u8>, String> {
    let (w, h) = (u16::try_from(pixmap.width()).map_err(|e| e.to_string())?, u16::try_from(pixmap.height()).map_err(|e| e.to_string())?);
    let mut out = Vec::with_capacity(32 * 1024);
    let mut encoder = jpeg_encoder::Encoder::new(&mut out, quality);
    // every pixel's own colour (below quality 90 the encoder halves the colour, which smears red
    // and orange edges: the owner saw it on Firefox's icon, 2026-10-26); the standard Huffman
    // tables only: with tables made for the picture the deck drew the first rows and then black
    // (camera, 2026-10-26 00:36), its decoder takes the standard tables
    encoder.set_sampling_factor(jpeg_encoder::SamplingFactor::F_1_1);
    encoder.encode(pixmap.data(), w, h, jpeg_encoder::ColorType::Rgba).map_err(|e| e.to_string())?;
    Ok(out)
}

/// Where the deck shows key `i`'s own image on the key panel: a 72x72 square, to the pixel
/// (camera, 2026-10-26, `PROTOCOL.md`: the first fit `4 + 99.5 x column, 1 + 99.1 x row` put the
/// left column and the middle row a pixel off, and keys jumped whenever the panel went out whole
/// between their squares).
pub fn key_slot(i: usize) -> (usize, usize) {
    ([5, 104, 203, 303, 402][i % 5], [1, 101, 199][(i / 5) % 3])
}

const SLOT: usize = 72;

/// `src` laid over `dst` at whole pixels (premultiplied RGBA, source-over), the way tiny-skia's
/// `draw_pixmap` does it but ten times quicker: an opaque pixel is copied, a clear one skipped, a
/// partly clear one blended two channels at a time with exact rounding. Session 33: laying the
/// cached tiles took 2.0 ms a frame on the owner's deck (45 tiles and layers), 5 % of a core at
/// 24 pictures a second.
pub fn blit(dst: &mut tiny_skia::Pixmap, src: &tiny_skia::Pixmap, x: i32, y: i32) {
    let (dw, dh) = (dst.width() as i32, dst.height() as i32);
    let (sw, sh) = (src.width() as i32, src.height() as i32);
    let (x0, y0) = (x.max(0), y.max(0));
    let (x1, y1) = ((x + sw).min(dw), (y + sh).min(dh));
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    let width = (x1 - x0) as usize;
    let sdata = src.data();
    let ddata = dst.data_mut();
    for row in y0..y1 {
        let s_at = (((row - y) * sw + (x0 - x)) * 4) as usize;
        let d_at = ((row * dw + x0) * 4) as usize;
        let s_row = &sdata[s_at..s_at + width * 4];
        let d_row = &mut ddata[d_at..d_at + width * 4];
        for (s, d) in s_row.as_chunks::<4>().0.iter().zip(d_row.as_chunks_mut::<4>().0) {
            let sa = s[3];
            if sa == 255 {
                d.copy_from_slice(s);
            } else if sa != 0 {
                let keep = 255 - sa as u32;
                let dp = u32::from_le_bytes([d[0], d[1], d[2], d[3]]);
                let sp = u32::from_le_bytes([s[0], s[1], s[2], s[3]]);
                // channels 0 and 2, then 1 and 3, each times `keep` and divided by 255 rounded
                let div = |v: u32| {
                    let v = v + 0x0080_0080;
                    ((v + ((v >> 8) & 0x00ff_00ff)) >> 8) & 0x00ff_00ff
                };
                let even = div((dp & 0x00ff_00ff) * keep);
                let odd = div(((dp >> 8) & 0x00ff_00ff) * keep) << 8;
                d.copy_from_slice(&(sp + (even | odd)).to_le_bytes());
            }
        }
    }
}

/// The keys whose square changed between two panel pictures (RGBA), or None when something
/// visible changed outside the squares too (in a key's window around its square: then the whole
/// panel goes out). A change of a few levels at the window's edge counts as none.
pub fn changed_keys(old: &[u8], new: &[u8], geometry: &PanelGeometry) -> Option<Vec<usize>> {
    let w = PANEL_W as usize;
    if old.len() != new.len() {
        return None;
    }
    let differs = |x: usize, y: usize, by: u8| {
        let i = (y * w + x) * 4;
        (0..3).any(|k| old[i + k].abs_diff(new[i + k]) > by)
    };
    let mut keys = Vec::new();
    for i in 0..15 {
        let (sx, sy) = key_slot(i);
        if (sy..sy + SLOT).any(|y| (sx..sx + SLOT).any(|x| differs(x, y, 0))) {
            keys.push(i);
        }
        // the rest of the key's window
        let (wx, wy, ww, wh) = geometry.key_rect(i);
        let (x0, y0) = (wx.max(0.0) as usize, wy.max(0.0) as usize);
        let (x1, y1) = (((wx + ww) as usize).min(w), ((wy + wh) as usize).min(PANEL_H as usize));
        for y in y0..y1 {
            for x in x0..x1 {
                let inside = x >= sx && x < sx + SLOT && y >= sy && y < sy + SLOT;
                if !inside && differs(x, y, 6) {
                    return None;
                }
            }
        }
    }
    Some(keys)
}

/// Key `i`'s square cut from the panel picture, as the JPEG the deck takes for one key.
pub fn key_slot_jpeg(panel: &tiny_skia::Pixmap, i: usize) -> Result<Vec<u8>, String> {
    let (sx, sy) = key_slot(i);
    let w = PANEL_W as usize;
    let mut cut = tiny_skia::Pixmap::new(SLOT as u32, SLOT as u32).ok_or("empty key")?;
    for y in 0..SLOT {
        let from = ((sy + y) * w + sx) * 4;
        cut.data_mut()[y * SLOT * 4..(y + 1) * SLOT * 4].copy_from_slice(&panel.data()[from..from + SLOT * 4]);
    }
    encode_jpeg(&cut, KEY_QUALITY)
}

/// The deck now has these keys' squares from `new`.
pub fn copy_key_slots(old: &mut [u8], new: &[u8], keys: &[usize]) {
    let w = PANEL_W as usize;
    for &i in keys {
        let (sx, sy) = key_slot(i);
        for y in sy..sy + SLOT {
            let a = (y * w + sx) * 4;
            old[a..a + SLOT * 4].copy_from_slice(&new[a..a + SLOT * 4]);
        }
    }
}

/// The key panel as 15 tiles instead of one SVG: each tile is the same drawing as in
/// [`panel_svg`] (its background with the bleed and the key), cut out by its viewBox at whole
/// pixels. Tiles that did not change are not drawn again ([`Renderer::panel_jpeg`]), so a page
/// of GIFs draws only the keys whose frame moved (2026-10-14: every frame drew all 15 pictures,
/// 20.7 % of a core on the busiest deck).
pub struct Tile {
    pub x: i32,
    pub y: i32,
    pub svg: String,
    /// A moving key's layers (under, content, over), each its own SVG of the tile's size, drawn
    /// and kept apart; empty for a still key (one drawing, `svg`).
    pub layers: Vec<String>,
    /// A moving key's content without its text (labels, values): the icon or the picture alone,
    /// for the living deck's layers that make objects of icons and must leave labels as they are.
    pub icons: Option<String>,
}

/// An SVG with its `<text>` elements and the dark strips under labels (`class="plate"`) taken out.
fn without_text(svg: &str) -> String {
    let mut plain = String::with_capacity(svg.len());
    let mut rest = svg;
    while let Some(at) = rest.find("<rect class=\"plate\"") {
        plain.push_str(&rest[..at]);
        match rest[at..].find("/>") {
            Some(end) => rest = &rest[at + end + 2..],
            None => {
                rest = "";
                break;
            }
        }
    }
    plain.push_str(rest);
    let svg = plain.as_str();
    let mut out = String::with_capacity(svg.len());
    let mut rest = svg;
    while let Some(at) = rest.find("<text") {
        out.push_str(&rest[..at]);
        match rest[at..].find("</text>") {
            Some(end) => rest = &rest[at + end + "</text>".len()..],
            None => {
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

pub fn panel_tiles(looks: &[Look], rotation: i32, geometry: &PanelGeometry) -> Vec<Tile> {
    let bleed_x = ((geometry.dx - geometry.w) / 2.0).max(0.0);
    let bleed_y = ((geometry.dy - geometry.h) / 2.0).max(0.0);
    looks
        .iter()
        .enumerate()
        .take(15)
        .map(|(i, look)| {
            let (x, y, w, h) = geometry.key_rect(i);
            let (left, top) = ((x - bleed_x).floor(), (y - bleed_y).floor());
            let (tw, th) = (((x + w + bleed_x).ceil() - left).max(1.0), ((y + h + bleed_y).ceil() - top).max(1.0));
            let bg = if look.style.as_deref() == Some("outline") { OUTLINE_FILL.to_string() } else { color(look.bg.as_ref(), DEFAULT_BG) };
            let scale = (w.min(h) / KEY_PX as f32).max(0.1);
            let side = KEY_PX as f32 * scale;
            let (cx, cy) = (x + (w - side) / 2.0, y + (h - side) / 2.0);
            let open = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="{tw}" height="{th}" viewBox="{left} {top} {tw} {th}">"#);
            let back = format!(r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{bg}"/>"#, x - bleed_x, y - bleed_y, w + 2.0 * bleed_x, h + 2.0 * bleed_y);
            let place = |inner: &str| format!(r#"<g transform="translate({cx} {cy}) scale({scale})">{inner}</g>"#);
            if look.t > 0.0 {
                let [under, body, over] = key_layers(look, rotation);
                // the first layer (it holds the tile's background) and the content always, so the
                // content is the second (the living deck's mask), what is over it when it draws something
                let mut layers = vec![format!("{open}{back}{}</svg>", place(&under)), format!("{open}{}</svg>", place(&body))];
                if !over.is_empty() {
                    layers.push(format!("{open}{}</svg>", place(&over)));
                }
                let icons = Some(format!("{open}{}</svg>", place(&without_text(&body))));
                return Tile { x: left as i32, y: top as i32, svg: layers.concat(), layers, icons };
            }
            let svg = format!("{open}{back}{}</svg>", place(&key_body(look, rotation)));
            Tile { x: left as i32, y: top as i32, svg, layers: Vec::new(), icons: None }
        })
        .collect()
}

/// One picture or a clock across the whole key panel, upright for the user: for a module turned
/// 90 or 270 degrees the content is laid out portrait and turned onto the landscape panel.
pub fn panel_content_svg(content: &super::profile::PanelContent, rotation: i32, time: &str, history: &[f32]) -> String {
    let (w, h) = if rotation % 180 == 0 { (PANEL_W as f32, PANEL_H as f32) } else { (PANEL_H as f32, PANEL_W as f32) };
    let (x, y) = ((PANEL_W as f32 - w) / 2.0, (PANEL_H as f32 - h) / 2.0);
    let (cx, cy) = (PANEL_W as f32 / 2.0, PANEL_H as f32 / 2.0);
    let inner = match content {
        super::profile::PanelContent::Picture { image } => match super::images::uri(image) {
            Some(uri) => format!(r#"<svg x="{x}" y="{y}" width="{w}" height="{h}" viewBox="0 0 {w} {h}"><image href="{uri}" width="{w}" height="{h}" preserveAspectRatio="xMidYMid slice"/></svg>"#),
            None => String::new(),
        },
        super::profile::PanelContent::Clock => text(cx, cy + w.min(h) * 0.2, w.min(h) * 0.55, DEFAULT_FG, time),
        super::profile::PanelContent::Graph { metric } => graph(*metric, history, x, y, w, h),
    };
    format!(r#"{}<g transform="rotate({rotation} {cx} {cy})">{inner}</g></svg>"#, panel_open())
}

/// A filled line graph of `history` (oldest first, 0..=100) with the metric's name and latest value.
fn graph(metric: super::monitor::Metric, history: &[f32], x: f32, y: f32, w: f32, h: f32) -> String {
    let n = super::monitor::HISTORY.max(2);
    let (top, bottom) = (y + h * 0.28, y + h - 6.0);
    let point = |i: usize, v: f32| (x + w * (n - history.len() + i) as f32 / (n - 1) as f32, bottom - (bottom - top) * v.clamp(0.0, 100.0) / 100.0);
    let mut out = String::new();
    for frac in [0.25f32, 0.5, 0.75] {
        let gy = bottom - (bottom - top) * frac;
        out += &format!(r##"<line x1="{x}" y1="{gy}" x2="{}" y2="{gy}" stroke="#2f2c3a" stroke-width="1"/>"##, x + w);
    }
    if history.len() >= 2 {
        let pts: Vec<String> = history
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let (px, py) = point(i, *v);
                format!("{px:.1},{py:.1}")
            })
            .collect();
        let (first_x, _) = point(0, 0.0);
        out += &format!(r##"<polygon points="{first_x:.1},{bottom} {} {:.1},{bottom}" fill="#8b7cf6" fill-opacity="0.35"/>"##, pts.join(" "), x + w);
        out += &format!(r##"<polyline points="{}" fill="none" stroke="#c4b5fd" stroke-width="3" stroke-linejoin="round"/>"##, pts.join(" "));
    }
    let latest = history.last().map(|v| metric.format(*v)).unwrap_or_else(|| "–".into());
    out += &format!(r#"<text x="{}" y="{}" font-family="{}" font-weight="600" font-size="{}" fill="{DEFAULT_FG}">{} {}</text>"#, x + 14.0, y + h * 0.2, font_family(), h * 0.14, metric.name(), latest);
    out
}

/// Calibration image: every key gets a green line just inside the assumed window edge and a red
/// line just outside it. Right when, on all four sides of every key, red is hidden and green shows.
pub fn calibration_svg(geometry: &PanelGeometry) -> String {
    let mut out = panel_open();
    for i in 0..15 {
        let (x, y, w, h) = geometry.key_rect(i);
        out += &format!(
            r##"<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="#26232f"/><rect x="{}" y="{}" width="{}" height="{}" fill="none" stroke="#ff2d2d" stroke-width="2"/><rect x="{}" y="{}" width="{}" height="{}" fill="none" stroke="#2dff6a" stroke-width="2"/>{}"##,
            x - 1.0,
            y - 1.0,
            w + 2.0,
            h + 2.0,
            x + 1.0,
            y + 1.0,
            w - 2.0,
            h - 2.0,
            text(x + w / 2.0, y + h / 2.0 + 7.0, 20.0, "#f0eef6", &(i + 1).to_string())
        );
    }
    out + "</svg>"
}

/// Dial screen during calibration: which parameter the dial changes and its value.
pub fn calibration_dial_svg(name: &str, value: f32, step: usize, steps: usize, rotation: i32) -> String {
    let s = DIAL_PX as f32;
    let c = s / 2.0;
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{s}" height="{s}" viewBox="0 0 {s} {s}"><g transform="rotate({rotation} {c} {c})"><rect width="{s}" height="{s}" fill="#14121a"/>{}{}{}{}</g></svg>"##,
        text(c, 70.0, 20.0, "#a8a4b5", &format!("{step} / {steps}")),
        text(c, 112.0, 24.0, "#f0eef6", name),
        text(c, 172.0, 52.0, "#c4b5fd", &format!("{value:.0}")),
        text(c, 205.0, 16.0, "#a8a4b5", "turn: change · press: next")
    )
}

/// Dial screen: the binding's icon and label, or a value ring with a number while something is
/// being turned (`value` 0.0..=1.0, `caption` under it).
pub fn dial_svg(look: &Look, value: Option<(Option<f32>, &str)>, rotation: i32) -> String {
    let s = DIAL_PX as f32;
    let c = s / 2.0;
    let bg = color(look.bg.as_ref(), DEFAULT_BG);
    let fg = color(look.fg.as_ref(), DEFAULT_FG);
    let body = match value {
        Some((None, caption)) => centered_label(caption, s, s, s * 0.78, &fg),
        Some((Some(v), caption)) => {
            let v = v.clamp(0.0, 1.0);
            let r = 104.0;
            let angle = v * 359.9_f32;
            let (sx, sy) = (c, c - r);
            let rad = (angle - 90.0).to_radians();
            let (ex, ey) = (c + r * rad.cos(), c + r * rad.sin());
            let large = if angle > 180.0 { 1 } else { 0 };
            let (caption, caption_px) = fit(caption, 20.0, 14.0, 2.0 * dial_chord(c + 62.0) - 40.0, false);
            // muted: the crossed speaker instead of a 0 (a 0 reads as a volume, not as "off")
            let middle = if caption[0].to_lowercase().ends_with("muted") || caption[0].to_lowercase().ends_with("wyciszone") {
                icon("volume-x", c - 30.0, c - 42.0, 60.0, &fg)
            } else {
                text(c, c + 22.0, 64.0, &fg, &format!("{}", (v * 100.0).round() as i32))
            };
            let arc =
                if v > 0.0 { format!(r##"<path d="M{sx} {sy} A{r} {r} 0 {large} 1 {ex:.2} {ey:.2}" fill="none" stroke="#c4b5fd" stroke-width="12" stroke-linecap="round"/>"##) } else { String::new() };
            format!(r##"<circle cx="{c}" cy="{c}" r="{r}" fill="none" stroke="#3a3646" stroke-width="12"/>{arc}{middle}{}"##, text(c, c + 62.0, caption_px, "#a8a4b5", &caption[0]))
        }
        None if !picture_as_icon(look) && look.image.as_deref().and_then(super::images::uri).is_some() => {
            let uri = look.image.as_deref().and_then(super::images::filled_uri).unwrap_or_default();
            picture(&uri, s, picture_label(look, look.label.as_deref().unwrap_or("").trim()), &fg, 24.0, true)
        }
        None => {
            let label = look.label.as_deref().unwrap_or("").trim();
            let icon_name = look.icon.as_deref().filter(|n| icon_svg(n).is_some());
            let ic = color(look.icon_color.as_ref(), &fg);
            let logo = look.image.as_deref().and_then(super::images::uri);
            let mark = |x: f32, y: f32, size: f32| match (&logo, icon_name) {
                (Some(uri), _) => logo_image(uri, x, y, size),
                (None, Some(i)) => icon(i, x, y, size, &ic),
                _ => String::new(),
            };
            match (logo.is_some() || icon_name.is_some(), label.is_empty()) {
                (true, true) => mark(c - 40.0, c - 40.0, 80.0),
                (true, false) => match fit(label, 26.0, 20.0, 2.0 * dial_chord(176.0) - 24.0, true) {
                    (rows, px) if rows.len() == 1 => mark(c - 34.0, 48.0, 68.0) + &text(c, 168.0, px, &fg, &rows[0]),
                    // two lines: the icon moves up a little
                    (rows, px) => mark(c - 30.0, 40.0, 60.0) + &lines(c, 150.0, px, 1.12, &fg, &rows),
                },
                (false, false) => centered_label(label, s, s, s * 0.78, &fg),
                (false, true) => String::new(),
            }
        }
    };
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{s}" height="{s}" viewBox="0 0 {s} {s}"><g transform="rotate({rotation} {c} {c})"><rect width="{s}" height="{s}" fill="{bg}"/>{body}</g></svg>"#
    )
}

/// The dial for a clock or a timer: the time big in the middle, a second line under it, and for a
/// timer a ring of the time left.
pub fn time_dial_svg(look: &Look, main: &str, second: Option<&str>, ring: Option<f32>, rotation: i32) -> String {
    let s = DIAL_PX as f32;
    let c = s / 2.0;
    let bg = color(look.bg.as_ref(), DEFAULT_BG);
    let fg = color(look.fg.as_ref(), DEFAULT_FG);
    let start = match main.chars().count() {
        0..=5 => 64.0,
        6..=7 => 52.0,
        _ => 44.0,
    };
    let (main_rows, size) = fit(main, start, 24.0, s * 0.82, false);
    let main = main_rows[0].as_str();
    let mut body = String::new();
    if let Some(v) = ring {
        let v = v.clamp(0.0, 1.0);
        let r = 104.0;
        let angle = v * 359.9_f32;
        let rad = (angle - 90.0).to_radians();
        let (ex, ey) = (c + r * rad.cos(), c + r * rad.sin());
        let large = if angle > 180.0 { 1 } else { 0 };
        body += &format!(r##"<circle cx="{c}" cy="{c}" r="{r}" fill="none" stroke="#3a3646" stroke-width="12"/>"##);
        if v > 0.0 {
            body += &format!(r##"<path d="M{c} {} A{r} {r} 0 {large} 1 {ex:.2} {ey:.2}" fill="none" stroke="#c4b5fd" stroke-width="12" stroke-linecap="round"/>"##, c - r);
        }
    }
    body += &text(c, c + size * 0.35, size, &fg, main);
    if let Some(second) = second.filter(|t| !t.trim().is_empty()) {
        let y = c + size * 0.35 + 40.0;
        let (rows, px) = fit(second, 20.0, 14.0, 2.0 * dial_chord(y) - 40.0, false);
        body += &text(c, y, px, "#a8a4b5", &rows[0]);
    }
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{s}" height="{s}" viewBox="0 0 {s} {s}"><g transform="rotate({rotation} {c} {c})"><rect width="{s}" height="{s}" fill="{bg}"/>{body}</g></svg>"#
    )
}

/// What the weather dial shows: the hours ahead (0 = now), or today, tomorrow, the next days.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct WeatherView {
    pub hour: usize,
    /// 0 hours, 1 today, 2 tomorrow, 3 three days.
    pub mode: u8,
}

/// The weather as a scene on the dial (owner's pick W1): the sky of the weather code, day or
/// night, moving with `t` (seconds), the temperature and a line under it.
pub fn weather_dial_svg(look: &Look, fc: Option<&super::live::Forecast>, now_temp: Option<f64>, view: WeatherView, t: f32, polish: bool, rotation: i32) -> String {
    let s = DIAL_PX as f32;
    let c = s / 2.0;
    let wrap = |body: String| format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="{s}" height="{s}" viewBox="0 0 {s} {s}"><g transform="rotate({rotation} {c} {c})">{body}</g></svg>"#);
    let Some(fc) = fc else {
        let label = look.label.clone().unwrap_or_else(|| (if polish { "Pogoda" } else { "Weather" }).into());
        return time_dial_svg(&Look { icon: None, ..look.clone() }, "…", Some(&label), None, rotation);
    };
    let word = |code: i64| if polish { super::live::condition_pl(code) } else { super::live::condition(code) };
    let deg = |v: f64| format!("{}°", v.round() as i64);
    let day_name = |date: &str, i: usize| -> String {
        match (i, polish) {
            (0, false) => "Today".into(),
            (0, true) => "Dziś".into(),
            (1, false) => "Tomorrow".into(),
            (1, true) => "Jutro".into(),
            _ => chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
                .map(|d| {
                    let n = d.format("%a").to_string();
                    if polish { ["pon", "wt", "śr", "czw", "pt", "sob", "nd"][chrono::Datelike::weekday(&d).num_days_from_monday() as usize].to_string() } else { n }
                })
                .unwrap_or_default(),
        }
    };
    // three days: a quiet list, no scene
    if view.mode == 3 {
        let mut body = format!(r##"<rect width="{s}" height="{s}" fill="#121a2a"/>"##);
        for (i, (date, lo, hi, code)) in fc.days.iter().take(3).enumerate() {
            let y = 78.0 + i as f32 * 44.0;
            body += &text(c, y, 19.0, "#ffffff", &format!("{}  {} / {}", day_name(date, i), deg(*lo), deg(*hi)));
            body += &text(c, y + 17.0, 12.0, "#9fb3d1", word(*code));
        }
        return wrap(body);
    }
    let (code, temp, line) = match view.mode {
        1 | 2 => {
            let i = view.mode as usize - 1;
            match fc.days.get(i) {
                Some((date, lo, hi, code)) => (*code, deg(*hi), format!("{} · {} / {}", day_name(date, i), deg(*lo), deg(*hi))),
                None => (fc.code, now_temp.map(deg).unwrap_or_default(), fc.place.clone()),
            }
        }
        _ => match fc.hours.get(view.hour).filter(|_| view.hour > 0) {
            Some((hour, temp, code)) => (*code, deg(*temp), format!("{hour} · {}", word(*code))),
            None => (fc.code, now_temp.map(deg).unwrap_or_default(), format!("{} · {}", fc.place, word(fc.code))),
        },
    };
    let night = !fc.day && view.mode == 0 && view.hour == 0;
    let kind = match code {
        0 | 1 => "clear",
        2 => "part",
        3 => "cloud",
        45 | 48 => "fog",
        51..=57 | 61..=67 | 80..=82 => "rain",
        71..=77 | 85 | 86 => "snow",
        95..=99 => "storm",
        _ => "cloud",
    };
    let (top, bottom) = match (kind, night) {
        (_, true) => ("#0b1230", "#04060f"),
        ("clear" | "part", false) => ("#2f6fb3", "#123056"),
        ("storm", false) => ("#2a2f45", "#0e1018"),
        ("snow", false) => ("#5a7192", "#26344a"),
        _ => ("#1f3b63", "#0f1726"),
    };
    let mut body = format!(
        r##"<defs><linearGradient id="{id}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="{top}"/><stop offset="1" stop-color="{bottom}"/></linearGradient></defs><rect width="{s}" height="{s}" fill="url(#{id})"/>"##,
        // its own id per sky: the window shows several dials on one page
        id = format!("wsky{}{}", &top[1..], &bottom[1..])
    );
    // stars at night, twinkling
    if night {
        for i in 0..14 {
            let (x, y) = (40.0 + ((i * 53) % 160) as f32, 40.0 + ((i * 37) % 90) as f32);
            let o = 0.35 + 0.65 * ((t * 1.7 + i as f32 * 1.3).sin() * 0.5 + 0.5);
            body += &format!(r##"<circle cx="{x}" cy="{y}" r="1.6" fill="#fff" opacity="{o:.2}"/>"##);
        }
    }
    let sun = |body: &mut String, x: f32, y: f32| {
        if night {
            *body += &format!(r##"<circle cx="{x}" cy="{y}" r="22" fill="#e8ecff"/><circle cx="{:.1}" cy="{:.1}" r="20" fill="{top}"/>"##, x + 10.0, y - 6.0);
        } else {
            let a = t * 20.0;
            *body += &format!(r##"<g transform="rotate({a:.1} {x} {y})" stroke="#ffd166" stroke-width="4" stroke-linecap="round">"##);
            for k in 0..8 {
                let r = (k as f32 * 45.0).to_radians();
                *body += &format!(r#"<line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}"/>"#, x + 30.0 * r.cos(), y + 30.0 * r.sin(), x + 40.0 * r.cos(), y + 40.0 * r.sin());
            }
            *body += &format!(r##"</g><circle cx="{x}" cy="{y}" r="22" fill="#ffc35a"/>"##);
        }
    };
    let cloud = |body: &mut String, dx: f32, y: f32, tone: &str| {
        *body += &format!(
            r#"<g fill="{tone}" transform="translate({dx:.1} 0)"><ellipse cx="112" cy="{y}" rx="44" ry="24"/><ellipse cx="140" cy="{:.1}" rx="30" ry="22"/><ellipse cx="90" cy="{:.1}" rx="26" ry="16"/></g>"#,
            y - 8.0,
            y + 8.0
        );
    };
    let drift = (t * 0.6).sin() * 8.0;
    match kind {
        "clear" => sun(&mut body, c, 84.0),
        "part" => {
            sun(&mut body, 96.0, 74.0);
            cloud(&mut body, 14.0 + drift, 96.0, "#dfe6f1");
        }
        "fog" => {
            for i in 0..4 {
                let y = 64.0 + i as f32 * 20.0;
                let dx = ((t * 0.5 + i as f32).sin() * 14.0) as i32;
                body += &format!(r##"<rect x="{}" y="{y}" width="150" height="8" rx="4" fill="#cfd8e6" opacity="0.5"/>"##, 45 + dx);
            }
        }
        _ => {
            let tone = if kind == "storm" { "#8a93a8" } else { "#cfd8e6" };
            cloud(&mut body, drift, 76.0, tone);
            if kind == "rain" || kind == "storm" {
                for i in 0..10 {
                    let x = 78.0 + ((i * 29) % 84) as f32;
                    let y = 100.0 + ((t * 110.0 + (i * 23) as f32) % 30.0);
                    body += &format!(r##"<line x1="{x}" y1="{y:.1}" x2="{:.1}" y2="{:.1}" stroke="#7fb3ff" stroke-width="3" stroke-linecap="round"/>"##, x - 4.0, y + 11.0);
                }
            }
            if kind == "snow" {
                for i in 0..12 {
                    let x = 74.0 + ((i * 31) % 92) as f32 + (t * 1.3 + i as f32).sin() * 5.0;
                    let y = 102.0 + ((t * 28.0 + (i * 19) as f32) % 36.0);
                    body += &format!(r##"<circle cx="{x:.1}" cy="{y:.1}" r="3" fill="#ffffff"/>"##);
                }
            }
            if kind == "storm" && (t * 1.3).fract() < 0.12 {
                body += &format!(
                    r##"<rect width="{s}" height="{s}" fill="#ffffff" opacity="0.25"/><path d="M126 98 L112 126 L124 126 L114 152" fill="none" stroke="#ffe066" stroke-width="5" stroke-linejoin="round"/>"##
                );
            }
        }
    }
    body += &text(c, 190.0, 40.0, "#ffffff", &temp);
    let (rows, px) = fit(&line, 14.0, 11.0, 2.0 * dial_chord(212.0) - 30.0, false);
    body += &text(c, 212.0, px, "#c7d3e6", &rows[0]);
    wrap(body)
}

/// A pomodoro on the dial (owner's pick P1): the time left in a ring, red while focusing and green
/// on a break, the phase under it and the rounds as dots.
pub fn pomodoro_dial_svg(look: &Look, pomo: &super::timer::Pomodoro, now: std::time::Instant, phase: &str, rotation: i32) -> String {
    let (main, ring, on_break, done, rounds) = (pomo.timer.label(now), pomo.timer.fraction(now), pomo.on_break, pomo.done, pomo.rounds);
    let main = main.as_str();
    let s = DIAL_PX as f32;
    let c = s / 2.0;
    let bg = color(look.bg.as_ref(), DEFAULT_BG);
    let fg = color(look.fg.as_ref(), DEFAULT_FG);
    let tone = if on_break { "#3fb950" } else { "#f05252" };
    let r = 100.0;
    let v = ring.clamp(0.0, 1.0);
    let mut body = format!(r##"<circle cx="{c}" cy="{c}" r="{r}" fill="none" stroke="#2a2833" stroke-width="12"/>"##);
    if v > 0.0 {
        let angle = v * 359.9_f32;
        let rad = (angle - 90.0).to_radians();
        let (ex, ey) = (c + r * rad.cos(), c + r * rad.sin());
        let large = if angle > 180.0 { 1 } else { 0 };
        body += &format!(r##"<path d="M{c} {} A{r} {r} 0 {large} 1 {ex:.2} {ey:.2}" fill="none" stroke="{tone}" stroke-width="12" stroke-linecap="round"/>"##, c - r);
    }
    let (rows, size) = fit(main, 54.0, 28.0, s * 0.66, false);
    body += &text(c, c + size * 0.3, size, &fg, &rows[0]);
    let (phase_rows, px) = fit(phase, 16.0, 12.0, s * 0.6, false);
    body += &text(c, c + size * 0.3 + 26.0, px, "#a8a4b5", &phase_rows[0]);
    // the rounds: done ones in the phase's color
    let n = rounds.clamp(1, 12) as f32;
    let gap = 18.0;
    let x0 = c - gap * (n - 1.0) / 2.0;
    for i in 0..rounds.clamp(1, 12) {
        let fill = if i < done { tone } else { "#3a3846" };
        body += &format!(r#"<circle cx="{:.1}" cy="{}" r="5.5" fill="{fill}"/>"#, x0 + gap * i as f32, c + 62.0);
    }
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{s}" height="{s}" viewBox="0 0 {s} {s}"><g transform="rotate({rotation} {c} {c})"><rect width="{s}" height="{s}" fill="{bg}"/>{body}</g></svg>"#
    )
}

/// The dial for "Now playing": the cover fills the screen, dimmed at the bottom under the title and
/// artist; a pause sign while paused. Without a track, the look's own face.
/// A value as a white ring along the dial's edge, on a dark track.
fn edge_ring(c: f32, v: f32) -> String {
    let v = v.clamp(0.0, 1.0);
    let r = 110.0;
    let mut out = format!(r##"<circle cx="{c}" cy="{c}" r="{r}" fill="none" stroke="#3a3646" stroke-opacity="0.85" stroke-width="7"/>"##);
    if v > 0.0 {
        let angle = v * 359.9_f32;
        let rad = (angle - 90.0).to_radians();
        let (ex, ey) = (c + r * rad.cos(), c + r * rad.sin());
        let large = if angle > 180.0 { 1 } else { 0 };
        out += &format!(r##"<path d="M{c} {} A{r} {r} 0 {large} 1 {ex:.2} {ey:.2}" fill="none" stroke="#c4b5fd" stroke-width="7" stroke-linecap="round"/>"##, c - r);
    }
    out
}

/// What plays now on the dial: its cover, title and artist; `progress` (0..=1) as a ring along the
/// edge when the player tells where in the song it is (owner: turning moves in the song, so show where).
pub fn now_playing_dial_svg(look: &Look, track: Option<(&str, &str, bool, Option<&str>)>, progress: Option<f32>, rotation: i32) -> String {
    let Some((title, artist, playing, cover)) = track else { return dial_svg(look, None, rotation) };
    let s = DIAL_PX as f32;
    let c = s / 2.0;
    let bg = color(look.bg.as_ref(), DEFAULT_BG);
    let mut body = match cover {
        Some(uri) => format!(r#"<image href="{uri}" width="{s}" height="{s}" preserveAspectRatio="xMidYMid slice"/>"#),
        // no cover: the deck's own background; paused shows the pause sign where the note was
        None if !playing => format!(r#"<rect width="{s}" height="{s}" fill="{bg}"/>"#),
        None => format!(r#"<rect width="{s}" height="{s}" fill="{bg}"/>"#) + &icon("music", c - 36.0, 46.0, 72.0, "#c4b5fd"),
    };
    body += r##"<defs><linearGradient id="fade" x1="0" y1="0" x2="0" y2="1"><stop offset="0.45" stop-color="#000" stop-opacity="0"/><stop offset="1" stop-color="#000" stop-opacity="0.85"/></linearGradient></defs>"##;
    body += &format!(r#"<rect width="{s}" height="{s}" fill="url(#fade)"/>"#);
    let (title, title_px) = fit(title, 22.0, 17.0, 2.0 * dial_chord(172.0) - 24.0, false);
    let (artist, artist_px) = fit(artist, 16.0, 13.0, 2.0 * dial_chord(198.0) - 24.0, false);
    body += &text(c, 172.0, title_px, DEFAULT_FG, &title[0]);
    body += &text(c, 198.0, artist_px, "#c9c4d6", &artist[0]);
    if !playing {
        body += &format!(
            r##"<rect x="{}" y="58" width="14" height="52" rx="4" fill="#fff" fill-opacity="0.9"/><rect x="{}" y="58" width="14" height="52" rx="4" fill="#fff" fill-opacity="0.9"/>"##,
            c - 19.0,
            c + 5.0
        );
    }
    if let Some(v) = progress {
        body += &edge_ring(c, v);
    }
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{s}" height="{s}" viewBox="0 0 {s} {s}"><g transform="rotate({rotation} {c} {c})"><rect width="{s}" height="{s}" fill="#000"/>{body}</g></svg>"##
    )
}

/// A question an assistant asks on the deck: a "?" mark on top, the question in up to three lines.
pub fn question_dial_svg(question: &str, rotation: i32) -> String {
    let s = DIAL_PX as f32;
    let c = s / 2.0;
    let (rows, px) = fit(question, 26.0, 16.0, 2.0 * dial_chord(c + 10.0) - 44.0, true);
    let start = c + 18.0 - (rows.len() as f32 - 1.0) * px * 0.56;
    let mark = format!(r##"<circle cx="{c}" cy="64" r="22" fill="#c4b5fd"/>{}"##, text(c, 64.0 + 10.0, 28.0, "#1b1a20", "?"));
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{s}" height="{s}" viewBox="0 0 {s} {s}"><g transform="rotate({rotation} {c} {c})"><rect width="{s}" height="{s}" fill="{}"/>{mark}{}</g></svg>"#,
        super::house::BG,
        lines(c, start, px, 1.12, super::house::LABEL, &rows)
    )
}

/// A picture over the whole dial (a song's cover from a plugin) with a title and a line on a dark
/// fade, and a value as a ring around the edge.
pub fn cover_dial_svg(cover: &str, title: &str, line: &str, ring: Option<f32>, rotation: i32) -> String {
    let s = DIAL_PX as f32;
    let c = s / 2.0;
    let mut body = format!(r#"<image href="{cover}" width="{s}" height="{s}" preserveAspectRatio="xMidYMid slice"/>"#);
    body += r##"<defs><linearGradient id="fade" x1="0" y1="0" x2="0" y2="1"><stop offset="0.4" stop-color="#000" stop-opacity="0"/><stop offset="1" stop-color="#000" stop-opacity="0.88"/></linearGradient></defs>"##;
    body += &format!(r#"<rect width="{s}" height="{s}" fill="url(#fade)"/>"#);
    let (title, title_px) = fit(title, 22.0, 15.0, 2.0 * dial_chord(166.0) - 28.0, false);
    let (line, line_px) = fit(line, 18.0, 13.0, 2.0 * dial_chord(192.0) - 28.0, false);
    body += &text(c, 166.0, title_px, DEFAULT_FG, &title[0]);
    body += &text(c, 192.0, line_px, "#d6d1e3", &line[0]);
    if let Some(v) = ring {
        body += &edge_ring(c, v);
    }
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{s}" height="{s}" viewBox="0 0 {s} {s}"><g transform="rotate({rotation} {c} {c})"><rect width="{s}" height="{s}" fill="#000"/>{body}</g></svg>"##
    )
}

/// A game's picture for the whole key panel (480x272 content), turned like the key faces.
pub fn panel_body_svg(body: &str, rotation: i32) -> String {
    format!(r#"{}<g transform="rotate({rotation} {} {})">{body}</g></svg>"#, panel_open(), PANEL_W as f32 / 2.0, PANEL_H as f32 / 2.0)
}

/// A game's dial drawing (240x240 content), turned upright like every other dial image.
pub fn dial_body_svg(body: &str, rotation: i32) -> String {
    let s = DIAL_PX as f32;
    let c = s / 2.0;
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="{s}" height="{s}" viewBox="0 0 {s} {s}"><g transform="rotate({rotation} {c} {c})">{body}</g></svg>"#)
}

/// JPEG quality. USB is not the bottleneck (a 14 KB panel image takes ~11 ms), so keep icons crisp.
pub const KEY_QUALITY: u8 = 98;
const DIAL_QUALITY: u8 = 90;

pub struct Renderer {
    options: usvg::Options<'static>,
    /// Key tiles drawn before, by the hash of their SVG (one per key look and GIF frame).
    tiles: std::sync::Mutex<TileCache>,
}

/// Key tiles (and moving keys' layers) kept in each of the cache's two generations (a tile is
/// 25-40 KB): a page of GIFs or of moving keys keeps its whole cycle; the most it holds is two
/// generations, reached only by many moving keys.
const TILES_KEPT: usize = 400;

/// Drawn tiles by the hash of their SVG, in two generations: a full new one becomes the old one and
/// the one before it goes, and a tile found in the old one moves back to the new one. What is
/// used stays; a page or look left behind goes (the cache emptied itself whole before: the text
/// of still keys was drawn again whenever many moving frames filled it, session 30).
#[derive(Default)]
struct TileCache {
    new: std::collections::HashMap<u64, std::sync::Arc<tiny_skia::Pixmap>>,
    old: std::collections::HashMap<u64, std::sync::Arc<tiny_skia::Pixmap>>,
}

impl TileCache {
    fn get(&mut self, key: u64) -> Option<std::sync::Arc<tiny_skia::Pixmap>> {
        if let Some(p) = self.new.get(&key) {
            return Some(p.clone());
        }
        let p = self.old.remove(&key)?;
        self.insert(key, p.clone());
        Some(p)
    }

    fn insert(&mut self, key: u64, p: std::sync::Arc<tiny_skia::Pixmap>) {
        if self.new.len() >= TILES_KEPT {
            self.old = std::mem::take(&mut self.new);
        }
        self.new.insert(key, p);
    }
}

impl Default for Renderer {
    fn default() -> Self {
        let options = usvg::Options { fontdb: FONTS.clone(), font_family: FONT_FAMILY.into(), ..Default::default() };
        Self { options, tiles: Default::default() }
    }
}

impl Renderer {
    pub fn jpeg(&self, svg: &str) -> Result<Vec<u8>, String> {
        self.jpeg_with(svg, None)
    }

    /// `quality`: override the default for this image size (used by the rendering comparison).
    pub fn jpeg_with(&self, svg: &str, quality: Option<u8>) -> Result<Vec<u8>, String> {
        let started = std::time::Instant::now();
        let tree = usvg::Tree::from_str(svg, &self.options).map_err(|e| e.to_string())?;
        let size = tree.size().to_int_size();
        let mut pixmap = tiny_skia::Pixmap::new(size.width(), size.height()).ok_or("empty image")?;
        resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());
        let rendered = started.elapsed();
        let quality = quality.unwrap_or(if size.width() == DIAL_PX { DIAL_QUALITY } else { KEY_QUALITY });
        let out = encode_jpeg(&pixmap, quality)?;
        log::debug!("image {}x{}: render {:?}, jpeg {:?}", size.width(), size.height(), rendered, started.elapsed() - rendered);
        Ok(out)
    }

    /// The key panel from tiles ([`panel_tiles`]): a tile drawn before is taken from memory.
    /// The same picture as [`Renderer::jpeg_with`] of [`panel_svg`].
    #[cfg(test)]
    pub fn panel_jpeg(&self, tiles: &[Tile], quality: Option<u8>) -> Result<Vec<u8>, String> {
        encode_jpeg(&self.panel_pixmap_with(tiles, None)?, quality.unwrap_or(KEY_QUALITY))
    }

    /// The panel with the living deck's canvas, as pixels: every key's first layer (its
    /// background), the canvas's `under`, the keys' content, the canvas's `over`; the content is
    /// drawn as it always is.
    pub fn panel_pixmap_with(&self, tiles: &[Tile], canvas: Option<&mut super::canvas::Canvas>) -> Result<tiny_skia::Pixmap, String> {
        let mut panel = tiny_skia::Pixmap::new(PANEL_W, PANEL_H).ok_or("empty image")?;
        panel.fill(tiny_skia::Color::BLACK);
        let first = tiles.iter().map(|t| (t, if t.layers.is_empty() { &t.svg } else { &t.layers[0] }));
        match canvas {
            Some(c) => {
                self.draw_tiles(&mut panel, first)?;
                // the content (icons, labels, pictures) on its own first: its cover is the mask the
                // effects settle on and light through (and the halo behind it is made from, so
                // before `under`), then it goes on the panel as before
                let mut content = tiny_skia::Pixmap::new(PANEL_W, PANEL_H).ok_or("empty image")?;
                self.draw_tiles(&mut content, tiles.iter().filter_map(|t| t.layers.get(1).map(|l| (t, l))))?;
                c.content(content.data());
                // a test build can keep the keys' content as raw RGBA, premultiplied, 480x272 (to try
                // other halos behind it offline: scripts/ui/halo-lab.py)
                if option_env!("HUBDECK_TEST_BUILD").is_some()
                    && let Some(path) = std::env::var_os("HUBDECK_DUMP_CONTENT")
                {
                    let _ = std::fs::write(path, content.data());
                }
                c.under(panel.data_mut());
                // the icons alone (the content without its text), for layers that make objects of them
                if c.wants_icons() {
                    let mut icons = tiny_skia::Pixmap::new(PANEL_W, PANEL_H).ok_or("empty image")?;
                    self.draw_tiles(&mut icons, tiles.iter().filter_map(|t| t.icons.as_ref().map(|l| (t, l))))?;
                    c.icons(icons.data());
                }
                blit(&mut panel, &content, 0, 0);
                self.draw_tiles(&mut panel, tiles.iter().flat_map(|t| t.layers.iter().skip(2).map(move |l| (t, l))))?;
                c.over(panel.data_mut());
            }
            None => {
                let drawings = tiles.iter().flat_map(|t| if t.layers.is_empty() { vec![(t, &t.svg)] } else { t.layers.iter().map(|l| (t, l)).collect() });
                self.draw_tiles(&mut panel, drawings)?;
            }
        }
        Ok(panel)
    }

    fn draw_tiles<'a>(&self, panel: &mut tiny_skia::Pixmap, drawings: impl Iterator<Item = (&'a Tile, &'a String)>) -> Result<(), String> {
        use std::hash::{Hash, Hasher};
        for (tile, svg) in drawings {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            svg.hash(&mut h);
            let key = h.finish();
            let kept = self.tiles.lock().unwrap_or_else(|e| e.into_inner()).get(key);
            let pixmap = match kept {
                Some(p) => p,
                None => {
                    let tree = usvg::Tree::from_str(svg, &self.options).map_err(|e| e.to_string())?;
                    let size = tree.size().to_int_size();
                    let mut p = tiny_skia::Pixmap::new(size.width(), size.height()).ok_or("empty tile")?;
                    resvg::render(&tree, tiny_skia::Transform::default(), &mut p.as_mut());
                    let p = std::sync::Arc::new(p);
                    self.tiles.lock().unwrap_or_else(|e| e.into_inner()).insert(key, p.clone());
                    p
                }
            };
            blit(panel, &pixmap, tile.x, tile.y);
        }
        Ok(())
    }

    /// Draw an SVG at its own size, or scaled to fit `size` and centered on it (transparent
    /// around); for previews and animation frames.
    pub fn rgba(&self, svg: &str, size: Option<(u32, u32)>) -> Result<image::RgbaImage, String> {
        let tree = usvg::Tree::from_str(svg, &self.options).map_err(|e| e.to_string())?;
        let own = tree.size();
        let (w, h) = size.unwrap_or((own.width().ceil() as u32, own.height().ceil() as u32));
        let mut pixmap = tiny_skia::Pixmap::new(w.max(1), h.max(1)).ok_or("empty image")?;
        let scale = (w as f32 / own.width()).min(h as f32 / own.height());
        let (dx, dy) = ((w as f32 - own.width() * scale) / 2.0, (h as f32 - own.height() * scale) / 2.0);
        resvg::render(&tree, tiny_skia::Transform::from_scale(scale, scale).post_translate(dx, dy), &mut pixmap.as_mut());
        Ok(image::RgbaImage::from_fn(pixmap.width(), pixmap.height(), |x, y| {
            let p = pixmap.pixel(x, y).expect("in bounds").demultiply();
            image::Rgba([p.red(), p.green(), p.blue(), p.alpha()])
        }))
    }

    /// An SVG as a PNG at its own size.
    pub fn png(&self, svg: &str) -> Result<Vec<u8>, String> {
        let mut out = Cursor::new(Vec::new());
        self.rgba(svg, None)?.write_to(&mut out, image::ImageFormat::Png).map_err(|e| e.to_string())?;
        Ok(out.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::super::house;

    /// Everything HubDeck draws on the deck, as SVG files, for a look at all of it at once:
    /// Every weather sky on the dial in one picture: `HUBDECK_OUT=<png> cargo test -p hubdeck weather_skies -- --ignored`.
    #[test]
    #[ignore]
    fn weather_skies() {
        let mut out = String::from(r##"<svg xmlns="http://www.w3.org/2000/svg" width="1040" height="540"><rect width="100%" height="100%" fill="#101016"/>"##);
        let skies = [(0, true), (0, false), (2, true), (3, true), (61, true), (71, true), (95, true), (45, true)];
        for (i, (code, day)) in skies.iter().enumerate() {
            let fc = super::super::live::Forecast {
                place: "Kraków".into(),
                code: *code,
                day: *day,
                hours: vec![("15:00".into(), 13.0, *code)],
                days: vec![("2026-10-14".into(), 9.0, 15.0, *code), ("2026-10-15".into(), 7.0, 12.0, 3), ("2026-10-16".into(), 5.0, 11.0, 71)],
                ..Default::default()
            };
            let view = WeatherView { hour: 0, mode: if i == 7 { 3 } else { 0 } };
            let svg = weather_dial_svg(&Look::default(), Some(&fc), Some(12.4), view, 0.6, true, 0);
            let (x, y) = (20.0 + (i % 4) as f32 * 255.0, 20.0 + (i / 4) as f32 * 260.0);
            out += &format!(
                r#"<svg x="{x}" y="{y}" width="240" height="240">{}</svg>"#,
                svg.replace(r#"<svg xmlns="http://www.w3.org/2000/svg" width="240" height="240" viewBox="0 0 240 240">"#, "").trim_end_matches("</svg>")
            );
        }
        out += "</svg>";
        let mut opt = usvg::Options::default();
        opt.fontdb_mut().load_system_fonts();
        let tree = usvg::Tree::from_str(&out, &opt).unwrap();
        let size = tree.size().to_int_size();
        let mut pixmap = tiny_skia::Pixmap::new(size.width(), size.height()).unwrap();
        resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());
        pixmap.save_png(std::env::var("HUBDECK_OUT").expect("HUBDECK_OUT")).unwrap();
    }

    /// App icons over the whole key, as they were (with the icon's own margin) and trimmed:
    /// `HUBDECK_ICONS=<data dir> HUBDECK_OUT=<png> cargo test -p hubdeck app_icons_before_after -- --ignored`
    /// with the images named in `HUBDECK_IMAGES` (comma-separated "file:label").
    #[test]
    #[ignore]
    fn app_icons_before_after() {
        let data = std::path::PathBuf::from(std::env::var("HUBDECK_ICONS").expect("HUBDECK_ICONS"));
        super::super::images::init(&data);
        set_app_icons("fillLabel");
        let keys: Vec<(String, String)> = std::env::var("HUBDECK_IMAGES").expect("HUBDECK_IMAGES").split(',').filter_map(|p| p.split_once(':')).map(|(a, b)| (a.into(), b.into())).collect();
        let s = KEY_PX as f32;
        let (gap, pad) = (16.0, 12.0);
        let w = pad * 2.0 + keys.len() as f32 * (s + gap) - gap;
        let mut out = format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{}"><rect width="100%" height="100%" fill="#16141c"/>"##, pad * 3.0 + s * 2.0);
        for (i, (image, label)) in keys.iter().enumerate() {
            let x = pad + i as f32 * (s + gap);
            let before = picture(&super::super::images::uri(image).unwrap(), s, label, DEFAULT_FG, 11.5, false);
            let look = Look { image: Some(image.clone()), label: Some(label.clone()), ..Look::default() };
            out += &format!(r#"<svg x="{x}" y="{pad}" width="{s}" height="{s}">{before}</svg><svg x="{x}" y="{}" width="{s}" height="{s}">{}</svg>"#, pad * 2.0 + s, key_body(&look, 0));
        }
        out += "</svg>";
        let tree = usvg::Tree::from_str(&out, &usvg::Options::default()).unwrap();
        let size = tree.size().to_int_size();
        let mut pixmap = tiny_skia::Pixmap::new(size.width() * 3, size.height() * 3).unwrap();
        resvg::render(&tree, tiny_skia::Transform::from_scale(3.0, 3.0), &mut pixmap.as_mut());
        pixmap.save_png(std::env::var("HUBDECK_OUT").expect("HUBDECK_OUT")).unwrap();
    }

    /// `HUBDECK_SHEET=<dir> cargo test -p hubdeck dump_render_sheet -- --ignored`.
    #[test]
    #[ignore]
    fn dump_render_sheet() {
        let dir = std::path::PathBuf::from(std::env::var("HUBDECK_SHEET").expect("HUBDECK_SHEET"));
        std::fs::create_dir_all(&dir).unwrap();
        let house_look = |action: &str, label: &str, icon: &str| {
            let mut l = Look { label: Some(label.into()), icon: Some(icon.into()), ..Look::default() };
            house::apply_look(&mut l, action, &serde_json::Value::Null);
            l
        };
        let mut n = 0;
        let mut put = |name: &str, svg: String| {
            n += 1;
            std::fs::write(dir.join(format!("{n:02}-{name}.svg")), svg).unwrap();
        };
        // the dial
        let vol = house_look("core.volume", "System", "volume-2");
        put("dial-volume-turned", dial_svg(&vol, Some((Some(0.72), "System")), 0));
        put("dial-volume-muted", dial_svg(&vol, Some((Some(0.0), "Music: muted")), 0));
        put("dial-label-only", dial_svg(&house_look("core.volume", "Volume", "volume-2"), None, 0));
        let clock = house_look("core.clock", "", "clock");
        put("dial-clock", time_dial_svg(&clock, "23:58", Some("Tue 13 Oct"), None, 0));
        put("dial-timer", time_dial_svg(&house_look("core.timer", "Focus", "timer"), "18:42", Some("Focus 25"), Some(0.74), 0));
        put("dial-cpu", dial_svg(&house_look("core.monitor", "CPU", "cpu"), Some((Some(0.37), "CPU")), 0));
        put("dial-gpu-temp", dial_svg(&house_look("core.monitor", "GPU", "thermometer"), Some((Some(0.61), "GPU temp")), 0));
        let np = house_look("core.nowPlaying", "Now playing", "music");
        put("dial-now-playing-no-cover", now_playing_dial_svg(&np, Some(("Na błysk", "Dawid Podsiadło", true, None)), Some(0.42), 0));
        put("dial-now-playing-paused", now_playing_dial_svg(&np, Some(("Tear", "Red Hot Chili Peppers", false, None)), Some(0.1), 0));
        put("dial-now-playing-nothing", now_playing_dial_svg(&np, None, None, 0));
        put("dial-status", time_dial_svg(&Look { bg: Some(house::BG.into()), fg: Some(house::LABEL.into()), ..Look::default() }, "2/3", Some("Discord · v2"), None, 0));
        put("dial-question", question_dial_svg("Keep this deck?", 0));
        put("dial-tour-highlight", with_ring(&dial_svg(&vol, None, 0), "#ffffff"));
        put("dial-gallery-caption", with_caption(&time_dial_svg(&clock, "23:58", Some("Tue 13 Oct"), None, 0), "Clock", 0));
        // keys
        for (name, action, label, icon) in [
            ("key-app", "core.launch", "Spotify", "app-window"),
            ("key-hotkey", "core.hotkey", "Copy", "copy"),
            ("key-mic", "core.mic", "Mic", "mic"),
            ("key-media", "core.media", "Next", "skip-forward"),
            ("key-long-label", "core.launch", "Visual Studio Code", "app-window"),
            ("key-page", "core.page", "Games", "folder"),
            ("key-obs", "obs.record", "Record", "circle"),
        ] {
            put(name, key_svg(&house_look(action, label, icon), 0));
        }
        put("key-cpu-value", key_svg(&Look { label: Some("37%\nCPU".into()), icon: None, face: true, ..house_look("core.monitor", "", "cpu") }, 0));
        put("key-clock-face", key_svg(&Look { label: Some("23:58\nTue 13".into()), icon: None, face: true, ..house_look("core.clock", "", "clock") }, 0));
        put("key-muted-on", key_svg(&Look { bg: Some("#7a1f24".into()), ..house_look("core.mic", "Mic", "mic-off") }, 0));
        put("key-prompt-option", key_svg(&Look { label: Some("Yes, keep".into()), bg: Some("#2b2540".into()), fg: Some("#ffffff".into()), ..Look::default() }, 0));
        put("key-tour-highlight", key_svg(&Look { bg: Some("#ffffff".into()), fg: Some("#111018".into()), ..house_look("core.mic", "Mic", "mic") }, 0));
        put("key-empty", key_svg(&Look { bg: Some("#000000".into()), ..Look::default() }, 0));
        put("key-no-icon", key_svg(&house_look("core.text", "Hello", ""), 0));
        println!("wrote {n} drawings to {}", dir.display());
    }

    use super::*;

    #[test]
    fn renders_keys_and_dial_to_jpeg() {
        let r = Renderer::default();
        let look = Look { label: Some("Mic & <cam>".into()), icon: Some("mic".into()), bg: Some("#203040".into()), fg: None, ..Look::default() };
        let svg = key_svg(&look, 90);
        assert!(svg.contains("&amp; &lt;cam&gt;"), "label is escaped");
        let jpeg = r.jpeg(&svg).unwrap();
        assert_eq!(&jpeg[..2], &[0xFF, 0xD8]);
        let img = image::load_from_memory(&jpeg).unwrap();
        assert_eq!((img.width(), img.height()), (72, 72));
        let dial = r.jpeg(&dial_svg(&Look::default(), Some((Some(0.42), "Volume")), 0)).unwrap();
        assert_eq!(image::load_from_memory(&dial).unwrap().width(), 240);
    }

    /// Japanese on a key: HubDeck's font has no such letters, a system font draws them
    /// (Discussions #3, 2026-10-18). Windows has Yu Gothic and Meiryo, macOS Hiragino.
    #[test]
    fn letters_our_font_lacks_come_from_a_system_font() {
        let r = Renderer::default();
        let lit = |label: &str| {
            let look = Look { label: Some(label.into()), bg: Some("#000000".into()), fg: Some("#ffffff".into()), ..Look::default() };
            let img = image::load_from_memory(&r.jpeg(&key_svg(&look, 0)).unwrap()).unwrap().to_luma8();
            img.pixels().filter(|p| p[0] > 128).count()
        };
        let (none, latin, japanese) = (lit(" "), lit("Mute"), lit("日本語"));
        assert!(latin > none + 20, "{latin} vs {none}");
        assert!(japanese > none + 20, "Japanese drawn: {japanese} lit pixels, {none} with no label");
    }

    /// Before and after for Discussions #3: the same Japanese keys drawn with HubDeck's font only
    /// (0.1.3) and with the computer's fonts, side by side, into `HUBDECK_OUT` (a PNG).
    #[test]
    #[ignore]
    fn japanese_before_after() {
        let keys: String = ["ミュート", "配信開始", "日本語", "Mute"]
            .iter()
            .enumerate()
            .map(|(i, label)| {
                let look = Look { label: Some((*label).into()), icon: Some(["mic-off", "radio", "globe", "volume-x"][i].into()), ..Look::default() };
                let svg = key_svg(&look, 0);
                let body = svg[svg.find('>').unwrap() + 1..svg.rfind("</svg>").unwrap()].to_string();
                format!(r#"<svg x="{}" y="0" width="72" height="72" viewBox="0 0 72 72">{body}</svg>"#, i * 80)
            })
            .collect();
        let sheet = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="312" height="72">{keys}</svg>"#);
        let draw = |opt: &usvg::Options| {
            let tree = usvg::Tree::from_str(&sheet, opt).unwrap();
            let mut pixmap = tiny_skia::Pixmap::new(312, 72).unwrap();
            resvg::render(&tree, tiny_skia::Transform::from_scale(1.0, 1.0), &mut pixmap.as_mut());
            pixmap
        };
        let mut before = usvg::Options::default();
        before.fontdb_mut().load_font_data(FONT.to_vec());
        let after = Renderer::default().options;
        let (b, a) = (draw(&before), draw(&after));
        let mut out = tiny_skia::Pixmap::new(312, 160).unwrap();
        out.fill(tiny_skia::Color::from_rgba8(20, 20, 26, 255));
        out.draw_pixmap(0, 0, b.as_ref(), &tiny_skia::PixmapPaint::default(), tiny_skia::Transform::identity(), None);
        out.draw_pixmap(0, 88, a.as_ref(), &tiny_skia::PixmapPaint::default(), tiny_skia::Transform::identity(), None);
        out.save_png(std::env::var("HUBDECK_OUT").expect("HUBDECK_OUT")).unwrap();
    }

    #[test]
    fn a_chosen_font_draws_the_text() {
        let fonts = deck_fonts();
        assert!(fonts.iter().any(|f| f == FONT_FAMILY), "HubDeck's own font is on the list");
        let Some(other) = fonts.iter().find(|f| ["Arial", "Segoe UI", "Helvetica"].contains(&f.as_str())).cloned() else { return };
        let look = Look { label: Some("Mute".into()), ..Look::default() };
        set_deck_font(Some(&other));
        let svg = key_svg(&look, 0);
        set_deck_font(Some("No Such Font Anywhere"));
        let unknown = key_svg(&look, 0);
        set_deck_font(None);
        assert!(svg.contains(&format!(r#"font-family="{other}""#)), "the chosen font in the image");
        assert!(unknown.contains(r#"font-family="IBM Plex Sans""#), "a font this computer lacks: our own");
        assert!(key_svg(&look, 0).contains(r#"font-family="IBM Plex Sans""#));
    }

    #[test]
    fn game_icons_are_filled_in_the_text_color() {
        assert!(GAME_ICONS.len() > 250);
        for (name, svg) in GAME_ICONS {
            assert!(svg.contains(r#"viewBox="0 0 512 512""#) && !svg.contains("M0 0h512v512H0z"), "{name}");
        }
        let look = Look { label: Some("Sword".into()), icon: Some("game:crossed-swords".into()), fg: Some("#ff0000".into()), ..Look::default() };
        let svg = key_svg(&look, 0);
        assert!(svg.contains(r##"fill="#ff0000""##) && svg.contains("scale(0.058"));
        let img = image::load_from_memory(&Renderer::default().jpeg(&svg).unwrap()).unwrap().to_rgb8();
        assert!(img.pixels().filter(|p| p[0] > 180 && p[1] < 80).count() > 60, "red drawing");
        assert!(icon_svg("game:no-such-icon").is_none() && icon_svg("crossed-swords").is_none());
    }

    #[test]
    fn key_styles_render() {
        let r = Renderer::default();
        let pixel = |svg: &str, x: u32, y: u32| {
            let img = image::load_from_memory(&r.jpeg(svg).unwrap()).unwrap().to_rgb8();
            *img.get_pixel(x, y)
        };
        for style in [None, Some("flat"), Some("gradient"), Some("glass"), Some("outline"), Some("bogus")] {
            let look = Look { label: Some("Go".into()), icon: Some("play".into()), bg: Some("#3060c0".into()), style: style.map(str::to_string), ..Look::default() };
            assert!(r.jpeg(&key_svg(&look, 0)).is_ok(), "{style:?}");
            assert!(r.jpeg(&panel_svg(&[look.clone(), look], 0, &PanelGeometry::default())).is_ok());
        }
        let look = |style: &str| Look { bg: Some("#3060c0".into()), style: Some(style.into()), ..Look::default() };
        // gradient: lighter at the top than at the bottom; outline: black inside a colored frame
        let g = key_svg(&look("gradient"), 0);
        assert!(pixel(&g, 36, 3)[2] > pixel(&g, 36, 68)[2] + 40);
        let o = key_svg(&look("outline"), 0);
        assert!(pixel(&o, 36, 36)[2] < 40 && pixel(&o, 36, 3)[2] > 120);
        assert_eq!(mix("#000000", "#ffffff", 0.5), "#808080");
        assert_eq!(mix("#abc", "#abc", 0.3), "#aabbcc");
    }

    #[test]
    fn icon_editor_looks_render() {
        let r = Renderer::default();
        let px = |svg: &str, x: u32, y: u32| *image::load_from_memory(&r.jpeg(svg).unwrap()).unwrap().to_rgb8().get_pixel(x, y);
        let base = Look { bg: Some("#000000".into()), style: Some("gradient".into()), bg2: Some("#ff0000".into()), ..Look::default() };
        // 180 (default): black at the top, red at the bottom; 90: black at the left, red at the right
        let down = key_svg(&base, 0);
        assert!(px(&down, 36, 2)[0] < 40 && px(&down, 36, 69)[0] > 200);
        let right = key_svg(&Look { angle: Some(90), ..base.clone() }, 0);
        assert!(px(&right, 2, 36)[0] < 40 && px(&right, 69, 36)[0] > 200);
        // matte darkens the corners, not the middle
        let flat = Look { bg: Some("#808080".into()), ..Look::default() };
        let matte = key_svg(&Look { gloss: Some("matte".into()), ..flat.clone() }, 0);
        assert!(px(&matte, 2, 70)[0] + 30 < px(&matte, 36, 30)[0]);
        // a ring at the edge, a badge at the top right in its color
        let ringed = key_svg(&Look { ring: Some("#00ff00".into()), ..flat.clone() }, 0);
        assert!(px(&ringed, 36, 2)[1] > 200 && px(&ringed, 36, 36)[1] < 150);
        let badged = key_svg(&Look { badge: Some("3".into()), badge_color: Some("#0000ff".into()), ..flat.clone() }, 0);
        assert!(px(&badged, 60, 6)[2] > 200 && px(&badged, 10, 6)[2] < 150);
        // effects: a shadow and an outline put dark pixels around light text
        for effect in ["shadow", "outline", "plain"] {
            let l = Look { label: Some("Go".into()), label_effect: Some(effect.into()), fg: Some("#ffffff".into()), ..flat.clone() };
            assert!(key_svg(&l, 0).contains("Go"), "{effect}");
        }
        let shadow = key_svg(&Look { label: Some("Go".into()), label_effect: Some("shadow".into()), ..flat.clone() }, 0);
        assert_eq!(shadow.matches(">Go<").count(), 2);
        // a pattern draws in the label color over the background; an unknown one draws nothing
        for pattern in PATTERNS {
            let with = key_svg(&Look { pattern: Some(pattern.to_string()), pattern_opacity: Some(60), fg: Some("#ffffff".into()), bg: Some("#000000".into()), ..Look::default() }, 0);
            // it fades behind the icon: counted in the outer 12 px, where it is strongest
            let img = image::load_from_memory(&r.jpeg(&with).unwrap()).unwrap().to_luma8();
            let outer = |x: u32, y: u32| x < 12 || y < 12 || x >= 60 || y >= 60;
            let lit = img.enumerate_pixels().filter(|(x, y, p)| outer(*x, *y) && p[0] > 45).count();
            assert!(lit > 150, "{pattern}: {lit} light pixels at the edges");
            let middle = img.enumerate_pixels().filter(|(x, y, p)| (28..44).contains(x) && (26..40).contains(y) && p[0] > 45).count();
            assert!(middle < lit / 4, "{pattern}: {middle} light pixels behind the icon");
        }
        assert_eq!(key_svg(&Look { pattern: Some("<x".into()), ..flat.clone() }, 0), key_svg(&flat, 0));
        // bad colors fall back, never reach the SVG
        let bad = key_svg(&Look { bg2: Some("red\"/><x".into()), ring: Some("url(#x)".into()), badge_color: Some("<".into()), badge: Some("<b>".into()), style: Some("gradient".into()), ..flat }, 0);
        assert!(!bad.contains("<x") && !bad.contains("url(#x)") && !bad.contains("<b>"));
    }

    #[test]
    fn key_motion_repeats_its_cycle_and_rests_between_shines() {
        let r = Renderer::default();
        let base = Look { bg: Some("#404040".into()), label: Some("Go".into()), ..Look::default() };
        let at = |motion: &str, t: f32, pattern: Option<&str>| key_svg(&Look { motion: Some(motion.into()), pattern: pattern.map(str::to_string), t, ..base.clone() }, 0);
        // whole cycles draw the very same picture (the tile cache keeps it): pulse 2.5 s, shine 3 s, slide 2 s
        assert_eq!(at("pulse", 1.0, None), at("pulse", 3.5, None));
        assert_eq!(at("shine", 0.25, None), at("shine", 3.25, None));
        assert_eq!(at("slide", 0.5, Some("stripes")), at("slide", 2.5, Some("stripes")));
        // the pulse at its top is lighter than at its start; the slide moves the stripes
        let px = |svg: &str| image::load_from_memory(&r.jpeg(svg).unwrap()).unwrap().to_rgb8().get_pixel(36, 20)[0];
        assert!(px(&at("pulse", 1.25, None)) > px(&at("pulse", 0.0625, None)) + 15);
        assert_ne!(at("slide", 0.5, Some("stripes")), at("slide", 1.0, Some("stripes")));
        // the shine crosses in the first 1.2 s of 3 and the key rests after it (the same picture
        // as a still key: nothing is drawn or sent then)
        assert!(at("shine", 0.5, None).contains("kshine"));
        let still = key_svg(&base, 0);
        assert_eq!(at("shine", 2.0, None).replace("<svg", "").len(), still.replace("<svg", "").len());
        // a slide without a pattern and a motion at rest (t = 0, the window's preview) draw nothing more
        assert_eq!(at("slide", 1.0, None), still);
        assert_eq!(key_svg(&Look { motion: Some("pulse".into()), ..base.clone() }, 0), still);
    }

    /// Every frame of one cycle of each built-in motion at 3x, as PNGs for an animation to show:
    /// `HUBDECK_SHEET=<dir> cargo test -p hubdeck dump_motion_frames -- --ignored`.
    #[test]
    #[ignore]
    fn dump_motion_frames() {
        let dir = std::path::PathBuf::from(std::env::var("HUBDECK_SHEET").expect("HUBDECK_SHEET")).join("frames");
        std::fs::create_dir_all(&dir).unwrap();
        let s = |v: &str| Some(v.to_string());
        let heart = Look { label: s("Lubię to"), icon: s("heart"), style: s("gradient"), bg: s("#6a3fd8"), bg2: s("#e0457b"), angle: Some(135), fg: s("#ffffff"), ..Look::default() };
        let r = Renderer::default();
        let night = Look { label: s("Lubię to"), icon: s("heart"), bg: s(house::BG), fg: s(house::LABEL), icon_color: s("#ec798c"), ..Look::default() };
        for (name, look, steps) in [
            ("pulse", Look { motion: s("pulse"), ..heart.clone() }, 40),
            ("shine", Look { motion: s("shine"), gloss: s("glass"), ..heart.clone() }, 48),
            ("slide", Look { motion: s("slide"), pattern: s("stripes"), pattern_opacity: Some(30), ..heart.clone() }, 32),
            ("glow", Look { motion: s("glow"), ..night.clone() }, 48),
            ("sweep", Look { motion: s("sweep"), ..night.clone() }, 48),
            ("sparkle", Look { motion: s("sparkle"), ..heart.clone() }, 32),
            ("flow", Look { motion: s("flow"), ..heart.clone() }, 96),
            ("pulse-night", Look { motion: s("pulse"), ..night.clone() }, 40),
            ("waves", Look { motion: s("slide"), pattern: s("waves"), pattern_opacity: Some(35), ..heart.clone() }, 72),
            ("hex", Look { motion: s("slide"), pattern: s("hex"), pattern_opacity: Some(35), ..night.clone() }, 60),
        ] {
            for step in 0..steps {
                let svg =
                    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="216" height="216" viewBox="0 0 72 72">{}</svg>"#, key_body(&Look { t: step as f32 / 16.0 + 0.001, ..look.clone() }, 0));
                std::fs::write(dir.join(format!("{name}-{step:03}.png")), r.png(&svg).unwrap()).unwrap();
            }
        }
    }

    /// What each built-in motion costs on a page of 15 moving keys: a cycle of frames drawn and
    /// encoded by a fresh renderer (no tiles kept), against still keys.
    /// `cargo test --release -p hubdeck motion_cost -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn motion_cost() {
        let s = |v: &str| Some(v.to_string());
        let kinds: [(&str, Look); 5] = [
            ("still", Look::default()),
            ("pulse", Look { motion: s("pulse"), style: s("gradient"), bg: s("#6a3fd8"), bg2: s("#e0457b"), ..Look::default() }),
            ("shine", Look { motion: s("shine"), gloss: s("glass"), bg: s("#1e3a8a"), ..Look::default() }),
            ("slide stripes", Look { motion: s("slide"), pattern: s("stripes"), bg: s("#c2410c"), ..Look::default() }),
            ("slide noise", Look { motion: s("slide"), pattern: s("noise"), pattern_opacity: Some(30), bg: s("#c2410c"), ..Look::default() }),
        ];
        for (name, base) in kinds {
            let r = Renderer::default();
            // two cycles of the slowest repeat (pulse 40 steps): the first with an empty tile cache, the second warm
            let (mut ms, mut bytes, n) = (0.0, 0usize, 48);
            let mut cold = 0.0;
            for step in 0..2 * n {
                if step == n {
                    cold = ms;
                    ms = 0.0;
                    bytes = 0;
                }
                let looks: Vec<Look> = (0..15)
                    .map(|i| Look { label: Some(format!("Key {i}")), icon: s("heart"), fg: s("#ffffff"), t: if base.motion.is_some() { step as f32 / 16.0 + 0.001 } else { 0.0 }, ..base.clone() })
                    .collect();
                let start = std::time::Instant::now();
                let jpeg = r.panel_jpeg(&panel_tiles(&looks, 0, &PanelGeometry::default()), Some(80)).unwrap();
                ms += start.elapsed().as_secs_f64() * 1000.0;
                bytes += jpeg.len();
            }
            println!("{name:14} cold {:6.2} ms, warm {:6.2} ms a frame, {:6.1} KB", cold / n as f64, ms / n as f64, bytes as f64 / n as f64 / 1024.0);
        }
    }

    #[test]
    fn a_key_can_be_moved_by_hand_and_have_its_own_font() {
        let base = Look { label: Some("Go".into()), icon: Some("play".into()), ..Look::default() };
        let plain = key_svg(&base, 0);
        let moved = key_svg(&Look { icon_dx: 6, icon_dy: -4, label_dx: -3, label_dy: 2, ..base.clone() }, 0);
        assert_ne!(plain, moved);
        assert!(moved.contains(r#"translate(-3 2)"#));
        // a font the computer has is used for this key only; one it lacks is the deck's
        let font = deck_fonts().into_iter().find(|f| f != FONT_FAMILY).expect("a system font");
        let own = key_svg(&Look { font: Some(font.clone()), ..base.clone() }, 0);
        assert!(own.contains(&format!(r#"font-family="{}""#, escape(&font))));
        assert!(!key_svg(&base, 0).contains(&format!(r#"font-family="{}""#, escape(&font))));
        let missing = key_svg(&Look { font: Some("No Such Font 123".into()), ..base }, 0);
        assert!(!missing.contains("No Such Font"));
    }

    /// Where a placed label really stands: drawn alone (white on black, no icon) in every position,
    /// size, line count and with or without a frame, its lit rows measured. Prints the margins;
    /// the test with the limits is `placed_labels_keep_clear_of_the_edges`.
    /// `cargo test -p hubdeck label_margins -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn label_margins() {
        for (name, look) in label_cases() {
            let (top, bottom) = lit_rows(&look);
            println!("{name:40} top {top:>3}  bottom {bottom:>3}");
        }
    }

    /// A label at the top or the bottom keeps the same clear edge from the key's edge (7 px, 9 with
    /// a frame): owner, 2026-10-25, Bottom sat on the frame (1-6 px) and Top was too high (4 px).
    #[test]
    fn placed_labels_keep_clear_of_the_edges() {
        for (name, look) in label_cases() {
            let edge = if look.ring.is_some() { 9 } else { 7 };
            let (top, bottom) = lit_rows(&look);
            match look.label_pos.as_deref() {
                Some("top") => assert!((edge - 1..=edge + 1).contains(&top), "{name}: {top} px from the top"),
                // "Play" has a tail (y): its lowest pixel is the edge; "to bardzo" has none and sits higher
                Some("bottom") if look.label.as_deref() == Some("Play") => assert!((edge - 1..=edge + 2).contains(&bottom), "{name}: {bottom} px from the bottom"),
                Some("bottom") => assert!(bottom >= edge - 1, "{name}: {bottom} px from the bottom"),
                // the middle: as far from the top as from the bottom, give or take a tail
                _ => assert!(top.abs_diff(bottom) <= 8, "{name}: {top} / {bottom}"),
            }
        }
    }

    /// Only the keys whose square changed go out alone; a change in a window outside its square
    /// sends the whole panel.
    #[test]
    fn changed_keys_are_found_by_their_squares() {
        let g = PanelGeometry::default();
        let old = vec![0u8; (PANEL_W * PANEL_H * 4) as usize];
        assert_eq!(changed_keys(&old, &old, &g), Some(vec![]));
        let mut new = old.clone();
        let (sx, sy) = key_slot(8);
        new[((sy + 30) * PANEL_W as usize + sx + 30) * 4] = 200;
        assert_eq!(changed_keys(&old, &new, &g), Some(vec![8]));
        // key 0's window starts at x 0, its square at x 5: a change at x 1 is outside the square
        let mut edge = old.clone();
        edge[(30 * PANEL_W as usize + 1) * 4] = 200;
        assert_eq!(changed_keys(&old, &edge, &g), None);
        // a few levels there count as none
        edge[(30 * PANEL_W as usize + 1) * 4] = 5;
        assert_eq!(changed_keys(&old, &edge, &g), Some(vec![]));
    }

    /// Every skin keeps the icon and the label whole on the key, and neon keeps them inside its
    /// tube (the tube's inner edge is at 3.75 px): measured on the drawn content in every label
    /// case and on keys with an icon, an emoji and a long label.
    #[test]
    fn skins_keep_the_content_on_the_key() {
        let s = |v: &str| Some(v.to_string());
        let mut cases = label_cases();
        for (label, icon) in
            [("Lubię to", Some("heart")), ("Song sorter", Some("music")), ("Lubię to bardzo", Some("heart")), ("Focus 25", None), ("Mikrofon", Some("mic-off")), ("Play", Some("emoji:🎨"))]
        {
            cases.push((format!("{label} {icon:?}"), Look { label: s(label), icon: icon.map(str::to_string), bg: s("#1b1a20"), fg: s("#ffffff"), ..Look::default() }));
        }
        for skin in SKINS {
            for (name, look) in &cases {
                let look = Look { skin: Some(skin.to_string()), ..look.clone() };
                // a glow (neon, pumpkin) may reach the edge; the letters and the icon may not
                let body = if matches!(*skin, "neon" | "pumpkin") { key_parts_plain(&skinned(&look, skin, "#ffffff")).1 } else { key_parts(&look).1 };
                let svg = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="72" height="72" viewBox="0 0 72 72">{body}</svg>"#);
                let img = Renderer::default().rgba(&svg, None).unwrap();
                let solid = |x: u32, y: u32| img.get_pixel(x, y)[3] > 160;
                let rows: Vec<u32> = (0..72).filter(|y| (0..72).any(|x| solid(x, *y))).collect();
                let cols: Vec<u32> = (0..72).filter(|x| (0..72).any(|y| solid(*x, y))).collect();
                let (Some(top), Some(bottom), Some(left), Some(right)) = (rows.first(), rows.last(), cols.first(), cols.last()) else { panic!("{skin} {name}: nothing drawn") };
                let margin = [*top, 71 - bottom, *left, 71 - right].into_iter().min().unwrap();
                let need = if *skin == "neon" { 5 } else { 2 };
                assert!(margin >= need, "{skin} {name}: the content is {margin} px from the edge (needs {need})");
            }
        }
    }

    /// What a skin costs to draw, a still key and a moving one's top layer, ms (release build):
    /// `cargo test --release -p hubdeck skin_draw_cost -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn skin_draw_cost() {
        let s = |v: &str| Some(v.to_string());
        let key = Look { label: s("Lubię to"), icon: s("heart"), bg: s("#1b1a20"), fg: s("#e6e4ee"), icon_color: s("#79ecc6"), ..Look::default() };
        let r = Renderer::default();
        let ms = |svg: &str, n: u32| {
            let start = std::time::Instant::now();
            for _ in 0..n {
                r.rgba(svg, None).unwrap();
            }
            start.elapsed().as_secs_f64() * 1000.0 / n as f64
        };
        println!("plain key: {:.2} ms", ms(&key_svg(&key, 0), 50));
        for (skin, motion) in [("film", "film"), ("glitch", "glitch"), ("neon", "flicker"), ("neon", "sparks"), ("glass", "shine")] {
            let look = Look { skin: s(skin), motion: s(motion), t: 0.0625, ..key.clone() };
            let [_, _, over] = key_layers(&look, 0);
            let over = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="72" height="72" viewBox="0 0 72 72">{over}</svg>"#);
            println!("{skin} {motion}: whole key {:.2} ms, moving layer {:.2} ms", ms(&key_svg(&look, 0), 50), ms(&over, 50));
        }
    }

    /// The label cases: top, middle, bottom x auto and every size x one and two lines x frame.
    fn label_cases() -> Vec<(String, Look)> {
        let mut out = Vec::new();
        for pos in ["top", "middle", "bottom"] {
            for size in [None, Some(8u8), Some(12), Some(16), Some(20), Some(24)] {
                for (lines, label) in [(1, "Play"), (2, "Lubię to bardzo")] {
                    for ring in [false, true] {
                        let look = Look {
                            label: Some(label.into()),
                            label_pos: Some(pos.into()),
                            label_size: size,
                            bg: Some("#000000".into()),
                            fg: Some("#ffffff".into()),
                            ring: ring.then(|| "#000001".into()),
                            ..Look::default()
                        };
                        out.push((format!("{pos} size {} {lines} line(s){}", size.map_or("auto".into(), |z| z.to_string()), if ring { " frame" } else { "" }), look));
                    }
                }
            }
        }
        out
    }

    /// The first and the last row of the key with light text in it, from the top and from the bottom
    /// (key pixels; the key is 72).
    fn lit_rows(look: &Look) -> (u32, u32) {
        let img = Renderer::default().rgba(&key_svg(look, 0), None).unwrap();
        let lit = |y: u32| (0..img.width()).any(|x| img.get_pixel(x, y)[0] > 90);
        let first = (0..img.height()).find(|y| lit(*y)).unwrap_or(0);
        let last = (0..img.height()).rev().find(|y| lit(*y)).unwrap_or(0);
        (first, img.height() - 1 - last)
    }

    /// Built-in motion frame by frame at 3x:
    /// `HUBDECK_SHEET=<dir> cargo test -p hubdeck dump_motion_sheet -- --ignored`.
    #[test]
    #[ignore]
    fn dump_motion_sheet() {
        let dir = std::path::PathBuf::from(std::env::var("HUBDECK_SHEET").expect("HUBDECK_SHEET"));
        std::fs::create_dir_all(&dir).unwrap();
        let s = |v: &str| Some(v.to_string());
        let heart = Look { label: s("Lubię to"), icon: s("heart"), style: s("gradient"), bg: s("#6a3fd8"), bg2: s("#e0457b"), angle: Some(135), fg: s("#ffffff"), ..Look::default() };
        let rows = [
            ("pulse", Look { motion: s("pulse"), ..heart.clone() }, [0.0625, 0.625, 1.25, 1.875, 2.4375]),
            ("shine", Look { motion: s("shine"), gloss: s("glass"), ..heart.clone() }, [0.0625, 0.3125, 0.5625, 0.8125, 1.0625]),
            ("slide", Look { motion: s("slide"), pattern: s("stripes"), pattern_opacity: Some(30), ..heart.clone() }, [0.0625, 0.5, 1.0, 1.5, 1.9375]),
        ];
        let cell = 240.0;
        let (w, h) = (cell * 5.0, (cell + 28.0) * rows.len() as f32);
        let mut svg = format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}"><rect width="{w}" height="{h}" fill="#1a1a1f"/>"##);
        for (r, (name, look, times)) in rows.iter().enumerate() {
            for (c, t) in times.iter().enumerate() {
                let (x, y) = (c as f32 * cell + 12.0, r as f32 * (cell + 28.0) + 12.0);
                svg += &format!(r#"<svg x="{x}" y="{y}" width="216" height="216" viewBox="0 0 72 72">{}</svg>"#, key_body(&Look { t: *t, ..look.clone() }, 0));
                svg += &text(x + 108.0, y + 240.0, 16.0, "#cccccc", &format!("{name} {t:.2} s"));
            }
        }
        svg += "</svg>";
        std::fs::write(dir.join("motion.png"), Renderer::default().png(&svg).unwrap()).unwrap();
    }

    /// The skins on six keys (rows: none and each skin) at 3x, and each skin's motion frame by
    /// frame: `HUBDECK_SHEET=<dir> cargo test -p hubdeck dump_skin_sheet -- --ignored`
    /// (`skins.png`, `skins-real.png` at the deck's own 72 px, `skin-motion.png`).
    #[test]
    #[ignore]
    fn dump_skin_sheet() {
        let dir = std::path::PathBuf::from(std::env::var("HUBDECK_SHEET").expect("HUBDECK_SHEET"));
        std::fs::create_dir_all(&dir).unwrap();
        let s = |v: &str| Some(v.to_string());
        let keys = [
            Look { label: s("Lubię to"), icon: s("heart"), style: s("gradient"), bg: s("#6a3fd8"), bg2: s("#e0457b"), angle: Some(135), fg: s("#ffffff"), ..Look::default() },
            Look { label: s("Lubię to"), icon: s("heart"), bg: s("#1b1a20"), fg: s("#e6e4ee"), icon_color: s("#79ecc6"), ..Look::default() },
            Look { label: s("Play"), icon: s("emoji:🎨"), bg: s("#16a36a"), fg: s("#ffffff"), icon_color: s("#79ecc6"), ..Look::default() },
            Look { label: s("Mikrofon"), icon: s("mic-off"), bg: s("#c2410c"), fg: s("#ffffff"), icon_color: s("#ffb38a"), ..Look::default() },
            Look { label: s("Discord"), icon: s("message-square"), bg: s("#1e3a8a"), fg: s("#e6ebff"), icon_color: s("#7aa2ff"), ..Look::default() },
            Look { label: s("Focus 25"), bg: s("#0f766e"), fg: s("#ffffff"), icon_color: s("#5eead4"), ..Look::default() },
        ];
        let rows: Vec<Option<&str>> = std::iter::once(None).chain(SKINS.iter().copied().map(Some)).collect();
        let sheet = |cell: f32, scale: f32, name: &str| {
            let (w, h) = (cell * keys.len() as f32 + 130.0, cell * rows.len() as f32);
            let mut svg = format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}"><rect width="{w}" height="{h}" fill="#141318"/>"##);
            for (r, skin) in rows.iter().enumerate() {
                svg += &text(60.0, r as f32 * cell + cell / 2.0, 16.0, "#cccccc", skin.unwrap_or("now"));
                for (c, key) in keys.iter().enumerate() {
                    let (x, y) = (130.0 + c as f32 * cell, r as f32 * cell + (cell - 72.0 * scale) / 2.0);
                    svg += &format!(r#"<g transform="translate({x} {y}) scale({scale})">{}</g>"#, key_body(&Look { skin: skin.map(str::to_string), ..key.clone() }, 0));
                }
            }
            svg += "</svg>";
            std::fs::write(dir.join(name), Renderer::default().png(&svg).unwrap()).unwrap();
        };
        sheet(232.0, 3.0, "skins.png");
        sheet(84.0, 1.0, "skins-real.png");
        let moving = [("film", "film", 0usize), ("glitch", "glitch", 4), ("neon", "flicker", 1), ("neon", "sparks", 3), ("glass", "shine", 2)];
        let times = [0.0625, 0.125, 0.1875, 0.375, 0.5625, 1.0];
        let cell = 232.0;
        let (w, h) = (cell * times.len() as f32 + 130.0, cell * moving.len() as f32);
        let mut svg = format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}"><rect width="{w}" height="{h}" fill="#141318"/>"##);
        for (r, (skin, motion, k)) in moving.iter().enumerate() {
            svg += &text(60.0, r as f32 * cell + cell / 2.0, 15.0, "#cccccc", &format!("{skin} {motion}"));
            for (c, t) in times.iter().enumerate() {
                let look = Look { skin: s(skin), motion: s(motion), t: *t, ..keys[*k].clone() };
                svg += &format!(r#"<g transform="translate({} {}) scale(3)">{}</g>"#, 130.0 + c as f32 * cell, r as f32 * cell + 8.0, key_body(&look, 0));
            }
        }
        svg += "</svg>";
        std::fs::write(dir.join("skin-motion.png"), Renderer::default().png(&svg).unwrap()).unwrap();
        // every frame of each motion's loop at 2x, for the page that plays them
        for (skin, motion, k) in moving {
            for n in 0..64u32 {
                let look = Look { skin: s(skin), motion: s(motion), t: n as f32 / 16.0 + 0.0001, ..keys[k].clone() };
                let svg = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="144" height="144" viewBox="0 0 72 72">{}</svg>"#, key_body(&look, 0));
                std::fs::write(dir.join(format!("frame-{skin}-{motion}-{n:02}.png")), Renderer::default().png(&svg).unwrap()).unwrap();
            }
        }
        // the owner's own page, every key in each skin: HUBDECK_PROFILE=<profile .json>
        if let Ok(path) = std::env::var("HUBDECK_PROFILE") {
            // its pictures from the same app data (read only)
            if let Some(data) = std::path::Path::new(&path).parent().and_then(|p| p.parent()) {
                super::super::images::init(data);
            }
            let profile: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            let looks: Vec<Look> = (0..15).map(|i| profile["controls"].get(format!("keys.{i}")).and_then(|c| serde_json::from_value(c["look"].clone()).ok()).unwrap_or_default()).collect();
            for skin in rows.iter() {
                let mut svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="808" height="484" viewBox="0 0 808 484"><rect width="808" height="484" fill="#0b0b0e"/>"##.to_string();
                for (i, look) in looks.iter().enumerate() {
                    let (x, y) = (8.0 + (i % 5) as f32 * 160.0, 8.0 + (i / 5) as f32 * 158.0);
                    // an empty key stays as it is, as it would on the deck
                    let empty = look.label.as_deref().is_none_or(|l| l.trim().is_empty()) && look.icon.is_none() && look.image.is_none();
                    svg += &format!(r#"<g transform="translate({x} {y}) scale(2)">{}</g>"#, key_body(&Look { skin: skin.filter(|_| !empty).map(str::to_string), ..look.clone() }, 0));
                }
                svg += "</svg>";
                std::fs::write(dir.join(format!("deck-{}.png", skin.unwrap_or("now"))), Renderer::default().png(&svg).unwrap()).unwrap();
            }
        }
    }

    /// The icon editor's templates and options at 3x, one sheet to look at:
    /// `HUBDECK_SHEET=<dir> cargo test -p hubdeck dump_icon_editor_sheet -- --ignored`.
    #[test]
    #[ignore]
    fn dump_icon_editor_sheet() {
        let dir = std::path::PathBuf::from(std::env::var("HUBDECK_SHEET").expect("HUBDECK_SHEET"));
        std::fs::create_dir_all(&dir).unwrap();
        let heart = |l: Look| Look { label: Some("Lubię to".into()), icon: Some("heart".into()), ..l };
        let s = |v: &str| Some(v.to_string());
        let grad = |a: &str, b: &str, angle: u16| Look { style: s("gradient"), bg: s(a), bg2: s(b), angle: Some(angle), fg: s("#ffffff"), ..Look::default() };
        let looks = [
            ("Neon", heart(Look { gloss: s("glass"), ..grad("#6a3fd8", "#e0457b", 135) })),
            ("Forest", heart(grad("#0f766e", "#22c55e", 160))),
            ("Ember", heart(grad("#f59e0b", "#c2410c", 200))),
            ("Night", heart(Look { gloss: s("glass"), fg: s("#e6ebff"), ..grad("#1e3a8a", "#0b1020", 180) })),
            ("Frame", heart(Look { bg: s(house::BG), fg: s(house::LABEL), ring: s("#f5c542"), ..Look::default() })),
            ("Counter", heart(Look { bg: s(house::BG), fg: s(house::LABEL), label_pos: s("top"), badge: s("3"), ..Look::default() })),
            ("matte", heart(Look { bg: s("#3060c0"), gloss: s("matte"), ..Look::default() })),
            ("shadow", heart(Look { bg: s("#f2d24b"), fg: s("#ffffff"), label_effect: s("shadow"), ..Look::default() })),
            ("outline", heart(Look { bg: s("#f2d24b"), fg: s("#ffffff"), label_effect: s("outline"), ..Look::default() })),
            ("badge NEW", heart(Look { bg: s(house::BG), fg: s(house::LABEL), badge: s("NEW"), badge_color: s("#2f9e44"), ..Look::default() })),
            ("emoji+badge", Look { label: s("Oceń"), icon: s("emoji:🔥"), badge: s("12"), ..grad("#3a1c71", "#d76d77", 135) }),
            ("label only", Look { label: s("Focus 25"), label_effect: s("outline"), ..grad("#0f766e", "#22c55e", 90) }),
            ("stripes", heart(Look { pattern: s("stripes"), ..grad("#6a3fd8", "#e0457b", 135) })),
            ("dots", heart(Look { bg: s(house::BG), fg: s(house::LABEL), pattern: s("dots"), ..Look::default() })),
            ("grid", heart(Look { bg: s("#0f766e"), fg: s("#ffffff"), pattern: s("grid"), ..Look::default() })),
            ("noise", heart(Look { pattern: s("noise"), pattern_opacity: Some(30), ..grad("#1e3a8a", "#0b1020", 180) })),
            ("noise 18%", heart(Look { bg: s("#c2410c"), fg: s("#ffffff"), pattern: s("noise"), ..Look::default() })),
            ("stripes 40%", heart(Look { bg: s(house::BG), fg: s(house::LABEL), pattern: s("stripes"), pattern_opacity: Some(40), ..Look::default() })),
        ];
        let (cell, cols) = (240.0, 6usize);
        let rows = looks.len().div_ceil(cols);
        let (w, h) = (cell * cols as f32, (cell + 28.0) * rows as f32);
        let mut svg = format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}"><rect width="{w}" height="{h}" fill="#1a1a1f"/>"##);
        for (i, (name, look)) in looks.iter().enumerate() {
            let (x, y) = ((i % cols) as f32 * cell + 12.0, (i / cols) as f32 * (cell + 28.0) + 12.0);
            svg += &format!(r#"<g transform="translate({x} {y}) scale(3)">{}</g>"#, key_body(look, 0));
            svg += &text(x + 108.0, y + 240.0, 16.0, "#cccccc", name);
        }
        svg += "</svg>";
        std::fs::write(dir.join("icon-editor.png"), Renderer::default().png(&svg).unwrap()).unwrap();
    }

    #[test]
    fn game_screens_render() {
        use super::super::games::{Arena, Game, Snake};
        let r = Renderer::default();
        let now = std::time::Instant::now();
        let snake = Snake::new(1, now).screen(now);
        let panel = r.jpeg(&panel_body_svg(snake.panel.as_deref().unwrap(), 0)).unwrap();
        assert_eq!(image::load_from_memory(&panel).unwrap().width(), PANEL_W);
        let pong = super::super::games::Pong::new(1, now).screen(now);
        assert!(r.jpeg(&panel_body_svg(pong.panel.as_deref().unwrap(), 180)).is_ok());
        let wheel = super::super::games::Wheel::new(1, now).screen(now);
        assert!(r.jpeg(&dial_body_svg(&wheel.dial, 0)).is_ok());
        let arena = Arena::new(1, now).screen(now);
        let dial = r.jpeg(&dial_body_svg(&arena.dial, 90)).unwrap();
        assert_eq!(image::load_from_memory(&dial).unwrap().width(), DIAL_PX);
    }

    #[test]
    fn panel_clock_and_mode_name_render() {
        let r = Renderer::default();
        for rotation in [0, 90, 180, 270] {
            let jpeg = r.jpeg(&panel_content_svg(&super::super::profile::PanelContent::Clock, rotation, "12:34", &[])).unwrap();
            let graph = super::super::profile::PanelContent::Graph { metric: super::super::monitor::Metric::Cpu };
            assert!(r.jpeg(&panel_content_svg(&graph, rotation, "", &[10.0, 80.0, 35.5])).is_ok());
            assert_eq!(image::load_from_memory(&jpeg).unwrap().width(), PANEL_W);
        }
        // a missing picture draws an empty panel instead of failing
        let missing = super::super::profile::PanelContent::Picture { image: "0000000000000000.png".into() };
        assert!(r.jpeg(&panel_content_svg(&missing, 0, "", &[])).is_ok());
        assert!(dial_svg(&Look::default(), Some((None, "Scroll")), 0).contains("Scroll"));
    }

    #[test]
    fn clock_and_timer_faces_render() {
        let r = Renderer::default();
        let face = Look { label: Some("21:05\nSun 11 Oct".into()), face: true, ..Look::default() };
        let svg = key_svg(&face, 0);
        assert!(svg.contains(">21:05<") && svg.contains(">Sun 11 Oct<"), "two lines");
        r.jpeg(&svg).unwrap();
        for ring in [None, Some(0.0), Some(0.4), Some(1.0)] {
            let svg = time_dial_svg(&Look::default(), "4:59", Some("Tea"), ring, 90);
            assert_eq!(svg.contains("<path"), ring.is_some_and(|v| v > 0.0));
            r.jpeg(&svg).unwrap();
        }
    }

    #[test]
    fn images_stay_small_enough_for_usb() {
        let r = Renderer::default();
        let dial = r.jpeg(&dial_svg(&Look::default(), Some((Some(0.42), "Volume")), 0)).unwrap();
        let key = r.jpeg(&key_svg(&Look { label: Some("Screenshot".into()), icon: Some("camera".into()), ..Look::default() }, 0)).unwrap();
        eprintln!("dial {} B, key {} B", dial.len(), key.len());
        assert!(dial.len() < 20_000, "dial frame {} B", dial.len());
        assert!(key.len() < 6_000, "key image {} B", key.len());
    }

    #[test]
    fn panel_places_keys_on_the_measured_grid() {
        let looks: Vec<Look> = (0..15).map(|i| Look { bg: Some(if i == 14 { "#ffffff".into() } else { "#000000".into() }), ..Look::default() }).collect();
        let photo = PanelGeometry { x0: 0.0, y0: 0.0, dx: 102.0, dy: 100.0, w: 72.0, h: 72.0 };
        let img = image::load_from_memory(&Renderer::default().jpeg(&panel_svg(&looks, 0, &photo)).unwrap()).unwrap().to_rgb8();
        assert_eq!((img.width(), img.height()), (480, 272));
        // key 15 (index 14) is the bottom-right square 408..480 x 200..272
        assert!(img.get_pixel(444, 236)[0] > 200, "inside key 15");
        assert!(img.get_pixel(400, 236)[0] > 200, "background bleeds into the hidden gap");
        assert!(img.get_pixel(385, 236)[0] < 60, "but not into the next key's half");
        assert!(img.get_pixel(444, 180)[0] < 60, "nor past half the vertical gap");
        assert_eq!(key_origin(6), (100.0, 99.0));
        let d = PanelGeometry::default();
        assert!(d.key_rect(14).0 + d.w <= 480.0 && d.key_rect(14).1 + d.h <= 273.0, "default keys fit the panel");
    }

    #[test]
    fn calibration_moves_the_double_border() {
        let mut g = PanelGeometry::default();
        g.adjust(0, 3.0); // left edge
        g.adjust(4, -2.0); // key width
        assert_eq!(g.key_rect(1), (103.0, 0.0, 77.0, 75.0));
        let img = image::load_from_memory(&Renderer::default().jpeg(&calibration_svg(&g)).unwrap()).unwrap().to_rgb8();
        let red = |p: &image::Rgb<u8>| p[0] > 180 && p[1] < 90;
        let green = |p: &image::Rgb<u8>| p[1] > 180 && p[0] < 120;
        assert!(red(img.get_pixel(1, 30)), "red just outside key 1's left edge (x0 = 3)");
        assert!(green(img.get_pixel(4, 30)), "green just inside it");
        assert_eq!(GEOMETRY_PARAMS.len(), 6);
    }

    #[test]
    fn rejects_color_injection_and_unknown_icons() {
        let look = Look { label: None, icon: Some("does-not-exist".into()), bg: Some("red\"/><script".into()), fg: None, ..Look::default() };
        let svg = key_svg(&look, 0);
        assert!(svg.contains(DEFAULT_BG));
        assert!(!svg.contains("script"));
    }

    #[test]
    fn long_labels_fit_their_screen() {
        // measured widths grow with the text and the size
        assert!(text_width("Diablo II Resurrected", 10.0) > text_width("Diablo", 10.0));
        assert!((text_width("Copy", 20.0) - 2.0 * text_width("Copy", 10.0)).abs() < 0.01);
        // under an icon on a key: two lines, each inside the key
        let (rows, px) = fit("Diablo II Resurrected", 11.5, 9.5, 66.0, true);
        assert_eq!(rows.len(), 2, "{rows:?}");
        assert!(rows.iter().all(|r| text_width(r, px) <= 66.0), "{rows:?} at {px}");
        // a short label stays as it is
        assert_eq!(fit("Copy", 11.5, 9.5, 66.0, true), (vec!["Copy".to_string()], 11.5));
        // one word too long for any size: cut with an ellipsis
        let (rows, px) = fit("Supercalifragilisticexpialidocious", 11.5, 9.5, 66.0, true);
        assert!(rows[0].ends_with('\u{2026}') && text_width(&rows[0], px) <= 66.0, "{rows:?}");
        // the dial's round screen is narrower near the bottom
        assert!(dial_chord(200.0) < dial_chord(120.0));
        assert!(dial_svg(&Look { label: Some("Diablo II Resurrected".into()), icon: Some("volume-2".into()), ..Look::default() }, None, 0).contains("Resurrected"));
    }

    #[test]
    fn text_is_actually_drawn() {
        // a white label on black must put light pixels in the middle of the key
        let r = Renderer::default();
        let look = Look { label: Some("88".into()), icon: None, bg: Some("#000000".into()), fg: Some("#ffffff".into()), ..Look::default() };
        let img = image::load_from_memory(&r.jpeg(&key_svg(&look, 0)).unwrap()).unwrap().to_rgb8();
        let bright = img.pixels().filter(|p| p[0] > 200).count();
        assert!(bright > 50, "font loaded and text rendered ({bright} bright pixels)");
    }
}

#[cfg(test)]
mod timing {
    use super::*;

    /// `cargo test -p hubdeck dump_key_styles -- --ignored`: the four key styles as JPEGs in the temp folder
    #[test]
    #[ignore]
    fn dump_key_styles() {
        let r = Renderer::default();
        let mut looks = Vec::new();
        for (i, style) in ["flat", "gradient", "glass", "outline"].into_iter().enumerate() {
            for (j, bg) in ["#7c5cff", "#1d2b3a", "#e5484d"].into_iter().enumerate() {
                let look = Look { label: Some(style.into()), icon: Some(["play", "mic", "camera"][j].into()), bg: Some(bg.into()), style: Some(style.into()), ..Look::default() };
                looks.push(look.clone());
                let _ = i;
            }
        }
        let panel = panel_svg(&looks, 0, &PanelGeometry::default());
        let path = std::env::temp_dir().join("hubdeck-key-styles.jpg");
        std::fs::write(&path, r.jpeg_with(&panel, Some(95)).unwrap()).unwrap();
        eprintln!("{}", path.display());
    }

    /// `cargo test -p hubdeck render_timing -- --ignored --nocapture` (debug vs --release)
    #[test]
    #[ignore]
    fn panel_with_gifs_memory() {
        // the "fun" page's animations drawn many times: does the process keep growing?
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/demo");
        // every frame of every GIF, as the engine has them (their sizes differ from frame to frame)
        let mut frames = Vec::new();
        for name in ["aurora.gif", "bars.gif", "dots.gif", "fire.gif", "heartbeat.gif", "lava.gif", "orbit.gif"] {
            let bytes = std::fs::read(dir.join(name)).unwrap();
            frames.extend(super::super::images::gif_frame_uris(&bytes));
        }
        eprintln!("{} frames", frames.len());
        let rss = || {
            let mut sys = sysinfo::System::new();
            let pid = sysinfo::Pid::from_u32(std::process::id());
            sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
            sys.process(pid).map(|p| p.memory() / 1_048_576).unwrap_or(0)
        };
        let r = Renderer::default();
        let geometry = PanelGeometry::default();
        for round in 0..6 {
            for i in 0..100 {
                let looks: Vec<Look> = (0..15).map(|k| Look { image: Some(frames[(k * 7 + i + round * 100) % frames.len()].clone()), label: Some("Fun".into()), ..Look::default() }).collect();
                r.jpeg(&panel_svg(&looks, 0, &geometry)).unwrap();
            }
            eprintln!("after {} frames: {} MB", (round + 1) * 100, rss());
        }
    }

    #[test]
    fn a_pressed_key_gets_a_frame_and_a_failed_one_a_red_corner() {
        let plain = Look { label: Some("Mic".into()), icon: Some("mic".into()), ..Look::default() };
        let svg = |l: &Look| key_svg(l, 0);
        assert!(!svg(&plain).contains("#f0506e") && !svg(&plain).contains("stroke-width=\"3\""));
        assert!(svg(&Look { pressed: true, ..plain.clone() }).contains("stroke-width=\"3\""));
        assert!(svg(&Look { failed: true, ..plain.clone() }).contains("#f0506e"));
        // the marks turn with the content, so the corner is top right as the user reads the key (each
        // of the three layers turns: under, content, over with the marks)
        assert!(svg(&Look { failed: true, ..plain.clone() }).matches("rotate(90").count() == 0);
        assert!(key_svg(&Look { failed: true, ..plain }, 90).matches("rotate(90").count() == 3);
    }

    /// Writes the press frame and the red corner on a few keys to `target/key-marks.jpg` to look at.
    #[test]
    #[ignore]
    fn key_marks_picture() {
        let r = Renderer::default();
        let looks: Vec<Look> = (0..15)
            .map(|i| Look {
                label: Some(["Mic", "Open", "Spotify", "Clock", "OBS"][i % 5].into()),
                icon: Some(["mic", "app-window", "music", "clock", "video"][i % 5].into()),
                pressed: i == 1 || i == 7 || i == 12,
                failed: i == 2 || i == 7,
                bg: (i == 12).then(|| "#f2f2f2".into()),
                fg: (i == 12).then(|| "#111111".into()),
                ..Look::default()
            })
            .collect();
        let jpeg = r.panel_jpeg(&panel_tiles(&looks, 0, &PanelGeometry::default()), Some(92)).unwrap();
        std::fs::write(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/key-marks.jpg"), jpeg).unwrap();
    }

    #[test]
    #[ignore]
    fn panel_breakdown_timing() {
        // where a cached panel's 7 ms go (2026-10-15, the release gate's CPU over 15 %)
        use base64::Engine as _;
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/demo");
        let mut frames = Vec::new();
        for name in ["aurora.gif", "bars.gif", "dots.gif", "fire.gif", "heartbeat.gif", "lava.gif", "orbit.gif"] {
            let bytes = std::fs::read(dir.join(name)).unwrap();
            let img = image::load_from_memory(&bytes).unwrap();
            let mut png = std::io::Cursor::new(Vec::new());
            img.write_to(&mut png, image::ImageFormat::Png).unwrap();
            frames.push(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png.get_ref())));
        }
        let looks: Vec<Look> = (0..15).map(|i| Look { image: Some(frames[i % frames.len()].clone()), label: Some("Fun".into()), ..Look::default() }).collect();
        let r = Renderer::default();
        let geometry = PanelGeometry::default();
        r.panel_jpeg(&panel_tiles(&looks, 0, &geometry), Some(80)).unwrap();
        let n = 20;
        let t = std::time::Instant::now();
        for _ in 0..n {
            std::hint::black_box(panel_tiles(&looks, 0, &geometry));
        }
        let build = t.elapsed() / n;
        let tiles = panel_tiles(&looks, 0, &geometry);
        let t = std::time::Instant::now();
        for _ in 0..n {
            for tile in &tiles {
                use std::hash::{Hash, Hasher};
                let mut h = std::collections::hash_map::DefaultHasher::new();
                tile.svg.hash(&mut h);
                std::hint::black_box(h.finish());
            }
        }
        let hash = t.elapsed() / n;
        let bytes: usize = tiles.iter().map(|t| t.svg.len()).sum();
        let mut panel = tiny_skia::Pixmap::new(PANEL_W, PANEL_H).unwrap();
        let t = std::time::Instant::now();
        for _ in 0..n {
            std::hint::black_box(image::RgbImage::from_fn(PANEL_W, PANEL_H, |x, y| {
                let p = panel.pixel(x, y).unwrap().demultiply();
                image::Rgb([p.red(), p.green(), p.blue()])
            }));
        }
        let convert = t.elapsed() / n;
        panel.fill(tiny_skia::Color::from_rgba8(40, 30, 60, 255));
        let rgb = image::RgbImage::from_pixel(PANEL_W, PANEL_H, image::Rgb([40, 30, 60]));
        let t = std::time::Instant::now();
        for _ in 0..n {
            let mut out = std::io::Cursor::new(Vec::new());
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 80).encode_image(&rgb).unwrap();
        }
        let encode = t.elapsed() / n;
        let t = std::time::Instant::now();
        for _ in 0..n {
            r.panel_jpeg(&tiles, Some(80)).unwrap();
        }
        let whole = t.elapsed() / n;
        eprintln!("build tiles {build:?} ({} KB of SVG), hash {hash:?}, to RGB {convert:?}, JPEG {encode:?}; panel_jpeg {whole:?}", bytes / 1024);
    }

    #[test]
    #[ignore]
    fn panel_with_gifs_timing() {
        // a "fun" page: every key a frame of an animated GIF, as the engine draws it (one panel picture)
        use base64::Engine as _;
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/demo");
        let mut frames = Vec::new();
        for name in ["aurora.gif", "bars.gif", "dots.gif", "fire.gif", "heartbeat.gif", "lava.gif", "orbit.gif"] {
            let bytes = std::fs::read(dir.join(name)).unwrap();
            let img = image::load_from_memory(&bytes).unwrap();
            let mut png = std::io::Cursor::new(Vec::new());
            img.write_to(&mut png, image::ImageFormat::Png).unwrap();
            frames.push(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png.get_ref())));
        }
        let looks: Vec<Look> = (0..15).map(|i| Look { image: Some(frames[i % frames.len()].clone()), label: Some("Fun".into()), ..Look::default() }).collect();
        let r = Renderer::default();
        let geometry = PanelGeometry::default();
        let t = std::time::Instant::now();
        let mut size = 0;
        for _ in 0..10 {
            size = r.jpeg(&panel_svg(&looks, 0, &geometry)).unwrap().len();
        }
        eprintln!("panel with 15 GIF frames: {:?} a frame, {} KB", t.elapsed() / 10, size / 1024);
        // the same panel from tiles: the first time each tile is drawn, then taken from memory
        let tiles = panel_tiles(&looks, 0, &geometry);
        let t = std::time::Instant::now();
        r.panel_jpeg(&tiles, Some(80)).unwrap();
        eprintln!("from tiles, first: {:?}", t.elapsed());
        let t = std::time::Instant::now();
        for _ in 0..10 {
            r.panel_jpeg(&panel_tiles(&looks, 0, &geometry), Some(80)).unwrap();
        }
        eprintln!("from tiles, again: {:?} a frame", t.elapsed() / 10);
        // how much lighter a moving panel is sent (the engine's MOVING_QUALITY)
        for q in [98, 90, 80, 70] {
            eprintln!("  at quality {q}: {} KB", r.jpeg_with(&panel_svg(&looks, 0, &geometry), Some(q)).unwrap().len() / 1024);
        }
    }

    /// The quick blit lays a picture as tiny-skia's draw_pixmap does, to a level (opaque, clear and
    /// partly clear pixels, cut at every edge).
    #[test]
    fn blit_matches_draw_pixmap() {
        let mut seed = 0x1234_5678u32;
        let mut rnd = || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed
        };
        let mut src = tiny_skia::Pixmap::new(37, 29).unwrap();
        for p in src.data_mut().as_chunks_mut::<4>().0 {
            let a = match rnd() % 3 {
                0 => 0,
                1 => 255,
                _ => (rnd() % 256) as u8,
            };
            for c in &mut p[..3] {
                *c = (rnd() % (a as u32 + 1)) as u8;
            }
            p[3] = a;
        }
        let mut base = tiny_skia::Pixmap::new(64, 48).unwrap();
        for p in base.data_mut().as_chunks_mut::<4>().0 {
            p.copy_from_slice(&[(rnd() % 256) as u8, (rnd() % 256) as u8, (rnd() % 256) as u8, 255]);
        }
        for (x, y) in [(0, 0), (40, 30), (-10, -5), (13, 7)] {
            let (mut ours, mut theirs) = (base.clone(), base.clone());
            blit(&mut ours, &src, x, y);
            theirs.draw_pixmap(x, y, src.as_ref(), &tiny_skia::PixmapPaint::default(), tiny_skia::Transform::identity(), None);
            let worst = ours.data().iter().zip(theirs.data()).map(|(a, b)| a.abs_diff(*b)).max().unwrap();
            assert!(worst <= 1, "at ({x}, {y}) off by {worst}");
        }
    }

    #[test]
    fn a_panel_from_tiles_looks_like_the_panel_drawn_at_once() {
        let r = Renderer::default();
        let looks: Vec<Look> = (0..15)
            .map(|i| Look {
                label: Some(format!("Key {i}")),
                icon: Some(["play", "mic", "camera"][i % 3].into()),
                bg: Some(["#1d1b2b", "#2a1a3a", "#103020"][i % 3].into()),
                style: [None, Some("outline".to_string()), Some("glass".to_string())][i % 3].clone(),
                ..Look::default()
            })
            .collect();
        for (rotation, geometry) in [(0, PanelGeometry::default()), (90, PanelGeometry { x0: 1.5, y0: 0.5, dx: 99.5, dy: 98.7, w: 78.6, h: 74.4 })] {
            let whole = image::load_from_memory(&r.jpeg_with(&panel_svg(&looks, rotation, &geometry), Some(95)).unwrap()).unwrap().to_rgb8();
            let tiled = image::load_from_memory(&r.panel_jpeg(&panel_tiles(&looks, rotation, &geometry), Some(95)).unwrap()).unwrap().to_rgb8();
            let diff: u64 = whole.as_raw().iter().zip(tiled.as_raw()).map(|(a, b)| a.abs_diff(*b) as u64).sum();
            let mean = diff as f64 / whole.as_raw().len() as f64;
            assert!(mean < 1.5, "rotation {rotation}: mean difference {mean:.2}");
        }
    }

    /// What the computer's fonts cost a key with a label: the same keys drawn with HubDeck's font
    /// only and with the system's fonts too (2026-10-18: the gate's CPU went from 8.6 % to 11.5 %).
    #[test]
    #[ignore]
    fn font_database_cost() {
        let svgs: Vec<String> =
            ["Mute", "Screenshot", "Next track", "Discord", "OBS scene 2"].iter().map(|l| key_svg(&Look { label: Some((*l).into()), icon: Some("camera".into()), ..Look::default() }, 0)).collect();
        let mut own = usvg::Options::default();
        own.fontdb_mut().load_font_data(FONT.to_vec());
        own.font_family = FONT_FAMILY.into();
        let all = Renderer::default().options;
        let time = |opt: &usvg::Options| {
            let n = 400;
            let t = std::time::Instant::now();
            for i in 0..n {
                std::hint::black_box(usvg::Tree::from_str(&svgs[i % svgs.len()], opt).unwrap());
            }
            t.elapsed().as_secs_f64() * 1e6 / n as f64
        };
        let _ = (time(&own), time(&all));
        eprintln!("parse+shape one key: own font {:.0} us, with the system's {:.0} us ({} faces)", time(&own), time(&all), all.fontdb.len());
    }

    #[test]
    #[ignore]
    fn render_timing() {
        let r = Renderer::default();
        let look = Look { label: Some("Screenshot".into()), icon: Some("camera".into()), ..Look::default() };
        let t = std::time::Instant::now();
        for i in 0..15 {
            r.jpeg(&key_svg(&look, (i % 4) * 90)).unwrap();
        }
        let keys = t.elapsed();
        let t = std::time::Instant::now();
        r.jpeg(&dial_svg(&Look::default(), Some((Some(0.5), "Volume")), 0)).unwrap();
        eprintln!("15 keys {:?}, dial {:?}", keys, t.elapsed());
    }
}

#[cfg(test)]
mod look_editor {
    use super::*;

    /// The look editor's new fields drawn as the deck draws them, a sheet of keys at 3x in
    /// logs/look-editor.jpg to be looked at (issue #10, owner's 1a; session 29).
    #[test]
    fn the_look_editor_keys_are_drawn() {
        let base = |label: &str| Look { label: Some(label.into()), icon: Some("music".into()), ..Look::default() };
        let looks = [
            ("auto", base("Song sorter")),
            ("top", Look { label_pos: Some("top".into()), ..base("Like") }),
            ("middle", Look { label_pos: Some("middle".into()), ..base("Like") }),
            ("bottom", Look { label_pos: Some("bottom".into()), ..base("Like") }),
            ("size 16", Look { label_size: Some(16), ..base("Next") }),
            ("size 8", Look { label_size: Some(8), ..base("Do wywalenia") }),
            ("icon small", Look { icon_size: Some("small".into()), ..base("Focus 25") }),
            ("icon large", Look { icon_size: Some("large".into()), ..base("CPU") }),
            ("emoji", Look { icon: Some("emoji:🔥".into()), ..base("Oceń") }),
            ("emoji alone", Look { icon: Some("emoji:🗑️".into()), label: None, ..Look::default() }),
            ("emoji top", Look { icon: Some("emoji:❤️".into()), label_pos: Some("top".into()), icon_size: Some("large".into()), ..base("Lubię to") }),
            ("text 24", Look { label: Some("OBS".into()), label_size: Some(24), ..Look::default() }),
        ];
        let mut sheet = String::from(r##"<svg xmlns="http://www.w3.org/2000/svg" width="1296" height="648" viewBox="0 0 432 216"><rect width="432" height="216" fill="#111"/>"##);
        for (i, (_, look)) in looks.iter().enumerate() {
            let (x, y) = ((i % 6) as f32 * 72.0, (i / 6) as f32 * 108.0);
            sheet += &format!(r#"<g transform="translate({x} {y})">{}</g>"#, key_body(look, 0));
            assert!(key_svg(look, 0).contains("<g"), "{} draws", looks[i].0);
        }
        sheet += "</svg>";
        let jpeg = Renderer::default().jpeg(&sheet).unwrap();
        // logs/ is not in the public repository (its CI failed on 0.1.12's first sync)
        let logs = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../logs");
        std::fs::create_dir_all(&logs).unwrap();
        std::fs::write(logs.join("look-editor.jpg"), jpeg).unwrap();
        // the emoji is drawn as text in the emoji font, escaped
        assert!(key_svg(&looks[8].1, 0).contains("Segoe UI Emoji"));
        assert!(!key_svg(&Look { icon: Some("emoji:<x>".into()), ..Look::default() }, 0).contains("<x>"));
    }
}
