//! `Ctrl+Shift+F13` -> key presses. Modifiers go down first and come up last, in reverse.

use enigo::{Direction, Enigo, Key, Keyboard};

/// Names the hotkey field offers in its list (besides recording from the keyboard).
pub const SPECIAL_KEYS: &[&str] = &[
    "F13",
    "F14",
    "F15",
    "F16",
    "F17",
    "F18",
    "F19",
    "F20",
    "F21",
    "F22",
    "F23",
    "F24",
    "PlayPause",
    "NextTrack",
    "PrevTrack",
    "VolumeUp",
    "VolumeDown",
    "VolumeMute",
    "PrintScreen",
    "Escape",
    "Enter",
    "Tab",
    "Space",
    "Backspace",
    "Delete",
    "Home",
    "End",
    "PageUp",
    "PageDown",
    "Up",
    "Down",
    "Left",
    "Right",
    "Insert",
    "CapsLock",
    "NumLock",
    "ScrollLock",
    "Pause",
    "Num0",
    "Num1",
    "Num2",
    "Num3",
    "Num4",
    "Num5",
    "Num6",
    "Num7",
    "Num8",
    "Num9",
    "NumAdd",
    "NumSubtract",
    "NumMultiply",
    "NumDivide",
    "NumDecimal",
];

/// Windows virtual-key codes of the lock keys, Insert, Pause and the numeric keypad (whose keys games
/// tell apart from the main digits).
#[cfg_attr(not(windows), allow(dead_code))]
fn vk_extra(lower: &str) -> Option<u16> {
    Some(match lower {
        "insert" | "ins" => 0x2D,
        "capslock" => 0x14,
        "numlock" => 0x90,
        "scrolllock" => 0x91,
        "pause" | "break" => 0x13,
        "numadd" => 0x6B,
        "numsubtract" => 0x6D,
        "nummultiply" => 0x6A,
        "numdivide" => 0x6F,
        "numdecimal" => 0x6E,
        n if n.len() == 4 && n.starts_with("num") => 0x60 + n[3..].parse::<u16>().ok().filter(|d| *d <= 9)?,
        _ => return None,
    })
}

/// macOS key codes of the keys enigo has no name for there (Scroll Lock and Pause have none).
#[cfg(target_os = "macos")]
fn mac_extra(lower: &str) -> Option<u16> {
    Some(match lower {
        "insert" | "ins" => 0x72,
        "numlock" => 0x47,
        "numadd" => 0x45,
        "numsubtract" => 0x4E,
        "nummultiply" => 0x43,
        "numdivide" => 0x4B,
        "numdecimal" => 0x41,
        n if n.len() == 4 && n.starts_with("num") => [0x52, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5B, 0x5C][n[3..].parse::<usize>().ok().filter(|d| *d <= 9)?],
        _ => return None,
    })
}

fn modifier(name: &str) -> Option<Key> {
    Some(match name {
        "ctrl" | "control" => Key::Control,
        "shift" => Key::Shift,
        "alt" | "option" => Key::Alt,
        "win" | "cmd" | "command" | "meta" | "super" => Key::Meta,
        _ => return None,
    })
}

