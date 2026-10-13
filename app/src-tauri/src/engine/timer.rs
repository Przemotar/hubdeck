//! Countdown timers and clock faces for a single key or the dial. The engine keeps one timer per
//! control (a hold or double press of the same key reaches the same timer) and redraws a key when
//! its text changes; nothing here touches the hardware.

use std::time::{Duration, Instant};

/// Longest timer: the official app's limit (99 h 59 min 59 s).
pub const MAX: Duration = Duration::from_secs(359_999);

#[derive(Debug, Clone, PartialEq)]
pub struct Timer {
    pub total: Duration,
    /// Left when it was last paused (or the total, when never started).
    left: Duration,
    /// Running since then, with `left` remaining at that moment.
    running_since: Option<Instant>,
    /// The end was signalled (sound played) for this run.
    pub rang: bool,
}

/// What a press, a hold or a turn does to a timer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TimerCmd {
    /// Start, pause or resume; once it ran out, back to the full time.
    StartPause,
    Reset,
    /// Detents of a knob or the dial: one minute each while it is not running.
    Adjust(i32),
}

impl Timer {
    pub fn new(total: Duration) -> Self {
        let total = total.min(MAX).max(Duration::from_secs(1));
        Self { total, left: total, running_since: None, rang: false }
    }

    pub fn remaining(&self, now: Instant) -> Duration {
        match self.running_since {
            Some(since) => self.left.saturating_sub(now.duration_since(since)),
            None => self.left,
        }
    }

    pub fn running(&self) -> bool {
        self.running_since.is_some()
    }

    pub fn finished(&self, now: Instant) -> bool {
        self.remaining(now).is_zero()
    }

    pub fn apply(&mut self, cmd: TimerCmd, now: Instant) {
        match cmd {
            TimerCmd::StartPause if self.finished(now) => self.reset(),
            TimerCmd::StartPause => match self.running_since {
                Some(_) => {
                    self.left = self.remaining(now);
                    self.running_since = None;
                }
                None => self.running_since = Some(now),
            },
            TimerCmd::Reset => self.reset(),
            TimerCmd::Adjust(_) if self.running() => {}
            TimerCmd::Adjust(d) => {
                let secs = (self.left.as_secs() as i64 + d as i64 * 60).clamp(60, MAX.as_secs() as i64);
                self.total = Duration::from_secs(secs as u64);
                self.left = self.total;
                self.rang = false;
            }
        }
    }

    fn reset(&mut self) {
        self.left = self.total;
        self.running_since = None;
        self.rang = false;
    }

    /// 0..=1 of the time left, for the ring on the dial.
    pub fn fraction(&self, now: Instant) -> f32 {
        self.remaining(now).as_secs_f32() / self.total.as_secs_f32().max(1.0)
    }

    pub fn label(&self, now: Instant) -> String {
        // round up, so "0:00" shows only once it really ran out
        let left = self.remaining(now);
        let secs = left.as_secs() + u64::from(left.subsec_nanos() > 0);
        format_secs(secs)
    }
}

/// `4:05`, `1:02:03`.
pub fn format_secs(secs: u64) -> String {
    let (h, m, s) = (secs / 3600, secs / 60 % 60, secs % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

/// The timer's length from its params (`minutes`, `seconds`).
pub fn total_of(params: &serde_json::Value) -> Duration {
    let n = |k: &str, d: f64| super::actions::param_f64(params, k, d).max(0.0) as u64;
    Duration::from_secs(n("minutes", 5.0) * 60 + n("seconds", 0.0)).min(MAX).max(Duration::from_secs(1))
}

/// A clock face: the main text and the date line (None when only one is shown).
/// `show`: `time`, `timeDate`, `date`; `hours`: `24` or `12`.
pub fn clock_text(now: chrono::NaiveDateTime, show: &str, hours: &str) -> (String, Option<String>) {
    let time = if hours == "12" { now.format("%-I:%M%p").to_string() } else { now.format("%H:%M").to_string() };
    let date = now.format("%a %-d %b").to_string();
    match show {
        "date" => (now.format("%a %-d").to_string(), Some(now.format("%B %Y").to_string())),
        "timeDate" => (time, Some(date)),
        _ => (time, None),
    }
}

/// Play the end-of-timer sound without blocking the engine.
pub fn ring() {
    std::thread::spawn(|| {
        #[cfg(windows)]
        {
            use windows::Win32::System::Diagnostics::Debug::MessageBeep;
            use windows::Win32::UI::WindowsAndMessaging::MB_ICONEXCLAMATION;
            for _ in 0..3 {
                // SAFETY: plays the system's own sound; no pointers involved.
                let _ = unsafe { MessageBeep(MB_ICONEXCLAMATION) };
                std::thread::sleep(Duration::from_millis(600));
            }
        }
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("afplay").arg("/System/Library/Sounds/Glass.aiff").status();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn timer_runs_pauses_and_resets() {
        let t0 = Instant::now();
        let at = |s: u64| t0 + Duration::from_secs(s);
        let mut t = Timer::new(Duration::from_secs(90));
        assert_eq!(t.label(t0), "1:30");
        t.apply(TimerCmd::StartPause, t0);
        assert_eq!(t.label(at(31)), "0:59");
        t.apply(TimerCmd::StartPause, at(31)); // pause
        assert_eq!(t.label(at(100)), "0:59", "paused time stands still");
        t.apply(TimerCmd::StartPause, at(100)); // resume
        assert!(t.finished(at(160)));
        assert_eq!(t.label(at(160)), "0:00");
        t.apply(TimerCmd::StartPause, at(161));
        assert_eq!(t.label(at(200)), "1:30", "a finished timer goes back to the full time");
        assert!(!t.running());
    }

    #[test]
    fn turning_sets_minutes_only_while_stopped() {
        let t0 = Instant::now();
        let mut t = Timer::new(Duration::from_secs(300));
        t.apply(TimerCmd::Adjust(2), t0);
        assert_eq!(t.label(t0), "7:00");
        t.apply(TimerCmd::Adjust(-20), t0);
        assert_eq!(t.label(t0), "1:00", "never below a minute");
        t.apply(TimerCmd::StartPause, t0);
        t.apply(TimerCmd::Adjust(5), t0);
        assert_eq!(t.total, Duration::from_secs(60));
    }

    #[test]
    fn lengths_and_labels() {
        assert_eq!(total_of(&json!({"minutes": 1, "seconds": 30})), Duration::from_secs(90));
        assert_eq!(total_of(&json!({})), Duration::from_secs(300));
        assert_eq!(total_of(&json!({"minutes": 0, "seconds": 0})), Duration::from_secs(1));
        assert_eq!(format_secs(3723), "1:02:03");
        let t = chrono::NaiveDate::from_ymd_opt(2026, 10, 11).unwrap().and_hms_opt(21, 5, 0).unwrap();
        assert_eq!(clock_text(t, "time", "24"), ("21:05".into(), None));
        assert_eq!(clock_text(t, "time", "12").0, "9:05PM");
        assert_eq!(clock_text(t, "timeDate", "24"), ("21:05".into(), Some("Sun 11 Oct".into())));
        assert_eq!(clock_text(t, "date", "24"), ("Sun 11".into(), Some("October 2026".into())));
    }
}
