//! The deck assistant (owner, 2026-10-12, roadmap phase 11): a conversation with Claude Code
//! running without a window, with only HubDeck's MCP tools. It gets to know the user (what they
//! have and use, from a scan they see first; an interview), proposes which profiles to make and
//! builds two or three versions of each as drafts, which the user tries on the deck and picks.
//!
//! Each turn is one `claude -p` run (`--resume` keeps the conversation); its stream of events goes
//! to the window as `assistant://event`. Nothing is sent before the user starts it, and the first
//! message carries exactly the scan text the window showed.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Stdio};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, Manager};

use super::Engine;
use super::profile::Profile;

pub const EVENT: &str = "assistant://event";

/// The conversation: Claude Code's session id once the first turn answered, and its process.
/// One process serves the whole talk (`--input-format stream-json`: each message is a line on its
/// stdin), so a turn does not start Claude Code cold; after a stop or a new model it is started
/// again with `--resume`.
struct Talk {
    session: Option<String>,
    /// The model Claude Code said it uses (its first line of every turn).
    model: Option<String>,
    /// Tokens in the context after the last call, and the model's context window.
    context: (u64, u64),
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    /// A turn is running (from a message sent to its result).
    busy: bool,
    /// Which process the reader thread serves (an old one ending must not touch a new one).
    run: u64,
}

static TALK: Mutex<Talk> = Mutex::new(Talk { session: None, model: None, context: (0, 0), child: None, stdin: None, busy: false, run: 0 });

fn talk() -> std::sync::MutexGuard<'static, Talk> {
    TALK.lock().unwrap_or_else(|e| e.into_inner())
}

/// What the window shows while a turn runs and when it ends.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AssistantEvent {
    /// The assistant said something (a whole message; the window adds it to the chat), with the
    /// answers it offers as buttons and the ideas it proposes as cards.
    Text { text: String, choices: Vec<String>, ideas: Vec<Idea> },
    /// A piece of the message being written (the window shows it growing; `Text` replaces it).
    Delta { text: String },
    /// It proposed an idea (`propose_idea`): HubDeck made a draft of its sketch at once (None when
    /// nothing of it checked out), and these controls were left out, with why.
    Idea { idea: Idea, draft: Option<String>, dropped: Vec<String> },
    /// Answers it offers as buttons (`offer_choices`).
    Choices { choices: Vec<String>, multiple: bool },
    /// It remembered something about the user's taste (`remember`; shown, can be removed).
    Remembered { text: String },
    /// More power: Claude Code wants to use one of its own tools; the window asks the user.
    Permission { id: u64, tool: String, detail: String },
    /// It used a tool (shown as a short line: "making a draft", "looking at your modules").
    Tool { name: String },
    /// The turn ended.
    Done,
    /// The turn failed (`no-cli` = Claude Code is not installed).
    Error { message: String },
    /// How full the conversation's context is: tokens in it now and the model's window.
    Context { used: u64, window: u64 },
    /// The model Claude Code runs this talk on (from its first line), for the bar above the talk.
    Model { model: String },
}

/// What the scan found, and the exact text the first message sends (shown to the user first).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Scan {
    /// Apps the user spends time in, most first: (name, hours, days).
    pub used: Vec<(String, f32, u32)>,
    /// Since when that is counted (local time).
    pub since: String,
    /// Attached to the base now (the assistant builds for these).
    pub modules: Vec<String>,
    /// Owned but not attached now.
    pub detached: Vec<String>,
    pub profiles: Vec<String>,
    /// What goes with the first message: the apps used most (names, hours), the deck, the taste
    /// notes (owner's pick 2a, 2026-10-14: the short consent says exactly this).
    pub text: String,
    /// The same without the app list ("Send without the app list").
    pub deck_text: String,
}

/// A friendly name for an exe the user has used (its own name when HubDeck has seen its path).
fn name_of(exe: &str) -> String {
    super::apps::path_of_seen(exe).and_then(|p| super::apps::friendly_name(&p)).unwrap_or_else(|| exe.trim_end_matches(".exe").trim_end_matches(".EXE").to_string())
}

/// Look at what the user has and uses; nothing leaves the computer here.
#[tauri::command]
pub async fn assistant_scan(app: AppHandle) -> Result<Scan, String> {
    let kinds = super::api::module_kinds(&app);
    let modules = kinds.attached.clone();
    let detached: Vec<String> = kinds.owned.into_iter().filter(|k| !kinds.attached.contains(k)).collect();
    let profiles: Vec<String> = {
        let engine = app.state::<Engine>();
        let s = engine.lock();
        s.store.profiles.values().filter(|p| !s.store.settings.drafts.contains(&p.id)).map(|p| p.name.clone()).collect()
    };
    tauri::async_runtime::spawn_blocking(move || {
        let used: Vec<(String, f32, u32)> = super::usage::top(25).into_iter().map(|(exe, h, d)| (name_of(&exe), h, d)).filter(|(_, h, _)| *h >= 0.05).collect();
        let since = super::usage::since();
        let taste = super::taste::text();
        let text = format!("{}\n{taste}", scan_text(Some((&used, &since)), &modules, &detached, &profiles));
        let deck_text = format!("{}\n{taste}", scan_text(None, &modules, &detached, &profiles));
        Scan { used, since, modules, detached, profiles, text, deck_text }
    })
    .await
    .map_err(|e| e.to_string())
}

/// An app used: (name, hours in front, days).
type Used = (String, f32, u32);

/// The scan as the assistant gets it: the apps used most (None: the user left them out) and the deck.
/// Installed apps and games are not in it; the assistant looks them up when an idea needs them.
fn scan_text(used: Option<(&[Used], &str)>, modules: &[String], detached: &[String], profiles: &[String]) -> String {
    let list = |v: &[String], max: usize| if v.is_empty() { "none".to_string() } else { v.iter().take(max).cloned().collect::<Vec<_>>().join(", ") };
    let apps = match used {
        Some(([], _)) => "\n- Apps used most: nothing measured yet (HubDeck counts it from now on)".to_string(),
        Some((used, since)) => {
            format!("\n- Apps used most (time in front, counted since {since}): {}", used.iter().map(|(n, h, d)| format!("{n} ({h:.1} h on {d} days)")).collect::<Vec<_>>().join(", "))
        }
        None => "\n- Apps used most: the user chose not to send them (ask what they use, or look up installed apps with list_apps)".to_string(),
    };
    format!(
        "What HubDeck found on this computer (the user saw this before it was sent):{apps}\n- Deck modules attached now (build for these): {}\n- Owned but not attached now: {}\n- Profiles the user has now: {}",
        list(modules, 10),
        list(detached, 10),
        list(profiles, 40)
    )
}

/// How the assistant works (owner: proactive and inventive, show on the deck, not a script):
/// `assistant-rules.txt`, with the house look and the language filled in. This is the whole
/// system prompt of its sessions.
const RULES: &str = include_str!("assistant-rules.txt");

fn rules(language: &str) -> String {
    RULES.replace("{HOUSE}", &super::mcp::house_rule()).replace("{LANGUAGE}", language)
}

/// An idea the assistant proposes (its `<ideas>` block); the window shows it as a card.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct Idea {
    pub title: String,
    pub app: String,
    pub pitch: String,
    pub extras: Vec<String>,
    /// A few sample keys, drawn on the card in the house look.
    pub keys: Vec<SampleKey>,
    /// The whole deck as a sketch: `{keys: [binding x15], dial, knobs: [x3], rollers: [x2],
    /// faders: [x5], edge}`, each binding `{action, params, label, icon}`. HubDeck makes a
    /// draft of it at once, so it can be seen and tried without another turn.
    pub deck: Option<Value>,
    /// The id of the user's own profile this idea is a new version of ("" = a new deck).
    #[serde(rename = "versionOf", skip_serializing_if = "String::is_empty")]
    pub version_of: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct SampleKey {
    pub label: String,
    pub icon: String,
}

