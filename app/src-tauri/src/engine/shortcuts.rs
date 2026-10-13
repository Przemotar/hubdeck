//! Checked shortcut packs (`shortcuts.json`): the default Windows shortcuts of popular apps, each
//! pack with the vendor page it was read from. The deck assistant puts only these on keys without
//! a mark; any other shortcut in a profile it made gets a "not checked" mark (`check.rs`), and the
//! ones that close, quit or delete go on a hold, never a plain press (`guard.rs`).

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

const PACKS: &str = include_str!("shortcuts.json");

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Shortcut {
    pub keys: String,
    pub does: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub destructive: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Pack {
    pub app: String,
    /// Programs the pack is for; `*` = everywhere (Windows itself).
    pub exes: Vec<String>,
    pub sources: Vec<String>,
    #[serde(default)]
    pub note: String,
    pub shortcuts: Vec<Shortcut>,
}

#[derive(Deserialize)]
struct File {
    packs: Vec<Pack>,
}

pub fn packs() -> &'static [Pack] {
    static P: OnceLock<Vec<Pack>> = OnceLock::new();
    P.get_or_init(|| serde_json::from_str::<File>(PACKS).map(|f| f.packs).unwrap_or_default())
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

fn exe_name(exe: &str) -> String {
    exe.rsplit(['\\', '/']).next().unwrap_or(exe).to_lowercase()
}

/// The packs that apply to a profile linked to `apps` (their own, and Windows itself).
pub fn for_apps(apps: &[String]) -> Vec<&'static Pack> {
    let names: Vec<String> = apps.iter().map(|a| exe_name(a)).collect();
    packs().iter().filter(|p| p.exes.iter().any(|e| e == "*" || names.iter().any(|n| wildcard(&e.to_lowercase(), n)))).collect()
}

/// `vegas*.exe` matches `vegas220.exe`; otherwise the names are equal.
fn wildcard(pattern: &str, name: &str) -> bool {
    match pattern.split_once('*') {
        Some((head, tail)) => name.len() >= head.len() + tail.len() && name.starts_with(head) && name.ends_with(tail),
        None => pattern == name,
    }
}

fn find(combo: &str, apps: &[String]) -> Option<&'static Shortcut> {
    let n = normalize(combo);
    for_apps(apps).into_iter().flat_map(|p| p.shortcuts.iter()).find(|s| normalize(&s.keys) == n)
}

/// Is `combo` in a checked pack for these apps (or Windows)?
pub fn is_checked(combo: &str, apps: &[String]) -> bool {
    find(combo, apps).is_some()
}

/// Shortcuts that close, quit or delete something wherever they are pressed.
const ALWAYS_DESTRUCTIVE: [&str; 7] = ["alt+f4", "ctrl+w", "ctrl+f4", "ctrl+q", "shift+delete", "ctrl+shift+w", "ctrl+alt+delete"];

/// Does `combo` close, quit or delete (in these apps, or anywhere)?
pub fn is_destructive(combo: &str, apps: &[String]) -> bool {
    ALWAYS_DESTRUCTIVE.contains(&normalize(combo).as_str()) || find(combo, apps).is_some_and(|s| s.destructive)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_packs_read_and_every_shortcut_parses() {
        assert!(!packs().is_empty(), "shortcuts.json reads");
        for p in packs() {
            assert!(!p.sources.is_empty(), "{} has its source", p.app);
            for s in &p.shortcuts {
                // the packs are Windows shortcuts; a Mac has no Print Screen key to parse
                if cfg!(target_os = "macos") && s.keys.contains("PrintScreen") {
                    continue;
                }
                assert!(super::super::hotkey::parse(&s.keys).is_some(), "{}: {:?} parses", p.app, s.keys);
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
}
