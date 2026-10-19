//! How long each app is in front, counted on this computer only (`usage.json`): the deck
//! assistant reads it to know which apps the user really works in. Nothing leaves the computer
//! unless the user sends it from the assistant, after seeing it.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// One stretch in front counts at most this long: an app left in front over lunch is not work.
const MAX_STRETCH: Duration = Duration::from_secs(15 * 60);
/// The file is written at most this often (and when HubDeck stops).
const SAVE_EVERY: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    /// Seconds in front, all together.
    pub seconds: u64,
    /// Days it was in front at all.
    pub days: u32,
    /// The last of those days (`2026-10-12`).
    pub last_day: String,
}

/// The file: since when it counts (local time, `2026-10-12 16:30`) and the apps.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct File {
    pub since: String,
    pub apps: BTreeMap<String, Usage>,
}

struct State {
    file: File,
    current: Option<(String, Instant)>,
    saved: Instant,
}

/// Reads the file; the first version was only the map of apps (counted since the file was made).
fn read(path: &std::path::Path) -> File {
    let now = || chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
    let Ok(bytes) = std::fs::read(path) else { return File { since: now(), apps: BTreeMap::new() } };
    if let Ok(f) = serde_json::from_slice::<File>(&bytes) {
        return f;
    }
    let apps = serde_json::from_slice(&bytes).unwrap_or_default();
    let made = std::fs::metadata(path).and_then(|m| m.created()).ok().map(|t| chrono::DateTime::<chrono::Local>::from(t).format("%Y-%m-%d %H:%M").to_string());
    File { since: made.unwrap_or_else(now), apps }
}

static FILE: OnceLock<PathBuf> = OnceLock::new();
static STATE: Mutex<Option<State>> = Mutex::new(None);

pub fn init(data_dir: &std::path::Path) {
    let _ = FILE.set(data_dir.join("usage.json"));
}

fn with<T>(f: impl FnOnce(&mut State) -> T) -> T {
    let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let state = guard.get_or_insert_with(|| State { file: FILE.get().map(|p| read(p)).unwrap_or_default(), current: None, saved: Instant::now() });
    f(state)
}

/// The program an exe is: a copy an app's own updater left running under a backup name
/// (`my-tool.bak-20261012-112411.exe`) counts as the app (owner's data, 2026-10-19: one
/// tool was four of his "apps used most").
fn program(exe: &str) -> String {
    let lower = exe.to_lowercase();
    match lower.split_once(".bak") {
        Some((stem, rest)) if !stem.is_empty() && rest.ends_with(".exe") => format!("{stem}.exe"),
        _ => lower,
    }
}

/// Add a stretch of `elapsed` in front to `exe` on `day`.
fn add(apps: &mut BTreeMap<String, Usage>, exe: &str, elapsed: Duration, day: &str) {
    let u = apps.entry(program(exe)).or_default();
    u.seconds += elapsed.min(MAX_STRETCH).as_secs();
    if u.last_day != day {
        u.days += 1;
        u.last_day = day.to_string();
    }
}

/// The app in front is now `exe` ("" = none: locked, asleep): the one before gets its time.
pub fn switched(exe: &str) {
    let day = chrono::Local::now().format("%Y-%m-%d").to_string();
    let now = Instant::now();
    let save = with(|s| {
        if let Some((prev, since)) = s.current.take() {
            add(&mut s.file.apps, &prev, now - since, &day);
        }
        // not HubDeck itself, and not the lock screen (owner, 2026-10-14: "loginwindow" became a key)
        if !exe.is_empty() && !exe.to_lowercase().starts_with("hubdeck") && !super::idle::is_lock_screen(exe) {
            s.current = Some((exe.to_string(), now));
        }
        (now - s.saved >= SAVE_EVERY).then(|| {
            s.saved = now;
            s.file.clone()
        })
    });
    if let Some(file) = save {
        write(&file);
    }
}

/// Write what is counted so far (HubDeck stops).
pub fn flush() {
    switched("");
    write(&with(|s| s.file.clone()));
}

fn write(data: &File) {
    if let (Some(file), Ok(json)) = (FILE.get(), serde_json::to_vec_pretty(data))
        && let Err(e) = super::profile::write_atomic(file, &json)
    {
        log::warn!("usage.json: {e}");
    }
}

/// Since when the time in front is counted (local, `2026-10-12 16:30`).
pub fn since() -> String {
    with(|s| s.file.since.clone())
}

/// The apps used most, most first: (exe, hours, days).
pub fn top(n: usize) -> Vec<(String, f32, u32)> {
    // what was counted under a backup name before goes to its program too
    let mut merged: BTreeMap<String, (u64, u32)> = BTreeMap::new();
    with(|s| {
        for (exe, u) in s.file.apps.iter().filter(|(exe, _)| !super::idle::is_lock_screen(exe)) {
            let m = merged.entry(program(exe)).or_default();
            m.0 += u.seconds;
            m.1 = m.1.max(u.days);
        }
    });
    let mut all: Vec<(String, f32, u32)> = merged.into_iter().map(|(exe, (seconds, days))| (exe, seconds as f32 / 3600.0, days)).collect();
    all.sort_by(|a, b| b.1.total_cmp(&a.1));
    all.truncate(n);
    all
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_backup_copy_counts_as_its_program() {
        assert_eq!(program("my-tool.bak-20261012-112411.exe"), "my-tool.exe");
        assert_eq!(program("My-Tool.exe"), "my-tool.exe");
        assert_eq!(program("backup.exe"), "backup.exe");
        assert_eq!(program(".bak.exe"), ".bak.exe");
        let mut apps = BTreeMap::new();
        add(&mut apps, "my-tool.exe", Duration::from_secs(60), "2026-10-19");
        add(&mut apps, "my-tool.bak-20261019-101657.exe", Duration::from_secs(60), "2026-10-19");
        assert_eq!(apps.len(), 1);
        assert_eq!(apps["my-tool.exe"].seconds, 120);
    }

    #[test]
    fn the_first_file_format_is_read_with_its_date() {
        let dir = std::env::temp_dir().join(format!("hubdeck-usage-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("usage.json");
        std::fs::write(&path, r#"{"code.exe": {"seconds": 60, "days": 1, "lastDay": "2026-10-12"}}"#).unwrap();
        let f = read(&path);
        assert_eq!(f.apps["code.exe"].seconds, 60);
        assert!(f.since.starts_with("20"), "{}", f.since);
        std::fs::write(&path, serde_json::to_vec(&File { since: "2026-10-12 16:30".into(), apps: f.apps }).unwrap()).unwrap();
        assert_eq!(read(&path).since, "2026-10-12 16:30");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn stretches_add_up_capped_and_days_count_once() {
        let mut apps = BTreeMap::new();
        add(&mut apps, "Code.exe", Duration::from_secs(600), "2026-10-12");
        add(&mut apps, "code.exe", Duration::from_secs(3 * 3600), "2026-10-12");
        add(&mut apps, "code.exe", Duration::from_secs(60), "2026-10-13");
        let u = &apps["code.exe"];
        assert_eq!(u.seconds, 600 + 15 * 60 + 60, "one stretch counts at most 15 minutes");
        assert_eq!((u.days, u.last_day.as_str()), (2, "2026-10-13"));
    }
}