/// A draft profile from an idea's sketch, in the house look; controls that do not check out
/// (unknown action, icon or params) are left out rather than refusing the whole sketch.
pub fn sketch_profile(idea: &Idea) -> (Option<Profile>, Vec<String>) {
    let mut dropped = Vec::new();
    let Some(deck) = idea.deck.as_ref() else { return (None, dropped) };
    let mut controls = serde_json::Map::new();
    let mut put = |id: String, b: &Value| {
        let Some(action) = b["action"].as_str().filter(|a| !a.is_empty()) else { return };
        let mut look = serde_json::Map::new();
        for k in ["label", "icon"] {
            if let Some(v) = b[k].as_str().filter(|v| !v.is_empty()) {
                look.insert(k.into(), json!(v));
            }
        }
        controls.insert(id, json!({"action": action, "params": if b["params"].is_object() { b["params"].clone() } else { json!({}) }, "look": look}));
    };
    for (kind, n) in [("keys", 15), ("knobs", 3), ("rollers", 2), ("faders", 5)] {
        for (i, b) in deck[kind].as_array().into_iter().flatten().take(n).enumerate() {
            put(format!("{kind}.{i}"), b);
        }
    }
    if deck["dial"].is_object() {
        put("dial".into(), &deck["dial"]);
    }
    let name = format!("{} · {}", idea.title.trim(), "sketch");
    let apps: Vec<String> = Some(idea.app.trim()).filter(|a| !a.is_empty()).map(str::to_string).into_iter().collect();
    let p: Option<Profile> = serde_json::from_value(json!({
        "version": super::profile::PROFILE_VERSION,
        "id": uuid::Uuid::new_v4().to_string(),
        "name": name,
        "apps": apps,
        "controls": controls,
    }))
    .ok();
    let Some(mut p) = p else { return (None, dropped) };
    p.assistant = true;
    p.version_of = Some(idea.version_of.clone()).filter(|v| !v.is_empty());
    with_real_apps(&mut p);
    p.edge = deck["edge"].as_str().filter(|e| e.len() == 7 && e.starts_with('#')).map(str::to_string);
    let apps = p.apps.clone();
    p.controls.retain(|id, b| {
        let (mut errors, mut warnings) = (Vec::new(), Vec::new());
        super::api::check_binding(id, b, &[], &mut errors, &mut warnings);
        if let Err(e) = super::guard::check(id, b, &apps) {
            errors.push(e);
        }
        // a key naming an app or game this computer does not have would not open: better empty
        // (persona check, session 6: "Media Player" for a user without it, not fixed when noted)
        if let Some(name) = unknown_app(b) {
            errors.push(format!("{id}: {name:?} is not an app or game on this computer (list_apps, list_games)"));
        }
        dropped.extend(errors.iter().cloned());
        errors.is_empty()
    });
    if p.controls.is_empty() {
        return (None, dropped);
    }
    super::house::apply(&mut p);
    (Some(p), dropped)
}

/// Keys that open an app or a game get the real thing (owner, session 6: a sketch promised game
/// covers and showed only text; keys named "Visual Studio Code" could not open): a name becomes
/// the installed app's or game's target, and a key without a picture gets the app's icon or the
/// game's cover at once.
pub(crate) fn with_real_apps(p: &mut Profile) {
    let is_target = |t: &str| t.contains("://") || t.contains('\\') || t.contains('/') || t.starts_with("shell:");
    for b in p.controls.values_mut() {
        // a key with only a label looks unfinished (owner: "same teksty"): its action's icon
        let bare = b.look.icon.as_deref().unwrap_or_default().is_empty() && b.look.image.as_deref().unwrap_or_default().is_empty();
        if bare && let Some(def) = super::actions::find(&b.action).filter(|d| super::render::icon_svg(d.icon).is_some()) {
            b.look.icon = Some(def.icon.to_string());
        }
        let key = match b.action.as_str() {
            "core.launch" => "target",
            "core.game" => "game",
            _ => continue,
        };
        let named = super::actions::param_str(&b.params, key).trim().to_string();
        let item = if is_target(&named) {
            super::installed::cached_items().into_iter().find(|i| i.target.eq_ignore_ascii_case(&named))
        } else {
            super::installed::item_named(&named).or_else(|| b.look.label.as_deref().and_then(|l| super::installed::item_named(&l.replace('\n', " "))))
        };
        let Some(item) = item else {
            // not installed but seen in front: the user's own tools
            if key == "target"
                && !is_target(&named)
                && let Some(path) = super::apps::seen_named(&named).or_else(|| b.look.label.as_deref().and_then(|l| super::apps::seen_named(&l.replace('\n', " "))))
            {
                b.params[key] = json!(path);
                if b.look.image.as_deref().unwrap_or_default().is_empty()
                    && let Some(image) = super::appicon::png(&path).ok().and_then(|png| super::images::import_bytes(&png, "png").ok())
                {
                    b.look.image = Some(image);
                    b.look.icon = None;
                }
            }
            continue;
        };
        if !is_target(&named) {
            b.params[key] = json!(item.target);
        }
        if b.look.image.as_deref().unwrap_or_default().is_empty()
            && let Ok(image) = super::installed::picture(&item)
        {
            // the picture stays on Auto: app icons follow Settings > Screens > App icons
            b.look.image = Some(image);
            b.look.icon = None;
        }
    }
}

/// The bare name of an app or game a key would open but this computer does not have (not a
/// path, not a link: `core.launch` with "Media Player" where there is none).
fn unknown_app(b: &super::profile::Binding) -> Option<String> {
    let opens = match b.action.as_str() {
        "core.launch" => super::actions::param_str(&b.params, "target"),
        "core.game" => super::actions::param_str(&b.params, "game"),
        _ => return None,
    }
    .trim()
    .to_string();
    (!opens.is_empty() && !opens.contains([':', '\\', '/']) && !super::check::name_opens(&opens)).then_some(opens)
}

/// Save an idea's sketch as a draft; (draft id, controls left out with why).
fn save_sketch(engine: &Engine, idea: &Idea) -> Result<(Option<String>, Vec<String>), String> {
    let (p, dropped) = sketch_profile(idea);
    let Some(p) = p else { return Ok((None, dropped)) };
    let id = p.id.clone();
    {
        let mut s = engine.lock();
        s.store.save_profile(p).map_err(|e| e.to_string())?;
        s.store.settings.drafts.push(id.clone());
        s.store.save_settings().map_err(|e| e.to_string())?;
    }
    engine.changed();
    Ok((Some(id), dropped))
}

/// `propose_idea` (through the local API): a draft of its sketch at once, the card in the window
/// at once, and what was left out back to the model to fix.
pub fn api_idea(app: &AppHandle, body: &Value) -> Result<Value, String> {
    let idea = idea_from(body);
    if idea.title.is_empty() {
        return Err("an idea needs a title".into());
    }
    let engine = app.state::<Engine>();
    let (draft, dropped) = if idea.deck.is_some() { save_sketch(&engine, &idea)? } else { (None, Vec::new()) };
    let _ = app.emit(EVENT, AssistantEvent::Idea { idea: idea.clone(), draft: draft.clone(), dropped: dropped.clone() });
    let to_fix = draft.as_ref().and_then(|id| engine.lock().store.profiles.get(id).map(sketch_notes)).unwrap_or_default();
    Ok(json!({
        "draft": draft,
        "left_out": dropped,
        "to_fix": to_fix,
        "note": if dropped.is_empty() && to_fix.is_empty() { "Every control checked out; the user sees the card and can try it on the deck." } else { "Fix these in the draft with set_control (left out: they are not in it; to fix: they are in it but weak)." },
    }))
}

