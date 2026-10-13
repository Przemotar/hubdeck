//! Report builders. Each function returns the bytes to send, starting with the report id.
//! The transport pads output reports to the module's output report length and feature
//! reports to [`FEATURE_LEN`].

use crate::module::ModuleKind;
use crate::safety::is_forbidden;

/// Feature reports are 31 bytes + report id on every module.
pub const FEATURE_LEN: usize = 0x20;
/// Bytes in front of the payload of an image chunk.
pub const CHUNK_HEADER_LEN: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Report {
    /// Sent with `hid_write` (report id 0x02).
    Output(Vec<u8>),
    /// Sent with `hid_send_feature_report` (report id 0x03).
    Feature(Vec<u8>),
}

impl Report {
    pub fn bytes(&self) -> &[u8] {
        match self {
            Report::Output(b) | Report::Feature(b) => b,
        }
    }

    pub fn is_forbidden(&self) -> bool {
        is_forbidden(self.bytes())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedMode {
    Off = 0,
    Static = 1,
    Breathing = 2,
    Strobe = 3,
    Spring = 4,
    Cycling = 5,
    Random = 6,
    Wave = 7,
    Water = 8,
    Rainbow = 9,
}

/// Which LED of a module: one control or all of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedTarget {
    All,
    Index(u8),
}

pub type Rgb = [u8; 3];

fn clamp_percent(v: u8) -> u8 {
    v.min(100)
}

/// M0 LED group byte and index byte; `None` for modules without LEDs.
fn m0_led_address(kind: ModuleKind, target: LedTarget) -> Option<(u8, u8)> {
    let index = match target {
        LedTarget::All => 0xFF,
        LedTarget::Index(i) => i,
    };
    match kind {
        ModuleKind::Base => Some((0, 0)),
        ModuleKind::Faders if index == 0xFF || index < 5 => Some((1, index)),
        ModuleKind::Knobs if index == 0xFF || index < 3 => Some((2, index)),
        _ => None,
    }
}

/// Keys and dial ignore everything until switched to software mode; `false` hands them back
/// to their firmware (idle animation). Other modules have no such mode.
pub fn software_mode(kind: ModuleKind, on: bool) -> Option<Report> {
    let command = match kind {
        ModuleKind::Keys => 0x3A,
        ModuleKind::Dial => 0x49,
        _ => return None,
    };
    Some(Report::Feature(vec![0x03, command, on as u8]))
}

/// `persist` stores the color in the module so it survives a power cycle.
pub fn led_color(kind: ModuleKind, target: LedTarget, rgb: Rgb, persist: bool) -> Option<Report> {
    let (group, index) = m0_led_address(kind, target)?;
    Some(Report::Output(vec![0x02, 0x2D, group, index, rgb[0], rgb[1], rgb[2], persist as u8]))
}

pub fn led_mode(kind: ModuleKind, target: LedTarget, mode: LedMode, persist: bool) -> Option<Report> {
    let (group, index) = m0_led_address(kind, target)?;
    Some(Report::Output(vec![0x02, 0x2C, group, index, mode as u8, persist as u8]))
}

pub fn led_brightness(kind: ModuleKind, target: LedTarget, percent: u8) -> Option<Report> {
    let (group, index) = m0_led_address(kind, target)?;
    Some(Report::Output(vec![0x02, 0x2E, group, index, clamp_percent(percent)]))
}

/// Speed of animated LED modes, 0..=100.
pub fn led_speed(kind: ModuleKind, percent: u8) -> Option<Report> {
    kind.is_m0().then(|| Report::Output(vec![0x02, 0x30, clamp_percent(percent)]))
}

/// Screen backlight of keys or dial, 0..=100.
pub fn backlight(kind: ModuleKind, percent: u8) -> Option<Report> {
    let command = match kind {
        ModuleKind::Keys => 0x34,
        ModuleKind::Dial => 0x44,
        _ => return None,
    };
    Some(Report::Feature(vec![0x03, command, clamp_percent(percent)]))
}

/// Fill the whole screen of keys or dial with one color.
pub fn fill_screen(kind: ModuleKind, rgb: Rgb) -> Option<Report> {
    let command = match kind {
        ModuleKind::Keys => 0x36,
        ModuleKind::Dial => 0x46,
        _ => return None,
    };
    Some(Report::Feature(vec![0x03, command, rgb[0], rgb[1], rgb[2]]))
}

/// Fill one key (0..15) with a color.
pub fn fill_key(index: u8, rgb: Rgb) -> Option<Report> {
    (index < 15).then(|| Report::Feature(vec![0x03, 0x37, index, rgb[0], rgb[1], rgb[2]]))
}

/// Image transfer: `02 cmd chunk(LE16) len(LE16) index last` followed by the data.
/// `output_len` is the module's output report length including the report id
/// (320 on keys, read it from the HID descriptor).
fn image_chunks(command: u8, index: u8, jpeg: &[u8], output_len: usize) -> Vec<Report> {
    let chunk = output_len.saturating_sub(CHUNK_HEADER_LEN).max(1);
    let count = jpeg.len().div_ceil(chunk);
    jpeg.chunks(chunk)
        .enumerate()
        .map(|(i, data)| {
            let mut r = Vec::with_capacity(CHUNK_HEADER_LEN + data.len());
            r.extend_from_slice(&[0x02, command]);
            r.extend_from_slice(&(i as u16).to_le_bytes());
            r.extend_from_slice(&(data.len() as u16).to_le_bytes());
            r.push(index);
            r.push((i + 1 == count) as u8);
            r.extend_from_slice(data);
            Report::Output(r)
        })
        .collect()
}

/// JPEG for one key (72x72 px recommended). Key index 0..15, row by row from the top left.
pub fn key_image(index: u8, jpeg: &[u8], output_len: usize) -> Vec<Report> {
    if index >= 15 {
        return Vec::new();
    }
    image_chunks(0x36, index, jpeg, output_len)
}

/// One 480x272 JPEG across the whole 15-key panel.
pub fn panel_image(jpeg: &[u8], output_len: usize) -> Vec<Report> {
    image_chunks(0x35, 0, jpeg, output_len)
}

/// One 240x240 JPEG on the dial screen.
pub fn dial_image(jpeg: &[u8], output_len: usize) -> Vec<Report> {
    image_chunks(0x45, 0, jpeg, output_len)
}

/// Feature reports that can be read (`hid_get_feature_report`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Query {
    Firmware,
    SerialNumber,
    /// Faders only: the five values 0..=100.
    FaderValues,
    /// Keys and dial: backlight 0..=100.
    Backlight,
    /// Keys and dial: screen geometry.
    DeviceInfo,
    /// Base only: orientation from its sensor.
    Orientation,
}

