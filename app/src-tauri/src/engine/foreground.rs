//! Which app is in front. Windows: a WinEvent hook reports foreground changes as they happen,
//! and a 250 ms check catches the ones the hook misses (Chrome sometimes activates without the
//! event); the same check reports the session being locked or unlocked. Elsewhere: polling every
//! 250 ms until a native notification is wired in.

use crossbeam_channel::Sender;

use super::Msg;

pub fn watch(tx: Sender<Msg>) {
    platform::watch(tx);
}

#[cfg(windows)]
mod platform {
    use std::sync::{Mutex, OnceLock};

    use crossbeam_channel::Sender;
    use windows::Win32::Foundation::{CloseHandle, HWND};
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW};
    use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook};
    use windows::Win32::UI::WindowsAndMessaging::{DispatchMessageW, EVENT_SYSTEM_FOREGROUND, GetForegroundWindow, GetMessageW, GetWindowTextW, GetWindowThreadProcessId, MSG, WINEVENT_OUTOFCONTEXT};
    use windows::core::PWSTR;

    use super::super::Msg;

    static SENDER: OnceLock<Sender<Msg>> = OnceLock::new();
    static LAST: Mutex<String> = Mutex::new(String::new());

    /// (process id, exe name, full path) of a window's program.
    fn exe_of_window(hwnd: HWND) -> Option<(u32, String, String)> {
        // SAFETY: plain Win32 queries; the process handle is closed before returning.
        unsafe {
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == 0 {
                return None;
            }
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut buf = [0u16; 1024];
            let mut len = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len).is_ok();
            let _ = CloseHandle(handle);
            if !ok {
                return None;
            }
            let full = String::from_utf16_lossy(&buf[..len as usize]);
            Some((pid, full.rsplit(['\\', '/']).next().unwrap_or(&full).to_string(), full.clone()))
        }
    }

    fn title_of(hwnd: HWND) -> String {
        let mut buf = [0u16; 512];
        // SAFETY: the buffer outlives the call; a closed window just gives an empty title.
        let n = unsafe { GetWindowTextW(hwnd, &mut buf) };
        String::from_utf16_lossy(&buf[..n.max(0) as usize])
    }

    fn report(hwnd: HWND) {
        let Some((pid, exe, path)) = exe_of_window(hwnd) else { return };
        let title = title_of(hwnd);
        let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
        if pid == std::process::id() {
            // using the settings window must not switch away from the profile being edited; but
            // forget the last app, so coming back to it re-applies its profile (the user may have
            // picked another one meanwhile)
            last.clear();
            return;
        }
        let key = format!("{exe}\t{title}");
        if *last != key {
            *last = key;
            super::super::apps::seen(&exe, &path, &title);
            if let Some(tx) = SENDER.get() {
                let _ = tx.send(Msg::Foreground { exe, title });
            }
        }
    }

    unsafe extern "system" fn on_event(_: HWINEVENTHOOK, _event: u32, hwnd: HWND, _: i32, _: i32, _: u32, _: u32) {
        report(hwnd);
    }

    pub fn watch(tx: Sender<Msg>) {
        let _ = SENDER.set(tx);
        // SAFETY: the hook and the message loop live on this thread for the life of the app.
        std::thread::Builder::new()
            .name("foreground-check".into())
            .spawn(|| {
                let mut locked = None;
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(250));
                    let now = super::super::idle::session_locked();
                    if now.is_some() && now != locked {
                        locked = now;
                        if now == Some(false) {
                            // report the app in front again: it may be the one from before the lock
                            LAST.lock().unwrap_or_else(|e| e.into_inner()).clear();
                        }
                        if let (Some(l), Some(tx)) = (now, SENDER.get()) {
                            let _ = tx.send(Msg::Locked(l));
                        }
                    }
                    // SAFETY: GetForegroundWindow has no preconditions.
                    report(unsafe { GetForegroundWindow() });
                }
            })
            .expect("spawn foreground check");
        unsafe {
            report(GetForegroundWindow());
            let hook = SetWinEventHook(EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND, None, Some(on_event), 0, 0, WINEVENT_OUTOFCONTEXT);
            if hook.is_invalid() {
                log::warn!("foreground hook unavailable; profiles will not follow the app in front");
                return;
            }
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                DispatchMessageW(&msg);
            }
        }
    }
}

#[cfg(not(windows))]
mod platform {
    use std::time::Duration;

    use crossbeam_channel::Sender;

    use super::super::Msg;

    pub fn watch(tx: Sender<Msg>) {
        let own = std::process::id() as u64;
        let mut last = String::new();
        loop {
            std::thread::sleep(Duration::from_millis(250));
            let Ok(win) = active_win_pos_rs::get_active_window() else { continue };
            if win.process_id == own {
                continue;
            }
            let exe = win.process_path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or(win.app_name);
            let key = format!("{exe}\t{}", win.title);
            if key != last {
                last = key;
                super::super::apps::seen(&exe, &win.process_path.to_string_lossy(), &win.title);
                if tx.send(Msg::Foreground { exe, title: win.title }).is_err() {
                    return;
                }
            }
        }
    }
}
