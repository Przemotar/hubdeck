//! The living deck's canvas (`docs/LIVING-DECK.md`): effects across the whole key screen, drawn
//! into the panel picture around the keys' own layers. An effect draws under the keys' content
//! (fog, fireflies, embers), over it (rain on the glass, snow, the light of a ripple) or both, and
//! answers presses. Every effect gets the same inputs ([`Inputs`]) in the panel's coordinates
//! with the screen's own "down", so a later kind of effect (a shader program, stage 4) plugs in
//! the same way; the content is drawn between `under` and `over` and never at a lower quality.
//!
//! Costs measured in a release bench of the whole 480x272 screen (2026-10-25, `docs/KNOWLEDGE.md`):
//! a wave field with refraction 2.2 ms a frame, 60 lens drops 0.13 ms.

use resvg::tiny_skia;

use super::render::{PANEL_H, PANEL_W};

const W: usize = PANEL_W as usize;
const H: usize = PANEL_H as usize;

/// The canvas effects (`Effects::canvas`).
pub const CANVASES: &[&str] = &["fog", "rain", "snow", "fireflies", "embers"];
/// What a press does (`Effects::reaction`).
pub const REACTIONS: &[&str] = &["ripple", "embers"];
/// What may live on the deck (`Effects::inhabitant`).
pub const INHABITANTS: &[&str] = &["bats"];

/// Where a key's window is on the panel, px.
#[derive(Clone, Copy, Debug, Default)]
pub struct KeyPlace {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// What every effect is given each frame.
pub struct Inputs<'a> {
    /// Seconds since the effect started.
    pub t: f32,
    /// Seconds since the last frame (at most a tenth: a pause is not a jump).
    pub dt: f32,
    pub keys: &'a [KeyPlace],
    /// The screen's own "down" in panel coordinates (a unit vector): the module's turn.
    pub down: (f32, f32),
    /// The effect's colour, 0..1.
    pub tint: [f32; 3],
    /// How strong, 0..1.
    pub strength: f32,
    /// How much of each panel pixel is the keys' content (icon, label, picture), 0..255; empty
    /// before the first picture. Effects use it to settle on icons, light pictures that hide
    /// what is under them, and keep off labels.
    pub mask: &'a [u8],
    /// The layer's pace, 0.25..4 (1 = its own); `t` and `dt` already run at it.
    pub speed: f32,
    /// How much of it, 0.25..4 (1 = its own amount).
    pub density: f32,
    /// Sideways, -1..1 (negative to the screen's left).
    pub wind: f32,
    /// The deck's one light: x, y, height above the screen (px), 1 (the lamp's place when the lamp
    /// is on, else a light drifting beyond the top edge); glass and frames are lit by it.
    pub light: [f32; 4],
    /// Like `mask`, the icons and pictures alone (no labels or values); empty unless a layer asks
    /// for it ([`Effect::wants_icons`]).
    pub icons: &'a [u8],
    /// How rarely each key is pressed, 0..1 (1 = never, from the owner's press counts): eyes and
    /// the spider visit those keys more often.
    pub rare: [f32; 15],
    /// It wanders (the lamp) and keeps to the left (jack-o'-lanterns, cobwebs).
    pub moving: bool,
    pub left: bool,
}

impl Inputs<'_> {
    /// The screen's "right" in panel coordinates (wind blows along it).
    pub fn side(&self) -> (f32, f32) {
        (self.down.1, -self.down.0)
    }
    /// How many of `base` things the layer's amount asks for, at most `max`.
    pub fn count(&self, base: usize, max: usize) -> usize {
        ((base as f32 * self.density).round() as usize).clamp(1, max)
    }

    /// The content's cover at a panel pixel, 0..1 (0 outside the panel or before a picture).
    #[inline]
    pub fn content(&self, x: usize, y: usize) -> f32 {
        if x >= W || y >= H {
            return 0.0;
        }
        self.mask.get(y * W + x).map_or(0.0, |&a| a as f32 / 255.0)
    }
}

/// One effect of the canvas.
pub trait Effect: Send {
    /// Draw under the keys' content; `panel` holds the keys' backgrounds.
    fn under(&mut self, _input: &Inputs, _panel: &mut [u8]) {}
    /// Draw over the keys' content; `panel` holds the whole picture.
    fn over(&mut self, _input: &Inputs, _panel: &mut [u8]) {}
    /// A key was pressed.
    fn press(&mut self, _key: usize, _input: &Inputs) {}
    /// A key whose action likes something (Spotify's "Like") was pressed.
    fn like(&mut self, _key: usize, _input: &Inputs) {}
    /// The page changed; `old` is the last picture of the page before (RGBA).
    fn page_turned(&mut self, _old: &[u8], _input: &Inputs) {}
    /// Whether it needs the last picture kept for a page turn.
    fn wants_last(&self) -> bool {
        false
    }
    /// Whether it needs the icons' cover without the labels ([`Inputs::icons`]).
    fn wants_icons(&self) -> bool {
        false
    }
    /// Its program on the card, when its `under` (or `over`) now would only run that program on
    /// the panel: the canvas then draws it in a chain with its neighbours ([`super::gpu::draw_chain`]).
    fn shader(&self, _over: bool, _input: &Inputs) -> Option<&super::gpu::ShaderLayer> {
        None
    }
    /// Moves the time on; false when it rests and needs no new pictures.
    fn step(&mut self, input: &Inputs) -> bool;
}

/// A colour added as light at `i` (an RGBA pixel), `a` 0..1.
#[inline]
fn add(px: &mut [u8], i: usize, c: [f32; 3], a: f32) {
    for k in 0..3 {
        let v = px[i + k] as f32 + c[k] * 255.0 * a;
        px[i + k] = v.min(255.0) as u8;
    }
}

/// A colour laid over `i`, `a` 0..1.
#[inline]
fn blend(px: &mut [u8], i: usize, c: [f32; 3], a: f32) {
    for k in 0..3 {
        let v = px[i + k] as f32 * (1.0 - a) + c[k] * 255.0 * a;
        px[i + k] = v.clamp(0.0, 255.0) as u8;
    }
}

/// A soft round light at (x, y): full at the centre, gone at `r`.
fn glow(px: &mut [u8], x: f32, y: f32, r: f32, c: [f32; 3], a: f32) {
    let (x0, x1) = ((x - r).floor().max(0.0) as usize, ((x + r).ceil() as usize).min(W - 1));
    let (y0, y1) = ((y - r).floor().max(0.0) as usize, ((y + r).ceil() as usize).min(H - 1));
    if x - r > W as f32 || y - r > H as f32 || x + r < 0.0 || y + r < 0.0 {
        return;
    }
    let r2 = r * r;
    for yy in y0..=y1 {
        for xx in x0..=x1 {
            let d2 = (xx as f32 + 0.5 - x).powi(2) + (yy as f32 + 0.5 - y).powi(2);
            if d2 < r2 {
                let f = 1.0 - d2 / r2;
                add(px, (yy * W + xx) * 4, c, a * f * f);
            }
        }
    }
}

/// A small random number generator (the same run every time it starts with the same seed).
struct Rng(u32);
impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 % 100_000) as f32 / 100_000.0
    }
    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.next()
    }
}

/// Smooth value noise, 0..1, repeating every `period` cells.
fn noise(x: f32, y: f32, period: i32, seed: u32) -> f32 {
    let hash = |x: i32, y: i32| {
        let (x, y) = (x.rem_euclid(period), y.rem_euclid(period));
        let mut h = (x as u32).wrapping_mul(374_761_393) ^ (y as u32).wrapping_mul(668_265_263) ^ seed.wrapping_mul(2_246_822_519);
        h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
        ((h ^ (h >> 16)) & 0xffff) as f32 / 65535.0
    };
    let (xi, yi) = (x.floor() as i32, y.floor() as i32);
    let (u, v) = (x - x.floor(), y - y.floor());
    let s = |t: f32| t * t * (3.0 - 2.0 * t);
    let (u, v) = (s(u), s(v));
    let (a, b, c, d) = (hash(xi, yi), hash(xi + 1, yi), hash(xi, yi + 1), hash(xi + 1, yi + 1));
    a + (b - a) * u + (c - a) * v + (a - b - c + d) * u * v
}

// ---------------------------------------------------------------------------------------------
// Design rules for this screen (camera, 2026-10-26): it crushes dark tones (under ~#24 is black)
// and shows each key through a small window at 16 pictures a second. An effect reads only when it
// is light on dark, big enough to cross windows, with a soft bloom round its bright parts, and
// moves in long smooth paths, never in specks.

/// A light line from `a` to `b`, `r` thick, fading toward `a` (a streak, a spark's trail).
fn streak(px: &mut [u8], a: (f32, f32), b: (f32, f32), r: f32, c: [f32; 3], alpha: f32) {
    let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt().max(0.5);
    let steps = (len / (r * 0.6).max(0.5)).ceil() as usize;
    for s in 0..=steps {
        let f = s as f32 / steps.max(1) as f32;
        glow(px, a.0 + (b.0 - a.0) * f, a.1 + (b.1 - a.1) * f, r, c, alpha * f / steps.max(1) as f32 * 3.0);
    }
}

// ---------------------------------------------------------------------------------------------
// The ripple: a press drops into a wave field at half the panel's size; the rings run across the
// keys as bands of light in the tint with a white crest, and bend everything under them, the
// icons and labels too, for the moment the ring passes; the pressed key flashes.

const RW: usize = W / 2;
const RH: usize = H / 2;

pub struct Ripple {
    a: Vec<f32>,
    b: Vec<f32>,
    energy: f32,
    picture: Vec<u8>,
    flashes: Vec<(usize, f32)>,
}

impl Ripple {
    pub fn new() -> Self {
        Self { a: vec![0.0; RW * RH], b: vec![0.0; RW * RH], energy: 0.0, picture: Vec::new(), flashes: Vec::new() }
    }
    /// The slope at a panel pixel, smoothly between the field's cells.
    fn slope(&self, x: usize, y: usize) -> (f32, f32) {
        let (fx, fy) = ((x as f32 / 2.0).clamp(1.0, (RW - 3) as f32), (y as f32 / 2.0).clamp(1.0, (RH - 3) as f32));
        let (ix, iy) = (fx as usize, fy as usize);
        let (u, v) = (fx - ix as f32, fy - iy as f32);
        let g = |x: usize, y: usize| {
            let i = y * RW + x;
            (self.a[i + 1] - self.a[i - 1], self.a[i + RW] - self.a[i - RW])
        };
        let (a, b, c, d) = (g(ix, iy), g(ix + 1, iy), g(ix, iy + 1), g(ix + 1, iy + 1));
        let lerp = |p: f32, q: f32, t: f32| p + (q - p) * t;
        (lerp(lerp(a.0, b.0, u), lerp(c.0, d.0, u), v), lerp(lerp(a.1, b.1, u), lerp(c.1, d.1, u), v))
    }
}

impl Effect for Ripple {
    fn press(&mut self, key: usize, input: &Inputs) {
        let Some(k) = input.keys.get(key) else { return };
        let (cx, cy) = ((k.x + k.w / 2.0) / 2.0, (k.y + k.h / 2.0) / 2.0);
        let r = 7.0;
        for y in (cy - r) as i32..=(cy + r) as i32 {
            for x in (cx - r) as i32..=(cx + r) as i32 {
                if x < 1 || y < 1 || x >= RW as i32 - 1 || y >= RH as i32 - 1 {
                    continue;
                }
                let d = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt() / r;
                if d < 1.0 {
                    self.a[y as usize * RW + x as usize] -= 14.0 * (0.4 + 0.6 * input.strength) * (0.5 + 0.5 * (d * std::f32::consts::PI).cos());
                }
            }
        }
        self.flashes.push((key, 1.0));
        self.energy = 1.0;
    }

    fn step(&mut self, input: &Inputs) -> bool {
        for f in &mut self.flashes {
            f.1 -= input.dt * 2.6;
        }
        self.flashes.retain(|f| f.1 > 0.0);
        if self.energy < 0.003 && self.flashes.is_empty() {
            return false;
        }
        // twelve small steps a frame at its own pace: a ring crosses the deck in about a second
        for _ in 0..((12.0 * input.speed).round() as usize).clamp(1, 48) {
            for y in 1..RH - 1 {
                for x in 1..RW - 1 {
                    let i = y * RW + x;
                    let v = (self.a[i - 1] + self.a[i + 1] + self.a[i - RW] + self.a[i + RW]) * 0.5 - self.b[i];
                    self.b[i] = v * 0.991;
                }
            }
            std::mem::swap(&mut self.a, &mut self.b);
        }
        let sum: f32 = self.a.iter().step_by(5).map(|v| v.abs()).sum();
        self.energy = sum / (RW * RH / 5) as f32;
        if self.energy < 0.003 {
            self.a.iter_mut().for_each(|v| *v = 0.0);
            self.b.iter_mut().for_each(|v| *v = 0.0);
        }
        true
    }

