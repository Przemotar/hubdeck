//! The music's beat for the lights (owner, 2026-10-19: "żeby deck podświetlał się w rytm muzyki,
//! która leci na spotify"). Spotify gives no beat to apps like HubDeck's (its audio analysis is
//! closed to them since 2024-11), so the sound itself is listened to: what goes to the speakers,
//! through Windows' loopback capture (WASAPI, the documented way audio visualizers in lighting
//! apps do it). Listening only while a light uses the effect, on a thread of its own; nothing is
//! recorded or sent anywhere, only a loudness and the time of the last beat are kept.
#![cfg_attr(not(windows), allow(dead_code))]

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// A light wants the beat now.
static WANT: AtomicBool = AtomicBool::new(false);
/// The listening thread runs.
static RUNNING: AtomicBool = AtomicBool::new(false);
/// The loudness now, 0..1, and against its average over the last seconds (1 = as usual; f32 bits).
static LOUD: AtomicU32 = AtomicU32::new(0);
static REL: AtomicU32 = AtomicU32::new(0);
/// When the last beat and the last sound came, in ms since `start()`.
static BEAT_AT: AtomicU64 = AtomicU64::new(0);
static HEARD_AT: AtomicU64 = AtomicU64::new(0);
/// The last other hit (a snare, a clap: a jump in the mids or highs) and how strong, 0..1.
static HIT_AT: AtomicU64 = AtomicU64::new(0);
static HIT_STRENGTH: AtomicU32 = AtomicU32::new(0);
/// The tempo found in the music: the period in ms (0 = none sure enough) and a beat on its grid.
static PERIOD_MS: AtomicU32 = AtomicU32::new(0);
static ANCHOR_AT: AtomicU64 = AtomicU64::new(0);

/// How long before a predicted beat its flash starts by default: about the way to the deck (the
/// engine's 20 ms step and an 8 ms LED report, 2026-10-19); the user sets it with the metronome.
#[cfg(test)]
const LEAD_MS: u64 = 20;

/// While the metronome test plays (Settings > Lights): the lights follow it even though no player
/// plays, until this time (ms since `start()`).
static TEST_UNTIL: AtomicU64 = AtomicU64::new(0);

/// The metronome test plays now.
pub fn testing(now: Instant) -> bool {
    ms(now) < TEST_UNTIL.load(Ordering::Relaxed)
}

/// How the lights follow the music (Settings > Lights > Lights to the music).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tuning {
    /// How dark between beats and from how loud a part they light up, 0..1.
    pub dim: f32,
    pub from: f32,
    /// Flash on the predicted beat (else only on what is heard).
    pub predict: bool,
    /// How early a predicted beat flashes, ms.
    pub lead_ms: u64,
    /// Snares and other hits flash whiter than the kick.
    pub white_hits: bool,
}

/// The light at a moment: how bright (0..1) and how much white is mixed into its color (0..1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Glow {
    pub level: f32,
    pub white: f32,
}

fn start() -> Instant {
    static AT: OnceLock<Instant> = OnceLock::new();
    *AT.get_or_init(Instant::now)
}

fn ms(at: Instant) -> u64 {
    at.saturating_duration_since(start()).as_millis() as u64 + 1
}

/// Listen while `yes` (a light on the deck uses the effect); the thread stops by itself after.
pub fn want(yes: bool) {
    WANT.store(yes, Ordering::Relaxed);
    if yes && !RUNNING.swap(true, Ordering::AcqRel) {
        let _ = start();
        #[cfg(windows)]
        let spawned = std::thread::Builder::new().name("beat".into()).spawn(win::listen);
        #[cfg(not(windows))]
        let spawned: std::io::Result<()> = {
            RUNNING.store(false, Ordering::Release);
            Ok(())
        };
        if spawned.is_err() {
            RUNNING.store(false, Ordering::Release);
        }
    }
}

/// How bright a light that follows the music is now, 0..1: a flash at each beat that fades in a
/// fifth of a second, a smaller one at other hits, over a glow that follows the loudness; None
/// while nothing is heard (the light then shows its color as it is). `predict`: when the kicks
/// keep a sure tempo, the flash comes on the beat it predicts, [`LEAD_MS`] early so it reaches the
/// deck on time; `false` only reacts to what was heard (the first way, kept to compare; owner,
/// 2026-10-19: "z mozliwoscia przelaczenia na stary dla testu").
pub fn glow(now: Instant, t: &Tuning) -> Option<Glow> {
    let heard = HEARD_AT.load(Ordering::Relaxed);
    let now_ms = ms(now);
    if heard == 0 || now_ms.saturating_sub(heard) > 600 {
        return None;
    }
    let rel = f32::from_bits(REL.load(Ordering::Relaxed));
    let mut since_beat = now_ms.saturating_sub(BEAT_AT.load(Ordering::Relaxed));
    let period = PERIOD_MS.load(Ordering::Relaxed) as u64;
    if t.predict && period > 0 {
        since_beat = since_beat.min(since_predicted(now_ms, period, ANCHOR_AT.load(Ordering::Relaxed), t.lead_ms));
    }
    // a flash fades in about a third of the time between beats (fast songs do not blur, slow ones
    // do not blink; owner's "1" of the polish, 2026-10-19), a fifth of a second without a tempo
    let fade = fade_ms(period);
    let kick = (-(since_beat as f32) / fade).exp();
    let hit = if t.predict { f32::from_bits(HIT_STRENGTH.load(Ordering::Relaxed)) * (-(now_ms.saturating_sub(HIT_AT.load(Ordering::Relaxed)) as f32) / (fade * 0.8)).exp() } else { 0.0 };
    let white = if t.white_hits && hit > kick { (hit - kick) * 0.9 } else { 0.0 };
    Some(Glow { level: envelope(rel, kick.max(hit), t.dim, t.from), white: white.clamp(0.0, 1.0) })
}

