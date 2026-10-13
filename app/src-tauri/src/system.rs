//! Operating system hooks: sleep and resume, restarting a wedged module's USB device, and opening
//! things the way the system's Run box does.

use crate::deck::Deck;

/// Release the deck before the computer sleeps and take it again after it wakes up (the official
/// app did the same on power events).
pub fn watch_power(deck: Deck) {
    platform::watch_power(deck);
}

/// Restart the USB device of every attached module of `kind` (Windows: `pnputil /restart-device`,
/// elevated, so the user sees a UAC prompt). Blocks until it is done; call from a blocking thread.
pub fn restart_module(kind: mh_protocol::ModuleKind) -> Result<(), String> {
    platform::restart_module(kind)
}

/// Open a program, file, folder or URL like the Run box (Win+R) does: a bare name such as `notepad`
/// or `calc` is found on the PATH and in the registered App Paths, `ms-settings:` and other
/// protocols work too.
pub fn shell_open(target: &str) -> Result<(), String> {
    // a test copy must not open windows on the screen of the person working here
    if crate::automation::blocks_opening(target) {
        return Ok(());
    }
    platform::shell_open(target).map_err(|e| format!("cannot open \"{target}\": {e}"))
}

#[cfg(windows)]
mod platform {
    use std::ffi::c_void;

    use base64::Engine as _;
    use windows::Win32::Foundation::{CloseHandle, ERROR_SUCCESS, HANDLE};
    use windows::Win32::System::Power::{DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS, PowerRegisterSuspendResumeNotification};
    use windows::Win32::System::Threading::{GetExitCodeProcess, INFINITE, WaitForSingleObject};
    use windows::Win32::UI::Shell::{SEE_MASK_FLAG_NO_UI, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW};
    use windows::Win32::UI::WindowsAndMessaging::{DEVICE_NOTIFY_CALLBACK, PBT_APMRESUMEAUTOMATIC, PBT_APMRESUMESUSPEND, PBT_APMSUSPEND, SW_HIDE, SW_SHOWNORMAL};
    use windows::core::{HSTRING, PCWSTR, w};

    use crate::deck::Deck;

    unsafe extern "system" fn on_power(context: *const c_void, kind: u32, _setting: *const c_void) -> u32 {
        // SAFETY: `context` is the leaked `Deck` registered below; it lives for the whole process.
        let deck = unsafe { &*(context as *const Deck) };
        match kind {
            PBT_APMSUSPEND => deck.suspend(),
            PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMESUSPEND => deck.resume(),
            _ => {}
        }
        0
    }

    pub fn watch_power(deck: Deck) {
        // both live until the process exits, like the registration itself
        let context = Box::into_raw(Box::new(deck)) as *mut c_void;
        let params = Box::leak(Box::new(DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS { Callback: Some(on_power), Context: context }));
        let mut handle = std::ptr::null_mut();
        // SAFETY: `params` stays valid forever; Windows calls `on_power` on its own thread.
        let err = unsafe { PowerRegisterSuspendResumeNotification(DEVICE_NOTIFY_CALLBACK, HANDLE(params as *mut _ as *mut c_void), &mut handle) };
        if err != ERROR_SUCCESS {
            log::warn!("sleep/resume notifications unavailable ({err:?}); the deck is not released before sleep");
        }
    }

    pub fn shell_open(target: &str) -> Result<(), String> {
        let file = HSTRING::from(target);
        let mut info = SHELLEXECUTEINFOW {
            cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            // no error dialog from the shell: the error goes to the app's banner instead
            fMask: SEE_MASK_FLAG_NO_UI,
            lpFile: PCWSTR(file.as_ptr()),
            nShow: SW_SHOWNORMAL.0,
            ..Default::default()
        };
        // SAFETY: every pointer in `info` outlives the call; no process handle is asked for.
        unsafe { ShellExecuteExW(&mut info) }.map_err(|e| e.message())
    }

    pub fn restart_module(kind: mh_protocol::ModuleKind) -> Result<(), String> {
        // the instance id carries the USB serial from when the module was attached; a wedged
        // module's serial reads as garbage now, so find it by vendor and product id instead
        let script = format!(
            "$ids = @(Get-PnpDevice -PresentOnly | Where-Object {{ $_.InstanceId -like 'USB\\VID_{:04X}&PID_{:04X}*' }} | ForEach-Object InstanceId)\n\
             if ($ids.Count -eq 0) {{ exit 2 }}\n\
             foreach ($id in $ids) {{ pnputil /restart-device $id | Out-Null }}\n\
             exit 0",
            mh_protocol::VENDOR_ID,
            kind.product_id()
        );
        let utf16: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
        let params = HSTRING::from(format!("-NoProfile -NonInteractive -ExecutionPolicy Bypass -EncodedCommand {}", base64::engine::general_purpose::STANDARD.encode(utf16)));
        let mut info = SHELLEXECUTEINFOW {
            cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOCLOSEPROCESS,
            lpVerb: w!("runas"),
            lpFile: w!("powershell.exe"),
            lpParameters: PCWSTR(params.as_ptr()),
            nShow: SW_HIDE.0,
            ..Default::default()
        };
        // SAFETY: every pointer in `info` outlives the call; the process handle is closed below.
        unsafe {
            ShellExecuteExW(&mut info).map_err(|e| if e.code().0 as u32 == 0x800704C7 { "cancelled".to_string() } else { e.message() })?;
            if info.hProcess.is_invalid() {
                return Err("could not start the restart".into());
            }
            WaitForSingleObject(info.hProcess, INFINITE);
            let mut code = 0u32;
            let _ = GetExitCodeProcess(info.hProcess, &mut code);
            let _ = CloseHandle(info.hProcess);
            match code {
                0 => Ok(()),
                2 => Err(format!("no {} module found", kind.name())),
                c => Err(format!("restart failed (exit code {c})")),
            }
        }
    }
}

#[cfg(not(windows))]
mod platform {
    use crate::deck::Deck;

    pub fn watch_power(_deck: Deck) {
        // TODO(macOS): IORegisterForSystemPower; until then USB disconnects cover sleep
        log::info!("sleep/resume notifications are not wired up on this platform yet");
    }

    pub fn restart_module(_kind: mh_protocol::ModuleKind) -> Result<(), String> {
        Err("not available on this system: pull the module off the base and put it back".into())
    }

    pub fn shell_open(target: &str) -> Result<(), String> {
        if target.contains(':') && !std::path::Path::new(target).exists() {
            return tauri_plugin_opener::open_url(target, None::<&str>).map_err(|e| e.to_string());
        }
        // macOS: a bare name like "Safari" is an application
        #[cfg(target_os = "macos")]
        if !std::path::Path::new(target).exists() {
            let status = std::process::Command::new("open").arg("-a").arg(target).status().map_err(|e| e.to_string())?;
            return if status.success() { Ok(()) } else { Err("not found".into()) };
        }
        tauri_plugin_opener::open_path(target, None::<&str>).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn unknown_program_says_what_was_not_found() {
        let e = super::shell_open("hubdeck-no-such-program-42").unwrap_err();
        assert!(e.contains("hubdeck-no-such-program-42"), "{e}");
    }
}
