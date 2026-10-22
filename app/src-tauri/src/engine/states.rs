//! States from outside (roadmap phase 12 B, `docs/APP-CONTRACT.md`): the user's own apps publish
//! their state (`GET /hubdeck/status`, `GET /hubdeck/events` as Server-Sent Events, a manifest
//! with actions) and HubDeck listens; scripts without a server push to `/api/state`. Each item
//! goes to the engine as `IntegrationValue { key: "state:<app>.<id>", value: <item JSON> }`; keys
//! (`core.state`) and the dial show an item or count them, and an item waiting for the user
//! pulses the edge like a Claude Code session does.
//!
//! One thread per connected app: it reads the status, then follows the event stream (reconnecting
//! with a growing pause) or, without one, reads the status every few seconds. Nothing runs while
//! no app is connected.

use std::collections::HashMap;
use std::io::BufRead;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crossbeam_channel::Sender;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::Msg;
use super::actions::param_str;
use super::profile::Look;

/// The action id of a key or the dial showing an app's state.
pub const ACTION: &str = "core.state";
/// How often an app without an event stream is read.
const POLL: Duration = Duration::from_secs(5);
/// Prefix of the engine's values.
pub const PREFIX: &str = "state:";

/// One thing an app reports (a session, a job, a build).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Item {
    pub id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub state: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
}

/// An action an app offers for keys (its manifest).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppAction {
    pub id: String,
    #[serde(default)]
    pub label: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub icon: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub method: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub url: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub body: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub link: String,
}

/// An app the user connected (Settings > Connections > Your apps).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppSource {
    /// Short name used in state names (`launcher` in `launcher.1`).
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub exe: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub events: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<AppAction>,
}

/// The shared states (the contract's scale): color, icon and the word on a key.
pub fn look_of(state: &str) -> (&'static str, &'static str) {
    match state {
        "waiting" => ("#ff8a00", "hand"),
        "working" => ("#3b82f6", "refresh-cw"),
        "done" => ("#22c55e", "check"),
        "error" => ("#ef4444", "x"),
        "offline" => ("#55555f", "minus"),
        _ => ("#8a8a96", "circle-dot"),
    }
}

/// Items from a status answer or an event: `{"items": [...]}`, a list, or one item.
pub fn items_of(v: &Value) -> Vec<Item> {
    let list = match v {
        Value::Array(a) => a.clone(),
        Value::Object(o) if o.contains_key("items") => o["items"].as_array().cloned().unwrap_or_default(),
        Value::Object(_) => vec![v.clone()],
        _ => Vec::new(),
    };
    list.into_iter()
        .filter_map(|i| {
            // ids may be numbers
            let mut i = i;
            if let Some(n) = i.get("id").filter(|x| x.is_number()).cloned() {
                i["id"] = json!(n.to_string());
            }
            serde_json::from_value::<Item>(i).ok()
        })
        .filter(|i| !i.id.trim().is_empty())
        .collect()
}

// ---------- the listeners ----------

static ENGINE: OnceLock<Sender<Msg>> = OnceLock::new();
/// Bumped on every reconnect of all apps: an older listener sees it and ends.
static GENERATION: AtomicU64 = AtomicU64::new(0);

pub fn init(engine: Sender<Msg>) {
    let _ = ENGINE.set(engine);
}

/// The latest item of every name, for the API and the assistant (the engine has its own copy).
static LAST: std::sync::Mutex<std::collections::BTreeMap<String, Item>> = std::sync::Mutex::new(std::collections::BTreeMap::new());

