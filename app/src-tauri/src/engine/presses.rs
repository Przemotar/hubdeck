//! What the user presses (roadmap phase 12 D and F, from the assistant's own analysis): which
//! controls of a draft they touched while trying it (kept in memory, for the assistant's next
//! step), and, only if they switched it on, how often each control of each profile is pressed
//! (`presses.json`, on this computer), so layouts can follow what is really used.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// Counts are written at most this often.
const SAVE_EVERY: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Count {
    pub count: u64,
    /// The last day it was pressed (`2026-10-12`).
    pub last_day: String,
}

/// Profile id -> control id -> count.
type Counts = BTreeMap<String, BTreeMap<String, Count>>;

struct State {
    counts: Counts,
    saved: Instant,
    dirty: bool,
    /// The draft being tried and what was touched on it, in order (no repeats).
    trying: Option<(String, Vec<String>)>,
}

static FILE: OnceLock<PathBuf> = OnceLock::new();
static STATE: Mutex<Option<State>> = Mutex::new(None);

pub fn init(data_dir: &std::path::Path) {
    let _ = FILE.set(data_dir.join("presses.json"));
}

fn with<T>(f: impl FnOnce(&mut State) -> T) -> T {
    let mut g = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let s = g.get_or_insert_with(|| State {
        counts: FILE.get().and_then(|p| std::fs::read(p).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default(),
        saved: Instant::now(),
        dirty: false,
        trying: None,
    });
    f(s)
}

fn save(s: &mut State) {
    if let (Some(path), Ok(bytes)) = (FILE.get(), serde_json::to_vec_pretty(&s.counts)) {
        let _ = super::profile::write_atomic(path, &bytes);
    }
    s.saved = Instant::now();
    s.dirty = false;
}

/// A press on `control` of `profile`; `trying`: the draft on the deck for a try (if any);
/// `counting`: the user allowed counting.
pub fn record(profile: &str, control: &str, trying: Option<&str>, counting: bool) {
    with(|s| {
        if let Some(draft) = trying {
            match &mut s.trying {
                Some((id, touched)) if id == draft => {
                    if !touched.iter().any(|c| c == control) {
                        touched.push(control.to_string());
                    }
                }
                _ => s.trying = Some((draft.to_string(), vec![control.to_string()])),
            }
        }
        if counting {
            let day = chrono::Local::now().format("%Y-%m-%d").to_string();
            let c = s.counts.entry(profile.to_string()).or_default().entry(control.to_string()).or_default();
            c.count += 1;
            c.last_day = day;
            s.dirty = true;
            if s.saved.elapsed() >= SAVE_EVERY {
                save(s);
            }
        }
    });
}

/// Write what is not written yet (HubDeck stops).
pub fn flush() {
    with(|s| {
        if s.dirty {
            save(s);
        }
    });
}

/// What was touched on the draft tried last: (draft id, controls in order).
pub fn touched() -> Option<(String, Vec<String>)> {
    with(|s| s.trying.clone())
}

/// A profile's counts, most pressed first.
pub fn counts(profile: &str) -> Vec<(String, Count)> {
    let mut list: Vec<(String, Count)> = with(|s| s.counts.get(profile).cloned().unwrap_or_default()).into_iter().collect();
    list.sort_by_key(|e| std::cmp::Reverse(e.1.count));
    list
}

/// Forget one profile's counts (the tour's deck starts counting when it is kept, not while tried).
pub fn forget_profile(profile: &str) {
    with(|s| {
        if s.counts.remove(profile).is_some() {
            save(s);
        }
    });
}

/// Forget every count (the switch turned off asks for it).
pub fn forget() {
    with(|s| {
        s.counts.clear();
        save(s);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tries_and_counts() {
        let dir = std::env::temp_dir().join(format!("hubdeck-presses-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        init(&dir);
        record("p", "keys.1", Some("d1"), false);
        record("p", "keys.1", Some("d1"), false);
        record("p", "dial", Some("d1"), false);
        assert_eq!(touched(), Some(("d1".into(), vec!["keys.1".into(), "dial".into()])));
        assert!(counts("p").is_empty(), "nothing counted without the user's yes");
        record("p", "keys.2", None, true);
        record("p", "keys.2", None, true);
        record("p", "keys.0", None, true);
        let c = counts("p");
        assert_eq!((c[0].0.as_str(), c[0].1.count), ("keys.2", 2));
        forget();
        assert!(counts("p").is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }
}
