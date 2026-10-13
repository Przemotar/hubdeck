//! Where a module sits: its USB port path. The base is a tree of USB hubs, so the slot a module
//! is attached to (and which way round it is) shows up as the chain of hub port numbers.

use std::ffi::CStr;

/// The base's own controller sits three hub levels below the top of the base's hub tree.
const BASE_DEPTH: usize = 3;

/// Port numbers from the root hub down to the device, e.g. `[6, 1, 4, 3]`; `None` when the
/// platform does not expose it (not implemented yet on macOS and Linux).
pub fn usb_port_path(hid_path: &CStr) -> Option<Vec<u8>> {
    platform::usb_port_path(hid_path)
}

/// Slot of a module inside its base as dash-separated port numbers relative to the top of the
/// base's hub tree, e.g. `"1-4-3"`. Returns `None` if the module is not under that base.
pub fn slot_in_base(module_path: &[u8], base_path: &[u8]) -> Option<String> {
    let root = base_path.get(..base_path.len().checked_sub(BASE_DEPTH)?)?;
    let rest = module_path.strip_prefix(root)?;
    (!rest.is_empty()).then(|| rest.iter().map(|p| p.to_string()).collect::<Vec<_>>().join("-"))
}

/// `PCIROOT(0)#PCI(0102)#USBROOT(0)#USB(6)#USB(1)#USB(4)#USB(3)` -> `[6, 1, 4, 3]`.
#[cfg_attr(not(windows), allow(dead_code))]
fn parse_location_path(s: &str) -> Vec<u8> {
    s.split('#').filter_map(|part| part.strip_prefix("USB(")?.strip_suffix(')')?.parse().ok()).collect()
}

#[cfg(windows)]
mod platform {
    use std::ffi::CStr;

    use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{CM_Get_DevNode_PropertyW, CM_Get_Parent, CM_LOCATE_DEVNODE_NORMAL, CM_Locate_DevNodeW, CR_SUCCESS};
    use windows_sys::Win32::Devices::Properties::DEVPKEY_Device_LocationPaths;