    fn over(&mut self, input: &Inputs, panel: &mut [u8]) {
        // the pressed key lights up in the tint and fades
        for &(key, f) in &self.flashes {
            if let Some(k) = input.keys.get(key) {
                let a = f * f * 0.8;
                for y in k.y as usize..((k.y + k.h) as usize).min(H) {
                    for x in k.x as usize..((k.x + k.w) as usize).min(W) {
                        add(panel, (y * W + x) * 4, input.tint, a);
                    }
                }
            }
        }
        if self.energy < 0.003 {
            return;
        }
        self.picture.clear();
        self.picture.extend_from_slice(panel);
        let white = [1.0, 1.0, 1.0];
        for y in 0..H {
            for x in 0..W {
                let (dx, dy) = self.slope(x, y);
                let g = (dx * dx + dy * dy).sqrt();
                if g < 0.03 {
                    continue;
                }
                let i = (y * W + x) * 4;
                // bend what is under the ring (the icons and labels too, for a moment)
                let (sx, sy) = ((x as f32 + dx * 4.5).clamp(0.0, (W - 1) as f32) as usize, (y as f32 + dy * 4.5).clamp(0.0, (H - 1) as f32) as usize);
                let j = (sy * W + sx) * 4;
                panel[i..i + 3].copy_from_slice(&self.picture[j..j + 3]);
                // a band of light on every ring, brightest on the slope facing the top left
                let facing = (-dx * 0.55 - dy * 0.83).max(0.0);
                let band = (g * 0.32).min(1.0);
                add(panel, i, input.tint, band * 0.9 * (0.5 + 0.5 * input.strength));
                add(panel, i, white, ((facing * 0.6) - 0.15).clamp(0.0, 1.0));
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Rain on the glass: big drops that stick, then slide down in fits, each a lens that turns the
// keys under it upside down, with a bright rim of light on top and a shadow under it; rain falls
// behind the glass in long faint streaks, and now and then lightning lights the whole deck.

struct Drop {
    x: f32,
    y: f32,
    r: f32,
    speed: f32,
    hold: f32,
}

pub struct Rain {
    rng: Rng,
    drops: Vec<Drop>,
    beads: Vec<(f32, f32, f32, f32)>,
    streaks: Vec<(f32, f32, f32)>,
    picture: Vec<u8>,
    lightning: f32,
    next_lightning: f32,
}

impl Rain {
    pub fn new() -> Self {
        let mut rng = Rng(0x2545_f491);
        let drops = (0..64).map(|_| Drop { x: rng.range(0.0, W as f32), y: rng.range(0.0, H as f32), r: rng.range(3.0, 6.5), speed: 0.0, hold: rng.range(0.0, 3.0) }).collect();
        let beads = (0..70).map(|_| (rng.range(0.0, W as f32), rng.range(0.0, H as f32), rng.range(1.2, 2.6), rng.range(0.5, 1.0))).collect();
        let streaks = (0..280).map(|_| (rng.range(0.0, W as f32), rng.range(0.0, H as f32), rng.range(0.5, 1.0))).collect();
        Self { rng, drops, beads, streaks, picture: Vec::new(), lightning: 0.0, next_lightning: 9.0 }
    }

    fn lens(&self, panel: &mut [u8], x: f32, y: f32, r: f32, down: (f32, f32), a: f32) {
        let (x0, x1) = ((x - r).floor().max(0.0) as i32, ((x + r).ceil() as i32).min(W as i32 - 1));
        let (y0, y1) = ((y - r).floor().max(0.0) as i32, ((y + r).ceil() as i32).min(H as i32 - 1));
        for yy in y0..=y1 {
            for xx in x0..=x1 {
                let (dx, dy) = (xx as f32 + 0.5 - x, yy as f32 + 0.5 - y);
                let d = (dx * dx + dy * dy).sqrt() / r;
                if d >= 1.0 {
                    continue;
                }
                let (sx, sy) = ((x - dx * 2.6).clamp(0.0, (W - 1) as f32) as usize, (y - dy * 2.6).clamp(0.0, (H - 1) as f32) as usize);
                let (i, j) = ((yy as usize * W + xx as usize) * 4, (sy * W + sx) * 4);
                // the water's body: what it sees, a little brighter, darker at its edge
                let edge = ((d - 0.6) / 0.4).max(0.0);
                for k in 0..3 {
                    // water catches the room's light: a clear body even over a black key
                    let seen = (self.picture[j + k] as f32 * 1.2 + 34.0 + 30.0 * (1.0 - d)).min(255.0);
                    let v = panel[i + k] as f32 * (1.0 - a) + seen * a;
                    panel[i + k] = (v * (1.0 - 0.5 * edge * edge * a)) as u8;
                }
                let toward = (dx * down.0 + dy * down.1) / r;
                // the rim of light on the side away from "down", and a highlight dot
                let rim = (1.0 - ((d - 0.86).abs() * 7.0)).max(0.0) * (-toward).max(0.0);
                let dot = (1.0 - (((dx / r + 0.32).powi(2) + (dy / r - toward * 0.0 + 0.36).powi(2)).sqrt() * 5.0)).max(0.0);
                add(panel, i, [0.92, 0.97, 1.0], (rim * 1.3 + dot * 1.2) * a);
            }
        }
    }
}

impl Effect for Rain {
    fn step(&mut self, input: &Inputs) -> bool {
        let (gx, gy) = input.down;
        let (sx, sy) = input.side();
        let drops = input.count(16, 64);
        for d in self.drops.iter_mut().take(drops) {
            if d.hold > 0.0 {
                d.hold -= input.dt;
                continue;
            }
            d.speed = (d.speed + 90.0 * input.dt).min(50.0 + d.r * 12.0);
            d.x += (gx * d.speed + sx * input.wind * 40.0) * input.dt + (self.rng.next() - 0.5) * 0.5;
            d.y += (gy * d.speed + sy * input.wind * 40.0) * input.dt;
            if self.rng.next() < 0.7 * input.dt {
                d.hold = self.rng.range(0.5, 2.5);
                d.speed = 0.0;
            }
            if self.rng.next() < 7.0 * input.dt {
                self.beads.push((d.x - gx * d.r, d.y - gy * d.r, d.r * 0.4, 1.0));
            }
            if d.x < -12.0 || d.y < -12.0 || d.x > W as f32 + 12.0 || d.y > H as f32 + 12.0 {
                d.x = if gx.abs() > 0.5 { if gx > 0.0 { -8.0 } else { W as f32 + 8.0 } } else { self.rng.range(0.0, W as f32) };
                d.y = if gy.abs() > 0.5 { if gy > 0.0 { -8.0 } else { H as f32 + 8.0 } } else { self.rng.range(0.0, H as f32) };
                d.r = self.rng.range(3.0, 6.5);
                d.hold = self.rng.range(0.0, 1.0);
            }
        }
        for b in &mut self.beads {
            b.3 -= input.dt * 0.04;
        }
        self.beads.retain(|b| b.3 > 0.0);
        if self.beads.len() > 160 {
            let extra = self.beads.len() - 160;
            self.beads.drain(..extra);
        }
        // rain behind the glass falls fast, a little slanted
        let slant = 60.0 - input.wind * 260.0;
        let streaks = input.count(70, 280);
        for s in self.streaks.iter_mut().take(streaks) {
            s.0 += (gx * 520.0 - gy * slant) * input.dt * s.2;
            s.1 += (gy * 520.0 + gx * slant) * input.dt * s.2;
            if s.0 < -30.0 || s.1 < -30.0 || s.0 > W as f32 + 30.0 || s.1 > H as f32 + 30.0 {
                s.0 = if gx.abs() > 0.5 { if gx > 0.0 { -20.0 } else { W as f32 + 20.0 } } else { self.rng.range(-40.0, W as f32) };
                s.1 = if gy.abs() > 0.5 { if gy > 0.0 { -20.0 } else { H as f32 + 20.0 } } else { self.rng.range(-40.0, H as f32) };
            }
        }
        // lightning: two quick flashes far apart (a flashing effect: the window warns of it)
        self.next_lightning -= input.dt;
        if self.next_lightning <= 0.0 {
            self.lightning = 1.0;
            self.next_lightning = self.rng.range(14.0, 28.0);
        }
        self.lightning = (self.lightning - input.dt * 2.2).max(0.0);
        true
    }

    fn under(&mut self, input: &Inputs, panel: &mut [u8]) {
        let (gx, gy) = input.down;
        let slant = (60.0 - input.wind * 260.0) / 520.0 * 22.0;
        for s in self.streaks.iter().take(input.count(70, 280)) {
            let tail = (s.0 - (gx * 22.0 - gy * slant) * s.2, s.1 - (gy * 22.0 + gx * slant) * s.2);
            streak(panel, tail, (s.0, s.1), 1.0, [0.7, 0.85, 1.0], 0.42 * s.2 * (0.5 + input.strength));
        }
        // the flash: bright, then a second smaller one (never more than two a second)
        let l = self.lightning;
        let flash = if l > 0.75 {
            (l - 0.75) * 4.0
        } else if (0.35..0.55).contains(&l) {
            (l - 0.35) * 2.5
        } else {
            0.0
        };
        if flash > 0.0 {
            for i in (0..W * H).map(|i| i * 4) {
                add(panel, i, [0.75, 0.85, 1.0], flash * 0.45);
            }
        }
    }

    fn over(&mut self, input: &Inputs, panel: &mut [u8]) {
        self.picture.clear();
        self.picture.extend_from_slice(panel);
        let a = 0.75 + 0.25 * input.strength;
        for b in &self.beads {
            self.lens(panel, b.0, b.1, b.2, input.down, a * b.3.min(1.0) * 0.85);
        }
        for d in self.drops.iter().take(input.count(16, 64)) {
            self.lens(panel, d.x, d.y, d.r, input.down, a);
        }
    }

    fn press(&mut self, key: usize, input: &Inputs) {
        let Some(k) = input.keys.get(key) else { return };
        for d in &mut self.drops {
            if d.x > k.x - 10.0 && d.x < k.x + k.w + 10.0 && d.y > k.y - 10.0 && d.y < k.y + k.h + 10.0 {
                d.hold = 0.0;
                d.speed = 80.0;
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Snow: soft flakes at three depths drift down, the near ones big and blurred like in a photo;
// snow settles on the top edge of every key window and on the top of what is drawn on the key
// (icons, labels, pictures: the content's mask), thick where it is flat, thin or none where it is
// steep, with a glint now and then; a press knocks a key's snow off in a puff (owner, 2026-10-26:
// "Zastanawiam się czy nie da się tak zrobić żeby faktycznie osiadał na ikonach od góry?").

struct Flake {
    x: f32,
    y: f32,
    depth: f32,
    phase: f32,
}

/// Where a point of a key is: `along` its top edge, `deep` from that edge toward "down".
fn on_key(p: &KeyPlace, down: (f32, f32), along: f32, deep: f32) -> (f32, f32) {
    let (gx, gy) = down;
    if gy.abs() >= gx.abs() { (p.x + along, if gy > 0.0 { p.y + deep } else { p.y + p.h - 1.0 - deep }) } else { (if gx > 0.0 { p.x + deep } else { p.x + p.w - 1.0 - deep }, p.y + along) }
}

/// A key's length along its top edge and its depth toward "down".
fn key_span(p: &KeyPlace, down: (f32, f32)) -> (usize, usize) {
    if down.1.abs() >= down.0.abs() { (p.w as usize, p.h as usize) } else { (p.h as usize, p.w as usize) }
}

/// Snow lying on a key's content: per column, how deep the content's top is and how much lies on it.
#[derive(Clone, Default)]
struct Drift {
    surface: Vec<f32>,
    height: Vec<f32>,
}

pub struct Snow {
    rng: Rng,
    flakes: Vec<Flake>,
    caps: Vec<Vec<f32>>,
    drifts: Vec<Drift>,
    falling: Vec<(f32, f32, f32, f32)>,
}

impl Snow {
    pub fn new(keys: usize) -> Self {
        let mut rng = Rng(0x9e37_79b9);
        let flakes = (0..440).map(|_| Flake { x: rng.range(0.0, W as f32), y: rng.range(0.0, H as f32), depth: rng.next().powf(1.6), phase: rng.range(0.0, std::f32::consts::TAU) }).collect();
        Self { rng, flakes, caps: vec![Vec::new(); keys], drifts: vec![Drift::default(); keys], falling: Vec::new() }
    }

    /// Snow lying `h` deep from depth `deep` up (against "down"), bright on top, a cool shade below.
    fn lay(panel: &mut [u8], p: &KeyPlace, down: (f32, f32), along: f32, deep: f32, h: f32) {
        for d in 0..(h.ceil() as usize) {
            let cover = (h - d as f32).min(1.0);
            let (x, y) = on_key(p, down, along, deep - d as f32);
            let (x, y) = (x as usize, y as usize);
            if x >= W || y >= H {
                continue;
            }
            let shade = 1.0 - d as f32 / h.max(1.0);
            let c = [0.97 - 0.25 * shade, 0.98 - 0.15 * shade, 1.0];
            blend(panel, (y * W + x) * 4, c, 0.95 * cover);
        }
    }
}

impl Effect for Snow {
    fn step(&mut self, input: &Inputs) -> bool {
        let (gx, gy) = input.down;
        let (sx, sy) = input.side();
        let flakes = input.count(110, 440);
        for f in self.flakes.iter_mut().take(flakes) {
            let speed = 12.0 + 34.0 * f.depth;
            let sway = (input.t * (0.5 + f.depth) + f.phase).sin() * (5.0 + 9.0 * f.depth);
            let blow = input.wind * (30.0 + 40.0 * f.depth);
            f.x += (gx * speed - gy * sway * 0.5 + sx * blow) * input.dt;
            f.y += (gy * speed + gx * sway * 0.5 + sy * blow) * input.dt;
            if f.x < -10.0 || f.y < -10.0 || f.x > W as f32 + 10.0 || f.y > H as f32 + 10.0 {
                f.x = if gx.abs() > 0.5 { if gx > 0.0 { -6.0 } else { W as f32 + 6.0 } } else { self.rng.range(0.0, W as f32) };
                f.y = if gy.abs() > 0.5 { if gy > 0.0 { -6.0 } else { H as f32 + 6.0 } } else { self.rng.range(0.0, H as f32) };
                // blown sideways off the screen: back in from the other side, anywhere along it
                if input.wind != 0.0 && self.rng.next() < input.wind.abs() * 0.6 {
                    let (along, from) = (self.rng.next(), input.wind.signum());
                    if sx.abs() > 0.5 {
                        f.x = if sx * from > 0.0 { -6.0 } else { W as f32 + 6.0 };
                        f.y = along * H as f32;
                    } else {
                        f.y = if sy * from > 0.0 { -6.0 } else { H as f32 + 6.0 };
                        f.x = along * W as f32;
                    }
                }
            }
        }
        for (k, cap) in self.caps.iter_mut().enumerate() {
            let Some(place) = input.keys.get(k) else { continue };
            let (len, _) = key_span(place, input.down);
            if cap.len() != len {
                *cap = vec![0.0; len];
            }
            let n = len as f32;
            for (i, c) in cap.iter_mut().enumerate() {
                // thicker in the middle, rounded at the ends, uneven like real snow
                let along = i as f32 / n;
                let hump = (along * std::f32::consts::PI).sin().powf(0.6);
                let want = hump * (5.0 + 3.0 * noise(i as f32 / 10.0, k as f32 * 3.0, 64, 5)) + 0.8 * noise(i as f32 / 3.0, k as f32, 64, 9);
                *c = (*c + input.dt * 0.4 * (0.6 + noise(i as f32 / 6.0, input.t * 0.2, 64, 3))).min(want);
            }
        }
        // on the content: find its top in each column, then let snow build where it is flat enough
        if !input.mask.is_empty() {
            for (k, drift) in self.drifts.iter_mut().enumerate() {
                let Some(place) = input.keys.get(k) else { continue };
                let (len, depth) = key_span(place, input.down);
                if drift.surface.len() != len {
                    *drift = Drift { surface: vec![-1.0; len], height: vec![0.0; len] };
                }
                for i in 0..len {
                    let top = (3..depth.saturating_sub(2)).find(|&d| {
                        let (x, y) = on_key(place, input.down, i as f32, d as f32);
                        input.content(x as usize, y as usize) > 0.5
                    });
                    // a picture filling the key from its top is under the key's own cap; an icon
                    // that moved loses its snow
                    let s = top.map_or(-1.0, |d| if d < 6 { -1.0 } else { d as f32 });
                    if (s - drift.surface[i]).abs() > 1.5 {
                        drift.height[i] = 0.0;
                    }
                    drift.surface[i] = s;
                }
                // snow lies on tops at least 8 px wide: not in specks on a label's letters
                let mut run = vec![0usize; len];
                let mut start = 0;
                for i in 0..=len {
                    let joined = i < len && drift.surface[i] >= 0.0 && (i == start || (drift.surface[i] - drift.surface[i - 1]).abs() <= 2.0);
                    if !joined {
                        for r in run.iter_mut().take(i).skip(start) {
                            *r = i - start;
                        }
                        start = if i < len && drift.surface[i] >= 0.0 { i } else { i + 1 };
                    }
                }
                let cap = self.caps.get(k);
                #[allow(clippy::needless_range_loop)] // the index reads several arrays at once
                for i in 0..len {
                    if drift.surface[i] < 0.0 || run[i] < 8 {
                        drift.height[i] = 0.0;
                        continue;
                    }
                    // snow keeps to flat tops and slides off steep sides
                    let l = drift.surface[i.saturating_sub(1)];
                    let r = drift.surface[(i + 1).min(len - 1)];
                    let slope = if l < 0.0 || r < 0.0 { 3.0 } else { (r - l).abs() / 2.0 };
                    let stick = (1.0 - (slope - 0.6) / 1.8).clamp(0.0, 1.0);
                    // it keeps building for half a minute (owner, 2026-10-26: "Może się nawarstwiać
                    // bardziej"), never into the key's own cap above it
                    let room = drift.surface[i] - cap.and_then(|c| c.get(i)).copied().unwrap_or(0.0) - 2.0;
                    let want = (stick * (5.0 + 2.6 * noise(i as f32 / 5.0, k as f32 * 7.0, 64, 41))).min(room.max(0.0));
                    let grow = input.dt * 0.32 * (0.6 + noise(i as f32 / 4.0, input.t * 0.25 + k as f32, 64, 43));
                    drift.height[i] = (drift.height[i] + grow).min(want);
                }
                // a blanket, not spikes: no column more than a little above its neighbours
                for i in 1..len.saturating_sub(1) {
                    let m = (drift.height[i - 1] + drift.height[i + 1]) / 2.0 + 1.1;
                    drift.height[i] = drift.height[i].min(m);
                }
            }
        }
        for p in &mut self.falling {
            p.3 += 160.0 * input.dt;
            p.0 += gx * p.3 * input.dt + p.2.sin() * 0.3;
            p.1 += gy * p.3 * input.dt;
            p.2 -= input.dt * 1.1;
        }
        self.falling.retain(|p| p.2 > 0.0);
        true
    }

    fn over(&mut self, input: &Inputs, panel: &mut [u8]) {
        let white = [0.94, 0.97, 1.0];
        for f in self.flakes.iter().take(input.count(110, 440)) {
            // near flakes big and soft, far ones small and sharp
            let r = 1.2 + 3.6 * f.depth;
            let a = (0.45 + 0.5 * (1.0 - f.depth * 0.4)) * (0.7 + 0.3 * input.strength);
            glow(panel, f.x, f.y, r * 1.8, white, a * 0.35);
            glow(panel, f.x, f.y, r * 0.8, white, a);
        }
        let down = input.down;
        for (k, cap) in self.caps.iter().enumerate() {
            let Some(p) = input.keys.get(k) else { continue };
            for (i, h) in cap.iter().enumerate() {
                // the key's cap lies from its top edge down
                Self::lay(panel, p, down, i as f32, h.ceil() - 1.0, *h);
                // a glint here and there
                if noise(i as f32 * 0.7, input.t * 1.3 + k as f32 * 5.0, 256, 21) > 0.86 && *h > 2.0 {
                    let (x, y) = on_key(p, down, i as f32, h * 0.3);
                    glow(panel, x, y, 2.2, [1.0, 1.0, 1.0], 0.9);
                }
            }
            if let Some(drift) = self.drifts.get(k) {
                for (i, (&s, &h)) in drift.surface.iter().zip(&drift.height).enumerate() {
                    if s < 0.0 || h < 0.25 {
                        continue;
                    }
                    Self::lay(panel, p, down, i as f32, s - 1.0, h);
                    if noise(i as f32 * 0.9, input.t * 1.1 + k as f32 * 3.0, 256, 47) > 0.9 && h > 1.5 {
                        let (x, y) = on_key(p, down, i as f32, s - h);
                        glow(panel, x, y, 1.8, [1.0, 1.0, 1.0], 0.85);
                    }
                }
            }
        }
        for p in &self.falling {
            glow(panel, p.0, p.1, 2.4, white, p.2.min(1.0));
        }
    }

    fn press(&mut self, key: usize, input: &Inputs) {
        let Some(p) = input.keys.get(key) else { return };
        let down = input.down;
        if let Some(cap) = self.caps.get_mut(key) {
            for (i, h) in cap.iter_mut().enumerate().step_by(2) {
                if *h > 1.0 {
                    let (x, y) = on_key(p, down, i as f32, *h);
                    self.falling.push((x + self.rng.range(-1.5, 1.5), y, 1.0, self.rng.range(0.0, 40.0)));
                }
            }
            cap.iter_mut().for_each(|h| *h = 0.0);
        }
        if let Some(drift) = self.drifts.get_mut(key) {
            for i in (0..drift.height.len()).step_by(2) {
                if drift.height[i] > 0.8 {
                    let (x, y) = on_key(p, down, i as f32, drift.surface[i] - drift.height[i]);
                    self.falling.push((x + self.rng.range(-1.0, 1.0), y, 0.8, self.rng.range(0.0, 30.0)));
                }
            }
            drift.height.iter_mut().for_each(|h| *h = 0.0);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fireflies, as on a summer evening (owner, 2026-10-26: the first ones were "chaotyczne"): few
// lights drifting slowly on smooth curving paths, low over the deck and rising a little; each is
// dark most of the time and flashes now and then (a quick rise, a slow fade), some twice; now and
// then a wave of flashes runs across the deck as real fireflies sync; near ones big and soft, far
// ones small and sharp; a flash lights the icons beside it; a press sends the near ones away.

struct Fly {
    x: f32,
    y: f32,
    heading: f32,
    speed: f32,
    depth: f32,
    seed: f32,
    /// Seconds to the next flash.
    next: f32,
    /// Seconds since this flash began (None while dark).
    flash: Option<f32>,
    double: bool,
    /// Pushed away by a press, decaying.
    push: (f32, f32),
}

pub struct Fireflies {
    rng: Rng,
    flies: Vec<Fly>,
    wave: f32,
}

impl Fireflies {
    pub fn new() -> Self {
        let mut rng = Rng(0x7f4a_7c15);
        let flies = (0..64)
            .map(|_| Fly {
                x: rng.range(20.0, W as f32 - 20.0),
                y: rng.range(H as f32 * 0.3, H as f32 - 15.0),
                heading: rng.range(0.0, std::f32::consts::TAU),
                speed: rng.range(7.0, 15.0),
                depth: rng.next(),
                seed: rng.range(0.0, 100.0),
                next: rng.range(0.2, 4.0),
                flash: None,
                double: rng.next() < 0.3,
                push: (0.0, 0.0),
            })
            .collect();
        Self { rng, flies, wave: 9.0 }
    }

    /// How bright a flash is `s` seconds in: up in a quarter second, down in two thirds; a double
    /// flash lights again after a short gap.
    fn bright(s: f32, double: bool) -> f32 {
        let one = |s: f32| {
            if s < 0.0 {
                0.0
            } else if s < 0.22 {
                (s / 0.22).powf(0.8)
            } else {
                (1.0 - (s - 0.22) / 0.65).max(0.0).powi(2)
            }
        };
        if double { one(s).max(one(s - 0.55) * 0.85) } else { one(s) }
    }
}

impl Effect for Fireflies {
    fn step(&mut self, input: &Inputs) -> bool {
        let up = (-input.down.0, -input.down.1);
        // a wave of flashes across the deck every 10-20 s
        self.wave -= input.dt;
        let wave = self.wave <= 0.0;
        if wave {
            self.wave = self.rng.range(10.0, 20.0);
        }
        let n = input.count(16, 64);
        for f in self.flies.iter_mut().take(n) {
            // a smooth wandering heading, slower near the ground of the deck
            let turn = noise(input.t * 0.3, f.seed, 256, 11) - 0.5;
            f.heading += turn * 2.4 * input.dt;
            // stay in the deck: turn back softly toward its middle near an edge
            let (cx, cy) = (W as f32 / 2.0 - f.x, H as f32 * 0.55 - f.y);
            let edge = ((f.x - 18.0).min(W as f32 - 18.0 - f.x).min(f.y - 18.0).min(H as f32 - 12.0 - f.y) / 30.0).clamp(0.0, 1.0);
            if edge < 1.0 {
                let want = cy.atan2(cx);
                let mut d = (want - f.heading + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
                d *= (1.0 - edge) * 2.0 * input.dt;
                f.heading += d;
            }
            let bob = (input.t * 1.1 + f.seed).sin() * 2.5;
            let v = f.speed * (0.7 + 0.6 * f.depth);
            f.x += (f.heading.cos() * v + up.0 * (1.5 + bob) + f.push.0) * input.dt;
            f.y += (f.heading.sin() * v + up.1 * (1.5 + bob) + f.push.1) * input.dt;
            f.x = f.x.clamp(4.0, W as f32 - 4.0);
            f.y = f.y.clamp(4.0, H as f32 - 4.0);
            f.push = (f.push.0 * (1.0 - 1.5 * input.dt), f.push.1 * (1.0 - 1.5 * input.dt));
            // flashes: alone now and then, or in the wave from left to right
            if wave {
                f.next = f.x / W as f32 * 1.4 + self.rng.range(0.0, 0.15);
            }
            match f.flash {
                Some(s) if s > 1.6 => {
                    f.flash = None;
                    f.next = self.rng.range(2.0, 6.0);
                    f.double = self.rng.next() < 0.3;
                }
                Some(s) => f.flash = Some(s + input.dt),
                None => {
                    f.next -= input.dt;
                    if f.next <= 0.0 {
                        f.flash = Some(0.0);
                    }
                }
            }
        }
        true
    }

    fn under(&mut self, input: &Inputs, panel: &mut [u8]) {
        let core = [1.0, 1.0, 0.82];
        for f in self.flies.iter().take(input.count(16, 64)) {
            let b = f.flash.map_or(0.0, |s| Self::bright(s, f.double));
            // a faint ember while dark, so the eye can follow one
            let a = (0.1 + 0.9 * b) * (0.6 + 0.4 * input.strength);
            let near = 0.7 + 0.9 * f.depth;
            glow(panel, f.x, f.y, 32.0 * near, input.tint, a * 0.26);
            glow(panel, f.x, f.y, 10.0 * near, input.tint, a * 0.7);
            glow(panel, f.x, f.y, 2.6 + 2.0 * f.depth, core, a);
        }
    }

    fn over(&mut self, input: &Inputs, panel: &mut [u8]) {
        // a flash lights the icons and labels beside it (they hide the glow drawn under them)
        for f in self.flies.iter().take(input.count(16, 64)) {
            let b = f.flash.map_or(0.0, |s| Self::bright(s, f.double));
            if b < 0.05 {
                continue;
            }
            let r = 34.0 * (0.7 + 0.9 * f.depth);
            light_content(panel, input, f.x, f.y, r, input.tint, b * 0.5 * (0.6 + 0.4 * input.strength));
        }
    }

    fn press(&mut self, key: usize, input: &Inputs) {
        let Some(k) = input.keys.get(key) else { return };
        let (cx, cy) = (k.x + k.w / 2.0, k.y + k.h / 2.0);
        for f in &mut self.flies {
            let (dx, dy) = (f.x - cx, f.y - cy);
            let d = (dx * dx + dy * dy).sqrt().max(1.0);
            if d < 130.0 {
                let kick = 70.0 * (1.0 - d / 130.0);
                f.push = (dx / d * kick, dy / d * kick);
                f.heading = dy.atan2(dx);
                // startled: they flash
                if f.flash.is_none() {
                    f.flash = Some(0.0);
                }
            }
        }
    }
}

/// Light added only where the keys' content is (it hides what is drawn under it): a soft round
/// light at (x, y) gone at `r`.
fn light_content(px: &mut [u8], input: &Inputs, x: f32, y: f32, r: f32, c: [f32; 3], a: f32) {
    if input.mask.is_empty() || x + r < 0.0 || y + r < 0.0 || x - r > W as f32 || y - r > H as f32 {
        return;
    }
    let (x0, x1) = ((x - r).floor().max(0.0) as usize, ((x + r).ceil() as usize).min(W - 1));
    let (y0, y1) = ((y - r).floor().max(0.0) as usize, ((y + r).ceil() as usize).min(H - 1));
    let r2 = r * r;
    for yy in y0..=y1 {
        for xx in x0..=x1 {
            let cover = input.mask[yy * W + xx];
            if cover == 0 {
                continue;
            }
            let d2 = (xx as f32 + 0.5 - x).powi(2) + (yy as f32 + 0.5 - y).powi(2);
            if d2 < r2 {
                let f = 1.0 - d2 / r2;
                add(px, (yy * W + xx) * 4, c, a * f * f * cover as f32 / 255.0);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Embers: a warm glow breathing along the bottom of the deck, and sparks rising from it in long
// glowing trails in front of the keys, yellow when hot, through orange to red as they cool; the
// glow also lights pictures and icons that cover it (a key with its own dark picture hid it:
// owner, 2026-10-26); a press throws a burst from the key.

struct Ember {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
    seed: f32,
}

pub struct Embers {
    rng: Rng,
    sparks: Vec<Ember>,
    rising: bool,
    /// The glow of the fire below, per pixel, as drawn under the content this frame.
    fire: Vec<f32>,
}

impl Embers {
    pub fn new(rising: bool) -> Self {
        Self { rng: Rng(0x1234_5679), sparks: Vec::new(), rising, fire: Vec::new() }
    }
    fn spawn(&mut self, x: f32, y: f32, up: (f32, f32), burst: bool) {
        let speed = if burst { self.rng.range(40.0, 110.0) } else { self.rng.range(30.0, 70.0) };
        let side = if burst { self.rng.range(-1.0, 1.0) } else { self.rng.range(-0.3, 0.3) };
        self.sparks.push(Ember { x, y, vx: up.0 * speed - up.1 * side * speed, vy: up.1 * speed + up.0 * side * speed, life: self.rng.range(0.85, 1.0), seed: self.rng.range(0.0, 100.0) });
    }
}

impl Effect for Embers {
    fn step(&mut self, input: &Inputs) -> bool {
        let up = (-input.down.0, -input.down.1);
        if self.rising {
            let rate = 26.0 * (0.5 + input.strength) * input.density;
            let n = (input.dt * rate) as usize + usize::from(self.rng.next() < (input.dt * rate).fract());
            for _ in 0..n {
                let along = self.rng.next();
                let (x, y) = if input.down.1.abs() >= input.down.0.abs() {
                    (along * W as f32, if input.down.1 > 0.0 { H as f32 + 2.0 } else { -2.0 })
                } else {
                    (if input.down.0 > 0.0 { W as f32 + 2.0 } else { -2.0 }, along * H as f32)
                };
                self.spawn(x, y, up, false);
            }
        }
        let (sx, sy) = input.side();
        for s in &mut self.sparks {
            let wob = noise(s.seed, input.t * 1.6, 256, 17) - 0.5;
            s.vx += (-up.1 * wob * 60.0 + sx * input.wind * 55.0) * input.dt;
            s.vy += (up.0 * wob * 60.0 + sy * input.wind * 55.0) * input.dt;
            // they slow as they rise
            s.vx *= 1.0 - 0.4 * input.dt;
            s.vy *= 1.0 - 0.4 * input.dt;
            s.x += s.vx * input.dt;
            s.y += s.vy * input.dt;
            s.life -= input.dt * 0.38;
        }
        self.sparks.retain(|s| s.life > 0.0 && s.x > -10.0 && s.y > -10.0 && s.x < W as f32 + 10.0 && s.y < H as f32 + 10.0);
        self.rising || !self.sparks.is_empty()
    }

    fn under(&mut self, input: &Inputs, panel: &mut [u8]) {
        if !self.rising {
            return;
        }
        // the glow of the fire below, breathing unevenly along the edge
        let (gx, gy) = input.down;
        self.fire.clear();
        self.fire.resize(W * H, 0.0);
        for y in 0..H {
            for x in 0..W {
                let low = ((x as f32 / W as f32 - 0.5) * gx + (y as f32 / H as f32 - 0.5) * gy + 0.5).clamp(0.0, 1.0);
                if low < 0.55 {
                    continue;
                }
                let along = if gy.abs() >= gx.abs() { x as f32 } else { y as f32 };
                let flame = noise(along / 40.0, input.t * 0.9, 256, 23) * 0.7 + noise(along / 13.0, input.t * 2.1, 256, 29) * 0.3;
                let a = ((low - 0.55) / 0.45).powi(2) * (0.35 + 0.5 * flame) * (0.6 + 0.4 * input.strength);
                self.fire[y * W + x] = a;
                add(panel, (y * W + x) * 4, input.tint, a * 0.8);
            }
        }
    }

    fn over(&mut self, input: &Inputs, panel: &mut [u8]) {
        // the fire's light on the content that covers it: a picture with its own dark background
        // glows warm at its bottom too, a little less than the open glass
        if self.rising && self.fire.len() == W * H && input.mask.len() == W * H {
            for i in 0..W * H {
                let (f, m) = (self.fire[i], input.mask[i]);
                if f > 0.0 && m > 0 {
                    add(panel, i * 4, input.tint, f * 0.55 * m as f32 / 255.0);
                }
            }
        }
        // the sparks fly in front of the keys
        for s in &self.sparks {
            let flick = 0.8 + 0.2 * (s.seed * 13.0 + input.t * 23.0).sin();
            let heat = s.life.clamp(0.0, 1.0);
            let c = [1.0, 0.3 + 0.62 * heat, 0.05 + 0.5 * heat * heat];
            let tail = (s.x - s.vx * 0.08, s.y - s.vy * 0.08);
            glow(panel, s.x, s.y, 7.0, input.tint, heat * 0.22 * flick);
            streak(panel, tail, (s.x, s.y), 1.4, c, heat * flick);
            glow(panel, s.x, s.y, 1.9, [1.0, 0.95, 0.8], heat * heat * flick);
        }
    }

    fn press(&mut self, key: usize, input: &Inputs) {
        let Some(k) = input.keys.get(key) else { return };
        let up = (-input.down.0, -input.down.1);
        for _ in 0..input.count(34, 136) {
            let (x, y) = (k.x + self.rng.range(0.15, 0.85) * k.w, k.y + self.rng.range(0.2, 0.9) * k.h);
            self.spawn(x, y, up, true);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fog (owner, 2026-10-26: the first one was hard to see and the keys seemed to tremble in it):
// big soft banks of mist, computed at a quarter of the panel's size from slow, low noise and
// smoothly enlarged, so nothing fine crawls between frames; it rolls in one direction, thick and
// lit toward the bottom with a brighter upper edge on each bank, in two colours; behind the keys'
// content, and over it only as a light veil in the thickest parts.

pub struct Fog {
    field: Vec<f32>,
    tone: Vec<f32>,
    /// How thick it may get by where a cell is on the screen (thicker toward its bottom).
    lowk: Vec<f32>,
    /// This frame's thickness of every pixel, made once in `under` and read again in `over`.
    dens: Vec<f32>,
}

const FW: usize = W / 4 + 2;
const FH: usize = H / 4 + 2;

impl Fog {
    pub fn new() -> Self {
        Self { field: vec![0.0; FW * FH], tone: vec![0.0; FW * FH], lowk: vec![0.0; FW * FH], dens: vec![0.0; W * H] }
    }
    /// A row of one of the fields between its cells, linearly (smooth enough for a field this
    /// soft, and with no flat steps at the cells' edges): `out[x]` for every x of panel row `y`.
    fn row(v: &[f32], y: usize, cols: &[(usize, f32)], out: &mut [f32]) {
        let fy = y as f32 / 4.0;
        let yi = (fy as usize).min(FH - 2);
        let w = (fy - yi as f32).clamp(0.0, 1.0);
        let (top, bottom) = (&v[yi * FW..(yi + 1) * FW], &v[(yi + 1) * FW..(yi + 2) * FW]);
        for (o, &(xi, u)) in out.iter_mut().zip(cols) {
            let t = top[xi] + (top[xi + 1] - top[xi]) * u;
            let b = bottom[xi] + (bottom[xi + 1] - bottom[xi]) * u;
            *o = t + (b - t) * w;
        }
    }
}

impl Effect for Fog {
    fn step(&mut self, input: &Inputs) -> bool {
        let t = input.t;
        let (gx, gy) = input.down;
        for cy in 0..FH {
            for cx in 0..FW {
                let (x, y) = (cx as f32 * 4.0, cy as f32 * 4.0);
                // the banks roll sideways at ~7 px a second (faster, or the other way, with the
                // wind) and slowly change shape
                let drift = 0.075 + input.wind * 0.22;
                let (u, v) = (x / 95.0 - t * drift, y / 60.0);
                let warp = noise(u * 0.7 + 3.1, v * 0.7 + t * 0.03, 256, 61) - 0.5;
                let a = noise(u + warp * 0.9, v + t * 0.02, 256, 31);
                let b = noise(u * 2.1 + 9.0 - t * 0.04, v * 2.1 + warp, 256, 37);
                // the raw density, cut at its threshold per pixel after smoothing (a cut per cell
                // left stair steps of 4 px round the banks)
                self.field[cy * FW + cx] = a * 0.72 + b * 0.38;
                self.tone[cy * FW + cx] = (b - a + 0.5).clamp(0.0, 1.0);
                // across the screen's own "down": 1 at its bottom; never quite gone at the top
                let low = ((x / W as f32 - 0.5) * gx + (y / H as f32 - 0.5) * gy + 0.5).clamp(0.0, 1.0);
                self.lowk[cy * FW + cx] = 0.35 + 1.15 * low.powf(1.3);
            }
        }
        true
    }

    fn under(&mut self, input: &Inputs, panel: &mut [u8]) {
        let (gx, gy) = input.down;
        // a second colour: the tint turned toward a cold green (Halloween's purple and green)
        let second = [input.tint[0] * 0.35 + 0.12, input.tint[1] * 0.3 + 0.42, input.tint[2] * 0.3 + 0.2];
        let s = 0.75 + 0.5 * input.strength;
        // more of it: a lower threshold (thicker, wider banks); less: a higher one
        let shift = input.density.log2() * 0.07;
        let cols: Vec<(usize, f32)> = (0..W)
            .map(|x| {
                let fx = x as f32 / 4.0;
                let xi = (fx as usize).min(FW - 2);
                (xi, (fx - xi as f32).clamp(0.0, 1.0))
            })
            .collect();
        let (mut raw, mut low, mut tone) = (vec![0f32; W], vec![0f32; W], vec![0f32; W]);
        // every pixel's thickness first (the lit edge reads it a few pixels "down")
        for y in 0..H {
            Self::row(&self.field, y, &cols, &mut raw);
            Self::row(&self.lowk, y, &cols, &mut low);
            for x in 0..W {
                // a soft knee instead of a hard cut, so a bank's edge fades over a few pixels
                let d = ((raw[x] - 0.42 + shift) * 1.9).max(0.0);
                self.dens[y * W + x] = (d * d / (d + 0.06) * low[x]).min(1.0);
            }
        }
        let (ox, oy) = ((gx * 6.0).round() as isize, (gy * 6.0).round() as isize);
        for y in 0..H {
            Self::row(&self.tone, y, &cols, &mut tone);
            #[allow(clippy::needless_range_loop)] // the index reads several arrays at once
            for x in 0..W {
                let d = self.dens[y * W + x];
                if d < 0.01 {
                    continue;
                }
                // a bank's upper edge catches light: brighter where the fog thickens downward
                let (bx, by) = ((x as isize + ox).clamp(0, W as isize - 1) as usize, (y as isize + oy).clamp(0, H as isize - 1) as usize);
                let lit = (self.dens[by * W + bx] - d).max(0.0) * 1.6;
                let m = tone[x];
                let c = [input.tint[0] * (1.0 - m) + second[0] * m, input.tint[1] * (1.0 - m) + second[1] * m, input.tint[2] * (1.0 - m) + second[2] * m];
                let i = (y * W + x) * 4;
                add(panel, i, c, (d * 0.62 * s).min(0.62));
                if lit > 0.01 {
                    add(panel, i, [0.85, 0.85, 0.95], (lit * 0.35 * s).min(0.3));
                }
            }
        }
    }

    fn over(&mut self, input: &Inputs, panel: &mut [u8]) {
        // a light veil over the content only where the fog is thickest: the icons sit in it,
        // labels stay readable (at most an eighth of the fog's colour over them)
        if input.mask.len() != W * H {
            return;
        }
        let s = 0.75 + 0.5 * input.strength;
        let veil_c = [input.tint[0] * 0.6 + 0.3, input.tint[1] * 0.6 + 0.3, input.tint[2] * 0.6 + 0.3];
        for (i, (&m, &d)) in input.mask.iter().zip(&self.dens).enumerate() {
            if m == 0 {
                continue;
            }
            let veil = ((d - 0.45) * 0.3 * s).clamp(0.0, 0.13);
            if veil > 0.005 {
                blend(panel, i * 4, veil_c, veil * m as f32 / 255.0);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Bats (owner, 2026-10-26: the first one looked "tanio", without real behaviour or variety): a
// bat drawn as one: membrane wings between finger bones with a scalloped trailing edge, a furry
// body, ears and two glints of eyes, a rim of the tint so it reads on black; its wings beat down
// fully spread and come up half folded, and now and then it glides. They come as one bat on an
// erratic path (sudden turns, flutter), as a small colony streaming across, or as one that flies
// to a key and hangs from its top edge upside down, wings wrapped, swaying, stretching a wing,
// until a press of that key or its own time sends it off. Near ones bigger and sharper.

#[derive(Clone, Copy, PartialEq)]
enum BatMode {
    Flying,
    /// Flying to hang from a key's top edge.
    Landing(usize),
    /// Hanging from a key's top edge, seconds left.
    Hanging(usize, f32),
}

struct Bat {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    size: f32,
    phase: f32,
    rate: f32,
    seed: f32,
    path: Vec<(f32, f32)>,
    mode: BatMode,
    glide: f32,
    /// Seconds before it starts (colonies stream in one after another).
    delay: f32,
    stretch: f32,
}

pub struct Bats {
    rng: Rng,
    bats: Vec<Bat>,
    wait: f32,
}

impl Bats {
    pub fn new() -> Self {
        Self { rng: Rng(0x0bad_ba75), bats: Vec::new(), wait: 1.5 }
    }

    /// A path across the deck: in from one side, through a few points, out on the other.
    fn path(&mut self, from_left: bool, band: (f32, f32)) -> Vec<(f32, f32)> {
        let n = 2 + (self.rng.next() * 3.0) as usize;
        let mut pts: Vec<(f32, f32)> = (0..n)
            .map(|i| {
                let along = (i as f32 + 0.5 + self.rng.range(-0.3, 0.3)) / n as f32;
                let x = if from_left { along * W as f32 } else { (1.0 - along) * W as f32 };
                (x, self.rng.range(band.0, band.1))
            })
            .collect();
        pts.push((if from_left { W as f32 + 70.0 } else { -70.0 }, self.rng.range(band.0, band.1)));
        pts
    }

    fn spawn(&mut self, keys: &[KeyPlace]) {
        let from_left = self.rng.next() < 0.5;
        let start_x = if from_left { -50.0 } else { W as f32 + 50.0 };
        let roll = self.rng.next();
        if roll < 0.28 {
            // a colony: four to seven small bats streaming along one path, each a little off it
            let path = self.path(from_left, (30.0, H as f32 - 40.0));
            let n = 4 + (self.rng.next() * 4.0) as usize;
            for k in 0..n {
                let off = (self.rng.range(-18.0, 18.0), self.rng.range(-14.0, 14.0));
                let p = path.iter().map(|&(x, y)| (x + off.0, y + off.1)).collect();
                let size = self.rng.range(0.9, 1.4);
                let delay = k as f32 * self.rng.range(0.12, 0.3);
                let bat = self.bat(start_x, path[0].1 + off.1, size, p, BatMode::Flying, delay);
                self.bats.push(bat);
            }
        } else if roll < 0.55 && !keys.is_empty() {
            // one flies to a key of the top row and hangs from it
            let k = (self.rng.next() * 5.0) as usize % keys.len().clamp(1, 5);
            let p = keys[k];
            let target = (p.x + p.w * self.rng.range(0.3, 0.7), p.y + 1.0);
            let size = self.rng.range(1.7, 2.2);
            let path = vec![(W as f32 / 2.0 + self.rng.range(-80.0, 80.0), self.rng.range(60.0, 140.0)), target];
            let y = self.rng.range(40.0, 120.0);
            let bat = self.bat(start_x, y, size, path, BatMode::Landing(k), 0.0);
            self.bats.push(bat);
        } else {
            let size = self.rng.range(1.6, 2.6);
            let path = self.path(from_left, (25.0, H as f32 - 35.0));
            let y = path[0].1;
            let bat = self.bat(start_x, y, size, path, BatMode::Flying, 0.0);
            self.bats.push(bat);
        }
    }

    fn bat(&mut self, x: f32, y: f32, size: f32, path: Vec<(f32, f32)>, mode: BatMode, delay: f32) -> Bat {
        Bat {
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            size,
            phase: self.rng.range(0.0, std::f32::consts::TAU),
            rate: self.rng.range(4.2, 5.6) / size.sqrt(),
            seed: self.rng.range(0.0, 100.0),
            path,
            mode,
            glide: 0.0,
            delay,
            stretch: 0.0,
        }
    }

    /// A flying bat's outline in its own units (body at 0,0; y down), wings at `up` (-1 down,
    /// 1 up) and spread `span` (1 = open).
    fn flying(pb: &mut tiny_skia::PathBuilder, up: f32, span: f32) {
        for side in [-1.0f32, 1.0] {
            let p = |x: f32, y: f32| (x * side, y);
            let shoulder = p(1.0, -1.2);
            let wrist = p(4.4 * span, -1.4 - 3.8 * up);
            let tip1 = p(11.0 * span, -0.4 - 6.4 * up);
            let tip2 = p(9.4 * span, 2.8 - 3.4 * up);
            let tip3 = p(6.6 * span, 4.6 - 1.4 * up);
            let foot = p(1.4, 3.6);
            let pull = |a: (f32, f32), b: (f32, f32), t: (f32, f32), k: f32| ((a.0 + b.0) / 2.0 * (1.0 - k) + t.0 * k, (a.1 + b.1) / 2.0 * (1.0 - k) + t.1 * k);
            pb.move_to(shoulder.0, shoulder.1);
            let arm = p(2.4 * span, -2.6 - 2.6 * up);
            pb.quad_to(arm.0, arm.1, wrist.0, wrist.1);
            pb.line_to(tip1.0, tip1.1);
            // the trailing edge: scallops between the finger tips, pulled in toward the wrist
            let c = pull(tip1, tip2, wrist, 0.38);
            pb.quad_to(c.0, c.1, tip2.0, tip2.1);
            let c = pull(tip2, tip3, wrist, 0.38);
            pb.quad_to(c.0, c.1, tip3.0, tip3.1);
            let c = pull(tip3, foot, p(2.6, 1.0), 0.45);
            pb.quad_to(c.0, c.1, foot.0, foot.1);
            pb.close();
            // ears
            let (a, b, e) = (p(0.3, -2.5), p(1.15, -4.7), p(1.35, -2.0));
            pb.move_to(a.0, a.1);
            pb.line_to(b.0, b.1);
            pb.line_to(e.0, e.1);
            pb.close();
        }
        if let Some(r) = tiny_skia::Rect::from_xywh(-1.6, -1.8, 3.2, 5.8) {
            pb.push_oval(r);
        }
        if let Some(r) = tiny_skia::Rect::from_xywh(-1.4, -3.3, 2.8, 2.7) {
            pb.push_oval(r);
        }
    }

    /// The bones of a flying bat's wings (drawn as faint lines of the rim's light).
    fn bones(pb: &mut tiny_skia::PathBuilder, up: f32, span: f32) {
        for side in [-1.0f32, 1.0] {
            let p = |x: f32, y: f32| (x * side, y);
            let wrist = p(4.4 * span, -1.4 - 3.8 * up);
            for tip in [p(9.4 * span, 2.8 - 3.4 * up), p(6.6 * span, 4.6 - 1.4 * up)] {
                pb.move_to(wrist.0, wrist.1);
                pb.line_to(tip.0, tip.1);
            }
        }
    }

    /// A hanging bat (head down from its feet at 0,0), wings wrapped; `stretch` 0..1 opens one.
    fn hanging(pb: &mut tiny_skia::PathBuilder, stretch: f32) {
        // feet gripping the edge
        if let Some(r) = tiny_skia::Rect::from_xywh(-1.0, -0.4, 2.0, 1.4) {
            pb.push_oval(r);
        }
        // the wrapped body, wider at the shoulders below
        pb.move_to(-0.9, 0.6);
        pb.cubic_to(-2.9, 2.0, -3.1, 6.4, -1.6, 8.0);
        pb.line_to(1.6, 8.0);
        pb.cubic_to(3.1, 6.4, 2.9, 2.0, 0.9, 0.6);
        pb.close();
        // the head and its ears pointing down
        if let Some(r) = tiny_skia::Rect::from_xywh(-1.45, 7.2, 2.9, 2.7) {
            pb.push_oval(r);
        }
        for side in [-1.0f32, 1.0] {
            pb.move_to(0.4 * side, 9.4);
            pb.line_to(1.2 * side, 11.4);
            pb.line_to(1.4 * side, 8.7);
            pb.close();
        }
        if stretch > 0.0 {
            // one wing opens to the side, upside down
            let s = stretch;
            pb.move_to(1.6, 2.0);
            pb.quad_to(4.0 * s + 2.0, -0.5, 8.5 * s + 1.5, 1.0 * s + 1.0);
            pb.quad_to(6.5 * s + 1.5, 3.2, 7.4 * s + 1.0, 5.0);
            pb.quad_to(4.5 * s + 1.0, 5.2, 5.0 * s + 1.0, 7.4);
            pb.quad_to(3.0, 6.6, 1.8, 7.0);
            pb.close();
        }
    }
}

impl Effect for Bats {
    fn step(&mut self, input: &Inputs) -> bool {
        let dt = input.dt;
        for b in &mut self.bats {
            if b.delay > 0.0 {
                b.delay -= dt;
                continue;
            }
            b.stretch = (b.stretch - dt * 1.2).max(0.0);
            if let BatMode::Hanging(k, left) = b.mode {
                let left = left - dt;
                // now and then it stretches a wing
                if self.rng.next() < 0.18 * dt {
                    b.stretch = 1.6;
                }
                if left <= 0.0 {
                    b.mode = BatMode::Flying;
                    let from_left = self.rng.next() < 0.5;
                    b.path = vec![(b.x + if from_left { 60.0 } else { -60.0 }, b.y + 40.0), (if from_left { W as f32 + 70.0 } else { -70.0 }, self.rng.range(30.0, H as f32 - 40.0))];
                    b.vy = 40.0;
                } else {
                    b.mode = BatMode::Hanging(k, left);
                }
                continue;
            }
            // fly toward the next point of the path, turning hard, with a flutter on top
            let Some(&(tx, ty)) = b.path.first() else { continue };
            let (dx, dy) = (tx - b.x, ty - b.y);
            let d = (dx * dx + dy * dy).sqrt().max(0.1);
            let landing = matches!(b.mode, BatMode::Landing(_));
            let cruise = if landing && b.path.len() == 1 { (d * 2.2).clamp(25.0, 140.0) } else { 150.0 / b.size.sqrt() + 40.0 };
            let steer = if landing { 6.0 } else { 3.2 };
            b.vx += (dx / d * cruise - b.vx) * steer * dt;
            b.vy += (dy / d * cruise - b.vy) * steer * dt;
            let flutter = noise(input.t * 3.0, b.seed, 256, 71) - 0.5;
            b.vy += flutter * 260.0 * dt;
            b.vx += (noise(b.seed, input.t * 2.3, 256, 73) - 0.5) * 120.0 * dt;
            b.x += b.vx * dt;
            b.y += b.vy * dt;
            if d < 14.0 {
                if landing && b.path.len() == 1 {
                    if let BatMode::Landing(k) = b.mode {
                        b.x = tx;
                        b.y = ty;
                        b.vx = 0.0;
                        b.vy = 0.0;
                        b.mode = BatMode::Hanging(k, self.rng.range(5.0, 10.0));
                    }
                } else {
                    b.path.remove(0);
                }
            }
            // a glide now and then, wings held open
            if b.glide > 0.0 {
                b.glide -= dt;
            } else {
                b.phase += b.rate * std::f32::consts::TAU * dt;
                if self.rng.next() < 0.25 * dt {
                    b.glide = self.rng.range(0.25, 0.5);
                }
            }
        }
        // gone when its path is flown (it ends off the deck) or it strays far off
        self.bats.retain(|b| (b.delay > 0.0 || !b.path.is_empty() || matches!(b.mode, BatMode::Hanging(..))) && b.x > -150.0 && b.x < W as f32 + 150.0 && b.y > -150.0 && b.y < H as f32 + 150.0);
        self.wait -= dt;
        if self.wait <= 0.0 {
            self.spawn(input.keys);
            self.wait = self.rng.range(6.0, 13.0) / input.density;
        }
        true
    }

    fn over(&mut self, input: &Inputs, panel: &mut [u8]) {
        if self.bats.is_empty() {
            return;
        }
        let Some(mut pixmap) = tiny_skia::PixmapMut::from_bytes(panel, PANEL_W, PANEL_H) else { return };
        let c = input.tint;
        let tint = |a: u8| {
            let mut p = tiny_skia::Paint::default();
            p.set_color_rgba8((c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8, a);
            p.anti_alias = true;
            p
        };
        let mut body = tiny_skia::Paint::default();
        body.set_color_rgba8(9, 6, 13, 248);
        body.anti_alias = true;
        let mut eye = tiny_skia::Paint::default();
        eye.set_color_rgba8(255, 214, 150, 230);
        eye.anti_alias = true;
        for b in &self.bats {
            if b.delay > 0.0 {
                continue;
            }
            let s = b.size;
            let mut pb = tiny_skia::PathBuilder::new();
            let mut bones = tiny_skia::PathBuilder::new();
            let (transform, eyes) = match b.mode {
                BatMode::Hanging(..) => {
                    let sway = (input.t * 1.3 + b.seed).sin() * 5.0;
                    Self::hanging(&mut pb, b.stretch.min(1.0));
                    (tiny_skia::Transform::from_scale(s, s).post_rotate(sway).post_translate(b.x, b.y), [(-0.55, 8.3), (0.55, 8.3)])
                }
                _ => {
                    let (up, span) = if b.glide > 0.0 {
                        (0.15, 1.0)
                    } else {
                        // down fully spread, up half folded
                        let up = b.phase.sin();
                        (up, if b.phase.cos() < 0.0 { 1.0 } else { 1.0 - 0.38 * b.phase.cos() })
                    };
                    Self::flying(&mut pb, up, span);
                    Self::bones(&mut bones, up, span);
                    let bank = (b.vy / 160.0).clamp(-0.5, 0.5) * b.vx.signum() * 20.0;
                    (tiny_skia::Transform::from_scale(s, s).post_rotate(bank).post_translate(b.x, b.y), [(-0.55, -2.0), (0.55, -2.0)])
                }
            };
            let Some(path) = pb.finish() else { continue };
            pixmap.stroke_path(&path, &tint(150), &tiny_skia::Stroke { width: 1.5 / s, ..Default::default() }, transform, None);
            pixmap.fill_path(&path, &body, tiny_skia::FillRule::Winding, transform, None);
            if let Some(bones) = bones.finish() {
                pixmap.stroke_path(&bones, &tint(70), &tiny_skia::Stroke { width: 0.7 / s, ..Default::default() }, transform, None);
            }
            // two glints of eyes (they blink while it hangs)
            let blink = matches!(b.mode, BatMode::Hanging(..)) && (input.t * 0.7 + b.seed).fract() < 0.06;
            if !blink && s > 1.3 {
                let mut eb = tiny_skia::PathBuilder::new();
                for (ex, ey) in eyes {
                    eb.push_circle(ex, ey, 0.32);
                }
                if let Some(e) = eb.finish() {
                    pixmap.fill_path(&e, &eye, tiny_skia::FillRule::Winding, transform, None);
                }
            }
        }
    }

    fn press(&mut self, key: usize, _input: &Inputs) {
        // a hanging bat is startled off its key
        for b in &mut self.bats {
            if let BatMode::Hanging(k, _) = b.mode
                && k == key
            {
                b.mode = BatMode::Hanging(k, 0.0);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------

/// One layer's options as the effects use them ([`Inputs`]).
#[derive(Clone, Copy)]
struct Opts {
    tint: [f32; 3],
    strength: f32,
    speed: f32,
    density: f32,
    wind: f32,
    /// It answers presses (`Layer::react`).
    react: bool,
    /// It wanders (`Layer::moving`).
    moving: bool,
    /// It keeps to the left of the keys (`Layer::side`).
    left: bool,
}

impl Opts {
    fn of(l: &super::profile::Layer) -> Self {
        let factor = |v: Option<u8>| 0.25 * 16f32.powf(v.map_or(50, |v| v.min(100)) as f32 / 100.0);
        Opts {
            tint: l.tint.as_deref().and_then(rgb).or_else(|| rgb(default_tint(base_kind(&l.kind)))).unwrap_or([0.6, 0.8, 1.0]),
            strength: l.strength.map_or(0.6, |s| s.min(100) as f32 / 100.0),
            speed: factor(l.speed),
            density: factor(l.density),
            wind: l.wind.map_or(0.0, |w| w.clamp(-100, 100) as f32 / 100.0),
            react: l.react != Some(false),
            moving: l.moving == Some(true),
            left: l.side.as_deref() == Some("left"),
        }
    }
}

/// Each kind's own colour (`engine.ts` `FX_TINT`).
fn default_tint(kind: &str) -> &'static str {
    match kind {
        "fog" => "#7a4fc0",
        "embers" | "burst" => "#ff7a1a",
        "fireflies" => "#c8f060",
        "light" => "#fff1d6",
        "scene" => "#3a4a8a",
        "finish" => "#ffb070",
        "aurora" => "#9a60ff",
        "aquarium" => "#2a8fb0",
        "lava" => "#ff5030",
        "fire" => "#ff7a1a",
        "frames" => "#ffe2b0",
        "liquid" => "#38b6ff",
        "heart" => "#ff4f8b",
        "hop" => "#ffd36b",
        "mist" => "#9fb4ff",
        "pumpkins" => "#ff8a1a",
        "eyes" => "#d8ff40",
        "ghost" | "ghostrise" => "#bcd8ff",
        "spider" | "webs" => "#e0e0ee",
        "batburst" => "#7a4fc0",
        "chain" => "#b48cff",
        "candle" | "carved" => "#ff8a2a",
        "slime" | "spectral" => "#6cff8a",
        "haunt" | "stone" => "#ffffff",
        "firework" | "domino" | "shockwave" | "magnet" | "bloom" => "#ffffff",
        "neonrain" => "#ff3cac",
        "leaves" | "goldlight" => "#ffb43c",
        "storks" => "#ffffff",
        "sakura" => "#ffb7cf",
        "lanterns" => "#ff6a3a",
        "ukiyo" => "#f4ecd2",
        "koi" => "#ff7a2a",
        "mushrooms" => "#c87a3a",
        "woodland" => "#ffd27a",
        "matrix" => "#3cff7a",
        "code" => "#7fb4ff",
        "synthwave" => "#ff3cd2",
        "ink" => "#4a7aff",
        "nebula" => "#8f7aff",
        "spectrum" => "#3cff9a",
        "circuit" => "#3ce8ff",
        "orbit" => "#6aa8ff",
        "vinyl" => "#ff4a6a",
        "impasto" => "#f2c440",
        "hollowhill" => "#ff8a2a",
        "lanternhollow" => "#ffa03a",
        "tokyorain" => "#ff3c9a",
        "fuji" => "#ff5a3a",
        "caustics" => "#7fe8ff",
        "holo" | "glass" | "frost" | "cover" | "springs" | "flip" | "relief" | "gold" | "chrome" | "crystal" => "#ffffff",
        _ => "#8fd8ff",
    }
}

/// The layer kinds the canvas knows (`Layer::kind`); those in [`GPU_KINDS`] run on the graphics card.
pub const KINDS: &[&str] = &[
    "fog",
    "rain",
    "snow",
    "fireflies",
    "embers",
    "ripple",
    "burst",
    "bats",
    "water",
    "light",
    "scene",
    "finish",
    "aurora",
    "aquarium",
    "cover",
    "lava",
    "fire",
    "glass",
    "frames",
    "liquid",
    "frost",
    "springs",
    "heart",
    "flip",
    "hop",
    "holo",
    "relief",
    "gold",
    "chrome",
    "crystal",
    "mist",
    "neonrain",
    "caustics",
    "pumpkins",
    "eyes",
    "ghost",
    "spider",
    "webs",
    "batburst",
    "ghostrise",
    "domino",
    "chain",
    "firework",
    "shockwave",
    "magnet",
    "bloom",
    "haunt",
    "candle",
    "carved",
    "stone",
    "slime",
    "spectral",
    "leaves",
    "storks",
    "goldlight",
    "sakura",
    "lanterns",
    "ukiyo",
    "koi",
    "mushrooms",
    "woodland",
    "fuji",
    "matrix",
    "code",
    "synthwave",
    "ink",
    "nebula",
    "spectrum",
    "circuit",
    "orbit",
    "vinyl",
    "impasto",
    "hollowhill",
    "lanternhollow",
    "tokyorain",
    "goldlight-up",
    "mist-up",
    "mushrooms-up",
    "leaves-up",
    "woodland-up",
    "fuji-up",
    "sakura-up",
    "koi-up",
    "matrix-up",
    "synthwave-up",
    "circuit-up",
    "nebula-up",
    "spectrum-up",
];
/// The kinds that need the graphics card (`gpu.rs`).
pub const GPU_KINDS: &[&str] = &[
    "water",
    "light",
    "scene",
    "finish",
    "aurora",
    "aquarium",
    "cover",
    "lava",
    "fire",
    "glass",
    "frames",
    "liquid",
    "frost",
    "springs",
    "heart",
    "flip",
    "hop",
    "holo",
    "relief",
    "gold",
    "chrome",
    "crystal",
    "mist",
    "neonrain",
    "caustics",
    "pumpkins",
    "eyes",
    "ghost",
    "spider",
    "webs",
    "batburst",
    "ghostrise",
    "domino",
    "chain",
    "firework",
    "shockwave",
    "magnet",
    "bloom",
    "haunt",
    "candle",
    "carved",
    "stone",
    "slime",
    "spectral",
    "leaves",
    "storks",
    "goldlight",
    "sakura",
    "lanterns",
    "ukiyo",
    "koi",
    "mushrooms",
    "woodland",
    "fuji",
    "matrix",
    "code",
    "synthwave",
    "ink",
    "nebula",
    "spectrum",
    "circuit",
    "orbit",
    "vinyl",
    "impasto",
    "hollowhill",
    "lanternhollow",
    "tokyorain",
    "goldlight-up",
    "mist-up",
    "mushrooms-up",
    "leaves-up",
    "woodland-up",
    "fuji-up",
    "sakura-up",
    "koi-up",
    "matrix-up",
    "synthwave-up",
    "circuit-up",
    "nebula-up",
    "spectrum-up",
];
/// The scenes with an upright copy, `<kind>-up`: the same layer (its state, its answer to a press,
/// its place in the stack) drawn from `shaders/upright/<kind>.wgsl`, composed for the deck standing
/// upright as well as lying (owner, 2026-10-29: "zrób ich kopie a nie przerabiaj na oryginałach").
/// Landscape they draw what the original draws, but for those in [`REDRAWN`].
pub const UPRIGHT: &[&str] = &["goldlight", "mist", "mushrooms", "leaves", "woodland", "fuji", "sakura", "koi", "matrix", "synthwave", "circuit", "nebula", "spectrum"];

/// The upright copies drawn anew as one picture in the world square, lying as well as upright (owner,
/// 2026-10-29: Fuji laid out on the key windows; "nie będziemy przerabiać pozostałych"; the koi pond
/// redone "bez kamieni"): lying
/// they no longer draw their original.
#[cfg(test)]
const REDRAWN: &[&str] = &["fuji", "koi", "woodland"];

/// The kind a layer behaves as: an upright copy as its original.
pub fn base_kind(kind: &str) -> &str {
    kind.strip_suffix("-up").filter(|k| UPRIGHT.contains(k)).unwrap_or(kind)
}

/// The kinds that are themselves an answer to a press: their reaction is never switched off.
const PRESS_KINDS: &[&str] = &["ripple", "burst", "water", "springs", "heart", "flip", "batburst", "ghostrise", "domino", "chain", "firework", "shockwave", "magnet", "bloom"];

/// The 4-neighbours of key `k` on the 5x3 deck.
fn neighbours(k: usize) -> impl Iterator<Item = usize> {
    let (c, r) = ((k % 5) as i32, (k / 5) as i32);
    [(c - 1, r), (c + 1, r), (c, r - 1), (c, r + 1)].into_iter().filter(|(c, r)| (0..5).contains(c) && (0..3).contains(r)).map(|(c, r)| (r * 5 + c) as usize)
}

/// What a shader layer keeps on the CPU between frames (springs, a ball's hops...), handed to its
/// shader in `aux` each frame.
enum Cpu {
    None,
    /// K4: each key's depth and speed on its spring (+ sunk).
    Springs {
        d: [f32; 15],
        v: [f32; 15],
    },
    /// K6: each jar's level, its surface's tilt and tilt speed, how much it sloshes.
    Liquid {
        level: [f32; 15],
        tilt: [f32; 15],
        spin: [f32; 15],
        slosh: [f32; 15],
    },
    /// Seconds (its clock) when each key was last pressed.
    Frost {
        pressed: [f32; 15],
    },
    /// Likes: key centre and start, newest last.
    Heart {
        likes: Vec<(f32, f32, f32)>,
    },
    /// A page turn under way: its start.
    Flip {
        start: Option<f32>,
    },
    /// K5: the ball's hop from a key to a key, from its start, and a key it is called to.
    Hop {
        from: usize,
        to: usize,
        start: f32,
        dur: f32,
        rest: f32,
        called: Option<usize>,
        seed: u32,
        at: (f32, f32),
    },
    /// The cover shown (its version).
    Cover {
        version: Option<u64>,
    },
    /// B5: which material, and the content mask's fingerprint the keys filled by a picture were
    /// found for (those keep their picture).
    Material {
        index: f32,
        photos_for: Option<u64>,
    },
    /// Halloween's jack-o'-lanterns: where they sit (found when the page changes) and when each
    /// key was last pressed (its pumpkin flares).
    Pumpkins {
        places_for: Option<u64>,
        places: [[f32; 2]; 15],
        pressed: [f32; 15],
    },
    /// Eyes in the dark: four pairs, the free places they may open in, and the press they look at.
    Eyes {
        pairs: [Pair; 4],
        seed: u32,
        look: Option<(f32, f32, f32)>,
        spots_for: Option<u64>,
        spots: Vec<(usize, f32, f32)>,
    },
    /// The spider on its thread: what it does, in which key, how far down it is and wants to go.
    Spider {
        state: Spin,
        key: usize,
        len: f32,
        want: f32,
        seed: u32,
        called: bool,
    },
    /// Cobwebs: the content mask they were placed for.
    Webs {
        places_for: Option<u64>,
    },
    /// Answers to a press that travel (bats, a ghost): the key's centre and the start, newest last.
    Bursts {
        list: Vec<(f32, f32, f32)>,
    },
    /// The music studio: the last presses as for bursts; the spectrum is read each frame.
    Music {
        list: Vec<(f32, f32, f32)>,
    },
    /// Chain lightning: when it struck and the keys it leaps through, from the pressed one.
    Chain {
        start: Option<f32>,
        keys: [f32; 4],
        n: usize,
        seed: u32,
    },
}

/// One pair of eyes in the dark: where, how big, how open and what it is doing.
#[derive(Clone, Copy, Default)]
struct Pair {
    x: f32,
    y: f32,
    size: f32,
    open: f32,
    /// What the pair does and until when (its clock): 0 hidden, 1 opening, 2 open, 3 closing.
    phase: u8,
    until: f32,
    key: usize,
}

/// What the spider does.
#[derive(Clone, Copy)]
enum Spin {
    /// Out of sight until then (its clock, seconds left).
    Waiting(f32),
    Down,
    /// Hanging, seconds left.
    Hanging(f32),
    Up,
}

/// A small xorshift step: the next number and the seed moved on.
fn next(seed: &mut u32) -> f32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 17;
    *seed ^= *seed << 5;
    (*seed % 10_000) as f32 / 10_000.0
}

/// How much of the content covers a disc (every other pixel), 0..1.
fn cover_in(mask: &[u8], cx: f32, cy: f32, r: f32) -> f32 {
    // no picture's content yet (the first frame): nothing covers it
    if mask.len() < W * H {
        return 0.0;
    }
    let (mut covered, mut n) = (0.0f32, 0.0f32);
    let mut dy = -r;
    while dy <= r {
        let mut dx = -r;
        while dx <= r {
            if dx * dx + dy * dy <= r * r {
                let (x, y) = ((cx + dx) as isize, (cy + dy) as isize);
                if x >= 0 && y >= 0 && (x as usize) < W && (y as usize) < H {
                    covered += mask[y as usize * W + x as usize] as f32 / 255.0;
                }
                n += 1.0;
            }
            dx += 2.0;
        }
        dy += 2.0;
    }
    covered / n.max(1.0)
}

/// The content mask's fingerprint: it changes with the page, not with the effects.
fn fingerprint(mask: &[u8]) -> u64 {
    mask.iter().step_by(7).fold(0xcbf2_9ce4_8422_2325u64, |h, &b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3))
}

/// The particle layers that see only the icons' cover (no labels) and so settle on icons alone.
const SETTLE_ON_ICONS: &[&str] = &["leaves", "sakura"];

/// A shader layer as an effect of the canvas (`gpu.rs`, `shaders.rs`).
struct OnGpu {
    layer: super::gpu::ShaderLayer,
    kind: &'static str,
    /// Its own time of the last press (water rests a while after it).
    pressed: Option<f32>,
    t: f32,
    /// The content mask's fingerprint the moon's place was found for (the night sky).
    placed_for: Option<u64>,
    cpu: Cpu,
    /// Its WGSL read from a folder (trying looks), the file's time and its state's size: a changed
    /// file is compiled again while the deck runs, so a look is tried without building the app.
    source: Option<(std::path::PathBuf, std::time::SystemTime, (u32, u32))>,
    checked: f32,
    /// An upright copy (`<kind>-up`): its CPU part places things as the deck stands.
    up: bool,
}

/// The folder a GPU layer's WGSL is read from instead of the built-in one, when one is set: the
/// `HUBDECK_SHADER_DIR` variable, or a test build's `shader-dir.txt` in the app's data folder
/// (owner, 2026-10-26: "rustc znowu masakrycznie zwiększa usage … nie możemy bez tego testować i
/// iterować?"). Release builds ignore the file (built without `HUBDECK_TEST_BUILD`).
/// Whether looks being tried (`x-<name>.wgsl`) can run here: a test build or `HUBDECK_SHADER_DIR`.
/// The layers of the built-in presets and the screensavers, compiled on the card in the background
/// a while after start, so the first switch to a big scene does not hold the deck for 1-2 s while
/// its program compiles (owner, 2026-10-30, "5. tak jak piszesz"; the compiled programs are kept in
/// `gpu.rs`). Kinds without a program of their own are skipped.
const WARM: &[&str] = &[
    "hollowhill",
    "lanternhollow",
    "tokyorain",
    "goldlight",
    "mist",
    "leaves",
    "mushrooms",
    "fuji-up",
    "sakura-up",
    "koi-up",
    "woodland-up",
    "mushrooms-up",
    "leaves-up",
    "matrix",
    "synthwave",
    "circuit",
    "nebula",
    "spectrum",
    "scene",
    "aurora",
    "aquarium",
    "caustics",
    "lava",
    "fire",
    "neonrain",
    "haunt",
    "ghost",
];

pub fn warm_up() {
    std::thread::Builder::new()
        .name("shader warm-up".into())
        .spawn(|| {
            // after the start's own work: the deck's first pictures, the window
            std::thread::sleep(std::time::Duration::from_secs(10));
            let started = std::time::Instant::now();
            let n = WARM.iter().filter(|k| OnGpu::new(k).is_some()).count();
            if n > 0 {
                log::info!("living deck: {n} scene programs compiled ahead in {} ms", started.elapsed().as_millis());
            }
        })
        .ok();
}

pub fn trials_here() -> bool {
    shader_dir().is_some()
}

/// The looks being tried in the shader folder, as (kind, name), sorted by name: every
/// `x-<name>.wgsl`, named by the folder's `trials.json` (`{"x-<name>": "R2 X1a"}`) or by its file.
/// Empty where no folder is read (a release).
pub fn trials() -> Vec<(String, String)> {
    let Some(dir) = shader_dir() else { return Vec::new() };
    let names: std::collections::BTreeMap<String, String> = std::fs::read_to_string(dir.join("trials.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    let mut found: Vec<(String, String)> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let file = e.file_name().into_string().ok()?;
            let kind = file.strip_suffix(".wgsl")?;
            kind.starts_with("x-").then(|| (kind.to_string(), names.get(kind).cloned().unwrap_or_else(|| kind.trim_start_matches("x-").to_string())))
        })
        .collect();
    found.sort_by(|a, b| a.1.cmp(&b.1));
    found
}

fn shader_dir() -> Option<std::path::PathBuf> {
    if let Some(d) = std::env::var_os("HUBDECK_SHADER_DIR") {
        return Some(d.into());
    }
    // only a test build (`scripts/test-build.ps1` sets `HUBDECK_TEST_BUILD` while it compiles): a
    // release reads its shaders from itself, whatever file is lying in the data folder
    option_env!("HUBDECK_TEST_BUILD")?;
    let file = dirs_data()?.join("shader-dir.txt");
    let dir = std::fs::read_to_string(file).ok()?;
    let dir = std::path::PathBuf::from(dir.trim());
    dir.is_dir().then_some(dir)
}

/// The app's data folder (`%APPDATA%\dev.hubdeck.app`, `~/Library/Application Support/...`).
fn dirs_data() -> Option<std::path::PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA").map(|a| std::path::PathBuf::from(a).join("dev.hubdeck.app"))
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join("Library/Application Support/dev.hubdeck.app"))
    }
}

/// The state of a layer of particles (leaves, petals): 64 of 8 cells.
const PARTICLE_CELLS: u32 = 64 * 8;

/// The moon's radius on the night sky, px.
const MOON_R: f32 = 15.5;

/// Where the night sky's moon leaves the content most room: the place in a key's window, top row
/// first, whose disc and a little round it the content covers least (owner, 2026-10-26: the moon
/// was "i tak za ikonkami"). x, y and the radius, panel pixels.
fn moon_place(keys: &[KeyPlace], mask: &[u8]) -> [f32; 4] {
    let r = MOON_R;
    let mut best = (f32::MAX, [0.0; 4]);
    for (i, k) in keys.iter().enumerate().take(15) {
        let row = (i / 5) as f32;
        let (x0, x1) = (k.x + r + 3.0, k.x + k.w - r - 3.0);
        let (y0, y1) = (k.y + r + 3.0, k.y + k.h - r - 3.0);
        let mut cy = y0;
        while cy <= y1 {
            let mut cx = x0;
            while cx <= x1 {
                // the content under the disc and its halo, read every other pixel
                let (mut cover, mut n) = (0.0f32, 0.0f32);
                let reach = r + 4.0;
                let mut dy = -reach;
                while dy <= reach {
                    let mut dx = -reach;
                    while dx <= reach {
                        if dx * dx + dy * dy <= reach * reach {
                            let (px, py) = ((cx + dx) as usize, (cy + dy) as usize);
                            if px < W && py < H {
                                cover += mask.get(py * W + px).copied().unwrap_or(0) as f32 / 255.0;
                            }
                            n += 1.0;
                        }
                        dx += 2.0;
                    }
                    dy += 2.0;
                }
                // the top row first, then the right; a free place anywhere beats a covered one
                let score = cover / n.max(1.0) + row * 0.04 + (1.0 - cx / W as f32) * 0.01;
                if score < best.0 {
                    best = (score, [cx, cy, r, 1.0]);
                }
                cx += 2.0;
            }
            cy += 2.0;
        }
    }
    best.1
}

impl OnGpu {
    fn new(kind: &str) -> Option<Self> {
        // an upright copy is its original with another body
        let up = base_kind(kind) != kind;
        let (kind, body, state): (&'static str, &str, (u32, u32)) = match base_kind(kind) {
            "water" => ("water", super::shaders::WATER, (W as u32, H as u32)),
            "light" => ("light", super::shaders::LIGHT, (1, 1)),
            "scene" => ("scene", super::shaders::SCENE, (1, 1)),
            "finish" => ("finish", super::shaders::FINISH, (1, 1)),
            "aurora" => ("aurora", include_str!("shaders/aurora.wgsl"), (1, 1)),
            "aquarium" => ("aquarium", include_str!("shaders/aquarium.wgsl"), (1, 1)),
            "cover" => ("cover", include_str!("shaders/cover.wgsl"), (64, 64)),
            "lava" => ("lava", include_str!("shaders/lava.wgsl"), (1, 1)),
            "fire" => ("fire", include_str!("shaders/fire.wgsl"), (1, 1)),
            "glass" => ("glass", include_str!("shaders/glass.wgsl"), (1, 1)),
            "frames" => ("frames", include_str!("shaders/frames.wgsl"), (1, 1)),
            "liquid" => ("liquid", include_str!("shaders/liquid.wgsl"), (1, 1)),
            "frost" => ("frost", include_str!("shaders/frost.wgsl"), (1, 1)),
            "springs" => ("springs", include_str!("shaders/springs.wgsl"), (1, 1)),
            "heart" => ("heart", include_str!("shaders/heart.wgsl"), (1, 1)),
            "flip" => ("flip", include_str!("shaders/flip.wgsl"), (W as u32, H as u32)),
            "hop" => ("hop", include_str!("shaders/hop.wgsl"), (1, 1)),
            "holo" => ("holo", include_str!("shaders/holo.wgsl"), (1, 1)),
            "relief" => ("relief", include_str!("shaders/material.wgsl"), (1, 1)),
            "gold" => ("gold", include_str!("shaders/material.wgsl"), (1, 1)),
            "chrome" => ("chrome", include_str!("shaders/material.wgsl"), (1, 1)),
            "crystal" => ("crystal", include_str!("shaders/material.wgsl"), (1, 1)),
            "mist" => ("mist", include_str!("shaders/mist.wgsl"), (1, 1)),
            "neonrain" => ("neonrain", include_str!("shaders/neonrain.wgsl"), (1, 1)),
            "caustics" => ("caustics", include_str!("shaders/caustics.wgsl"), (1, 1)),
            "pumpkins" => ("pumpkins", include_str!("shaders/pumpkins.wgsl"), (1, 1)),
            "eyes" => ("eyes", include_str!("shaders/eyes.wgsl"), (1, 1)),
            "ghost" => ("ghost", include_str!("shaders/ghost.wgsl"), (1, 1)),
            "spider" => ("spider", include_str!("shaders/spider.wgsl"), (1, 1)),
            "webs" => ("webs", include_str!("shaders/webs.wgsl"), (1, 1)),
            "batburst" => ("batburst", include_str!("shaders/batburst.wgsl"), (1, 1)),
            "ghostrise" => ("ghostrise", include_str!("shaders/ghostrise.wgsl"), (1, 1)),
            "domino" => ("domino", include_str!("shaders/domino.wgsl"), (1, 1)),
            "chain" => ("chain", include_str!("shaders/chain.wgsl"), (1, 1)),
            "firework" => ("firework", include_str!("shaders/firework.wgsl"), (1, 1)),
            "shockwave" => ("shockwave", include_str!("shaders/shockwave.wgsl"), (1, 1)),
            "magnet" => ("magnet", include_str!("shaders/magnet.wgsl"), (1, 1)),
            "bloom" => ("bloom", include_str!("shaders/bloom.wgsl"), (1, 1)),
            "haunt" => ("haunt", include_str!("shaders/haunt.wgsl"), (1, 1)),
            "candle" => ("candle", include_str!("shaders/candle.wgsl"), (1, 1)),
            "carved" => ("carved", include_str!("shaders/carved.wgsl"), (1, 1)),
            "stone" => ("stone", include_str!("shaders/stone.wgsl"), (1, 1)),
            "slime" => ("slime", include_str!("shaders/slime.wgsl"), (1, 1)),
            "spectral" => ("spectral", include_str!("shaders/spectral.wgsl"), (1, 1)),
            // Poland's golden autumn and Japan (owner, 2026-10-26: "Zacznik polskiej złotej jesieni
            // i japonii"); leaves and petals are particles kept on the card, 64 of 8 cells each
            "leaves" => ("leaves", include_str!("shaders/leaves.wgsl"), (PARTICLE_CELLS, 1)),
            "sakura" => ("sakura", include_str!("shaders/sakura.wgsl"), (PARTICLE_CELLS, 1)),
            "storks" => ("storks", include_str!("shaders/storks.wgsl"), (1, 1)),
            "goldlight" => ("goldlight", include_str!("shaders/goldlight.wgsl"), (1, 1)),
            "lanterns" => ("lanterns", include_str!("shaders/lanterns.wgsl"), (1, 1)),
            "ukiyo" => ("ukiyo", include_str!("shaders/ukiyo.wgsl"), (1, 1)),
            "koi" => ("koi", include_str!("shaders/koi.wgsl"), (1, 1)),
            "mushrooms" => ("mushrooms", include_str!("shaders/mushrooms.wgsl"), (1, 1)),
            // Poland B, the mushroom hunt, and Japan A after the owner's reference picture (session 35)
            "woodland" => ("woodland", include_str!("shaders/woodland.wgsl"), (1, 1)),
            "fuji" => ("fuji", include_str!("shaders/fuji.wgsl"), (1, 1)),
            // the themes for developers, gamers, creators, sim players and musicians (session 35)
            "matrix" => ("matrix", include_str!("shaders/matrix.wgsl"), (1, 1)),
            "code" => ("code", include_str!("shaders/code.wgsl"), (1, 1)),
            "synthwave" => ("synthwave", include_str!("shaders/synthwave.wgsl"), (1, 1)),
            "ink" => ("ink", include_str!("shaders/ink.wgsl"), (1, 1)),
            "nebula" => ("nebula", include_str!("shaders/nebula.wgsl"), (1, 1)),
            "spectrum" => ("spectrum", include_str!("shaders/spectrum.wgsl"), (1, 1)),
            "circuit" => ("circuit", include_str!("shaders/circuit.wgsl"), (1, 1)),
            "orbit" => ("orbit", include_str!("shaders/orbit.wgsl"), (1, 1)),
            "vinyl" => ("vinyl", include_str!("shaders/vinyl.wgsl"), (1, 1)),
            "impasto" => ("impasto", include_str!("shaders/impasto.wgsl"), (1, 1)),
            // Halloween for 0.2.0, made by the skill's agents (owner, 2026-10-29: "Z halloween zostaje
            // R2 X2a i R4 C+"): Hollow Hill and Lantern Hollow, kept as he judged them in trial slots
            "hollowhill" => ("hollowhill", include_str!("shaders/hollowhill.wgsl"), (1, 1)),
            "lanternhollow" => ("lanternhollow", include_str!("shaders/lanternhollow.wgsl"), (1, 1)),
            // Tokyo Rain, made by the skill's agent and kept from its trial slot (owner, 2026-10-30: "bo jest spoko")
            "tokyorain" => ("tokyorain", include_str!("shaders/tokyorain.wgsl"), (1, 1)),
            // a look being tried: `x-<name>.wgsl` from the shader folder, so only where one is read
            // (`HUBDECK_SHADER_DIR`, or a test build's `shader-dir.txt`; a release never)
            k if k.starts_with("x-") && shader_dir().is_some() => (Box::leak(k.to_string().into_boxed_str()), "", (W as u32, H as u32)),
            _ => return None,
        };
        // a shader's WGSL from a folder instead (`HUBDECK_SHADER_DIR`, `<kind>.wgsl`): new looks are
        // tried without building the app again (`fx_film`)
        let body = if up {
            match kind {
                "goldlight" => include_str!("shaders/upright/goldlight.wgsl"),
                "mist" => include_str!("shaders/upright/mist.wgsl"),
                "mushrooms" => include_str!("shaders/upright/mushrooms.wgsl"),
                "leaves" => include_str!("shaders/upright/leaves.wgsl"),
                "woodland" => include_str!("shaders/upright/woodland.wgsl"),
                "fuji" => include_str!("shaders/upright/fuji.wgsl"),
                "sakura" => include_str!("shaders/upright/sakura.wgsl"),
                "koi" => include_str!("shaders/upright/koi.wgsl"),
                "matrix" => include_str!("shaders/upright/matrix.wgsl"),
                "synthwave" => include_str!("shaders/upright/synthwave.wgsl"),
                "circuit" => include_str!("shaders/upright/circuit.wgsl"),
                "nebula" => include_str!("shaders/upright/nebula.wgsl"),
                "spectrum" => include_str!("shaders/upright/spectrum.wgsl"),
                _ => return None,
            }
        } else {
            body
        };
        let file_name = if matches!(kind, "relief" | "gold" | "chrome" | "crystal") {
            "material".to_string()
        } else if up {
            format!("{kind}-up")
        } else {
            kind.to_string()
        };
        let file = shader_dir().map(|d| d.join(format!("{file_name}.wgsl"))).filter(|f| f.is_file());
        let own = file.as_ref().and_then(|f| std::fs::read_to_string(f).ok());
        let source = file.as_ref().and_then(|f| Some((f.clone(), std::fs::metadata(f).ok()?.modified().ok()?, state)));
        let layer = super::gpu::ShaderLayer::new(kind, own.as_deref().unwrap_or(body), state)?;
        let cpu = match kind {
            "springs" => Cpu::Springs { d: [0.0; 15], v: [0.0; 15] },
            "liquid" => Cpu::Liquid { level: [0.0; 15], tilt: [0.0; 15], spin: [0.0; 15], slosh: [0.0; 15] },
            "frost" => Cpu::Frost { pressed: [-1e4; 15] },
            "heart" => Cpu::Heart { likes: Vec::new() },
            "flip" => Cpu::Flip { start: None },
            "hop" => Cpu::Hop { from: 7, to: 7, start: 0.0, dur: 0.01, rest: 0.0, called: None, seed: 0x9e37_79b9, at: (0.0, 0.0) },
            "cover" => Cpu::Cover { version: None },
            // mushrooms sit where the jack-o'-lanterns would: each free key's floor
            "pumpkins" | "mushrooms" => Cpu::Pumpkins { places_for: None, places: [[0.0; 2]; 15], pressed: [-1e4; 15] },
            "eyes" => Cpu::Eyes { pairs: [Pair::default(); 4], seed: 0x2f6e_2b1d, look: None, spots_for: None, spots: Vec::new() },
            "spider" => Cpu::Spider { state: Spin::Waiting(0.3), key: 0, len: 0.0, want: 0.0, seed: 0x51ed_270b, called: false },
            "webs" => Cpu::Webs { places_for: None },
            // Fuji's upright copy hears presses: a press shakes petals off the branch above the key
            "fuji" if up => Cpu::Bursts { list: Vec::new() },
            "batburst" | "ghostrise" | "domino" | "firework" | "shockwave" | "magnet" | "bloom" | "koi" | "matrix" | "code" | "synthwave" | "ink" | "nebula" | "circuit" | "orbit" | "impasto"
            | "hollowhill" | "lanternhollow" | "tokyorain" => Cpu::Bursts { list: Vec::new() },
            // a look being tried gets the last presses too (`aux[0..3]`), so a press's answer is
            // tried without building
            k if k.starts_with("x-") => Cpu::Bursts { list: Vec::new() },
            // the music studio: the spectrum of what plays (`beat::bands`) and the last presses
            "spectrum" | "vinyl" => Cpu::Music { list: Vec::new() },
            "chain" => Cpu::Chain { start: None, keys: [0.0; 4], n: 0, seed: 0x6a09_e667 },
            "relief" | "gold" | "chrome" | "crystal" => Cpu::Material { index: ["relief", "gold", "chrome", "crystal"].iter().position(|k| *k == kind).unwrap_or(0) as f32, photos_for: None },
            _ => Cpu::None,
        };
        let mut layer = layer;
        // falling leaves and petals settle on the icons' tops, never on a label (a leaf resting on
        // "Song sorter" hid its letters, 2026-10-27)
        layer.icons_only = matches!(cpu, Cpu::Material { .. }) || SETTLE_ON_ICONS.contains(&kind);
        Some(Self { layer, kind, pressed: None, t: 0.0, placed_for: None, cpu, source, checked: 0.0, up })
    }
}

impl Effect for OnGpu {
    fn step(&mut self, input: &Inputs) -> bool {
        self.t = input.t;
        // a look being tried: its file changed, so it is compiled again (its own state starts anew)
        if let Some((file, time, state)) = &self.source
            && (input.t - self.checked).abs() > 0.7
        {
            self.checked = input.t;
            let now = std::fs::metadata(file).and_then(|m| m.modified()).ok();
            if let Some(now) = now.filter(|now| now != time) {
                let (file, state) = (file.clone(), *state);
                self.source = Some((file.clone(), now, state));
                match std::fs::read_to_string(&file).ok().and_then(|code| super::gpu::ShaderLayer::new(self.kind, &code, state)) {
                    Some(mut fresh) => {
                        fresh.icons_only = self.layer.icons_only;
                        fresh.aux = self.layer.aux;
                        self.layer = fresh;
                        self.placed_for = None;
                        if let Cpu::Cover { version } = &mut self.cpu {
                            *version = None;
                        }
                        log::info!("living deck: {} read again from {}", self.kind, file.display());
                    }
                    None => log::warn!("living deck: {} from {} does not compile; the last one stays", self.kind, file.display()),
                }
            }
        }
        self.layer.aux[15] = input.light;
        self.layer.aux[14] = [if input.moving { 1.0 } else { 0.0 }, if input.left { 1.0 } else { 0.0 }, 0.0, 0.0];
        if let Some(busy) = self.step_cpu(input) {
            return busy;
        }
        if self.kind == "scene" && input.mask.len() == W * H {
            // the mask changes with the page, not with the effects: a cheap fingerprint of it
            let print = input.mask.iter().step_by(7).fold(0xcbf2_9ce4_8422_2325u64, |h, &b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3));
            if self.placed_for != Some(print) {
                self.layer.aux[0] = moon_place(input.keys, input.mask);
                self.placed_for = Some(print);
            }
        }
        match self.kind {
            // the water moves after a press, then rests (no pictures, no CPU)
            "water" => {
                let moving = self.pressed.is_some_and(|at| input.t - at < 9.0);
                if moving {
                    self.layer.step(input, ((24.0 * input.speed).round() as u32).clamp(2, 96));
                }
                moving
            }
            // the lamp drifts, the stars twinkle; the finish moves only with grain
            "light" | "scene" => true,
            "aurora" | "aquarium" | "lava" | "fire" | "frames" | "holo" | "haunt" | "candle" | "carved" | "slime" | "spectral" => true,
            "storks" | "goldlight" | "lanterns" | "ukiyo" | "woodland" | "fuji" | "hollowhill" | "lanternhollow" | "tokyorain" => true,
            // fog, neon rain and the pool's light move all the time: they fell to the grain's rule
            // below and moved only above the middle amount (neon rain as a screensaver: 4.9 pictures
            // a second on the owner's deck, 2026-10-26)
            "mist" | "neonrain" | "caustics" => true,
            // leaves and petals move on the card: a step a frame (presses blow them away)
            "leaves" | "sakura" => {
                self.layer.step(input, 1);
                true
            }
            // still stone: a picture when it changes, none after
            "stone" => false,
            // still glass, unless the deck's light moves (the lamp) or its sheen slides
            "glass" => true,
            k if k.starts_with("x-") => true,
            _ => input.density > 1.0,
        }
    }
    fn under(&mut self, input: &Inputs, panel: &mut [u8]) {
        if self.layer.has_under() {
            self.layer.draw(input, panel, false);
        }
    }
    fn over(&mut self, input: &Inputs, panel: &mut [u8]) {
        if self.layer.has_over() && (self.kind != "water" || self.pressed.is_some_and(|at| input.t - at < 9.5)) {
            self.layer.draw(input, panel, true);
        }
    }
    fn press(&mut self, key: usize, input: &Inputs) {
        if let Some(k) = input.keys.get(key) {
            self.layer.press(k.x + k.w / 2.0, k.y + k.h / 2.0, input.strength, key);
            self.pressed = Some(self.t);
        }
        let t = self.t;
        match &mut self.cpu {
            Cpu::Springs { v, .. } if key < 15 => {
                v[key] += 9.0;
                for n in neighbours(key) {
                    v[n] += 2.5;
                }
            }
            Cpu::Liquid { spin, slosh, .. } if key < 15 => {
                let dir = if (key % 5) < 2 { 1.0 } else { -1.0 };
                spin[key] += 0.9 * dir;
                slosh[key] = 1.0;
                for n in neighbours(key) {
                    spin[n] += 0.35 * dir;
                    slosh[n] = slosh[n].max(0.5);
                }
            }
            Cpu::Frost { pressed } if key < 15 => pressed[key] = t,
            Cpu::Hop { called, .. } if key < 15 => *called = Some(key),
            Cpu::Pumpkins { pressed, .. } if key < 15 => pressed[key] = t,
            Cpu::Eyes { pairs, look, .. } => {
                if let Some(k) = input.keys.get(key) {
                    *look = Some((k.x + k.w / 2.0, k.y + k.h / 2.0, t));
                }
                // eyes in the pressed key shut at once
                for p in pairs.iter_mut().filter(|p| p.key == key && p.phase != 0) {
                    p.phase = 3;
                    p.until = t + 0.12;
                }
            }
            Cpu::Spider { key: k, called, .. } if *k == key => *called = true,
            Cpu::Chain { start, keys, n, seed } if key < 15 => {
                // from the pressed key, three or four leaps, each to a near key not struck yet
                let mut chain = vec![key];
                let leaps = 3 + (next(seed) * 2.0) as usize;
                while chain.len() <= leaps {
                    let last = *chain.last().unwrap_or(&key);
                    let (lc, lr) = ((last % 5) as i32, (last / 5) as i32);
                    let mut options: Vec<usize> = (0..15).filter(|k| !chain.contains(k)).filter(|k| ((*k % 5) as i32 - lc).abs() <= 2 && ((*k / 5) as i32 - lr).abs() <= 1).collect();
                    if options.is_empty() {
                        break;
                    }
                    let pick = (next(seed) * options.len() as f32) as usize;
                    chain.push(options.swap_remove(pick.min(options.len() - 1)));
                }
                *keys = [0.0; 4];
                for (i, k) in chain.iter().take(4).enumerate() {
                    keys[i] = *k as f32;
                }
                *n = chain.len().min(4);
                *start = Some(t);
            }
            Cpu::Bursts { list } | Cpu::Music { list } => {
                if let Some(k) = input.keys.get(key) {
                    list.push((k.x + k.w / 2.0, k.y + k.h / 2.0, t));
                    if list.len() > 3 {
                        list.remove(0);
                    }
                }
            }
            _ => {}
        }
    }
    fn like(&mut self, key: usize, input: &Inputs) {
        if let (Cpu::Heart { likes }, Some(k)) = (&mut self.cpu, input.keys.get(key)) {
            likes.push((k.x + k.w / 2.0, k.y + k.h / 2.0, self.t));
            if likes.len() > 4 {
                likes.remove(0);
            }
        }
    }
    fn page_turned(&mut self, old: &[u8], _input: &Inputs) {
        if let Cpu::Flip { start } = &mut self.cpu
            && old.len() == W * H * 4
        {
            let packed: Vec<u32> = old.as_chunks::<4>().0.iter().map(|p| u32::from_le_bytes([p[0], p[1], p[2], 255])).collect();
            self.layer.upload_state(0, &packed);
            *start = Some(self.t);
        }
    }
    fn wants_last(&self) -> bool {
        matches!(self.cpu, Cpu::Flip { .. })
    }
    fn wants_icons(&self) -> bool {
        matches!(self.cpu, Cpu::Material { .. }) || SETTLE_ON_ICONS.contains(&self.kind)
    }
    fn shader(&self, over: bool, input: &Inputs) -> Option<&super::gpu::ShaderLayer> {
        // the same conditions as `under` and `over` below
        let draws = if over { self.layer.has_over() && (self.kind != "water" || self.pressed.is_some_and(|at| input.t - at < 9.5)) } else { self.layer.has_under() };
        draws.then_some(&self.layer)
    }
}

impl OnGpu {
    /// The CPU part of the layers that have one: moves it on, writes it to `aux`; None for the
    /// others. True while it moves.
    fn step_cpu(&mut self, input: &Inputs) -> Option<bool> {
        let dt = input.dt.max(0.0);
        let t = input.t;
        let kind = self.kind;
        let up = self.up;
        let aux = &mut self.layer.aux;
        let set = |aux: &mut [[f32; 4]; 16], i: usize, v: f32| aux[i / 4][i % 4] = v;
        match &mut self.cpu {
            Cpu::None => None,
            Cpu::Springs { d, v } => {
                // each key on a spring to rest, pulled a little toward its neighbours: a press
                // runs out over the deck and dies away (about a second)
                let mut busy = false;
                let steps = ((dt / 0.004).ceil() as usize).clamp(1, 30);
                let h = dt / steps as f32;
                for _ in 0..steps {
                    let before = *d;
                    for k in 0..15 {
                        let pull: f32 = neighbours(k).map(|n| before[n] - before[k]).sum();
                        // about 1.6 wobbles a second, dying out in a second and a half: slow enough
                        // to be seen at 24 pictures a second (the first, 2 a second, was over in 0.3 s)
                        let a = -100.0 * before[k] + 34.0 * pull - 4.2 * v[k];
                        v[k] += a * h;
                    }
                    for k in 0..15 {
                        d[k] = (d[k] + v[k] * h).clamp(-1.0, 1.0);
                    }
                }
                for k in 0..15 {
                    busy |= d[k].abs() > 0.002 || v[k].abs() > 0.01;
                    set(aux, k, d[k]);
                }
                Some(busy)
            }
            Cpu::Liquid { level, tilt, spin, slosh } => {
                // the level each jar settles to: its amount, a little different jar to jar
                for k in 0..15 {
                    let own = 0.30 + 0.08 * ((k * 7 % 5) as f32 / 4.0);
                    let want = (own * input.density).clamp(0.08, 0.92);
                    level[k] += (want - level[k]) * (1.0 - (-dt * 1.5).exp());
                    let a = -38.0 * tilt[k] - 2.2 * spin[k];
                    spin[k] += a * dt;
                    tilt[k] = (tilt[k] + spin[k] * dt).clamp(-0.5, 0.5);
                    slosh[k] = (slosh[k] - dt * 0.6).max(0.0);
                    set(aux, k, level[k]);
                    set(aux, 16 + k, tilt[k]);
                    set(aux, 32 + k, slosh[k]);
                }
                // the surfaces ripple gently all the time
                Some(true)
            }
            Cpu::Frost { pressed } => {
                #[allow(clippy::needless_range_loop)] // the index reads several arrays at once
                for k in 0..15 {
                    set(aux, k, t - pressed[k]);
                }
                // its sparkles catch the light all the time
                Some(true)
            }
            Cpu::Heart { likes } => {
                likes.retain(|l| t - l.2 < 2.5);
                #[allow(clippy::needless_range_loop)] // the index reads several arrays at once
                for i in 0..4 {
                    aux[i] = likes.get(i).map_or([0.0; 4], |l| [l.0, l.1, l.2, 1.0]);
                }
                Some(!likes.is_empty())
            }
            Cpu::Flip { start } => {
                // a card takes 0.42 s, the cascade a step of 0.045 s (the last card ends after ~0.8 s)
                let (dur, step) = (0.42 / input.speed.max(0.25), 0.045 / input.speed.max(0.25));
                let running = start.is_some_and(|s| t - s < dur + 7.0 * step + 0.05);
                aux[0] = match start {
                    Some(s) if running => [*s, dur, step, 1.0],
                    _ => [0.0; 4],
                };
                if !running {
                    *start = None;
                }
                Some(running)
            }
            Cpu::Hop { from, to, start, dur, rest, called, seed, at } => {
                let centre = |k: usize| input.keys.get(k).map_or((W as f32 / 2.0, H as f32 / 2.0), |p| (p.x + p.w / 2.0, p.y + p.h * 0.62));
                let s = ((t - *start) / *dur).clamp(0.0, 1.0);
                let pos = {
                    let (a, b) = (centre(*from), centre(*to));
                    (a.0 + (b.0 - a.0) * s, a.1 + (b.1 - a.1) * s)
                };
                if let Some(k) = called.take() {
                    // called by a press: from where it is now, straight there
                    *at = pos;
                    *from = usize::MAX;
                    *to = k;
                    *start = t;
                    *dur = 0.45;
                    *rest = 0.0;
                } else if s >= 1.0 && t - (*start + *dur) >= *rest {
                    // the next hop: a key near, now and then one far
                    *seed ^= *seed << 13;
                    *seed ^= *seed >> 17;
                    *seed ^= *seed << 5;
                    let near: Vec<usize> = neighbours(*to).collect();
                    let next = if *seed % 5 == 0 { (*seed / 7) as usize % 15 } else { near[(*seed / 3) as usize % near.len()] };
                    *at = centre(*to);
                    *from = usize::MAX;
                    *to = if next == *to { near[0] } else { next };
                    let dist = {
                        let b = centre(*to);
                        ((b.0 - at.0).powi(2) + (b.1 - at.1).powi(2)).sqrt()
                    };
                    *start = t;
                    *dur = (0.38 + dist / 420.0) / input.speed.max(0.25);
                    *rest = (0.15 + 0.5 * ((*seed >> 9) % 100) as f32 / 100.0) / input.speed.max(0.25);
                }
                let a = if *from == usize::MAX { *at } else { centre(*from) };
                let b = centre(*to);
                let dist = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
                let s = ((t - *start) / *dur).clamp(0.0, 1.0);
                // squashed just after leaving and just after landing
                let land = (1.0 - (s / 0.12).min(1.0)) * 0.6 + if s >= 1.0 { (1.0 - ((t - *start - *dur) / 0.12).min(1.0)).max(0.0) } else { 0.0 };
                aux[0] = [a.0, a.1, b.0, b.1];
                aux[1] = [*start, *dur, 12.0 + dist * 0.18, land];
                Some(true)
            }
            Cpu::Material { index, photos_for } => {
                aux[0] = [*index, 0.0, 0.0, 0.0];
                if input.mask.len() == W * H {
                    let print = input.mask.iter().step_by(7).fold(0xcbf2_9ce4_8422_2325u64, |h, &b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3));
                    if *photos_for != Some(print) {
                        *photos_for = Some(print);
                        // a key whose window is four fifths covered holds a picture, not an icon
                        for (k, p) in input.keys.iter().enumerate().take(15) {
                            let (mut covered, mut n) = (0u32, 0u32);
                            let mut y = p.y.max(0.0) as usize + 1;
                            while y < ((p.y + p.h) as usize).min(H) {
                                let mut x = p.x.max(0.0) as usize + 1;
                                while x < ((p.x + p.w) as usize).min(W) {
                                    covered += (input.mask[y * W + x] > 128) as u32;
                                    n += 1;
                                    x += 3;
                                }
                                y += 3;
                            }
                            set(aux, 16 + k, if n > 0 && covered as f32 / n as f32 > 0.8 { 1.0 } else { 0.0 });
                        }
                    }
                }
                // still unless the light moves
                Some(true)
            }
            Cpu::Pumpkins { places_for, places, pressed } => {
                if input.mask.len() == W * H {
                    let down = input.down;
                    // an upright copy places again when the deck turns
                    let print = fingerprint(input.mask) ^ if up { ((down.0 + 2.0) * 5.0 + down.1 + 2.0) as u64 } else { 0 };
                    if *places_for != Some(print) {
                        *places_for = Some(print);
                        // each key's freer bottom corner; the six keys with the most room get one
                        // (mushrooms only on the bottom row: the forest floor of the golden autumn's
                        // view; on the rows above they stood in the air in front of the trees)
                        let mut free: Vec<(f32, usize, [f32; 2])> = Vec::new();
                        let first = if kind == "mushrooms" && !up { 10 } else { 0 };
                        // upright copy: the bottom row and each key's floor as the deck stands
                        let along = |p: &KeyPlace| (p.x + p.w / 2.0) * down.0 + (p.y + p.h / 2.0) * down.1;
                        let lowest = input.keys.iter().take(15).map(along).fold(f32::MIN, f32::max);
                        for (k, p) in input.keys.iter().enumerate().take(15).skip(first) {
                            if up && kind == "mushrooms" && along(p) < lowest - 1.0 {
                                continue;
                            }
                            if cover_in(input.mask, p.x + p.w / 2.0, p.y + p.h / 2.0, p.w.min(p.h) * 0.45) > 0.8 {
                                continue;
                            }
                            // on the side the owner can see (his deck stands to his right: the right)
                            let y = p.y + p.h - 12.0;
                            let at = if up {
                                let right = (down.1, -down.0);
                                let side = if input.left { -1.0 } else { 1.0 };
                                let deep = p.w / 2.0 * down.0.abs() + p.h / 2.0 * down.1.abs() - 12.0;
                                let across = (p.w / 2.0 * right.0.abs() + p.h / 2.0 * right.1.abs() - 15.0) * side;
                                [p.x + p.w / 2.0 + down.0 * deep + right.0 * across, p.y + p.h / 2.0 + down.1 * deep + right.1 * across]
                            } else if input.left {
                                [p.x + 15.0, y]
                            } else {
                                [p.x + p.w - 15.0, y]
                            };
                            free.push((cover_in(input.mask, at[0], at[1], 11.0), k, at));
                        }
                        free.sort_by(|a, b| a.0.total_cmp(&b.0));
                        *places = [[0.0; 2]; 15];
                        for (c, k, at) in free.into_iter().take(6) {
                            if c < 0.4 {
                                places[k] = at;
                            }
                        }
                    }
                }
                for k in 0..15 {
                    set(aux, 2 * k, places[k][0]);
                    set(aux, 2 * k + 1, places[k][1]);
                    set(aux, 32 + k, t - pressed[k]);
                }
                Some(true)
            }
            Cpu::Eyes { pairs, seed, look, spots_for, spots } => {
                // free places for eyes, found when the page changes: per key the emptiest of nine
                if input.mask.len() == W * H {
                    let print = fingerprint(input.mask);
                    if *spots_for != Some(print) {
                        *spots_for = Some(print);
                        spots.clear();
                        for (k, p) in input.keys.iter().enumerate().take(15) {
                            let mut best = (1.0f32, 0.0, 0.0);
                            for gy in 0..3 {
                                for gx in 0..3 {
                                    let (x, y) = (p.x + p.w * (0.25 + 0.25 * gx as f32), p.y + p.h * (0.22 + 0.22 * gy as f32));
                                    let c = cover_in(input.mask, x, y, 11.0);
                                    if c < best.0 {
                                        best = (c, x, y);
                                    }
                                }
                            }
                            if best.0 < 0.2 {
                                spots.push((k, best.1, best.2));
                            }
                        }
                    }
                }
                if let Some((_, _, at)) = look
                    && t - *at > 4.0
                {
                    *look = None;
                }
                for i in 0..4 {
                    let (phase, until) = (pairs[i].phase, pairs[i].until);
                    match phase {
                        0 if t >= until && !spots.is_empty() => {
                            // a new place: keys rarely pressed are picked more often, never one
                            // another pair watches from
                            let total: f32 = spots.iter().map(|s| input.rare[s.0]).sum();
                            let mut pick = next(seed) * total;
                            let mut chosen = spots[0];
                            for s in spots.iter() {
                                pick -= input.rare[s.0];
                                if pick <= 0.0 {
                                    chosen = *s;
                                    break;
                                }
                            }
                            let taken = pairs.iter().enumerate().any(|(j, p)| j != i && p.phase != 0 && p.key == chosen.0);
                            if taken {
                                pairs[i].until = t + 1.0;
                            } else {
                                let size = 11.0 + 4.0 * next(seed);
                                pairs[i] = Pair { x: chosen.1, y: chosen.2, size, open: 0.0, phase: 1, until: t + 0.35, key: chosen.0 };
                            }
                        }
                        1 => {
                            pairs[i].open = (1.0 - (until - t) / 0.35).clamp(0.0, 1.0);
                            if t >= until {
                                pairs[i].phase = 2;
                                pairs[i].until = t + 4.0 + 5.0 * next(seed);
                            }
                        }
                        2 => {
                            // a blink now and then
                            let blink = ((t + i as f32 * 3.3) % 3.7) < 0.12;
                            pairs[i].open = if blink { 0.15 } else { 1.0 };
                            if t >= until {
                                pairs[i].phase = 3;
                                pairs[i].until = t + 0.3;
                            }
                        }
                        3 => {
                            pairs[i].open = ((until - t) / 0.3).clamp(0.0, 1.0);
                            if t >= until {
                                pairs[i].phase = 0;
                                pairs[i].open = 0.0;
                                pairs[i].until = t + 2.0 + 6.0 * next(seed);
                            }
                        }
                        _ => {}
                    }
                    aux[i] = if pairs[i].phase == 0 { [0.0; 4] } else { [pairs[i].x, pairs[i].y, pairs[i].open, pairs[i].size] };
                }
                aux[4] = match look {
                    Some((x, y, _)) => [*x, *y, 0.0, 1.0],
                    None => [0.0; 4],
                };
                Some(true)
            }
            Cpu::Spider { state, key, len, want, seed, called } => {
                let p = input.keys.get(*key).copied().unwrap_or_default();
                match *state {
                    Spin::Waiting(left) => {
                        let left = left - dt;
                        if left <= 0.0 {
                            // a key with room in its upper middle, rarely pressed ones more often
                            let mut best = (f32::MAX, 0usize);
                            for (k, q) in input.keys.iter().enumerate().take(15) {
                                let c = cover_in(input.mask, q.x + q.w * 0.5, q.y + q.h * 0.3, 8.0);
                                let score = c - input.rare[k] * 0.3 + next(seed) * 0.35;
                                if score < best.0 {
                                    best = (score, k);
                                }
                            }
                            *key = best.1;
                            *len = 0.0;
                            let q = input.keys.get(*key).copied().unwrap_or_default();
                            *want = q.h * (0.32 + 0.22 * next(seed));
                            *state = Spin::Down;
                        } else {
                            *state = Spin::Waiting(left);
                        }
                    }
                    Spin::Down => {
                        *len = (*len + dt * 16.0).min(*want);
                        if *len >= *want {
                            *state = Spin::Hanging(6.0 + 7.0 * next(seed));
                        }
                    }
                    Spin::Hanging(left) => {
                        *state = if left - dt <= 0.0 || *called { Spin::Up } else { Spin::Hanging(left - dt) };
                    }
                    Spin::Up => {
                        *len -= dt * if *called { 90.0 } else { 26.0 };
                        if *len <= 0.0 {
                            *len = 0.0;
                            *called = false;
                            *state = Spin::Waiting(4.0 + 10.0 * next(seed));
                        }
                    }
                }
                let out = !matches!(state, Spin::Waiting(_));
                let moving = matches!(state, Spin::Down | Spin::Up);
                aux[0] = [p.x + p.w * 0.5 + 6.0, p.y, *len, if out { 1.0 } else { 0.0 }];
                aux[1] = [(t * 1.3).sin() * 2.2 * (*len / 30.0), if moving { 7.0 } else { 1.6 }, 0.0, 0.0];
                Some(true)
            }
            Cpu::Webs { places_for } => {
                if input.mask.len() == W * H {
                    let print = fingerprint(input.mask);
                    if *places_for != Some(print) {
                        *places_for = Some(print);
                        // free corners, the upper ones first, one a key, six at most
                        let mut found: Vec<(f32, [f32; 4])> = Vec::new();
                        for (k, p) in input.keys.iter().enumerate().take(15) {
                            if cover_in(input.mask, p.x + p.w / 2.0, p.y + p.h / 2.0, p.w.min(p.h) * 0.45) > 0.8 {
                                continue;
                            }
                            let corners: Vec<(f32, f32, f32)> = if input.left { vec![(p.x, p.y, 0.0), (p.x, p.y + p.h, 2.0)] } else { vec![(p.x + p.w, p.y, 1.0), (p.x + p.w, p.y + p.h, 3.0)] };
                            let mut best = (f32::MAX, [0.0; 4]);
                            for (i, (cx, cy, code)) in corners.iter().enumerate() {
                                let inward = (if *code as u8 & 1 == 1 { -1.0 } else { 1.0 }, if *code as u8 & 2 == 2 { -1.0 } else { 1.0 });
                                let c = cover_in(input.mask, cx + inward.0 * 9.0, cy + inward.1 * 9.0, 9.0) + if i >= 1 { 0.1 } else { 0.0 };
                                if c < best.0 {
                                    best = (c, [*cx, *cy, *code, 24.0 + 4.0 * ((k * 7) % 3) as f32]);
                                }
                            }
                            if best.0 < 0.15 {
                                found.push(best);
                            }
                        }
                        found.sort_by(|a, b| a.0.total_cmp(&b.0));
                        // spread over the deck: every other one of the freest
                        #[allow(clippy::needless_range_loop)] // the index reads several arrays at once
                        for i in 0..8 {
                            aux[i] = found.get(i * 2).map_or([0.0; 4], |f| f.1);
                        }
                    }
                }
                Some(true)
            }
            Cpu::Chain { start, keys, n, .. } => {
                let on = start.is_some_and(|s| t - s < 0.6);
                aux[0] = [start.unwrap_or(0.0), keys[0], keys[1], keys[2]];
                aux[1] = [keys[3], *n as f32, 0.0, if on { 1.0 } else { 0.0 }];
                if !on {
                    *start = None;
                }
                Some(on)
            }
            Cpu::Bursts { list } => {
                // the koi swim all the time and stay gathered at a press for a while; a look being
                // tried moves all the time and steps its state every frame
                let koi = self.kind == "koi" || (up && self.kind == "fuji") || ALWAYS_MOVING.contains(&self.kind);
                let trying = self.kind.starts_with("x-");
                // the Halloween scenes made in trial slots keep a press as long as they had there
                let held = trying || HELD_PRESS.contains(&self.kind);
                list.retain(|b| {
                    t - b.2
                        < if koi {
                            8.0
                        } else if held {
                            4.0
                        } else {
                            2.5
                        }
                });
                // the newest press first: scenes that show one press read `aux[0]`, and the oldest there
                // kept a second press on the same key from playing again (owner, 2026-10-30: "przycisk
                // nacisnąć i sekundę później nacisnąć i powinien być ten sam efekt powtórzony")
                #[allow(clippy::needless_range_loop)] // the index reads several arrays at once
                for i in 0..3 {
                    aux[i] = list.iter().rev().nth(i).map_or([0.0; 4], |b| [b.0, b.1, b.2, 1.0]);
                }
                if trying {
                    self.layer.step(input, 1);
                }
                Some(koi || held || !list.is_empty())
            }
            Cpu::Music { list } => {
                // `aux[0..3]` the last presses, `aux[4..8]` the sixteen bands, `aux[8]`: 1 while
                // music is heard, seconds since its last beat
                super::beat::want_spectrum();
                list.retain(|b| t - b.2 < 3.0);
                #[allow(clippy::needless_range_loop)] // the index reads several arrays at once
                for i in 0..3 {
                    aux[i] = list.iter().rev().nth(i).map_or([0.0; 4], |b| [b.0, b.1, b.2, 1.0]);
                }
                let bands = super::beat::bands();
                for i in 0..4 {
                    aux[4 + i] = bands.map_or([0.0; 4], |b| [b[i * 4], b[i * 4 + 1], b[i * 4 + 2], b[i * 4 + 3]]);
                }
                aux[8] = [if bands.is_some() { 1.0 } else { 0.0 }, super::beat::since_beat(), 0.0, 0.0];
                Some(true)
            }
            Cpu::Cover { version } => {
                let now = super::images::cover_version();
                if *version != Some(now) {
                    *version = Some(now);
                    match super::images::cover_small() {
                        Some((pixels, _)) => {
                            self.layer.upload_state(0, &pixels);
                            self.layer.aux[0] = [1.0, 0.0, 0.0, 0.0];
                        }
                        None => self.layer.aux[0] = [0.0; 4],
                    }
                }
                Some(true)
            }
        }
    }
}

/// A layer at work: its effect, its options and its own clock (its speed runs it faster or slower).
struct Running {
    effect: Box<dyn Effect>,
    opts: Opts,
    t: f32,
    kind: String,
    /// The time its step and drawing took since the costs were last read ([`Canvas::costs`]).
    spent: std::time::Duration,
    /// Its own drawing time the last time it was drawn alone (a chain's time is shared by these).
    alone: std::time::Duration,
}

/// What a profile asks of the canvas (`Profile::effects`), and the layers it runs.
pub struct Canvas {
    /// The kinds in order: the canvas is made anew only when they change (options change live).
    key: String,
    started: std::time::Instant,
    last: std::time::Instant,
    layers: Vec<Running>,
    /// The last frame's keys and "down", for presses between frames.
    keys: Vec<KeyPlace>,
    down: (f32, f32),
    t: f32,
    busy: bool,
    /// The keys' content cover of the last picture ([`Inputs::mask`]).
    mask: Vec<u8>,
    /// The last whole picture, kept only for a page turn.
    last_picture: Vec<u8>,
    /// The deck's one light this frame ([`Inputs::light`]).
    light: [f32; 4],
    /// The icons' cover without labels, when a layer wants it ([`Inputs::icons`]).
    icons: Vec<u8>,
    /// How rarely each key is pressed ([`Inputs::rare`]), and whether it was told.
    rare: [f32; 15],
    rare_set: bool,
    /// The soft shadow behind the icons and labels on a scene ([`shadow_of`]), as the pixels it
    /// touches: the pixel and how much of it stays (0..255); and the content it was made from.
    halo: Vec<(u32, u16)>,
    halo_for: Option<u64>,
    /// The halo's time since the costs were last read.
    halo_spent: std::time::Duration,
    /// Frames stepped so far (one in 24 draws its layers each alone, for the frame cost).
    frames: u64,
}

/// The kinds that paint the keys' backgrounds as a scene: the icons and labels on them get a soft
/// shadow of their own shape, so they stand out from the scene (owner's "1 a", 2026-10-27: "spada
/// wtedy czytelnosc przyciskó (np. tych szarych)"; the shape, not a box: his "1 a", 2026-10-30, to
/// "TE cienie pod ikonami strasznie slabo wygladaja", https://claude.ai/artifact/49hkreHQu3U55R4iKmEtHG).
/// How dark the shadow is at its densest (0..255): about three fifths.
const SHADOW_DARKEN: u32 = 158;
/// How far the shadow is spread: three box blurs of this radius (a gaussian of ~2.2 px).
const SHADOW_SPREAD: usize = 2;
const BACKDROP_KINDS: &[&str] = &[
    "scene",
    "aurora",
    "aquarium",
    "cover",
    "lava",
    "caustics",
    "haunt",
    "goldlight",
    "ukiyo",
    "lanterns",
    "koi",
    "fire",
    "woodland",
    "fuji",
    "matrix",
    "code",
    "synthwave",
    "ink",
    "nebula",
    "spectrum",
    "circuit",
    "orbit",
    "vinyl",
    "impasto",
    "hollowhill",
    "lanternhollow",
    "tokyorain",
];
/// The scenes that were made as looks being tried and keep a press for 4 s, as they did there.
const HELD_PRESS: &[&str] = &["hollowhill", "lanternhollow", "tokyorain"];

/// Scenes that answer presses (their last presses in `aux[0..3]`) and move all the time.
const ALWAYS_MOVING: &[&str] = &["matrix", "code", "synthwave", "ink", "nebula", "circuit", "orbit", "impasto"];

/// The shadow behind the content: its cover moved a pixel down and blurred (three box blurs, close to
/// a gaussian of ~2.2 px), strengthened so the glyph's own edge is dark; 0..255 per pixel.
fn shadow_of(mask: &[u8]) -> Vec<u32> {
    let mut down = vec![0u32; W * H];
    for y in 1..H {
        for x in 0..W {
            down[y * W + x] = mask[(y - 1) * W + x] as u32;
        }
    }
    let mut a = down;
    for _ in 0..3 {
        a = box_blur(a, SHADOW_SPREAD);
    }
    a.into_iter().map(|v| (v * 18 / 10).min(255)).collect()
}

/// One box blur of `r` px each way (rows, then columns), with a running sum.
fn box_blur(a: Vec<u32>, r: usize) -> Vec<u32> {
    let mut b = vec![0u32; W * H];
    for y in 0..H {
        let row = &a[y * W..(y + 1) * W];
        let mut sum: u32 = row[..r.min(W)].iter().sum();
        for x in 0..W {
            if x + r < W {
                sum += row[x + r];
            }
            if x > r {
                sum -= row[x - r - 1];
            }
            let n = (x + r).min(W - 1) - x.saturating_sub(r) + 1;
            b[y * W + x] = sum / n as u32;
        }
    }
    let mut out = vec![0u32; W * H];
    for x in 0..W {
        let mut sum: u32 = (0..r.min(H)).map(|y| b[y * W + x]).sum();
        for y in 0..H {
            if y + r < H {
                sum += b[(y + r) * W + x];
            }
            if y > r {
                sum -= b[(y - r - 1) * W + x];
            }
            let n = (y + r).min(H - 1) - y.saturating_sub(r) + 1;
            out[y * W + x] = sum / n as u32;
        }
    }
    out
}

/// `#rrggbb` as 0..1 colour.
pub fn rgb(hex: &str) -> Option<[f32; 3]> {
    let h = hex.strip_prefix('#')?;
    if h.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some([((v >> 16) & 255) as f32 / 255.0, ((v >> 8) & 255) as f32 / 255.0, (v & 255) as f32 / 255.0])
}

fn kinds_key(stack: &[super::profile::Layer]) -> String {
    stack.iter().map(|l| l.kind.as_str()).collect::<Vec<_>>().join("|")
}

/// The layers of `fx` the canvas runs: the known kinds, and looks being tried (`x-<name>`) where a
/// shader folder is read (`trials_here`: on the owner's deck too, to test A/B branches, 2026-10-29). One filter for making, matching and updating the canvas: when
/// `is_for` left the tried ones out, the canvas was made anew every frame and they stood still.
fn running_stack(fx: &super::profile::Effects) -> Vec<super::profile::Layer> {
    let trying = trials_here();
    fx.stack().into_iter().filter(|l| KINDS.contains(&l.kind.as_str()) || (trying && l.kind.starts_with("x-"))).collect()
}

impl Canvas {
    /// The canvas for these effects, or None when none is asked for.
    pub fn new(fx: &super::profile::Effects) -> Option<Self> {
        let stack = running_stack(fx);
        let layers: Vec<Running> = stack
            .iter()
            .filter_map(|l| {
                let opts = Opts::of(l);
                if GPU_KINDS.contains(&l.kind.as_str()) || l.kind.starts_with("x-") {
                    // without a graphics card (or a shader that does not compile) the layer is left
                    // out, and `gpu::get` logged why
                    let effect = OnGpu::new(&l.kind)?;
                    return Some(Running { effect: Box::new(effect), opts, t: 0.0, kind: l.kind.clone(), spent: std::time::Duration::ZERO, alone: std::time::Duration::ZERO });
                }
                let effect: Box<dyn Effect> = match l.kind.as_str() {
                    "fog" => Box::new(Fog::new()),
                    "rain" => Box::new(Rain::new()),
                    "snow" => Box::new(Snow::new(15)),
                    "fireflies" => Box::new(Fireflies::new()),
                    "embers" => Box::new(Embers::new(true)),
                    "burst" => Box::new(Embers::new(false)),
                    "ripple" => Box::new(Ripple::new()),
                    _ => Box::new(Bats::new()),
                };
                Some(Running { effect, opts, t: 0.0, kind: l.kind.clone(), spent: std::time::Duration::ZERO, alone: std::time::Duration::ZERO })
            })
            .collect();
        if layers.is_empty() {
            return None;
        }
        let now = std::time::Instant::now();
        Some(Self {
            key: kinds_key(&stack),
            started: now,
            last: now,
            layers,
            keys: Vec::new(),
            down: (0.0, 1.0),
            t: 0.0,
            busy: true,
            mask: Vec::new(),
            last_picture: Vec::new(),
            light: [0.0; 4],
            icons: Vec::new(),
            rare: [1.0; 15],
            rare_set: false,
            halo: Vec::new(),
            halo_for: None,
            halo_spent: std::time::Duration::ZERO,
            frames: 0,
        })
    }

    /// Whether this canvas runs the layers of `fx` (else it is made anew).
    pub fn is_for(&self, fx: &super::profile::Effects) -> bool {
        self.key == kinds_key(&running_stack(fx))
    }

    /// The layers' options from `fx` (the same kinds in the same order): a slider moves the effect
    /// as it runs, without starting it again (snow stays on the keys).
    pub fn update(&mut self, fx: &super::profile::Effects) {
        let stack = running_stack(fx);
        for (running, l) in self.layers.iter_mut().zip(&stack) {
            running.opts = Opts::of(l);
        }
    }

    /// The deck's one light: the lamp's place when a lamp layer runs, else a light drifting
    /// beyond the top edge on the canvas's own clock.
    fn light_now(&self) -> [f32; 4] {
        let (w, h) = (W as f32, H as f32);
        if let Some(lamp) = self.layers.iter().find(|l| l.kind == "light") {
            let a = if lamp.opts.moving { lamp.t * 0.22 } else { 0.0 } + lamp.opts.wind * std::f32::consts::PI;
            return [w * 0.5 + a.cos() * w * 0.42, h * 0.5 + (a * 1.3).sin() * h * 0.36, 110.0, 1.0];
        }
        let a = self.t * 0.16;
        [w * (0.5 + 0.62 * a.cos()), -30.0 + 22.0 * (a * 1.3).sin(), 110.0, 1.0]
    }

    /// A key whose action likes something was pressed (the heart).
    pub fn like(&mut self, key: usize) {
        let keys = std::mem::take(&mut self.keys);
        let mask = std::mem::take(&mut self.mask);
        let mut layers = std::mem::take(&mut self.layers);
        for l in &mut layers {
            let input = self.inputs(&l.opts, l.t, &keys, &mask, 0.0);
            l.effect.like(key, &input);
        }
        self.layers = layers;
        self.keys = keys;
        self.mask = mask;
        self.busy = true;
    }

    /// The page changed: the layers that turn pages get the picture of the page before.
    pub fn page_turned(&mut self) {
        if self.last_picture.is_empty() {
            return;
        }
        let keys = std::mem::take(&mut self.keys);
        let mask = std::mem::take(&mut self.mask);
        let old = std::mem::take(&mut self.last_picture);
        let mut layers = std::mem::take(&mut self.layers);
        for l in &mut layers {
            let input = self.inputs(&l.opts, l.t, &keys, &mask, 0.0);
            l.effect.page_turned(&old, &input);
        }
        self.layers = layers;
        self.keys = keys;
        self.mask = mask;
        self.last_picture = old;
        self.busy = true;
    }

    fn inputs<'a>(&'a self, o: &Opts, t: f32, keys: &'a [KeyPlace], mask: &'a [u8], dt: f32) -> Inputs<'a> {
        Inputs {
            t,
            dt,
            keys,
            down: self.down,
            tint: o.tint,
            strength: o.strength,
            mask,
            speed: o.speed,
            density: o.density,
            wind: o.wind,
            light: self.light,
            icons: &self.icons,
            rare: self.rare,
            moving: o.moving,
            left: o.left,
        }
    }

    /// How rarely each key of the page is pressed, 0..1 (1 = never).
    pub fn set_rare(&mut self, rare: [f32; 15]) {
        self.rare = rare;
        self.rare_set = true;
    }

    /// Whether it was just made (it has not been told how rarely keys are pressed yet).
    pub fn fresh(&self) -> bool {
        !self.rare_set
    }

    /// Whether a layer wants the icons' cover without labels.
    pub fn wants_icons(&self) -> bool {
        self.layers.iter().any(|l| l.effect.wants_icons())
    }

    /// The icons alone of this picture (RGBA, premultiplied): their cover is kept ([`Inputs::icons`]).
    pub fn icons(&mut self, rgba: &[u8]) {
        self.icons.clear();
        self.icons.extend(rgba.as_chunks::<4>().0.iter().map(|p| p[3]));
    }

    /// Moves every layer on to now (each by its own speed); true when a new picture is needed.
    pub fn step(&mut self, keys: &[KeyPlace], down: (f32, f32)) -> bool {
        let now = std::time::Instant::now();
        let dt = now.duration_since(self.last).as_secs_f32().min(0.1);
        self.last = now;
        self.frames += 1;
        self.t = now.duration_since(self.started).as_secs_f32();
        self.keys = keys.to_vec();
        self.down = down;
        let mask = std::mem::take(&mut self.mask);
        let mut layers = std::mem::take(&mut self.layers);
        let mut busy = false;
        for l in &mut layers {
            let ldt = (dt * l.opts.speed).min(0.1);
            l.t += ldt;
        }
        self.layers = layers;
        self.light = self.light_now();
        let mut layers = std::mem::take(&mut self.layers);
        for l in &mut layers {
            let ldt = (dt * l.opts.speed).min(0.1);
            let input = self.inputs(&l.opts, l.t, keys, &mask, ldt);
            let at = std::time::Instant::now();
            busy |= l.effect.step(&input);
            l.spent += at.elapsed();
        }
        self.layers = layers;
        self.mask = mask;
        self.busy = busy;
        busy
    }

    pub fn busy(&self) -> bool {
        self.busy
    }

    /// Each layer's time (its step and drawing, the GPU's upload and read-back included) since the
    /// last call, by kind in the engine's order, then the halo behind the icons ("halo"); the
    /// frame cost debugger (owner, 2026-10-27: "debugger który pokaże mi ile każdy z
    /// efektów/laerów dodane ms na obliczenia").
    pub fn costs(&mut self) -> Vec<(String, std::time::Duration)> {
        let mut out: Vec<(String, std::time::Duration)> = self.layers.iter_mut().map(|l| (l.kind.clone(), std::mem::take(&mut l.spent))).collect();
        if !self.halo_spent.is_zero() {
            out.push(("halo".to_string(), std::mem::take(&mut self.halo_spent)));
        }
        out
    }

    /// The keys' content of this picture, drawn on its own (RGBA, premultiplied): its cover is
    /// kept as the mask the effects see.
    pub fn content(&mut self, rgba: &[u8]) {
        self.mask.clear();
        self.mask.extend(rgba.as_chunks::<4>().0.iter().map(|p| p[3]));
    }

    pub fn press(&mut self, key: usize) {
        let keys = std::mem::take(&mut self.keys);
        let mask = std::mem::take(&mut self.mask);
        let mut layers = std::mem::take(&mut self.layers);
        for l in &mut layers {
            // a layer whose answer to presses is switched off keeps still (the press effects are
            // themselves the answer)
            if !l.opts.react && !PRESS_KINDS.contains(&base_kind(&l.kind)) {
                continue;
            }
            let input = self.inputs(&l.opts, l.t, &keys, &mask, 0.0);
            l.effect.press(key, &input);
        }
        self.layers = layers;
        self.keys = keys;
        self.mask = mask;
        self.busy = true;
    }

    /// Draws under the keys' content: `panel` is the RGBA panel with the keys' backgrounds.
    pub fn under(&mut self, panel: &mut [u8]) {
        self.draw(panel, false);
        let halo_at = std::time::Instant::now();
        // a scene behind the icons: a soft shadow of each icon's and label's own shape, made again
        // only when the content changes
        if self.mask.len() == W * H && self.layers.iter().any(|l| BACKDROP_KINDS.contains(&base_kind(&l.kind)) || l.kind.starts_with("x-")) {
            let print = fingerprint(&self.mask);
            if self.halo_for != Some(print) {
                // what each touched pixel keeps, worked out once per content (it was worked out for
                // all 130 560 pixels every frame: 0.7 ms a frame, session 35's frame cost)
                self.halo = shadow_of(&self.mask).into_iter().enumerate().filter(|(_, s)| *s > 0).map(|(i, s)| (i as u32, (255 - s * SHADOW_DARKEN / 255) as u16)).collect();
                self.halo_for = Some(print);
            }
            for &(i, keep) in &self.halo {
                let k = keep as u32;
                for c in &mut panel[i as usize * 4..i as usize * 4 + 3] {
                    *c = (*c as u32 * k / 255) as u8;
                }
            }
        }
        self.halo_spent += halo_at.elapsed();
    }

    /// Draws over the keys' content.
    pub fn over(&mut self, panel: &mut [u8]) {
        self.draw(panel, true);
    }

    fn draw(&mut self, panel: &mut [u8], over: bool) {
        let keys = std::mem::take(&mut self.keys);
        let mask = std::mem::take(&mut self.mask);
        let mut layers = std::mem::take(&mut self.layers);
        // neighbouring layers on the card are drawn as one chain (the panel up and back once); one
        // frame in 24 draws each alone, so the frame cost knows each one's own share
        let sample = self.frames.is_multiple_of(24);
        let mut i = 0;
        while i < layers.len() {
            let mut j = i;
            if !sample {
                while j < layers.len() && layers[j].effect.shader(over, &self.inputs(&layers[j].opts, layers[j].t, &keys, &mask, 0.0)).is_some() {
                    j += 1;
                }
            }
            if j - i >= 2 {
                let inputs: Vec<Inputs> = layers[i..j].iter().map(|l| self.inputs(&l.opts, l.t, &keys, &mask, 0.0)).collect();
                let at = std::time::Instant::now();
                let drawn = {
                    let chain: Vec<(&super::gpu::ShaderLayer, &Inputs)> = layers[i..j].iter().zip(&inputs).filter_map(|(l, input)| l.effect.shader(over, input).map(|s| (s, input))).collect();
                    chain.len() == j - i && super::gpu::draw_chain(&chain, panel, over)
                };
                let spent = at.elapsed();
                drop(inputs);
                if drawn {
                    let weight: f32 = layers[i..j].iter().map(|l| l.alone.as_secs_f32().max(1e-4)).sum();
                    for l in &mut layers[i..j] {
                        l.spent += spent.mul_f32(l.alone.as_secs_f32().max(1e-4) / weight);
                    }
                    i = j;
                    continue;
                }
            }
            let l = &mut layers[i];
            let input = self.inputs(&l.opts, l.t, &keys, &mask, 0.0);
            let at = std::time::Instant::now();
            if over {
                l.effect.over(&input, panel)
            } else {
                l.effect.under(&input, panel)
            }
            let spent = at.elapsed();
            l.spent += spent;
            l.alone = spent;
            i += 1;
        }
        // the picture as the deck gets it, for a page turn
        if over && layers.iter().any(|l| l.effect.wants_last()) {
            self.last_picture.clear();
            self.last_picture.extend_from_slice(panel);
        }
        self.layers = layers;
        self.keys = keys;
        self.mask = mask;
    }
}

/// The screen's "down" in panel coordinates for content turned `rotation` degrees clockwise.
pub fn down_for(rotation: i32) -> (f32, f32) {
    match rotation.rem_euclid(360) {
        90 => (-1.0, 0.0),
        180 => (0.0, -1.0),
        270 => (1.0, 0.0),
        _ => (0.0, 1.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::profile::Effects;
    use crate::engine::render::PanelGeometry;

    fn places() -> Vec<KeyPlace> {
        let g = PanelGeometry::default();
        (0..15)
            .map(|i| {
                let (x, y, w, h) = g.key_rect(i);
                KeyPlace { x, y, w, h }
            })
            .collect()
    }

    fn layers(kinds: &[&str]) -> Effects {
        use crate::engine::profile::Layer;
        Effects { layers: kinds.iter().map(|k| Layer { kind: k.to_string(), ..Layer::default() }).collect(), ..Effects::default() }
    }

    fn fx(canvas: Option<&str>, reaction: Option<&str>, inhabitant: Option<&str>) -> Effects {
        Effects { canvas: canvas.map(str::to_string), reaction: reaction.map(str::to_string), inhabitant: inhabitant.map(str::to_string), ..Effects::default() }
    }

    /// Every effect runs, draws something and keeps to the panel, in every turn of the module.
    #[test]
    fn every_effect_draws_in_every_turn() {
        let keys = places();
        for rotation in [0, 90, 180, 270] {
            for f in CANVASES.iter().map(|c| fx(Some(c), None, None)).chain(REACTIONS.iter().map(|r| fx(None, Some(r), None))).chain(INHABITANTS.iter().map(|i| fx(None, None, Some(i)))) {
                let mut c = Canvas::new(&f).expect("an effect");
                let grey = vec![40u8; W * H * 4];
                let mut panel = grey.clone();
                for n in 0..60 {
                    if n == 5 {
                        c.press(7);
                    }
                    // a tenth of a second a frame, as if time went by
                    c.last -= std::time::Duration::from_millis(100);
                    c.started -= std::time::Duration::from_millis(100);
                    c.step(&keys, down_for(rotation));
                    c.under(&mut panel);
                    c.over(&mut panel);
                }
                assert!(panel != grey, "{f:?} at {rotation} drew nothing");
            }
        }
    }

    /// Every layer kind at the ends of its options (speed, amount, wind) draws and keeps to the
    /// panel in every turn; a moved option keeps the same canvas (snow stays on the keys), a new
    /// kind or order makes a new one; the old three places read as layers.
    #[test]
    fn layers_take_their_options() {
        use crate::engine::profile::Layer;
        let keys = places();
        // the GPU layers only where there is a graphics card (not on CI)
        let gpu = crate::engine::gpu::get().is_some();
        for rotation in [0, 90, 180, 270] {
            for kind in KINDS.iter().filter(|k| gpu || !GPU_KINDS.contains(k)) {
                for (speed, density, wind) in [(0u8, 0u8, -100i8), (100, 100, 100), (50, 50, 0)] {
                    let layer = Layer {
                        kind: kind.to_string(),
                        speed: Some(speed),
                        density: Some(density),
                        wind: Some(wind),
                        strength: Some(100),
                        tint: Some("#ff8800".into()),
                        on: None,
                        react: None,
                        moving: None,
                        side: None,
                    };
                    let f = Effects { layers: vec![layer], ..Effects::default() };
                    let mut c = Canvas::new(&f).expect("a layer");
                    let grey = vec![40u8; W * H * 4];
                    let mut panel = grey.clone();
                    // bats come first after 1.5 s of their own clock (at the slowest pace, 60 frames of 0.1 s)
                    for n in 0..(if *kind == "bats" { 80 } else { 40 }) {
                        if n == 5 {
                            c.press(7);
                            // the heart answers a Like, the cards a page turn
                            c.like(7);
                        }
                        if n == 8 {
                            c.page_turned();
                        }
                        c.last -= std::time::Duration::from_millis(100);
                        c.started -= std::time::Duration::from_millis(100);
                        c.step(&keys, down_for(rotation));
                        c.under(&mut panel);
                        c.over(&mut panel);
                    }
                    assert!(panel != grey, "{kind} {speed}/{density}/{wind} at {rotation} drew nothing");
                }
            }
        }
        let snow = |density: u8| Effects {
            layers: vec![Layer { kind: "snow".into(), density: Some(density), ..Layer::default() }, Layer { kind: "ripple".into(), ..Layer::default() }],
            ..Effects::default()
        };
        let c = Canvas::new(&snow(50)).unwrap();
        assert!(c.is_for(&snow(90)), "a moved option keeps the canvas");
        let other = Effects { layers: vec![Layer { kind: "ripple".into(), ..Layer::default() }, Layer { kind: "snow".into(), ..Layer::default() }], ..Effects::default() };
        assert!(!c.is_for(&other), "another order is another canvas");
        let off = Effects { layers: vec![Layer { kind: "snow".into(), on: Some(false), ..Layer::default() }], ..Effects::default() };
        assert!(!off.active() && Canvas::new(&off).is_none(), "a layer switched off draws nothing");
        let old = fx(Some("fog"), Some("ripple"), Some("bats"));
        assert_eq!(old.stack().iter().map(|l| l.kind.as_str()).collect::<Vec<_>>(), ["ripple", "fog", "bats"]);
    }

    /// Every GPU layer compiles on this machine's graphics card (owner's "2 a", 2026-10-26: the
    /// eyes and chain lightning never ran on his deck, a layer that does not compile is left out
    /// without a word); skipped where there is no card (CI).
    #[test]
    fn every_gpu_layer_compiles() {
        if crate::engine::gpu::get().is_none() {
            return;
        }
        let broken: Vec<&str> = GPU_KINDS.iter().copied().filter(|k| OnGpu::new(k).is_none()).collect();
        assert!(broken.is_empty(), "GPU layers that do not compile: {broken:?}");
    }

    /// An upright copy lying down draws what its original draws, frame by frame, a press included
    /// (owner, 2026-10-29: copies for the upright deck, the originals as they are); skipped where
    /// there is no card (CI). Fuji's copy is its own picture in both turns (`REDRAWN`).
    #[test]
    fn upright_copies_lying_draw_their_originals() {
        if crate::engine::gpu::get().is_none() {
            return;
        }
        let keys = places();
        // icons on some keys, so leaves settle and mushrooms find their room
        let mut mask = vec![0u8; W * H];
        for k in [0usize, 3, 6, 7, 11, 13] {
            let p = &keys[k];
            for y in (p.y + 20.0) as usize..(p.y + 50.0) as usize {
                for x in (p.x + 25.0) as usize..(p.x + 55.0) as usize {
                    mask[y * W + x] = 255;
                }
            }
        }
        let base: Vec<u8> = (0..W * H).flat_map(|i| [0x1b, 0x1a, (0x20 + (i % W) / 8) as u8, 255]).collect();
        // the spectrum follows what the computer plays right now (its bands are read live), so two copies
        // drawn a moment apart differ whenever music plays on this machine: left out
        for kind in UPRIGHT.iter().filter(|k| !REDRAWN.contains(k) && **k != "spectrum") {
            let mut a = OnGpu::new(kind).expect("the original");
            let mut b = OnGpu::new(&format!("{kind}-up")).expect("its upright copy");
            let (mut worst, mut off) = (0u8, 0usize);
            for n in 0..48 {
                let t = n as f32 / 24.0;
                let input = Inputs {
                    t,
                    dt: 1.0 / 24.0,
                    keys: &keys,
                    down: (0.0, 1.0),
                    tint: rgb(default_tint(kind)).unwrap(),
                    strength: 0.6,
                    mask: &mask,
                    speed: 1.0,
                    density: 1.0,
                    wind: 0.0,
                    light: [240.0, -60.0, 120.0, 1.0],
                    icons: &mask,
                    rare: [0.5; 15],
                    moving: false,
                    left: false,
                };
                if n == 12 {
                    a.press(7, &input);
                    b.press(7, &input);
                }
                let (mut pa, mut pb) = (base.clone(), base.clone());
                for (e, p) in [(&mut a, &mut pa), (&mut b, &mut pb)] {
                    e.step(&input);
                    e.under(&input, p);
                    e.over(&input, p);
                }
                for (x, y) in pa.iter().zip(&pb) {
                    let d = x.abs_diff(*y);
                    worst = worst.max(d);
                    off += (d > 2) as usize;
                }
            }
            // the same sums in another order may round a level apart; nothing more
            assert!(off < W * H * 48 / 2000, "{kind}-up lying differs from {kind}: {off} values more than 2 apart, the worst {worst}");
        }
    }

    /// Layers drawn as one chain on the card give the very picture they give drawn one by one (one
    /// frame in 24 draws them one by one for the frame cost: a difference would blink once a
    /// second); skipped where there is no card (CI).
    #[test]
    fn a_chain_draws_as_one_by_one() {
        if crate::engine::gpu::get().is_none() {
            return;
        }
        let keys = places();
        let layers = ["scene", "mist", "pumpkins", "webs", "candle"].iter().map(|k| crate::engine::profile::Layer { kind: k.to_string(), ..Default::default() }).collect();
        let mut c = Canvas::new(&Effects { layers, ..Effects::default() }).expect("a canvas");
        c.step(&keys, (0.0, 1.0));
        // a panel of something to draw on: a gradient and a square of content
        let base: Vec<u8> = (0..W * H).flat_map(|i| [(i % W) as u8, (i / W) as u8, 90, 255]).collect();
        let mut content = vec![0u8; W * H * 4];
        for y in 30..60 {
            for x in 30..60 {
                content[(y * W + x) * 4..(y * W + x) * 4 + 4].copy_from_slice(&[255, 255, 255, 255]);
            }
        }
        c.content(&content);
        let draw = |c: &mut Canvas, alone: bool| {
            c.frames = if alone { 24 } else { 25 };
            let mut panel = base.clone();
            c.under(&mut panel);
            c.over(&mut panel);
            panel
        };
        let one_by_one = draw(&mut c, true);
        let chained = draw(&mut c, false);
        let differ = one_by_one.iter().zip(&chained).filter(|(a, b)| a != b).count();
        assert_eq!(differ, 0, "{differ} bytes differ between the chain and one by one");
    }

    /// A ripple rests when it has died out: no pictures at rest, so no CPU.
    #[test]
    fn a_ripple_rests() {
        let keys = places();
        let mut c = Canvas::new(&fx(None, Some("ripple"), None)).expect("ripple");
        assert!(!c.step(&keys, (0.0, 1.0)), "nothing to draw before a press");
        c.press(2);
        assert!(c.step(&keys, (0.0, 1.0)));
        let mut steps = 0;
        // a sixteenth of a second a frame, as in the engine (the pressed key's flash fades by time)
        loop {
            c.last -= std::time::Duration::from_micros(62_500);
            c.started -= std::time::Duration::from_micros(62_500);
            if !c.step(&keys, (0.0, 1.0)) {
                break;
            }
            steps += 1;
            assert!(steps < 400, "the ripple never settles");
        }
    }

    /// Each effect on a sample deck, frame by frame, as the engine draws it (backgrounds, the
    /// canvas, the keys' content; the bottom row's keys are opaque pictures with their own black
    /// background, as many of the owner's are): `HUBDECK_SHEET=<dir> cargo test -p hubdeck
    /// dump_canvas_frames -- --ignored` (`canvas-<name>.png`: shots in a grid of four, a press of
    /// key 7 at the start).
    #[test]
    #[ignore]
    fn dump_canvas_frames() {
        use crate::engine::profile::Look;
        use crate::engine::render::{Renderer, panel_tiles};
        use base64::Engine as _;
        let dir = std::path::PathBuf::from(std::env::var("HUBDECK_SHEET").expect("HUBDECK_SHEET"));
        std::fs::create_dir_all(&dir).unwrap();
        let s = |v: &str| Some(v.to_string());
        // an opaque picture: black with a coloured disc, no alpha
        let picture = |hue: [u8; 3]| {
            let img = image::RgbImage::from_fn(96, 96, |x, y| {
                let d = ((x as f32 - 48.0).powi(2) + (y as f32 - 44.0).powi(2)).sqrt();
                if d < 26.0 { image::Rgb(hue) } else { image::Rgb([0, 0, 0]) }
            });
            let mut png = std::io::Cursor::new(Vec::new());
            img.write_to(&mut png, image::ImageFormat::Png).unwrap();
            format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png.get_ref()))
        };
        let icons = ["heart", "star", "trash-2", "bookmark", "skip-forward", "cpu", "headphones", "music", "sparkles", "mic"];
        let mut looks: Vec<Look> = icons.iter().map(|i| Look { icon: s(i), label: s(i), bg: s("#1b1a20"), fg: s("#e6e4ee"), icon_color: s("#79ecc6"), t: 0.0001, ..Look::default() }).collect();
        for hue in [[230u8, 60, 60], [60, 160, 240], [240, 200, 40], [120, 220, 120], [200, 90, 230]] {
            looks.push(Look { image: Some(picture(hue)), fit: Some("fill".into()), label: s("pic"), t: 0.0001, ..Look::default() });
        }
        let geometry = PanelGeometry::default();
        let r = Renderer::default();
        let tiles = panel_tiles(&looks, 0, &geometry);
        let keys = places();
        // (effects, name, shots, steps of a sixteenth of a second between shots)
        let all = [
            (fx(None, Some("ripple"), None), "ripple", 8, 3),
            (fx(Some("rain"), None, None), "rain", 8, 3),
            (fx(Some("snow"), None, None), "snow", 8, 80),
            (fx(Some("fireflies"), None, None), "fireflies", 12, 6),
            (fx(Some("embers"), Some("embers"), None), "embers", 8, 3),
            (fx(Some("fog"), None, None), "fog", 8, 8),
            (fx(None, None, Some("bats")), "bats", 24, 10),
            (layers(&["water"]), "water", 8, 3),
            (layers(&["light"]), "light", 8, 6),
            (layers(&["scene"]), "scene", 4, 8),
            (layers(&["finish"]), "finish", 4, 8),
            (layers(&["scene", "light", "water", "finish"]), "all-gpu", 8, 4),
            // the walk's stack (living-deck.mjs): GPU layers among the CPU ones, grain at 80
            (
                Effects {
                    layers: ["water", "scene", "ripple", "bats", "snow", "light", "finish"]
                        .iter()
                        .map(|k| crate::engine::profile::Layer { kind: k.to_string(), density: (*k == "finish").then_some(80), ..Default::default() })
                        .collect(),
                    ..Effects::default()
                },
                "walk-stack",
                8,
                4,
            ),
        ];
        for (f, name, shots, gap) in all {
            let mut c = Canvas::new(&f).expect("an effect");
            c.step(&keys, (0.0, 1.0));
            c.press(7);
            let rows = (shots + 3) / 4;
            let mut sheet = image::RgbaImage::new(W as u32 * 4, H as u32 * rows as u32);
            for n in 0..shots {
                for _ in 0..gap {
                    c.last -= std::time::Duration::from_micros(62_500);
                    c.started -= std::time::Duration::from_micros(62_500);
                    c.step(&keys, (0.0, 1.0));
                    // the mask follows the content as in the engine (each picture sets it)
                    let _ = r.panel_pixmap_with(&tiles, Some(&mut c));
                }
                let pm = r.panel_pixmap_with(&tiles, Some(&mut c)).unwrap();
                let img = image::RgbaImage::from_raw(W as u32, H as u32, pm.data().to_vec()).unwrap();
                image::imageops::overlay(&mut sheet, &img, (n % 4) as i64 * W as i64, (n / 4) as i64 * H as i64);
                if n == shots - 1 {
                    img.save(dir.join(format!("canvas-{name}-last.png"))).unwrap();
                }
            }
            sheet.save(dir.join(format!("canvas-{name}.png"))).unwrap();
        }
    }

    /// A film of any layers on a sample deck at the deck's 24 pictures a second, with the numbers
    /// each new look is judged by (session 33): the CPU time a frame (the canvas and the panel),
    /// the JPEG bytes a frame at full quality, and the jitter test: a key whose picture moves a
    /// whole pixel between two frames (what the owner saw as keys jumping) is counted.
    ///
    /// `HUBDECK_FX=scene,light` (or `HUBDECK_FX_JSON` = an `Effects`), `HUBDECK_SHEET=<dir>`,
    /// optional `HUBDECK_SECONDS` (6), `HUBDECK_PRESSES=7@1.0,2@3.5`, `HUBDECK_SHADER_DIR` (WGSL
    /// to try): `cargo test --release -p hubdeck fx_film -- --ignored --nocapture`; then
    /// `ffmpeg -framerate 24 -i <dir>/%04d.png ... film.mp4`.
    #[test]
    #[ignore]
    fn fx_film() {
        use crate::engine::profile::{Layer, Look};
        use crate::engine::render::{KEY_QUALITY, Renderer, encode_jpeg, panel_tiles};
        use base64::Engine as _;
        let dir = std::path::PathBuf::from(std::env::var("HUBDECK_SHEET").expect("HUBDECK_SHEET"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let f: Effects = match std::env::var("HUBDECK_FX_JSON") {
            Ok(json) => serde_json::from_str(&json).expect("HUBDECK_FX_JSON"),
            Err(_) => Effects { layers: std::env::var("HUBDECK_FX").expect("HUBDECK_FX").split(',').map(|k| Layer { kind: k.trim().to_string(), ..Layer::default() }).collect(), ..Effects::default() },
        };
        let seconds: f32 = std::env::var("HUBDECK_SECONDS").ok().and_then(|v| v.parse().ok()).unwrap_or(6.0);
        // "7@1.0" a press of key 7 at 1 s, "like:7@1.0" a Like, "page@2.0" a page turn
        let presses: Vec<(String, f32)> = std::env::var("HUBDECK_PRESSES")
            .unwrap_or_default()
            .split(',')
            .filter_map(|p| {
                let (k, t) = p.split_once('@')?;
                Some((k.trim().to_string(), t.trim().parse().ok()?))
            })
            .collect();
        if let Ok(path) = std::env::var("HUBDECK_COVER") {
            crate::engine::images::set_cover(Some(("image/png", &std::fs::read(path).expect("HUBDECK_COVER"))));
        }
        // a deck like the owner's: line icons on dark keys, two app logos as pictures with alpha,
        // and opaque pictures with their own black background on the bottom row
        let s = |v: &str| Some(v.to_string());
        let png = |img: image::RgbaImage| {
            let mut out = std::io::Cursor::new(Vec::new());
            img.write_to(&mut out, image::ImageFormat::Png).unwrap();
            format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(out.get_ref()))
        };
        let logo = |a: [u8; 3], b: [u8; 3]| {
            png(image::RgbaImage::from_fn(96, 96, |x, y| {
                let (dx, dy) = (x as f32 - 48.0, y as f32 - 48.0);
                let d = (dx * dx + dy * dy).sqrt();
                let t = ((dy.atan2(dx) + std::f32::consts::PI) / std::f32::consts::TAU).clamp(0.0, 1.0);
                let c = [0, 1, 2].map(|i| (a[i] as f32 * (1.0 - t) + b[i] as f32 * t) as u8);
                image::Rgba([c[0], c[1], c[2], ((36.0 - d) * 255.0).clamp(0.0, 255.0) as u8])
            }))
        };
        let opaque = |hue: [u8; 3]| {
            png(image::RgbaImage::from_fn(96, 96, |x, y| {
                let d = ((x as f32 - 48.0).powi(2) + (y as f32 - 44.0).powi(2)).sqrt();
                if d < 26.0 { image::Rgba([hue[0], hue[1], hue[2], 255]) } else { image::Rgba([0, 0, 0, 255]) }
            }))
        };
        let icons = ["folder", "", "", "music", "play", "heart", "star", "trash-2", "bookmark", "skip-forward"];
        let mut looks: Vec<Look> = icons
            .iter()
            .map(|i| Look { icon: s(i), label: s(if i.is_empty() { "app" } else { i }), bg: s("#1b1a20"), fg: s("#e6e4ee"), icon_color: s("#79ecc6"), t: 0.0001, ..Look::default() })
            .collect();
        looks[1] = Look { icon: None, image: Some(logo([255, 120, 20], [120, 40, 200])), label: s("Firefox"), bg: s("#1b1a20"), fg: s("#e6e4ee"), t: 0.0001, ..Look::default() };
        looks[2] = Look { icon: None, image: Some(logo([60, 200, 120], [40, 120, 230])), label: s("Slack"), bg: s("#1b1a20"), fg: s("#e6e4ee"), t: 0.0001, ..Look::default() };
        for hue in [[230u8, 60, 60], [60, 160, 240], [240, 200, 40], [120, 220, 120], [200, 90, 230]] {
            looks.push(Look { image: Some(opaque(hue)), fit: Some("fill".into()), label: s("pic"), t: 0.0001, ..Look::default() });
        }
        // the owner's real page instead of the made-up one (his "1 a", 2026-10-29): the main page's keys
        // of a profile file, their pictures from the images folder beside its profiles folder
        if let Ok(path) = std::env::var("HUBDECK_PROFILE") {
            let p = std::path::PathBuf::from(&path);
            if let Some(data) = p.parent().and_then(|d| d.parent()) {
                crate::engine::images::init(data);
            }
            let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&p).expect("HUBDECK_PROFILE")).expect("a profile");
            looks = (0..15)
                .map(|i| {
                    let mut look: Look = serde_json::from_value(v["controls"][format!("keys.{i}")]["look"].clone()).unwrap_or_default();
                    if look.bg.is_none() {
                        look.bg = s("#1b1a20");
                    }
                    look.t = 0.0001;
                    look
                })
                .collect();
        }
        // the bare scene: keys with no icon, label or picture (owner, 2026-10-29: "Pokażesz mi gołe tła?
        // Bez ikon i ramek?")
        if std::env::var("HUBDECK_BARE").is_ok() {
            looks = (0..15).map(|_| Look { bg: s("#1b1a20"), t: 0.0001, ..Look::default() }).collect();
        }
        let geometry = PanelGeometry::default();
        let r = Renderer::default();
        // the deck's turn (0, 90, 180, 270): 90 and 270 stand it upright (owner, 2026-10-29)
        let turn: i32 = std::env::var("HUBDECK_TURN").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
        let tiles = panel_tiles(&looks, turn, &geometry);
        let keys = places();
        let mut c = Canvas::new(&f).expect("an effect");
        let frames = (seconds * 24.0).round() as usize;
        let tick = std::time::Duration::from_micros(41_667);
        let (mut cpu, mut bytes, mut moving) = (0f64, 0usize, 0usize);
        let mut jumps = [0usize; 15];
        let mut prev: Option<Vec<u8>> = None;
        let grey = |d: &[u8], x: usize, y: usize| {
            let i = (y * W + x) * 4;
            d[i] as f32 * 0.299 + d[i + 1] as f32 * 0.587 + d[i + 2] as f32 * 0.114
        };
        for n in 0..frames {
            let t = n as f32 / 24.0;
            for (k, at) in &presses {
                if (*at - t).abs() < 0.5 / 24.0 {
                    match k.split_once(':') {
                        Some(("like", key)) => {
                            let key = key.parse().unwrap_or(5);
                            c.press(key);
                            c.like(key);
                        }
                        _ if k == "page" => c.page_turned(),
                        _ => c.press(k.parse().unwrap_or(7)),
                    }
                }
            }
            c.last -= tick;
            c.started -= tick;
            let started = std::time::Instant::now();
            c.step(&keys, down_for(turn));
            let pm = r.panel_pixmap_with(&tiles, Some(&mut c)).unwrap();
            cpu += started.elapsed().as_secs_f64() * 1000.0;
            bytes += encode_jpeg(&pm, KEY_QUALITY).unwrap().len();
            let now = pm.data().to_vec();
            if let Some(old) = &prev {
                if old != &now {
                    moving += 1;
                }
                // the jitter test: a key's window that matches the last frame clearly better moved a
                // pixel than in place has jumped
                for (k, p) in keys.iter().enumerate() {
                    let (x0, y0) = (p.x as usize + 2, p.y as usize + 2);
                    let (x1, y1) = ((p.x + p.w) as usize - 2, ((p.y + p.h) as usize - 2).min(H - 1));
                    let diff = |sx: i32, sy: i32| {
                        let mut d = 0.0f32;
                        for y in y0..y1 {
                            for x in x0..x1 {
                                let (ox, oy) = ((x as i32 + sx) as usize, (y as i32 + sy) as usize);
                                d += (grey(&now, x, y) - grey(old, ox, oy)).abs();
                            }
                        }
                        d / ((x1 - x0) * (y1 - y0)) as f32
                    };
                    let still = diff(0, 0);
                    let shifted = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().map(|(a, b)| diff(*a, *b)).fold(f32::MAX, f32::min);
                    if still > 3.0 && shifted < still * 0.5 {
                        jumps[k] += 1;
                    }
                }
            }
            image::RgbaImage::from_raw(W as u32, H as u32, now.clone()).unwrap().save(dir.join(format!("{n:04}.png"))).unwrap();
            prev = Some(now);
        }
        let report = format!(
            "{} frames at 24 a second; CPU {:.2} ms a frame (canvas and panel); JPEG {:.1} KB a frame at q{}; a new picture in {} of {} frames; keys that jumped a pixel: {:?}",
            frames,
            cpu / frames as f64,
            bytes as f64 / frames as f64 / 1024.0,
            KEY_QUALITY,
            moving,
            frames.saturating_sub(1),
            jumps
        );
        println!("{report}");
        std::fs::write(dir.join("stats.txt"), report).unwrap();
    }

    /// Where a frame's CPU goes on the owner's own keys (session 33, the CPU budget): his active
    /// profile's main page read from the app's data (read only), the living deck's layers from
    /// `HUBDECK_FX` (default the night sky); per frame: making the tiles, painting (tiles and
    /// layers), finding the changed keys, the keys' JPEGs and the whole panel's JPEG, in ms:
    /// `cargo test --release -p hubdeck frame_cost -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn frame_cost() {
        use crate::engine::profile::{Layer, Profile};
        use crate::engine::render::{KEY_QUALITY, Renderer, changed_keys, encode_jpeg, key_slot_jpeg, panel_tiles};
        use std::time::Instant;
        let data = std::path::PathBuf::from(std::env::var("APPDATA").unwrap()).join("dev.hubdeck.app");
        crate::engine::images::init(&data);
        let settings: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(data.join("settings.json")).unwrap()).unwrap();
        let id = settings["active"].as_str().unwrap();
        let profile: Profile = serde_json::from_str(&std::fs::read_to_string(data.join("profiles").join(format!("{id}.json"))).unwrap()).unwrap();
        let mut looks: Vec<crate::engine::profile::Look> = (0..15).map(|i| profile.controls.get(&format!("keys.{i}")).map(|b| b.look_for(None)).unwrap_or_default()).collect();
        for l in &mut looks {
            l.t = l.t.max(0.0001);
        }
        let kinds = std::env::var("HUBDECK_FX").unwrap_or_else(|_| "scene".into());
        let f = Effects { layers: kinds.split(',').map(|k| Layer { kind: k.trim().to_string(), ..Layer::default() }).collect(), ..Effects::default() };
        let geometry = PanelGeometry::default();
        let r = Renderer::default();
        let keys = places();
        let mut c = Canvas::new(&f).expect("an effect");
        let tick = std::time::Duration::from_micros(41_667);
        let (mut t_tiles, mut t_paint, mut t_diff, mut t_keys, mut t_whole, mut n_keys) = (0f64, 0f64, 0f64, 0f64, 0f64, 0usize);
        let mut old: Option<Vec<u8>> = None;
        let frames = 96;
        let ms = |t: Instant| t.elapsed().as_secs_f64() * 1000.0;
        for _ in 0..frames {
            c.last -= tick;
            c.started -= tick;
            c.step(&keys, (0.0, 1.0));
            let t = Instant::now();
            let tiles = panel_tiles(&looks, 0, &geometry);
            t_tiles += ms(t);
            let t = Instant::now();
            let pm = r.panel_pixmap_with(&tiles, Some(&mut c)).unwrap();
            t_paint += ms(t);
            let t = Instant::now();
            let changed = old.as_ref().and_then(|o| changed_keys(o, pm.data(), &geometry));
            t_diff += ms(t);
            match changed {
                Some(list) => {
                    let t = Instant::now();
                    for i in &list {
                        key_slot_jpeg(&pm, *i).unwrap();
                    }
                    t_keys += ms(t);
                    n_keys += 1;
                }
                None => {
                    let t = Instant::now();
                    encode_jpeg(&pm, KEY_QUALITY).unwrap();
                    t_whole += ms(t);
                }
            }
            old = Some(pm.data().to_vec());
        }
        let n = frames as f64;
        println!(
            "{kinds} on {}: tiles {:.2} ms, painting {:.2}, changed keys {:.2}, keys' JPEGs {:.2} ({} frames), whole JPEG {:.2} ({} frames) a frame",
            profile.name,
            t_tiles / n,
            t_paint / n,
            t_diff / n,
            t_keys / n_keys.max(1) as f64,
            n_keys,
            t_whole / (frames - n_keys).max(1) as f64,
            frames - n_keys
        );
        // painting split: the tiles alone (no layers)
        let t = Instant::now();
        let tiles = panel_tiles(&looks, 0, &geometry);
        for _ in 0..20 {
            r.panel_pixmap_with(&tiles, None).unwrap();
        }
        println!("painting the tiles alone: {:.2} ms", ms(t) / 20.0);
        // hashing the tiles' SVGs (the cache's key each frame)
        let t = Instant::now();
        let mut total = 0usize;
        for _ in 0..20 {
            for tile in &tiles {
                use std::hash::{Hash, Hasher};
                let mut h = std::collections::hash_map::DefaultHasher::new();
                tile.svg.hash(&mut h);
                for l in &tile.layers {
                    l.hash(&mut h);
                    total += l.len();
                }
                total += tile.svg.len();
                std::hint::black_box(h.finish());
            }
        }
        println!("hashing the tiles' SVGs: {:.2} ms ({} KB of SVG a frame)", ms(t) / 20.0, total / 20 / 1024);
    }

    /// Halloween theme variants for the owner's pick, as frames at 16 a second (3 presses along the
    /// way): `HUBDECK_SHEET=<dir> cargo test --release -p hubdeck dump_halloween -- --ignored`
    /// (`halloween-<variant>/NNN.png`).
    #[test]
    #[ignore]
    fn dump_halloween() {
        use crate::engine::profile::Look;
        use crate::engine::render::{Renderer, panel_tiles};
        let dir = std::path::PathBuf::from(std::env::var("HUBDECK_SHEET").expect("HUBDECK_SHEET"));
        let s = |v: &str| Some(v.to_string());
        let icons = ["music", "skip-forward", "headphones", "mic", "globe", "folder", "cpu", "moon", "star", "heart", "play", "bookmark", "zap", "sparkles", "trash-2"];
        let looks: Vec<Look> = icons.iter().map(|i| Look { icon: s(i), label: s(i), bg: s("#1b1a20"), fg: s("#e6e4ee"), icon_color: s("#f2b45a"), t: 0.0001, ..Look::default() }).collect();
        let r = Renderer::default();
        let tiles = panel_tiles(&looks, 0, &PanelGeometry::default());
        let keys = places();
        let tinted = |mut f: Effects, tint: &str| {
            f.tint = Some(tint.to_string());
            f
        };
        let variants = [
            ("a-fog", tinted(fx(Some("fog"), Some("embers"), Some("bats")), "#7a4fc0")),
            ("b-fire", tinted(fx(Some("embers"), Some("ripple"), Some("bats")), "#ff7a1a")),
            ("c-wisps", tinted(fx(Some("fireflies"), Some("embers"), Some("bats")), "#ffb04a")),
        ];
        for (name, f) in variants {
            let out = dir.join(format!("halloween-{name}"));
            std::fs::create_dir_all(&out).unwrap();
            let mut c = Canvas::new(&f).expect("an effect");
            // a few seconds in, so the bats and the fog are under way
            for _ in 0..40 {
                c.last -= std::time::Duration::from_micros(62_500);
                c.started -= std::time::Duration::from_micros(62_500);
                c.step(&keys, (0.0, 1.0));
                let _ = r.panel_pixmap_with(&tiles, Some(&mut c));
            }
            for n in 0..160 {
                if n == 20 || n == 75 || n == 120 {
                    c.press([7, 2, 11][n / 50 % 3]);
                }
                c.last -= std::time::Duration::from_micros(62_500);
                c.started -= std::time::Duration::from_micros(62_500);
                c.step(&keys, (0.0, 1.0));
                let pm = r.panel_pixmap_with(&tiles, Some(&mut c)).unwrap();
                let img = image::RgbaImage::from_raw(W as u32, H as u32, pm.data().to_vec()).unwrap();
                img.save(out.join(format!("{n:03}.png"))).unwrap();
            }
        }
    }

    /// The screensaver's scenes on black keys (`saver.rs`), frames 3 and 8 s in, and what a frame
    /// costs: `HUBDECK_SHEET=<dir> cargo test --release -p hubdeck dump_saver_scenes -- --ignored
    /// --nocapture` (`saver-<scene>-<s>s.png`).
    #[test]
    #[ignore]
    fn dump_saver_scenes() {
        use crate::engine::profile::Look;
        use crate::engine::render::{Renderer, panel_tiles};
        let dir = std::path::PathBuf::from(std::env::var("HUBDECK_SHEET").expect("HUBDECK_SHEET"));
        std::fs::create_dir_all(&dir).unwrap();
        let looks = vec![Look { bg: Some("#000000".into()), t: 0.0001, ..Look::default() }; 15];
        let r = Renderer::default();
        let tiles = panel_tiles(&looks, 0, &PanelGeometry::default());
        let keys = places();
        for (id, _) in crate::engine::saver::SCENES {
            let Some(f) = crate::engine::saver::scene(id) else { continue };
            let Some(mut c) = Canvas::new(&f) else {
                println!("{id}: no canvas (no graphics card?)");
                continue;
            };
            let mut spent = std::time::Duration::ZERO;
            for n in 1..=(8 * 24) {
                c.last -= std::time::Duration::from_micros(41_667);
                c.started -= std::time::Duration::from_micros(41_667);
                let t0 = std::time::Instant::now();
                c.step(&keys, (0.0, 1.0));
                let pm = r.panel_pixmap_with(&tiles, Some(&mut c)).unwrap();
                spent += t0.elapsed();
                if n == 3 * 24 || n == 8 * 24 {
                    let img = image::RgbaImage::from_raw(W as u32, H as u32, pm.data().to_vec()).unwrap();
                    img.save(dir.join(format!("saver-{id}-{}s.png", n / 24))).unwrap();
                }
            }
            println!("{id}: {:.2} ms a frame", spent.as_secs_f64() * 1000.0 / (8.0 * 24.0));
        }
    }

    /// How many bytes a frame of each effect takes over USB at full quality: the whole panel and the
    /// keys' squares one by one (2026-10-26: the hidden gaps kept still saved 5 % on snow and cost
    /// 5 % on fog, so they are drawn as they come):
    /// `cargo test --release -p hubdeck canvas_bytes -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn canvas_bytes() {
        use crate::engine::profile::Look;
        use crate::engine::render::{KEY_QUALITY, Renderer, encode_jpeg, key_slot_jpeg, panel_tiles};
        let s = |v: &str| Some(v.to_string());
        let icons = ["music", "skip-forward", "headphones", "mic", "globe", "folder", "cpu", "moon", "star", "heart", "play", "bookmark", "zap", "sparkles", "trash-2"];
        let looks: Vec<Look> = icons.iter().map(|i| Look { icon: s(i), label: s(i), bg: s("#1b1a20"), fg: s("#e6e4ee"), icon_color: s("#79ecc6"), t: 0.0001, ..Look::default() }).collect();
        let g = PanelGeometry::default();
        let r = Renderer::default();
        let tiles = panel_tiles(&looks, 0, &g);
        let keys = places();
        for f in [
            fx(None, None, None),
            fx(Some("snow"), Some("ripple"), Some("bats")),
            fx(Some("fog"), None, None),
            fx(Some("rain"), None, None),
            fx(Some("embers"), None, None),
            fx(Some("fireflies"), None, None),
        ] {
            let (mut whole, mut slots, mut n) = (0usize, 0usize, 0usize);
            let mut c = Canvas::new(&f);
            for step in 0..64 {
                let pm = match c.as_mut() {
                    Some(c) => {
                        c.last -= std::time::Duration::from_micros(62_500);
                        c.started -= std::time::Duration::from_micros(62_500);
                        c.step(&keys, (0.0, 1.0));
                        if step == 20 {
                            c.press(7);
                        }
                        r.panel_pixmap_with(&tiles, Some(c)).unwrap()
                    }
                    None => r.panel_pixmap_with(&tiles, None).unwrap(),
                };
                if step < 32 {
                    continue;
                }
                whole += encode_jpeg(&pm, KEY_QUALITY).unwrap().len();
                slots += (0..15).map(|i| key_slot_jpeg(&pm, i).unwrap().len()).sum::<usize>();
                n += 1;
            }
            println!("{:<44} whole {:>6} B, 15 squares {:>6} B", format!("{:?}", f.stack().iter().map(|l| l.kind.clone()).collect::<Vec<_>>()), whole / n, slots / n);
        }
    }

    /// What each effect costs on the whole panel, a frame (release build):
    /// `cargo test --release -p hubdeck canvas_cost -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn canvas_cost() {
        let keys = places();
        for f in CANVASES
            .iter()
            .map(|c| fx(Some(c), None, None))
            .chain(REACTIONS.iter().map(|r| fx(None, Some(r), None)))
            .chain(INHABITANTS.iter().map(|i| fx(None, None, Some(i))))
            .chain(GPU_KINDS.iter().map(|k| layers(&[k])))
        {
            let Some(mut c) = Canvas::new(&f) else { continue };
            let mut panel = vec![30u8; W * H * 4];
            let start = std::time::Instant::now();
            for n in 0..200 {
                if n % 40 == 0 {
                    c.press(n % 15);
                }
                c.step(&keys, (0.0, 1.0));
                c.under(&mut panel);
                c.over(&mut panel);
            }
            let ms = start.elapsed().as_secs_f64() * 1000.0 / 200.0;
            println!("{:<40} {ms:.3} ms a frame, {:.1} % of a core at 16 a second", format!("{:?}", f.stack().iter().map(|l| l.kind.clone()).collect::<Vec<_>>()), ms * 1.6);
        }
    }
}
