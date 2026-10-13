//! "A HID device came or went": lets the hub scan only when something changed. Windows: a
//! configuration-manager notification for the HID interface class. Elsewhere: none, so the hub
//! keeps polling.

use std::sync::atomic::{AtomicBool, Ordering};

static CHANGED: AtomicBool = AtomicBool::new(false);

/// True once since the last call if a HID device arrived or left.
pub fn changed() -> bool {
    CHANGED.swap(false, Ordering::SeqCst)
}

/// Start listening; false if this system cannot tell (the hub then polls every second).
pub fn watch() -> bool {
    platform::watch()
}

#[cfg(windows)]
mod platform {
    use std::sync::OnceLock;
    use std::sync::atomic::Ordering;

    use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
        CM_NOTIFY_ACTION, CM_NOTIFY_EVENT_DATA, CM_NOTIFY_FILTER, CM_NOTIFY_FILTER_TYPE_DEVICEINTERFACE, CM_Register_Notification, CR_SUCCESS, HCMNOTIFICATION,
    };
    use windows_sys::Win32::Devices::HumanInterfaceDevice::GUID_DEVINTERFACE_HID;

    /// The registration lives as long as the process (stored, never unregistered).
    static HANDLE: OnceLock<usize> = OnceLock::new();

    unsafe extern "system" fn on_change(_: HCMNOTIFICATION, _: *const core::ffi::c_void, _: CM_NOTIFY_ACTION, _: *const CM_NOTIFY_EVENT_DATA, _: u32) -> u32 {
        super::CHANGED.store(true, Ordering::SeqCst);
        0
    }

    pub fn watch() -> bool {
        if HANDLE.get().is_some() {
            return true;
        }
        let mut filter = CM_NOTIFY_FILTER { cbSize: std::mem::size_of::<CM_NOTIFY_FILTER>() as u32, FilterType: CM_NOTIFY_FILTER_TYPE_DEVICEINTERFACE, ..Default::default() };
        filter.u.DeviceInterface.ClassGuid = GUID_DEVINTERFACE_HID;
        let mut handle: HCMNOTIFICATION = std::ptr::null_mut();
        // SAFETY: the filter is fully initialised; the callback only flips an atomic flag.
        let result = unsafe { CM_Register_Notification(&filter, std::ptr::null(), Some(on_change), &mut handle) };
        if result != CR_SUCCESS {
            log::warn!("device notifications unavailable ({result}); scanning every second");
            return false;
        }
        let _ = HANDLE.set(handle as usize);
        true
    }
}

#[cfg(not(windows))]
mod platform {
    pub fn watch() -> bool {
        // TODO(macOS): IOKit matching notifications; enumeration is cheap there, so polling is fine
        false
    }
}
