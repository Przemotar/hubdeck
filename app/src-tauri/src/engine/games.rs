//! Game mode: the deck becomes a small console (`docs/GAMES.md`). A launcher puts one game on each
//! key; a game owns the keys, the dial screen and the knob and fader LEDs until it ends. Holding
//! the dial for a second goes back one level (game -> launcher -> the profile).
//!
//! Games are pure state machines: they get inputs and the time, and describe what the screens
//! show. The engine draws that, so the logic is tested without hardware.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use base64::Engine as _;
use mh_protocol::report::Rgb;

use super::profile::Look;

/// Hold the dial this long to leave a game (or the launcher).
const LEAVE_AFTER: Duration = Duration::from_millis(1000);
/// A dial press shorter than this is a click for the game.
const CLICK_UNDER: Duration = Duration::from_millis(600);
pub const KEYS: usize = 15;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Input {
    Key {
        index: usize,
        down: bool,
    },
    /// The dial turned (detents, + clockwise).
    Dial(i8),
    /// A short dial press.
    DialClick,
    /// A knob or roller turned: (which, detents).
    Knob(usize, i8),
    Roller(usize, i8),
    Fader(usize, u8),
}

/// What the deck shows for one frame.
#[derive(Debug, Clone, PartialEq)]
pub struct Screen {
    pub keys: Vec<Look>,
    /// A picture for the whole 480x272 key panel instead of the key faces (SVG content).
    pub panel: Option<String>,
    /// SVG content for the 240x240 dial (no `<svg>` wrapper; the engine turns it upright).
    pub dial: String,
    pub knob_leds: Option<Vec<Rgb>>,
    pub fader_leds: Option<Vec<Rgb>>,
}

pub trait Game: Send {
    fn input(&mut self, input: Input, now: Instant);
    fn tick(&mut self, now: Instant);
    fn screen(&self, now: Instant) -> Screen;
    /// Some(score) once the game is over.
    fn over(&self) -> Option<u32>;
}

/// Game mode is switched off for now (owner, 2026-10-11: features first, games polished later;
/// `docs/GAMES.md` > "Polish plan"). Off hides the tray item, the Games action and the Games tab
/// (`GAMES_ENABLED` in `src/engine.ts`), and the engine refuses to start game mode.
pub const GAMES_ENABLED: bool = false;

/// The games on offer, in launcher order: (id, name, one line, icon, constructor).
pub struct Entry {
    pub id: &'static str,
    pub name: &'static str,
    pub about: &'static str,
    pub icon: &'static str,
    /// Module kinds the game cannot do without.
    pub needs: &'static [&'static str],
    pub make: fn(u64, Instant) -> Box<dyn Game>,
}

pub fn catalog() -> Vec<Entry> {
    vec![
        Entry { id: "mole", name: "Mole", about: "Hit the moles before they hide. Three misses and it is over.", icon: "zap", needs: &["keys"], make: |seed, now| Box::new(Mole::new(seed, now)) },
        Entry {
            id: "simon",
            name: "Simon",
            about: "Watch the keys light up, then play the sequence back. It grows every round.",
            icon: "sparkles",
            needs: &["keys"],
            make: |seed, now| Box::new(Simon::new(seed, now)),
        },
        Entry {
            id: "safe",
            name: "Safe",
            about: "Crack the combination with the dial: find each number by the glow, push to lock it in.",
            icon: "lock",
            needs: &["dial"],
            make: |seed, now| Box::new(Safe::new(seed, now)),
        },
        Entry {
            id: "snake",
            name: "Snake",
            about: "The whole key panel is the field, seen through the key windows. Steer with the dial; eat, grow, do not hit a wall.",
            icon: "git-branch",
            needs: &["keys", "dial"],
            make: |seed, now| Box::new(Snake::new(seed, now)),
        },
        Entry {
            id: "arena",
            name: "Arena",
            about: "Defend the core on the round screen: turn the dial to move the shield into whatever flies in.",
            icon: "circle-dot",
            needs: &["dial"],
            make: |seed, now| Box::new(Arena::new(seed, now)),
        },
        Entry {
            id: "rhythm",
            name: "Rhythm",
            about: "Notes come down the key columns; hit each one as it lands on the bottom row, on the beat.",
            icon: "music",
            needs: &["keys"],
            make: |seed, now| Box::new(Rhythm::new(seed, now)),
        },
        Entry {
            id: "pong",
            name: "Pong",
            about: "The whole key panel is the court. Your paddle is on the left, moved by the dial; turn a knob or roller and a second player takes the right one. First to 7.",
            icon: "gamepad-2",
            needs: &["keys", "dial"],
            make: |seed, now| Box::new(Pong::new(seed, now)),
        },
        Entry {
            id: "duel",
            name: "Duel",
            about: "Two players: left keys against right keys. Hit your lit key first; a press before it lights costs a point.",
            icon: "zap",
            needs: &["keys"],
            make: |seed, now| Box::new(Duel::new(seed, now)),
        },
        Entry {
            id: "wheel",
            name: "Wheel",
            about: "Spin the dial and let the round screen decide: lunch, who pays, what to play.",
            icon: "refresh-cw",
            needs: &["dial"],
            make: |seed, now| Box::new(Wheel::new(seed, now)),
        },
        Entry {
            id: "memory",
            name: "Memory",
            about: "Seven pairs of pictures hide under the keys; find them in as few moves as you can.",
            icon: "layers",
            needs: &["keys"],
            make: |seed, now| Box::new(Memory::new(seed, now)),
        },
        Entry {
            id: "reflex",
            name: "Reflex",
            about: "A key lights up at a random moment: press it as fast as you can. Five rounds, your average in milliseconds.",
            icon: "timer",
            needs: &["keys"],
            make: |seed, now| Box::new(Reflex::new(seed, now)),
        },
        Entry {
            id: "balance",
            name: "Balance",
            about: "Keep all five faders on their moving targets; the lit keys and the fader lights show where.",
            icon: "arrow-up-down",
            needs: &["faders"],
            make: |seed, now| Box::new(Balance::new(seed, now)),
        },
    ]
}

// ---------- small helpers ----------

/// xorshift64: enough randomness for games, and a seed makes tests repeatable.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.max(1))
    }
    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
}

fn svg_uri(svg: &str) -> String {
    format!("data:image/svg+xml;base64,{}", base64::engine::general_purpose::STANDARD.encode(svg))
}

fn key(label: &str, bg: &str) -> Look {
    Look { label: (!label.is_empty()).then(|| label.to_string()), bg: Some(bg.into()), ..Look::default() }
}

fn key_icon(icon: &str, label: &str, bg: &str) -> Look {
    Look { icon: Some(icon.into()), ..key(label, bg) }
}

fn blank() -> Look {
    key("", "#000000")
}

