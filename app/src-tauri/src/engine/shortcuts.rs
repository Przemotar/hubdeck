//! Checked shortcut packs (`shortcuts.json`): the default shortcuts of popular apps on Windows and
//! on macOS (`os`), each pack with the vendor page it was read from. The deck assistant puts only these on keys without
//! a mark; any other shortcut in a profile it made gets a "not checked" mark (`check.rs`), and the
//! ones that close, quit or delete go on a hold, never a plain press (`guard.rs`).

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use super::profile::app_stem;

const PACKS: &str = include_str!("shortcuts.json");

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Shortcut {
    pub keys: String,
    pub does: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub destructive: bool,
    /// On the app's ready profile, in this order, with this icon (`icons.rs`); a short `label`
    /// for the key when `does` is too long.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Pack {
    pub app: String,
    /// Programs the pack is for (`Spotify.exe`, `Spotify.app`: compared by [`app_stem`]); `*` =
    /// everywhere (the system itself).
    pub exes: Vec<String>,
    /// The system its keys are for: `windows` or `macos` (`std::env::consts::OS`); "" = both.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub os: String,
    pub sources: Vec<String>,
    #[serde(default)]
    pub note: String,
    pub shortcuts: Vec<Shortcut>,
}

#[derive(Deserialize)]
struct File {
    packs: Vec<Pack>,
}

fn all() -> &'static [Pack] {
    static P: OnceLock<Vec<Pack>> = OnceLock::new();
    P.get_or_init(|| serde_json::from_str::<File>(PACKS).map(|f| f.packs).unwrap_or_default())
}

/// The packs for this system (a Windows pack's Ctrl+T is not a Mac's Cmd+T).
pub fn packs() -> Vec<&'static Pack> {
    all().iter().filter(|p| p.os.is_empty() || p.os == std::env::consts::OS).collect()
}

/// `ctrl + shift+m` and `Shift+Ctrl+M` -> `ctrl+shift+m` (modifiers in one order, one spelling).
pub fn normalize(combo: &str) -> String {
    let parts: Vec<String> = combo.split('+').map(|p| p.trim().to_lowercase()).filter(|p| !p.is_empty()).collect();
    let Some((key, mods)) = parts.split_last() else { return String::new() };
    let rank = |m: &str| match m {
        "ctrl" | "control" => (0, "ctrl"),
        "alt" | "option" => (1, "alt"),
        "shift" => (2, "shift"),
        "win" | "cmd" | "command" | "meta" | "super" => (3, "win"),
        _ => (4, ""),
    };
    let mut mods: Vec<(i32, String)> = mods.iter().map(|m| rank(m)).map(|(r, n)| (r, n.to_string())).collect();
    mods.sort();
    mods.dedup();
    let key = match key.as_str() {
        "del" => "delete",
        "esc" => "escape",
        "return" => "enter",
        k => k,
    };
    mods.into_iter().map(|(_, n)| n).chain(std::iter::once(key.to_string())).collect::<Vec<_>>().join("+")
}

/// The packs that apply to a profile linked to `apps` (their own, and the system itself), by one
/// name for an app on both systems (`Google Chrome.app` = `chrome.exe`).
pub fn for_apps(apps: &[String]) -> Vec<&'static Pack> {
    let names: Vec<String> = apps.iter().map(|a| app_stem(a.split(':').next().unwrap_or(a))).collect();
    packs().into_iter().filter(|p| p.exes.iter().any(|e| e == "*" || names.iter().any(|n| wildcard(&app_stem(e), n)))).collect()
}

/// The pack of this program itself (not the system's), for this system.
pub fn own_pack(exe: &str) -> Option<&'static Pack> {
    for_apps(&[exe.to_string()]).into_iter().find(|p| !p.exes.iter().any(|e| e == "*"))
}

/// `vegas*.exe` matches `vegas220.exe`; otherwise the names are equal.
fn wildcard(pattern: &str, name: &str) -> bool {
    match pattern.split_once('*') {
        Some((head, tail)) => name.len() >= head.len() + tail.len() && name.starts_with(head) && name.ends_with(tail),
        None => pattern == name,
    }
}

fn find(combo: &str, apps: &[String]) -> Vec<&'static Shortcut> {
    let n = normalize(combo);
    for_apps(apps).into_iter().flat_map(|p| p.shortcuts.iter()).filter(|s| normalize(&s.keys) == n).collect()
}

