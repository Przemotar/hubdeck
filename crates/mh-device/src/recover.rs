//! Bringing back a module whose firmware stopped taking commands, the way pulling it off the base
//! and back does. macOS: the USB device is re-enumerated through IOUSBHost (no admin rights).
//! Windows needs an administrator for the same (`pnputil /restart-device`), so it is not done here.

use std::ffi::CStr;

/// Re-enumerate the USB device behind a hidapi path. The module disconnects and attaches again
/// with a new path within a few seconds; the hub picks it up as a new connection.
pub fn reenumerate(hid_path: &CStr) -> Result<(), String> {
    platform::reenumerate(hid_path)
}

/// Send the HID class request SET_IDLE(0) to the module behind a hidapi path. Windows' HID driver
/// sends it to every HID device at start; macOS does not, and the rollers module (firmware
/// 01.00.03.01) sends no input at all until it gets one (owner's deck, 2026-10-15). Elsewhere a no-op.
pub fn set_idle(hid_path: &CStr) -> Result<(), String> {
    platform::set_idle(hid_path)
}

#[cfg(target_os = "macos")]
mod platform {
    use std::ffi::CStr;

    unsafe extern "C" {
        fn mh_usb_reenumerate(hid_registry_id: u64) -> i32;
        fn mh_usb_set_idle(hid_registry_id: u64) -> i32;
    }

    /// hidapi's macOS path is `DevSrvsID:<registry entry id>` of the HID service.
    fn registry_id(hid_path: &CStr) -> Result<u64, String> {
        hid_path.to_str().ok().and_then(|p| p.strip_prefix("DevSrvsID:")).and_then(|n| n.parse().ok()).ok_or_else(|| format!("not a macOS HID path: {hid_path:?}"))
    }

    fn result(code: i32, failed: &str) -> Result<(), String> {
        match code {
            0 => Ok(()),
            1 => Err("the HID service is gone".into()),
            2 => Err("no USB device above the HID service".into()),
            3 => Err("the USB device could not be opened".into()),
            _ => Err(failed.into()),
        }
    }

    pub fn reenumerate(hid_path: &CStr) -> Result<(), String> {
        let id = registry_id(hid_path)?;
        // SAFETY: a plain C function taking a number; it owns and releases its IOKit objects.
        result(unsafe { mh_usb_reenumerate(id) }, "the USB reset failed")
    }

    pub fn set_idle(hid_path: &CStr) -> Result<(), String> {
        let id = registry_id(hid_path)?;
        // SAFETY: as above.
        result(unsafe { mh_usb_set_idle(id) }, "the SET_IDLE request failed")
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use std::ffi::CStr;

    pub fn reenumerate(_hid_path: &CStr) -> Result<(), String> {
        Err("re-enumerating a module is done on macOS only".into())
    }

    /// The system's HID driver already sends it.
    pub fn set_idle(_hid_path: &CStr) -> Result<(), String> {
        Ok(())
    }
}
