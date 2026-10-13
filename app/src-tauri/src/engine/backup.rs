//! Backups of the profiles and settings: `<data>/backups/<when> <reason>/` holds a copy of
//! `settings.json`, `placements.json` and `profiles/`. Made once a day at start and before every
//! change that is hard to undo (import, delete, restore); the newest `KEEP` are kept.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::Serialize;

const KEEP: usize = 30;
const DAILY: Duration = Duration::from_secs(24 * 3600);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    /// Folder name, also the id for restoring.
    pub id: String,
    /// Local time, `2026-10-10 01:20:33`.
    pub when: String,
    pub reason: String,
    pub profiles: usize,
}

fn root(data: &Path) -> PathBuf {
    data.join("backups")
}

/// Copy settings, placements and the profiles from `from_profiles` into `to` (+ `to_profiles`).
fn copy_set(from: &Path, from_profiles: &Path, to: &Path, to_profiles: &Path) -> std::io::Result<usize> {
    std::fs::create_dir_all(to_profiles)?;
    for name in ["settings.json", "placements.json"] {
        if from.join(name).is_file() {
            std::fs::copy(from.join(name), to.join(name))?;
        }
    }
    let mut n = 0;
    if let Ok(entries) = std::fs::read_dir(from_profiles) {
        for e in entries.flatten().filter(|e| e.path().extension().is_some_and(|x| x == "json")) {
            std::fs::copy(e.path(), to_profiles.join(e.file_name()))?;
            n += 1;
        }
    }
    Ok(n)
}

/// Copy the current profiles and settings into a new backup; returns its id.
pub fn create(data: &Path, reason: &str) -> std::io::Result<String> {
    let stamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
    let safe: String = reason.chars().filter(|c| c.is_ascii_alphanumeric() || *c == ' ').take(30).collect();
    let mut id = format!("{stamp} {}", safe.trim());
    let mut dir = root(data).join(&id);
    // two backups in the same second get a counter
    let mut k = 2;
    while dir.exists() {
        id = format!("{stamp} {} {k}", safe.trim());
        dir = root(data).join(&id);
        k += 1;
    }
    copy_set(data, &super::profile::profiles_dir_of(data), &dir, &dir.join("profiles"))?;
    prune(data);
    Ok(id)
}

/// At start: one backup a day.
pub fn daily(data: &Path) {
    let newest = list(data).first().and_then(|b| std::fs::metadata(root(data).join(&b.id)).ok()?.modified().ok());
    let due = newest.is_none_or(|t| SystemTime::now().duration_since(t).unwrap_or_default() >= DAILY);
    if due && super::profile::profiles_dir_of(data).is_dir() {
        match create(data, "daily") {
            Ok(id) => log::info!("backup {id}"),
            Err(e) => log::warn!("backup: {e}"),
        }
    }
}

fn prune(data: &Path) {
    for old in list(data).into_iter().skip(KEEP) {
        let _ = std::fs::remove_dir_all(root(data).join(old.id));
    }
}

/// Newest first.
pub fn list(data: &Path) -> Vec<BackupInfo> {
    let Ok(entries) = std::fs::read_dir(root(data)) else { return Vec::new() };
    let mut out: Vec<BackupInfo> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| {
            let id = e.file_name().to_string_lossy().to_string();
            let (stamp, reason) = id.split_once(' ').unwrap_or((&id, ""));
            let when = stamp.replacen('_', " ", 1).replace('-', ":").replacen(':', "-", 2);
            let profiles = std::fs::read_dir(e.path().join("profiles")).map(|r| r.count()).unwrap_or(0);
            BackupInfo { id: id.clone(), when, reason: reason.to_string(), profiles }
        })
        .collect();
    out.sort_by(|a, b| b.id.cmp(&a.id));
    out
}

/// Put a backup back. The current state is backed up first, so a restore can be undone.
pub fn restore(data: &Path, id: &str) -> Result<(), String> {
    if id.contains(['/', '\\']) || id.contains("..") {
        return Err("bad backup id".into());
    }
    let from = root(data).join(id);
    if !from.is_dir() {
        return Err(format!("no backup {id}"));
    }
    create(data, "before restore").map_err(|e| e.to_string())?;
    // the profiles go back where they live now (a shared folder stays shared)
    let profiles = super::profile::profiles_dir_of(data);
    // replace the profiles folder as a whole: profiles made after the backup go away
    if let Ok(entries) = std::fs::read_dir(&profiles) {
        for e in entries.flatten().filter(|e| e.path().extension().is_some_and(|x| x == "json")) {
            std::fs::remove_file(e.path()).map_err(|e| e.to_string())?;
        }
    }
    let keep_dir = super::profile::profiles_dir_of(data);
    copy_set(&from, &from.join("profiles"), data, &profiles).map_err(|e| e.to_string())?;
    // the restored settings must not move the profiles folder away from where it is now
    if super::profile::profiles_dir_of(data) != keep_dir
        && let Ok(mut v) = std::fs::read(data.join("settings.json")).map_err(|e| e.to_string()).and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).map_err(|e| e.to_string()))
    {
        if let Some(o) = v.as_object_mut() {
            match keep_dir == data.join("profiles") {
                true => o.remove("profilesDir"),
                false => o.insert("profilesDir".into(), serde_json::json!(keep_dir.to_string_lossy())),
            };
        }
        let _ = super::profile::write_atomic(&data.join("settings.json"), &serde_json::to_vec_pretty(&v).unwrap_or_default());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_restore_round_trip() {
        let data = std::env::temp_dir().join(format!("hubdeck-backup-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&data);
        std::fs::create_dir_all(data.join("profiles")).unwrap();
        std::fs::write(data.join("settings.json"), "{\"autoSwitch\":false}").unwrap();
        std::fs::write(data.join("profiles/one.json"), "{}").unwrap();
        let id = create(&data, "before import").unwrap();
        assert!(id.ends_with("before import"));
        // a profile added after the backup, and changed settings
        std::fs::write(data.join("profiles/two.json"), "{}").unwrap();
        std::fs::write(data.join("settings.json"), "{\"autoSwitch\":true}").unwrap();
        restore(&data, &id).unwrap();
        assert!(data.join("profiles/one.json").is_file());
        assert!(!data.join("profiles/two.json").exists(), "the restore replaces the set");
        assert_eq!(std::fs::read_to_string(data.join("settings.json")).unwrap(), "{\"autoSwitch\":false}");
        // the state before the restore was kept
        let all = list(&data);
        assert_eq!(all.len(), 2);
        assert!(all.iter().any(|b| b.reason == "before restore" && b.profiles == 2));
        assert!(restore(&data, "../x").is_err());
        let _ = std::fs::remove_dir_all(data);
    }
}
