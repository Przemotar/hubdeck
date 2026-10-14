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
    /// One more minute, running or not (the dial's double press).
    AddMinute,
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
            TimerCmd::AddMinute => {
                let left = self.remaining(now) + Duration::from_secs(60);
                self.total = (self.total + Duration::from_secs(60)).min(MAX);
                self.left = left.min(MAX);
                if self.running_since.is_some() {
                    self.running_since = Some(now);
                }
                self.rang = false;
            }
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

/// A pomodoro as a binding sets it up (`focus`, `short`, `long` in minutes, `rounds`).
pub fn pomodoro_of(params: &serde_json::Value) -> Pomodoro {
    let mins = |k: &str, d: f64| Duration::from_secs((super::actions::param_f64(params, k, d).clamp(1.0, 180.0) * 60.0) as u64);
    Pomodoro::new(mins("focus", 25.0), mins("short", 5.0), mins("long", 15.0), super::actions::param_f64(params, "rounds", 4.0).clamp(1.0, 12.0) as u8)
}

/// A clock face: the main text and the date line (None when only one is shown).
/// `show`: `time`, `timeDate`, `date`; `hours`: `24` or `12`.
pub fn clock_text(now: chrono::NaiveDateTime, show: &str, hours: &str) -> (String, Option<String>) {
    let time = if hours == "12" { now.format("%-I:%M%p").to_string() } else { now.format("%H:%M").to_string() };
    let date = now.format("%a %-d %b").to_string();
    match show {
        "date" => (now.format("%a %-d").to_string(), Some(now.format("%B %Y").to_string())),
        "timeDate" => (time, Some(date)),
        "seconds" => (if hours == "12" { now.format("%-I:%M:%S").to_string() } else { now.format("%H:%M:%S").to_string() }, Some(date)),
        "week" => (now.format("Week %V").to_string(), Some(now.format("Day %-j of %Y").to_string())),
        _ => (time, None),
    }
}

/// A pomodoro on one control (owner's pick P1, 2026-10-14): focus, a short break, again; after
/// `rounds` focus sessions a long break. A phase that ends rings and waits for a press.
#[derive(Debug, Clone, PartialEq)]
pub struct Pomodoro {
    pub focus: Duration,
    pub short: Duration,
    pub long: Duration,
    pub rounds: u8,
    /// Focus sessions finished in this set.
    pub done: u8,
    pub on_break: bool,
    pub timer: Timer,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PomodoroCmd {
    StartPause,
    /// On to the next phase (a break now, or the next focus).
    Skip,
    /// Back to the first focus session.
    Reset,
    /// Detents: the focus length, a minute each, while it is not running.
    Adjust(i32),
}

impl Pomodoro {
    pub fn new(focus: Duration, short: Duration, long: Duration, rounds: u8) -> Self {
        Self { focus, short, long, rounds: rounds.clamp(1, 12), done: 0, on_break: false, timer: Timer::new(focus) }
    }

    /// The phase after this one, paused.
    pub fn advance(&mut self) {
        if self.on_break {
            self.on_break = false;
            if self.done >= self.rounds {
                self.done = 0;
            }
            self.timer = Timer::new(self.focus);
        } else {
            self.done = (self.done + 1).min(self.rounds);
            self.on_break = true;
            self.timer = Timer::new(if self.done >= self.rounds { self.long } else { self.short });
        }
    }

    pub fn apply(&mut self, cmd: PomodoroCmd, now: Instant) {
        match cmd {
            PomodoroCmd::StartPause => self.timer.apply(TimerCmd::StartPause, now),
            PomodoroCmd::Skip => self.advance(),
            PomodoroCmd::Reset => *self = Pomodoro::new(self.focus, self.short, self.long, self.rounds),
            PomodoroCmd::Adjust(_) if self.timer.running() || self.on_break => {}
            PomodoroCmd::Adjust(d) => {
                let mins = (self.focus.as_secs() as i64 / 60 + d as i64).clamp(1, 180) as u64;
                self.focus = Duration::from_secs(mins * 60);
                self.timer = Timer::new(self.focus);
            }
        }
    }

