//! Bringing back a module whose firmware stopped taking commands, the way pulling it off the base
//! and back does. macOS: the USB device is re-enumerated through IOUSBHost (no admin rights).
//! Windows needs an administrator for the same (`pnputil /restart-device`), so it is not done here.

use std::ffi::CStr;

/// Re-enumerate the USB device behind a hidapi path. The module disconnects and attaches again
/// with a new path within a few seconds; the hub picks it up as a new connection.
pub fn reenumerate(hid_path: &CStr) -> Result<(), String> {
    platform::reenumerate(hid_path)
}

#[cfg(target_os = "macos")]
mod platform {
    use std::ffi::CStr;

    unsafe extern "C" {
        fn mh_usb_reenumerate(hid_registry_id: u64) -> i32;
    }

    pub fn reenumerate(hid_path: &CStr) -> Result<(), String> {
        // hidapi's macOS path is `DevSrvsID:<registry entry id>` of the HID service
        let id: u64 = hid_path.to_str().ok().and_then(|p| p.strip_prefix("DevSrvsID:")).and_then(|n| n.parse().ok()).ok_or_else(|| format!("not a macOS HID path: {hid_path:?}"))?;
        // SAFETY: a plain C function taking a number; it owns and releases its IOKit objects.
        match unsafe { mh_usb_reenumerate(id) } {
            0 => Ok(()),
            1 => Err("the HID service is gone".into()),
            2 => Err("no USB device above the HID service".into()),
            3 => Err("the USB device could not be opened".into()),
            _ => Err("the USB reset failed".into()),
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use std::ffi::CStr;

    pub fn reenumerate(_hid_path: &CStr) -> Result<(), String> {
        Err("re-enumerating a module is done on macOS only".into())
    }
}