/// Every item now, by name (`launcher.1`).
pub fn snapshot() -> std::collections::BTreeMap<String, Item> {
    LAST.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

fn send(app: &str, item: &Item) {
    LAST.lock().unwrap_or_else(|e| e.into_inner()).insert(format!("{app}.{}", item.id), item.clone());
    if let Some(tx) = ENGINE.get() {
        let _ = tx.send(Msg::IntegrationValue { key: format!("{PREFIX}{app}.{}", item.id), value: json!(item).to_string() });
    }
}

/// An app's items as `offline` (it stopped answering), so keys do not show a stale state.
fn offline(app: &str, known: &HashMap<String, Item>) {
    for item in known.values() {
        send(app, &Item { state: "offline".into(), text: String::new(), ..item.clone() });
    }
}

/// Listen to these apps (from the settings); the ones before stop.
pub fn connect_all(apps: Vec<AppSource>) {
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    for app in apps {
        let _ = std::thread::Builder::new().name(format!("app-state-{}", app.id)).spawn(move || listen(app, generation));
    }
}

fn current(generation: u64) -> bool {
    GENERATION.load(Ordering::SeqCst) == generation
}

fn listen(app: AppSource, generation: u64) {
    let quick: ureq::Agent = crate::engine::calls::builder().timeout_global(Some(Duration::from_secs(5))).http_status_as_error(false).build().into();
    // a stream stays open: no overall timeout, only for connecting
    let stream: ureq::Agent = crate::engine::calls::builder().timeout_connect(Some(Duration::from_secs(5))).http_status_as_error(false).build().into();
    let mut known: HashMap<String, Item> = HashMap::new();
    let mut pause = Duration::from_secs(2);
    while current(generation) {
        let read = quick.get(&app.status).call().map_err(|e| e.to_string()).and_then(|mut r| r.body_mut().read_json::<Value>().map_err(|e| e.to_string()));
        match read {
            Ok(v) => {
                for item in items_of(&v) {
                    send(&app.id, &item);
                    known.insert(item.id.clone(), item);
                }
                pause = Duration::from_secs(2);
            }
            Err(e) => {
                log::debug!("{}: {e}", app.name);
                offline(&app.id, &known);
            }
        }
        if !app.events.is_empty() {
            // follow the stream until it drops
            if let Ok(mut answer) = stream.get(&app.events).header("Accept", "text/event-stream").call()
                && answer.status().as_u16() < 400
            {
                let reader = std::io::BufReader::new(answer.body_mut().as_reader());
                for line in reader.lines() {
                    let Ok(line) = line else { break };
                    if !current(generation) {
                        return;
                    }
                    if let Some(data) = line.strip_prefix("data:")
                        && let Ok(v) = serde_json::from_str::<Value>(data.trim())
                    {
                        for item in items_of(&v) {
                            let merged = merge(known.get(&item.id), item);
                            send(&app.id, &merged);
                            known.insert(merged.id.clone(), merged);
                        }
                    }
                }
            }
            std::thread::sleep(pause);
            pause = (pause * 2).min(Duration::from_secs(30));
        } else {
            std::thread::sleep(POLL);
        }
    }
}

/// An event's item over the one known: fields it leaves out stay.
fn merge(old: Option<&Item>, new: Item) -> Item {
    let Some(old) = old else { return new };
    Item {
        label: if new.label.is_empty() { old.label.clone() } else { new.label },
        state: if new.state.is_empty() { old.state.clone() } else { new.state },
        text: new.text,
        value: new.value.or(old.value),
        id: new.id,
    }
}

/// A push from a script: `{"source": "backup", "items": [...]}`.
pub fn push(body: &Value) -> Result<usize, String> {
    let source = body["source"]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        .ok_or("source: a short name (letters, digits, - _)")?;
    let items = items_of(body);
    if items.is_empty() {
        return Err("no items (each with an id)".into());
    }
    for item in &items {
        send(source, item);
    }
    Ok(items.len())
}

// ---------- connecting an app ----------

/// A short name for state names from an app's name (`Agent Desk` -> `agent-desk`).
fn slug(name: &str) -> String {
    let s: String = name.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    let s = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    if s.is_empty() { "app".into() } else { s }
}

/// What an address answers: a manifest (status, events, actions) or a status itself.
pub fn source_from(url: &str, answer: &Value, taken: &[String]) -> Result<AppSource, String> {
    let base = url.trim().trim_end_matches('/');
    let mut app = if answer.get("status").and_then(Value::as_str).is_some() || answer.get("actions").is_some() {
        AppSource {
            name: answer["app"].as_str().unwrap_or("App").to_string(),
            exe: answer["exe"].as_str().unwrap_or_default().to_string(),
            status: answer["status"].as_str().unwrap_or_default().to_string(),
            events: answer["events"].as_str().unwrap_or_default().to_string(),
            actions: answer["actions"].as_array().into_iter().flatten().filter_map(|a| serde_json::from_value(a.clone()).ok()).collect(),
            ..AppSource::default()
        }
    } else if answer.get("items").is_some() || answer.is_array() {
        // a status address; its events next to it by the contract's names
        let events = base.strip_suffix("/status").map(|b| format!("{b}/events")).unwrap_or_default();
        AppSource { name: answer["app"].as_str().unwrap_or("App").to_string(), status: base.to_string(), events, ..AppSource::default() }
    } else {
        return Err("this address answers neither a HubDeck manifest nor a status (docs/APP-CONTRACT.md)".into());
    };
    if app.status.is_empty() {
        return Err("the manifest names no status address".into());
    }
    let mut id = slug(&app.name);
    let stem = id.clone();
    let mut n = 2;
    while taken.contains(&id) {
        id = format!("{stem}-{n}");
        n += 1;
    }
    app.id = id;
    Ok(app)
}

/// Read an address and make an app source of it (one GET, five seconds at most).
pub fn discover(url: &str, taken: &[String]) -> Result<AppSource, String> {
    let (_, text) = super::exec::http_request("GET", url, "", "")?;
    let answer: Value = serde_json::from_str(&text).map_err(|_| "the answer is not JSON".to_string())?;
    source_from(url, &answer, taken)
}

// ---------- the window (Settings > Connections > Your apps) ----------

#[tauri::command]
pub fn apps_state(engine: tauri::State<'_, super::Engine>) -> Value {
    engine.app_states()
}

/// Connect an app by its manifest or status address (one GET, off the window's thread).
#[tauri::command]
pub async fn app_connect(app: tauri::AppHandle, url: String) -> Result<AppSource, String> {
    use tauri::Manager;
    tauri::async_runtime::spawn_blocking(move || app.state::<super::Engine>().connect_app(&url)).await.map_err(|e| e.to_string())?
}

/// The user looked at the apps that connected themselves.
#[tauri::command]
pub fn apps_seen(engine: tauri::State<'_, super::Engine>) -> Result<(), String> {
    {
        let mut s = engine.lock();
        s.store.settings.apps_joined.clear();
        s.store.save_settings().map_err(|e| e.to_string())?;
    }
    engine.changed();
    Ok(())
}

#[tauri::command]
pub fn app_remove(engine: tauri::State<'_, super::Engine>, id: String) -> Result<(), String> {
    engine.remove_app(&id)
}

/// The prompt that adds the contract to an app (docs/APP-CONTRACT.md, section 6).
pub const CLAUDE_PROMPT: &str = "Add a HubDeck integration to this app (HubDeck's contract, docs/APP-CONTRACT.md in the HubDeck repository): serve on the app's local port (127.0.0.1 only) GET /hubdeck/status answering {\"app\": <name>, \"items\": [{\"id\", \"label\", \"state\", \"text\", \"value\"}]} with one item per session, job or thing worth seeing, state one of waiting (it needs the user), working, done, error, idle, offline; GET /hubdeck/events as Server-Sent Events, each event `data: {\"items\": [<the items that changed>]}`; and GET /hubdeck/manifest {\"app\", \"exe\", \"status\": <url>, \"events\": <url>, \"actions\": [{\"id\", \"label\", \"icon\", \"method\", \"url\" (may hold {id}), \"body\"} or {\"id\", \"label\", \"link\"}]} listing the app's useful actions. On start, announce the manifest to HubDeck: read its local API port from api-http-port and its token from api-token in HubDeck's data folder (%APPDATA%\\dev.hubdeck.app on Windows, ~/Library/Application Support/dev.hubdeck.app on macOS) and POST http://127.0.0.1:<port>/api/apps/announce with Authorization: Bearer <token> and {\"url\": <manifest url>}; stay quiet if the files are missing or the call fails (HubDeck not running). Read-only except the actions; no auth beyond 127.0.0.1. Add a test that the three endpoints answer in these shapes, and tell me the manifest address.";

#[tauri::command]
pub fn app_contract_prompt() -> &'static str {
    CLAUDE_PROMPT
}