/// What is in a sketch but weak, for the model to fix: labels too long for a key, shortcuts not
/// in a checked pack for its apps (persona checks, session 6: Polish labels ran long, a
/// whole-desk idea used Excel shortcuts).
fn sketch_notes(p: &Profile) -> Vec<String> {
    let mut out = Vec::new();
    for (id, b) in &p.controls {
        let (mut errors, mut warnings) = (Vec::new(), Vec::new());
        super::api::check_binding(id, b, &p.pages, &mut errors, &mut warnings);
        out.extend(warnings);
        for combo in super::guard::unchecked_shortcuts(&b.action, &b.params, &p.apps) {
            out.push(format!(
                "{id}: {combo} is not a checked shortcut for {} (list_shortcuts); use another action or leave the key out",
                if p.apps.is_empty() { "a whole-desk profile (only Windows' own count)".to_string() } else { p.apps.join(", ") }
            ));
        }
    }
    out
}

/// `offer_choices`: answers as buttons under the message.
pub fn api_choices(app: &AppHandle, body: &Value) -> Result<Value, String> {
    let choices: Vec<String> = body["choices"].as_array().into_iter().flatten().filter_map(|c| c.as_str()).map(|c| c.trim().to_string()).filter(|c| !c.is_empty()).take(8).collect();
    if choices.is_empty() {
        return Err("give two to eight short answers".into());
    }
    let _ = app.emit(EVENT, AssistantEvent::Choices { choices, multiple: body["multiple"].as_bool().unwrap_or(false) });
    Ok(json!({"ok": true}))
}

/// Programs nobody makes a deck for (the Windows shell, HubDeck, terminals, system tools).
pub fn not_for_a_deck(exe: &str) -> bool {
    let e = exe.to_lowercase();
    [
        "hubdeck",
        "explorer.exe",
        "searchhost",
        "searchapp",
        "shellexperiencehost",
        "startmenuexperiencehost",
        "applicationframehost",
        "lockapp",
        "taskmgr",
        "cmd.exe",
        "windowsterminal",
        "powershell",
        "conhost",
        "systemsettings",
        "claude.exe",
        "rundll32",
        "dllhost",
    ]
    .iter()
    .any(|s| e.contains(s))
}

/// The apps used most that could get a deck (no profile yet first): (program, name, path).
/// An app's name for people: its own (from the program seen), the installed app's, or the
/// program's name capitalised.
pub fn app_name(exe: &str) -> String {
    let file = |p: &str| p.rsplit(['\\', '/']).next().unwrap_or(p).to_lowercase();
    super::apps::path_of_seen(exe)
        .and_then(|p| super::apps::friendly_name(&p))
        .or_else(|| super::installed::list(false).apps.into_iter().find(|a| file(&a.exe) == exe.to_lowercase()).map(|a| a.name))
        .unwrap_or_else(|| {
            let stem = exe.trim_end_matches(".exe").trim_end_matches(".EXE");
            let mut c = stem.chars();
            c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
        })
}

pub(crate) fn top_apps(have_profile: &[String], n: usize) -> Vec<(String, String, Option<String>)> {
    let mut list: Vec<(String, String, Option<String>, bool)> = super::usage::top(30)
        .into_iter()
        .filter(|(exe, h, _)| *h >= 0.05 && !not_for_a_deck(exe))
        .map(|(exe, _, _)| {
            let path = super::apps::path_of_seen(&exe);
            let name = app_name(&exe);
            let has = have_profile.iter().any(|a| a.eq_ignore_ascii_case(&exe));
            (exe, name, path, has)
        })
        .collect();
    // nothing measured yet (a new install): the apps seen in front lately
    if list.is_empty() {
        for a in super::apps::list().into_iter().filter(|a| !not_for_a_deck(&a.exe)) {
            let has = have_profile.iter().any(|x| x.eq_ignore_ascii_case(&a.exe));
            let name = a.name.clone().unwrap_or_else(|| a.exe.trim_end_matches(".exe").to_string());
            list.push((a.exe, name, Some(a.path).filter(|p| !p.is_empty()), has));
        }
    }
    list.sort_by_key(|x| x.3);
    list.into_iter().take(n).map(|(e, n, p, _)| (e, n, p)).collect()
}

/// One app's quick sketch as a draft (its preset or a starter, for the attached modules, in the
/// house look); the caller saves the settings.
fn sketch_app(engine: &Engine, kinds: &[String], exe: &str, name: &str, path: Option<String>, polish: bool) -> Option<(Idea, String)> {
    // the program seen, else the installed app or game (a game seen only by its program's name
    // would not open from that name: persona check, session 6)
    let target = path.or_else(|| super::installed::target_named(exe)).or_else(|| super::installed::target_named(name)).unwrap_or_else(|| exe.to_string());
    let mut p = super::starter::for_app(exe, name, &target, None, if kinds.is_empty() { None } else { Some(kinds) });
    p.id = uuid::Uuid::new_v4().to_string();
    p.name = format!("{name} · {}", if polish { "szybki szkic" } else { "quick sketch" });
    p.in_bar = None;
    // only the modules attached now (a starter fills faders too)
    if !kinds.is_empty() {
        p.controls.retain(|id, _| {
            let base = id.rsplit('/').next().unwrap_or(id);
            let kind = base.split(['.', ':']).next().unwrap_or(base);
            kinds.iter().any(|k| k == kind)
        });
    }
    super::house::apply(&mut p);
    let pitch = if polish {
        format!("Gotowy od razu, bez AI: {name}, jego skróty, multimedia i głośność na gałkach.")
    } else {
        format!("Ready at once, no AI: {name}, its shortcuts, media and volume on the knobs.")
    };
    let idea = Idea { title: p.name.clone(), app: exe.to_string(), pitch, ..Idea::default() };
    let id = p.id.clone();
    let mut s = engine.lock();
    s.store.save_profile(p).ok()?;
    s.store.settings.drafts.push(id.clone());
    Some((idea, id))
}

