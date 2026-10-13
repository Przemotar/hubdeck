//! Commands this crate never builds and the transport refuses to send.
//!
//! The firmware accepts reports that flash firmware, rewrite the serial number, save factory
//! data or reset the module. None of them has a builder in this crate; this list exists for the
//! raw-report path so a hand-built report cannot slip one through.

/// (report id, command byte)
const FORBIDDEN: &[(u8, u8)] = &[
    // M0 family (base, faders, knobs, rollers), output reports
    (0x02, 0x2A), // update firmware
    (0x02, 0x2B), // set serial number
    (0x02, 0x31), // save factory data
    (0x02, 0x49), // test firmware recovery
    (0x02, 0x4A), // test factory reset
    (0x02, 0x2F), // recalibrate the base's tilt sensor (Calibrate_Direction; no known way back, docs/DEVICE-AUDIT.md)
    // Keys (M4315)
    (0x02, 0x34), // update firmware
    (0x03, 0x35), // set serial number
    (0x03, 0x39), // test firmware recovery
    (0x03, 0x3C), // save factory data
    (0x03, 0x3D), // test factory reset
    (0x03, 0x3E), // test animation recovery
    (0x02, 0x38), // save screen picture to flash (not needed, wears the flash)
    // Dial (M131E); bytes read from the vendor SDK (CoolTEK_SDK_CM_M131E 03.02.00)
    (0x02, 0x44), // update firmware
    (0x03, 0x45), // set serial number
    (0x03, 0x48), // test firmware recovery
    (0x03, 0x4B), // save factory data
    (0x03, 0x4C), // test factory reset
    (0x03, 0x4D), // test animation recovery
    (0x02, 0x47), // save screen picture to flash
];

pub fn is_forbidden(report: &[u8]) -> bool {
    match report {
        [id, command, ..] => FORBIDDEN.contains(&(*id, *command)),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::ModuleKind;
    use crate::report::*;

    #[test]
    fn no_builder_produces_a_forbidden_report() {
        let mut all: Vec<Report> = Vec::new();
        for kind in ModuleKind::ALL {
            all.extend(software_mode(kind, true));
            all.extend(software_mode(kind, false));
            all.extend(backlight(kind, 50));
            all.extend(fill_screen(kind, [1, 2, 3]));
            all.extend(led_speed(kind, 50));
            for target in [LedTarget::All, LedTarget::Index(0)] {
                all.extend(led_color(kind, target, [1, 2, 3], true));
                all.extend(led_mode(kind, target, LedMode::Static, true));
                all.extend(led_brightness(kind, target, 50));
            }
        }
        all.extend(fill_key(0, [0; 3]));
        all.extend(key_image(0, &[0; 10], 320));
        all.extend(panel_image(&[0; 10], 320));
        all.extend(dial_image(&[0; 10], 64));
        for r in &all {
            assert!(!r.is_forbidden(), "builder produced a forbidden report: {:02x?}", &r.bytes()[..2]);
        }
    }

    #[test]
    fn refuses_firmware_and_serial_commands() {
        assert!(is_forbidden(&[0x02, 0x2A, 0, 0]));
        assert!(is_forbidden(&[0x02, 0x2F, 0]));
        assert!(is_forbidden(&[0x03, 0x35, b'M', b'H']));
        assert!(!is_forbidden(&[0x02, 0x36, 0, 0]));
        assert!(!is_forbidden(&[0x02]));
    }
}
