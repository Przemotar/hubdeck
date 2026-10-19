//! The dial pet, Mochi (owner's picks, 2026-10-14: a sweet companion that lives only on the dial;
//! its mood follows the day and shows in the dial's color, nowhere else on the deck; it never
//! dies, it only gets sad and falls asleep). This is the first version: turning plays with it, a
//! press pets it, a double press feeds it, a hold puts it to sleep or wakes it. Growing up,
//! accessories and the treat game come later.
//!
//! The state lives in `pet.json` next to the profiles, so Mochi remembers across restarts.

use std::path::Path;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub const ACTION: &str = "core.pet";

/// What a gesture does to the pet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PetCmd {
    /// Detents: it plays and looks that way.
    Play(i32),
    Pet,
    Feed,
    /// To sleep, or awake.
    Sleep,
}

/// Mochi's needs, 0..=1, and when they were last worked out (seconds since 1970).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Needs {
    pub fullness: f32,
    pub joy: f32,
    pub asleep: bool,
    pub at: u64,
    /// What a profile's dial had before a key put Mochi there, by profile id (`{"dial": binding,
    /// "dial:double": .., "dial:hold": ..}`), given back when it is taken off.
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub dials: std::collections::BTreeMap<String, serde_json::Value>,
    /// Mochi's own dial bindings as the user set them (its name, body, fit; `{"dial": binding, ..}`),
    /// kept while a key has taken it off, so it comes back as it was (Discussions #3: hidden and
    /// shown again, its name and body went back to the defaults).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mochi: Option<serde_json::Value>,
}

impl Default for Needs {
    fn default() -> Self {
        Self { fullness: 0.8, joy: 0.8, asleep: false, at: now_secs(), dials: Default::default(), mochi: None }
    }
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Fullness goes from full to empty in about 8 hours, joy in about 5; both rest while it sleeps.
const FULL_HOURS: f32 = 8.0;
/// Sad after about 8.5 hours alone (owner's 2a, 2026-10-16: at 5 it was sad every time he looked).
const JOY_HOURS: f32 = 12.0;

impl Needs {
    /// Time passes (`now` in seconds since 1970).
    pub fn age(&mut self, now: u64) {
        let hours = now.saturating_sub(self.at) as f32 / 3600.0;
        if !self.asleep {
            self.fullness = (self.fullness - hours / FULL_HOURS).max(0.0);
            self.joy = (self.joy - hours / JOY_HOURS).max(0.0);
        }
        self.at = now;
    }
}

/// How it feels, from its needs, the time of day and the computer (locked = asleep).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mood {
    Happy,
    Hungry,
    Sad,
    Sleepy,
    Asleep,
}

impl Mood {
    /// The dial's background for the mood (the only place it shows).
    fn sky(self) -> (&'static str, &'static str) {
        match self {
            Mood::Happy => ("#2a2250", "#120f22"),
            Mood::Hungry => ("#3a2618", "#170e0a"),
            Mood::Sad => ("#1d2a3d", "#0b1018"),
            Mood::Sleepy => ("#141a42", "#05060f"),
            Mood::Asleep => ("#0f1438", "#04050c"),
        }
    }
}

/// The last thing that happened, for its little animation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Moment {
    Hearts,
    Munch,
    Hop(i32),
    Yawn,
}

/// Mochi as the engine keeps it.
#[derive(Debug, Clone)]
pub struct Pet {
    pub needs: Needs,
    pub moment: Option<(Moment, Instant)>,
    /// Where it looks: -1 left .. 1 right, from the last turn.
    pub look: f32,
    file: Option<std::path::PathBuf>,
}

impl Pet {
    pub fn load(data_dir: &Path) -> Self {
        let file = data_dir.join("pet.json");
        let mut needs: Needs = std::fs::read(&file).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        needs.age(now_secs());
        Self { needs, moment: None, look: 0.0, file: Some(file) }
    }

    pub fn save(&self) {
        if let Some(f) = &self.file
            && let Ok(json) = serde_json::to_vec_pretty(&self.needs)
            && let Err(e) = super::profile::write_atomic(f, &json)
        {
            log::warn!("pet.json: {e}");
        }
    }

    pub fn apply(&mut self, cmd: PetCmd, now: Instant) {
        self.needs.age(now_secs());
        match cmd {
            PetCmd::Play(_) | PetCmd::Pet | PetCmd::Feed if self.needs.asleep => {
                // any touch wakes it, with a yawn (owner, 2026-10-14: it "slept all the time", only
                // a second hold woke it)
                self.needs.asleep = false;
                self.moment = Some((Moment::Yawn, now));
            }
            PetCmd::Play(d) => {
                self.look = (d as f32 / 3.0).clamp(-1.0, 1.0);
                self.needs.joy = (self.needs.joy + 0.03 * d.unsigned_abs() as f32).min(1.0);
                self.moment = Some((Moment::Hop(d.signum()), now));
            }
            PetCmd::Pet => {
                self.needs.joy = (self.needs.joy + 0.15).min(1.0);
                self.moment = Some((Moment::Hearts, now));
            }
            PetCmd::Feed => {
                self.needs.fullness = (self.needs.fullness + 0.35).min(1.0);
                self.moment = Some((Moment::Munch, now));
            }
            PetCmd::Sleep => {
                self.needs.asleep = !self.needs.asleep;
                self.moment = Some((Moment::Yawn, now));
            }
        }
        self.save();
    }

    /// How it feels now: asleep while the computer is locked or put to bed, sleepy late at night.
    pub fn mood(&self, locked: bool, hour: u32) -> Mood {
        if self.needs.asleep || locked {
            Mood::Asleep
        } else if self.needs.fullness < 0.25 {
            Mood::Hungry
        } else if self.needs.joy < 0.3 {
            Mood::Sad
        } else if !(7..23).contains(&hour) {
            Mood::Sleepy
        } else {
            Mood::Happy
        }
    }
}

/// The dust floating in the background, in the mood's color.
fn dust(mood: Mood) -> &'static str {
    match mood {
        Mood::Happy => "#b2a6ff",
        Mood::Hungry => "#ffb15c",
        Mood::Sad => "#7fa6d9",
        Mood::Sleepy | Mood::Asleep => "#8f9cff",
    }
}

const SHAPE: &str = "M60 170 Q60 72 120 72 Q180 72 180 170 Q180 196 120 196 Q60 196 60 170Z";
const HEART: &str = "M0 -4 C0 -10 -9 -10 -9 -3 C-9 3 0 8 0 11 C0 8 9 3 9 -3 C9 -10 0 -10 0 -4Z";
const INK: &str = "#1a1433";
const MOUTH: &str = "#2a1240";
/// The deck's own font (the only one the renderer has: any other name drew boxes for missing glyphs).
const FONT: &str = super::render::FONT_FAMILY;

