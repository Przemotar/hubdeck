use mh_protocol::{Input, ModuleKind, Orientation};

/// A module is identified by its kind and USB serial number, which survive a reconnect.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ModuleId {
    pub kind: ModuleKind,
    pub serial: String,
}

impl std::fmt::Display for ModuleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.kind.name(), self.serial)
    }
}

/// Edge events: the hub turns state snapshots (all keys, all faders) into changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// The module is open, initialized (software mode for keys and dial) and ready to draw.
    Connected(ModuleId),
    /// The module was pulled off the base or unplugged; its handle is closed.
    Disconnected(ModuleId),
    /// Keys or dial did not answer after entering software mode (firmware wedged, see
    /// [`crate::Module`]). Only pulling it off the base and back fixes it; ask the user to.
    /// The serial may be garbage in this state.
    Unresponsive(ModuleId),
    KeyDown {
        module: ModuleId,
        index: u8,
    },
    KeyUp {
        module: ModuleId,
        index: u8,
    },
    /// Absolute fader position 0..=100.
    Fader {
        module: ModuleId,
        index: u8,
        value: u8,
    },
    /// +1 clockwise, -1 counter-clockwise, one detent.
    Knob {
        module: ModuleId,
        index: u8,
        delta: i8,
    },
    KnobDown {
        module: ModuleId,
        index: u8,
    },
    KnobUp {
        module: ModuleId,
        index: u8,
    },
    /// +1 up, -1 down, one notch.
    Roller {
        module: ModuleId,
        index: u8,
        delta: i8,
    },
    /// +1 clockwise, -1 counter-clockwise, one detent.
    Dial {
        module: ModuleId,
        delta: i8,
    },
    DialDown {
        module: ModuleId,
    },
    DialUp {
        module: ModuleId,
    },
    /// The base was turned (its sensor reports horizontal or vertical); emitted on change only.
    Orientation {
        module: ModuleId,
        orientation: Orientation,
    },
}

impl Event {
    pub fn module(&self) -> &ModuleId {
        match self {
            Event::Connected(m) | Event::Disconnected(m) | Event::Unresponsive(m) => m,
            Event::KeyDown { module, .. }
            | Event::KeyUp { module, .. }
            | Event::Fader { module, .. }
            | Event::Knob { module, .. }
            | Event::KnobDown { module, .. }
            | Event::KnobUp { module, .. }
            | Event::Roller { module, .. }
            | Event::Dial { module, .. }
            | Event::DialDown { module }
            | Event::DialUp { module }
            | Event::Orientation { module, .. } => module,
        }
    }
}

/// Per-module memory used to turn snapshots into edges.
#[derive(Default)]
pub(crate) struct EdgeState {
    keys: [bool; 15],
    faders: Option<[u8; 5]>,
    knobs: [bool; 3],
    orientation: Option<Orientation>,
}

impl EdgeState {
    pub(crate) fn apply(&mut self, id: &ModuleId, input: Input) -> Vec<Event> {
        let module = || id.clone();
        match input {
            Input::Keys(now) => {
                let mut out = Vec::new();
                for (i, (old, new)) in self.keys.iter().zip(now.iter()).enumerate() {
                    if old != new {
                        let index = i as u8;
                        out.push(if *new { Event::KeyDown { module: module(), index } } else { Event::KeyUp { module: module(), index } });
                    }
                }
                self.keys = now;
                out
            }
            Input::Faders(now) => {
                let before = self.faders.replace(now);
                now.iter().enumerate().filter(|(i, v)| before.is_none_or(|b| b[*i] != **v)).map(|(i, v)| Event::Fader { module: module(), index: i as u8, value: *v }).collect()
            }
            Input::Knob { index, delta } => vec![Event::Knob { module: module(), index, delta }],
            Input::KnobPress { index, pressed } => {
                let slot = &mut self.knobs[index.min(2) as usize];
                if *slot == pressed {
                    return Vec::new();
                }
                *slot = pressed;
                vec![if pressed { Event::KnobDown { module: module(), index } } else { Event::KnobUp { module: module(), index } }]
            }
            Input::Roller { index, delta } => vec![Event::Roller { module: module(), index, delta }],
            Input::Dial { delta } => vec![Event::Dial { module: module(), delta }],
            Input::DialPress { pressed } => vec![if pressed { Event::DialDown { module: module() } } else { Event::DialUp { module: module() } }],
            Input::Orientation(o) => {
                if self.orientation.replace(o) == Some(o) {
                    return Vec::new();
                }
                vec![Event::Orientation { module: module(), orientation: o }]
            }
            Input::Unknown { .. } => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(kind: ModuleKind) -> ModuleId {
        ModuleId { kind, serial: "SN".into() }
    }

    #[test]
    fn keys_snapshots_become_edges() {
        let mut s = EdgeState::default();
        let m = id(ModuleKind::Keys);
        let mut k = [false; 15];
        k[3] = true;
        assert_eq!(s.apply(&m, Input::Keys(k)), vec![Event::KeyDown { module: m.clone(), index: 3 }]);
        k[7] = true;
        assert_eq!(s.apply(&m, Input::Keys(k)), vec![Event::KeyDown { module: m.clone(), index: 7 }]);
        assert_eq!(s.apply(&m, Input::Keys([false; 15])).len(), 2);
    }

    #[test]
    fn faders_report_only_moves_after_the_first_snapshot() {
        let mut s = EdgeState::default();
        let m = id(ModuleKind::Faders);
        assert_eq!(s.apply(&m, Input::Faders([1, 2, 3, 4, 5])).len(), 5);
        assert_eq!(s.apply(&m, Input::Faders([1, 9, 3, 4, 5])), vec![Event::Fader { module: m.clone(), index: 1, value: 9 }]);
    }

    #[test]
    fn knob_press_is_deduplicated() {
        let mut s = EdgeState::default();
        let m = id(ModuleKind::Knobs);
        assert_eq!(s.apply(&m, Input::KnobPress { index: 1, pressed: false }), vec![]);
        assert_eq!(s.apply(&m, Input::KnobPress { index: 1, pressed: true }).len(), 1);
        assert_eq!(s.apply(&m, Input::KnobPress { index: 1, pressed: true }), vec![]);
    }
}