fn esc(t: &str) -> String {
    t.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn text(x: f32, y: f32, size: f32, fill: &str, t: &str) -> String {
    format!(r#"<text x="{x}" y="{y}" font-family="{}" font-weight="600" font-size="{size}" fill="{fill}" text-anchor="middle">{}</text>"#, super::render::font_family(), esc(t))
}

/// A ring from 12 o'clock filled to `frac`, for timers and lives.
fn ring(frac: f32, color: &str) -> String {
    let (c, r) = (120.0f32, 108.0f32);
    let frac = frac.clamp(0.0, 1.0);
    let mut out = format!(r##"<circle cx="{c}" cy="{c}" r="{r}" fill="none" stroke="#2a2733" stroke-width="10"/>"##);
    if frac > 0.0 {
        let angle = frac * 359.9;
        let rad = (angle - 90.0f32).to_radians();
        let (ex, ey) = (c + r * rad.cos(), c + r * rad.sin());
        let large = if angle > 180.0 { 1 } else { 0 };
        out += &format!(r#"<path d="M{c} {} A{r} {r} 0 {large} 1 {ex:.2} {ey:.2}" fill="none" stroke="{color}" stroke-width="10" stroke-linecap="round"/>"#, c - r);
    }
    out
}

fn dial_message(title: &str, line: &str, hint: &str) -> String {
    format!(
        r##"<rect width="240" height="240" fill="#0d0c12"/>{}{}{}"##,
        text(120.0, 118.0, 36.0, "#f0eef6", title),
        text(120.0, 152.0, 20.0, "#c4b5fd", line),
        text(120.0, 196.0, 14.0, "#8a8599", hint)
    )
}

const MOLE_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 72 72"><rect width="72" height="72" fill="#3b2a17"/><ellipse cx="36" cy="66" rx="30" ry="9" fill="#1c140b"/><ellipse cx="36" cy="44" rx="20" ry="22" fill="#8a6239"/><circle cx="28" cy="38" r="3.4" fill="#111"/><circle cx="44" cy="38" r="3.4" fill="#111"/><ellipse cx="36" cy="47" rx="6" ry="4.5" fill="#f2a0a8"/><rect x="31" y="51" width="4" height="5" fill="#fff"/><rect x="37" y="51" width="4" height="5" fill="#fff"/></svg>"##;
const HOLE_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 72 72"><rect width="72" height="72" fill="#1a140c"/><ellipse cx="36" cy="58" rx="26" ry="9" fill="#050403"/></svg>"##;
const BONK_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 72 72"><rect width="72" height="72" fill="#3b2a17"/><path d="M36 8 L43 28 L64 28 L47 40 L54 62 L36 49 L18 62 L25 40 L8 28 L29 28 Z" fill="#f2c14e"/></svg>"##;

// ---------- Mole ----------

pub struct Mole {
    rng: Rng,
    /// Until when a mole shows under each key.
    up: [Option<Instant>; KEYS],
    /// When a key was last hit (shows a star for a moment).
    hit: [Option<Instant>; KEYS],
    score: u32,
    misses: u32,
    next_spawn: Instant,
    start: Instant,
}

const MOLE_LIVES: u32 = 3;

impl Mole {
    pub fn new(seed: u64, now: Instant) -> Self {
        Mole { rng: Rng::new(seed), up: [None; KEYS], hit: [None; KEYS], score: 0, misses: 0, next_spawn: now + Duration::from_millis(800), start: now }
    }

    /// Faster as the score grows: (time between moles, time a mole stays up).
    fn pace(&self) -> (Duration, Duration) {
        let s = self.score.min(40) as u64;
        (Duration::from_millis(900 - s * 14), Duration::from_millis(1400 - s * 20))
    }
}

impl Game for Mole {
    fn input(&mut self, input: Input, now: Instant) {
        if self.over().is_some() {
            return;
        }
        if let Input::Key { index, down: true } = input
            && index < KEYS
            && self.up[index].is_some_and(|until| now < until)
        {
            self.up[index] = None;
            self.hit[index] = Some(now);
            self.score += 1;
        }
    }

    fn tick(&mut self, now: Instant) {
        if self.over().is_some() {
            return;
        }
        for slot in self.up.iter_mut() {
            if slot.is_some_and(|until| now >= until) {
                *slot = None;
                self.misses += 1;
            }
        }
        if now >= self.next_spawn {
            let (every, stay) = self.pace();
            let free: Vec<usize> = (0..KEYS).filter(|i| self.up[*i].is_none()).collect();
            if !free.is_empty() {
                let i = free[self.rng.below(free.len() as u64) as usize];
                self.up[i] = Some(now + stay);
            }
            self.next_spawn = now + every;
        }
    }

    fn screen(&self, now: Instant) -> Screen {
        let keys = (0..KEYS)
            .map(|i| {
                let image = if self.hit[i].is_some_and(|t| now.duration_since(t) < Duration::from_millis(180)) {
                    BONK_SVG
                } else if self.up[i].is_some() {
                    MOLE_SVG
                } else {
                    HOLE_SVG
                };
                Look { image: Some(svg_uri(image)), bg: Some("#1a140c".into()), ..Look::default() }
            })
            .collect();
        let lives_left = MOLE_LIVES.saturating_sub(self.misses);
        let dial = format!(
            r##"<rect width="240" height="240" fill="#0d0c12"/>{}{}{}{}"##,
            ring(lives_left as f32 / MOLE_LIVES as f32, "#f2c14e"),
            text(120.0, 140.0, 64.0, "#f0eef6", &self.score.to_string()),
            text(120.0, 176.0, 16.0, "#a8a4b5", &format!("{} {}", lives_left, if lives_left == 1 { "life" } else { "lives" })),
            text(120.0, 62.0, 14.0, "#8a8599", &format!("{}s", now.duration_since(self.start).as_secs()))
        );
        let led = if lives_left == 1 { [255, 40, 30] } else { [240, 180, 40] };
        Screen { keys, panel: None, dial, knob_leds: Some((0..3).map(|i| if (i as u32) < lives_left { led } else { [0, 0, 0] }).collect()), fader_leds: None }
    }

    fn over(&self) -> Option<u32> {
        (self.misses >= MOLE_LIVES).then_some(self.score)
    }
}

// ---------- Simon ----------

/// One color per key, bright when lit.
fn simon_color(i: usize, lit: bool) -> String {
    let hue = (i as f32 * 360.0 / KEYS as f32) as u32;
    let (s, l) = if lit { (90, 62) } else { (35, 16) };
    hsl(hue, s, l)
}

fn hsl(h: u32, s: u32, l: u32) -> String {
    let (h, s, l) = (h as f32 / 360.0, s as f32 / 100.0, l as f32 / 100.0);
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let f = |t: f32| {
        let t = (t + 1.0) % 1.0;
        let v = if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        };
        (v * 255.0).round() as u8
    };
    format!("#{:02x}{:02x}{:02x}", f(h + 1.0 / 3.0), f(h), f(h - 1.0 / 3.0))
}

fn hex_rgb(hex: &str) -> Rgb {
    super::leds::parse_hex(hex).unwrap_or([0, 0, 0])
}

enum SimonPhase {
    /// Playing the sequence back to the player: step index and when it started.
    Showing {
        step: usize,
        since: Instant,
    },
    /// The player repeats: how many are right so far.
    Repeat {
        pos: usize,
    },
    /// Got the round right; next one starts at the instant.
    Pause(Instant),
    Lost,
}

pub struct Simon {
    rng: Rng,
    seq: Vec<usize>,
    phase: SimonPhase,
    /// The key the player pressed last and when (lights it briefly).
    pressed: Option<(usize, Instant)>,
    wrong: Option<usize>,
}

impl Simon {
    pub fn new(seed: u64, now: Instant) -> Self {
        let mut rng = Rng::new(seed);
        let first = rng.below(KEYS as u64) as usize;
        Simon { rng, seq: vec![first], phase: SimonPhase::Showing { step: 0, since: now + Duration::from_millis(600) }, pressed: None, wrong: None }
    }

    /// How long each shown step lights and the gap after it; quicker in long rounds.
    fn step_time(&self) -> (Duration, Duration) {
        let n = self.seq.len().min(15) as u64;
        (Duration::from_millis(560 - n * 20), Duration::from_millis(200 - n * 6))
    }

    fn lit(&self, now: Instant) -> Option<usize> {
        match self.phase {
            SimonPhase::Showing { step, since } if now >= since => {
                let (on, _) = self.step_time();
                (now.duration_since(since) < on).then(|| self.seq[step])
            }
            _ => self.pressed.filter(|(_, t)| now.duration_since(*t) < Duration::from_millis(250)).map(|(k, _)| k),
        }
    }
}

impl Game for Simon {
    fn input(&mut self, input: Input, now: Instant) {
        let Input::Key { index, down: true } = input else { return };
        let SimonPhase::Repeat { pos } = self.phase else { return };
        if index >= KEYS {
            return;
        }
        self.pressed = Some((index, now));
        if self.seq[pos] != index {
            self.wrong = Some(index);
            self.phase = SimonPhase::Lost;
        } else if pos + 1 == self.seq.len() {
            self.phase = SimonPhase::Pause(now + Duration::from_millis(700));
        } else {
            self.phase = SimonPhase::Repeat { pos: pos + 1 };
        }
    }

    fn tick(&mut self, now: Instant) {
        match self.phase {
            SimonPhase::Showing { step, since } if now >= since => {
                let (on, gap) = self.step_time();
                if now.duration_since(since) >= on + gap {
                    self.phase = if step + 1 < self.seq.len() { SimonPhase::Showing { step: step + 1, since: now } } else { SimonPhase::Repeat { pos: 0 } };
                }
            }
            SimonPhase::Pause(at) if now >= at => {
                let next = self.rng.below(KEYS as u64) as usize;
                self.seq.push(next);
                self.phase = SimonPhase::Showing { step: 0, since: now };
            }
            _ => {}
        }
    }

    fn screen(&self, now: Instant) -> Screen {
        let lit = self.lit(now);
        let lost = matches!(self.phase, SimonPhase::Lost);
        let keys = (0..KEYS)
            .map(|i| match (lost, self.wrong == Some(i), self.seq.last() == Some(&i)) {
                (true, true, _) => key("X", "#7a1f24"),
                (true, false, true) => key("this", &simon_color(i, true)),
                _ => key("", &simon_color(i, lit == Some(i))),
            })
            .collect();
        let (title, line) = match self.phase {
            SimonPhase::Showing { .. } => ("Watch", format!("round {}", self.seq.len())),
            SimonPhase::Repeat { pos } => ("Your turn", format!("{} of {}", pos, self.seq.len())),
            SimonPhase::Pause(_) => ("Right!", format!("round {}", self.seq.len())),
            SimonPhase::Lost => ("Wrong key", format!("{} rounds", self.seq.len() - 1)),
        };
        let led = lit.map(|k| hex_rgb(&simon_color(k, true))).unwrap_or([10, 10, 16]);
        Screen {
            keys,
            panel: None,
            dial: format!(
                r##"<rect width="240" height="240" fill="#0d0c12"/>{}{}{}"##,
                ring(self.seq.len() as f32 / 20.0, "#c4b5fd"),
                text(120.0, 122.0, 34.0, "#f0eef6", title),
                text(120.0, 156.0, 20.0, "#c4b5fd", &line)
            ),
            knob_leds: Some(vec![led; 3]),
            fader_leds: Some(vec![led; 5]),
        }
    }

    fn over(&self) -> Option<u32> {
        matches!(self.phase, SimonPhase::Lost).then(|| self.seq.len() as u32 - 1)
    }
}

// ---------- Safe ----------

const SAFE_NUMBERS: i32 = 40;
const SAFE_TIME: Duration = Duration::from_secs(60);
const SAFE_PENALTY: Duration = Duration::from_secs(5);

pub struct Safe {
    combo: [i32; 3],
    found: usize,
    dial: i32,
    start: Instant,
    penalty: Duration,
    /// When the dial last landed on the right number (the screen glows).
    clicked: Option<Instant>,
    /// When a wrong push was made (the screen flashes red).
    bad: Option<Instant>,
    end: Option<Instant>,
}

impl Safe {
    pub fn new(seed: u64, now: Instant) -> Self {
        let mut rng = Rng::new(seed);
        let mut combo = [0; 3];
        for c in combo.iter_mut() {
            *c = rng.below(SAFE_NUMBERS as u64) as i32;
        }
        Safe { combo, found: 0, dial: 0, start: now, penalty: Duration::ZERO, clicked: None, bad: None, end: None }
    }

    fn left(&self, now: Instant) -> Duration {
        let used = self.end.unwrap_or(now).duration_since(self.start) + self.penalty;
        SAFE_TIME.saturating_sub(used)
    }

    /// 0 far .. 1 on the number, along the shorter way round the dial.
    fn closeness(&self) -> f32 {
        let Some(target) = self.combo.get(self.found) else { return 1.0 };
        let d = (self.dial - target).rem_euclid(SAFE_NUMBERS);
        let d = d.min(SAFE_NUMBERS - d) as f32;
        1.0 - d / (SAFE_NUMBERS as f32 / 2.0)
    }
}

impl Game for Safe {
    fn input(&mut self, input: Input, now: Instant) {
        if self.over().is_some() {
            return;
        }
        match input {
            Input::Dial(d) | Input::Knob(_, d) | Input::Roller(_, d) => {
                self.dial = (self.dial + d as i32).rem_euclid(SAFE_NUMBERS);
                if self.combo.get(self.found) == Some(&self.dial) {
                    self.clicked = Some(now);
                }
            }
            Input::DialClick | Input::Key { down: true, .. } => {
                if self.combo.get(self.found) == Some(&self.dial) {
                    self.found += 1;
                    if self.found == self.combo.len() {
                        self.end = Some(now);
                    }
                } else {
                    self.penalty += SAFE_PENALTY;
                    self.bad = Some(now);
                }
            }
            _ => {}
        }
    }

    fn tick(&mut self, now: Instant) {
        if self.end.is_none() && self.left(now).is_zero() {
            self.end = Some(now);
        }
    }

    fn screen(&self, now: Instant) -> Screen {
        let near = self.closeness();
        let glow = self.clicked.is_some_and(|t| now.duration_since(t) < Duration::from_millis(400)) && self.combo.get(self.found) == Some(&self.dial);
        let bad = self.bad.is_some_and(|t| now.duration_since(t) < Duration::from_millis(400));
        let heat = hsl((near * 120.0) as u32, 80, 45); // red far, green near
        let bg = if bad {
            "#5a1218".to_string()
        } else if glow {
            "#123d1f".to_string()
        } else {
            "#0d0c12".to_string()
        };
        // tick marks round the dial, the current number at the top
        let mut marks = String::new();
        for n in 0..SAFE_NUMBERS {
            let a = ((n - self.dial) as f32 * 360.0 / SAFE_NUMBERS as f32 - 90.0).to_radians();
            let (r1, r2) = (if n % 5 == 0 { 92.0 } else { 100.0 }, 108.0);
            marks += &format!(
                r##"<line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="#8a8599" stroke-width="{}"/>"##,
                120.0 + r1 * a.cos(),
                120.0 + r1 * a.sin(),
                120.0 + r2 * a.cos(),
                120.0 + r2 * a.sin(),
                if n % 5 == 0 { 3.0 } else { 1.5 }
            );
        }
        let won = self.found == self.combo.len();
        let center = if won {
            text(120.0, 136.0, 38.0, "#7dffa0", "OPEN")
        } else {
            format!(r#"<circle cx="120" cy="120" r="56" fill="{heat}"/>{}"#, text(120.0, 140.0, 54.0, "#ffffff", &self.dial.to_string()))
        };
        let dial = format!(r##"<rect width="240" height="240" fill="{bg}"/>{marks}<path d="M120 4 L112 20 L128 20 Z" fill="#f2c14e"/>{center}"##);
        let secs = self.left(now).as_secs();
        let mut keys = vec![blank(); KEYS];
        for (i, c) in self.combo.iter().enumerate() {
            keys[1 + i] = if i < self.found {
                key(&c.to_string(), "#1f5a2c")
            } else if i == self.found {
                key_icon("lock", "", "#3a3448")
            } else {
                key("?", "#26232f")
            };
        }
        keys[7] = key(&format!("{secs}s"), if secs <= 10 { "#7a1f24" } else { "#26232f" });
        keys[11] = key("push", "#26232f");
        keys[12] = key("to", "#26232f");
        keys[13] = key("lock", "#26232f");
        let led = hex_rgb(&heat);
        Screen { keys, panel: None, dial, knob_leds: Some(vec![led; 3]), fader_leds: Some(vec![led; 5]) }
    }

    fn over(&self) -> Option<u32> {
        self.end.map(|_| if self.found == self.combo.len() { (self.left(self.end.unwrap_or(self.start)).as_millis() / 100) as u32 } else { 0 })
    }
}

// ---------- Snake (the whole panel is the field, seen through the key windows) ----------

const CELL: i32 = 16;
const COLS: i32 = 480 / CELL;
const ROWS: i32 = 272 / CELL;

/// Whether a cell's centre shows through a key window (default calibration). Food only appears
/// where it can be seen; the snake may cross the hidden gaps.
fn cell_visible(c: i32, r: i32) -> bool {
    let (x, y) = ((c * CELL + CELL / 2) as f32, (r * CELL + CELL / 2) as f32);
    (0..KEYS).any(|i| {
        let (kx, ky) = ((i % 5) as f32 * 100.0, (i / 5) as f32 * 99.0);
        x >= kx + 2.0 && x <= kx + 77.0 && y >= ky + 2.0 && y <= ky + 73.0
    })
}

pub struct Snake {
    rng: Rng,
    body: std::collections::VecDeque<(i32, i32)>,
    dir: (i32, i32),
    /// Turns asked for since the last step (one is used per step, so quick turns are not lost).
    turns: std::collections::VecDeque<i8>,
    food: (i32, i32),
    last_step: Instant,
    alive: bool,
    eaten: u32,
}

impl Snake {
    pub fn new(seed: u64, now: Instant) -> Self {
        let mut s = Snake { rng: Rng::new(seed), body: [(6, 8), (5, 8), (4, 8)].into_iter().collect(), dir: (1, 0), turns: Default::default(), food: (0, 0), last_step: now, alive: true, eaten: 0 };
        s.place_food();
        s
    }

    fn place_food(&mut self) {
        for _ in 0..500 {
            let cell = (self.rng.below(COLS as u64) as i32, self.rng.below(ROWS as u64) as i32);
            if cell_visible(cell.0, cell.1) && !self.body.contains(&cell) {
                self.food = cell;
                return;
            }
        }
    }

    fn step_every(&self) -> Duration {
        Duration::from_millis(220u64.saturating_sub(self.eaten as u64 * 6).max(90))
    }
}

impl Game for Snake {
    fn input(&mut self, input: Input, _now: Instant) {
        if let Input::Dial(d) | Input::Knob(_, d) | Input::Roller(_, d) = input
            && self.turns.len() < 3
            && d != 0
        {
            self.turns.push_back(d.signum());
        }
    }

    fn tick(&mut self, now: Instant) {
        if !self.alive || now.duration_since(self.last_step) < self.step_every() {
            return;
        }
        self.last_step = now;
        // clockwise turn: (dx, dy) -> (-dy, dx) on screen (y down)
        if let Some(t) = self.turns.pop_front() {
            self.dir = if t > 0 { (-self.dir.1, self.dir.0) } else { (self.dir.1, -self.dir.0) };
        }
        let head = self.body[0];
        let next = (head.0 + self.dir.0, head.1 + self.dir.1);
        if next.0 < 0 || next.1 < 0 || next.0 >= COLS || next.1 >= ROWS || self.body.contains(&next) {
            self.alive = false;
            return;
        }
        self.body.push_front(next);
        if next == self.food {
            self.eaten += 1;
            self.place_food();
        } else {
            self.body.pop_back();
        }
    }

    fn screen(&self, _now: Instant) -> Screen {
        let mut panel = String::from(r##"<rect width="480" height="272" fill="#08110b"/>"##);
        let (fx, fy) = (self.food.0 * CELL + CELL / 2, self.food.1 * CELL + CELL / 2);
        panel += &format!(r##"<circle cx="{fx}" cy="{fy}" r="6" fill="#ff5a4e"/>"##);
        for (i, (c, r)) in self.body.iter().enumerate() {
            let fill = if i == 0 {
                "#b8ff7a"
            } else if self.alive {
                "#4fc33a"
            } else {
                "#7a3a3a"
            };
            panel += &format!(r#"<rect x="{}" y="{}" width="{}" height="{}" rx="3" fill="{fill}"/>"#, c * CELL + 1, r * CELL + 1, CELL - 2, CELL - 2);
        }
        // the direction as an arrow on the dial
        let angle = match self.dir {
            (1, 0) => 90,
            (0, 1) => 180,
            (-1, 0) => 270,
            _ => 0,
        };
        let dial = format!(
            r##"<rect width="240" height="240" fill="#08110b"/><g transform="rotate({angle} 120 120)"><path d="M120 34 L146 74 L94 74 Z" fill="#b8ff7a"/></g>{}{}"##,
            text(120.0, 150.0, 56.0, "#f0eef6", &self.eaten.to_string()),
            text(120.0, 184.0, 16.0, "#a8a4b5", "turn the dial to steer")
        );
        Screen { keys: vec![blank(); KEYS], panel: Some(panel), dial, knob_leds: Some(vec![[40, 160, 40]; 3]), fader_leds: None }
    }

    fn over(&self) -> Option<u32> {
        (!self.alive).then_some(self.eaten)
    }
}

// ---------- Arena (defend the core on the round screen) ----------

struct Enemy {
    angle: f32,
    radius: f32,
}

pub struct Arena {
    rng: Rng,
    /// Middle of the shield, degrees clockwise from the top.
    shield: f32,
    enemies: Vec<Enemy>,
    lives: u32,
    score: u32,
    last: Instant,
    next_spawn: Instant,
    start: Instant,
    /// When the core was last hit (the screen flashes).
    hurt: Option<Instant>,
}

const SHIELD_R: f32 = 62.0;
const SHIELD_WIDTH: f32 = 70.0;
const CORE_R: f32 = 20.0;

impl Arena {
    pub fn new(seed: u64, now: Instant) -> Self {
        Arena { rng: Rng::new(seed), shield: 0.0, enemies: Vec::new(), lives: 3, score: 0, last: now, next_spawn: now + Duration::from_millis(900), start: now, hurt: None }
    }

    fn speed(&self) -> f32 {
        // pixels per second, faster over time
        28.0 + self.score as f32 * 1.5
    }
}

/// Smallest difference between two angles in degrees.
fn angle_gap(a: f32, b: f32) -> f32 {
    let d = (a - b).rem_euclid(360.0);
    d.min(360.0 - d)
}

impl Game for Arena {
    fn input(&mut self, input: Input, _now: Instant) {
        if let Input::Dial(d) | Input::Knob(_, d) | Input::Roller(_, d) = input {
            self.shield = (self.shield + d as f32 * 12.0).rem_euclid(360.0);
        }
    }

    fn tick(&mut self, now: Instant) {
        if self.lives == 0 {
            return;
        }
        let dt = now.duration_since(self.last).as_secs_f32().min(0.2);
        self.last = now;
        let speed = self.speed();
        let mut lost = 0;
        let mut blocked = 0;
        let shield = self.shield;
        self.enemies.retain_mut(|e| {
            let before = e.radius;
            e.radius -= speed * dt;
            if before > SHIELD_R && e.radius <= SHIELD_R && angle_gap(e.angle, shield) <= SHIELD_WIDTH / 2.0 {
                blocked += 1;
                return false;
            }
            if e.radius <= CORE_R {
                lost += 1;
                return false;
            }
            true
        });
        self.score += blocked;
        if lost > 0 {
            self.lives = self.lives.saturating_sub(lost);
            self.hurt = Some(now);
        }
        if now >= self.next_spawn {
            self.enemies.push(Enemy { angle: self.rng.below(360) as f32, radius: 118.0 });
            let gap = 1400u64.saturating_sub(self.score as u64 * 25).max(450);
            self.next_spawn = now + Duration::from_millis(gap);
        }
    }

    fn screen(&self, now: Instant) -> Screen {
        let hurt = self.hurt.is_some_and(|t| now.duration_since(t) < Duration::from_millis(250));
        let polar = |angle: f32, r: f32| {
            let a = (angle - 90.0).to_radians();
            (120.0 + r * a.cos(), 120.0 + r * a.sin())
        };
        let (a0, a1) = (self.shield - SHIELD_WIDTH / 2.0, self.shield + SHIELD_WIDTH / 2.0);
        let ((x0, y0), (x1, y1)) = (polar(a0, SHIELD_R), polar(a1, SHIELD_R));
        let mut dial =
            format!(r##"<rect width="240" height="240" fill="{}"/><circle cx="120" cy="120" r="116" fill="none" stroke="#2a2733" stroke-width="2"/>"##, if hurt { "#3a0d10" } else { "#0b0a12" });
        dial += &format!(r##"<path d="M{x0:.1} {y0:.1} A{SHIELD_R} {SHIELD_R} 0 0 1 {x1:.1} {y1:.1}" fill="none" stroke="#7fd4ff" stroke-width="8" stroke-linecap="round"/>"##);
        dial += &format!(r##"<circle cx="120" cy="120" r="{CORE_R}" fill="{}"/>"##, if self.lives > 1 { "#c4b5fd" } else { "#ff5a4e" });
        for e in &self.enemies {
            let (x, y) = polar(e.angle, e.radius);
            dial += &format!(r##"<circle cx="{x:.1}" cy="{y:.1}" r="7" fill="#f2c14e"/>"##);
        }
        dial += &text(120.0, 127.0, 16.0, "#0b0a12", &self.score.to_string());
        let mut keys = vec![blank(); KEYS];
        for i in 0..3 {
            keys[1 + i] = if (i as u32) < self.lives { key_icon("heart", "", "#5a1f2c") } else { key("", "#1a1820") };
        }
        keys[7] = key(&self.score.to_string(), "#2a2340");
        keys[12] = key(&format!("{}s", now.duration_since(self.start).as_secs()), "#1a1820");
        let led = if self.lives > 1 { [120, 80, 255] } else { [255, 40, 30] };
        Screen { keys, panel: None, dial, knob_leds: Some((0..3).map(|i| if (i as u32) < self.lives { led } else { [0, 0, 0] }).collect()), fader_leds: None }
    }

    fn over(&self) -> Option<u32> {
        (self.lives == 0).then_some(self.score)
    }
}

// ---------- Balance (keep the faders on moving targets) ----------

const BALANCE_TIME: Duration = Duration::from_secs(60);
const BALANCE_NEAR: f32 = 8.0;

pub struct Balance {
    start: Instant,
    last: Instant,
    faders: [f32; 5],
    /// Phase and speed of each target's slow wave.
    waves: [(f32, f32); 5],
    score: f32,
}

impl Balance {
    pub fn new(seed: u64, now: Instant) -> Self {
        let mut rng = Rng::new(seed);
        let mut waves = [(0.0, 0.0); 5];
        for w in waves.iter_mut() {
            *w = (rng.below(628) as f32 / 100.0, 0.25 + rng.below(40) as f32 / 100.0);
        }
        Balance { start: now, last: now, faders: [50.0; 5], waves, score: 0.0 }
    }

    fn target(&self, i: usize, now: Instant) -> f32 {
        let t = now.duration_since(self.start).as_secs_f32();
        let (phase, speed) = self.waves[i];
        50.0 + 38.0 * (phase + t * speed).sin()
    }

    fn left(&self, now: Instant) -> Duration {
        BALANCE_TIME.saturating_sub(now.duration_since(self.start))
    }
}

impl Game for Balance {
    fn input(&mut self, input: Input, _now: Instant) {
        if let Input::Fader(i, v) = input
            && i < 5
        {
            self.faders[i] = v as f32;
        }
    }

    fn tick(&mut self, now: Instant) {
        // time stops at the end of the round, which is what ends the game
        let now = now.min(self.start + BALANCE_TIME);
        let dt = now.duration_since(self.last).as_secs_f32().min(0.2);
        self.last = now;
        let near = (0..5).filter(|i| (self.faders[*i] - self.target(*i, now)).abs() <= BALANCE_NEAR).count();
        // all five on target scores five times as fast as one
        self.score += dt * near as f32 * 2.0;
    }

    fn screen(&self, now: Instant) -> Screen {
        let mut keys = vec![blank(); KEYS];
        let mut leds = Vec::new();
        for i in 0..5 {
            let target = self.target(i, now);
            let gap = (self.faders[i] - target).abs();
            let row = if target > 66.0 {
                0
            } else if target > 33.0 {
                1
            } else {
                2
            };
            let near = gap <= BALANCE_NEAR;
            keys[row * 5 + i] = key("", if near { "#2c8a3a" } else { "#b8862a" });
            let k = (1.0 - (gap / 50.0).min(1.0)) * 255.0;
            leds.push(if near { [40, 255, 60] } else { [255, k as u8 / 2, 0] });
        }
        let left = self.left(now);
        let dial = format!(
            r##"<rect width="240" height="240" fill="#0d0c12"/>{}{}{}"##,
            ring(left.as_secs_f32() / BALANCE_TIME.as_secs_f32(), "#2c8a3a"),
            text(120.0, 138.0, 56.0, "#f0eef6", &(self.score as u32).to_string()),
            text(120.0, 172.0, 16.0, "#a8a4b5", "follow the lit keys")
        );
        Screen { keys, panel: None, dial, knob_leds: None, fader_leds: Some(leds) }
    }

    fn over(&self) -> Option<u32> {
        (self.last.duration_since(self.start) >= BALANCE_TIME).then_some(self.score as u32)
    }
}

// ---------- Wheel (spin the dial, the round screen decides) ----------

/// The wheel's fields when the user set none.
pub const WHEEL_DEFAULT: [&str; 8] = ["Pizza", "Sushi", "Burger", "Salad", "Pasta", "Tacos", "Curry", "Soup"];

/// The wheel's fields (2..=12), set from the Games tab and kept in `<data>/games-settings.json`.
static WHEEL_CHOICES: std::sync::RwLock<Vec<String>> = std::sync::RwLock::new(Vec::new());

pub fn wheel_choices() -> Vec<String> {
    let c = WHEEL_CHOICES.read().unwrap_or_else(|e| e.into_inner()).clone();
    if c.len() >= 2 { c } else { WHEEL_DEFAULT.iter().map(|s| s.to_string()).collect() }
}

/// Keep 2..=12 non-empty fields of at most 14 characters; anything else means the defaults.
pub fn set_wheel_choices(choices: Vec<String>) -> Vec<String> {
    let clean: Vec<String> = choices.into_iter().map(|c| c.trim().chars().take(14).collect::<String>()).filter(|c| !c.is_empty()).take(12).collect();
    *WHEEL_CHOICES.write().unwrap_or_else(|e| e.into_inner()) = if clean.len() >= 2 { clean } else { Vec::new() };
    wheel_choices()
}

pub struct Wheel {
    labels: Vec<String>,
    angle: f32,
    /// Degrees per second; the wheel slows down by itself.
    speed: f32,
    last: Instant,
    spins: u32,
    result: Option<usize>,
}

impl Wheel {
    pub fn new(_seed: u64, now: Instant) -> Self {
        Wheel { labels: wheel_choices(), angle: 0.0, speed: 0.0, last: now, spins: 0, result: None }
    }

    fn slot(&self) -> usize {
        // the pointer is at the top; slot i covers [i*45, (i+1)*45) degrees of the wheel
        let at = (360.0 - self.angle.rem_euclid(360.0)).rem_euclid(360.0);
        (at / (360.0 / self.labels.len() as f32)) as usize % self.labels.len()
    }
}

impl Game for Wheel {
    fn input(&mut self, input: Input, _now: Instant) {
        match input {
            // a flick of the dial spins it; harder turns spin faster
            Input::Dial(d) | Input::Knob(_, d) | Input::Roller(_, d) => {
                self.speed = (self.speed + d as f32 * 140.0).clamp(-1800.0, 1800.0);
                self.result = None;
            }
            Input::DialClick | Input::Key { down: true, .. } if self.speed.abs() < 1.0 => {
                self.speed = 900.0 + (self.last.elapsed().subsec_nanos() % 600) as f32;
                self.result = None;
            }
            _ => {}
        }
    }

    fn tick(&mut self, now: Instant) {
        let dt = now.duration_since(self.last).as_secs_f32().min(0.2);
        self.last = now;
        if self.speed.abs() < 1.0 {
            return;
        }
        self.angle += self.speed * dt;
        // friction: lose a third of the speed per second, and a little more at the end
        self.speed *= (1.0 - 0.45 * dt).max(0.0);
        if self.speed.abs() < 12.0 {
            self.speed = 0.0;
            self.result = Some(self.slot());
            self.spins += 1;
        }
    }

    fn screen(&self, _now: Instant) -> Screen {
        let n = self.labels.len();
        let step = 360.0 / n as f32;
        let mut dial = String::from(r##"<rect width="240" height="240" fill="#0d0c12"/>"##);
        dial += &format!(r#"<g transform="rotate({:.2} 120 120)">"#, self.angle);
        for (i, label) in self.labels.iter().enumerate() {
            let (a0, a1) = ((i as f32 * step - 90.0).to_radians(), ((i as f32 + 1.0) * step - 90.0).to_radians());
            let r = 112.0;
            let fill = hsl((i * 360 / n) as u32, 55, if i % 2 == 0 { 42 } else { 34 });
            dial += &format!(r#"<path d="M120 120 L{:.1} {:.1} A{r} {r} 0 0 1 {:.1} {:.1} Z" fill="{fill}"/>"#, 120.0 + r * a0.cos(), 120.0 + r * a0.sin(), 120.0 + r * a1.cos(), 120.0 + r * a1.sin());
            let mid = i as f32 * step + step / 2.0;
            dial += &format!(r#"<g transform="rotate({mid:.1} 120 120)">{}</g>"#, text(120.0, 48.0, 15.0, "#ffffff", label));
        }
        dial += r##"</g><circle cx="120" cy="120" r="16" fill="#0d0c12"/><path d="M120 22 L110 4 L130 4 Z" fill="#f2c14e"/>"##;
        let mut keys = vec![blank(); KEYS];
        match self.result {
            Some(i) => {
                for k in keys.iter_mut().skip(5).take(5) {
                    *k = key("", "#2c8a3a");
                }
                keys[7] = key(&self.labels[i], "#2c8a3a");
            }
            None => keys[7] = key(if self.speed.abs() > 0.0 { "..." } else { "spin!" }, "#2a2340"),
        }
        Screen { keys, panel: None, dial, knob_leds: None, fader_leds: None }
    }

    /// The wheel never ends by itself: hold the dial to leave. It reports the number of spins.
    fn over(&self) -> Option<u32> {
        None
    }
}

// ---------- Duel (two players, left and right halves of the keys) ----------

pub struct Duel {
    rng: Rng,
    /// The key that lit up and when, or the time the next one lights.
    lit: Option<(usize, Instant)>,
    next_at: Instant,
    score: [u32; 2],
    /// A false start or a win flashes this side.
    flash: Option<(usize, bool, Instant)>,
    target: u32,
}

/// Left player: columns 0 and 1; right player: columns 3 and 4; the middle column is the referee.
fn duel_side(key: usize) -> Option<usize> {
    match key % 5 {
        0 | 1 => Some(0),
        3 | 4 => Some(1),
        _ => None,
    }
}

impl Duel {
    pub fn new(seed: u64, now: Instant) -> Self {
        let mut rng = Rng::new(seed);
        let first = now + Duration::from_millis(1500 + rng.below(2000));
        Duel { rng, lit: None, next_at: first, score: [0, 0], flash: None, target: 5 }
    }

    fn arm(&mut self, now: Instant) {
        self.lit = None;
        self.next_at = now + Duration::from_millis(1200 + self.rng.below(2500));
    }
}

impl Game for Duel {
    fn input(&mut self, input: Input, now: Instant) {
        let Input::Key { index, down: true } = input else { return };
        let Some(side) = duel_side(index) else { return };
        if self.over().is_some() {
            return;
        }
        match self.lit {
            // the first to hit their own lit key (one per side, mirrored) wins the point
            Some((k, _)) if index == k || index == (k / 5) * 5 + (4 - k % 5) => {
                self.score[side] += 1;
                self.flash = Some((side, true, now));
                self.arm(now);
            }
            // pressing before anything is lit costs a point
            None => {
                self.score[side] = self.score[side].saturating_sub(1);
                self.flash = Some((side, false, now));
            }
            _ => {}
        }
    }

    fn tick(&mut self, now: Instant) {
        if self.lit.is_none() && now >= self.next_at && self.over().is_none() {
            // one key lights on each side: the same row and the mirrored column
            let row = self.rng.below(3) as usize;
            let col = self.rng.below(2) as usize;
            let side_first = self.rng.below(2) as usize;
            let k = row * 5 + if side_first == 0 { col } else { 4 - col };
            self.lit = Some((k, now));
        }
    }

    fn screen(&self, now: Instant) -> Screen {
        let mut keys = vec![blank(); KEYS];
        for r in 0..3 {
            keys[r * 5 + 2] = key("", "#1a1820");
        }
        keys[2] = key(&self.score[0].to_string(), "#3a2a5a");
        keys[12] = key(&self.score[1].to_string(), "#5a2a3a");
        if let Some((k, _)) = self.lit {
            let mirror = (k / 5) * 5 + (4 - k % 5);
            keys[k] = key("", "#f2c14e");
            keys[mirror] = key("", "#f2c14e");
        }
        if let Some((side, good, at)) = self.flash
            && now.duration_since(at) < Duration::from_millis(300)
        {
            for r in 0..3 {
                for c in if side == 0 { [0, 1] } else { [3, 4] } {
                    keys[r * 5 + c] = key("", if good { "#2c8a3a" } else { "#7a1f24" });
                }
            }
        }
        let (title, line) = match self.over() {
            Some(_) => (if self.score[0] > self.score[1] { "LEFT WINS" } else { "RIGHT WINS" }.to_string(), format!("{} : {}", self.score[0], self.score[1])),
            None => (format!("{} : {}", self.score[0], self.score[1]), format!("first to {}", self.target)),
        };
        Screen { keys, panel: None, dial: dial_message(&title, &line, "left keys vs right keys"), knob_leds: None, fader_leds: None }
    }

    fn over(&self) -> Option<u32> {
        (self.score[0] >= self.target || self.score[1] >= self.target).then(|| self.score[0].max(self.score[1]))
    }
}

// ---------- Pong (the whole panel is the court) ----------

pub struct Pong {
    /// A second player turned a knob or roller: the right paddle is theirs, not the deck's.
    two_players: bool,
    ball: (f32, f32),
    vel: (f32, f32),
    /// Centre of each paddle (y on the panel); the left one is yours, the right one the deck's.
    paddles: [f32; 2],
    score: [u32; 2],
    last: Instant,
    rng: Rng,
}

const PADDLE_H: f32 = 60.0;
const PONG_TO: u32 = 7;

impl Pong {
    pub fn new(seed: u64, now: Instant) -> Self {
        let mut p = Pong { two_players: false, ball: (240.0, 136.0), vel: (0.0, 0.0), paddles: [136.0, 136.0], score: [0, 0], last: now, rng: Rng::new(seed) };
        p.serve(1.0);
        p
    }

    fn serve(&mut self, dir: f32) {
        self.ball = (240.0, 136.0);
        let up = if self.rng.below(2) == 0 { -1.0 } else { 1.0 };
        self.vel = (190.0 * dir, (60.0 + self.rng.below(60) as f32) * up);
    }
}

impl Game for Pong {
    fn input(&mut self, input: Input, _now: Instant) {
        let clamp = |y: f32| y.clamp(PADDLE_H / 2.0, 272.0 - PADDLE_H / 2.0);
        match input {
            Input::Dial(d) => self.paddles[0] = clamp(self.paddles[0] + d as f32 * 14.0),
            // a knob or a roller is the second player's paddle
            Input::Knob(_, d) | Input::Roller(_, d) => {
                self.two_players = true;
                self.paddles[1] = clamp(self.paddles[1] + d as f32 * 14.0);
            }
            _ => {}
        }
    }

    fn tick(&mut self, now: Instant) {
        if self.over().is_some() {
            return;
        }
        let dt = now.duration_since(self.last).as_secs_f32().min(0.05);
        self.last = now;
        // the deck's paddle follows the ball, a little slower than it
        if !self.two_players {
            let follow = (self.ball.1 - self.paddles[1]).clamp(-150.0 * dt, 150.0 * dt);
            self.paddles[1] = (self.paddles[1] + follow).clamp(PADDLE_H / 2.0, 272.0 - PADDLE_H / 2.0);
        }
        self.ball.0 += self.vel.0 * dt;
        self.ball.1 += self.vel.1 * dt;
        if self.ball.1 < 6.0 || self.ball.1 > 266.0 {
            self.vel.1 = -self.vel.1;
            self.ball.1 = self.ball.1.clamp(6.0, 266.0);
        }
        for (side, x) in [(0usize, 24.0f32), (1, 456.0)] {
            let toward = if side == 0 { self.vel.0 < 0.0 } else { self.vel.0 > 0.0 };
            if toward && (self.ball.0 - x).abs() < 8.0 && (self.ball.1 - self.paddles[side]).abs() <= PADDLE_H / 2.0 + 6.0 {
                // the further from the paddle's middle, the steeper the bounce; a bit faster each time
                let off = (self.ball.1 - self.paddles[side]) / (PADDLE_H / 2.0);
                self.vel.0 = -self.vel.0 * 1.06;
                self.vel.1 = off * 220.0;
            }
        }
        if self.ball.0 < 0.0 {
            self.score[1] += 1;
            self.serve(1.0);
        } else if self.ball.0 > 480.0 {
            self.score[0] += 1;
            self.serve(-1.0);
        }
    }

    fn screen(&self, _now: Instant) -> Screen {
        let mut panel = String::from(r##"<rect width="480" height="272" fill="#06080f"/><line x1="240" y1="0" x2="240" y2="272" stroke="#1d2233" stroke-width="4" stroke-dasharray="10 10"/>"##);
        for (x, y) in [(18.0, self.paddles[0]), (450.0, self.paddles[1])] {
            panel += &format!(r##"<rect x="{x}" y="{:.1}" width="12" height="{PADDLE_H}" rx="4" fill="#7fd4ff"/>"##, y - PADDLE_H / 2.0);
        }
        panel += &format!(r##"<circle cx="{:.1}" cy="{:.1}" r="8" fill="#f2c14e"/>"##, self.ball.0, self.ball.1);
        let line = match (self.over().is_some(), self.two_players, self.score[0] > self.score[1]) {
            (true, true, true) => "left wins",
            (true, true, false) => "right wins",
            (true, false, true) => "you win",
            (true, false, false) => "the deck wins",
            (false, true, _) => "dial vs knob",
            (false, false, _) => "turn the dial",
        };
        Screen {
            keys: vec![blank(); KEYS],
            panel: Some(panel),
            dial: dial_message(&format!("{} : {}", self.score[0], self.score[1]), line, &format!("first to {PONG_TO}")),
            knob_leds: None,
            fader_leds: None,
        }
    }

    fn over(&self) -> Option<u32> {
        (self.score[0] >= PONG_TO || self.score[1] >= PONG_TO).then_some(self.score[0])
    }
}

// ---------- Rhythm (notes come down the key columns, hit them on the bottom row) ----------

pub struct Rhythm {
    rng: Rng,
    start: Instant,
    /// (column, when it reaches the bottom row)
    notes: Vec<(usize, Instant)>,
    next_note: Instant,
    score: u32,
    misses: u32,
    streak: u32,
    flash: Option<(usize, bool, Instant)>,
}

const BEAT: Duration = Duration::from_millis(480);
const FALL: Duration = Duration::from_millis(1440);
const RHYTHM_MISSES: u32 = 8;

impl Rhythm {
    pub fn new(seed: u64, now: Instant) -> Self {
        Rhythm { rng: Rng::new(seed), start: now, notes: Vec::new(), next_note: now + BEAT, score: 0, misses: 0, streak: 0, flash: None }
    }
}

impl Game for Rhythm {
    fn input(&mut self, input: Input, now: Instant) {
        let Input::Key { index, down: true } = input else { return };
        let col = index % 5;
        // a hit counts within a third of a beat of the note landing
        let window = BEAT / 3;
        if let Some(pos) = self.notes.iter().position(|(c, at)| *c == col && (if now > *at { now - *at } else { *at - now }) <= window) {
            self.notes.remove(pos);
            self.streak += 1;
            self.score += 1 + self.streak / 5;
            self.flash = Some((col, true, now));
        } else {
            self.streak = 0;
            self.flash = Some((col, false, now));
        }
    }

    fn tick(&mut self, now: Instant) {
        if self.over().is_some() {
            return;
        }
        let window = BEAT / 3;
        let before = self.notes.len();
        self.notes.retain(|(_, at)| now <= *at + window);
        let missed = (before - self.notes.len()) as u32;
        if missed > 0 {
            self.misses += missed;
            self.streak = 0;
        }
        if now >= self.next_note {
            // sometimes two notes at once, more often as the song goes on
            let col = self.rng.below(5) as usize;
            self.notes.push((col, now + FALL));
            if self.rng.below(100) < (10 + self.score.min(40)) as u64 {
                let other = (col + 1 + self.rng.below(4) as usize) % 5;
                self.notes.push((other, now + FALL));
            }
            let halves = if self.score > 20 && self.rng.below(3) == 0 { 1 } else { 2 };
            self.next_note = now + BEAT * halves / 2;
        }
    }

    fn screen(&self, now: Instant) -> Screen {
        let mut keys = vec![blank(); KEYS];
        for c in 0..5 {
            keys[10 + c] = key("", "#1d1a2a");
        }
        for (c, at) in &self.notes {
            // row from the time left: 2 beats before = top, 1 = middle, landing = bottom
            let left = at.saturating_duration_since(now);
            let row = if left > BEAT * 2 {
                continue;
            } else if left > BEAT {
                0
            } else if left > BEAT / 3 {
                1
            } else {
                2
            };
            keys[row * 5 + c] = key("", if row == 2 { "#f2c14e" } else { "#8b7cf6" });
        }
        if let Some((c, good, at)) = self.flash
            && now.duration_since(at) < Duration::from_millis(160)
        {
            keys[10 + c] = key("", if good { "#2c8a3a" } else { "#7a1f24" });
        }
        let left = RHYTHM_MISSES.saturating_sub(self.misses);
        let dial = format!(
            r##"<rect width="240" height="240" fill="#0d0c12"/>{}{}{}"##,
            ring(left as f32 / RHYTHM_MISSES as f32, "#8b7cf6"),
            text(120.0, 138.0, 56.0, "#f0eef6", &self.score.to_string()),
            text(120.0, 172.0, 16.0, "#a8a4b5", &format!("streak {}", self.streak))
        );
        // the knob lights pulse on the beat
        let beat = (now.duration_since(self.start).as_millis() % BEAT.as_millis()) < 90;
        Screen { keys, panel: None, dial, knob_leds: Some(vec![if beat { [139, 124, 246] } else { [20, 16, 40] }; 3]), fader_leds: None }
    }

    fn over(&self) -> Option<u32> {
        (self.misses >= RHYTHM_MISSES).then_some(self.score)
    }
}

// ---------- Memory (find the pairs under the keys) ----------

/// Seven pictures, two of each, under 14 keys; the middle key of the top row shows the moves.
const MEMORY_ICONS: [&str; 7] = ["star", "heart", "music", "zap", "sun", "moon", "rocket"];
const MEMORY_SCORE_KEY: usize = 2;

pub struct Memory {
    /// Picture index under each key (None for the score key).
    cards: [Option<usize>; KEYS],
    found: [bool; KEYS],
    /// Keys turned up in this move.
    up: Vec<usize>,
    /// When two non-matching cards were turned: they turn back after a moment.
    hide_at: Option<Instant>,
    moves: u32,
}

impl Memory {
    pub fn new(seed: u64, _now: Instant) -> Self {
        let mut rng = Rng::new(seed);
        let mut deck: Vec<usize> = (0..MEMORY_ICONS.len()).flat_map(|i| [i, i]).collect();
        // Fisher-Yates
        for i in (1..deck.len()).rev() {
            let j = rng.below(i as u64 + 1) as usize;
            deck.swap(i, j);
        }
        let mut cards = [None; KEYS];
        let mut it = deck.into_iter();
        for (k, slot) in cards.iter_mut().enumerate() {
            if k != MEMORY_SCORE_KEY {
                *slot = it.next();
            }
        }
        Memory { cards, found: [false; KEYS], up: Vec::new(), hide_at: None, moves: 0 }
    }
}

impl Game for Memory {
    fn input(&mut self, input: Input, now: Instant) {
        let Input::Key { index, down: true } = input else { return };
        if index >= KEYS || self.cards[index].is_none() || self.found[index] || self.up.contains(&index) {
            return;
        }
        if self.hide_at.is_some() {
            // a third card while two wrong ones show: turn those back first
            self.up.clear();
            self.hide_at = None;
        }
        self.up.push(index);
        if self.up.len() == 2 {
            self.moves += 1;
            let (a, b) = (self.up[0], self.up[1]);
            if self.cards[a] == self.cards[b] {
                self.found[a] = true;
                self.found[b] = true;
                self.up.clear();
            } else {
                self.hide_at = Some(now + Duration::from_millis(900));
            }
        }
    }

    fn tick(&mut self, now: Instant) {
        if self.hide_at.is_some_and(|t| now >= t) {
            self.up.clear();
            self.hide_at = None;
        }
    }

    fn screen(&self, _now: Instant) -> Screen {
        let keys = (0..KEYS)
            .map(|k| match self.cards[k] {
                None => key(&self.moves.to_string(), "#2a2340"),
                Some(i) if self.found[k] => key_icon(MEMORY_ICONS[i], "", "#1d4a2a"),
                Some(i) if self.up.contains(&k) => key_icon(MEMORY_ICONS[i], "", &hsl((i * 360 / MEMORY_ICONS.len()) as u32, 60, 32)),
                Some(_) => key("?", "#26232f"),
            })
            .collect();
        let pairs = self.found.iter().filter(|f| **f).count() / 2;
        let dial = format!(
            r##"<rect width="240" height="240" fill="#0d0c12"/>{}{}{}"##,
            ring(pairs as f32 / MEMORY_ICONS.len() as f32, "#2c8a3a"),
            text(120.0, 138.0, 52.0, "#f0eef6", &format!("{pairs}/{}", MEMORY_ICONS.len())),
            text(120.0, 172.0, 16.0, "#a8a4b5", &format!("{} moves", self.moves))
        );
        Screen { keys, panel: None, dial, knob_leds: None, fader_leds: None }
    }

    /// Fewer moves is better: the score is what is left of 40.
    fn over(&self) -> Option<u32> {
        self.found.iter().filter(|f| **f).count().eq(&(MEMORY_ICONS.len() * 2)).then(|| 40u32.saturating_sub(self.moves))
    }
}

// ---------- Reflex (how fast are you?) ----------

const REFLEX_ROUNDS: usize = 5;

pub struct Reflex {
    rng: Rng,
    /// The key that is lit and since when, or when the next one lights.
    lit: Option<(usize, Instant)>,
    next_at: Instant,
    times: Vec<u32>,
    early: bool,
}

impl Reflex {
    pub fn new(seed: u64, now: Instant) -> Self {
        let mut rng = Rng::new(seed);
        let next_at = now + Duration::from_millis(1500 + rng.below(2500));
        Reflex { rng, lit: None, next_at, times: Vec::new(), early: false }
    }

    fn average(&self) -> Option<u32> {
        (!self.times.is_empty()).then(|| self.times.iter().sum::<u32>() / self.times.len() as u32)
    }
}

impl Game for Reflex {
    fn input(&mut self, input: Input, now: Instant) {
        if self.over().is_some() {
            return;
        }
        let pressed = match input {
            Input::Key { index, down: true } => Some(index),
            Input::DialClick => self.lit.map(|(k, _)| k),
            _ => None,
        };
        let Some(index) = pressed else { return };
        match self.lit {
            Some((k, since)) if k == index => {
                self.times.push(now.duration_since(since).as_millis() as u32);
                self.lit = None;
                self.early = false;
                self.next_at = now + Duration::from_millis(1200 + self.rng.below(2800));
            }
            None => {
                // too early: wait longer
                self.early = true;
                self.next_at = now + Duration::from_millis(1500 + self.rng.below(2000));
            }
            _ => {}
        }
    }

    fn tick(&mut self, now: Instant) {
        if self.lit.is_none() && now >= self.next_at && self.over().is_none() {
            self.lit = Some((self.rng.below(KEYS as u64) as usize, now));
        }
    }

    fn screen(&self, now: Instant) -> Screen {
        let mut keys = vec![blank(); KEYS];
        if let Some((k, _)) = self.lit {
            keys[k] = key("NOW", "#f2c14e");
        }
        let (title, line) = match (self.lit, self.times.last(), self.early) {
            (_, _, true) => ("Too early".to_string(), "wait for the light".to_string()),
            (Some((_, since)), _, _) => (format!("{}", now.duration_since(since).as_millis()), "ms".to_string()),
            (None, Some(t), _) => (format!("{t} ms"), format!("round {} of {REFLEX_ROUNDS}", self.times.len())),
            (None, None, _) => ("Wait...".to_string(), "press the key that lights".to_string()),
        };
        let line = match self.over() {
            Some(_) => format!("average {} ms", self.average().unwrap_or(0)),
            None => line,
        };
        Screen { keys, panel: None, dial: dial_message(&title, &line, "keys or the dial"), knob_leds: None, fader_leds: None }
    }

    /// Faster is better: the score is 1000 minus the average reaction in ms.
    fn over(&self) -> Option<u32> {
        (self.times.len() >= REFLEX_ROUNDS).then(|| 1000u32.saturating_sub(self.average().unwrap_or(1000)))
    }
}

// ---------- game settings ----------

/// Read `<data>/games-settings.json` (the wheel's fields).
pub fn load_settings(data_dir: &std::path::Path) {
    let v: serde_json::Value = std::fs::read(data_dir.join("games-settings.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    let wheel: Vec<String> = v["wheel"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default();
    set_wheel_choices(wheel);
}

pub fn save_settings(data_dir: &std::path::Path) -> std::io::Result<()> {
    let custom = WHEEL_CHOICES.read().unwrap_or_else(|e| e.into_inner()).clone();
    super::profile::write_atomic(&data_dir.join("games-settings.json"), &serde_json::to_vec_pretty(&serde_json::json!({ "wheel": custom }))?)
}

// ---------- the arcade: launcher, playing, scores ----------

enum Mode {
    Launcher,
    Playing {
        index: usize,
        game: Box<dyn Game>,
    },
    /// The last game ended with this score; any key plays again.
    Over {
        index: usize,
        score: u32,
        best: bool,
    },
}

pub struct Arcade {
    mode: Mode,
    scores: HashMap<String, u32>,
    path: PathBuf,
    dial_down: Option<Instant>,
    leave: bool,
    seed: u64,
    /// Module kinds attached now; games that need a missing one cannot start.
    modules: Vec<String>,
}

impl Arcade {
    pub fn new(data_dir: &std::path::Path) -> Self {
        load_settings(data_dir);
        let path = data_dir.join("games.json");
        let scores = std::fs::read(&path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(7);
        Arcade { mode: Mode::Launcher, scores, path, dial_down: None, leave: false, seed, modules: Vec::new() }
    }

    pub fn set_modules(&mut self, kinds: Vec<String>) {
        self.modules = kinds;
    }

    fn playable(&self, e: &Entry) -> bool {
        // before the deck has been reported, allow everything
        self.modules.is_empty() || e.needs.iter().all(|n| self.modules.iter().any(|m| m == n))
    }

    /// True once the player left the launcher: the engine ends game mode.
    pub fn wants_to_leave(&self) -> bool {
        self.leave
    }

    pub fn best(&self, id: &str) -> Option<u32> {
        self.scores.get(id).copied()
    }

    fn start(&mut self, index: usize, now: Instant) {
        let catalog = catalog();
        if let Some(e) = catalog.get(index) {
            self.seed = Rng::new(self.seed).next();
            self.mode = Mode::Playing { index, game: (e.make)(self.seed, now) };
        }
    }

    fn back(&mut self) {
        self.mode = match self.mode {
            Mode::Launcher => {
                self.leave = true;
                Mode::Launcher
            }
            _ => Mode::Launcher,
        };
    }

    pub fn input(&mut self, input: Input, now: Instant) {
        match &mut self.mode {
            Mode::Launcher => match input {
                Input::Key { index: 14, down: true } => self.leave = true,
                Input::Key { index, down: true } if catalog().get(index).is_some_and(|e| self.playable(e)) => self.start(index, now),
                _ => {}
            },
            Mode::Playing { game, .. } => game.input(input, now),
            Mode::Over { index, .. } => {
                let index = *index;
                if let Input::Key { down: true, .. } | Input::DialClick = input {
                    self.start(index, now);
                }
            }
        }
    }

    /// The dial's own press: short = a click for the game, held = go back one level.
    pub fn dial_press(&mut self, down: bool, now: Instant) {
        if down {
            self.dial_down = Some(now);
        } else if let Some(at) = self.dial_down.take()
            && now.duration_since(at) < CLICK_UNDER
        {
            self.input(Input::DialClick, now);
        }
    }

    pub fn tick(&mut self, now: Instant) {
        if self.dial_down.is_some_and(|at| now.duration_since(at) >= LEAVE_AFTER) {
            self.dial_down = None;
            self.back();
            return;
        }
        if let Mode::Playing { index, game } = &mut self.mode {
            game.tick(now);
            if let Some(score) = game.over() {
                let index = *index;
                let id = catalog()[index].id;
                let best = score > 0 && self.scores.get(id).is_none_or(|b| score > *b);
                if best {
                    self.scores.insert(id.to_string(), score);
                    let _ = super::profile::write_atomic(&self.path, &serde_json::to_vec_pretty(&self.scores).unwrap_or_default());
                }
                self.mode = Mode::Over { index, score, best };
            }
        }
    }

    pub fn screen(&self, now: Instant) -> Screen {
        match &self.mode {
            Mode::Playing { game, .. } => game.screen(now),
            Mode::Launcher => {
                let mut keys: Vec<Look> =
                    catalog().iter().map(|e| if self.playable(e) { key_icon(e.icon, e.name, "#2a2340") } else { Look { fg: Some("#5a5866".into()), ..key_icon(e.icon, e.name, "#141318") } }).collect();
                keys.resize(KEYS, blank());
                keys[14] = key_icon("x", "Exit", "#3a1f22");
                Screen { keys, panel: None, dial: dial_message("GAMES", "press a key", "hold the dial to leave"), knob_leds: Some(vec![[90, 60, 200]; 3]), fader_leds: Some(vec![[90, 60, 200]; 5]) }
            }
            Mode::Over { index, score, best } => {
                let e = &catalog()[*index];
                let mut keys = vec![key("again", "#1f3326"); KEYS];
                keys[7] = key(&score.to_string(), "#3b3450");
                let line = if *best { "new best!".to_string() } else { format!("best {}", self.best(e.id).unwrap_or(0)) };
                Screen { keys, panel: None, dial: dial_message(&score.to_string(), &line, "any key: again · hold: back"), knob_leds: Some(vec![[0, 0, 0]; 3]), fader_leds: Some(vec![[0, 0, 0]; 5]) }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The wheel's fields are global and `Arcade::new` loads them: tests touching them take turns.
    static WHEEL_FIELDS: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn wheel_fields() -> std::sync::MutexGuard<'static, ()> {
        WHEEL_FIELDS.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn ms(t: Instant, n: u64) -> Instant {
        t + Duration::from_millis(n)
    }

    #[test]
    fn mole_hits_score_and_misses_end_the_game() {
        let t = Instant::now();
        let mut g = Mole::new(42, t);
        g.tick(ms(t, 900));
        let up = (0..KEYS).find(|i| g.up[*i].is_some()).expect("a mole is up");
        g.input(Input::Key { index: up, down: true }, ms(t, 950));
        assert_eq!(g.score, 1);
        // nobody hits: moles hide and count as misses
        let mut now = ms(t, 1000);
        while g.over().is_none() {
            now += Duration::from_millis(100);
            g.tick(now);
            assert!(now < ms(t, 20_000), "the game ends");
        }
        assert_eq!(g.over(), Some(1));
        assert_eq!(g.screen(now).keys.len(), KEYS);
    }

    #[test]
    fn simon_plays_rounds_and_ends_on_a_wrong_key() {
        let t = Instant::now();
        let mut g = Simon::new(7, t);
        let mut now = t;
        // let it show the first step, then repeat it
        while !matches!(g.phase, SimonPhase::Repeat { .. }) {
            now += Duration::from_millis(50);
            g.tick(now);
        }
        g.input(Input::Key { index: g.seq[0], down: true }, now);
        assert!(matches!(g.phase, SimonPhase::Pause(_)));
        while !matches!(g.phase, SimonPhase::Repeat { .. }) {
            now += Duration::from_millis(50);
            g.tick(now);
        }
        assert_eq!(g.seq.len(), 2);
        let wrong = (g.seq[0] + 1) % KEYS;
        g.input(Input::Key { index: wrong, down: true }, now);
        assert_eq!(g.over(), Some(1));
    }

    #[test]
    fn safe_opens_with_the_right_numbers() {
        let t = Instant::now();
        let mut g = Safe::new(3, t);
        for i in 0..3 {
            let target = g.combo[i];
            let steps = (target - g.dial).rem_euclid(SAFE_NUMBERS);
            for _ in 0..steps {
                g.input(Input::Dial(1), ms(t, 100));
            }
            assert_eq!(g.closeness(), 1.0);
            g.input(Input::DialClick, ms(t, 200));
        }
        assert!(g.over().is_some_and(|s| s > 500), "opened fast: most of the minute left");
        // a wrong push costs time
        let mut g = Safe::new(3, t);
        g.dial = (g.combo[0] + 5) % SAFE_NUMBERS;
        g.input(Input::DialClick, ms(t, 100));
        assert_eq!(g.penalty, SAFE_PENALTY);
    }

    #[test]
    fn arcade_launches_scores_and_leaves() {
        let dir = std::env::temp_dir().join(format!("hubdeck-games-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let t = Instant::now();
        let _fields = wheel_fields();
        let mut a = Arcade::new(&dir);
        assert_eq!(a.screen(t).keys[0].label.as_deref(), Some("Mole"));
        a.input(Input::Key { index: 0, down: true }, t);
        assert!(matches!(a.mode, Mode::Playing { .. }));
        // hold the dial: back to the launcher, hold again: leave
        a.dial_press(true, t);
        a.tick(ms(t, 1100));
        assert!(matches!(a.mode, Mode::Launcher));
        a.dial_press(false, ms(t, 1200));
        a.dial_press(true, ms(t, 1300));
        a.tick(ms(t, 2400));
        assert!(a.wants_to_leave());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn snake_eats_and_dies_at_the_wall() {
        let t = Instant::now();
        let mut g = Snake::new(5, t);
        g.food = (7, 8);
        let mut now = t;
        now += Duration::from_millis(300);
        g.tick(now);
        assert_eq!(g.eaten, 1);
        assert!(cell_visible(g.food.0, g.food.1), "new food is where it can be seen");
        // run straight into the right wall
        while g.over().is_none() {
            now += Duration::from_millis(300);
            g.tick(now);
            assert!(now < t + Duration::from_secs(60));
        }
        assert_eq!(g.over(), Some(1));
        // a turn: right then down
        let mut g = Snake::new(5, t);
        g.input(Input::Dial(1), t);
        g.tick(t + Duration::from_millis(300));
        assert_eq!(g.dir, (0, 1));
    }

    #[test]
    fn arena_shield_blocks_and_core_takes_hits() {
        let t = Instant::now();
        let mut g = Arena::new(9, t);
        g.next_spawn = t + Duration::from_secs(3600);
        g.enemies.push(Enemy { angle: 0.0, radius: 70.0 });
        g.enemies.push(Enemy { angle: 180.0, radius: 25.0 });
        g.tick(t + Duration::from_millis(200));
        g.tick(t + Duration::from_millis(400));
        assert_eq!(g.score, 1, "the shield at 0 degrees stopped the first");
        assert_eq!(g.lives, 2, "the second reached the core");
        assert_eq!(angle_gap(350.0, 10.0), 20.0);
    }

    #[test]
    fn balance_scores_only_on_target_and_ends() {
        let t = Instant::now();
        let mut g = Balance::new(4, t);
        let mut now = t;
        for _ in 0..10 {
            now += Duration::from_millis(100);
            for i in 0..5 {
                let target = g.target(i, now);
                g.input(Input::Fader(i, target.round() as u8), now);
            }
            g.tick(now);
        }
        assert!(g.score > 5.0, "all five on target for a second: {}", g.score);
        g.tick(t + BALANCE_TIME + Duration::from_millis(10));
        assert!(g.over().is_some());
    }

    #[test]
    fn games_needing_missing_modules_do_not_start() {
        let dir = std::env::temp_dir().join(format!("hubdeck-games2-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let t = Instant::now();
        let _fields = wheel_fields();
        let mut a = Arcade::new(&dir);
        a.set_modules(vec!["keys".into(), "dial".into()]);
        let balance = catalog().iter().position(|e| e.id == "balance").unwrap();
        a.input(Input::Key { index: balance, down: true }, t);
        assert!(matches!(a.mode, Mode::Launcher), "no faders: Balance stays shut");
        let snake = catalog().iter().position(|e| e.id == "snake").unwrap();
        a.input(Input::Key { index: snake, down: true }, t);
        assert!(matches!(a.mode, Mode::Playing { .. }));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn wheel_spins_and_settles() {
        let _fields = wheel_fields();
        let t = Instant::now();
        let mut g = Wheel::new(1, t);
        g.input(Input::Dial(8), t);
        let mut now = t;
        while g.result.is_none() {
            now += Duration::from_millis(30);
            g.tick(now);
            assert!(now < t + Duration::from_secs(60), "it stops by itself");
        }
        assert!(g.result.unwrap() < g.labels.len());
        assert_eq!(set_wheel_choices(vec!["  yes ".into(), "".into()]).len(), 8, "one field is not a wheel: defaults");
        assert_eq!(set_wheel_choices(vec!["yes".into(), "no".into()]), vec!["yes", "no"]);
        assert_eq!(Wheel::new(1, t).labels, vec!["yes", "no"]);
        set_wheel_choices(Vec::new());
        assert_eq!(g.over(), None, "the wheel never ends a game");
    }

    #[test]
    fn duel_points_and_false_starts() {
        let t = Instant::now();
        let mut g = Duel::new(2, t);
        g.input(Input::Key { index: 0, down: true }, t);
        assert_eq!(g.score, [0, 0], "a false start at zero stays zero");
        g.tick(g.next_at);
        let (k, _) = g.lit.unwrap();
        let mirror = (k / 5) * 5 + (4 - k % 5);
        g.input(Input::Key { index: mirror, down: true }, g.next_at);
        let winner = duel_side(mirror).unwrap();
        assert_eq!(g.score[winner], 1);
        assert_eq!(duel_side(2), None, "the middle column is nobody's");
    }

    #[test]
    fn pong_scores_when_the_ball_passes() {
        let t = Instant::now();
        let mut g = Pong::new(3, t);
        g.ball = (5.0, 10.0);
        g.vel = (-300.0, 0.0);
        g.paddles[0] = 250.0;
        g.tick(t + Duration::from_millis(40));
        assert_eq!(g.score, [0, 1], "missed on the left: a point for the deck");
        g.input(Input::Dial(-50), t);
        assert_eq!(g.paddles[0], PADDLE_H / 2.0, "the paddle stops at the top");
    }

    #[test]
    fn rhythm_hits_on_the_beat_and_counts_misses() {
        let t = Instant::now();
        let mut g = Rhythm::new(4, t);
        g.notes.push((2, t + Duration::from_millis(500)));
        g.input(Input::Key { index: 12, down: true }, t + Duration::from_millis(480));
        assert_eq!(g.score, 1);
        g.notes.clear();
        g.next_note = t + Duration::from_secs(3600);
        g.notes.push((0, t + Duration::from_millis(100)));
        g.tick(t + Duration::from_millis(600));
        assert_eq!(g.misses, 1);
    }

    #[test]
    fn memory_pairs_and_mismatches() {
        let t = Instant::now();
        let mut g = Memory::new(6, t);
        // find two keys with the same picture
        let a = (0..KEYS).find(|k| g.cards[*k].is_some()).unwrap();
        let b = (0..KEYS).find(|k| *k != a && g.cards[*k] == g.cards[a]).unwrap();
        let wrong = (0..KEYS).find(|k| g.cards[*k].is_some() && g.cards[*k] != g.cards[a]).unwrap();
        g.input(Input::Key { index: a, down: true }, t);
        g.input(Input::Key { index: wrong, down: true }, t);
        assert!(g.hide_at.is_some(), "no match: they turn back");
        g.tick(t + Duration::from_secs(1));
        assert!(g.up.is_empty());
        g.input(Input::Key { index: a, down: true }, t);
        g.input(Input::Key { index: b, down: true }, t);
        assert!(g.found[a] && g.found[b]);
        assert_eq!(g.moves, 2);
        assert_eq!(g.cards[MEMORY_SCORE_KEY], None);
        assert_eq!(g.cards.iter().filter(|c| c.is_some()).count(), 14);
    }

    #[test]
    fn reflex_measures_and_punishes_early_presses() {
        let t = Instant::now();
        let mut g = Reflex::new(7, t);
        g.input(Input::Key { index: 0, down: true }, t);
        assert!(g.early);
        for round in 0..REFLEX_ROUNDS {
            let at = g.next_at;
            g.tick(at);
            let (k, _) = g.lit.unwrap();
            g.input(Input::Key { index: k, down: true }, at + Duration::from_millis(200 + round as u64 * 10));
        }
        assert_eq!(g.average(), Some(220));
        assert_eq!(g.over(), Some(780));
    }

    #[test]
    fn colors() {
        assert_eq!(hsl(0, 100, 50), "#ff0000");
        assert_eq!(hsl(120, 100, 50), "#00ff00");
    }
}