/// Report id for a query on a module kind, `None` if the module does not support it.
pub fn query_id(kind: ModuleKind, query: Query) -> Option<u8> {
    use ModuleKind::*;
    Some(match (kind, query) {
        (Keys, Query::Firmware) => 0x38,
        (Dial, Query::Firmware) => 0x48,
        (_, Query::Firmware) => 0x2E,
        (Keys, Query::SerialNumber) => 0x3A,
        (Dial, Query::SerialNumber) => 0x4A,
        (_, Query::SerialNumber) => 0x30,
        (Faders, Query::FaderValues) => 0x33,
        (Keys, Query::Backlight) => 0x3C,
        (Dial, Query::Backlight) => 0x4C,
        (Keys, Query::DeviceInfo) => 0x3B,
        (Dial, Query::DeviceInfo) => 0x4B,
        (Base, Query::Orientation) => 0x35,
        _ => return None,
    })
}

/// Payload of a feature report answer: `[id, len, payload...]` -> `payload`.
pub fn feature_payload(answer: &[u8]) -> &[u8] {
    if answer.len() < 2 {
        return &[];
    }
    let end = (2 + answer[1] as usize).min(answer.len());
    &answer[2..end]
}

fn ascii(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).trim_matches('\0').trim().to_string()
}

/// Firmware answer: 4 unknown bytes, then an ASCII version like `02.00.01.00`.
pub fn parse_firmware(answer: &[u8]) -> Option<String> {
    let p = feature_payload(answer);
    (p.len() > 4).then(|| ascii(&p[4..])).filter(|s| !s.is_empty())
}

pub fn parse_serial_number(answer: &[u8]) -> Option<String> {
    Some(ascii(feature_payload(answer))).filter(|s| !s.is_empty())
}

pub fn parse_fader_values(answer: &[u8]) -> Option<[u8; 5]> {
    feature_payload(answer).get(..5)?.try_into().ok()
}

pub fn parse_percent(answer: &[u8]) -> Option<u8> {
    feature_payload(answer).first().copied()
}

/// Base orientation answer: `35 00 vv` (the length byte is 0, the value sits right after it).
pub fn parse_orientation(answer: &[u8]) -> Option<crate::input::Orientation> {
    (answer.first() == Some(&0x35)).then(|| answer.get(2).copied()).flatten().map(crate::input::Orientation::from_byte)
}

