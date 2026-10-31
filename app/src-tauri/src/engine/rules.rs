//! Looks by conditions and live values in labels (owner's "2 a", 2026-10-25, step 4 of the icon
//! work; as Bitfocus Companion's feedbacks: a key changes how it looks while something holds).
//! A key's look carries its rules in order; every rule that holds lays its look's fields over the
//! key's (a later one wins), and `{cpu}`, `{song}` and the like in a label become their values.
//! Drawn by the engine at its live pace (four times a second), only when what is shown changes.

use serde::{Deserialize, Serialize};

use super::monitor::{Metric, Sample};
use super::profile::Look;

/// One rule: while `when` holds, the key takes the fields `look` sets.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LookRule {
    pub when: Condition,
    pub look: Look,
}

/// What a rule waits for.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Condition {
    /// A computer's value over a number: `cpu`, `ram`, `gpu` (%), `cpuTemp`, `gpuTemp` (°C).
    Metric { metric: String, above: f32 },
    /// Music plays (Now playing).
    Playing,
    /// The microphone is muted.
    MicMuted,
    /// An app is in front, by its program name (`obs64.exe`, `Spotify`).
    AppFront { app: String },
    /// The clock is between two times, `HH:MM` (over midnight when `from` is later than `to`).
    Time { from: String, to: String },
}

/// What the rules and labels read, taken once per drawing.
pub struct Now<'a> {
    pub sample: Option<&'a Sample>,
    pub track: Option<(&'a str, &'a str, bool)>,
    pub mic_muted: bool,
    pub foreground: &'a str,
    pub clock: chrono::NaiveDateTime,
}

fn metric(name: &str) -> Option<Metric> {
    Some(match name {
        "cpu" => Metric::Cpu,
        "ram" => Metric::Ram,
        "gpu" => Metric::Gpu,
        "cpuTemp" => Metric::CpuTemp,
        "gpuTemp" => Metric::GpuTemp,
        _ => return None,
    })
}

fn minutes(hhmm: &str) -> Option<u32> {
    let (h, m) = hhmm.trim().split_once(':')?;
    let (h, m): (u32, u32) = (h.parse().ok()?, m.parse().ok()?);
    (h < 24 && m < 60).then_some(h * 60 + m)
}

/// The program name without a path or `.exe`, lower case.
fn app_stem(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name).to_lowercase();
    base.strip_suffix(".exe").unwrap_or(&base).to_string()
}

impl Condition {
    pub fn holds(&self, now: &Now) -> bool {
        use chrono::Timelike as _;
        match self {
            Condition::Metric { metric: m, above } => metric(m).and_then(|m| now.sample.and_then(|s| s.get(m))).is_some_and(|v| v > *above),
            Condition::Playing => now.track.is_some_and(|(_, _, playing)| playing),
            Condition::MicMuted => now.mic_muted,
            Condition::AppFront { app } => !app.trim().is_empty() && app_stem(now.foreground) == app_stem(app),
            Condition::Time { from, to } => match (minutes(from), minutes(to)) {
                (Some(a), Some(b)) => {
                    let t = now.clock.hour() * 60 + now.clock.minute();
                    if a <= b { (a..b).contains(&t) } else { t >= a || t < b }
                }
                _ => false,
            },
        }
    }

    /// Whether the rule is well formed (the local API's check).
    pub fn problem(&self) -> Option<String> {
        match self {
            Condition::Metric { metric: m, .. } if metric(m).is_none() => Some(format!("metric {m:?} is not one of cpu, ram, gpu, cpuTemp, gpuTemp")),
            Condition::Time { from, to } if minutes(from).is_none() || minutes(to).is_none() => Some("time is HH:MM".into()),
            _ => None,
        }
    }
}

/// `look` with every rule that holds laid over it (the fields a rule sets win; its own rules and
/// label stay the key's unless the rule sets a label), and the values in its label filled in.
/// Also a short text of what was applied, to tell when the picture must change.
pub fn apply(look: Look, now: &Now) -> (Look, String) {
    let mut sig = String::new();
    let mut out = look;
    if !out.rules.is_empty() {
        let rules = out.rules.clone();
        for (i, rule) in rules.iter().enumerate() {
            if rule.when.holds(now) {
                sig.push_str(&format!("r{i};"));
                out = overlay(out, &rule.look);
            }
        }
    }
    if let Some(label) = out.label.as_deref().filter(|l| l.contains('{')) {
        let filled = fill(label, now);
        sig.push_str(&filled);
        out.label = Some(filled);
    }
    (out, sig)
}

