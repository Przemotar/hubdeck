//! Minimal HID report-descriptor reading: only what the transport needs.

/// Length in bytes (report id included) of the output report with id `report_id`,
/// or `None` if the descriptor does not declare one.
pub fn output_report_len(descriptor: &[u8], report_id: u8) -> Option<usize> {
    let (mut current_id, mut count, mut size) = (0u32, 0u32, 0u32);
    let mut i = 0;
    while i < descriptor.len() {
        let prefix = descriptor[i];
        let n = [0usize, 1, 2, 4][(prefix & 0x03) as usize];
        let value = descriptor.get(i + 1..i + 1 + n)?.iter().rev().fold(0u32, |acc, b| (acc << 8) | *b as u32);
        match prefix & 0xFC {
            0x84 => current_id = value,                                                             // Report ID
            0x94 => count = value,                                                                  // Report Count
            0x74 => size = value,                                                                   // Report Size
            0x90 if current_id == report_id as u32 => return Some(1 + (count * size / 8) as usize), // Output
            _ => {}
        }
        i += 1 + n;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_and_m0_descriptors_captured_on_hardware() {
        // first items of the 15-key descriptor: input 0x01 (63 B), output 0x02 (319 B via a 2-byte count)
        let keys = [
            0x05, 0x0c, 0x09, 0x01, 0xa1, 0x01, 0x85, 0x01, 0x0a, 0x00, 0xff, 0x15, 0x00, 0x26, 0xff, 0x00, 0x75, 0x08, 0x95, 0x3f, 0x81, 0x02, 0x85, 0x02, 0x0a, 0x00, 0xff, 0x15, 0x00, 0x26, 0xff,
            0x00, 0x75, 0x08, 0x96, 0x3f, 0x01, 0x91, 0x02,
        ];
        assert_eq!(output_report_len(&keys, 0x02), Some(320));
        let m0 = [
            0x05, 0x0c, 0x09, 0x01, 0xa1, 0x01, 0x85, 0x01, 0x0a, 0x00, 0xff, 0x15, 0x00, 0x26, 0xff, 0x00, 0x75, 0x08, 0x95, 0x3f, 0x81, 0x02, 0x85, 0x02, 0x0a, 0x00, 0xff, 0x15, 0x00, 0x26, 0xff,
            0x00, 0x75, 0x08, 0x95, 0x3f, 0x91, 0x02,
        ];
        assert_eq!(output_report_len(&m0, 0x02), Some(64));
        assert_eq!(output_report_len(&m0, 0x07), None);
        assert_eq!(output_report_len(&[0x85], 0x02), None, "truncated descriptor");
    }
}