/// Everything Mochi does at rest repeats every `LOOP` seconds on a grid of `STEP` (one engine
/// tick), so each resting frame is drawn once and then only sent again (owner, 2026-10-14: more
/// than 12 frames a second without doubling the CPU).
pub const LOOP: f32 = 16.8;
const STEP_MS: u64 = 40;
const LOOP_FRAMES: u64 = 420;

/// The clock Mochi is drawn at, on the loop's grid (whole milliseconds: an `f32` of seconds gets
/// coarser than a step after a few days).
pub fn loop_clock() -> f32 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    loop_at(START.get_or_init(Instant::now).elapsed())
}

fn loop_at(since: Duration) -> f32 {
    ((since.as_millis() as u64 / STEP_MS) % LOOP_FRAMES * STEP_MS) as f32 / 1000.0
}

/// A speed that turns `k` whole times in a loop.
fn w(k: f32) -> f32 {
    2.0 * std::f32::consts::PI * k / LOOP
}

/// Mochi on the dial at `t` seconds, with its name and mood on top (owner's pick, 2026-10-14: the
/// polished one: soft light from above, a shine and a lit edge, a jelly squash on every touch,
/// glossy eyes, lids when sleepy, a tear when sad, hearts that sway up, a bitten cookie, a
/// nightcap; still a drawing made by code, no picture files).
pub fn dial_svg(pet: &Pet, mood: Mood, t: f32, now: Instant, rotation: i32) -> String {
    dial_svg_with(pet, mood, t, now, rotation, None)
}

/// The action's param of a picture of the user's own for Mochi's body (Discussions #3; owner,
/// 2026-10-18: "tylko ta grafika bez oczu i ust i emocji"): only the body changes, the face, the
/// moods and every reaction are drawn over it as on Mochi.
pub const BODY_PARAM: &str = "body";

/// The param of how the body picture sits: `{size, x, y}`, size in percent, x and y in dial pixels.
pub const FIT_PARAM: &str = "bodyFit";

/// A body picture of the user's own and how it sits (owner's 2a, 2026-10-18: "położenie i
/// rozmiar ... żeby sobie sam mógł dopasować ten obrazek").
#[derive(Debug, Clone, PartialEq)]
pub struct Body {
    pub name: String,
    /// 1.0 = as big as Mochi.
    pub size: f32,
    pub x: f32,
    pub y: f32,
}

/// The body picture in a binding's params; None: Mochi's own body.
pub fn body_of(params: &serde_json::Value) -> Option<Body> {
    let name = params.get(BODY_PARAM).and_then(|v| v.as_str()).map(str::trim).filter(|v| !v.is_empty())?;
    let fit = params.get(FIT_PARAM);
    let num = |k: &str, d: f64, lo: f64, hi: f64| fit.and_then(|f| f.get(k)).and_then(|v| v.as_f64()).unwrap_or(d).clamp(lo, hi) as f32;
    Some(Body { name: name.to_string(), size: num("size", 100.0, 40.0, 200.0) / 100.0, x: num("x", 0.0, -100.0, 100.0), y: num("y", 0.0, -100.0, 100.0) })
}

/// Mochi with the body of these params in four moments: happy, petted, hungry, asleep (the Fit
/// field's previews); empty without a body picture.
pub fn previews(params: &serde_json::Value) -> Vec<String> {
    let Some(body) = body_of(params) else { return Vec::new() };
    let now = Instant::now();
    let pet = |moment: Option<(Moment, f32)>| Pet { needs: Needs::default(), moment: moment.map(|(m, age)| (m, now - Duration::from_secs_f32(age))), look: 0.0, file: None };
    [(pet(None), Mood::Happy), (pet(Some((Moment::Hearts, 0.6))), Mood::Happy), (pet(None), Mood::Hungry), (pet(None), Mood::Asleep)]
        .iter()
        .map(|(p, mood)| dial_svg_with(p, *mood, 0.3, now, 0, Some(&body)))
        .collect()
}

/// The dial's param naming the key that put Mochi there.
pub const FROM_PARAM: &str = "from";

/// Mochi's body on the dial when the dial's own binding has none: the one set on the key that put
/// it there (`from`), else on any Mochi key of the profile, in their order.
pub fn body_on(controls: &std::collections::BTreeMap<String, super::profile::Binding>, from: Option<&str>) -> Option<Body> {
    let mochi = |b: &&super::profile::Binding| b.action == ACTION;
    from.and_then(|f| controls.get(f)).filter(mochi).and_then(|b| body_of(&b.params)).or_else(|| controls.values().filter(mochi).find_map(|b| body_of(&b.params)))
}

/// Mochi's bindings among the dial's `gestures`, as kept in [`Needs::mochi`] when a key takes it
/// off: everything the user set, without the key that brought it.
pub fn kept_bindings(controls: &std::collections::BTreeMap<String, super::profile::Binding>, gestures: &[&str]) -> Option<serde_json::Value> {
    let kept: serde_json::Map<String, serde_json::Value> = gestures
        .iter()
        .filter_map(|g| controls.get(*g).filter(|b| b.action == ACTION).map(|b| (g.to_string(), b)))
        .map(|(g, b)| {
            let mut b = b.clone();
            if let Some(params) = b.params.as_object_mut() {
                params.remove(FROM_PARAM);
            }
            (g, serde_json::json!(b))
        })
        .collect();
    (!kept.is_empty()).then_some(serde_json::Value::Object(kept))
}

/// A body picture made once and kept, the last one used (the dial draws up to 25 frames a second).
fn body_picture(name: &str) -> Option<(String, bool)> {
    type Kept = Option<(String, Option<(String, bool)>)>;
    static KEPT: std::sync::Mutex<Kept> = std::sync::Mutex::new(None);
    let mut kept = KEPT.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((n, picture)) = kept.as_ref()
        && n == name
    {
        return picture.clone();
    }
    // about the body's size: a bigger picture costs every frame and shows nothing more
    let picture = super::images::fitted(name, 160);
    *kept = Some((name.to_string(), picture.clone()));
    picture
}