    /// `\\?\HID#VID_2516&PID_01FE#9&3620a662&0&0000#{4d1e55b2-...}` -> `HID\VID_2516&PID_01FE\9&3620a662&0&0000`
    fn instance_id(hid_path: &str) -> Option<String> {
        let trimmed = hid_path.strip_prefix(r"\\?\")?;
        let without_guid = trimmed.rsplit_once("#{").map_or(trimmed, |(head, _)| head);
        Some(without_guid.replace('#', r"\"))
    }

    pub fn usb_port_path(hid_path: &CStr) -> Option<Vec<u8>> {
        let id = instance_id(hid_path.to_str().ok()?)?;
        let wide: Vec<u16> = id.encode_utf16().chain(std::iter::once(0)).collect();
        let mut hid_node = 0u32;
        let mut usb_node = 0u32;
        // SAFETY: plain CfgMgr32 calls with valid, NUL-terminated input and owned out-params.
        unsafe {
            if CM_Locate_DevNodeW(&mut hid_node, wide.as_ptr(), CM_LOCATE_DEVNODE_NORMAL) != CR_SUCCESS {
                return None;
            }
            // the HID node has no location; its parent (the USB device) does
            if CM_Get_Parent(&mut usb_node, hid_node, 0) != CR_SUCCESS {
                return None;
            }
            let mut prop_type = 0u32;
            let mut size = 0u32;
            CM_Get_DevNode_PropertyW(usb_node, &DEVPKEY_Device_LocationPaths, &mut prop_type, std::ptr::null_mut(), &mut size, 0);
            if size == 0 {
                return None;
            }
            let mut buf = vec![0u8; size as usize];
            if CM_Get_DevNode_PropertyW(usb_node, &DEVPKEY_Device_LocationPaths, &mut prop_type, buf.as_mut_ptr(), &mut size, 0) != CR_SUCCESS {
                return None;
            }
            // REG_MULTI_SZ in UTF-16; the first string is the PCI path
            let wide: Vec<u16> = buf.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect();
            let first = wide.split(|c| *c == 0).next()?;
            let path = super::parse_location_path(&String::from_utf16_lossy(first));
            (!path.is_empty()).then_some(path)
        }
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn instance_id_from_hid_path() {
            let p = r"\\?\HID#VID_2516&PID_01FE#9&3620a662&0&0000#{4d1e55b2-f16f-11cf-88cb-001111000030}";
            assert_eq!(super::instance_id(p).as_deref(), Some(r"HID\VID_2516&PID_01FE\9&3620a662&0&0000"));
        }
    }
}

/// macOS `locationID`: the top byte is the bus, then one nibble per port from the root down,
/// ending at the first zero nibble. `0x00114300` -> `[1, 1, 4, 3]`.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn parse_location_id(id: u32) -> Vec<u8> {
    (0..6).map(|i| ((id >> (20 - 4 * i)) & 0xF) as u8).take_while(|&p| p != 0).collect()
}

#[cfg(target_os = "macos")]
mod platform {
    use std::ffi::{CStr, c_char, c_void};

    use core_foundation_sys::base::{CFRelease, CFTypeRef, kCFAllocatorDefault};
    use core_foundation_sys::dictionary::CFMutableDictionaryRef;
    use core_foundation_sys::number::{CFNumberGetValue, CFNumberRef, kCFNumberSInt64Type};
    use core_foundation_sys::string::{CFStringCreateWithCString, CFStringRef, kCFStringEncodingUTF8};

    type IoObject = u32;

    #[link(name = "IOKit", kind = "framework")]
    unsafe extern "C" {
        fn IORegistryEntryIDMatching(entry_id: u64) -> CFMutableDictionaryRef;
        fn IOServiceGetMatchingService(main_port: u32, matching: CFMutableDictionaryRef) -> IoObject;
        fn IORegistryEntryGetParentEntry(entry: IoObject, plane: *const c_char, parent: *mut IoObject) -> i32;
        fn IORegistryEntryCreateCFProperty(entry: IoObject, key: CFStringRef, allocator: *const c_void, options: u32) -> CFTypeRef;
        fn IOObjectRelease(object: IoObject) -> i32;
    }

    /// hidapi's macOS path is `DevSrvsID:<registry entry id>`; walk up from the HID service to
    /// the first entry that carries `locationID` (the USB device or its interface).
    pub fn usb_port_path(hid_path: &CStr) -> Option<Vec<u8>> {
        let entry_id: u64 = hid_path.to_str().ok()?.strip_prefix("DevSrvsID:")?.parse().ok()?;
        // SAFETY: IOKit/CoreFoundation calls with owned objects; every object we get is released.
        unsafe {
            let key = CFStringCreateWithCString(kCFAllocatorDefault, c"locationID".as_ptr(), kCFStringEncodingUTF8);
            if key.is_null() {
                return None;
            }
            // IOServiceGetMatchingService consumes the matching dictionary
            let mut entry = IOServiceGetMatchingService(0, IORegistryEntryIDMatching(entry_id));
            let mut found = None;
            for _ in 0..8 {
                if entry == 0 {
                    break;
                }
                let value = IORegistryEntryCreateCFProperty(entry, key, kCFAllocatorDefault, 0);
                if !value.is_null() {
                    let mut id = 0i64;
                    if CFNumberGetValue(value as CFNumberRef, kCFNumberSInt64Type, &mut id as *mut i64 as *mut c_void) {
                        found = Some(id as u32);
                    }
                    CFRelease(value);
                    if found.is_some() {
                        break;
                    }
                }
                let mut parent = 0;
                let ok = IORegistryEntryGetParentEntry(entry, c"IOService".as_ptr(), &mut parent) == 0;
                IOObjectRelease(entry);
                entry = if ok { parent } else { 0 };
            }
            if entry != 0 {
                IOObjectRelease(entry);
            }
            CFRelease(key as CFTypeRef);
            let path = super::parse_location_id(found?);
            (!path.is_empty()).then_some(path)
        }
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod platform {
    use std::ffi::CStr;

    pub fn usb_port_path(_hid_path: &CStr) -> Option<Vec<u8>> {
        None // TODO: Linux sysfs devpath
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_windows_location_paths() {
        assert_eq!(parse_location_path("PCIROOT(0)#PCI(0102)#PCI(0000)#USBROOT(0)#USB(6)#USB(1)#USB(4)#USB(3)"), vec![6, 1, 4, 3]);
        assert_eq!(parse_location_path("ACPI(_SB_)#ACPI(PCI0)"), Vec::<u8>::new());
    }

    #[test]
    fn parses_macos_location_ids_measured_2026_10_13() {
        assert_eq!(parse_location_id(0x0011_4300), vec![1, 1, 4, 3]); // knobs
        assert_eq!(parse_location_id(0x0011_4400), vec![1, 1, 4, 4]); // base
        assert_eq!(parse_location_id(0x0012_3000), vec![1, 2, 3]); // keys
        assert_eq!(parse_location_id(0x0100_0000), Vec::<u8>::new());
        assert_eq!(slot_in_base(&parse_location_id(0x0011_3400), &parse_location_id(0x0011_4400)).as_deref(), Some("1-3-4")); // dial
    }

    #[test]
    fn slots_relative_to_the_base_measured_2026_10_09() {
        let base = [6, 1, 4, 4];
        assert_eq!(slot_in_base(&[6, 1, 4, 3], &base).as_deref(), Some("1-4-3")); // knobs
        assert_eq!(slot_in_base(&[6, 1, 3, 4], &base).as_deref(), Some("1-3-4")); // dial
        assert_eq!(slot_in_base(&[6, 2, 3], &base).as_deref(), Some("2-3")); // keys
        assert_eq!(slot_in_base(&[6, 3, 3], &base).as_deref(), Some("3-3")); // rollers
        assert_eq!(slot_in_base(&[7, 1], &base), None, "other USB branch");
        assert_eq!(slot_in_base(&[6, 1], &[4, 4]), None, "base path too short");
    }
}
