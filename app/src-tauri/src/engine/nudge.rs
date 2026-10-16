//! Suggestions outside the talk (both design reviews; the owner picked them, 2026-10-12): HubDeck
//! notices on its own, with no AI, that an app the user works in a lot has no deck yet, or that a
//! kind of module was attached for the first time, and offers a sketch made on this computer. The
//! model only comes in if the user asks for it.
//!
//! The day after the tour's deck was kept, if the user let HubDeck count presses, the card can
//! also be about that deck: a control never touched since ("knob 3 never turned, swap it?").
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

/// The tour's deck counts as used (so an untouched control means something) after this many presses.
const MIN_USES: u64 = 10;

/// An app counts as worked in after this long in front, on at least two days.
const MIN_SECONDS: u64 = 30 * 60;
const MIN_DAYS: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Nudge {
    /// `app:<exe>` or `module:<kind>`: what it is about (quiet days are kept per subject).
    pub subject: String,
    /// `app`, `module` or `unused` (a control of the tour's deck nobody touched).
    pub kind: String,
    /// The app's program and name, the module kind, or the control (`knobs.2`) and its label.
    pub exe: String,
    pub name: String,
    /// Hours in front (apps).
    pub hours: f32,
    /// `unused`: the deck's name and the day it was kept.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deck: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
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
    /// The deck the tour made and the user kept: (profile id, the day).
    #[serde(default)]
    tour_deck: Option<(String, String)>,
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
    /// Kinds the user has arranged before (placements): never news, attached now or not.
    pub remembered: &'a [String],
    /// Programs some profile (or draft) is linked to.
    pub linked: &'a [String],
    /// (program, hours, days) from `usage.json`, most first.
    pub used: Vec<(String, f32, u32)>,
    pub name_of: &'a dyn Fn(&str) -> String,
    /// The tour's kept deck as used since, if presses are counted: None otherwise.
    pub deck: Option<DeckUse>,
}

/// The tour's kept deck: its name, its controls (id, label) and how often each was pressed.
pub struct DeckUse {
    pub name: String,
    pub controls: Vec<(String, String)>,
    pub counts: BTreeMap<String, u64>,
}

impl DeckUse {
    /// The first control nobody touched, knobs, faders, the dial and rollers before keys (they
    /// are the fewest and the easiest to swap); nothing until the deck was really used.
    fn unused(&self, quiet: impl Fn(&str) -> bool) -> Option<(String, String)> {
        if self.counts.values().sum::<u64>() < MIN_USES {
            return None;
        }
        let rank = |c: &str| if c.starts_with("keys.") { 1 } else { 0 };
        let mut list: Vec<&(String, String)> = self.controls.iter().filter(|(c, _)| !c.contains('/') && self.counts.get(c).copied().unwrap_or(0) == 0 && !quiet(c)).collect();
        list.sort_by_key(|(c, _)| rank(c));
        list.first().map(|(c, l)| (c.clone(), l.clone()))
    }
}

/// The suggestion for now, if any (and the file updated: the day's subject, kinds seen).
fn choose(f: &mut File, now: &Now) -> Option<Nudge> {
    // the first time: the kinds attached now are known already, not news
    let seen = f.seen_kinds.get_or_insert_with(|| now.attached.to_vec());
    // a kind the user arranged before is known even if it was not attached at the first look
    // (owner, 2026-10-14: "the keys are attached for the first time", months after; the first look
    // came while the keys were still connecting)
    for k in now.remembered {
        if !seen.contains(k) {
            seen.push(k.clone());
        }
    }
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
            return Some(Nudge { subject: subject.clone(), kind: "module".into(), exe: kind.into(), name: kind.into(), hours: 0.0, deck: None, since: None });
        }
        if let Some((exe, h, _)) = now.used.iter().find(|(e, _, _)| format!("app:{}", e.to_lowercase()) == *subject) {
            return Some(Nudge { subject: subject.clone(), kind: "app".into(), exe: exe.clone(), name: (now.name_of)(exe), hours: *h, deck: None, since: None });
        }
        if let (Some(control), Some(deck), Some((_, since))) = (subject.strip_prefix("unused:"), &now.deck, &f.tour_deck)
            && let Some((_, label)) = deck.controls.iter().find(|(c, _)| c == control)
            && deck.counts.get(control).copied().unwrap_or(0) == 0
        {
            return Some(Nudge { subject: subject.clone(), kind: "unused".into(), exe: control.into(), name: label.clone(), hours: 0.0, deck: Some(deck.name.clone()), since: Some(since.clone()) });
        }
    }
    // the tour's deck: from the day after it was kept
    let unused = match (&now.deck, &f.tour_deck) {
        (Some(deck), Some((_, since))) if since.as_str() < now.today.as_str() => deck.unused(|c| quiet(&format!("unused:{c}"))).map(|(c, l)| (c, l, deck.name.clone(), since.clone())),
        _ => None,
    };
    let nudge = if let Some(kind) = new_kind.filter(|k| !quiet(&format!("module:{k}"))) {
        Some(Nudge { subject: format!("module:{kind}"), kind: "module".into(), exe: kind.clone(), name: kind, hours: 0.0, deck: None, since: None })
    } else if let Some((control, label, deck, since)) = unused {
        Some(Nudge { subject: format!("unused:{control}"), kind: "unused".into(), exe: control, name: label, hours: 0.0, deck: Some(deck), since: Some(since) })
    } else {
        now.used
            .iter()
            .filter(|(exe, h, d)| (*h * 3600.0) as u64 >= MIN_SECONDS && *d >= MIN_DAYS && !super::assistant::not_for_a_deck(exe))
            .filter(|(exe, _, _)| !now.linked.iter().any(|l| l.eq_ignore_ascii_case(exe)))
            .filter(|(exe, _, _)| !quiet(&format!("app:{}", exe.to_lowercase())))
            .map(|(exe, h, _)| Nudge { subject: format!("app:{}", exe.to_lowercase()), kind: "app".into(), exe: exe.clone(), name: (now.name_of)(exe), hours: *h, deck: None, since: None })
            .next()
    };
    if let Some(n) = &nudge {
        f.today = Some((now.today.clone(), n.subject.clone(), false));
        // a kind is news on one day only: a module that drops off USB and comes back (a loose
        // contact) is not "attached for the first time" every day (owner, 2026-10-16: the dot on
        // Get started never went away)
        if n.kind == "module"
            && let Some(seen) = f.seen_kinds.as_mut()
            && !seen.contains(&n.exe)
        {
            seen.push(n.exe.clone());
        }
    }
    nudge
}