    /// The phase's name: `Focus`, `Break`, `Long break` (Polish too).
    pub fn phase(&self, polish: bool) -> &'static str {
        match (self.on_break, self.done >= self.rounds, polish) {
            (false, _, false) => "Focus",
            (false, _, true) => "Skupienie",
            (true, false, false) => "Break",
            (true, false, true) => "Przerwa",
            (true, true, false) => "Long break",
            (true, true, true) => "Długa przerwa",
        }
    }
}

/// What turning a clock flips through.
pub const CLOCK_VIEWS: [&str; 4] = ["timeDate", "seconds", "date", "week"];

/// A clock's own state on one control (the dial's gestures): what it shows, the hours, a stopwatch.
#[derive(Debug, Clone, Default)]
pub struct ClockState {
    /// An index into `CLOCK_VIEWS` once turned; None = the binding's own setting.
    pub view: Option<usize>,
    /// 12-hour (true) or 24-hour once switched; None = the binding's own setting.
    pub h12: Option<bool>,
    since: Option<Instant>,
    acc: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ClockCmd {
    /// Start or stop the stopwatch.
    Stopwatch,
    StopwatchReset,
    /// Flip through the views (detents).
    View(i32),
    /// 12 <-> 24 hours.
    Hours,
}

impl ClockState {
    /// The stopwatch's time, while it runs or after it stopped (None once reset).
    pub fn stopwatch(&self, now: Instant) -> Option<Duration> {
        (self.since.is_some() || !self.acc.is_zero()).then(|| self.acc + self.since.map(|s| now.saturating_duration_since(s)).unwrap_or_default())
    }

    pub fn running(&self) -> bool {
        self.since.is_some()
    }

    pub fn apply(&mut self, cmd: ClockCmd, now: Instant, own_view: &str, own_h12: bool) {
        match cmd {
            ClockCmd::Stopwatch => match self.since.take() {
                Some(s) => self.acc += now.saturating_duration_since(s),
                None => self.since = Some(now),
            },
            ClockCmd::StopwatchReset => (self.since, self.acc) = (None, Duration::ZERO),
            ClockCmd::View(d) => {
                let n = CLOCK_VIEWS.len() as i32;
                let at = self.view.unwrap_or_else(|| CLOCK_VIEWS.iter().position(|v| *v == own_view).unwrap_or(0)) as i32;
                self.view = Some((at + d).rem_euclid(n) as usize);
            }
            ClockCmd::Hours => self.h12 = Some(!self.h12.unwrap_or(own_h12)),
        }
    }
}

/// A stopwatch's time: `1:05.3` under an hour, `1:02:05` after.
pub fn stopwatch_text(d: Duration) -> String {
    let s = d.as_secs();
    if s >= 3600 { format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60) } else { format!("{}:{:02}.{}", s / 60, s % 60, d.subsec_millis() / 100) }
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

    #[test]
    fn a_pomodoro_goes_focus_break_and_a_long_break_after_its_rounds() {
        let m = |n: u64| Duration::from_secs(n * 60);
        let mut p = Pomodoro::new(m(25), m(5), m(15), 2);
        let now = Instant::now();
        assert_eq!((p.on_break, p.timer.total), (false, m(25)));
        p.apply(PomodoroCmd::Adjust(5), now);
        assert_eq!(p.focus, m(30), "turning sets the focus while stopped");
        p.advance();
        assert_eq!((p.on_break, p.done, p.timer.total), (true, 1, m(5)));
        assert!(!p.timer.running(), "a new phase waits for a press");
        p.advance();
        assert_eq!((p.on_break, p.timer.total), (false, m(30)));
        p.advance();
        assert_eq!((p.done, p.timer.total, p.phase(false)), (2, m(15), "Long break"));
        p.advance();
        assert_eq!((p.done, p.on_break), (0, false), "after the long break a new set");
        p.apply(PomodoroCmd::Skip, now);
        p.apply(PomodoroCmd::Reset, now);
        assert_eq!((p.done, p.on_break, p.timer.total), (0, false, m(30)));
    }
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
