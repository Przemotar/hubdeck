//! Windows keeps key presses of a normal program from a program that runs as administrator (UIPI):
//! the keys of the deck then do nothing in that app and nobody knows why (pain point SD7). Before
//! sending keys HubDeck checks the app in front; the keys still go, and the window says why they
//! may not arrive.

/// Why keys cannot reach the app in front, or None when they can.
pub fn blocked_reason() -> Option<String> {
    platform::blocked()
        .map(|exe| format!("{exe} runs as administrator, so Windows keeps HubDeck's key presses from it. Start HubDeck as administrator (right-click > Run as administrator) to control it."))
}

#[cfg(windows)]
mod platform {
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation};
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW};
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
    use windows::core::PWSTR;

    /// Whether a process runs elevated; a token we may not even read belongs to one that does.
    fn elevated(process: HANDLE) -> bool {
        // SAFETY: the token handle is closed below; the struct is the size we pass.
        unsafe {
            let mut token = HANDLE::default();
            if OpenProcessToken(process, TOKEN_QUERY, &mut token).is_err() {
                return true;
            }
            let mut info = TOKEN_ELEVATION::default();
            let mut len = 0u32;
            let ok = GetTokenInformation(token, TokenElevation, Some(&mut info as *mut _ as *mut _), std::mem::size_of::<TOKEN_ELEVATION>() as u32, &mut len).is_ok();
            let _ = CloseHandle(token);
            ok && info.TokenIsElevated != 0
        }
    }

    /// The app in front when it runs elevated and HubDeck does not.
    pub fn blocked() -> Option<String> {
        // SAFETY: plain Win32 queries; the process handle is closed before returning.
        unsafe {
            if elevated(GetCurrentProcess()) {
                return None;
            }
            let mut pid = 0u32;
            GetWindowThreadProcessId(GetForegroundWindow(), Some(&mut pid));
            if pid == 0 {
                return None;
            }
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut buf = [0u16; 1024];
            let mut len = buf.len() as u32;
            let named = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len).is_ok();
            let up = elevated(process);
            let _ = CloseHandle(process);
            if !named || !up {
                return None;
            }
            let full = String::from_utf16_lossy(&buf[..len as usize]);
            Some(full.rsplit(['\\', '/']).next().unwrap_or(&full).to_string())
        }
    }
}

#[cfg(not(windows))]
mod platform {
    pub fn blocked() -> Option<String> {
        None
    }
}