/// The window asks now and then (when the Assistant tab shows, and every few minutes).
#[tauri::command]
pub async fn nudge_get(app: AppHandle) -> Option<Nudge> {
    // the modules connect in the first seconds: no judging what is attached before that
    if super::saver::clock() < 20.0 {
        return None;
    }
    let tour_deck = {
        let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        read().tour_deck.map(|(id, _)| id)
    };
    let (busy, linked, foreground, deck) = {
        let engine = app.state::<Engine>();
        let s = engine.lock();
        let st = s.state();
        let linked: Vec<String> = s.store.profiles.values().flat_map(|p| p.apps.clone()).collect();
        // the tour's deck, only if presses are counted (the user's yes) and it is still there
        let deck = tour_deck.filter(|_| s.store.settings.count_presses).and_then(|id| {
            let p = s.store.profiles.get(&id)?;
            let controls = p.controls.iter().map(|(c, b)| (c.clone(), b.look.label.clone().filter(|l| !l.is_empty()).unwrap_or_else(|| b.action.clone()))).collect();
            let counts = super::presses::counts(&id).into_iter().map(|(c, n)| (c, n.count)).collect();
            Some(DeckUse { name: p.name.clone(), controls, counts })
        });
        (st.in_call || s.trying.is_some(), linked, st.foreground, deck)
    };
    let kinds = super::api::module_kinds(&app);
    let (attached, remembered): (Vec<String>, Vec<String>) = (kinds.attached.clone(), kinds.owned.iter().filter(|k| !kinds.attached.contains(k)).cloned().collect());
    tauri::async_runtime::spawn_blocking(move || {
        let busy = busy || (!foreground.is_empty() && super::starter::is_game(&foreground, ""));
        let name_of = |exe: &str| super::assistant::app_name(exe);
        let now = Now { today: day(0), busy, attached: &attached, remembered: &remembered, linked: &linked, used: super::usage::top(30), name_of: &name_of, deck };
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

/// The tour's deck was kept today: from tomorrow its untouched controls can be suggested for a swap.
pub fn tour_kept(profile: &str) {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut f = read();
    f.tour_deck = Some((profile.to_string(), day(0)));
    write(&f);
}

/// Suggestions on or off (the Assistant tab).
/// A deck from the tour was kept (Get started shows the tour as done then).
#[tauri::command]
pub fn tour_done() -> bool {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    read().tour_deck.is_some()
}

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
        Now { today: today.into(), busy, attached, remembered: &[], linked, used, name_of: &|e: &str| e.trim_end_matches(".exe").to_string(), deck: None }
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
    fn the_tour_deck_s_untouched_knob_is_suggested_from_the_next_day() {
        let deck = || DeckUse {
            name: "My deck".into(),
            controls: vec![("keys.0".into(), "Chrome".into()), ("knobs.2".into(), "Music".into()), ("knobs.0".into(), "Game".into()), ("fun/keys.1".into(), String::new())],
            counts: [("keys.1".to_string(), 9), ("knobs.0".to_string(), 3)].into_iter().collect(),
        };
        let kinds = vec!["keys".to_string()];
        let mut f = File { seen_kinds: Some(kinds.clone()), tour_deck: Some(("p".into(), "2026-10-13".into())), ..File::default() };
        let at = |today: &str, deck: Option<DeckUse>, f: &mut File| choose(f, &Now { deck, ..now(today, false, &kinds, &[], vec![]) });
        assert!(at("2026-10-13", Some(deck()), &mut f).is_none(), "not on the day it was kept");
        let n = at("2026-10-14", Some(deck()), &mut f).expect("knob 3 was never turned");
        assert_eq!((n.kind.as_str(), n.exe.as_str(), n.name.as_str()), ("unused", "knobs.2", "Music"));
        assert_eq!(n.since.as_deref(), Some("2026-10-13"));
        let mut little = deck();
        little.counts = [("keys.1".to_string(), 2)].into_iter().collect();
        assert!(at("2026-10-15", Some(little), &mut File { tour_deck: f.tour_deck.clone(), seen_kinds: Some(kinds.clone()), ..File::default() }).is_none(), "barely used: nothing to say yet");
        assert!(at("2026-10-15", None, &mut File { tour_deck: f.tour_deck.clone(), seen_kinds: Some(kinds.clone()), ..File::default() }).is_none(), "no counting, no card");
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

    #[test]
    fn a_kind_arranged_before_is_not_news() {
        // the first look came while the keys were still connecting; they were arranged long ago
        let mut f = File::default();
        let early = vec!["dial".to_string()];
        assert!(choose(&mut f, &now("2026-10-12", false, &early, &[], vec![])).is_none());
        let all = vec!["dial".to_string(), "keys".to_string()];
        let remembered = vec!["keys".to_string()];
        assert!(choose(&mut f, &Now { remembered: &remembered, ..now("2026-10-13", false, &all, &[], vec![]) }).is_none(), "the keys are known");
    }
}