/// How long a flash takes to fade (to a third) for a tempo's period (0: none known).
fn fade_ms(period: u64) -> f32 {
    if period == 0 { 180.0 } else { (period as f32 * 0.3).clamp(90.0, 240.0) }
}

/// The metronome test: `wav` (made by [`metronome_wav`]) plays now for `seconds`, and the lights
/// follow it (no player needs to play).
pub fn start_test(seconds: u64) {
    TEST_UNTIL.store(ms(Instant::now()) + seconds * 1000, Ordering::Relaxed);
}

/// A metronome, 120 beats a minute for `seconds`: a kick (a 55 Hz thump that fades) with a click
/// on each beat, as a 16-bit mono WAV at 48 kHz.
pub fn metronome_wav(seconds: u32) -> Vec<u8> {
    let rate = 48_000u32;
    let samples: Vec<i16> = (0..rate * seconds)
        .map(|i| {
            let into = (i % (rate / 2)) as f32 / rate as f32;
            let thump = (2.0 * std::f32::consts::PI * 55.0 * into).sin() * (-into / 0.06).exp();
            let click = if into < 0.004 { (2.0 * std::f32::consts::PI * 2500.0 * into).sin() * (1.0 - into / 0.004) * 0.4 } else { 0.0 };
            ((thump * 0.7 + click) * 32_000.0) as i16
        })
        .collect();
    let data = samples.len() as u32 * 2;
    let mut out = Vec::with_capacity(44 + data as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

/// The ms since the last predicted beat's flash: beats every `period` ms from `anchor`, each
/// flash `lead` ms before its beat.
fn since_predicted(now_ms: u64, period: u64, anchor: u64, lead: u64) -> u64 {
    let ahead = now_ms + lead;
    if period == 0 || ahead < anchor {
        return u64::MAX;
    }
    (ahead - anchor) % period
}

/// Ready-made tunings of (dim, from), 0..=100 each (owner, 2026-10-19: "jakies kilka presetow dac
/// i ustawic najlepszy"); `beat` is the default.
pub const PRESETS: [(&str, u8, u8); 4] = [("soft", 20, 10), ("beat", 60, 45), ("punchy", 85, 60), ("beats", 100, 100)];

/// The brightness for the loudness against its usual level (`rel`, 1 = as loud as the last
/// seconds), the ms since the last beat, how dark it gets between beats (`dim` 0..1; owner:
/// "moglyby bardziej przygasac") and from how loud a part it lights up at all (`from` 0..1;
/// owner: "muzyka jest praktycznie caly czas wiec ten poziom bazowy mozna by bylo ustawiac od
/// jakiegos poziomu"): 0 follows every sound, 1 only parts well above the usual.
fn envelope(rel: f32, flash: f32, dim: f32, from: f32) -> f32 {
    let (dim, from) = (dim.clamp(0.0, 1.0), from.clamp(0.0, 1.0));
    let threshold = 0.5 + 0.9 * from;
    let level = ((rel - threshold) / 0.6).clamp(0.0, 1.0);
    let floor = 0.36 * (1.0 - dim);
    let follow = 0.8 * (1.0 - dim) + 0.1;
    (floor + follow * level).max(flash).clamp(0.0, 1.0)
}

/// What one block of sound held.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Heard {
    /// A kick: the bass jumped well above its average.
    pub beat: bool,
    /// Another hit (a snare, a clap, a hi-hat): how strong, 0..1.
    pub hit: Option<f32>,
}

/// Beats from the sound: the energy of the bass (a low-pass filter around 150 Hz) against its
/// average over about the last second; a beat when it jumps well above it, at most ~3.5 a second.
/// Other hits from a sudden rise in the mids (300-2000 Hz) and highs (above 4 kHz), against how
/// such rises usually go ("spectral flux" over three bands). The tempo comes from how these rises
/// repeat over the last 6 s (autocorrelation, as beat trackers like BTrack do), which missed or
/// extra kicks do not throw off (2026-10-19: real music gave kick gaps of 472, 929, 290, 871 ms).
pub struct Detector {
    low: f32,
    a: f32,
    /// One-pole low-passes at 300, 2000 and 4000 Hz: mids = 2000 - 300, highs = all - 4000.
    lp: [f32; 3],
    lpa: [f32; 3],
    /// The last block's energy of the mids and highs (log), and how their rises usually go.
    last_log: [f32; 2],
    flux_mean: f32,
    flux_var: f32,
    last_hit: Option<Instant>,
    average: f32,
    loud: f32,
    /// The loudness over the last seconds.
    usual: f32,
    last_beat: Option<Instant>,
    blocks: u64,
    /// How strongly each block rose (bass, mids, highs; one per block) over the last 6 s, the
    /// bass's last log energy, and the tempo found in them.
    onsets: std::collections::VecDeque<(f32, Instant)>,
    bass_log: f32,
    /// The spectrum's rise for the tempo: the last FFT_SIZE samples, the FFT, the last block's
    /// log magnitudes (the standard "spectral flux" onset strength, as librosa computes it).
    window: std::collections::VecDeque<f32>,
    fft: std::sync::Arc<dyn rustfft::Fft<f32>>,
    last_mags: Vec<f32>,
    tempo: Option<Tempo>,
}

/// A tempo the music keeps: the time between beats and a beat on that grid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tempo {
    pub period_ms: f32,
    pub anchor: Instant,
    /// How alike the music is one period apart: the correlation, 0 (noise) to 1 (a clean beat).
    pub sure: f32,
    /// How far that period stands out from the other periods (in their spread).
    pub stands_out: f32,
}

