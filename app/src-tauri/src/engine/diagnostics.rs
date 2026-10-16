//! One text file with what a bug report needs: versions, system, modules, settings flags, plugins
//! and the end of the log. Private things stay out: no serial numbers (only their last 4 signs), no
//! tokens or passwords, no song titles.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::deck::DeckSnapshot;

const LOG_LINES: usize = 400;

fn mask(serial: &str) -> String {
    let n = serial.chars().count();
    format!("...{}", serial.chars().skip(n.saturating_sub(4)).collect::<String>())
}

/// Log lines worth sharing: drops what could be private.
pub fn scrub(line: &str) -> Option<String> {
    if line.contains("now playing:") {
        return None;
    }
    // serial numbers in hub lines: "connected keys MHKD15AA431251603100 (...)"; web addresses
    // (they can carry keys and name the user's machines); a word after "Bearer"; the home folder
    let home = dirs_home();
    let mut bearer = false;
    let words: Vec<String> = line
        .split(' ')
        .map(|w| {
            let out = if bearer {
                "(hidden)".to_string()
            } else if w.len() >= 16 && w.starts_with("MH") && w.chars().all(|c| c.is_ascii_alphanumeric()) {
                mask(w)
            } else if let Some(i) = w.find("://") {
                format!("{}://(address)", w[..i].trim_start_matches(|c: char| !c.is_ascii_alphanumeric()))
            } else {
                match &home {
                    Some(h) if w.contains(h.as_str()) => w.replace(h.as_str(), "~"),
                    _ => w.to_string(),
                }
            };
            bearer = w.eq_ignore_ascii_case("bearer");
            out
        })
        .collect();
    Some(words.join(" "))
}

fn dirs_home() -> Option<String> {
    std::env::var(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).ok().filter(|h| h.len() > 3)
}

/// Settings as the diagnostics show them: switches, numbers and short names stay; anything that
/// could be an address, a path, a key or a name of the user's becomes "(set)"; lists are counted.
pub fn settings_view(v: &serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match v {
        Value::Object(o) => Value::Object(o.iter().map(|(k, v)| (k.clone(), settings_view(v))).collect()),
        Value::Array(a) if a.iter().all(|x| x.is_string() && x.as_str().is_some_and(|s| s.len() <= 16 && !s.contains(['/', '\\', ':', '.']))) => v.clone(),
        Value::Array(a) => serde_json::json!(format!("({} items)", a.len())),
        Value::String(s) if s.is_empty() || (s.len() <= 16 && !s.contains(['/', '\\', ':', '@', '.'])) => v.clone(),
        Value::String(_) => serde_json::json!("(set)"),
        _ => v.clone(),
    }
}

/// A short version for the bug form on GitHub (owner's pick 2a, 2026-10-16): version, system,
/// modules and the last warnings and errors, scrubbed like the file, short enough for the form's
/// address; the person sees it in the form and can change it before sending.
pub fn brief(log_file: &Path, version: &str, deck: &DeckSnapshot) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "HubDeck {version}, {} ({}, {})", sysinfo::System::long_os_version().unwrap_or_default(), std::env::consts::OS, std::env::consts::ARCH);
    let modules: Vec<String> = deck.modules.iter().map(|m| format!("{} {}", m.kind, m.firmware.as_deref().unwrap_or("?"))).collect();
    let _ = writeln!(out, "Modules: {}", if modules.is_empty() { "none".to_string() } else { modules.join(", ") });
    if !deck.unresponsive.is_empty() {
        let _ = writeln!(out, "Not responding: {}", deck.unresponsive.join(", "));
    }
    let log = std::fs::read_to_string(log_file).unwrap_or_default();
    let problems: Vec<String> = log.lines().filter(|l| l.contains("][WARN][") || l.contains("][ERROR][")).filter_map(scrub).map(|l| l.chars().take(220).collect()).collect();
    let _ = writeln!(out, "Last warnings and errors ({} of {}):", problems.len().min(BRIEF_LINES), problems.len());
    for l in problems.iter().skip(problems.len().saturating_sub(BRIEF_LINES)) {
        let _ = writeln!(out, "{l}");
    }
    out
}

const BRIEF_LINES: usize = 20;

