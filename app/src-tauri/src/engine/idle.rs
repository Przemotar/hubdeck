//! How long the computer has had no keyboard or mouse input, for turning the deck's screens off.

use std::time::Duration;

#[cfg(windows)]
pub fn system_idle() -> Option<Duration> {
    use windows::Win32::System::SystemInformation::GetTickCount;
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
    let mut info = LASTINPUTINFO { cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
    // SAFETY: plain Win32 calls with a correctly sized struct.
    unsafe {
        if !GetLastInputInfo(&mut info).as_bool() {
            return None;
        }
        Some(Duration::from_millis(GetTickCount().wrapping_sub(info.dwTime) as u64))
    }
}

#[cfg(target_os = "macos")]
pub fn system_idle() -> Option<Duration> {
    super::machw::idle_seconds().map(Duration::from_secs_f64)
}

#[cfg(target_os = "linux")]
pub fn system_idle() -> Option<Duration> {
    // logind keeps the last input as CLOCK_MONOTONIC microseconds; 0 means it has no hint
    let out =
        std::process::Command::new("busctl").args(["get-property", "org.freedesktop.login1", "/org/freedesktop/login1", "org.freedesktop.login1.Manager", "IdleSinceHintMonotonic"]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let since: u64 = text.split_whitespace().last()?.parse().ok()?;
    if since == 0 {
        return None;
    }
    let mut now = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: clock_gettime writes into the struct we own.
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut now) } != 0 {
        return None;
    }
    let now_us = now.tv_sec as u64 * 1_000_000 + now.tv_nsec as u64 / 1_000;
    Some(Duration::from_micros(now_us.saturating_sub(since)))
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
pub fn system_idle() -> Option<Duration> {
    None
}

/// Whether the session is locked, from Terminal Services (None = unknown).
#[cfg(windows)]
#[cfg_attr(not(windows), allow(dead_code))]
pub fn session_locked() -> Option<bool> {
    use windows::Win32::System::RemoteDesktop::{
        WTS_CURRENT_SERVER_HANDLE, WTS_CURRENT_SESSION, WTS_SESSIONSTATE_LOCK, WTS_SESSIONSTATE_UNLOCK, WTSFreeMemory, WTSINFOEXW, WTSQuerySessionInformationW, WTSSessionInfoEx,
    };
    use windows::core::PWSTR;
    let mut buf = PWSTR::null();
    let mut len = 0u32;
    // SAFETY: the buffer is allocated by the call, read only when large enough, and freed here.
    unsafe {
        WTSQuerySessionInformationW(Some(WTS_CURRENT_SERVER_HANDLE), WTS_CURRENT_SESSION, WTSSessionInfoEx, &mut buf, &mut len).ok()?;
        if buf.is_null() {
            return None;
        }
        let info = (len as usize >= std::mem::size_of::<WTSINFOEXW>()).then(|| *(buf.0 as *const WTSINFOEXW));
        WTSFreeMemory(buf.0 as _);
        let info = info.filter(|i| i.Level == 1)?;
        match info.Data.WTSInfoExLevel1.SessionFlags as u32 {
            WTS_SESSIONSTATE_LOCK => Some(true),
            WTS_SESSIONSTATE_UNLOCK => Some(false),
            _ => None,
        }
    }
}

/// Whether the screen is locked, from the window server's session (None = unknown).
#[cfg(target_os = "macos")]
pub fn session_locked() -> Option<bool> {
    super::machw::session_locked()
}

#[cfg(target_os = "linux")]
#[allow(dead_code)]
pub fn session_locked() -> Option<bool> {
    // logind knows the session's lock state; KDE and GNOME both set it
    let out = std::process::Command::new("loginctl").args(["show-session", "auto", "-p", "LockedHint"]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let value = text.trim().strip_prefix("LockedHint=")?;
    Some(value.eq_ignore_ascii_case("yes"))
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
#[allow(dead_code)]
pub fn session_locked() -> Option<bool> {
    None
}

/// The lock screen as the app in front: LockApp on Windows, loginwindow on macOS.
pub fn is_lock_screen(exe: &str) -> bool {
    let name = exe.trim().trim_end_matches(".app").trim_end_matches(".APP");
    ["LockApp.exe", "loginwindow", "ScreenSaverEngine"].iter().any(|e| name.eq_ignore_ascii_case(e))
}

/// Whether the screens should be off: locked (if that is wanted) or idle longer than `after`.
pub fn should_blank(locked: bool, blank_locked: bool, idle: Duration, after_minutes: u32) -> bool {
    (locked && blank_locked) || (after_minutes > 0 && idle >= Duration::from_secs(after_minutes as u64 * 60))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blanking_rules() {
        assert!(should_blank(true, true, Duration::ZERO, 0));
        assert!(!should_blank(true, false, Duration::ZERO, 0));
        assert!(!should_blank(false, true, Duration::from_secs(3600), 0), "0 minutes = never");
        assert!(should_blank(false, true, Duration::from_secs(600), 10));
        assert!(!should_blank(false, true, Duration::from_secs(599), 10));
        assert!(is_lock_screen("lockapp.exe"));
        assert!(is_lock_screen("loginwindow"), "the macOS lock screen");
        assert!(is_lock_screen("loginwindow.app"));
        assert!(!is_lock_screen("Finder"));
    }

    /// Reads the real lock state (run while unlocked: expects Some(false)).
    #[test]
    #[ignore]
    fn session_lock_state() {
        println!("session locked: {:?}", session_locked());
        #[cfg(windows)]
        assert_eq!(session_locked(), Some(false));
    }
}
