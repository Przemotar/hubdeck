//! Module kinds, identified by USB product id (vendor id is always [`VENDOR_ID`]).

pub const VENDOR_ID: u16 = 0x2516;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ModuleKind {
    /// M0064, 4x6 grid that the other modules attach to. Edge LED, heartbeat.
    Base,
    /// M4315, 15 keys over one 480x272 IPS panel (5x3 keys of 72x72 px).
    Keys,
    /// M005S, 5 faders with LEDs, values 0..=100.
    Faders,
    /// M003K, 3 endless knobs with LEDs and push.
    Knobs,
    /// M032LE, 2 notched rollers.
    Rollers,
    /// M131E, one endless dial with push and a 240x240 IPS screen.
    Dial,
}

impl ModuleKind {
    pub const ALL: [ModuleKind; 6] = [Self::Base, Self::Keys, Self::Faders, Self::Knobs, Self::Rollers, Self::Dial];

    pub fn from_product_id(pid: u16) -> Option<Self> {
        Some(match pid {
            0x01F6 => Self::Base,
            0x01FE => Self::Keys,
            0x0200 => Self::Faders,
            0x0202 => Self::Knobs,
            0x0204 => Self::Rollers,
            0x0206 => Self::Dial,
            _ => return None,
        })
    }

    pub fn product_id(self) -> u16 {
        match self {
            Self::Base => 0x01F6,
            Self::Keys => 0x01FE,
            Self::Faders => 0x0200,
            Self::Knobs => 0x0202,
            Self::Rollers => 0x0204,
            Self::Dial => 0x0206,
        }
    }

    /// Stable lowercase name used in configs and APIs.
    pub fn name(self) -> &'static str {
        match self {
            Self::Base => "base",
            Self::Keys => "keys",
            Self::Faders => "faders",
            Self::Knobs => "knobs",
            Self::Rollers => "rollers",
            Self::Dial => "dial",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.name() == name)
    }

    /// Footprint on the base grid as (columns, rows); the base itself is 4x6.
    pub fn footprint(self) -> (u8, u8) {
        match self {
            Self::Base => (4, 6),
            Self::Keys => (4, 3),
            Self::Faders => (4, 2),
            Self::Knobs => (2, 2),
            Self::Rollers => (4, 1),
            Self::Dial => (2, 2),
        }
    }

    /// Number of physical controls (keys, faders, knobs, rollers, dials).
    pub fn control_count(self) -> usize {
        match self {
            Self::Base => 0,
            Self::Keys => 15,
            Self::Faders => 5,
            Self::Knobs => 3,
            Self::Rollers => 2,
            Self::Dial => 1,
        }
    }

    /// Screen size in pixels, if the module has one.
    pub fn screen_size(self) -> Option<(u32, u32)> {
        match self {
            Self::Keys => Some((480, 272)),
            Self::Dial => Some((240, 240)),
            _ => None,
        }
    }

    /// Keys and dial only accept commands and report input in software mode.
    pub fn needs_software_mode(self) -> bool {
        matches!(self, Self::Keys | Self::Dial)
    }

    /// The M0 family (base, faders, knobs, rollers) shares one command set.
    pub fn is_m0(self) -> bool {
        matches!(self, Self::Base | Self::Faders | Self::Knobs | Self::Rollers)
    }
}

/// Key grid of the 15-key module.
pub const KEY_COLUMNS: u8 = 5;
pub const KEY_ROWS: u8 = 3;
pub const KEY_PX: u32 = 72;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_ids_round_trip() {
        for kind in ModuleKind::ALL {
            assert_eq!(ModuleKind::from_product_id(kind.product_id()), Some(kind));
            assert_eq!(ModuleKind::from_name(kind.name()), Some(kind));
        }
        assert_eq!(ModuleKind::from_product_id(0x1234), None);
    }

    #[test]
    fn modules_fill_the_base_in_both_layouts() {
        let area = |k: ModuleKind| {
            let (c, r) = k.footprint();
            c as u32 * r as u32
        };
        let base = area(ModuleKind::Base);
        let layout_a = [ModuleKind::Keys, ModuleKind::Dial, ModuleKind::Knobs, ModuleKind::Rollers];
        let layout_b = [ModuleKind::Keys, ModuleKind::Faders, ModuleKind::Dial];
        assert_eq!(layout_a.iter().map(|k| area(*k)).sum::<u32>(), base);
        assert_eq!(layout_b.iter().map(|k| area(*k)).sum::<u32>(), base);
    }
}