fn key(name: &str) -> Option<Key> {
    let lower = name.to_lowercase();
    let k = match lower.as_str() {
        "f1" => Key::F1,
        "f2" => Key::F2,
        "f3" => Key::F3,
        "f4" => Key::F4,
        "f5" => Key::F5,
        "f6" => Key::F6,
        "f7" => Key::F7,
        "f8" => Key::F8,
        "f9" => Key::F9,
        "f10" => Key::F10,
        "f11" => Key::F11,
        "f12" => Key::F12,
        "f13" => Key::F13,
        "f14" => Key::F14,
        "f15" => Key::F15,
        "f16" => Key::F16,
        "f17" => Key::F17,
        "f18" => Key::F18,
        "f19" => Key::F19,
        "f20" => Key::F20,
        #[cfg(not(target_os = "macos"))]
        "f21" => Key::F21,
        #[cfg(not(target_os = "macos"))]
        "f22" => Key::F22,
        #[cfg(not(target_os = "macos"))]
        "f23" => Key::F23,
        #[cfg(not(target_os = "macos"))]
        "f24" => Key::F24,
        "playpause" | "mediaplaypause" => Key::MediaPlayPause,
        "nexttrack" | "medianext" => Key::MediaNextTrack,
        "prevtrack" | "mediaprev" => Key::MediaPrevTrack,
        "volumeup" => Key::VolumeUp,
        "volumedown" => Key::VolumeDown,
        "volumemute" | "mute" => Key::VolumeMute,
        // macOS has no Print Screen key (enigo gates it); the Mac preset uses Cmd+Shift+4 instead
        #[cfg(not(target_os = "macos"))]
        "printscreen" | "prtsc" => Key::PrintScr,
        "escape" | "esc" => Key::Escape,
        "enter" | "return" => Key::Return,
        "tab" => Key::Tab,
        "space" => Key::Space,
        "backspace" => Key::Backspace,
        "delete" | "del" => Key::Delete,
        "home" => Key::Home,
        "end" => Key::End,
        "pageup" => Key::PageUp,
        "pagedown" => Key::PageDown,
        "up" => Key::UpArrow,
        "down" => Key::DownArrow,
        "left" => Key::LeftArrow,
        "right" => Key::RightArrow,
        #[cfg(windows)]
        other if vk_extra(other).is_some() => Key::Other(u32::from(vk_extra(other)?)),
        #[cfg(target_os = "macos")]
        "capslock" => Key::CapsLock,
        // a Mac keyboard's Insert is the Help key, Num Lock the keypad's Clear; the keypad by its
        // own key codes (kVK_ANSI_Keypad*)
        #[cfg(target_os = "macos")]
        other if mac_extra(other).is_some() => Key::Other(u32::from(mac_extra(other)?)),
        _ => {
            let mut chars = name.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => Key::Unicode(c.to_ascii_lowercase()),
                _ => return None,
            }
        }
    };
    Some(k)
}

#[derive(Debug, PartialEq)]
pub struct Combo {
    pub modifiers: Vec<Key>,
    pub key: Key,
}

/// Parse `Ctrl+Shift+F13`; `None` for an empty or unknown combination.
pub fn parse(text: &str) -> Option<Combo> {
    let parts: Vec<&str> = text.split('+').map(str::trim).filter(|p| !p.is_empty()).collect();
    let (last, mods) = parts.split_last()?;
    let modifiers = mods.iter().map(|m| modifier(&m.to_lowercase())).collect::<Option<Vec<_>>>()?;
    // a lone modifier ("Shift") is a valid key on its own
    let key = key(last).or_else(|| modifier(&last.to_lowercase()))?;
    Some(Combo { modifiers, key })
}

/// Windows virtual-key code of a key name, for sending hardware scan codes to games.
#[cfg(windows)]
fn vk_of(name: &str) -> Option<u16> {
    let lower = name.to_lowercase();
    Some(match lower.as_str() {
        "ctrl" | "control" => 0x11,
        "shift" => 0x10,
        "alt" | "option" => 0x12,
        "win" | "cmd" | "command" | "meta" | "super" => 0x5B,
        "space" => 0x20,
        "enter" | "return" => 0x0D,
        "tab" => 0x09,
        "escape" | "esc" => 0x1B,
        "backspace" => 0x08,
        "delete" | "del" => 0x2E,
        "home" => 0x24,
        "end" => 0x23,
        "pageup" => 0x21,
        "pagedown" => 0x22,
        "left" => 0x25,
        "up" => 0x26,
        "right" => 0x27,
        "down" => 0x28,
        other if vk_extra(other).is_some() => vk_extra(other)?,
        f if f.starts_with('f') && f.len() <= 3 => {
            let n: u16 = f[1..].parse().ok()?;
            if !(1..=24).contains(&n) {
                return None;
            }
            0x6F + n
        }
        _ => {
            let mut chars = name.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) if c.is_ascii_alphanumeric() => c.to_ascii_uppercase() as u16,
                _ => return None,
            }
        }
    })
}