/// Screen geometry from the device-info answer: (width, height).
/// Keys: `14 03 05 0f e0 01 10 01 01 48 48` -> 480x272; dial: `14 00 00 00 f0 00 f0 00 ...` -> 240x240.
pub fn parse_screen_size(answer: &[u8]) -> Option<(u32, u32)> {
    let p = feature_payload(answer);
    let w = u16::from_le_bytes([*p.get(4)?, *p.get(5)?]) as u32;
    let h = u16::from_le_bytes([*p.get(6)?, *p.get(7)?]) as u32;
    (w > 0 && h > 0).then_some((w, h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn software_mode_bytes() {
        assert_eq!(software_mode(ModuleKind::Keys, true), Some(Report::Feature(vec![0x03, 0x3A, 1])));
        assert_eq!(software_mode(ModuleKind::Dial, false), Some(Report::Feature(vec![0x03, 0x49, 0])));
        assert_eq!(software_mode(ModuleKind::Faders, true), None);
    }

    #[test]
    fn led_addresses() {
        let r = led_color(ModuleKind::Faders, LedTarget::Index(2), [1, 2, 3], false).unwrap();
        assert_eq!(r.bytes(), &[0x02, 0x2D, 1, 2, 1, 2, 3, 0]);
        let r = led_mode(ModuleKind::Base, LedTarget::Index(7), LedMode::Static, false).unwrap();
        assert_eq!(r.bytes(), &[0x02, 0x2C, 0, 0, 1, 0], "base has a single edge LED, index forced to 0");
        assert!(led_color(ModuleKind::Knobs, LedTarget::Index(3), [0; 3], false).is_none());
        assert!(led_color(ModuleKind::Rollers, LedTarget::All, [0; 3], false).is_none());
    }

    #[test]
    fn image_chunks_match_the_prototype() {
        let jpeg: Vec<u8> = (0..700u32).map(|i| i as u8).collect();
        let chunks = key_image(4, &jpeg, 320);
        assert_eq!(chunks.len(), 3);
        let first = chunks[0].bytes();
        assert_eq!(&first[..8], &[0x02, 0x36, 0, 0, 0x38, 0x01, 4, 0]);
        assert_eq!(first.len(), 320);
        let last = chunks[2].bytes();
        assert_eq!(&last[..8], &[0x02, 0x36, 2, 0, 76, 0, 4, 1]);
        let joined: Vec<u8> = chunks.iter().flat_map(|c| c.bytes()[8..].to_vec()).collect();
        assert_eq!(joined, jpeg);
    }

    #[test]
    fn single_chunk_is_last() {
        let chunks = dial_image(&[0xFF, 0xD8, 0xFF, 0xD9], 320);
        assert_eq!(chunks.len(), 1);
        assert_eq!(&chunks[0].bytes()[..8], &[0x02, 0x45, 0, 0, 4, 0, 0, 1]);
    }

    #[test]
    fn parses_answers_captured_on_hardware() {
        let fw = [0x38, 0x0f, 0x29, 0x63, 0x1c, 0x5d, b'0', b'2', b'.', b'0', b'0', b'.', b'0', b'1', b'.', b'0', b'0', 0, 0];
        assert_eq!(parse_firmware(&fw).as_deref(), Some("02.00.01.00"));
        let mut sn = vec![0x3a, 0x14];
        sn.extend_from_slice(b"MHKD15AA431251603100");
        sn.extend_from_slice(&[0; 8]);
        assert_eq!(parse_serial_number(&sn).as_deref(), Some("MHKD15AA431251603100"));
        let keys_info = [0x3b, 0x0b, 0x14, 0x03, 0x05, 0x0f, 0xe0, 0x01, 0x10, 0x01, 0x01, 0x48, 0x48];
        assert_eq!(parse_screen_size(&keys_info), Some((480, 272)));
        let dial_info = [0x4b, 0x0b, 0x14, 0, 0, 0, 0xf0, 0, 0xf0, 0, 0x01, 0xf0, 0xf0];
        assert_eq!(parse_screen_size(&dial_info), Some((240, 240)));
        assert_eq!(parse_fader_values(&[0x33, 0x05, 30, 42, 41, 49, 84]), Some([30, 42, 41, 49, 84]));
        assert_eq!(parse_firmware(&[]), None);
        assert_eq!(parse_orientation(&[0x35, 0x00, 0x01, 0]), Some(crate::input::Orientation::Horizontal));
        assert_eq!(parse_orientation(&[0x35, 0x00, 0x00, 0]), Some(crate::input::Orientation::Vertical));
    }
}