/// Is `combo` in a checked pack for these apps (or the system)?
pub fn is_checked(combo: &str, apps: &[String]) -> bool {
    MEDIA_KEYS.contains(&normalize(combo).as_str()) || !find(combo, apps).is_empty()
}

/// A media key alone does the same in every app (the system takes it): no app's pack needed
/// (personas 2026-10-19: Volume keys on an office desk counted as unchecked shortcuts).
const MEDIA_KEYS: [&str; 10] = ["playpause", "nexttrack", "medianext", "prevtrack", "mediaprev", "volumeup", "volumedown", "volumemute", "mute", "mediaplaypause"];

/// Shortcuts that close, quit or delete something wherever they are pressed.
const ALWAYS_DESTRUCTIVE: [&str; 7] = ["alt+f4", "ctrl+w", "ctrl+f4", "ctrl+q", "shift+delete", "ctrl+shift+w", "ctrl+alt+delete"];

/// Does `combo` close, quit or delete (in these apps, or anywhere)?
pub fn is_destructive(combo: &str, apps: &[String]) -> bool {
    // any pack that calls it closing wins (the Mac's own pack: Cmd+W closes in every app)
    ALWAYS_DESTRUCTIVE.contains(&normalize(combo).as_str()) || find(combo, apps).iter().any(|s| s.destructive)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_media_key_alone_is_checked_anywhere() {
        let apps = ["EXCEL.EXE".to_string()];
        assert!(is_checked("VolumeMute", &apps) && is_checked("volumeup", &[]) && is_checked("PlayPause", &apps));
        assert!(!is_checked("ctrl+VolumeUp", &apps), "with a modifier it is an app's shortcut");
    }

    #[test]
    fn the_packs_read_and_every_shortcut_parses() {
        assert!(!all().is_empty(), "shortcuts.json reads");
        for p in all() {
            assert!(["", "windows", "macos"].contains(&p.os.as_str()), "{}: os {:?}", p.app, p.os);
            // a pack without shortcuts says in its note why none could be checked
            assert!(!p.sources.is_empty() || (p.shortcuts.is_empty() && !p.note.is_empty()), "{} has its source", p.app);
            for s in &p.shortcuts {
                // a Mac has no Print Screen key to parse
                if cfg!(target_os = "macos") && s.keys.contains("PrintScreen") {
                    continue;
                }
                assert!(super::super::hotkey::parse(&s.keys).is_some(), "{}: {:?} parses", p.app, s.keys);
                if let Some(icon) = &s.icon {
                    assert!(super::super::icons::ICONS.iter().any(|(n, _)| n == icon), "{}: icon {icon} exists", p.app);
                    assert!(!s.destructive, "{}: {} closes something: never a plain key on the template", p.app, s.keys);
                    let label = s.label.as_deref().unwrap_or(&s.does);
                    assert!(label.chars().count() <= 12, "{}: {label:?} fits a key", p.app);
                }
            }
        }
    }

    #[test]
    fn combos_compare_in_one_spelling() {
        assert_eq!(normalize("Shift + Ctrl+m"), "ctrl+shift+m");
        assert_eq!(normalize("Win+Shift+S"), "shift+win+s");
        assert_eq!(normalize("Ctrl+Del"), "ctrl+delete");
        assert!(wildcard("vegas*.exe", "vegas220.exe") && !wildcard("vegas*.exe", "veg.exe"));
        assert!(is_destructive("Alt+F4", &[]));
        assert!(!is_destructive("Ctrl+Z", &[]));
    }

    #[test]
    fn each_system_checks_its_own_keys() {
        let mac = cfg!(target_os = "macos");
        assert_eq!(is_checked("Cmd+Shift+3", &[]), mac, "the Mac's screenshot");
        assert_eq!(is_checked("Ctrl+Cmd+Q", &[]), mac, "the Mac's lock screen");
        assert_eq!(is_checked("Win+L", &[]), !mac, "Windows' lock");
        assert_eq!(is_destructive("Cmd+Q", &[]), mac);
        // one name for an app on both systems
        assert_eq!(for_apps(&["Google Chrome.app".into()]).len(), for_apps(&["chrome.exe".into()]).len());
    }
}