pub fn write(dir: &Path, log_file: &Path, version: &str, deck: &DeckSnapshot, settings: &serde_json::Value, plugins: &[super::plugins::PluginInfo]) -> std::io::Result<PathBuf> {
    let mut out = String::new();
    let _ = writeln!(out, "HubDeck {version} diagnostics, {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
    let _ = writeln!(out, "System: {} ({}, {})", sysinfo::System::long_os_version().unwrap_or_default(), std::env::consts::OS, std::env::consts::ARCH);
    let _ = writeln!(out, "\nModules (base turn {}, sensor {:?}, paused {}):", deck.turn, deck.orientation, deck.paused);
    for m in &deck.modules {
        let _ = writeln!(out, "  {:<8} serial {} firmware {} slot {}", m.kind, mask(&m.serial), m.firmware.as_deref().unwrap_or("?"), m.slot.as_deref().unwrap_or("?"));
    }
    if !deck.unresponsive.is_empty() {
        let _ = writeln!(out, "  not responding: {}", deck.unresponsive.join(", "));
    }
    let _ = writeln!(out, "\nSettings: {settings}");
    let _ = writeln!(out, "\nPlugins:");
    for p in plugins {
        let _ = writeln!(out, "  {} {} ({} actions): {}{}", p.id, p.version, p.actions, p.status, if p.connected { ", connected" } else { "" });
    }
    let _ = writeln!(out, "\nLast {LOG_LINES} log lines:");
    let log = std::fs::read_to_string(log_file).unwrap_or_default();
    let lines: Vec<&str> = log.lines().collect();
    for line in lines.iter().skip(lines.len().saturating_sub(LOG_LINES)) {
        if let Some(l) = scrub(line) {
            let _ = writeln!(out, "{l}");
        }
    }
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("hubdeck-diagnostics-{}.txt", chrono::Local::now().format("%Y-%m-%d_%H-%M-%S")));
    std::fs::write(&path, out)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostics_hide_addresses_keys_and_paths() {
        let l = scrub("GET https://api.example.com/x?key=abc failed: Bearer abc123 for MHKD15AA431251603100").unwrap();
        assert!(!l.contains("example.com") && !l.contains("abc123") && !l.contains("key=abc") && l.contains("...3100"), "{l}");
        let v = settings_view(
            &serde_json::json!({"language": "pl", "obsUrl": "ws://192.168.1.5:4455", "profilesDir": "/Users/me/Dropbox", "count": 3, "on": true, "spotifyClientId": "0123456789abcdef0123456789abcdef", "apps": [{"url": "http://x"}]}),
        );
        assert_eq!(v["language"], "pl");
        assert_eq!(v["obsUrl"], "(set)");
        assert_eq!(v["profilesDir"], "(set)");
        assert_eq!(v["spotifyClientId"], "(set)");
        assert_eq!(v["apps"], "(1 items)");
        assert_eq!(v["count"], 3);
    }

    #[test]
    fn the_brief_has_only_the_last_warnings_and_errors_scrubbed() {
        let dir = std::env::temp_dir().join(format!("hubdeck-brief-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let log = dir.join("hubdeck.log");
        let mut text = String::from(
            "[2026-10-16][10:00:00][INFO][x] fine
",
        );
        for i in 0..30 {
            text += &format!(
                "[2026-10-16][10:00:{i:02}][WARN][x] keys MHKD15AA431251603100 dropped {i}
"
            );
        }
        std::fs::write(&log, text).unwrap();
        let b = brief(&log, "0.1.0", &DeckSnapshot::default());
        assert!(b.starts_with("HubDeck 0.1.0") && b.contains("Modules: none") && b.contains("(20 of 30)"), "{b}");
        assert!(
            !b.contains("fine")
                && !b.contains("MHKD15AA431251603100")
                && b.contains("dropped 29")
                && !b.contains(
                    "dropped 9
"
                ),
            "{b}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn private_things_stay_out() {
        assert_eq!(scrub("connected keys MHKD15AA431251603100 (output report 320 B)").unwrap(), "connected keys ...3100 (output report 320 B)");
        assert!(scrub("[..][INFO] now playing: Artist - Song (playing, cover none)").is_none());
        assert_eq!(scrub("profile switched").unwrap(), "profile switched");
    }
}