// ---------- what keys and the dial show ----------

/// The items of `name` from the engine's values: one (`launcher.1`) or all of an app (`launcher.*`).
pub fn items(values: &HashMap<String, String>, name: &str) -> Vec<Item> {
    let name = name.trim();
    if let Some(app) = name.strip_suffix(".*") {
        let prefix = format!("{PREFIX}{app}.");
        let mut all: Vec<Item> = values.iter().filter(|(k, _)| k.starts_with(&prefix)).filter_map(|(_, v)| serde_json::from_str(v).ok()).collect();
        all.sort_by(|a, b| a.id.cmp(&b.id));
        return all;
    }
    values.get(&format!("{PREFIX}{name}")).and_then(|v| serde_json::from_str(v).ok()).into_iter().collect()
}

/// Does anything an app reports wait for the user (the edge pulses then)?
pub fn any_waiting(values: &HashMap<String, String>) -> bool {
    values.iter().any(|(k, v)| k.starts_with(PREFIX) && serde_json::from_str::<Item>(v).is_ok_and(|i| i.state == "waiting"))
}

/// What a state control shows: the main line, the second, its color and icon.
pub fn face(params: &Value, look: &Look, values: &HashMap<String, String>) -> (String, Option<String>, &'static str, &'static str) {
    let name = param_str(params, "name");
    let caption = look.label.clone().filter(|l| !l.trim().is_empty());
    if name.trim().is_empty() {
        return ("Set up".into(), caption.or(Some("App state".into())), "#8a8a96", "circle-dot");
    }
    let list = items(values, name);
    if name.trim().ends_with(".*") {
        // the dial or a key counting an app's items: how many wait for the user, else how many work
        let waiting = list.iter().filter(|i| i.state == "waiting").count();
        let working = list.iter().filter(|i| i.state == "working").count();
        let (n, state, word) = if waiting > 0 { (waiting, "waiting", "waiting") } else { (working, "working", "working") };
        let (color, icon) = if list.is_empty() { look_of("offline") } else { look_of(state) };
        return (if list.is_empty() { "–".into() } else { n.to_string() }, caption.or(Some(word.into())), color, icon);
    }
    let Some(item) = list.into_iter().next() else {
        let (color, icon) = look_of("offline");
        return ("…".into(), caption, color, icon);
    };
    let (color, icon) = look_of(&item.state);
    let main = caption.unwrap_or_else(|| if item.label.is_empty() { item.id.clone() } else { item.label.clone() });
    let second = Some(if item.text.is_empty() { item.state.clone() } else { item.text.clone() }).filter(|s| !s.is_empty());
    (main, second, color, icon)
}