impl Detector {
    pub fn new(rate: u32) -> Self {
        let coef = |hz: f32| 1.0 - (-2.0 * std::f32::consts::PI * hz / rate.max(1) as f32).exp();
        Self {
            low: 0.0,
            a: coef(150.0),
            lp: [0.0; 3],
            lpa: [coef(300.0), coef(2000.0), coef(4000.0)],
            last_log: [f32::MIN; 2],
            flux_mean: 0.0,
            flux_var: 0.0,
            last_hit: None,
            average: 0.0,
            loud: 0.0,
            usual: 0.0,
            last_beat: None,
            blocks: 0,
            onsets: std::collections::VecDeque::new(),
            bass_log: f32::MIN,
            window: std::collections::VecDeque::from(vec![0.0; FFT_SIZE]),
            fft: rustfft::FftPlanner::new().plan_fft_forward(FFT_SIZE),
            last_mags: Vec::new(),
            tempo: None,
        }
    }

    /// The tempo, when the kicks keep one surely.
    pub fn tempo(&self) -> Option<Tempo> {
        self.tempo
    }

    /// One block of mono samples (about 10 ms) heard at `at`.
    pub fn feed(&mut self, samples: &[f32], at: Instant) -> Heard {
        if samples.is_empty() {
            return Heard::default();
        }
        let (mut bass, mut all, mut mids, mut highs) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
        for &x in samples {
            self.low += self.a * (x - self.low);
            for (lp, a) in self.lp.iter_mut().zip(self.lpa) {
                *lp += a * (x - *lp);
            }
            let (mid, high) = (self.lp[1] - self.lp[0], x - self.lp[2]);
            bass += self.low * self.low;
            all += x * x;
            mids += mid * mid;
            highs += high * high;
        }
        let n = samples.len() as f32;
        let (bass, rms) = (bass / n, (all / n).sqrt());
        let (hit, flux) = self.other_hit([mids / n, highs / n], at);
        // how strongly this block rose, for the tempo: the spectrum's rise
        let _ = flux;
        self.bass_log = (bass / n + 1e-9).ln();
        let rise = self.spectral_flux(samples);
        self.onsets.push_back((rise, at));
        while self.onsets.len() > ONSET_BLOCKS {
            self.onsets.pop_front();
        }
        self.loud = self.loud * 0.8 + rms * 0.2;
        // the first block sets the average: sound that starts loud is not one long beat
        if self.blocks == 0 {
            self.average = bass;
            self.usual = rms;
        }
        self.usual = self.usual * 0.995 + rms * 0.005;
        self.blocks += 1;
        let beat = bass > 1e-5 && bass > self.average * 1.6 && self.last_beat.is_none_or(|b| at.saturating_duration_since(b) >= Duration::from_millis(280));
        self.average = self.average * 0.97 + bass * 0.03;
        if beat {
            self.last_beat = Some(at);
        }
        // the tempo again every half second; it counts when three in a row agree (real music is
        // only weakly alike a beat apart, 0.1-0.2, but steadily so; noise jumps between periods)
        if self.blocks.is_multiple_of(50) {
            let raw = tempo_raw(&self.onsets, self.tempo.map(|t| t.period_ms));
            // a tempo held goes on through weaker moments while its period stays (hysteresis)
            let same = |t: &Tempo| self.tempo.is_some_and(|held| (held.period_ms - t.period_ms).abs() <= held.period_ms * 0.03);
            self.tempo = raw.filter(|t| sure_enough(t) || (same(t) && t.sure >= HOLD_SURE && t.stands_out >= HOLD_STANDS_OUT));
        }
        // a hit right with the kick is the kick
        let hit = hit.filter(|_| !beat);
        Heard { beat, hit }
    }

    /// The rise of the spectrum since the last block: the last FFT_SIZE samples through a Hann
    /// window and an FFT, log magnitudes, the sum of the bins that grew, per bin.
    fn spectral_flux(&mut self, samples: &[f32]) -> f32 {
        for &x in samples {
            self.window.pop_front();
            self.window.push_back(x);
        }
        let mut buf: Vec<rustfft::num_complex::Complex<f32>> = self
            .window
            .iter()
            .enumerate()
            .map(|(i, x)| {
                let hann = 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / (FFT_SIZE - 1) as f32).cos();
                rustfft::num_complex::Complex { re: x * hann, im: 0.0 }
            })
            .collect();
        self.fft.process(&mut buf);
        // bins from ~47 Hz to ~16 kHz at 48 kHz
        let mags: Vec<f32> = buf[1..FFT_SIZE / 3].iter().map(|c| (1.0 + 100.0 * c.norm()).ln()).collect();
        let flux = if self.last_mags.len() == mags.len() { mags.iter().zip(&self.last_mags).map(|(m, p)| (m - p).max(0.0)).sum::<f32>() / mags.len() as f32 } else { 0.0 };
        self.last_mags = mags;
        flux
    }

    /// A sudden rise in the mids or highs: the rises' sum (in log energy) against how they usually
    /// go; at most one hit per 100 ms.
    fn other_hit(&mut self, energy: [f32; 2], at: Instant) -> (Option<f32>, f32) {
        let logs = energy.map(|e| (e + 1e-9).ln());
        let flux: f32 = if self.last_log[0] == f32::MIN { 0.0 } else { logs.iter().zip(self.last_log).map(|(l, p)| (l - p).max(0.0)).sum() };
        self.last_log = logs;
        let quiet = energy.iter().all(|e| *e < 1e-7);
        let deviation = self.flux_var.sqrt();
        let jump = flux - self.flux_mean;
        self.flux_mean = self.flux_mean * 0.98 + flux * 0.02;
        self.flux_var = self.flux_var * 0.98 + jump * jump * 0.02;
        let ready = self.blocks > 50 && self.last_hit.is_none_or(|h| at.saturating_duration_since(h) >= Duration::from_millis(100));
        if quiet || !ready || flux < 0.8 || jump < 3.0 * deviation {
            return (None, if quiet { 0.0 } else { flux });
        }
        self.last_hit = Some(at);
        (Some((jump / (6.0 * deviation.max(0.05))).clamp(0.3, 0.7)), flux)
    }

    pub fn loudness(&self) -> f32 {
        self.loud
    }

    /// The loudness against the last seconds' (1 = as usual).
    pub fn relative(&self) -> f32 {
        if self.usual > 1e-4 { self.loud / self.usual } else { 0.0 }
    }
}

