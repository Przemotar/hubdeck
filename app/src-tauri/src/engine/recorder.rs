//! Macro recorder: while recording, a low-level keyboard hook notes every key the user presses
//! (not the ones HubDeck itself sends), with the time; stopping turns them into multi-action steps:
//! one Hotkey step per key with its modifiers, each waiting as long as the user did. Windows only
//! for now; the hook runs on its own thread only while recording, and stops itself after two
//! minutes if nobody stops it.
#![cfg_attr(not(windows), allow(dead_code))]

use std::time::Duration;

use serde_json::{Value, json};

/// Recording stops by itself after this long.
pub const MAX_RECORDING: Duration = Duration::from_secs(120);
/// At most this many key events are kept.
const MAX_EVENTS: usize = 2000;

/// One key going down or up, `at` milliseconds after the recording started.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeyEvent {
    pub at: u64,
    pub vk: u32,
    pub down: bool,
}

fn modifier(vk: u32) -> Option<&'static str> {
    Some(match vk {
        0x11 | 0xA2 | 0xA3 => "Ctrl",
        0x10 | 0xA0 | 0xA1 => "Shift",
        0x12 | 0xA4 | 0xA5 => "Alt",
        0x5B | 0x5C => "Win",
        _ => return None,
    })
}

/// A Windows virtual-key code as HubDeck's key name (see `hotkey.rs`); None for keys it cannot send.
pub fn key_name(vk: u32) -> Option<String> {
    let s = match vk {
        0x30..=0x39 | 0x41..=0x5A => return char::from_u32(vk).map(|c| c.to_string()),
        0x60..=0x69 => return char::from_u32(vk - 0x60 + 0x30).map(|c| c.to_string()),
        0x70..=0x87 => return Some(format!("F{}", vk - 0x6F)),
        0x08 => "Backspace",
        0x09 => "Tab",
        0x0D => "Enter",
        0x1B => "Escape",
        0x20 => "Space",
        0x21 => "PageUp",
        0x22 => "PageDown",
        0x23 => "End",
        0x24 => "Home",
        0x25 => "Left",
        0x26 => "Up",
        0x27 => "Right",
        0x28 => "Down",
        0x2C => "PrintScreen",
        0x2E => "Delete",
        0xAD => "VolumeMute",
        0xAE => "VolumeDown",
        0xAF => "VolumeUp",
        0xB0 => "NextTrack",
        0xB1 => "PrevTrack",
        0xB3 => "PlayPause",
        0xBA => ";",
        0xBB => "=",
        0xBC => ",",
        0xBD => "-",
        0xBE => ".",
        0xBF => "/",
        0xC0 => "`",
        0xDB => "[",
        0xDC => "\\",
        0xDD => "]",
        0xDE => "'",
        _ => return None,
    };
    Some(s.to_string())
}

