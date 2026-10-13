//! What the deck assistant remembers about the user's taste (`taste.json`, on this computer only):
//! short notes the user can read, change and remove in the Assistant tab ("likes few animations",
//! "keeps Spotify on the first knob"), and which of its ideas they kept or threw away. A summary
//! goes with the first message of a talk, inside the scan text the user sees before sending.

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

/// At most this many notes and outcomes are kept (oldest go first).
const MAX_NOTES: usize = 30;
const MAX_HISTORY: usize = 40;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub text: String,
    /// `assistant` (it noticed) or `user` (written in the tab).
    pub by: String,
    pub at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    /// The draft's name, and the app it was for.
    pub title: String,
    #[serde(default)]
    pub app: String,
    /// `kept` or `thrown`.
    pub outcome: String,
    pub at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Taste {
    #[serde(default)]
    pub notes: Vec<Note>,
    #[serde(default)]
    pub history: Vec<Outcome>,
}

static FILE: OnceLock<PathBuf> = OnceLock::new();
static LOCK: Mutex<()> = Mutex::new(());

pub fn init(data_dir: &std::path::Path) {
    let _ = FILE.set(data_dir.join("taste.json"));
}

fn now() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M").to_string()
}

pub fn get() -> Taste {
    FILE.get().and_then(|f| std::fs::read(f).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn change(f: impl FnOnce(&mut Taste)) -> Result<Taste, String> {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut t = get();
    f(&mut t);
    let excess = t.notes.len().saturating_sub(MAX_NOTES);
    t.notes.drain(..excess);
    let excess = t.history.len().saturating_sub(MAX_HISTORY);
    t.history.drain(..excess);
    let path = FILE.get().ok_or("no data folder")?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(&t).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())?;
    Ok(t)
}

/// The assistant noticed something (one short sentence); a note it already has is not repeated.
pub fn remember(text: &str) -> Result<String, String> {
    let text: String = text.trim().chars().take(160).collect();
    if text.is_empty() {
        return Err("say what to remember in one short sentence".into());
    }
    change(|t| {
        if !t.notes.iter().any(|n| n.text.eq_ignore_ascii_case(&text)) {
            t.notes.push(Note { text: text.clone(), by: "assistant".into(), at: now() });
        }
    })?;
    Ok(text)
}

/// An idea's draft was kept or thrown away.
pub fn outcome(title: &str, app: &str, kept: bool) {
    let _ = change(|t| t.history.push(Outcome { title: title.into(), app: app.into(), outcome: if kept { "kept" } else { "thrown" }.into(), at: now() }));
}

/// What goes with the first message of a talk (and is shown in the scan card first).
pub fn text() -> String {
    let t = get();
    let mut out = String::from("What HubDeck remembers about this user's taste (they can see and edit it in the window):");
    if t.notes.is_empty() {
        out.push_str(" nothing yet.");
    }
    for n in &t.notes {
        out.push_str(&format!("\n- {}", n.text));
    }
    let list = |o: &str| t.history.iter().rev().filter(|h| h.outcome == o).take(8).map(|h| h.title.clone()).collect::<Vec<_>>();
    let (kept, thrown) = (list("kept"), list("thrown"));
    if !kept.is_empty() {
        out.push_str(&format!("\nIdeas they kept before: {}.", kept.join(", ")));
    }
    if !thrown.is_empty() {
        out.push_str(&format!("\nIdeas they threw away: {}.", thrown.join(", ")));
    }
    out
}

/// The notes and outcomes, for the Assistant tab.
#[tauri::command]
pub fn taste_get() -> Taste {
    get()
}

/// The user changed the notes (edited, removed or added some; their own get `by: user`).
#[tauri::command]
pub fn taste_set(notes: Vec<String>) -> Result<Taste, String> {
    change(|t| {
        let old = std::mem::take(&mut t.notes);
        t.notes = notes
            .into_iter()
            .map(|n| n.trim().chars().take(160).collect::<String>())
            .filter(|n| !n.is_empty())
            .map(|text| old.iter().find(|o| o.text == text).cloned().unwrap_or(Note { text, by: "user".into(), at: now() }))
            .collect();
    })
}

/// Forget everything (notes and outcomes).
#[tauri::command]
pub fn taste_forget() -> Result<Taste, String> {
    change(|t| *t = Taste::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_and_outcomes_make_the_summary() {
        let dir = std::env::temp_dir().join(format!("hubdeck-taste-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        init(&dir);
        assert!(text().contains("nothing yet"));
        remember("Likes few animations").unwrap();
        remember("likes few animations").unwrap();
        assert!(remember("  ").is_err());
        outcome("Spotify mixer · sketch", "Spotify.exe", true);
        outcome("Neon desk · sketch", "", false);
        let t = text();
        assert!(t.contains("- Likes few animations") && t.matches("animations").count() == 1, "{t}");
        assert!(t.contains("kept before: Spotify mixer") && t.contains("threw away: Neon desk"), "{t}");
        let after = taste_set(vec!["Likes few animations".into(), "Left-handed".into()]).unwrap();
        assert_eq!(after.notes[0].by, "assistant", "a kept note keeps who wrote it");
        assert_eq!(after.notes[1].by, "user");
        assert!(taste_forget().unwrap().notes.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }
}