/// The FFT for the spectrum's rise: 1024 samples, about 21 ms at 48 kHz.
const FFT_SIZE: usize = 1024;

/// How many blocks of rises the tempo is found in (about 6 s of 10 ms blocks).
const ONSET_BLOCKS: usize = 600;

/// The tempo in the rises of the last seconds (one value per ~10 ms block, the newest last, with
/// its time): the lag between 300 and 750 ms (80 to 200 beats a minute) at which the rises repeat
/// most, leaning towards 120 a minute, and the beat on that grid closest to now; None in silence,
/// with under 4 s heard, or when no period stands out from the rest.
#[cfg(test)]
fn tempo_of(onsets: &std::collections::VecDeque<(f32, Instant)>) -> Option<Tempo> {
    tempo_raw(onsets, None).filter(sure_enough)
}

/// How alike the rises must be a period apart, and how far that period must stand out from the
/// others, for a tempo to count; and for one held to go on. Measured 2026-10-19: noise 0.09-0.14
/// and 2.0-2.8, the owner's music 0.16-0.25 and 2.5-3.7.
const SURE: f32 = 0.16;
const STANDS_OUT: f32 = 3.0;
const HOLD_SURE: f32 = 0.12;
const HOLD_STANDS_OUT: f32 = 2.5;

fn sure_enough(t: &Tempo) -> bool {
    t.sure >= SURE && t.stands_out >= STANDS_OUT
}

/// [`tempo_of`] without the sureness it needs (for tuning).
fn tempo_raw(onsets: &std::collections::VecDeque<(f32, Instant)>, held: Option<f32>) -> Option<Tempo> {
    if onsets.len() < 400 {
        return None;
    }
    let o: Vec<f32> = onsets.iter().map(|(v, _)| *v).collect();
    let mean = o.iter().sum::<f32>() / o.len() as f32;
    // silence, or the music stopped in the last 1.5 s
    let recent = o[o.len() - 150..].iter().sum::<f32>() / 150.0;
    if mean < 1e-3 || recent < 1e-3 {
        return None;
    }
    // the block's length from the times (packets are about 10 ms but not exactly)
    let span = onsets.back()?.1.saturating_duration_since(onsets.front()?.1).as_secs_f32() * 1000.0;
    let block_ms = span / (o.len() - 1) as f32;
    let centred: Vec<f32> = o.iter().map(|v| v - mean).collect();
    let lags = (300.0 / block_ms).floor() as usize..=(750.0 / block_ms).ceil() as usize;
    let acf: Vec<(usize, f32)> = lags
        .map(|lag| {
            let sum: f32 = centred[lag..].iter().zip(&centred).map(|(a, b)| a * b).sum();
            (lag, sum / (o.len() - lag) as f32)
        })
        .collect();
    // a gentle lean towards 500 ms (120 a minute) against half and double tempo, and a stronger
    // one towards the tempo held: music keeps its tempo, so a period near it wins over a chance
    // peak elsewhere (as BTrack's tempo transitions do; owner's "3", 2026-10-19)
    let weight = |lag: usize| {
        let period = lag as f32 * block_ms;
        let octaves = (period / 500.0).log2();
        let near_held = held.map_or(0.0, |h| (-0.5 * ((period - h) / (0.04 * h)).powi(2)).exp());
        (-0.5 * (octaves / 0.6).powi(2)).exp() * (1.0 + 1.5 * near_held)
    };
    let (best_i, &(lag, peak)) = acf.iter().enumerate().max_by(|(_, a), (_, b)| (a.1 * weight(a.0)).total_cmp(&(b.1 * weight(b.0))))?;
    // how alike the rises are a period apart: the correlation, 0 for noise, 1 for a clean beat
    let variance = centred.iter().map(|v| v * v).sum::<f32>() / centred.len() as f32;
    let sure = if variance > 0.0 { peak / variance } else { 0.0 };
    // how far the peak stands out from the other lags, in their spread
    let values: Vec<f32> = acf.iter().map(|(_, v)| *v).collect();
    let acf_mean = values.iter().sum::<f32>() / values.len() as f32;
    let spread = (values.iter().map(|v| (v - acf_mean).powi(2)).sum::<f32>() / values.len() as f32).sqrt();
    let stands_out = if spread > 0.0 { (peak - acf_mean) / spread } else { 0.0 };
    // between blocks: the peak of a parabola through the best lag and its neighbours
    let shift = match (best_i.checked_sub(1).and_then(|i| acf.get(i)), acf.get(best_i + 1)) {
        (Some(l), Some(r)) => {
            let d = l.1 - 2.0 * peak + r.1;
            if d < 0.0 { (0.5 * (l.1 - r.1) / d).clamp(-0.5, 0.5) } else { 0.0 }
        }
        _ => 0.0,
    };
    let period_blocks = lag as f32 + shift;
    // the phase: the offset whose beats, going back from now, hold the most rise
    let n = o.len();
    let phase = (0..lag)
        .max_by(|a, b| {
            let at = |phase: usize| (0..n).map(|k| n as isize - 1 - phase as isize - (k as f32 * period_blocks).round() as isize).take_while(|i| *i >= 0).map(|i| o[i as usize]).sum::<f32>();
            at(*a).total_cmp(&at(*b))
        })
        .unwrap_or(0);
    let anchor = onsets[n - 1 - phase].1;
    Some(Tempo { period_ms: period_blocks * block_ms, anchor, sure, stands_out })
}