/// Recorded key events -> multi-action steps (`core.hotkey`, keys like `Ctrl+Shift+S`, delay =
/// the pause before the key, at most 10 s). Held keys repeating are one press; a modifier pressed
/// and released alone is a step of its own.
pub fn to_steps(events: &[KeyEvent]) -> Vec<Value> {
    let mut steps = Vec::new();
    let mut held: Vec<u32> = Vec::new();
    let mut mods: Vec<(&'static str, bool)> = Vec::new(); // (name, used with another key)
    let mut last_at: Option<u64> = None;
    let mut push = |keys: String, at: u64, steps: &mut Vec<Value>| {
        let delay = last_at.map(|l| at.saturating_sub(l)).unwrap_or(0).min(super::actions::MAX_STEP_DELAY_MS);
        last_at = Some(at);
        steps.push(json!({"action": "core.hotkey", "params": {"keys": keys}, "delay": delay}));
    };
    for e in events {
        match (modifier(e.vk), e.down) {
            (Some(name), true) => {
                if !mods.iter().any(|(n, _)| *n == name) {
                    mods.push((name, false));
                }
            }
            (Some(name), false) => {
                if let Some(i) = mods.iter().position(|(n, _)| *n == name) {
                    let (_, used) = mods.remove(i);
                    if !used {
                        push(name.to_string(), e.at, &mut steps);
                    }
                }
            }
            (None, true) => {
                if held.contains(&e.vk) {
                    continue; // auto-repeat
                }
                held.push(e.vk);
                let Some(key) = key_name(e.vk) else { continue };
                for m in mods.iter_mut() {
                    m.1 = true;
                }
                let mut combo: Vec<&str> = ["Ctrl", "Shift", "Alt", "Win"].into_iter().filter(|n| mods.iter().any(|(m, _)| m == n)).collect();
                combo.push(&key);
                push(combo.join("+"), e.at, &mut steps);
            }
            (None, false) => held.retain(|k| *k != e.vk),
        }
    }
    steps
}

#[cfg(windows)]
mod hook {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use std::time::Instant;

    use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, GetMessageW, KBDLLHOOKSTRUCT, LLKHF_INJECTED, MSG, PostThreadMessageW, SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_QUIT, WM_SYSKEYDOWN,
        WM_SYSKEYUP,
    };

    use super::{KeyEvent, MAX_EVENTS, MAX_RECORDING};

    static RECORDING: AtomicBool = AtomicBool::new(false);
    static THREAD: AtomicU32 = AtomicU32::new(0);
    static EVENTS: Mutex<Vec<KeyEvent>> = Mutex::new(Vec::new());
    static STARTED: Mutex<Option<Instant>> = Mutex::new(None);

    unsafe extern "system" fn on_key(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 && RECORDING.load(Ordering::Relaxed) {
            // SAFETY: for WH_KEYBOARD_LL, lparam points to a KBDLLHOOKSTRUCT for this call.
            let k = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
            let down = matches!(wparam.0 as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
            let up = matches!(wparam.0 as u32, WM_KEYUP | WM_SYSKEYUP);
            // keys HubDeck (or any program) types are not the user's
            if (down || up) && (k.flags.0 & LLKHF_INJECTED.0) == 0 {
                let at = STARTED.lock().map(|s| s.map(|t| t.elapsed().as_millis() as u64).unwrap_or(0)).unwrap_or(0);
                if let Ok(mut ev) = EVENTS.lock()
                    && ev.len() < MAX_EVENTS
                {
                    ev.push(KeyEvent { at, vk: k.vkCode, down });
                }
            }
        }
        // SAFETY: passing the event on, as every hook must.
        unsafe { CallNextHookEx(None, code, wparam, lparam) }
    }

    pub fn start() -> Result<(), String> {
        if RECORDING.swap(true, Ordering::SeqCst) {
            return Err("already recording".into());
        }
        EVENTS.lock().map_err(|e| e.to_string())?.clear();
        *STARTED.lock().map_err(|e| e.to_string())? = Some(Instant::now());
        std::thread::Builder::new()
            .name("key-recorder".into())
            .spawn(|| {
                // SAFETY: plain Win32 calls; the hook lives only on this thread, which pumps messages.
                unsafe {
                    let hook = match SetWindowsHookExW(WH_KEYBOARD_LL, Some(on_key), None, 0) {
                        Ok(h) => h,
                        Err(e) => {
                            log::warn!("key recorder: {e}");
                            RECORDING.store(false, Ordering::SeqCst);
                            return;
                        }
                    };
                    let id = GetCurrentThreadId();
                    THREAD.store(id, Ordering::SeqCst);
                    // stops by itself if nobody stops it
                    std::thread::spawn(move || {
                        std::thread::sleep(MAX_RECORDING);
                        if THREAD.load(Ordering::SeqCst) == id {
                            RECORDING.store(false, Ordering::SeqCst);
                            let _ = PostThreadMessageW(id, WM_QUIT, WPARAM(0), LPARAM(0));
                        }
                    });
                    let mut msg = MSG::default();
                    while GetMessageW(&mut msg, None, 0, 0).as_bool() {}
                    let _ = UnhookWindowsHookEx(hook);
                    THREAD.store(0, Ordering::SeqCst);
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn stop() -> Vec<KeyEvent> {
        RECORDING.store(false, Ordering::SeqCst);
        let id = THREAD.swap(0, Ordering::SeqCst);
        if id != 0 {
            // SAFETY: asks the recorder thread to leave its message loop and unhook.
            let _ = unsafe { PostThreadMessageW(id, WM_QUIT, WPARAM(0), LPARAM(0)) };
        }
        EVENTS.lock().map(|mut e| std::mem::take(&mut *e)).unwrap_or_default()
    }
}

/// Start recording the user's keys.
pub fn start() -> Result<(), String> {
    #[cfg(windows)]
    return hook::start();
    #[cfg(not(windows))]
    Err("recording keys works on Windows only for now".into())
}

/// Stop recording; the keys as multi-action steps.
pub fn stop() -> Vec<Value> {
    #[cfg(windows)]
    return to_steps(&hook::stop());
    #[cfg(not(windows))]
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(at: u64, vk: u32, down: bool) -> KeyEvent {
        KeyEvent { at, vk, down }
    }

    #[test]
    fn keys_become_hotkey_steps_with_their_pauses() {
        let events = [
            ev(0, 0xA2, true),   // Ctrl down
            ev(50, 0x53, true),  // S
            ev(60, 0x53, true),  // auto-repeat
            ev(90, 0x53, false), //
            ev(120, 0xA2, false),
            ev(700, 0x48, true), // H
            ev(750, 0x48, false),
            ev(20_000, 0x0D, true), // Enter after a long pause
            ev(20_050, 0x0D, false),
            ev(20_100, 0x5B, true), // Win alone
            ev(20_150, 0x5B, false),
            ev(20_200, 0xFF, true), // a key HubDeck cannot send
        ];
        let steps = to_steps(&events);
        let keys: Vec<&str> = steps.iter().map(|s| s["params"]["keys"].as_str().unwrap()).collect();
        assert_eq!(keys, ["Ctrl+S", "H", "Enter", "Win"]);
        let delays: Vec<u64> = steps.iter().map(|s| s["delay"].as_u64().unwrap()).collect();
        assert_eq!(delays, [0, 650, 10_000, 150], "the pause before each key, capped at 10 s");
        for k in keys {
            assert!(super::super::hotkey::parse(k).is_some(), "{k} is sendable");
        }
    }

    #[test]
    fn key_names_cover_the_keyboard() {
        assert_eq!(key_name(0x41).as_deref(), Some("A"));
        assert_eq!(key_name(0x35).as_deref(), Some("5"));
        assert_eq!(key_name(0x63).as_deref(), Some("3"));
        assert_eq!(key_name(0x7C).as_deref(), Some("F13"));
        assert_eq!(key_name(0xBB).as_deref(), Some("="));
        assert_eq!(key_name(0xFF), None);
    }
}
