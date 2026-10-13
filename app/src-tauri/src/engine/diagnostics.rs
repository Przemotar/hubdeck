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
    // serial numbers in hub lines: "connected keys MHKD15AA431251603100 (...)"
    let words: Vec<String> = line.split(' ').map(|w| if w.len() >= 16 && w.starts_with("MH") && w.chars().all(|c| c.is_ascii_alphanumeric()) { mask(w) } else { w.to_string() }).collect();
    Some(words.join(" "))
}

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
    fn private_things_stay_out() {
        assert_eq!(scrub("connected keys MHKD15AA431251603100 (output report 320 B)").unwrap(), "connected keys ...3100 (output report 320 B)");
        assert!(scrub("[..][INFO] now playing: Artist - Song (playing, cover none)").is_none());
        assert_eq!(scrub("profile switched").unwrap(), "profile switched");
    }
}