/// `base` with the fields `top` sets (a rule's look never brings rules of its own).
fn overlay(base: Look, top: &Look) -> Look {
    let (Ok(serde_json::Value::Object(mut b)), Ok(serde_json::Value::Object(t))) = (serde_json::to_value(&base), serde_json::to_value(top)) else { return base };
    for (k, v) in t {
        if k != "rules" {
            b.insert(k, v);
        }
    }
    let mut out: Look = serde_json::from_value(serde_json::Value::Object(b)).unwrap_or(base.clone());
    // what the engine set while drawing (never saved) stays
    (out.t, out.face, out.pressed, out.failed) = (base.t, base.face, base.pressed, base.failed);
    out
}

/// A label with `{cpu}`, `{song}` and the like as their values now; an unknown name stays.
pub fn fill(label: &str, now: &Now) -> String {
    let mut out = String::new();
    let mut rest = label;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else {
            out.push_str(&rest[open..]);
            return out;
        };
        let name = &after[..close];
        match value(name, now) {
            Some(v) => out.push_str(&v),
            None => out.push_str(&rest[open..open + close + 2]),
        }
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    out
}

fn value(name: &str, now: &Now) -> Option<String> {
    if let Some(m) = metric(name) {
        return Some(now.sample.and_then(|s| s.get(m)).map(|v| m.format(v)).unwrap_or_else(|| "–".into()));
    }
    Some(match name {
        "time" => now.clock.format("%H:%M").to_string(),
        "date" => now.clock.format("%d.%m").to_string(),
        "song" => now.track.map(|(t, _, _)| t.to_string()).unwrap_or_default(),
        "artist" => now.track.map(|(_, a, _)| a.to_string()).unwrap_or_default(),
        "app" => {
            let stem = app_stem(now.foreground);
            let mut c = stem.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now<'a>(sample: Option<&'a Sample>, foreground: &'a str, hh: u32, mm: u32) -> Now<'a> {
        let clock = chrono::NaiveDate::from_ymd_opt(2026, 10, 25).unwrap().and_hms_opt(hh, mm, 0).unwrap();
        Now { sample, track: Some(("Tear", "RHCP", true)), mic_muted: false, foreground, clock }
    }

    #[test]
    fn rules_lay_their_look_over_the_key_while_they_hold() {
        let sample = Sample { cpu: 91.0, ..Sample::default() };
        let key = Look {
            label: Some("CPU {cpu}".into()),
            bg: Some("#222222".into()),
            rules: vec![
                LookRule { when: Condition::Metric { metric: "cpu".into(), above: 80.0 }, look: Look { bg: Some("#c0392b".into()), ..Look::default() } },
                LookRule { when: Condition::AppFront { app: "obs64.exe".into() }, look: Look { ring: Some("#ffffff".into()), ..Look::default() } },
            ],
            ..Look::default()
        };
        let (busy, sig) = apply(key.clone(), &now(Some(&sample), "C:\\Program Files\\obs-studio\\bin\\64bit\\obs64.exe", 12, 0));
        assert_eq!(busy.bg.as_deref(), Some("#c0392b"));
        assert_eq!(busy.ring.as_deref(), Some("#ffffff"));
        assert_eq!(busy.label.as_deref(), Some("CPU 91%"));
        assert_eq!(busy.rules.len(), 2);
        let calm = Sample { cpu: 20.0, ..Sample::default() };
        let (quiet, sig2) = apply(key, &now(Some(&calm), "firefox.exe", 12, 0));
        assert_eq!(quiet.bg.as_deref(), Some("#222222"));
        assert!(quiet.ring.is_none());
        assert_ne!(sig, sig2);
    }

    #[test]
    fn times_go_over_midnight_and_labels_keep_unknown_names() {
        let late = Condition::Time { from: "22:00".into(), to: "06:00".into() };
        assert!(late.holds(&now(None, "", 23, 30)));
        assert!(late.holds(&now(None, "", 5, 59)));
        assert!(!late.holds(&now(None, "", 12, 0)));
        assert_eq!(fill("{song} · {artist} {nope} {", &now(None, "", 12, 0)), "Tear · RHCP {nope} {");
        assert_eq!(fill("{time} {app}", &now(None, "spotify.exe", 9, 5)), "09:05 Spotify");
        assert!(Condition::Metric { metric: "disk".into(), above: 1.0 }.problem().is_some());
        assert!(Condition::Time { from: "25:00".into(), to: "1:00".into() }.problem().is_some());
    }
}
