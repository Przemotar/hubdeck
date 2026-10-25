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

/// The font family text on the deck is drawn in, ready for an SVG attribute.
pub fn font_family() -> String {
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

/// A key's background for its style, and what goes over the content (the glass highlight).
fn key_background(style: Option<&str>, s: f32, bg: &str) -> (String, String) {
    match style {
        Some("gradient") => {
            let (top, bottom) = (mix(bg, "#ffffff", 0.22), mix(bg, "#000000", 0.38));
            let id = format!("kg{}{}", &top[1..], &bottom[1..]);
            (
                format!(
                    r#"<defs><linearGradient id="{id}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="{top}"/><stop offset="1" stop-color="{bottom}"/></linearGradient></defs><rect width="{s}" height="{s}" fill="url(#{id})"/>"#
                ),
                String::new(),
            )
        }
        Some("glass") => (
            format!(r#"<rect width="{s}" height="{s}" fill="{bg}"/>"#),
            format!(
                r##"<defs><linearGradient id="kglass" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#fff" stop-opacity="0.34"/><stop offset="0.46" stop-color="#fff" stop-opacity="0.07"/><stop offset="0.5" stop-color="#fff" stop-opacity="0"/><stop offset="1" stop-color="#000" stop-opacity="0.22"/></linearGradient></defs><rect width="{s}" height="{s}" fill="url(#kglass)"/><rect x="1" y="1" width="{w}" height="{w}" rx="8" fill="none" stroke="#fff" stroke-opacity="0.22" stroke-width="1.5"/>"##,
                w = s - 2.0
            ),
        ),
        Some("outline") => (
            format!(r#"<rect width="{s}" height="{s}" fill="{OUTLINE_FILL}"/><rect x="3.5" y="3.5" width="{w}" height="{w}" rx="10" fill="none" stroke="{bg}" stroke-width="3"/>"#, w = s - 7.0),
            String::new(),
        ),
        _ => (format!(r#"<rect width="{s}" height="{s}" fill="{bg}"/>"#), String::new()),
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
/// A picture filling the screen, with the label on a dark strip at the bottom; `round` (the dial):
/// the strip sits higher, where the round screen is wide enough.
fn picture(uri: &str, size: f32, label: &str, fg: &str, label_px: f32, round: bool) -> String {
    let mut out = format!(r#"<svg width="{size}" height="{size}" viewBox="0 0 {size} {size}"><image href="{uri}" width="{size}" height="{size}" preserveAspectRatio="xMidYMid slice"/></svg>"#);
    if !label.is_empty() {
        let strip = label_px * 1.7;
        let top = if round { size * 0.8 - strip } else { size - strip };
        let baseline = top + strip * 0.7;
        let max_w = if round { 2.0 * dial_chord(baseline) - 16.0 } else { size - 6.0 };
        let (rows, px) = fit(label, label_px, label_px * 0.72, max_w, false);
        out += &format!(r##"<rect y="{top}" width="{size}" height="{strip}" fill="#000" fill-opacity="0.55"/>"##);
        out += &text(size / 2.0, baseline - (label_px - px) * 0.35, px, fg, &rows[0]);
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
    let s = KEY_PX as f32;
    let body = key_content(look, rotation);
    let marks = key_marks(look, s);
    if marks.is_empty() {
        return body;
    }
    format!(r#"{body}<g transform="rotate({rotation} {c} {c})">{marks}</g>"#, c = s / 2.0)
}

fn key_content(look: &Look, rotation: i32) -> String {
    let s = KEY_PX as f32;
    let bg = color(look.bg.as_ref(), DEFAULT_BG);
    let fg = color(look.fg.as_ref(), DEFAULT_FG);
    let label = look.label.as_deref().unwrap_or("").trim();
    let (under, over) = key_background(look.style.as_deref(), s, &bg);
    let ic = color(look.icon_color.as_ref(), &fg);
    // an app's icon or a logo stands where an icon would; other pictures fill the key
    let logo = look.image.as_deref().filter(|_| picture_as_icon(look)).and_then(super::images::uri);
    if logo.is_none()
        && let Some(uri) = look.image.as_deref().and_then(super::images::filled_uri)
    {
        return format!(r#"<g transform="rotate({rotation} {c} {c})">{under}{}{over}</g>"#, picture(&uri, s, picture_label(look, label), &fg, 11.5, false), c = s / 2.0);
    }
    if look.face {
        let (main, second) = label.split_once('\n').map(|(a, b)| (a, Some(b))).unwrap_or((label, None));
        let start = match main.chars().count() {
            0..=4 => 26.0,
            5 => 23.0,
            6..=7 => 17.0,
            _ => 14.0,
        };
        let (main_rows, size) = fit(main, start, 9.0, s - 6.0, false);
        let y = if second.is_some() { s / 2.0 + size * 0.1 } else { s / 2.0 + size * 0.35 };
        let mut body = text(s / 2.0, y, size, &fg, &main_rows[0]);
        if let Some(second) = second {
            let (rows, px) = fit(second, 11.0, 8.0, s - 6.0, false);
            body += &text(s / 2.0, y + 19.0, px, &fg, &rows[0]);
        }
        return format!(r#"<g transform="rotate({rotation} {c} {c})">{under}{body}{over}</g>"#, c = s / 2.0);
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
            let (rows, px) = fit(label, size.unwrap_or(11.5), size.map_or(7.5, |z| z.min(7.5)), s - 6.0, true);
            let gap = 1.1;
            let block = (rows.len() - 1) as f32 * px * gap;
            let y = match pos {
                "top" => px * 0.8 + 3.0,
                "middle" => s / 2.0 + px * 0.35 - block / 2.0,
                _ => s - 6.0 - block,
            };
            let edge = if has_mark { r##" stroke="#000" stroke-opacity="0.55" stroke-width="2.4" stroke-linejoin="round" paint-order="stroke""## } else { "" };
            mark(16.0, 16.0, 40.0) + &format!("<g{edge}>{}</g>", lines(s / 2.0, y, px, gap, &fg, &rows))
        }
        (_, true, true) => mark(16.0, 16.0, 40.0),
        (_, true, false) => match fit(label, size.unwrap_or(11.5), size.map_or(9.5, |z| z.min(9.5)), s - 6.0, true) {
            (rows, px) if rows.len() == 1 => mark(21.0, 8.0, 30.0) + &text(s / 2.0, 61.0, px, &fg, &rows[0]),
            // a long label: a smaller icon above two lines
            (rows, px) => mark(25.0, 6.0, 22.0) + &lines(s / 2.0, 47.0, if size.is_some() { px } else { px.min(10.5) }, 1.12, &fg, &rows),
        },
        (_, false, false) => match size {
            Some(z) => {
                let (rows, px) = fit(label, z, z.min(7.5), s - 8.0, true);
                lines(s / 2.0, s / 2.0 + px * 0.35 - (rows.len() - 1) as f32 * px * 1.1 / 2.0, px, 1.1, &fg, &rows)
            }
            None => centered_label(label, s, s, s - 8.0, &fg),
        },
        (_, false, true) => String::new(),
    };
    format!(r#"<g transform="rotate({rotation} {c} {c})">{under}{body}{over}</g>"#, c = s / 2.0)
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
    jpeg_encoder::Encoder::new(&mut out, quality).encode(pixmap.data(), w, h, jpeg_encoder::ColorType::Rgba).map_err(|e| e.to_string())?;
    Ok(out)
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
            let svg = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="{tw}" height="{th}" viewBox="{left} {top} {tw} {th}"><rect x="{}" y="{}" width="{}" height="{}" fill="{bg}"/><g transform="translate({cx} {cy}) scale({scale})">{}</g></svg>"#,
                x - bleed_x,
                y - bleed_y,
                w + 2.0 * bleed_x,
                h + 2.0 * bleed_y,
                key_body(look, rotation)
            );
            Tile { x: left as i32, y: top as i32, svg }
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
const KEY_QUALITY: u8 = 98;
const DIAL_QUALITY: u8 = 90;

pub struct Renderer {
    options: usvg::Options<'static>,
    /// Key tiles drawn before, by the hash of their SVG (one per key look and GIF frame).
    tiles: std::sync::Mutex<std::collections::HashMap<u64, std::sync::Arc<tiny_skia::Pixmap>>>,
}

/// Key tiles kept (a key tile is ~40 KB: 400 is ~16 MB, a page of GIFs of up to ~26 frames).
const TILES_KEPT: usize = 400;

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
    pub fn panel_jpeg(&self, tiles: &[Tile], quality: Option<u8>) -> Result<Vec<u8>, String> {
        use std::hash::{Hash, Hasher};
        let mut panel = tiny_skia::Pixmap::new(PANEL_W, PANEL_H).ok_or("empty image")?;
        panel.fill(tiny_skia::Color::BLACK);
        for tile in tiles {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            tile.svg.hash(&mut h);
            let key = h.finish();
            let kept = self.tiles.lock().unwrap_or_else(|e| e.into_inner()).get(&key).cloned();
            let pixmap = match kept {
                Some(p) => p,
                None => {
                    let tree = usvg::Tree::from_str(&tile.svg, &self.options).map_err(|e| e.to_string())?;
                    let size = tree.size().to_int_size();
                    let mut p = tiny_skia::Pixmap::new(size.width(), size.height()).ok_or("empty tile")?;
                    resvg::render(&tree, tiny_skia::Transform::default(), &mut p.as_mut());
                    let p = std::sync::Arc::new(p);
                    let mut all = self.tiles.lock().unwrap_or_else(|e| e.into_inner());
                    // a new page or new looks: start over rather than grow
                    if all.len() >= TILES_KEPT {
                        all.clear();
                    }
                    all.insert(key, p.clone());
                    p
                }
            };
            panel.draw_pixmap(tile.x, tile.y, (*pixmap).as_ref(), &tiny_skia::PixmapPaint::default(), tiny_skia::Transform::identity(), None);
        }
        encode_jpeg(&panel, quality.unwrap_or(KEY_QUALITY))
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
        // the marks turn with the content, so the corner is top right as the user reads the key
        assert!(svg(&Look { failed: true, ..plain.clone() }).matches("rotate(90").count() == 0);
        assert!(key_svg(&Look { failed: true, ..plain }, 90).matches("rotate(90").count() == 2);
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
