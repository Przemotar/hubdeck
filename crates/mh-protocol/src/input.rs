//! Input reports: `[0x01, tag, len(LE16), payload...]`, payload = `event_id, count, values...`.

use crate::module::ModuleKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    /// Full state of the 15 keys (true = pressed); sent on every change.
    Keys([bool; 15]),
    /// All five fader values 0..=100; sent when any of them moves.
    Faders([u8; 5]),
    /// One detent of a knob (every 12 degrees): +1 clockwise, -1 counter-clockwise.
    Knob {
        index: u8,
        delta: i8,
    },
    KnobPress {
        index: u8,
        pressed: bool,
    },
    /// One notch of a roller: +1 up, -1 down.
    Roller {
        index: u8,
        delta: i8,
    },
    /// One detent of the dial: +1 clockwise, -1 counter-clockwise.
    Dial {
        delta: i8,
    },
    DialPress {
        pressed: bool,
    },
    /// Base orientation from its built-in sensor; sent when it changes (and repeated while the
    /// base is moving). Tag 0x2B, payload `01` horizontal / `00` vertical.
    Orientation(Orientation),
    Unknown {
        tag: u8,
        payload: Vec<u8>,
    },
}

const NO_CHANGE: u8 = 0xFF;

/// How the base stands. The official app shows `1` as horizontal and anything else as vertical.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Orientation {
    Horizontal,
    Vertical,
}

impl Orientation {
    pub fn from_byte(v: u8) -> Self {
        if v == 1 { Self::Horizontal } else { Self::Vertical }
    }
}

/// Decode one raw read (as returned by `hid_read`, report id included).
pub fn parse(kind: ModuleKind, raw: &[u8]) -> Vec<Input> {
    if raw.len() < 4 || raw[0] != 0x01 {
        return Vec::new();
    }
    let tag = raw[1];
    let len = u16::from_le_bytes([raw[2], raw[3]]) as usize;
    let payload = &raw[4..(4 + len).min(raw.len())];
    let unknown = || vec![Input::Unknown { tag, payload: payload.to_vec() }];

    match (kind, tag, payload) {
        (ModuleKind::Base, 0x2B, [v, ..]) => vec![Input::Orientation(Orientation::from_byte(*v))],
        (ModuleKind::Keys, 0x34, [0x02, 15, rest @ ..]) if rest.len() >= 15 => {
            let mut keys = [false; 15];
            for (k, v) in keys.iter_mut().zip(rest) {
                *k = *v != 0;
            }
            vec![Input::Keys(keys)]
        }
        (ModuleKind::Faders, 0x2A, [0x02, 5, rest @ ..]) if rest.len() >= 5 => {
            vec![Input::Faders(rest[..5].try_into().unwrap())]
        }
        (ModuleKind::Knobs, 0x2A, [0x00, 3, rest @ ..]) => {
            rest.iter().take(3).enumerate().filter(|(_, v)| **v != NO_CHANGE).map(|(i, v)| Input::Knob { index: i as u8, delta: if *v == 1 { 1 } else { -1 } }).collect()
        }
        (ModuleKind::Knobs, 0x2A, [0x03, 3, rest @ ..]) => rest.iter().take(3).enumerate().map(|(i, v)| Input::KnobPress { index: i as u8, pressed: *v != 0 }).collect(),
        (ModuleKind::Rollers, 0x2A, [0x01, 2, rest @ ..]) => {
            rest.iter().take(2).enumerate().filter(|(_, v)| **v != NO_CHANGE).map(|(i, v)| Input::Roller { index: i as u8, delta: if *v == 0 { 1 } else { -1 } }).collect()
        }
        (ModuleKind::Dial, 0x44, [0x03, 1, dir, ..]) => vec![Input::Dial { delta: if *dir == 0 { 1 } else { -1 } }],
        (ModuleKind::Dial, 0x44, [0x04, 1, state, ..]) => vec![Input::DialPress { pressed: *state != 0 }],
        _ => unknown(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(tag: u8, payload: &[u8]) -> Vec<u8> {
        let mut r = vec![0x01, tag, payload.len() as u8, 0];
        r.extend_from_slice(payload);
        r.resize(64, 0);
        r
    }

    #[test]
    fn keys_state() {
        let mut p = vec![0x02, 0x0f];
        p.extend_from_slice(&[0; 15]);
        p[2 + 9] = 1;
        p[2 + 13] = 1;
        let Input::Keys(k) = &parse(ModuleKind::Keys, &raw(0x34, &p))[0] else { panic!() };
        assert!(k[9] && k[13] && !k[0]);
    }

    #[test]
    fn captured_reports() {
        assert_eq!(parse(ModuleKind::Faders, &raw(0x2A, &[0x02, 0x05, 0x33, 0x00, 0x64, 0x19, 0x4b])), vec![Input::Faders([0x33, 0, 100, 0x19, 0x4b])]);
        assert_eq!(parse(ModuleKind::Knobs, &raw(0x2A, &[0x00, 0x03, 0x01, 0xff, 0xff])), vec![Input::Knob { index: 0, delta: 1 }]);
        assert_eq!(parse(ModuleKind::Knobs, &raw(0x2A, &[0x00, 0x03, 0xff, 0xff, 0x00])), vec![Input::Knob { index: 2, delta: -1 }]);
        assert_eq!(parse(ModuleKind::Knobs, &raw(0x2A, &[0x03, 0x03, 0x01, 0x00, 0x00]))[0], Input::KnobPress { index: 0, pressed: true });
        assert_eq!(parse(ModuleKind::Rollers, &raw(0x2A, &[0x01, 0x02, 0x00, 0xff])), vec![Input::Roller { index: 0, delta: 1 }]);
        assert_eq!(parse(ModuleKind::Dial, &raw(0x44, &[0x03, 0x01, 0x00])), vec![Input::Dial { delta: 1 }]);
        assert_eq!(parse(ModuleKind::Dial, &raw(0x44, &[0x03, 0x01, 0x01])), vec![Input::Dial { delta: -1 }]);
        assert_eq!(parse(ModuleKind::Dial, &raw(0x44, &[0x04, 0x01, 0x01])), vec![Input::DialPress { pressed: true }]);
        assert_eq!(parse(ModuleKind::Base, &raw(0x2B, &[0x01])), vec![Input::Orientation(Orientation::Horizontal)]);
        assert_eq!(parse(ModuleKind::Base, &raw(0x2B, &[0x00])), vec![Input::Orientation(Orientation::Vertical)]);
    }

    #[test]
    fn junk_is_ignored_or_unknown() {
        assert!(parse(ModuleKind::Keys, &[]).is_empty());
        assert!(parse(ModuleKind::Keys, &[0x02, 0, 0, 0]).is_empty());
        assert!(matches!(parse(ModuleKind::Rollers, &raw(0x2C, &[9]))[0], Input::Unknown { tag: 0x2C, .. }));
    }
}