/// A suggestion's "Show me a sketch": that app's quick sketch as a draft; its id.
#[tauri::command]
pub async fn assistant_sketch_app(app: AppHandle, exe: String, name: String, language: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let kinds = super::api::module_kinds(&app).attached;
        let engine = app.state::<Engine>();
        let path = super::apps::path_of_seen(&exe);
        let (_, id) = sketch_app(&engine, &kinds, &exe, &name, path, language == "pl").ok_or("could not make a sketch")?;
        engine.lock().store.save_settings().map_err(|e| e.to_string())?;
        engine.changed();
        Ok(id)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// A personal HubDeck tour as a draft, made on this computer at once (it took the model ~6 min):
/// "Discover HubDeck" with the user's own apps (their real icons) on the first page and a page of
/// their games with covers. The model only walks them through it.
pub fn personal_tour(app: &AppHandle, polish: bool) -> Result<String, String> {
    super::demo::set_language(if polish { "pl" } else { "en" });
    let mut p = super::demo::discover();
    p.id = uuid::Uuid::new_v4().to_string();
    p.name = if polish { "Twoja wycieczka po HubDecku".into() } else { "Your HubDeck tour".into() };
    let launch = |target: &str, label: &str, image: Option<String>| -> super::profile::Binding {
        let short: String = label.chars().take(12).collect();
        serde_json::from_value(json!({"action": "core.launch", "params": {"target": target}, "look": {"label": short, "image": image}})).unwrap_or_else(|_| super::profile::Binding {
            action: String::new(),
            params: json!({}),
            look: Default::default(),
            accel: false,
            look_on: None,
            swap_turns: false,
        })
    };
    // the example apps of the first page (Notepad, a website, Calculator) become their own
    for ((exe, name, path), key) in top_apps(&[], 3).into_iter().zip(["keys.1", "keys.2", "keys.10"]) {
        let image = super::api::picture_of(&exe).or_else(|_| super::api::picture_of(&name)).ok();
        p.controls.insert(key.into(), launch(path.as_deref().unwrap_or(&exe), &name, image));
    }
    let games = super::installed::list(false).games;
    if !games.is_empty() {
        let page = super::profile::Page { id: "games".into(), name: if polish { "Twoje gry".into() } else { "Your games".into() } };
        p.pages.push(page);
        for (i, g) in games.iter().take(14).enumerate() {
            let cover = super::installed::picture(g).ok();
            let b = serde_json::from_value(json!({"action": "core.game", "params": {"game": g.target}, "look": {"label": g.name.chars().take(12).collect::<String>(), "image": cover}}));
            if let Ok(b) = b {
                p.controls.insert(format!("games/keys.{i}"), b);
            }
        }
        if let Ok(back) = serde_json::from_value(json!({"action": "core.page", "params": {"page": ""}, "look": {"label": if polish { "Wróć" } else { "Back" }, "icon": "arrow-left"}})) {
            p.controls.insert("games/keys.14".into(), back);
        }
        // the "type text" example of the first page opens their games instead
        if let Ok(open) = serde_json::from_value(json!({"action": "core.page", "params": {"page": "games"}, "look": {"label": if polish { "Twoje gry" } else { "Your games" }, "icon": "gamepad-2"}})) {
            p.controls.insert("keys.4".into(), open);
        }
    }
    let id = p.id.clone();
    let engine = app.state::<Engine>();
    {
        let mut s = engine.lock();
        s.store.save_profile(p).map_err(|e| e.to_string())?;
        s.store.settings.drafts.push(id.clone());
        s.store.save_settings().map_err(|e| e.to_string())?;
    }
    engine.changed();
    Ok(id)
}

/// `make_tour`: the personal tour as a draft (the model then shows it with try_on_deck).
pub fn api_tour(app: &AppHandle, body: &Value) -> Result<Value, String> {
    let polish = body["language"].as_str().is_some_and(|l| l.to_lowercase().starts_with("pl"));
    let id = personal_tour(app, polish)?;
    Ok(
        json!({"draft": id, "note": "Made at once from their apps and games: the first page has their top apps with real icons, then pages for live values on the dial and keys, looks, control, animations, one picture across the keys, integrations, and their games with covers. Show it with try_on_deck and walk them through it in three or four short steps."}),
    )
}

/// `remember`: a short note about the user's taste, shown in the window (they can remove it).
pub fn api_remember(app: &AppHandle, body: &Value) -> Result<Value, String> {
    let text = super::taste::remember(body["text"].as_str().unwrap_or_default())?;
    let _ = app.emit(EVENT, AssistantEvent::Remembered { text });
    Ok(json!({"ok": true}))
}

/// Make drafts of the ideas' sketches; returns (title, draft id) of each made.
#[tauri::command]
pub fn sketches_make(engine: tauri::State<'_, Engine>, ideas: Vec<Idea>) -> Result<Vec<(String, String)>, String> {
    let mut made = Vec::new();
    for idea in &ideas {
        if let (Some(id), _) = save_sketch(&engine, idea)? {
            made.push((idea.title.clone(), id));
        }
    }
    Ok(made)
}

/// Put several drafts on the deck to compare, from the one at `at`: the dial flips between them
/// (None: stop).
#[tauri::command]
pub fn compare_drafts(engine: tauri::State<'_, Engine>, ids: Option<Vec<String>>, at: Option<usize>) -> Result<(), String> {
    engine.compare(ids.unwrap_or_default(), at.unwrap_or(0))
}

/// An idea from its JSON, taking a string where a list was meant and skipping what does not fit.
fn idea_from(v: &Value) -> Idea {
    let text = |k: &str| v[k].as_str().unwrap_or_default().trim().to_string();
    let list = |k: &str| match &v[k] {
        Value::String(s) if !s.trim().is_empty() => vec![s.trim().to_string()],
        Value::Array(a) => a.iter().filter_map(|x| x.as_str()).map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect(),
        _ => Vec::new(),
    };
    let keys = v["keys"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|k| SampleKey { label: k["label"].as_str().unwrap_or_default().to_string(), icon: k["icon"].as_str().unwrap_or_default().to_string() })
        .collect();
    Idea { title: text("title"), app: text("app"), pitch: text("pitch"), extras: list("extras"), keys, deck: v["deck"].is_object().then(|| v["deck"].clone()), version_of: text("versionOf") }
}

/// The `<ideas>[...]</ideas>` block of a message, split off: (text, ideas).
pub fn split_ideas(text: &str) -> (String, Vec<Idea>) {
    let Some(start) = text.find("<ideas>") else { return (text.to_string(), Vec::new()) };
    let rest = &text[start + "<ideas>".len()..];
    let Some(end) = rest.find("</ideas>") else { return (text.to_string(), Vec::new()) };
    // read each idea on its own and forgivingly: one odd field (extras as one string) must not
    // lose every idea and sketch of the answer
    let ideas: Vec<Idea> = serde_json::from_str::<Vec<Value>>(rest[..end].trim()).unwrap_or_default().iter().map(idea_from).collect();
    let mut body = text[..start].to_string();
    body.push_str(&rest[end + "</ideas>".len()..]);
    (body, ideas.into_iter().filter(|i| !i.title.trim().is_empty()).take(6).collect())
}

/// The `<choices>a|b</choices>` line of a message, split off: (text, answers).
pub fn split_choices(text: &str) -> (String, Vec<String>) {
    let Some(start) = text.rfind("<choices>") else { return (text.trim().to_string(), Vec::new()) };
    let rest = &text[start + "<choices>".len()..];
    let Some(end) = rest.find("</choices>") else { return (text.trim().to_string(), Vec::new()) };
    let choices = rest[..end].split('|').map(|c| c.trim().to_string()).filter(|c| !c.is_empty()).take(5).collect();
    let mut body = text[..start].to_string();
    body.push_str(&rest[end + "</choices>".len()..]);
    (body.trim().to_string(), choices)
}

/// What one line of Claude Code's `stream-json` output means for the window.
#[derive(Debug, Default, PartialEq)]
struct Read {
    events: Vec<AssistantEvent>,
    session: Option<String>,
    /// The turn ended (its result line).
    end: bool,
}

/// One line of Claude Code's `stream-json` output -> what to tell the window.
fn read_line(line: &str) -> Read {
    let Ok(v) = serde_json::from_str::<Value>(line) else { return Read::default() };
    let session = v["session_id"].as_str().map(str::to_string);
    let mut out = Vec::new();
    if v["type"] == "system"
        && let Some(model) = v["model"].as_str()
    {
        talk().model = Some(model.to_string());
        out.push(AssistantEvent::Model { model: model.to_string() });
    }
    match v["type"].as_str() {
        // the message as it is written (`--include-partial-messages`)
        Some("stream_event") if v["event"]["type"] == "content_block_delta" && v["event"]["delta"]["type"] == "text_delta" => {
            if let Some(text) = v["event"]["delta"]["text"].as_str().filter(|t| !t.is_empty()) {
                out.push(AssistantEvent::Delta { text: text.to_string() });
            }
        }
        Some("assistant") => {
            // what the model read and wrote in this call is what the context holds now
            let u = &v["message"]["usage"];
            let used: u64 = ["input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens", "output_tokens"].iter().filter_map(|k| u[k].as_u64()).sum();
            if used > 0 {
                let mut t = talk();
                t.context.0 = used;
                out.push(AssistantEvent::Context { used, window: t.context.1 });
            }
            for part in v["message"]["content"].as_array().into_iter().flatten() {
                match part["type"].as_str() {
                    Some("text") => {
                        let (text, ideas) = split_ideas(part["text"].as_str().unwrap_or_default());
                        let (text, choices) = split_choices(&text);
                        if !text.is_empty() || !choices.is_empty() || !ideas.is_empty() {
                            out.push(AssistantEvent::Text { text, choices, ideas });
                        }
                    }
                    Some("tool_use") => {
                        let name = part["name"].as_str().unwrap_or_default();
                        out.push(AssistantEvent::Tool { name: name.rsplit("__").next().unwrap_or(name).to_string() });
                    }
                    _ => {}
                }
            }
        }
        Some("result") => {
            if v["is_error"].as_bool() == Some(true) {
                out.push(AssistantEvent::Error { message: v["result"].as_str().unwrap_or("Claude Code stopped with an error").to_string() });
            } else if let Some(window) = v["modelUsage"].as_object().and_then(|m| m.values().find_map(|x| x["contextWindow"].as_u64())).filter(|w| *w > 0) {
                let mut t = talk();
                t.context.1 = window;
                out.push(AssistantEvent::Context { used: t.context.0, window });
            }
            return Read { events: out, session, end: true };
        }
        _ => {}
    }
    Read { events: out, session, end: false }
}

/// A user message as a line of Claude Code's `stream-json` input.
fn input_line(text: &str) -> String {
    json!({"type": "user", "message": {"role": "user", "content": text}}).to_string()
}

/// What the assistant can use, sent once with the first message (so it does not have to look it
/// up before its first idea, and it stays in the prompt cache): every action with its inputs and
/// params, the special hotkey names, the LED effects, the icons and the checked shortcut packs.
pub fn catalog_text() -> String {
    let mut lines = vec!["HubDeck's actions (id (name): inputs; params; list_actions with search gives a param's options and help):".to_string()];
    for a in super::actions::all().into_iter().filter(|a| super::games::GAMES_ENABLED || a.id != super::actions::GAMES) {
        let inputs: Vec<String> = a.inputs.iter().map(|i| format!("{i:?}").to_lowercase()).collect();
        let params: Vec<&str> = a.params.iter().map(|p| p.key).collect();
        lines.push(format!("- {} ({}): {}; {}", a.id, a.name, inputs.join("/"), if params.is_empty() { "no params".to_string() } else { params.join(", ") }));
    }
    lines.push(format!("Hotkey names besides letters, digits and F1-F12: {}.", super::hotkey::SPECIAL_KEYS.join(", ")));
    lines.push(format!("LED effects: {}.", super::leds::EFFECTS.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", ")));
    // the names themselves, so every key can have a fitting icon without a lookup first
    lines.push(format!(
        "Line icons (look.icon): {}. Game icons: {} more as game:<name> (list_icons with a search word).",
        super::icons::ICONS.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", "),
        super::game_icons::GAME_ICONS.len()
    ));
    lines.push(format!("Checked shortcut packs (list_shortcuts gives them): {}.", super::shortcuts::packs().iter().map(|p| p.app.as_str()).collect::<Vec<_>>().join(", ")));
    lines.join("\n")
}