/// What the listening thread found in one block.
fn publish(d: &Detector, heard: Heard, at: Instant, sound: bool) {
    LOUD.store(d.loudness().to_bits(), Ordering::Relaxed);
    REL.store(d.relative().to_bits(), Ordering::Relaxed);
    if sound {
        HEARD_AT.store(ms(at), Ordering::Relaxed);
    }
    if heard.beat {
        BEAT_AT.store(ms(at), Ordering::Relaxed);
    }
    if let Some(strength) = heard.hit {
        HIT_AT.store(ms(at), Ordering::Relaxed);
        HIT_STRENGTH.store(strength.to_bits(), Ordering::Relaxed);
    }
    match d.tempo() {
        Some(t) => {
            ANCHOR_AT.store(ms(t.anchor), Ordering::Relaxed);
            PERIOD_MS.store(t.period_ms.round() as u32, Ordering::Relaxed);
        }
        None => PERIOD_MS.store(0, Ordering::Relaxed),
    }
}

#[cfg(windows)]
mod win {
    use super::*;
    use windows::Win32::Media::Audio::{
        AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK, IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator, MMDeviceEnumerator, WAVEFORMATEX, eConsole, eRender,
    };
    use windows::Win32::System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree};

    /// The listening thread: the default speakers' loopback until no light wants it; a device
    /// that goes away (headphones unplugged) is opened again a second later.
    pub fn listen() {
        // SAFETY: COM on this thread, multithreaded apartment.
        let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        while WANT.load(Ordering::Relaxed) {
            if let Err(e) = session() {
                log::info!("music beat: {e}");
                std::thread::sleep(Duration::from_secs(1));
            }
        }
        RUNNING.store(false, Ordering::Release);
        // a want that came while it was stopping
        if WANT.load(Ordering::Relaxed) {
            super::want(true);
        }
    }

    /// Tests: the listening into a detector of the caller's for `how_long`.
    #[cfg(test)]
    pub fn listen_into(d: std::sync::Arc<std::sync::Mutex<Detector>>, how_long: Duration) {
        // SAFETY: COM on this thread, multithreaded apartment.
        let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        let _ = session_with(Some((d, how_long)));
    }

    fn session() -> Result<(), String> {
        session_with(None)
    }

    type Probe = Option<(std::sync::Arc<std::sync::Mutex<Detector>>, Duration)>;

    fn session_with(probe: Probe) -> Result<(), String> {
        let err = |e: windows::core::Error| e.to_string();
        // SAFETY: COM calls with valid arguments; the mix format is freed with CoTaskMemFree,
        // and every buffer taken is released.
        unsafe {
            let devices: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(err)?;
            let device = devices.GetDefaultAudioEndpoint(eRender, eConsole).map_err(err)?;
            let client: IAudioClient = device.Activate(CLSCTX_ALL, None).map_err(err)?;
            let format = client.GetMixFormat().map_err(err)?;
            let f: WAVEFORMATEX = *format;
            let (rate, channels, bits) = (f.nSamplesPerSec, f.nChannels.max(1) as usize, f.wBitsPerSample);
            let init = client.Initialize(AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK, 2_000_000, 0, format, None);
            CoTaskMemFree(Some(format as *const _));
            init.map_err(err)?;
            if bits != 32 && bits != 16 {
                return Err(format!("{bits}-bit sound is not read"));
            }
            let capture: IAudioCaptureClient = client.GetService().map_err(err)?;
            client.Start().map_err(err)?;
            log::info!("music beat: listening ({rate} Hz, {channels} channels)");
            let mut detector = Detector::new(rate);
            let mut mono: Vec<f32> = Vec::with_capacity(2048);
            let until = probe.as_ref().map(|(_, d)| Instant::now() + *d);
            // what tempo it hears, in the log every 10 s while it listens
            let mut told = Instant::now();
            while WANT.load(Ordering::Relaxed) || until.is_some_and(|u| Instant::now() < u) {
                std::thread::sleep(Duration::from_millis(10));
                loop {
                    let frames_ready = capture.GetNextPacketSize().map_err(err)?;
                    if frames_ready == 0 {
                        break;
                    }
                    let (mut data, mut frames, mut flags) = (std::ptr::null_mut(), 0u32, 0u32);
                    capture.GetBuffer(&mut data, &mut frames, &mut flags, None, None).map_err(err)?;
                    mono.clear();
                    let silent = flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 || data.is_null();
                    let count = frames as usize * channels;
                    if silent {
                        mono.resize(frames as usize, 0.0);
                    } else if bits == 32 {
                        let s = std::slice::from_raw_parts(data as *const f32, count);
                        mono.extend(s.chunks(channels).map(|c| c.iter().sum::<f32>() / channels as f32));
                    } else {
                        let s = std::slice::from_raw_parts(data as *const i16, count);
                        mono.extend(s.chunks(channels).map(|c| c.iter().map(|&v| v as f32 / 32768.0).sum::<f32>() / channels as f32));
                    }
                    capture.ReleaseBuffer(frames).map_err(err)?;
                    let now = Instant::now();
                    if let Some((d, _)) = probe.as_ref() {
                        d.lock().unwrap_or_else(|e| e.into_inner()).feed(&mono, now);
                        continue;
                    }
                    let heard = detector.feed(&mono, now);
                    let sound = mono.iter().any(|v| v.abs() > 1e-4);
                    publish(&detector, heard, now, sound);
                    if told.elapsed() >= Duration::from_secs(10) {
                        told = now;
                        match detector.tempo() {
                            Some(t) => log::info!("music beat: tempo {:.0} ms ({:.0} a minute, sure {:.2}, stands out {:.1})", t.period_ms, 60_000.0 / t.period_ms, t.sure, t.stands_out),
                            None => log::info!("music beat: no sure tempo, flashes follow what is heard"),
                        }
                    }
                }
            }
            let _ = client.Stop();
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 48 kHz mono blocks of 10 ms: a kick (a 60 Hz burst) every `every_ms`, quiet in between.
    fn song(every_ms: u64, seconds: u64) -> Vec<(Vec<f32>, u64)> {
        let rate = 48_000.0;
        (0..seconds * 100)
            .map(|block| {
                let t0 = block * 10;
                let samples = (0..480)
                    .map(|i| {
                        let t = (t0 as f32 + i as f32 / 48.0) / 1000.0;
                        let since = (t0 % every_ms) as f32 / 1000.0 + i as f32 / rate;
                        let kick = if since < 0.06 { (2.0 * std::f32::consts::PI * 60.0 * t).sin() * 0.8 } else { 0.0 };
                        kick + (2.0 * std::f32::consts::PI * 2000.0 * t).sin() * 0.02
                    })
                    .collect();
                (samples, t0)
            })
            .collect()
    }

    /// With music playing: `cargo test -p hubdeck beat_live -- --ignored --nocapture`
    #[cfg(windows)]
    #[test]
    #[ignore]
    fn beat_live() {
        want(true);
        let (mut beats, mut last, mut glows) = (0, 0, Vec::new());
        for _ in 0..400 {
            std::thread::sleep(Duration::from_millis(10));
            let b = BEAT_AT.load(Ordering::Relaxed);
            if b != last && b != 0 {
                beats += 1;
                last = b;
            }
            glows.push(glow(Instant::now(), &Tuning { dim: 0.6, from: 0.45, predict: true, lead_ms: 20, white_hits: true }).map(|g| g.level));
        }
        want(false);
        let heard = glows.iter().filter(|g| g.is_some()).count();
        let max = glows.iter().flatten().fold(0.0f32, |m, g| m.max(*g));
        println!(
            "4 s: {beats} beats, heard in {heard} of 400 samples, loudness {:.3}, brightest {max:.2}, tempo {} ms",
            f32::from_bits(LOUD.load(Ordering::Relaxed)),
            PERIOD_MS.load(Ordering::Relaxed)
        );
    }

    /// With music playing, 12 s: the gaps between kicks and the tempo found, to tune `tempo_of`.
    /// `cargo test -p hubdeck beat_live_tempo -- --ignored --nocapture`
    #[cfg(windows)]
    #[test]
    #[ignore]
    fn beat_live_tempo() {
        want(true);
        let (mut last, mut gaps, mut periods) = (0u64, Vec::new(), Vec::new());
        for _ in 0..1200 {
            std::thread::sleep(Duration::from_millis(10));
            let b = BEAT_AT.load(Ordering::Relaxed);
            if b != last && b != 0 {
                if last != 0 {
                    gaps.push(b - last);
                }
                last = b;
                periods.push(PERIOD_MS.load(Ordering::Relaxed));
            }
        }
        want(false);
        println!("gaps between kicks (ms): {gaps:?}");
        println!("tempo at each kick (ms, 0 = none): {periods:?}");
    }

    /// With music playing, 15 s: the best period and how sure, every half second, from the live
    /// sound (`cargo test -p hubdeck beat_live_raw -- --ignored --nocapture`).
    #[cfg(windows)]
    #[test]
    #[ignore]
    fn beat_live_raw() {
        // the same listening as the app's, into a detector of the test's own
        want(true);
        std::thread::sleep(Duration::from_millis(500));
        want(false);
        let mut found = Vec::new();
        let d = std::sync::Arc::new(std::sync::Mutex::new(Detector::new(48_000)));
        let probe = d.clone();
        let listen = std::thread::spawn(move || win::listen_into(probe, Duration::from_secs(15)));
        for _ in 0..30 {
            std::thread::sleep(Duration::from_millis(500));
            let raw = tempo_raw(&d.lock().unwrap().onsets, None).map(|t| format!("{:.0} ms {:.2} z{:.1}", t.period_ms, t.sure, t.stands_out));
            found.push(raw.unwrap_or_else(|| "-".into()));
        }
        let _ = listen.join();
        println!("{}", found.join(" | "));
    }

    #[test]
    fn finds_the_beat_of_a_120_bpm_kick() {
        let start = Instant::now();
        let mut d = Detector::new(48_000);
        let beats: Vec<u64> = song(500, 6).into_iter().filter_map(|(s, t)| d.feed(&s, start + Duration::from_millis(t)).beat.then_some(t)).collect();
        // after the first second the average has settled: one beat per kick
        let settled: Vec<&u64> = beats.iter().filter(|t| **t >= 1000).collect();
        assert!((9..=11).contains(&settled.len()), "{beats:?}");
        assert!(settled.windows(2).all(|w| (w[1] - w[0]).abs_diff(500) <= 30), "{beats:?}");
    }

    #[test]
    fn hears_no_beat_in_quiet_or_steady_sound() {
        let start = Instant::now();
        let mut d = Detector::new(48_000);
        let hum: Vec<f32> = (0..480).map(|i| (i as f32 / 48_000.0 * 2.0 * std::f32::consts::PI * 80.0).sin() * 0.3).collect();
        let heard: Vec<Heard> = (0..300).map(|b| d.feed(&hum, start + Duration::from_millis(b * 10))).collect();
        let beats = heard.iter().filter(|h| h.beat).count();
        assert!(beats <= 1, "a steady hum is no beat ({beats})");
        assert!(heard.iter().filter(|h| h.hit.is_some()).count() <= 1, "nor a hit");
        assert!(d.tempo().is_none());
        let mut q = Detector::new(48_000);
        assert!((0..100).all(|b| q.feed(&[0.0; 480], start + Duration::from_millis(b * 10)) == Heard::default()));
    }

    #[test]
    fn flashes_at_the_beat_and_glows_with_the_loudness() {
        let (dim, from) = (0.6, 0.45);
        let after = |ms: f32| (-ms / 180.0).exp();
        assert!(envelope(1.0, after(0.0), dim, from) > 0.99, "full at the beat");
        assert!(envelope(0.3, after(1000.0), dim, from) < 0.2, "a quiet part between beats is dim");
        assert!(envelope(1.6, after(1000.0), dim, from) > envelope(1.1, after(1000.0), dim, from), "a louder part than usual glows brighter");
        // darker between beats as dim goes up; the beat stays full
        assert!(envelope(1.3, after(1000.0), 1.0, from) < envelope(1.3, after(1000.0), 0.5, from) && envelope(1.3, after(1000.0), 0.5, from) < envelope(1.3, after(1000.0), 0.0, from));
        assert!(envelope(1.3, after(1000.0), 1.0, 1.0) < 0.02, "beats only: dark between them");
        assert!(envelope(1.3, after(0.0), 1.0, 1.0) > 0.99);
        // music at its usual loudness: from 0 it glows, from the default it stays at its floor
        assert!(envelope(1.0, after(1000.0), dim, 0.0) > envelope(1.0, after(1000.0), dim, from));
        assert_eq!(PRESETS.iter().find(|p| p.0 == "beat").map(|p| (p.1, p.2)), Some((60, 45)), "the default preset");
    }

    /// Kicks every `every_ms` and a snare (a 1.5 kHz tone with noise) half way between them.
    fn kick_and_snare(every_ms: u64, seconds: u64) -> Vec<(Vec<f32>, u64)> {
        let mut seed = 7u32;
        let mut noise = move || {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (seed >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0
        };
        (0..seconds * 100)
            .map(|block| {
                let t0 = block * 10;
                let samples = (0..480)
                    .map(|i| {
                        let t = (t0 as f32 + i as f32 / 48.0) / 1000.0;
                        let into = (t0 % every_ms) as f32 + i as f32 / 48.0;
                        let kick = if into < 60.0 { (2.0 * std::f32::consts::PI * 60.0 * t).sin() * 0.8 } else { 0.0 };
                        let snare_at = (every_ms / 2) as f32;
                        let snare = if (snare_at..snare_at + 50.0).contains(&into) { (2.0 * std::f32::consts::PI * 1500.0 * t).sin() * 0.3 + noise() * 0.3 } else { 0.0 };
                        kick + snare + noise() * 0.003
                    })
                    .collect();
                (samples, t0)
            })
            .collect()
    }

    #[test]
    fn hears_the_snare_between_the_kicks() {
        let start = Instant::now();
        let mut d = Detector::new(48_000);
        let heard: Vec<(Heard, u64)> = kick_and_snare(500, 6).into_iter().map(|(s, t)| (d.feed(&s, start + Duration::from_millis(t)), t)).collect();
        let hits: Vec<u64> = heard.iter().filter(|(h, t)| h.hit.is_some() && *t >= 1000).map(|(_, t)| *t).collect();
        // one hit per snare, half way between the kicks
        assert!((8..=11).contains(&hits.len()), "{hits:?}");
        assert!(hits.iter().all(|t| (t % 500).abs_diff(250) <= 20), "{hits:?}");
        assert!(heard.iter().filter_map(|(h, _)| h.hit).all(|s| (0.3..=0.7).contains(&s)), "a snare flashes less than a kick");
        assert!(heard.iter().filter(|(h, t)| h.beat && *t >= 1000).count() >= 9, "the kicks still count");
    }

    #[test]
    fn finds_the_tempo_and_follows_its_change() {
        let start = Instant::now();
        let mut d = Detector::new(48_000);
        for (s, t) in song(500, 6) {
            d.feed(&s, start + Duration::from_millis(t));
        }
        let t = d.tempo().expect("120 bpm found");
        assert!((t.period_ms - 500.0).abs() <= 10.0, "{t:?}");
        // the song slows to 100 bpm: after six seconds the new tempo
        for (s, t) in song(600, 7) {
            d.feed(&s, start + Duration::from_millis(6000 + t));
        }
        let t = d.tempo().expect("100 bpm found");
        assert!((t.period_ms - 600.0).abs() <= 10.0, "{t:?}");
        // the music stops: the tempo goes
        for b in 0..300 {
            d.feed(&[0.0; 480], start + Duration::from_millis(13_000 + b * 10));
        }
        assert!(d.tempo().is_none());
    }

    /// Rises every `every_ms` (with some missed and some extra, as real kicks are heard) for
    /// `seconds`, one value per 10 ms block.
    fn rises(every_ms: u64, seconds: u64, start: Instant, messy: bool) -> std::collections::VecDeque<(f32, Instant)> {
        let mut seed = 3u32;
        let mut rand = move || {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (seed >> 8) as f32 / (1u32 << 24) as f32
        };
        (0..seconds * 100)
            .map(|b| {
                let t = b * 10;
                let on = t % every_ms < 10;
                let v = match (on, messy) {
                    (true, true) if rand() < 0.3 => 0.0,
                    (true, _) => 1.0,
                    (false, true) if rand() < 0.015 => 0.8,
                    _ => 0.02 * rand(),
                };
                (v, start + Duration::from_millis(t))
            })
            .collect()
    }

    #[test]
    fn the_tempo_stands_out_even_with_missed_and_extra_kicks() {
        let start = Instant::now();
        for (every, messy) in [(500, false), (500, true), (600, true), (400, true)] {
            let t = tempo_of(&rises(every, 6, start, messy)).unwrap_or_else(|| panic!("{every} ms, messy {messy}: no tempo"));
            assert!((t.period_ms - every as f32).abs() <= 12.0, "{every} ms, messy {messy}: {t:?}");
            // the anchor is a beat: on the grid of the rises
            let at = t.anchor.saturating_duration_since(start).as_millis() as u64;
            assert!(at % every <= 25 || every - at % every <= 25, "{every}: anchor at {at} ms");
        }
        // noise has no tempo, and silence none either
        let mut seed = 11u32;
        let noise: std::collections::VecDeque<(f32, Instant)> = (0..600)
            .map(|b| {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                ((seed >> 8) as f32 / (1u32 << 24) as f32, start + Duration::from_millis(b * 10))
            })
            .collect();
        assert!(tempo_of(&noise).is_none(), "{:?}", tempo_of(&noise));
        // nor a detector fed noise for 10 s: its periods never agree
        let mut d = Detector::new(48_000);
        // xorshift: a plain LCG repeats in patterns the tempo finder hears as a beat
        let mut seed = 0x9E37_79B9u32;
        let mut noisy = false;
        for b in 0..1000u64 {
            let block: Vec<f32> = (0..480)
                .map(|_| {
                    seed ^= seed << 13;
                    seed ^= seed >> 17;
                    seed ^= seed << 5;
                    ((seed >> 8) as f32 / (1u32 << 24) as f32 - 0.5) * 0.4
                })
                .collect();
            d.feed(&block, start + Duration::from_millis(b * 10));
            noisy |= d.tempo().is_some();
        }
        assert!(!noisy, "noise never has a tempo");
        let silence: std::collections::VecDeque<(f32, Instant)> = (0..600).map(|b| (0.0, start + Duration::from_millis(b * 10))).collect();
        assert!(tempo_of(&silence).is_none());
    }

    #[test]
    fn a_flash_fades_with_the_tempo() {
        assert_eq!(fade_ms(0), 180.0, "no tempo: a fifth of a second");
        assert_eq!(fade_ms(500), 150.0, "a third of the time between beats");
        assert_eq!(fade_ms(250), 90.0, "fast: not shorter than 90 ms");
        assert_eq!(fade_ms(1000), 240.0, "slow: not longer than 240 ms");
    }

    #[test]
    fn the_metronome_is_a_wav_with_its_beats() {
        let wav = metronome_wav(2);
        assert_eq!(&wav[..4], b"RIFF");
        assert_eq!(&wav[8..16], b"WAVEfmt ");
        assert_eq!(wav.len(), 44 + 2 * 48_000 * 2);
        // and the detector finds its tempo: 500 ms
        let samples: Vec<f32> = wav[44..].chunks(2).map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0).collect();
        let mut long = samples.clone();
        for _ in 0..3 {
            long.extend_from_slice(&samples);
        }
        let start = Instant::now();
        let mut d = Detector::new(48_000);
        for (i, block) in long.chunks(480).enumerate() {
            d.feed(block, start + Duration::from_millis(i as u64 * 10));
        }
        let t = d.tempo().expect("the metronome's tempo");
        assert!((t.period_ms - 500.0).abs() <= 8.0, "{t:?}");
    }

    #[test]
    fn a_held_tempo_wins_over_a_chance_peak() {
        let start = Instant::now();
        let rises = rises(500, 6, start, true);
        // without a tempo held, and with one held at 500: the same 500
        let free = tempo_raw(&rises, None).map(|t| t.period_ms);
        let kept = tempo_raw(&rises, Some(500.0)).map(|t| t.period_ms);
        assert!(kept.is_some_and(|p| (p - 500.0).abs() <= 12.0), "{free:?} {kept:?}");
    }

    #[test]
    fn a_predicted_flash_comes_just_before_its_beat() {
        // beats every 500 ms from 1000; the flash LEAD_MS before each
        assert_eq!(since_predicted(1500 - LEAD_MS, 500, 1000, LEAD_MS), 0, "the flash starts LEAD_MS early");
        assert_eq!(since_predicted(1500, 500, 1000, LEAD_MS), LEAD_MS);
        assert_eq!(since_predicted(1700, 500, 1000, LEAD_MS), 220);
        assert_eq!(since_predicted(900, 500, 1000, 20), u64::MAX, "nothing before the first kick");
    }
}