/// Mochi with a body picture of the user's own when `body` names one (see [`BODY_PARAM`]).
pub fn dial_svg_with(pet: &Pet, mood: Mood, t: f32, now: Instant, rotation: i32, body: Option<&Body>) -> String {
    use std::f32::consts::PI;
    let (s, c) = (240.0_f32, 120.0_f32);
    let (top, bottom) = mood.sky();
    let asleep = mood == Mood::Asleep;
    let moment = pet.moment.filter(|(_, at)| now.duration_since(*at) < Duration::from_millis(2400)).map(|(m, at)| (m, now.duration_since(at).as_secs_f32()));
    let age = moment.map_or(99.0, |(_, a)| a);
    let is = |m: Moment| moment.is_some_and(|(x, _)| std::mem::discriminant(&x) == std::mem::discriminant(&m));
    // jelly: a press squashes and springs back; a hop stretches up and squashes on landing
    let breath = if asleep { (t * w(4.0)).sin() * 0.045 } else { (t * w(6.0)).sin() * 0.025 };
    let (mut sx, mut sy, mut lift) = (1.0 - breath, 1.0 + breath, 0.0_f32);
    if (is(Moment::Hearts) || is(Moment::Munch)) && age < 1.0 {
        let k = (-5.0 * age).exp() * (16.0 * age).cos() * 0.14;
        sx += k;
        sy -= k;
    }
    if is(Moment::Hop(0)) && age < 0.8 {
        if age < 0.45 {
            let u = (age / 0.45 * PI).sin();
            lift = u * 20.0;
            sy += 0.08 * u;
            sx -= 0.05 * u;
        } else {
            let k = (-7.0 * (age - 0.45)).exp() * (20.0 * (age - 0.45)).cos() * 0.12;
            sx += k;
            sy -= k;
        }
    }
    let tilt = pet.look * 4.0 + if mood == Mood::Sleepy { (t * w(2.0)).sin() * 3.0 } else { 0.0 };
    let m = mood as u8;
    let mut b = format!(
        r##"<defs><radialGradient id="psk{m}" cx="50%" cy="28%" r="80%"><stop offset="0" stop-color="{top}"/><stop offset="1" stop-color="{bottom}"/></radialGradient><radialGradient id="pbd" cx="36%" cy="28%" r="78%"><stop offset="0" stop-color="#ece6ff"/><stop offset=".38" stop-color="#b3a7ff"/><stop offset=".78" stop-color="#8371f2"/><stop offset="1" stop-color="#5d4bd2"/></radialGradient><linearGradient id="psh" x1="0" y1="0" x2="0" y2="1"><stop offset=".55" stop-color="#2a1a6e" stop-opacity="0"/><stop offset="1" stop-color="#2a1a6e" stop-opacity=".38"/></linearGradient><radialGradient id="pey" cx="50%" cy="30%"><stop offset="0" stop-color="#3a2d6e"/><stop offset="1" stop-color="#140f2b"/></radialGradient><linearGradient id="php" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#ffb3dc"/><stop offset="1" stop-color="#f0569c"/></linearGradient><radialGradient id="pck" cx="40%" cy="35%"><stop offset="0" stop-color="#f2c48a"/><stop offset="1" stop-color="#c98a45"/></radialGradient><radialGradient id="pshd"><stop offset="0" stop-color="#000" stop-opacity=".9"/><stop offset="1" stop-color="#000" stop-opacity="0"/></radialGradient><radialGradient id="pglo"><stop offset="0" stop-color="#fff" stop-opacity=".9"/><stop offset="1" stop-color="#fff" stop-opacity="0"/></radialGradient><radialGradient id="pblu"><stop offset="0" stop-color="#ff8fc7" stop-opacity=".9"/><stop offset="1" stop-color="#ff8fc7" stop-opacity="0"/></radialGradient></defs><rect width="{s}" height="{s}" fill="url(#psk{m})"/>"##
    );
    let dust_color = dust(mood);
    for i in 0..9 {
        let fi = i as f32;
        let x = (30.0 + fi * 23.0 + (t * w(1.0) + fi).sin() * 8.0) % 220.0 + 10.0;
        let y = 40.0 + ((i * 37) % 120) as f32 - (t * (40.0 * 3.0 / LOOP) + fi * 13.0) % 40.0;
        b += &format!(r#"<circle cx="{x:.1}" cy="{y:.1}" r="{:.1}" fill="{dust_color}" opacity="{:.2}"/>"#, 1.0 + (i % 3) as f32 * 0.6, 0.18 + 0.15 * (t * w(3.0) + fi).sin());
    }
    if matches!(mood, Mood::Sleepy | Mood::Asleep) {
        for i in 0..10 {
            let (x, y) = (34 + (i * 47) % 172, 30 + (i * 29) % 70);
            b += &format!(r##"<circle cx="{x}" cy="{y}" r="1.3" fill="#fff" opacity="{:.2}"/>"##, 0.3 + 0.5 * ((t * w(5.0) + i as f32 * 1.3).sin() * 0.5 + 0.5));
        }
    }
    // the shadow shrinks while it is up in the air
    // soft edges from gradients, not blur filters: a blur was 60% of a frame (2026-10-14)
    b += &format!(r##"<ellipse cx="120" cy="199" rx="{:.1}" ry="12" fill="url(#pshd)" opacity="{:.2}"/>"##, 60.0 * (1.0 - lift / 60.0), 0.55 - lift / 90.0);
    b += &format!(r#"<g transform="translate(0 {:.1}) rotate({tilt:.1} 120 196) translate(120 196) scale({sx:.3} {sy:.3}) translate(-120 -196)">"#, -lift);
    match body.and_then(|fit| body_picture(&fit.name).map(|p| (p, fit))) {
        // the picture stands where Mochi stands, a little bigger than it, in its own shape: what
        // is see-through in it stays so, a picture without see-through parts is its rectangle
        // (owner, 2026-10-18: "alfa jest brana z obrazka - a nie że zawsze jest kształt mochi");
        // its size grows from the ground up
        Some(((uri, _), fit)) => {
            let (w, h) = (152.0 * fit.size, 140.0 * fit.size);
            b += &format!(r#"<image href="{uri}" x="{:.1}" y="{:.1}" width="{w:.1}" height="{h:.1}" preserveAspectRatio="xMidYMax meet"/>"#, 120.0 - w / 2.0 + fit.x, 196.0 - h + fit.y)
        }
        None => {
            b += &format!(r#"<path d="{SHAPE}" fill="url(#pbd)"/><path d="{SHAPE}" fill="url(#psh)"/>"#);
            // the soft shine on the left (the lit edge on the right went: owner, "wygląda słabo")
            b += r##"<ellipse cx="94" cy="100" rx="22" ry="14" fill="url(#pglo)" opacity=".6" transform="rotate(-28 94 100)"/><circle cx="86" cy="95" r="3.2" fill="#fff" opacity=".9"/>"##;
        }
    }
    // eyes: shut asleep or blinking, happy arcs while petted, glossy otherwise (lids when sleepy)
    let lx = pet.look * 6.0;
    let blink = !asleep && (t % 4.2) > 4.05;
    let happy_eyes = is(Moment::Hearts) && age < 1.8;
    for x in [98.0_f32, 142.0] {
        let ex = x + lx;
        if asleep || blink {
            b += &format!(r#"<path d="M{:.1} 132 Q{ex:.1} 140 {:.1} 132" fill="none" stroke="{INK}" stroke-width="4" stroke-linecap="round"/>"#, ex - 10.0, ex + 10.0);
        } else if happy_eyes {
            b += &format!(r#"<path d="M{:.1} 134 Q{ex:.1} 122 {:.1} 134" fill="none" stroke="{INK}" stroke-width="4.5" stroke-linecap="round"/>"#, ex - 10.0, ex + 10.0);
        } else {
            if mood == Mood::Sleepy {
                // tired: half shut under a heavy, nearly flat lid, a soft shadow under the eye
                b += &format!(
                    r##"<ellipse cx="{ex:.1}" cy="145" rx="9" ry="3" fill="#5a46c8" opacity=".3"/><path d="M{l:.1} 131 A10.5 10 0 0 0 {r:.1} 131 Z" fill="url(#pey)"/><circle cx="{:.1}" cy="134.5" r="2.2" fill="#fff" opacity=".85"/><path d="M{l2:.1} 131 Q{ex:.1} 128 {r2:.1} 131" fill="none" stroke="{INK}" stroke-width="3.8" stroke-linecap="round"/>"##,
                    ex + 3.0,
                    l = ex - 10.5,
                    r = ex + 10.5,
                    l2 = ex - 12.0,
                    r2 = ex + 12.0
                );
            } else {
                b += &format!(
                    r##"<ellipse cx="{ex:.1}" cy="130" rx="10.5" ry="13.5" fill="url(#pey)"/><circle cx="{:.1}" cy="124.5" r="3.8" fill="#fff"/><circle cx="{:.1}" cy="136" r="1.7" fill="#fff" opacity=".8"/>"##,
                    ex + 3.5,
                    ex - 3.5
                );
            }
        }
    }
    b += r##"<ellipse cx="82" cy="153" rx="14" ry="8" fill="url(#pblu)" opacity=".6"/><ellipse cx="158" cy="153" rx="14" ry="8" fill="url(#pblu)" opacity=".6"/>"##;
    // the mouth
    b += &if is(Moment::Munch) && age < 2.0 {
        format!(r#"<ellipse cx="120" cy="158" rx="8" ry="{:.1}" fill="{MOUTH}"/>"#, ((age * 13.0).sin() * 0.5 + 0.5) * 6.0 + 3.0)
    } else if is(Moment::Yawn) && age < 1.5 {
        let o = 3.0 + (age / 1.5 * PI).sin() * 8.0;
        format!(r##"<ellipse cx="120" cy="159" rx="7.5" ry="{o:.1}" fill="{MOUTH}"/><ellipse cx="120" cy="{:.1}" rx="4" ry="{:.1}" fill="#ff7aa8"/>"##, 159.0 + o * 0.5, o * 0.35)
    } else {
        match mood {
            Mood::Sad => {
                let fall = (t * (24.0 * 13.0 / LOOP)) % 24.0;
                format!(
                    r##"<path d="M109 161 Q120 151 131 161" fill="none" stroke="{INK}" stroke-width="4" stroke-linecap="round"/><path d="M{:.1} {:.1} q4 7 0 10 q-4 -3 0 -10z" fill="#9ed2ff" opacity="{:.2}"/>"##,
                    150.0 + lx,
                    140.0 + fall,
                    1.0 - fall / 24.0
                )
            }
            Mood::Hungry => format!(r#"<path d="M108 158 Q114 152 120 158 Q126 164 132 158" fill="none" stroke="{INK}" stroke-width="4" stroke-linecap="round"/>"#),
            Mood::Asleep => format!(r#"<ellipse cx="120" cy="157" rx="4" ry="3" fill="{MOUTH}"/>"#),
            _ if happy_eyes => format!(r##"<path d="M108 152 Q120 170 132 152 Z" fill="{MOUTH}"/><path d="M113 158 Q120 166 127 158 Z" fill="#ff7aa8"/>"##),
            _ => format!(r#"<path d="M108 153 Q114 160 120 154 Q126 160 132 153" fill="none" stroke="{INK}" stroke-width="4" stroke-linecap="round"/>"#),
        }
    };
    if asleep {
        b += r##"<path d="M78 92 Q96 60 132 66 Q168 70 186 54 Q176 92 160 96 Q120 84 78 92Z" fill="#2f3a8f"/><path d="M76 92 Q120 80 162 96" fill="none" stroke="#e8ecff" stroke-width="7" stroke-linecap="round"/><circle cx="188" cy="54" r="8" fill="#e8ecff"/>"##;
    }
    b += "</g>";
    // hearts float up and sway
    if is(Moment::Hearts) {
        for (i, (x, delay)) in [(150.0_f32, 0.0_f32), (176.0, 0.22), (92.0, 0.4)].into_iter().enumerate() {
            let p = (age - delay).max(0.0) / 1.9;
            if p > 0.0 && p < 1.0 {
                let scale = (0.8 + i as f32 * 0.15) * if p < 0.15 { p / 0.15 } else { 1.0 };
                b += &format!(
                    r#"<path d="{HEART}" fill="url(#php)" transform="translate({:.1} {:.1}) scale({scale:.2})" opacity="{:.2}"/>"#,
                    x + (p * 8.0 + i as f32).sin() * 6.0,
                    100.0 - p * 58.0,
                    1.0 - p * p
                );
            }
        }
    }
    // the cookie, bitten three times, crumbs falling
    if is(Moment::Munch) && age < 1.4 {
        let bites = ((age / 0.35) as usize).min(3);
        // the bites cut the cookie (a mask), whatever is behind it shows through
        b += r##"<mask id="pbite"><rect x="-20" y="-20" width="40" height="40" fill="#fff"/>"##;
        for k in 0..bites {
            b += &format!(r##"<circle cx="{}" cy="-13" r="6.5" fill="#000"/>"##, -8 + k as i32 * 8);
        }
        b += r##"</mask><g transform="translate(120 186)"><g mask="url(#pbite)"><circle r="14" fill="url(#pck)"/><circle cx="-5" cy="-3" r="2.2" fill="#6b3a12"/><circle cx="5" cy="4" r="2.2" fill="#6b3a12"/><circle cx="2" cy="-6" r="1.6" fill="#6b3a12"/></g></g>"##;
        for k in 0..6 {
            let p = (age - 0.3 - k as f32 * 0.08).max(0.0);
            if p > 0.0 {
                b += &format!(r##"<circle cx="{}" cy="{:.1}" r="1.6" fill="#d9a066" opacity="{:.2}"/>"##, 108 + k * 5, 190.0 + p * 40.0, (1.0 - p * 1.4).max(0.0));
            }
        }
    }
    // what it wants, in a thought bubble with a picture and no words (owner, 2026-10-16: "nie
    // wiedziałem czemu był smutny i nie ma napisane co trzeba zrobić"; no text over it, his pick
    // of 2026-10-14): a cookie when hungry (double press), a heart when sad (press), a moon late
    // at night (hold); gone while a touch plays and once it has what it wanted
    if moment.is_none() {
        let icon = match mood {
            Mood::Hungry => Some(r##"<circle r="12" fill="url(#pck)"/><circle cx="-4.5" cy="-3" r="2" fill="#6b3a12"/><circle cx="4.5" cy="3.5" r="2" fill="#6b3a12"/><circle cx="1.5" cy="-6" r="1.5" fill="#6b3a12"/><circle cx="-3" cy="5" r="1.5" fill="#6b3a12"/>"##.to_string()),
            Mood::Sad => Some(format!(r#"<path d="{HEART}" fill="url(#php)" transform="translate(0 -2) scale(1.15)"/>"#)),
            Mood::Sleepy => Some(r##"<circle r="11" fill="#ffd76a"/><circle cx="6" cy="-4" r="10" fill="#f4f1ff"/>"##.to_string()),
            _ => None,
        };
        if let Some(icon) = icon {
            let bob = (t * w(3.0)).sin() * 2.0;
            b += &format!(
                r##"<g transform="translate(0 {bob:.1})"><circle cx="160" cy="96" r="4.5" fill="#f4f1ff" opacity=".9"/><circle cx="170" cy="84" r="7" fill="#f4f1ff" opacity=".92"/><circle cx="186" cy="60" r="24" fill="#f4f1ff" opacity=".95"/><g transform="translate(186 60)">{icon}</g></g>"##
            );
            // and how to give it: a fingertip that taps once (press), twice (double press) or
            // stays down while a ring fills (hold), every 2.1 s (owner, 2026-10-16: "może powinna
            // być animacja gestu pokazana?"); on the loop's grid, so the resting frames still repeat
            let cycle = LOOP / 8.0;
            let p = (t % cycle) / cycle;
            let (fx, fy) = (206.0_f32, 108.0_f32);
            let mut g = String::new();
            let tap = |start: f32, g: &mut String| {
                let q = (p - start) / 0.22;
                if (0.0..1.0).contains(&q) {
                    *g += &format!(r##"<circle cx="{fx}" cy="{fy}" r="{:.1}" fill="none" stroke="#f4f1ff" stroke-width="2.6" opacity="{:.2}"/>"##, 9.0 + q * 11.0, 1.0 - q);
                }
            };
            let down = match mood {
                Mood::Sad => {
                    tap(0.1, &mut g);
                    (0.06..0.16).contains(&p)
                }
                Mood::Hungry => {
                    tap(0.1, &mut g);
                    tap(0.3, &mut g);
                    (0.06..0.14).contains(&p) || (0.26..0.34).contains(&p)
                }
                _ => {
                    // hold: the ring fills while the fingertip stays down, then lets go
                    let q = ((p - 0.08) / 0.6).clamp(0.0, 1.0);
                    if p > 0.08 && p < 0.78 {
                        let a = q * std::f32::consts::TAU;
                        let (ex, ey) = (fx + 13.0 * a.sin(), fy - 13.0 * a.cos());
                        let large = if a > std::f32::consts::PI { 1 } else { 0 };
                        g += &format!(r##"<path d="M{fx} {:.1} A13 13 0 {large} 1 {ex:.2} {ey:.2}" fill="none" stroke="#f4f1ff" stroke-width="3" stroke-linecap="round"/>"##, fy - 13.0);
                    }
                    p > 0.08 && p < 0.78
                }
            };
            g += &format!(r##"<circle cx="{fx}" cy="{fy}" r="{:.1}" fill="#f4f1ff" opacity=".95"/>"##, if down { 6.0 } else { 7.5 });
            b += &format!(r#"<g transform="translate(0 {bob:.1})">{g}</g>"#);
        }
    }
    if asleep {
        for i in 0..3 {
            let p = (t * (8.0 / LOOP) + i as f32 / 3.0) % 1.0;
            b += &format!(
                r##"<text x="{:.1}" y="{:.1}" font-family="{FONT}" font-size="{:.0}" fill="#c4bcff" opacity="{:.2}" font-weight="700">z</text>"##,
                196.0 + p * 16.0,
                112.0 - p * 40.0,
                13.0 + p * 10.0,
                (1.0 - p) * 0.9
            );
        }
    }
    // no name or mood over it (owner, 2026-10-14: "nie da się bez tekstu, żeby było czyściej?"):
    // its face and the dial's color say how it feels
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="{s}" height="{s}" viewBox="0 0 {s} {s}"><g transform="rotate({rotation} {c} {c})">{b}</g></svg>"#)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mochi_gets_hungry_and_sad_over_hours_but_not_asleep() {
        let mut n = Needs { fullness: 1.0, joy: 1.0, asleep: false, at: 0, ..Needs::default() };
        n.age(4 * 3600);
        assert!((n.fullness - 0.5).abs() < 0.01 && (n.joy - 2.0 / 3.0).abs() < 0.01);
        let mut s = Needs { fullness: 1.0, joy: 1.0, asleep: true, at: 0, ..Needs::default() };
        s.age(10 * 3600);
        assert_eq!((s.fullness, s.joy), (1.0, 1.0), "asleep, nothing runs down");
    }

    /// What it wants and the gesture, at moments of the 2.1 s gesture cycle:
    /// `HUBDECK_OUT=<png> cargo test -p hubdeck mochi_gestures -- --ignored`.
    #[test]
    #[ignore]
    fn mochi_gestures() {
        use resvg::{tiny_skia, usvg};
        let now = Instant::now();
        let pet = Pet { needs: Needs::default(), moment: None, look: 0.0, file: None };
        let mut out = String::from(r##"<svg xmlns="http://www.w3.org/2000/svg" width="1040" height="800"><rect width="100%" height="100%" fill="#101016"/>"##);
        for (row, mood) in [Mood::Sad, Mood::Hungry, Mood::Sleepy].into_iter().enumerate() {
            for (col, p) in [0.03_f32, 0.15, 0.33, 0.55].into_iter().enumerate() {
                let svg = dial_svg(&pet, mood, p * LOOP / 8.0, now, 0);
                let (x, y) = (20 + col * 255, 20 + row * 260);
                out += &format!(
                    r#"<svg x="{x}" y="{y}" width="240" height="240">{}</svg>"#,
                    svg.trim_start_matches(r#"<svg xmlns="http://www.w3.org/2000/svg" width="240" height="240" viewBox="0 0 240 240">"#).trim_end_matches("</svg>")
                );
            }
        }
        out += "</svg>";
        let mut opt = usvg::Options::default();
        opt.fontdb_mut().load_system_fonts();
        let tree = usvg::Tree::from_str(&out, &opt).unwrap();
        let size = tree.size().to_int_size();
        let mut pixmap = tiny_skia::Pixmap::new(size.width(), size.height()).unwrap();
        resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());
        pixmap.save_png(std::env::var("HUBDECK_OUT").unwrap_or_else(|_| "mochi-gestures.png".into())).unwrap();
    }

    /// One gesture cycle per mood as PNG frames, 15 a second, for an animated preview:
    /// `HUBDECK_OUT=<dir> cargo test -p hubdeck mochi_gesture_frames -- --ignored`.
    #[test]
    #[ignore]
    fn mochi_gesture_frames() {
        use resvg::{tiny_skia, usvg};
        let dir = std::path::PathBuf::from(std::env::var("HUBDECK_OUT").unwrap_or_else(|_| "mochi-frames".into()));
        std::fs::create_dir_all(&dir).unwrap();
        let now = Instant::now();
        let pet = Pet { needs: Needs::default(), moment: None, look: 0.0, file: None };
        let mut opt = usvg::Options::default();
        opt.fontdb_mut().load_system_fonts();
        for (name, mood) in [("sad", Mood::Sad), ("hungry", Mood::Hungry), ("sleepy", Mood::Sleepy)] {
            for f in 0..32 {
                let svg = dial_svg(&pet, mood, f as f32 * (LOOP / 8.0) / 32.0, now, 0);
                let tree = usvg::Tree::from_str(&svg, &opt).unwrap();
                let mut pixmap = tiny_skia::Pixmap::new(240, 240).unwrap();
                resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());
                pixmap.save_png(dir.join(format!("{name}-{f:02}.png"))).unwrap();
            }
        }
    }

    /// Every mood and moment in one picture: `HUBDECK_OUT=<png> cargo test -p hubdeck mochi_sheet -- --ignored`.
    #[test]
    #[ignore]
    fn mochi_sheet() {
        use resvg::{tiny_skia, usvg};
        let base = Instant::now();
        let mut out = String::from(r##"<svg xmlns="http://www.w3.org/2000/svg" width="1300" height="540"><rect width="100%" height="100%" fill="#101016"/>"##);
        let cases: [(Mood, Option<Moment>, f32); 10] = [
            (Mood::Happy, None, 0.0),
            (Mood::Hungry, None, 0.0),
            (Mood::Sad, None, 0.0),
            (Mood::Sleepy, None, 0.0),
            (Mood::Asleep, None, 0.0),
            (Mood::Happy, Some(Moment::Hearts), 0.6),
            (Mood::Happy, Some(Moment::Munch), 0.9),
            (Mood::Happy, Some(Moment::Hop(1)), 0.2),
            (Mood::Sleepy, Some(Moment::Yawn), 0.7),
            (Mood::Happy, Some(Moment::Hop(-1)), 0.6),
        ];
        for (i, (mood, moment, age)) in cases.iter().enumerate() {
            let now = base + Duration::from_secs(5);
            let pet = Pet {
                needs: Needs::default(),
                moment: moment.map(|m| (m, now - Duration::from_secs_f32(*age))),
                look: if i == 9 {
                    -1.0
                } else if i == 7 {
                    1.0
                } else {
                    0.0
                },
                file: None,
            };
            let svg = dial_svg(&pet, *mood, 1.3, now, 0);
            let (x, y) = (20 + (i % 5) * 255, 20 + (i / 5) * 260);
            out += &format!(
                r#"<svg x="{x}" y="{y}" width="240" height="240">{}</svg>"#,
                svg.trim_start_matches(r#"<svg xmlns="http://www.w3.org/2000/svg" width="240" height="240" viewBox="0 0 240 240">"#).trim_end_matches("</svg>")
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

    /// Mochi and the same moments with body pictures of the user's own (owner, 2026-10-18: only
    /// the body changes, the face and reactions stay):
    /// `HUBDECK_OUT=<png> HUBDECK_PICTURES=<file>,<file> cargo test -p hubdeck pet_body_sheet -- --ignored`
    /// (one row of Mochi, then one row a picture)
    #[test]
    #[ignore]
    fn pet_body_sheet() {
        super::super::images::init(&std::env::temp_dir().join(format!("hubdeck-pet-sheet-{}", std::process::id())));
        let mut rows: Vec<Option<String>> = vec![None];
        for file in std::env::var("HUBDECK_PICTURES").expect("HUBDECK_PICTURES").split(',') {
            let ext = file.rsplit('.').next().unwrap_or("png").to_lowercase();
            rows.push(Some(super::super::images::import_bytes(&std::fs::read(file).unwrap(), &ext).unwrap()));
        }
        let now = Instant::now();
        let at = |m: Moment, age: f32| Pet { needs: Needs::default(), moment: Some((m, now - Duration::from_secs_f32(age))), look: 0.0, file: None };
        let calm = Pet { needs: Needs::default(), moment: None, look: 0.0, file: None };
        // happy, a press (hearts), a turn (hop), a feed (cookie), hungry, sad, late (sleepy), asleep
        let shots: Vec<(Pet, Mood)> = vec![
            (calm.clone(), Mood::Happy),
            (at(Moment::Hearts, 0.5), Mood::Happy),
            (at(Moment::Hop(1), 0.2), Mood::Happy),
            (at(Moment::Munch, 0.6), Mood::Happy),
            (calm.clone(), Mood::Hungry),
            (calm.clone(), Mood::Sad),
            (calm.clone(), Mood::Sleepy),
            (calm.clone(), Mood::Asleep),
        ];
        let mut out = format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="2040" height="{}"><rect width="100%" height="100%" fill="#16141c"/>"##, 15 + rows.len() * 255);
        for (row, body) in rows.iter().enumerate() {
            for (col, (pet, mood)) in shots.iter().enumerate() {
                let fit = body.as_ref().map(|name| Body { name: name.clone(), size: 1.0, x: 0.0, y: 0.0 });
                let svg = dial_svg_with(pet, *mood, 0.3, now, 0, fit.as_ref());
                let inner = svg.trim_start_matches(r#"<svg xmlns="http://www.w3.org/2000/svg" width="240" height="240" viewBox="0 0 240 240">"#).trim_end_matches("</svg>");
                // every dial's ids are its own: the clip path of one must not cut the next
                let inner = inner.replace("pbody", &format!("pbody{row}x{col}"));
                out += &format!(r#"<svg x="{}" y="{}" width="240" height="240">{inner}</svg>"#, 15 + col * 253, 15 + row * 255);
            }
        }
        out += "</svg>";
        let r = super::super::render::Renderer::default();
        std::fs::write(std::env::var("HUBDECK_OUT").expect("HUBDECK_OUT"), r.png(&out).unwrap()).unwrap();
    }

    /// A body picture of the user's own: only the body changes (owner, 2026-10-18).
    #[test]
    fn a_body_picture_keeps_the_face() {
        // the pictures folder of this test run (another test may have set it first)
        super::super::images::init(&std::env::temp_dir().join(format!("hubdeck-pet-body-{}", std::process::id())));
        let store = |img: image::RgbaImage| {
            let mut png = Vec::new();
            img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
            super::super::images::import_bytes(&png, "png").unwrap()
        };
        let red = store(image::RgbaImage::from_pixel(400, 400, image::Rgba([230, 20, 20, 255])));
        // a cut-out: a red square on a see-through background
        let cut = store(image::RgbaImage::from_fn(300, 300, |x, y| if (60..240).contains(&x) && (60..240).contains(&y) { image::Rgba([230, 20, 20, 255]) } else { image::Rgba([0, 0, 0, 0]) }));
        let pet = Pet { needs: Needs::default(), moment: None, look: 0.0, file: None };
        let r = super::super::render::Renderer::default();
        let reds = |svg: &str| image::load_from_memory(&r.jpeg(svg).unwrap()).unwrap().to_rgb8().pixels().filter(|p| p[0] > 180 && p[1] < 80 && p[2] < 80).count();
        let now = Instant::now();
        let mochi = dial_svg(&pet, Mood::Happy, 0.3, now, 0);
        for (name, cut_out) in [(&red, false), (&cut, true)] {
            let svg = dial_svg_with(&pet, Mood::Happy, 0.3, now, 0, body_of(&serde_json::json!({ "body": name })).as_ref());
            assert!(svg.contains("<image") && !svg.contains("url(#pbd)"), "the picture is the body");
            assert!(!svg.contains("clip-path"), "its own shape, never Mochi's (cut-out: {cut_out})");
            assert!(svg.contains("url(#pey)") && svg.contains("url(#pblu)"), "its eyes and blush stay");
            assert!(reds(&svg) > 5000, "the picture shows: {}", reds(&svg));
            // the face is drawn over the picture: the same eyes and mouth as Mochi's
            let face = |s: &str| s[s.find("url(#pey)").unwrap()..].to_string();
            assert_eq!(face(&svg)[..400], face(&mochi)[..400]);
        }
        // the moods and reactions stay with a picture
        let red_body = body_of(&serde_json::json!({ "body": red })).unwrap();
        let asleep = dial_svg_with(&pet, Mood::Asleep, 0.3, now, 0, Some(&red_body));
        assert!(asleep.contains("#2f3a8f"), "the nightcap");
        let fed = Pet { moment: Some((Moment::Munch, now)), ..pet.clone() };
        assert!(dial_svg_with(&fed, Mood::Happy, 0.3, now, 0, Some(&red_body)).contains("pbite"), "the cookie");
        // set on the key that brings Mochi to the dial (the Fun page's Mochi key): its body there
        let key: super::super::profile::Binding = serde_json::from_value(serde_json::json!({"action": ACTION, "params": {"press": "dial", "body": red}})).unwrap();
        let dial: super::super::profile::Binding = serde_json::from_value(serde_json::json!({"action": ACTION, "params": {}})).unwrap();
        let controls = std::collections::BTreeMap::from([("dial".to_string(), dial), ("fun/keys.10".to_string(), key)]);
        assert_eq!(body_on(&controls, None).map(|b| b.name), Some(red.clone()));
        // two Mochi keys with their own pictures: the dial shows the one of the key that put it there
        let blue = store(image::RgbaImage::from_pixel(50, 50, image::Rgba([20, 40, 230, 255])));
        let mut controls = controls;
        controls.insert("keys.10".into(), serde_json::from_value(serde_json::json!({"action": ACTION, "params": {"press": "dial", "body": blue}})).unwrap());
        assert_eq!(body_on(&controls, Some("keys.10")).map(|b| b.name), Some(blue));
        assert_eq!(body_on(&controls, Some("fun/keys.10")).map(|b| b.name), Some(red.clone()));
        // a picture that is gone: Mochi's own body
        assert!(dial_svg_with(&pet, Mood::Happy, 0.3, now, 0, body_of(&serde_json::json!({ "body": "gone.png" })).as_ref()).contains("url(#pbd)"));
        // its size and place: half as big, moved right and down; values out of range are held in
        let small = body_of(&serde_json::json!({ "body": red, "bodyFit": { "size": 50, "x": 20, "y": 10 } })).unwrap();
        assert_eq!((small.size, small.x, small.y), (0.5, 20.0, 10.0));
        assert!(dial_svg_with(&pet, Mood::Happy, 0.3, now, 0, Some(&small)).contains(r#"x="102.0" y="136.0" width="76.0" height="70.0""#));
        assert!(reds(&dial_svg_with(&pet, Mood::Happy, 0.3, now, 0, Some(&small))) < reds(&dial_svg_with(&pet, Mood::Happy, 0.3, now, 0, Some(&red_body))));
        assert_eq!(body_of(&serde_json::json!({ "body": red, "bodyFit": { "size": 9000 } })).unwrap().size, 2.0);
        assert_eq!(body_of(&serde_json::json!({ "body": " " })), None);
    }

    /// How long a dial frame of Mochi takes to draw and encode (release: `--release`).
    #[test]
    #[ignore]
    fn mochi_frame_time() {
        let r = super::super::render::Renderer::default();
        let base = Instant::now();
        // where a frame's time goes: the drawing alone (no blur), the whole (draw + JPEG)
        let pet = Pet { needs: Needs::default(), moment: None, look: 0.0, file: None };
        let svg = dial_svg(&pet, Mood::Happy, 0.3, base, 0);
        let plain = svg.replace(r#" filter="url(#pbl)""#, "").replace(r#" filter="url(#pbl1)""#, "");
        for (label, svg) in [("with blur", &svg), ("without blur", &plain)] {
            let started = Instant::now();
            for _ in 0..30 {
                r.jpeg(svg).unwrap();
            }
            println!("{label}: {:?} a frame", started.elapsed() / 30);
        }
        for (label, moment) in [("rest", None), ("hearts", Some(Moment::Hearts)), ("munch", Some(Moment::Munch))] {
            let pet = Pet { needs: Needs::default(), moment: moment.map(|m| (m, base)), look: 0.0, file: None };
            let started = Instant::now();
            for f in 0..30 {
                let svg = dial_svg(&pet, Mood::Happy, f as f32 * 0.06, base + Duration::from_millis(f * 60), 0);
                r.jpeg(&svg).unwrap();
            }
            println!("{label}: {:?} a frame", started.elapsed() / 30);
        }
        // a body picture of the user's own: a photo-like one, 1200 px, made once to 160 px
        super::super::images::init(&std::env::temp_dir().join(format!("hubdeck-pet-time-{}", std::process::id())));
        let mut png = Vec::new();
        image::RgbImage::from_fn(1200, 1200, |x, y| image::Rgb([(x % 256) as u8, (y % 256) as u8, ((x * y) % 256) as u8]))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let body = super::super::images::import_bytes(&png, "png").unwrap();
        let pet = Pet { needs: Needs::default(), moment: Some((Moment::Hearts, base)), look: 0.0, file: None };
        let started = Instant::now();
        for f in 0..30 {
            let svg = dial_svg_with(&pet, Mood::Happy, f as f32 * 0.06, base + Duration::from_millis(f * 60), 0, body_of(&serde_json::json!({ "body": body })).as_ref());
            r.jpeg(&svg).unwrap();
        }
        println!("hearts with a body picture: {:?} a frame", started.elapsed() / 30);
    }

    /// What a second of Mochi at rest costs to draw: before (12 frames, each drawn) and now (25,
    /// drawn once a loop, then taken by the hash of the picture): `--release -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn mochi_rest_cost() {
        use std::hash::{Hash, Hasher};
        let r = super::super::render::Renderer::default();
        let pet = Pet { needs: Needs::default(), moment: None, look: 0.0, file: None };
        let now = Instant::now();
        let started = Instant::now();
        for f in 0..120 {
            r.jpeg(&dial_svg(&pet, Mood::Happy, f as f32 / 12.0, now, 0)).unwrap();
        }
        println!("before, 12 a second, drawn: {:?} of work a second", started.elapsed() / 10);
        let mut kept: std::collections::HashMap<u64, Vec<u8>> = std::collections::HashMap::new();
        let mut round = |frames: std::ops::Range<u64>| {
            let started = Instant::now();
            for f in frames.clone() {
                let svg = dial_svg(&pet, Mood::Happy, loop_at(Duration::from_millis(f * STEP_MS)), now, 0);
                let mut h = std::collections::hash_map::DefaultHasher::new();
                svg.hash(&mut h);
                let key = h.finish();
                kept.entry(key).or_insert_with(|| r.jpeg(&svg).unwrap());
            }
            started.elapsed() * 25 / (frames.end - frames.start) as u32
        };
        println!("now, 25 a second, first loop: {:?} of work a second", round(0..LOOP_FRAMES));
        println!("now, 25 a second, after it: {:?} of work a second", round(LOOP_FRAMES..3 * LOOP_FRAMES));
        println!("kept: {} frames, {} KB", kept.len(), kept.values().map(Vec::len).sum::<usize>() / 1024);
    }

    #[test]
    fn taken_off_the_dial_it_keeps_its_name_and_body() {
        // Discussions #3: hidden and shown again by a key, the name and body set on the dial reset
        let b = |v: serde_json::Value| serde_json::from_value::<super::super::profile::Binding>(v).unwrap();
        let controls = std::collections::BTreeMap::from([
            ("dial".to_string(), b(serde_json::json!({"action": ACTION, "params": {"name": "Tama", "body": "cat.png", "bodyFit": {"size": 80}, "from": "keys.3"}}))),
            ("dial:hold".to_string(), b(serde_json::json!({"action": ACTION, "params": {"name": "Tama", "press": "sleep"}}))),
            ("dial:double".to_string(), b(serde_json::json!({"action": "core.media", "params": {}}))),
            ("keys.3".to_string(), b(serde_json::json!({"action": ACTION, "params": {"press": "dial"}}))),
        ]);
        let kept = kept_bindings(&controls, &["dial", "dial:double", "dial:hold"]).unwrap();
        assert_eq!(kept["dial"]["params"], serde_json::json!({"name": "Tama", "body": "cat.png", "bodyFit": {"size": 80}}), "without the key that brought it");
        assert_eq!(kept["dial:hold"]["params"]["name"], "Tama");
        assert!(kept.get("dial:double").is_none(), "only Mochi's own");
        // what is kept goes through pet.json and back
        let needs = Needs { mochi: Some(kept.clone()), ..Needs::default() };
        let back: Needs = serde_json::from_str(&serde_json::to_string(&needs).unwrap()).unwrap();
        assert_eq!(back.mochi, Some(kept));
        assert!(kept_bindings(&std::collections::BTreeMap::new(), &["dial"]).is_none());
    }

    #[test]
    fn its_rest_is_a_loop_drawn_once() {
        let pet = Pet { needs: Needs::default(), moment: None, look: 0.0, file: None };
        let now = Instant::now();
        let at = |ms: u64| dial_svg(&pet, Mood::Happy, loop_at(Duration::from_millis(ms)), now, 0);
        assert_eq!(at(3_000), at(3_000 + 16_800), "the same frame a loop later");
        assert_eq!(at(3_000), at(9 * 24 * 3600 * 1000 + 3_000 - 9 * 24 * 3600 * 1000 % 16_800), "and after days");
        let frames: std::collections::HashSet<String> = (0..2 * LOOP_FRAMES).map(|f| at(f * STEP_MS)).collect();
        assert!(frames.len() <= LOOP_FRAMES as usize && frames.len() > 300, "{} frames", frames.len());
    }

    #[test]
    fn its_mood_follows_needs_the_day_and_the_lock() {
        let mut p = Pet { needs: Needs { fullness: 0.9, joy: 0.9, asleep: false, at: now_secs(), ..Needs::default() }, moment: None, look: 0.0, file: None };
        assert_eq!(p.mood(false, 14), Mood::Happy);
        assert_eq!(p.mood(true, 14), Mood::Asleep, "the computer is locked");
        assert_eq!(p.mood(false, 2), Mood::Sleepy);
        p.needs.fullness = 0.1;
        assert_eq!(p.mood(false, 14), Mood::Hungry);
        p.apply(PetCmd::Feed, Instant::now());
        assert_eq!(p.mood(false, 14), Mood::Happy);
        p.apply(PetCmd::Sleep, Instant::now());
        assert_eq!(p.mood(false, 14), Mood::Asleep);
        p.apply(PetCmd::Pet, Instant::now());
        assert!(!p.needs.asleep, "any touch wakes it");
        let svg = dial_svg(&p, Mood::Asleep, 1.0, Instant::now(), 0);
        assert!(svg.starts_with("<svg") && !svg.contains("Mochi"), "no text over it");
        // every mood and moment is a picture the renderer reads (a doubled attribute once broke one)
        let r = super::super::render::Renderer::default();
        for mood in [Mood::Happy, Mood::Hungry, Mood::Sad, Mood::Sleepy, Mood::Asleep] {
            for moment in [None, Some(Moment::Hearts), Some(Moment::Munch), Some(Moment::Hop(1)), Some(Moment::Yawn)] {
                let now = Instant::now() + Duration::from_secs(3);
                let pet = Pet { moment: moment.map(|m| (m, now - Duration::from_millis(500))), ..p.clone() };
                r.jpeg(&dial_svg(&pet, mood, 1.0, now, 0)).unwrap_or_else(|e| panic!("{mood:?} {moment:?}: {e}"));
            }
        }
    }
}