/// For games: send the combination as hardware scan codes and hold it `hold` long. Many games read
/// the key state (DirectInput, raw input) and miss a press that is released in the same instant.
#[cfg(windows)]
pub fn send_game(enigo: &mut Enigo, text: &str, hold: std::time::Duration) -> Result<(), String> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{MAPVK_VK_TO_VSC_EX, MapVirtualKeyW};
    let parts: Vec<&str> = text.split('+').map(str::trim).filter(|p| !p.is_empty()).collect();
    if parts.is_empty() {
        return Ok(());
    }
    let mut scans = Vec::new();
    for p in &parts {
        let vk = vk_of(p).ok_or_else(|| format!("{p:?} cannot be sent as a game key; use letters, digits, F1-F24, arrows and the common keys"))?;
        // SAFETY: plain lookup of the keyboard layout's scan code.
        let scan = unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC_EX) } as u16;
        if scan == 0 {
            return Err(format!("no scan code for {p:?}"));
        }
        scans.push(scan);
    }
    for s in &scans {
        enigo.raw(*s, Direction::Press).map_err(|e| e.to_string())?;
    }
    std::thread::sleep(hold);
    for s in scans.iter().rev() {
        let _ = enigo.raw(*s, Direction::Release);
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn send_game(enigo: &mut Enigo, text: &str, _hold: std::time::Duration) -> Result<(), String> {
    send(enigo, text)
}

pub fn send(enigo: &mut Enigo, text: &str) -> Result<(), String> {
    let combo = parse(text).ok_or_else(|| format!("unknown key combination: {text:?}"))?;
    for m in &combo.modifiers {
        enigo.key(*m, Direction::Press).map_err(|e| e.to_string())?;
    }
    let result = enigo.key(combo.key, Direction::Click).map_err(|e| e.to_string());
    for m in combo.modifiers.iter().rev() {
        let _ = enigo.key(*m, Direction::Release);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_combinations() {
        assert_eq!(parse("Ctrl+Shift+F13"), Some(Combo { modifiers: vec![Key::Control, Key::Shift], key: Key::F13 }));
        assert_eq!(parse("ctrl + s"), Some(Combo { modifiers: vec![Key::Control], key: Key::Unicode('s') }));
        assert_eq!(vk_extra("num7"), Some(0x67), "the keypad's 7, not the main 7");
        assert_eq!(vk_extra("numlock"), Some(0x90));
        assert_eq!(vk_extra("num10"), None);
        #[cfg(windows)]
        assert_eq!(parse("Shift+Num5").map(|c| c.key), Some(Key::Other(0x65)));
        assert_eq!(parse("PlayPause"), Some(Combo { modifiers: vec![], key: Key::MediaPlayPause }));
        assert_eq!(parse("Shift"), Some(Combo { modifiers: vec![], key: Key::Shift }));
        assert_eq!(parse(""), None);
        assert_eq!(parse("Ctrl+Nope"), None);
        assert_eq!(parse("Hyper+A"), None);
    }

    #[cfg(windows)]
    #[test]
    fn game_keys_have_virtual_key_codes() {
        assert_eq!(vk_of("W"), Some(0x57));
        assert_eq!(vk_of("5"), Some(0x35));
        assert_eq!(vk_of("F13"), Some(0x7C));
        assert_eq!(vk_of("Space"), Some(0x20));
        assert_eq!(vk_of("F25"), None);
        assert_eq!(vk_of("ä"), None);
    }

    #[test]
    fn every_listed_special_key_parses() {
        for name in SPECIAL_KEYS {
            #[cfg(target_os = "macos")]
            if ["F21", "F22", "F23", "F24", "PrintScreen", "ScrollLock", "Pause"].contains(name) {
                continue;
            }
            assert!(parse(name).is_some(), "{name}");
        }
    }
}
