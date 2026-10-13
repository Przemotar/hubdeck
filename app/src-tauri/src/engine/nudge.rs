//! Suggestions outside the talk (both design reviews; the owner picked them, 2026-10-12): HubDeck
//! notices on its own, with no AI, that an app the user works in a lot has no deck yet, or that a
//! kind of module was attached for the first time, and offers a sketch made on this computer. The
//! model only comes in if the user asks for it.
//!
//! Brakes: a calm card in the Assistant tab (no pop-ups, nothing taken over on the deck), at most
//! one a day, never during a call or with a game in front, "not now" = 7 quiet days for that
//! subject, "don't suggest this" = 30, and one switch for all (`nudges.json`).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use super::Engine;

/// An app counts as worked in after this long in front, on at least two days.
const MIN_SECONDS: u64 = 30 * 60;
const MIN_DAYS: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Nudge {
    /// `app:<exe>` or `module:<kind>`: what it is about (quiet days are kept per subject).
    pub subject: String,
    /// `app` or `module`.
    pub kind: String,
    /// The app's program and name, or the module kind.
    pub exe: String,
    pub name: String,
    /// Hours in front (apps).
    pub hours: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct File {
    #[serde(default)]
    off: bool,
    /// Subject -> quiet until this day (`2026-10-19`).
    #[serde(default)]
    quiet: BTreeMap<String, String>,
    /// Module kinds seen so far (the first sight of a new kind is a suggestion).
    #[serde(default)]
    seen_kinds: Option<Vec<String>>,
    /// The day's suggestion: (day, subject, acted on).
    #[serde(default)]
    today: Option<(String, String, bool)>,
}

static FILE: OnceLock<PathBuf> = OnceLock::new();
static LOCK: Mutex<()> = Mutex::new(());

pub fn init(data_dir: &std::path::Path) {
    let _ = FILE.set(data_dir.join("nudges.json"));
}

fn read() -> File {
    FILE.get().and_then(|f| std::fs::read(f).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn write(f: &File) {
    if let (Some(path), Ok(bytes)) = (FILE.get(), serde_json::to_vec_pretty(f)) {
        let _ = std::fs::write(path, bytes);
    }
}

fn day(offset_days: i64) -> String {
    (chrono::Local::now() + chrono::Duration::days(offset_days)).format("%Y-%m-%d").to_string()
}

/// What the engine knows now that the choice needs.
pub struct Now<'a> {
    pub today: String,
    pub busy: bool,
    pub attached: &'a [String],
    /// Programs some profile (or draft) is linked to.
    pub linked: &'a [String],
    /// (program, hours, days) from `usage.json`, most first.
    pub used: Vec<(String, f32, u32)>,
    pub name_of: &'a dyn Fn(&str) -> String,
}

/// The suggestion for now, if any (and the file updated: the day's subject, kinds seen).
fn choose(f: &mut File, now: &Now) -> Option<Nudge> {
    // the first time: the kinds attached now are known already, not news
    let seen = f.seen_kinds.get_or_insert_with(|| now.attached.to_vec());
    let new_kind = now.attached.iter().find(|k| !seen.contains(k)).cloned();
    if f.off || now.busy {
        return None;
    }
    let quiet = |subject: &str| f.quiet.get(subject).is_some_and(|until| until.as_str() > now.today.as_str());
    // one a day: today's stays until it is acted on, then nothing more today
    if let Some((d, subject, acted)) = &f.today
        && *d == now.today
    {
        if *acted || quiet(subject) {
            return None;
        }
        if let Some(kind) = subject.strip_prefix("module:") {
            return Some(Nudge { subject: subject.clone(), kind: "module".into(), exe: kind.into(), name: kind.into(), hours: 0.0 });
        }
        if let Some((exe, h, _)) = now.used.iter().find(|(e, _, _)| format!("app:{}", e.to_lowercase()) == *subject) {
            return Some(Nudge { subject: subject.clone(), kind: "app".into(), exe: exe.clone(), name: (now.name_of)(exe), hours: *h });
        }
    }
    let nudge = if let Some(kind) = new_kind.filter(|k| !quiet(&format!("module:{k}"))) {
        Some(Nudge { subject: format!("module:{kind}"), kind: "module".into(), exe: kind.clone(), name: kind, hours: 0.0 })
    } else {
        now.used
            .iter()
            .filter(|(exe, h, d)| (*h * 3600.0) as u64 >= MIN_SECONDS && *d >= MIN_DAYS && !super::assistant::not_for_a_deck(exe))
            .filter(|(exe, _, _)| !now.linked.iter().any(|l| l.eq_ignore_ascii_case(exe)))
            .filter(|(exe, _, _)| !quiet(&format!("app:{}", exe.to_lowercase())))
            .map(|(exe, h, _)| Nudge { subject: format!("app:{}", exe.to_lowercase()), kind: "app".into(), exe: exe.clone(), name: (now.name_of)(exe), hours: *h })
            .next()
    };
    if let Some(n) = &nudge {
        f.today = Some((now.today.clone(), n.subject.clone(), false));
    }
    nudge
}

/// The window asks now and then (when the Assistant tab shows, and every few minutes).
#[tauri::command]
pub async fn nudge_get(app: AppHandle) -> Option<Nudge> {
    let (busy, linked, foreground) = {
        let engine = app.state::<Engine>();
        let s = engine.lock();
        let st = s.state();
        let linked: Vec<String> = s.store.profiles.values().flat_map(|p| p.apps.clone()).collect();
        (st.in_call || s.trying.is_some(), linked, st.foreground)
    };
    let attached = super::api::module_kinds(&app).attached;
    tauri::async_runtime::spawn_blocking(move || {
        let busy = busy || (!foreground.is_empty() && super::starter::is_game(&foreground, ""));
        let name_of = |exe: &str| super::assistant::app_name(exe);
        let now = Now { today: day(0), busy, attached: &attached, linked: &linked, used: super::usage::top(30), name_of: &name_of };
        let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut f = read();
        let n = choose(&mut f, &now);
        write(&f);
        n
    })
    .await
    .ok()
    .flatten()
}

/// What the user did with it: `later` (7 quiet days), `never` (30), `off` (no more suggestions),
/// `done` (they took it: a sketch or a talk).
#[tauri::command]
pub fn nudge_act(subject: String, action: String) -> Result<(), String> {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut f = read();
    match action.as_str() {
        "later" => {
            f.quiet.insert(subject.clone(), day(7));
        }
        "never" => {
            f.quiet.insert(subject.clone(), day(30));
        }
        "off" => f.off = true,
        "done" => {}
        other => return Err(format!("unknown action {other}")),
    }
    if let Some(t) = f.today.as_mut().filter(|t| t.1 == subject) {
        t.2 = true;
    }
    if let Some(kind) = subject.strip_prefix("module:")
        && let Some(seen) = f.seen_kinds.as_mut()
        && !seen.iter().any(|k| k == kind)
    {
        seen.push(kind.to_string());
    }
    write(&f);
    Ok(())
}

/// Suggestions on or off (the Assistant tab).
#[tauri::command]
pub fn nudge_enable(on: bool) {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut f = read();
    f.off = !on;
    write(&f);
}

#[tauri::command]
pub fn nudge_enabled() -> bool {
    !read().off
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now<'a>(today: &str, busy: bool, attached: &'a [String], linked: &'a [String], used: Vec<(String, f32, u32)>) -> Now<'a> {
        Now { today: today.into(), busy, attached, linked, used, name_of: &|e: &str| e.trim_end_matches(".exe").to_string() }
    }

    #[test]
    fn one_suggestion_a_day_with_brakes() {
        let kinds = vec!["keys".to_string()];
        let blender = vec![("blender.exe".to_string(), 3.0, 4), ("short.exe".to_string(), 0.2, 9)];
        let mut f = File::default();
        assert!(choose(&mut f, &now("2026-10-12", true, &kinds, &[], blender.clone())).is_none(), "not during a call or a game");
        let n = choose(&mut f, &now("2026-10-12", false, &kinds, &[], blender.clone())).expect("Blender has no deck");
        assert_eq!((n.kind.as_str(), n.subject.as_str()), ("app", "app:blender.exe"));
        assert_eq!(choose(&mut f, &now("2026-10-12", false, &kinds, &[], blender.clone())).map(|n| n.subject), Some("app:blender.exe".into()), "the same one all day");
        f.quiet.insert("app:blender.exe".into(), "2026-10-19".into());
        f.today.as_mut().unwrap().2 = true;
        assert!(choose(&mut f, &now("2026-10-12", false, &kinds, &[], blender.clone())).is_none(), "nothing more today");
        assert!(choose(&mut f, &now("2026-10-13", false, &kinds, &[], blender.clone())).is_none(), "quiet for a week");
        assert!(choose(&mut f, &now("2026-10-20", false, &kinds, &[], blender.clone())).is_some(), "back after it");
        let linked = vec!["Blender.exe".to_string()];
        assert!(choose(&mut File::default(), &now("2026-10-20", false, &kinds, &linked, blender.clone())).is_none(), "it has a profile");
        let mut off = File { off: true, ..File::default() };
        assert!(choose(&mut off, &now("2026-10-20", false, &kinds, &[], blender)).is_none());
    }

    #[test]
    fn a_new_kind_of_module_is_news_once() {
        let mut f = File::default();
        let first = vec!["keys".to_string()];
        assert!(choose(&mut f, &now("2026-10-12", false, &first, &[], vec![])).is_none(), "what was there at the start is not news");
        let more = vec!["keys".to_string(), "faders".to_string()];
        let n = choose(&mut f, &now("2026-10-13", false, &more, &[], vec![])).expect("faders are new");
        assert_eq!(n.subject, "module:faders");
    }
}
