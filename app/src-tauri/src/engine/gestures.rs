//! Press gestures: tap, hold and double press on keys, knob pushes and the dial.
//!
//! A control with only a tap binding fires on the way down, as before. A control that also has a
//! hold or double press binding has to wait: a tap is only known when the key comes up (and, with
//! a double press bound, when no second press follows in time).

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// How long a press must last to count as a hold (the official app used 600 ms on the dial).
pub const HOLD_AFTER: Duration = Duration::from_millis(500);
/// How soon the second press of a double press must come after the first release.
pub const DOUBLE_WITHIN: Duration = Duration::from_millis(300);

/// Faders: snap the last 2 % at each end to 0 and 100, and ignore a change of 1 (a resting fader
/// can flicker between two values). Returns the value to use, or None to ignore this report.
pub fn fader_filter(last: Option<u8>, raw: u8) -> Option<u8> {
    let v = match raw {
        0..=2 => 0,
        98.. => 100,
        v => v,
    };
    match last {
        Some(l) if l == v => None,
        Some(l) if l.abs_diff(v) < 2 && v != 0 && v != 100 => None,
        _ => Some(v),
    }
}

/// Turn acceleration: how many detents one detent counts for, from the time since the last one.
pub fn accel_factor(since_last: Duration) -> i8 {
    match since_last.as_millis() {
        0..50 => 3,
        50..110 => 2,
        _ => 1,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gesture {
    Tap,
    Hold,
    Double,
}

impl Gesture {
    /// Suffix of the control id that holds this gesture's binding (`keys.4:hold`).
    pub fn suffix(self) -> &'static str {
        match self {
            Gesture::Tap => "",
            Gesture::Hold => ":hold",
            Gesture::Double => ":double",
        }
    }
}

/// Which gestures a control has bindings for, besides the tap.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Bound {
    pub hold: bool,
    pub double: bool,
}

#[derive(Debug, Clone, Copy)]
enum State {
    /// Down since `at`; a hold fires at `at + HOLD_AFTER` if bound.
    Down { at: Instant, bound: Bound },
    /// The hold fired; ignore the release.
    Held,
    /// Released once at `at`; a second press before `at + DOUBLE_WITHIN` is a double press.
    Released { at: Instant },
    /// The double press fired; ignore its release.
    Doubled,
}

#[derive(Default)]
pub struct Gestures {
    state: HashMap<String, State>,
}

impl Gestures {
    /// A press or release of `control`. Returns what fires right now.
    pub fn press(&mut self, control: &str, down: bool, now: Instant, bound: Bound) -> Option<Gesture> {
        if !bound.hold && !bound.double {
            self.state.remove(control);
            return down.then_some(Gesture::Tap);
        }
        let previous = self.state.remove(control);
        match (down, previous) {
            (true, Some(State::Released { at, .. })) if bound.double && now.duration_since(at) <= DOUBLE_WITHIN => {
                self.state.insert(control.into(), State::Doubled);
                Some(Gesture::Double)
            }
            (true, _) => {
                self.state.insert(control.into(), State::Down { at: now, bound });
                None
            }
            (false, Some(State::Down { bound, .. })) if bound.double => {
                self.state.insert(control.into(), State::Released { at: now });
                None
            }
            (false, Some(State::Down { .. })) => Some(Gesture::Tap),
            (false, _) => None,
        }
    }

    /// Gestures that are decided by time passing: holds, and taps that waited for a second press.
    pub fn tick(&mut self, now: Instant) -> Vec<(String, Gesture)> {
        let mut fired = Vec::new();
        for (control, state) in self.state.iter_mut() {
            match *state {
                State::Down { at, bound } if bound.hold && now.duration_since(at) >= HOLD_AFTER => {
                    *state = State::Held;
                    fired.push((control.clone(), Gesture::Hold));
                }
                State::Released { at, .. } if now.duration_since(at) > DOUBLE_WITHIN => {
                    fired.push((control.clone(), Gesture::Tap));
                }
                _ => {}
            }
        }
        self.state.retain(|c, s| !(matches!(s, State::Released { .. }) && fired.iter().any(|(f, _)| f == c)));
        fired
    }