/// Start Claude Code for this talk (resuming the session when there is one).
fn start(app: &AppHandle, language: &str) -> Result<(), String> {
    let exe = super::mcp::exe()?;
    let Some(cli) = super::mcp::find_claude_cli() else { return Err("no-cli".into()) };
    let session = talk().session.clone();
    let dir = std::env::temp_dir().join("hubdeck-assistant");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    // the assistant's own MCP server: only drafts can be changed, closing shortcuts go on a hold
    let mut entry = super::mcp::assistant_entry(&exe);
    if let Some(a) = crate::automation::get() {
        entry["env"] = json!({"HUBDECK_AUTOMATION": a.data_dir});
    }
    let config = dir.join("mcp.json");
    std::fs::write(&config, json!({"mcpServers": {super::mcp::SERVER_NAME: entry}}).to_string()).map_err(|e| e.to_string())?;
    let power = app.state::<Engine>().lock().store.settings.assistant_power;
    if power {
        entry["env"]["HUBDECK_ASSISTANT_POWER"] = json!("1");
        std::fs::write(&config, json!({"mcpServers": {super::mcp::SERVER_NAME: entry}}).to_string()).map_err(|e| e.to_string())?;
    }
    let mut rules = rules(language);
    if power {
        rules.push_str(POWER_RULES);
    }
    let mut cmd = std::process::Command::new(&cli);
    cmd.args(["-p", "--input-format", "stream-json", "--output-format", "stream-json", "--verbose", "--include-partial-messages", "--tools", if power { POWER_TOOLS } else { "" }, "--mcp-config"])
        .arg(&config)
        .args([
            "--strict-mcp-config",
            "--allowedTools",
            &format!("mcp__{}", super::mcp::SERVER_NAME),
            // its own system prompt (not Claude Code's coding one), and none of the user's own Claude
            // Code setup: no hooks, CLAUDE.md files, skills, permissions or auto-memory (owner: what
            // does the session get?). The sign-in stays the user's.
            "--system-prompt",
            &rules,
            "--setting-sources",
            "local",
            "--disable-slash-commands",
            "--settings",
            r#"{"autoMemoryEnabled":false}"#,
        ]);
    if let Some(id) = &session {
        cmd.args(["--resume", id]);
    }
    // more power: each command and web fetch of Claude Code's own tools is asked in the window
    if power {
        cmd.args(["--permission-prompt-tool", &format!("mcp__{}__permission_prompt", super::mcp::SERVER_NAME)]);
    }
    // only what the user picked in HubDeck; otherwise Claude Code's own default model and effort
    let (model, effort) = {
        let engine = app.state::<Engine>();
        let s = engine.lock();
        (s.store.settings.assistant_model.clone(), s.store.settings.assistant_effort.clone())
    };
    // the user's settings are not loaded, so their default model and effort are passed on
    let (default_model, default_effort) = claude_defaults();
    if let Some(m) = model.or(default_model) {
        cmd.args(["--model", &m]);
    }
    if let Some(e) = effort.or(default_effort) {
        cmd.args(["--effort", &e]);
    }
    cmd.current_dir(&dir).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = cmd.spawn().map_err(|e| format!("could not start Claude Code: {e}"))?;
    let stdout = child.stdout.take().ok_or("no output")?;
    // what Claude Code says when it cannot run (not signed in, no plan, a limit): shown as is
    let stderr = child.stderr.take();
    let errors = std::thread::spawn(move || {
        let mut text = String::new();
        if let Some(mut e) = stderr {
            let _ = std::io::Read::read_to_string(&mut e, &mut text);
        }
        text
    });
    let run = {
        let mut t = talk();
        t.run += 1;
        t.stdin = child.stdin.take();
        t.child = Some(child);
        t.run
    };
    let app = app.clone();
    std::thread::Builder::new()
        .name("assistant".into())
        .spawn(move || {
            let mut started = std::time::Instant::now();
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                let read = read_line(&line);
                if let Some(id) = read.session {
                    talk().session = Some(id);
                }
                for e in read.events {
                    let _ = app.emit(EVENT, e);
                }
                if read.end {
                    log::info!("assistant: turn finished in {:?}", started.elapsed());
                    started = std::time::Instant::now();
                    talk().busy = false;
                    let _ = app.emit(EVENT, AssistantEvent::Done);
                }
            }
            // the process ended: stopped, a new model picked, or it failed
            let (child, busy) = {
                let mut t = talk();
                if t.run != run {
                    return;
                }
                t.stdin = None;
                let busy = std::mem::replace(&mut t.busy, false);
                (t.child.take(), busy)
            };
            let ok = child.map(|mut c| c.wait().map(|s| s.success()).unwrap_or(false)).unwrap_or(true);
            let said = errors.join().unwrap_or_default();
            log::info!("assistant: Claude Code ended (ok: {ok})");
            if busy {
                if !ok {
                    let said: String = said.trim().chars().rev().take(400).collect::<Vec<_>>().into_iter().rev().collect();
                    let message = if said.is_empty() { "Claude Code did not answer (is it signed in? run `claude` once in a terminal)".to_string() } else { format!("Claude Code: {said}") };
                    let _ = app.emit(EVENT, AssistantEvent::Error { message });
                }
                let _ = app.emit(EVENT, AssistantEvent::Done);
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Send the user's message; the answer comes as events. The first message of a talk makes the
/// quick sketches at once (shown before the model says anything) and carries the scan text, the
/// user's taste, those sketches and the catalog.
#[tauri::command]
pub fn assistant_send(app: AppHandle, message: String, scan: Option<String>, language: String) -> Result<(), String> {
    if super::mcp::find_claude_cli().is_none() {
        return Err("no-cli".into());
    }
    let polish = language == "pl";
    let (first, running) = {
        let mut t = talk();
        if t.busy {
            return Err("the assistant is still answering".into());
        }
        t.busy = true;
        (t.session.is_none(), t.stdin.is_some())
    };
    std::thread::spawn(move || {
        let sent = (|| -> Result<(), String> {
            let prompt = match (first, scan) {
                // the first message: the scan, the catalog and the user's words; sketches come
                // from the interview (owner, 2026-10-12: "mocniejszy wywiad", picked "Wywiad, potem
                // szkice"), so no quick sketches here any more
                (true, Some(scan)) => format!("{scan}\n\n{}\n\nThe user says: {message}", catalog_text()),
                _ => message,
            };
            if !running {
                start(&app, if polish { "Polish" } else { "English" })?;
            }
            let mut t = talk();
            let Some(stdin) = t.stdin.as_mut() else { return Err("Claude Code did not start".into()) };
            writeln!(stdin, "{}", input_line(&prompt)).and_then(|_| stdin.flush()).map_err(|e| format!("Claude Code stopped: {e}"))
        })();
        if let Err(message) = sent {
            talk().busy = false;
            let _ = app.emit(EVENT, AssistantEvent::Error { message });
            let _ = app.emit(EVENT, AssistantEvent::Done);
        }
    });
    Ok(())
}

/// "Show me on my deck": the personal tour as a draft at once; its id (the window tries it).
#[tauri::command]
pub async fn assistant_tour(app: AppHandle, language: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || personal_tour(&app, language == "pl")).await.map_err(|e| e.to_string())?
}

// ---------- more power: Claude Code's own tools, each command and web fetch asked first ----------

/// What the session gets on top of HubDeck's tools when the user switched more power on.
const POWER_TOOLS: &str = "Read,Glob,Grep,Bash,WebFetch,WebSearch";

/// The rules added to the instructions while more power is on.
const POWER_RULES: &str = "\nMore power is on: you can also read files, search them, fetch web pages and run commands on this computer, so you can learn how the user's own apps can be driven (their local HTTP endpoints, an event stream, deep links, command-line switches, shortcuts, a hubdeck.json manifest) and check that a key will really work before you build it. Every command and web fetch shows in the user's window and waits for their yes: say in one line what you want to run and why before you run it, prefer read-only checks (GET requests, reading code and docs), never change, move or delete the user's files or settings outside HubDeck, and never send their data anywhere. Then build keys with core.http (an app's API), core.launch (deep links) or core.run (a command of their own tools).";

/// Questions Claude Code asks before it uses one of its own tools: id -> (tool, what, answer).
struct Asks {
    next: u64,
    open: std::collections::HashMap<u64, (String, Option<bool>)>,
    /// Tools the user allowed for the rest of this talk.
    allowed: std::collections::HashSet<String>,
}

static ASKS: Mutex<Option<Asks>> = Mutex::new(None);

fn asks<T>(f: impl FnOnce(&mut Asks) -> T) -> T {
    let mut g = ASKS.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(|| Asks { next: 1, open: Default::default(), allowed: Default::default() }))
}

/// What a tool use does, in one line for the window: the command, the address, the file, the search.
pub fn tool_detail(tool: &str, input: &Value) -> String {
    let field = |k: &str| input[k].as_str().unwrap_or_default().to_string();
    let detail = match tool {
        "Bash" => field("command"),
        "WebFetch" => field("url"),
        "WebSearch" => field("query"),
        "Read" => field("file_path"),
        "Glob" | "Grep" => format!("{} {}", field("pattern"), field("path")).trim().to_string(),
        _ => input.to_string(),
    };
    detail.chars().take(400).collect()
}

/// Claude Code asks (through `permission_prompt` of the assistant's MCP server): allowed already
/// for this talk, or a card in the window and an id to wait on.
pub fn api_permission(app: &AppHandle, body: &Value) -> Result<Value, String> {
    let tool = body["tool"].as_str().unwrap_or_default().to_string();
    if tool.is_empty() {
        return Err("tool is missing".into());
    }
    if asks(|a| a.allowed.contains(&tool)) {
        return Ok(json!({"allow": true}));
    }
    let detail = tool_detail(&tool, &body["input"]);
    let id = asks(|a| {
        let id = a.next;
        a.next += 1;
        a.open.insert(id, (tool.clone(), None));
        id
    });
    let _ = app.emit(EVENT, AssistantEvent::Permission { id, tool, detail });
    Ok(json!({"id": id}))
}

/// The user's answer to an ask, once there is one.
pub fn api_permission_answer(id: u64) -> Value {
    match asks(|a| a.open.get(&id).map(|(_, answer)| *answer)) {
        Some(Some(allow)) => json!({"answered": true, "allow": allow}),
        Some(None) => json!({"answered": false}),
        None => json!({"answered": true, "allow": false}),
    }
}

/// The window answered an ask: `once`, `talk` (this tool for the rest of the talk) or `deny`.
#[tauri::command]
pub fn assistant_permit(id: u64, choice: String) -> Result<(), String> {
    asks(|a| {
        let Some((tool, answer)) = a.open.get_mut(&id) else { return Err("this question is gone".to_string()) };
        *answer = Some(choice != "deny");
        if choice == "talk" {
            let tool = tool.clone();
            a.allowed.insert(tool);
        }
        Ok(())
    })
}

/// No answer will come (the talk stopped or started over): the open asks are refused.
fn close_asks(forget_allowed: bool) {
    asks(|a| {
        for (_, answer) in a.open.values_mut() {
            answer.get_or_insert(false);
        }
        if forget_allowed {
            a.allowed.clear();
        }
    });
}

/// Switch more power on or off; the next message starts Claude Code with it.
#[tauri::command]
pub fn assistant_power_set(engine: tauri::State<'_, Engine>, on: bool) -> Result<(), String> {
    {
        let mut s = engine.lock();
        s.store.settings.assistant_power = on;
        s.store.save_settings().map_err(|e| e.to_string())?;
    }
    super::guard::set_power(on);
    if !talk().busy {
        end_process();
    }
    Ok(())
}

/// What the assistant runs on, for the window to show before and during the talk: Claude Code,
/// its version, the account it is signed in with (`claude auth status`, read on this computer,
/// nothing sent), the model and session of the talk, and exactly how it is started.
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AssistantInfo {
    pub cli: Option<String>,
    pub version: Option<String>,
    pub logged_in: bool,
    /// `claude.ai` (a Claude plan) or an API key method.
    pub auth_method: Option<String>,
    /// `pro`, `max`... when signed in with a Claude account.
    pub plan: Option<String>,
    pub email: Option<String>,
    pub model: Option<String>,
    pub session: Option<String>,
    /// Tokens in the conversation's context now and the model's window (0 before the first answer).
    pub context_used: u64,
    pub context_window: u64,
    pub folder: String,
    pub command: String,
    pub rules: String,
    /// Picked in HubDeck (None = Claude Code's default).
    pub chosen_model: Option<String>,
    pub chosen_effort: Option<String>,
    /// Claude Code's own defaults from its user settings (`~/.claude/settings.json`).
    pub default_model: Option<String>,
    pub default_effort: Option<String>,
    /// More power is on (Claude Code's own tools, each command and web fetch asked first).
    pub power: bool,
}

/// Model aliases and effort levels HubDeck offers (Claude Code resolves the aliases).
pub const MODELS: [&str; 4] = ["fable", "opus", "sonnet", "haiku"];
pub const EFFORTS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];