/// A key's look for an app's state: the state's icon in its color, the item's name and its line.
pub fn key_look(params: &Value, look: Look, values: &HashMap<String, String>, blink: bool) -> Look {
    let (main, second, color, icon) = face(params, &look, values);
    // an item waiting for the user pulses (motion with a meaning: calm keys stand still)
    let pulse = blink && color == look_of("waiting").0;
    let label = match second {
        Some(s) => format!("{main}\n{s}"),
        None => main,
    };
    let has_picture = look.image.as_deref().is_some_and(|i| !i.is_empty());
    // a key's own icon stays and the state shows in its color (2026-10-19: five tag keys of the
    // owner's music app all showed the waiting hand)
    let own_icon = look.icon.as_deref().is_some_and(|i| !i.is_empty());
    Look {
        label: Some(label),
        icon: if has_picture || own_icon { look.icon.clone() } else { Some(icon.to_string()) },
        icon_color: Some(color.to_string()),
        bg: if pulse { Some(PULSE_BG.to_string()) } else { look.bg.clone() },
        ..look
    }
}

/// The background of a waiting item's key on the pulse's other beat.
const PULSE_BG: &str = "#4a2a08";

/// A press: the address it calls (POST by default, `{id}` = the item's id) or the link it opens.
pub fn press(params: &Value) -> Result<Option<String>, String> {
    let id = param_str(params, "name").rsplit('.').next().unwrap_or_default().to_string();
    let url = param_str(params, "url").trim().replace("{id}", &id);
    if !url.is_empty() {
        let method = Some(param_str(params, "method")).filter(|m| !m.is_empty()).unwrap_or("POST");
        let (code, _) = super::exec::http_request(method, &url, param_str(params, "body"), "")?;
        return Ok(Some(format!("{code}")));
    }
    let link = param_str(params, "link").trim().replace("{id}", &id);
    if !link.is_empty() {
        crate::system::shell_open(&link)?;
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn items_come_in_the_contract_shapes() {
        let v = json!({"app": "L", "items": [{"id": 1, "label": "Agent 1", "state": "waiting"}, {"label": "no id"}]});
        let items = items_of(&v);
        assert_eq!(items.len(), 1, "an item needs an id");
        assert_eq!((items[0].id.as_str(), items[0].state.as_str()), ("1", "waiting"));
        assert_eq!(items_of(&json!({"id": "x", "state": "done"}))[0].state, "done", "one item alone");
        let merged = merge(Some(&items[0]), Item { id: "1".into(), state: "working".into(), ..Item::default() });
        assert_eq!((merged.label.as_str(), merged.state.as_str()), ("Agent 1", "working"), "an event keeps what it leaves out");
    }

    #[test]
    fn keys_and_the_dial_show_states() {
        let mut values = HashMap::new();
        values.insert("state:launcher.1".into(), json!({"id": "1", "label": "Agent 1", "state": "waiting", "text": "Needs your OK"}).to_string());
        values.insert("state:launcher.2".into(), json!({"id": "2", "state": "working"}).to_string());
        let look = key_look(&json!({"name": "launcher.1"}), Look::default(), &values, false);
        assert_eq!(key_look(&json!({"name": "launcher.1"}), Look::default(), &values, true).bg.as_deref(), Some(PULSE_BG), "waiting pulses");
        assert_eq!(key_look(&json!({"name": "launcher.2"}), Look::default(), &values, true).bg, None, "working stands still");
        assert_eq!(look.label.as_deref(), Some("Agent 1\nNeeds your OK"));
        assert_eq!((look.icon.as_deref(), look.icon_color.as_deref()), (Some("hand"), Some("#ff8a00")));
        // a key with its own icon keeps it; the state shows in its color
        let own = key_look(&json!({"name": "launcher.1"}), Look { icon: Some("bookmark".into()), ..Look::default() }, &values, false);
        assert_eq!((own.icon.as_deref(), own.icon_color.as_deref()), (Some("bookmark"), Some("#ff8a00")));
        let (main, second, ..) = face(&json!({"name": "launcher.*"}), &Look::default(), &values);
        assert_eq!((main.as_str(), second.as_deref()), ("1", Some("waiting")), "the dial counts who waits");
        assert!(any_waiting(&values));
        let (main, ..) = face(&json!({"name": "other.1"}), &Look::default(), &values);
        assert_eq!(main, "…");
    }

    #[test]
    fn an_address_becomes_an_app() {
        let manifest = json!({"app": "Agent Desk", "status": "http://127.0.0.1:4317/hubdeck/status", "events": "http://127.0.0.1:4317/hubdeck/events", "actions": [{"id": "new", "label": "New agent", "method": "POST", "url": "http://127.0.0.1:4317/agents"}]});
        let app = source_from("http://127.0.0.1:4317/hubdeck/manifest", &manifest, &[]).unwrap();
        assert_eq!((app.id.as_str(), app.actions.len()), ("agent-desk", 1));
        let status = source_from("http://127.0.0.1:9/hubdeck/status", &json!({"app": "Agent Desk", "items": []}), &["agent-desk".into()]).unwrap();
        assert_eq!(status.id, "agent-desk-2", "names stay unique");
        assert_eq!(status.events, "http://127.0.0.1:9/hubdeck/events");
        assert!(source_from("http://x", &json!({"hello": 1}), &[]).is_err());
        assert!(push(&json!({"source": "bad name", "items": [{"id": "1"}]})).is_err());
    }
}