    /// Forget everything (profile or page changed while a key was down).
    pub fn clear(&mut self) {
        self.state.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOLD: Bound = Bound { hold: true, double: false };
    const DOUBLE: Bound = Bound { hold: false, double: true };
    const BOTH: Bound = Bound { hold: true, double: true };

    fn ms(t0: Instant, n: u64) -> Instant {
        t0 + Duration::from_millis(n)
    }

    #[test]
    fn faders_snap_and_ignore_flicker() {
        assert_eq!(fader_filter(None, 1), Some(0));
        assert_eq!(fader_filter(Some(0), 2), None, "still at the bottom");
        assert_eq!(fader_filter(Some(50), 51), None, "flicker");
        assert_eq!(fader_filter(Some(50), 52), Some(52));
        assert_eq!(fader_filter(Some(96), 99), Some(100), "the top snaps");
        assert_eq!(fader_filter(Some(4), 3), None, "a change of 1 is ignored everywhere but the ends");
        assert_eq!(fader_filter(Some(3), 2), Some(0), "reaching the end always counts");
    }

    #[test]
    fn fast_turns_count_more() {
        assert_eq!(accel_factor(Duration::from_millis(30)), 3);
        assert_eq!(accel_factor(Duration::from_millis(80)), 2);
        assert_eq!(accel_factor(Duration::from_millis(400)), 1);
    }

    #[test]
    fn plain_tap_fires_on_the_way_down() {
        let mut g = Gestures::default();
        let t = Instant::now();
        assert_eq!(g.press("keys.0", true, t, Bound::default()), Some(Gesture::Tap));
        assert_eq!(g.press("keys.0", false, ms(t, 50), Bound::default()), None);
    }

    #[test]
    fn hold_fires_after_the_delay_and_swallows_the_release() {
        let mut g = Gestures::default();
        let t = Instant::now();
        assert_eq!(g.press("dial", true, t, HOLD), None);
        assert!(g.tick(ms(t, 400)).is_empty());
        assert_eq!(g.tick(ms(t, 520)), vec![("dial".to_string(), Gesture::Hold)]);
        assert!(g.tick(ms(t, 800)).is_empty(), "fires once");
        assert_eq!(g.press("dial", false, ms(t, 900), HOLD), None);
    }

    #[test]
    fn short_press_with_hold_bound_is_a_tap_on_release() {
        let mut g = Gestures::default();
        let t = Instant::now();
        assert_eq!(g.press("keys.3", true, t, HOLD), None);
        assert_eq!(g.press("keys.3", false, ms(t, 150), HOLD), Some(Gesture::Tap));
    }

    #[test]
    fn double_press_and_single_tap_after_waiting() {
        let mut g = Gestures::default();
        let t = Instant::now();
        assert_eq!(g.press("keys.1", true, t, DOUBLE), None);
        assert_eq!(g.press("keys.1", false, ms(t, 80), DOUBLE), None);
        assert_eq!(g.press("keys.1", true, ms(t, 250), DOUBLE), Some(Gesture::Double));
        assert_eq!(g.press("keys.1", false, ms(t, 320), DOUBLE), None);
        assert!(g.tick(ms(t, 1000)).is_empty(), "a double press leaves no tap behind");

        assert_eq!(g.press("keys.1", true, ms(t, 2000), DOUBLE), None);
        assert_eq!(g.press("keys.1", false, ms(t, 2080), DOUBLE), None);
        assert!(g.tick(ms(t, 2300)).is_empty(), "still waiting for a second press");
        assert_eq!(g.tick(ms(t, 2400)), vec![("keys.1".to_string(), Gesture::Tap)]);
        assert!(g.tick(ms(t, 2600)).is_empty());
    }

    #[test]
    fn hold_wins_over_double_when_the_key_stays_down() {
        let mut g = Gestures::default();
        let t = Instant::now();
        assert_eq!(g.press("knobs.0", true, t, BOTH), None);
        assert_eq!(g.tick(ms(t, 600)), vec![("knobs.0".to_string(), Gesture::Hold)]);
        assert_eq!(g.press("knobs.0", false, ms(t, 700), BOTH), None);
        assert!(g.tick(ms(t, 1200)).is_empty());
    }
}