/// Claude Code's own `model` and `effortLevel` from the user's settings file, if set.
fn claude_defaults() -> (Option<String>, Option<String>) {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"));
    let settings: Option<Value> = home.and_then(|h| std::fs::read(std::path::Path::new(&h).join(".claude").join("settings.json")).ok()).and_then(|b| serde_json::from_slice(&b).ok());
    let get = |k: &str| settings.as_ref().and_then(|s| s[k].as_str()).map(str::to_string);
    (get("model"), get("effortLevel"))
}

fn quiet(cli: &std::path::Path, args: &[&str]) -> Option<String> {
    let mut cmd = std::process::Command::new(cli);
    cmd.args(args).stdin(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd.output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

#[tauri::command]
pub async fn assistant_info(engine: tauri::State<'_, Engine>, language: String) -> Result<AssistantInfo, String> {
    let (chosen_model, chosen_effort, power) = {
        let s = engine.lock();
        (s.store.settings.assistant_model.clone(), s.store.settings.assistant_effort.clone(), s.store.settings.assistant_power)
    };
    tauri::async_runtime::spawn_blocking(move || {
        let (default_model, default_effort) = claude_defaults();
        let language = if language == "pl" { "Polish" } else { "English" };
        let folder = std::env::temp_dir().join("hubdeck-assistant").to_string_lossy().to_string();
        let (session, model, (context_used, context_window)) = {
            let t = talk();
            (t.session.clone(), t.model.clone(), t.context)
        };
        let mut info = AssistantInfo {
            folder,
            session,
            model,
            context_used,
            context_window,
            rules: rules(language),
            chosen_model,
            chosen_effort,
            default_model,
            default_effort,
            power,
            command: format!(
                "claude -p --input-format stream-json --output-format stream-json --verbose --include-partial-messages --tools {} --mcp-config mcp.json (hubdeck --mcp-assistant: drafts only) --strict-mcp-config --allowedTools mcp__{} --system-prompt <the instructions below> --setting-sources local --disable-slash-commands --settings {{\"autoMemoryEnabled\":false}} --model <yours or picked> --effort <yours or picked> [--resume <session>]{}",
                if power { POWER_TOOLS } else { "\"\"" },
                super::mcp::SERVER_NAME,
                if power { " --permission-prompt-tool mcp__hubdeck__permission_prompt (each command and web fetch asked in the window)" } else { "" }
            ),
            ..AssistantInfo::default()
        };
        // a test copy can pretend Claude Code is missing, to check what such a user sees
        let pretend_missing = crate::automation::get().is_some() && std::env::var_os("HUBDECK_TEST_NO_CLAUDE").is_some();
        let Some(cli) = super::mcp::find_claude_cli().filter(|_| !pretend_missing) else { return info };
        info.cli = Some(cli.to_string_lossy().to_string());
        info.version = quiet(&cli, &["--version"]);
        if let Some(status) = quiet(&cli, &["auth", "status"]).and_then(|s| serde_json::from_str::<Value>(&s).ok()) {
            info.logged_in = status["loggedIn"].as_bool().unwrap_or(false);
            info.auth_method = status["authMethod"].as_str().map(str::to_string);
            info.plan = status["subscriptionType"].as_str().map(str::to_string);
            info.email = status["email"].as_str().map(str::to_string);
        }
        info
    })
    .await
    .map_err(|e| e.to_string())
}

/// Pick the assistant's model alias and effort (None = Claude Code's default); from the next turn.
#[tauri::command]
pub fn assistant_set(engine: tauri::State<'_, Engine>, model: Option<String>, effort: Option<String>) -> Result<(), String> {
    let model = model.filter(|m| MODELS.contains(&m.as_str()));
    let effort = effort.filter(|e| EFFORTS.contains(&e.as_str()));
    let mut s = engine.lock();
    s.store.settings.assistant_model = model;
    s.store.settings.assistant_effort = effort;
    s.store.save_settings().map_err(|e| e.to_string())?;
    drop(s);
    // the running process keeps its model: end it between turns, the next message resumes
    if !talk().busy {
        end_process();
    }
    Ok(())
}

/// End the talk's Claude Code process (the session stays, to be resumed); true if a turn was running.
fn end_process() -> bool {
    let mut t = talk();
    t.run += 1;
    t.stdin = None;
    if let Some(mut child) = t.child.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
    std::mem::replace(&mut t.busy, false)
}

/// Stop the answer that is running (the next message resumes the talk).
#[tauri::command]
pub fn assistant_stop(app: AppHandle) {
    close_asks(false);
    if end_process() {
        let _ = app.emit(EVENT, AssistantEvent::Done);
    }
}

/// Start over: the next message begins a new conversation (with a new scan).
#[tauri::command]
pub fn assistant_reset(app: AppHandle) {
    assistant_stop(app);
    close_asks(true);
    let mut t = talk();
    t.session = None;
    t.context = (0, 0);
}

/// Show a draft on the deck for a try (None: back to what was showing).
#[tauri::command]
pub fn profile_try(engine: tauri::State<'_, Engine>, id: Option<String>) -> Result<(), String> {
    engine.try_profile(id.as_deref())
}

/// "Use this" (keep = true: on the deck as home, replacing the profile it is a new version of)
/// or throw a draft away. Kept: `{id, replaced}` (the name of the profile it went into).
#[tauri::command]
pub fn draft_settle(engine: tauri::State<'_, Engine>, id: String, keep: bool) -> Result<Option<Value>, String> {
    // which ideas the user keeps or throws away is part of their taste (`taste.json`)
    let made_by_assistant = engine.lock().store.profiles.get(&id).filter(|p| p.assistant).map(|p| (p.name.clone(), p.apps.first().cloned().unwrap_or_default()));
    let kept = if keep {
        let (kept, replaced) = engine.use_draft(&id)?;
        Some(json!({"id": kept, "replaced": replaced}))
    } else {
        engine.settle_draft(&id, false)?;
        None
    };
    if let Some((name, app)) = made_by_assistant {
        super::taste::outcome(&name, &app, keep);
    }
    Ok(kept)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_are_split_off_the_message() {
        let (text, choices) = split_choices("What do you do most?\n<choices>Streaming|Coding| Gaming </choices>");
        assert_eq!(text, "What do you do most?");
        assert_eq!(choices, ["Streaming", "Coding", "Gaming"]);
        assert_eq!(split_choices("No buttons here."), ("No buttons here.".to_string(), vec![]));
        assert_eq!(split_choices("Broken <choices>a|b").1, Vec::<String>::new());
    }

    #[test]
    fn a_sketch_becomes_a_draft_and_bad_controls_are_left_out() {
        let idea = Idea {
            title: "Spotify mixer".into(),
            app: "Spotify.exe".into(),
            deck: Some(json!({
                "keys": [
                    {"action": "core.media", "params": {"command": "playPause"}, "label": "Play", "icon": "play"},
                    {"action": "no.such", "label": "Bad"},
                    {"action": "", "label": "Empty"}
                ],
                "dial": {"action": "core.nowPlaying", "label": "Now playing"},
                "knobs": [{"action": "core.volume", "params": {"target": "app", "app": "Spotify.exe"}, "label": "Spotify"}],
                "edge": "#79ecc6"
            })),
            ..Idea::default()
        };
        let (p, dropped) = sketch_profile(&idea);
        let p = p.expect("a draft");
        assert_eq!(dropped.len(), 1, "the unknown action is reported: {dropped:?}");
        assert_eq!(p.name, "Spotify mixer · sketch");
        assert_eq!(p.apps, ["Spotify.exe"]);
        assert!(p.controls.contains_key("keys.0") && p.controls.contains_key("dial") && p.controls.contains_key("knobs.0"));
        assert!(!p.controls.contains_key("keys.1") && !p.controls.contains_key("keys.2"), "unknown and empty actions are left out");
        assert_eq!(p.controls["keys.0"].look.bg.as_deref(), Some(super::super::house::BG), "in the house look");
        assert_eq!(p.edge.as_deref(), Some("#79ecc6"));
        assert!(sketch_profile(&Idea { deck: Some(json!({"keys": [{"action": "no.such"}]})), ..Idea::default() }).0.is_none());
        let (p, dropped) = sketch_profile(&Idea {
            deck: Some(json!({"keys": [{"action": "core.hotkey", "params": {"keys": "Alt+F4"}, "label": "Quit"}, {"action": "core.media", "params": {"command": "playPause"}}]})),
            ..Idea::default()
        });
        assert!(p.is_some_and(|p| p.assistant && !p.controls.contains_key("keys.0")), "a closing shortcut on a press is left out");
        assert!(dropped[0].contains("hold"), "{dropped:?}");
    }

    #[test]
    fn ideas_are_split_off_the_message() {
        let (text, ideas) = split_ideas(
            "Three ideas:\n<ideas>[{\"title\":\"Spotify mixer\",\"app\":\"Spotify.exe\",\"pitch\":\"Play and volume\",\"extras\":[\"Cover on the dial\"]},{\"title\":\"\"}]</ideas>\n<choices>a|b</choices>",
        );
        assert_eq!(ideas.len(), 1, "an idea without a title is dropped");
        assert_eq!(ideas[0].extras, ["Cover on the dial"]);
        assert_eq!(split_choices(&text), ("Three ideas:".to_string(), vec!["a".to_string(), "b".to_string()]));
        assert!(split_ideas("<ideas>not json</ideas>").1.is_empty());
        let (_, odd) = split_ideas(r#"<ideas>[{"title":"A","extras":"one thing"},{"title":"B","extras":["x"],"deck":{"keys":[]}}]</ideas>"#);
        assert_eq!(odd.len(), 2, "an odd field does not lose the other ideas");
        assert_eq!(odd[0].extras, ["one thing"]);
        assert!(odd[1].deck.is_some());
        let (_, with_keys) = split_ideas(r#"<ideas>[{"title":"T","keys":[{"label":"Play","icon":"play"}]}]</ideas>"#);
        assert_eq!(with_keys[0].keys[0].icon, "play");
        assert!(rules("Polish").contains("in Polish, the language of their window") && !rules("Polish").contains("{HOUSE}"));
    }

    #[test]
    fn the_context_fill_comes_from_the_last_call_and_the_window() {
        let e = read_line(r#"{"type":"assistant","message":{"usage":{"input_tokens":2,"cache_creation_input_tokens":600,"cache_read_input_tokens":12000,"output_tokens":40},"content":[]}}"#).events;
        assert!(matches!(e[0], AssistantEvent::Context { used: 12642, .. }), "{e:?}");
        let e = read_line(r#"{"type":"result","is_error":false,"modelUsage":{"claude-opus-5-5":{"contextWindow":1000000}}}"#).events;
        assert_eq!(e, vec![AssistantEvent::Context { used: 12642, window: 1_000_000 }]);
    }

    #[test]
    fn stream_lines_become_window_events() {
        let r = read_line(r#"{"type":"system","subtype":"init","session_id":"abc"}"#);
        let (e, s) = (r.events, r.session);
        assert!(e.is_empty());
        assert_eq!(s.as_deref(), Some("abc"));
        let e = read_line(r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Hi"},{"type":"tool_use","name":"mcp__hubdeck__create_profile"}]}}"#).events;
        assert_eq!(e, vec![AssistantEvent::Text { text: "Hi".into(), choices: vec![], ideas: vec![] }, AssistantEvent::Tool { name: "create_profile".into() }]);
        let e = read_line(r#"{"type":"result","is_error":true,"result":"limit"}"#).events;
        assert_eq!(e, vec![AssistantEvent::Error { message: "limit".into() }]);
        assert!(read_line("not json").events.is_empty());
        let r = read_line(r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"Hel"}}}"#);
        assert_eq!(r.events, vec![AssistantEvent::Delta { text: "Hel".into() }]);
        assert!(!r.end);
        assert!(read_line(r#"{"type":"result","subtype":"success","is_error":false,"result":"ok"}"#).end, "a result ends the turn");
        let line: Value = serde_json::from_str(&input_line("hi \"you\"\nok")).unwrap();
        assert_eq!((line["type"].as_str(), line["message"]["content"].as_str()), (Some("user"), Some("hi \"you\"\nok")));
        assert!(catalog_text().contains("core.hotkey") && catalog_text().contains("Spotify"));
    }

    #[test]
    fn the_scan_text_says_what_was_found() {
        let used = [("Visual Studio Code".to_string(), 12.5, 4)];
        let t = scan_text(Some((&used, "2026-10-12 16:30")), &["keys".into(), "dial".into()], &["faders".into()], &["Everyday desk".into()]);
        assert!(t.contains("Owned but not attached now: faders"));
        assert!(t.contains("Visual Studio Code (12.5 h on 4 days)"));
        assert!(t.contains("keys, dial") && t.contains("Everyday desk"));
        let without = scan_text(None, &["keys".into()], &[], &[]);
        assert!(!without.contains("Visual Studio Code") && without.contains("chose not to send them"), "without the app list: no app names");
    }
}
