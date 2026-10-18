//! Stream Deck plugins (Elgato's SDK, also OpenAction's): the plugins people already use for
//! Discord, Spotify, OBS, lights and hundreds more run in HubDeck (owner, 2026-10-13; research and
//! plan in docs/STREAMDECK-PLUGINS.md). Written from Elgato's public SDK documentation.
//!
//! A plugin is a folder `<uuid>.sdPlugin` with a `manifest.json` (`UUID`, `CodePath`, `Actions`...)
//! in `<app data>/plugins/`. Its program is Node (`Nodejs` in the manifest), a native program, or an
//! HTML page HubDeck opens in a hidden window. It connects to the plugin server's WebSocket and
//! registers with `{"event": "registerPlugin", "uuid": <the -pluginUUID it was given>}`; its
//! settings pages (property inspectors) do the same with `registerPropertyInspector`.
//!
//! This module translates between that protocol and HubDeck's own plugin messages
//! (`plugins.rs`): the 15 keys are a 5x3 keypad, the dial and the knobs are encoders.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, RwLock};

use base64::Engine as _;
use crossbeam_channel::Sender;
use serde_json::{Value, json};

use super::actions::InputKind;
use super::plugins::{Manifest, ManifestAction};

/// The Stream Deck version HubDeck says it is (manifests ask for 6.4 .. 7.6).
const SD_VERSION: &str = "6.9.0";
/// The one device HubDeck shows plugins: a keypad of 5x3 with encoders (typed as a Stream Deck +,
/// type 7, so encoder actions are offered).
pub const DEVICE: &str = "hubdeck";

/// What HubDeck keeps of a Stream Deck manifest.
#[derive(Debug, Clone, Default)]
pub struct SdManifest {
    pub uuid: String,
    pub name: String,
    pub version: String,
    #[allow(dead_code)]
    pub author: String,
    /// The entry point for this system (`CodePathMac` / `CodePathWin` before `CodePath`).
    pub code_path: String,
    /// Run by Node (the manifest's `Nodejs`, or a `.js` entry).
    pub node: bool,
    /// The systems the plugin is made for (`mac`, `windows`).
    pub platforms: Vec<String>,
    pub actions: Vec<SdAction>,
    /// The plugin-wide settings page, used by actions without their own.
    pub inspector: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct SdAction {
    pub uuid: String,
    pub name: String,
    pub tooltip: String,
    /// The action's icon in the library, as a data URI.
    pub icon: Option<String>,
    /// Per state: its picture (data URI) and title.
    pub states: Vec<(Option<String>, Option<String>)>,
    pub keypad: bool,
    pub encoder: bool,
    pub inspector: Option<String>,
}

/// An HTML plugin runs in a hidden window; the app sets how (a Tauri window per plugin).
type HtmlLauncher = Box<dyn Fn(&str, &str) + Send + Sync>;
static HTML: OnceLock<HtmlLauncher> = OnceLock::new();

pub fn set_html_launcher(f: HtmlLauncher) {
    let _ = HTML.set(f);
}

/// Action uuid -> what drawing and input need (states, controllers).
static ACTIONS: RwLock<Vec<(String, SdAction)>> = RwLock::new(Vec::new());
/// Context (control id) -> the state a plugin set or a key-up toggled.
static STATES: Mutex<Option<HashMap<String, u8>>> = Mutex::new(None);
/// Context -> its settings (the binding's params), kept so getSettings answers at once.
static SETTINGS: Mutex<Option<HashMap<String, Value>>> = Mutex::new(None);
/// (plugin, action, context).
type Owner = (String, String, String);
/// Property inspector registration uuid -> who it belongs to.
static INSPECTORS: Mutex<Option<HashMap<String, Owner>>> = Mutex::new(None);
/// Context -> the open property inspector's connection.
static INSPECTOR_OUT: Mutex<Option<HashMap<String, Sender<String>>>> = Mutex::new(None);
/// The local server for plugin files (settings pages, HTML plugins): its port.
static FILES_PORT: OnceLock<u16> = OnceLock::new();
/// `<app data>`: global settings live in `<data>/plugin-settings/<uuid>.json`.
static DATA: OnceLock<PathBuf> = OnceLock::new();

fn with<T: Default, R>(m: &Mutex<Option<T>>, f: impl FnOnce(&mut T) -> R) -> R {
    f(m.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(T::default))
}

fn platform() -> &'static str {
    if cfg!(windows) { "windows" } else { "mac" }
}

/// This machine as OpenAction's `CodePaths` names it.
fn target_triple() -> &'static str {
    match (cfg!(windows), cfg!(target_arch = "aarch64")) {
        (true, true) => "aarch64-pc-windows-msvc",
        (true, false) => "x86_64-pc-windows-msvc",
        (false, true) => "aarch64-apple-darwin",
        (false, false) => "x86_64-apple-darwin",
    }
}

/// Is this folder's manifest a Stream Deck one (`Actions` and a program: `UUID`, `CodePath`, the
/// per-system `CodePathWin` / `CodePathMac`, or OpenAction's `CodePaths` by target; the catalog's
/// OpenAction plugins give only the last ones, plugin check 2026-10-16)?
pub fn is_sd_manifest(v: &Value) -> bool {
    v.get("Actions").is_some() && ["UUID", "CodePath", "CodePathWin", "CodePathMac", "CodePaths"].iter().any(|k| v.get(*k).is_some())
}

/// A file inside the plugin folder; never outside it: only plain names (no `..`, no drive or root,
/// no `:`, so `C:/Users/...` cannot replace the folder on Windows; release audit, 2026-10-13), and
/// a file that exists must still be inside the folder once links are followed.
pub fn inside(dir: &Path, rel: &str) -> Option<PathBuf> {
    let rel = rel.trim_start_matches(['/', '\\']);
    if rel.split(['/', '\\']).any(|p| p == ".." || p.contains(':')) {
        return None;
    }
    let rel = Path::new(rel);
    if !rel.components().all(|c| matches!(c, std::path::Component::Normal(_) | std::path::Component::CurDir)) {
        return None;
    }
    let path = dir.join(rel);
    if let (Ok(real), Ok(base)) = (path.canonicalize(), dir.canonicalize())
        && !real.starts_with(&base)
    {
        return None;
    }
    Some(path)
}

/// A plugin id as it may appear in a URL: letters, digits, `.`, `-`, `_`, not `.` or `..`.
fn plain_id(id: &str) -> bool {
    !id.is_empty() && id != "." && id != ".." && id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

/// A manifest picture: the path has no extension; Stream Deck picks `@2x.png`, `.png`, `.svg`...
pub fn image_uri(dir: &Path, rel: &str) -> Option<String> {
    if rel.is_empty() {
        return None;
    }
    let base = inside(dir, rel)?;
    let candidates = [format!("{rel}@2x.png"), format!("{rel}.png"), format!("{rel}.svg"), format!("{rel}.jpg"), format!("{rel}.gif"), rel.to_string()];
    for c in candidates {
        let p = inside(dir, &c)?;
        if p.is_file() {
            return file_uri(&p);
        }
    }
    let _ = base;
    None
}

fn file_uri(p: &Path) -> Option<String> {
    let bytes = std::fs::read(p).ok()?;
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let mime = match ext.as_str() {
        "png" => "png",
        "jpg" | "jpeg" => "jpeg",
        "gif" => "gif",
        "svg" => "svg+xml",
        "webp" => "webp",
        _ => return None,
    };
    Some(format!("data:image/{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

/// A picture a plugin sends with `setImage`: a base64 data URL, an SVG data URL in plain text, or a
/// file in its folder. HubDeck keeps only base64 data URIs.
pub fn normalize_image(dir: &Path, image: &str) -> Option<String> {
    let image = image.trim();
    if image.is_empty() {
        return None;
    }
    if let Some(rest) = image.strip_prefix("data:") {
        let (head, data) = rest.split_once(',')?;
        if head.ends_with(";base64") {
            return Some(image.to_string());
        }
        // `data:image/svg+xml;charset=utf8,<svg ...>` (often percent-encoded)
        let mime = head.split(';').next().unwrap_or("image/svg+xml");
        let text = percent_decode(data);
        return Some(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(text.as_bytes())));
    }
    image_uri(dir, image).or_else(|| inside(dir, image).and_then(|p| file_uri(&p)))
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Ok(v) = u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("zz"), 16)
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Read a Stream Deck manifest; pictures are resolved against `dir`.
pub fn read(v: &Value, dir: &Path) -> Result<SdManifest, String> {
    let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let folder_uuid = dir.file_name().and_then(|n| n.to_str()).and_then(|n| n.strip_suffix(".sdPlugin")).unwrap_or("").to_string();
    let uuid = if s("UUID").is_empty() { folder_uuid } else { s("UUID") };
    if uuid.is_empty() {
        return Err("no UUID".into());
    }
    // OpenAction's `CodePaths` by target first (the native build for this machine), then
    // Stream Deck's per-system and plain entries
    let by_target = v.get("CodePaths").and_then(|c| c.get(target_triple())).and_then(Value::as_str).unwrap_or("").to_string();
    let code_path = match platform() {
        "mac" => [by_target, s("CodePathMac"), s("CodePath")],
        _ => [by_target, s("CodePathWin"), s("CodePath")],
    }
    .into_iter()
    .find(|c| !c.is_empty())
    .ok_or("no CodePath")?;
    let platforms = v.get("OS").and_then(Value::as_array).map(|a| a.iter().filter_map(|o| o["Platform"].as_str().map(str::to_string)).collect()).unwrap_or_default();
    let node = v.get("Nodejs").is_some_and(Value::is_object) || [".js", ".mjs", ".cjs"].iter().any(|e| code_path.ends_with(e));
    let inspector = Some(s("PropertyInspectorPath")).filter(|p| !p.is_empty());
    let actions = v
        .get("Actions")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|a| {
                    let uuid = a["UUID"].as_str()?.to_string();
                    let controllers: Vec<&str> = a["Controllers"].as_array().map(|c| c.iter().filter_map(Value::as_str).collect()).unwrap_or_else(|| vec!["Keypad"]);
                    let states = a["States"]
                        .as_array()
                        .map(|st| st.iter().map(|x| (x["Image"].as_str().and_then(|i| image_uri(dir, i)), x["Title"].as_str().map(str::to_string).filter(|t| !t.is_empty()))).collect())
                        .unwrap_or_else(|| vec![(None, None)]);
                    Some(SdAction {
                        name: a["Name"].as_str().unwrap_or(&uuid).to_string(),
                        tooltip: a["Tooltip"].as_str().unwrap_or("").to_string(),
                        icon: a["Icon"].as_str().and_then(|i| image_uri(dir, i)),
                        states,
                        keypad: controllers.contains(&"Keypad"),
                        encoder: controllers.contains(&"Encoder"),
                        inspector: a["PropertyInspectorPath"].as_str().map(str::to_string).filter(|p| !p.is_empty()),
                        uuid,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(SdManifest { uuid, name: s("Name"), version: s("Version"), author: s("Author"), code_path, node, platforms, actions, inspector })
}

/// The plugin as HubDeck's own manifest: its actions join the library under the plugin's name.
pub fn to_manifest(sd: SdManifest) -> Manifest {
    let actions = sd
        .actions
        .iter()
        .map(|a| {
            let mut inputs = Vec::new();
            if a.keypad || a.encoder {
                inputs.push(InputKind::Press);
            }
            if a.encoder {
                inputs.push(InputKind::Turn);
            }
            ManifestAction { id: a.uuid.clone(), name: a.name.clone(), description: a.tooltip.clone(), icon: a.icon.clone().unwrap_or_default(), inputs, params: Vec::new() }
        })
        .collect();
    {
        let mut reg = ACTIONS.write().unwrap_or_else(|e| e.into_inner());
        reg.retain(|(p, _)| *p != sd.uuid);
        reg.extend(sd.actions.iter().map(|a| (sd.uuid.clone(), a.clone())));
    }
    let mut m = Manifest {
        id: sd.uuid.clone(),
        name: if sd.name.is_empty() { sd.uuid.clone() } else { sd.name.clone() },
        version: sd.version.clone(),
        command: String::new(),
        actions,
        permissions: Vec::new(),
        sd: None,
    };
    m.sd = Some(std::sync::Arc::new(sd));
    m
}

fn action(uuid: &str) -> Option<SdAction> {
    ACTIONS.read().unwrap_or_else(|e| e.into_inner()).iter().find(|(_, a)| a.uuid == uuid).map(|(_, a)| a.clone())
}

/// Is this a Stream Deck plugin's action (`<plugin>/<action uuid>`)?
pub fn is_sd_action(action_id: &str) -> bool {
    action_id.split_once('/').is_some_and(|(_, uuid)| action(uuid).is_some())
}

/// The picture and title Stream Deck would show for a state when the plugin set none.
pub fn state_look(action_id: &str, state: Option<bool>) -> Option<(Option<String>, Option<String>)> {
    let (_, uuid) = action_id.split_once('/')?;
    let a = action(uuid)?;
    let i = usize::from(state.unwrap_or(false)).min(a.states.len().saturating_sub(1));
    a.states.get(i).cloned()
}

/// The system's version as Stream Deck gives it, dotted numbers ("10.0.19045", "14.5.0"): .NET
/// plugins read it as a `Version` and stop at an empty one (Windows Display Settings, 2026-10-18:
/// "The JSON value could not be converted ... $.application.platformVersion").
fn platform_version() -> String {
    let parts = |v: &str| v.split(|c: char| !c.is_ascii_digit()).filter(|p| !p.is_empty()).map(str::to_string).collect::<Vec<_>>();
    let mut v = if cfg!(windows) {
        // Windows 10 and 11 are both 10.0.<build>, as .NET's Environment.OSVersion says
        let build = sysinfo::System::kernel_version().map(|k| parts(&k)).unwrap_or_default();
        let mut v = vec!["10".to_string(), "0".to_string()];
        v.extend(build.into_iter().take(1));
        v
    } else {
        sysinfo::System::os_version().map(|o| parts(&o)).unwrap_or_default()
    };
    v.truncate(4);
    while v.len() < 3 {
        v.push("0".into());
    }
    v.join(".")
}

/// The `-info` argument and the registration info: the app, the system, the device.
pub fn info(sd: &SdManifest) -> Value {
    json!({
        "application": {"font": if cfg!(windows) { "Segoe UI" } else { ".AppleSystemUIFont" }, "language": "en", "platform": platform(), "platformVersion": platform_version(), "version": SD_VERSION},
        "plugin": {"uuid": sd.uuid, "version": sd.version},
        "devicePixelRatio": 2,
        "colors": {"buttonPressedBackgroundColor": "#303030FF", "buttonPressedBorderColor": "#646464FF", "buttonPressedTextColor": "#969696FF", "disabledColor": "#007AFF59", "highlightColor": "#007AFFFF", "mouseDownColor": "#2EA8FFFF"},
        "devices": [device()],
    })
}

fn device() -> Value {
    json!({"id": DEVICE, "name": "HubDeck", "size": {"columns": 5, "rows": 3}, "type": 7})
}

/// Where a control sits for the plugin: keys are a 5x3 keypad, the rest encoders.
fn place(context: &str) -> (&'static str, Value) {
    let base = context.split(':').next().unwrap_or(context);
    let control = base.rsplit_once('/').map_or(base, |(_, c)| c);
    if let Some(n) = control.strip_prefix("keys.").and_then(|n| n.parse::<u32>().ok()) {
        return ("Keypad", json!({"column": n % 5, "row": n / 5}));
    }
    let column = match control {
        "dial" => 0,
        c => c.rsplit_once('.').and_then(|(_, n)| n.parse::<u32>().ok()).map_or(0, |n| n + 1),
    };
    ("Encoder", json!({"column": column, "row": 0}))
}

fn state_of(context: &str) -> u8 {
    with(&STATES, |s| s.get(context).copied().unwrap_or(0))
}

/// These are the settings HubDeck last told or heard from the plugin for this control.
pub fn settings_known(context: &str, settings: &Value) -> bool {
    with(&SETTINGS, |s| s.get(context) == Some(settings))
}

/// A key-up on a two-state action flips its state, as Stream Deck does; returns the new state.
pub fn toggle_after_key_up(action_id: &str, context: &str) -> Option<bool> {
    let (_, uuid) = action_id.split_once('/')?;
    let a = action(uuid)?;
    (a.states.len() == 2).then(|| {
        with(&STATES, |s| {
            let next = 1 - s.get(context).copied().unwrap_or(0).min(1);
            s.insert(context.to_string(), next);
            next == 1
        })
    })
}

/// Controls held down now.
static HELD: Mutex<Option<std::collections::HashSet<String>>> = Mutex::new(None);

/// When each control was last pressed or turned (for knob feedback on the dial).
static TOUCHED: Mutex<Option<HashMap<String, std::time::Instant>>> = Mutex::new(None);

/// Was this control pressed or turned in the last two seconds?
pub fn touched_lately(context: &str) -> bool {
    with(&TOUCHED, |t| t.get(context).is_some_and(|at| at.elapsed() < std::time::Duration::from_secs(2)))
}

/// A photo a layout shows (a cover): the first JPEG picture among its items; plugin glyphs (PNG
/// icons) are not covers.
pub fn feedback_cover(f: &Value) -> Option<String> {
    f.as_object()?.values().find_map(|v| {
        let s = v.as_str().or_else(|| v.get("value").and_then(Value::as_str))?;
        s.starts_with("data:image/jpeg;base64,").then(|| s.to_string())
    })
}

/// A `setFeedback` payload: (the big text, the title, a bar 0..1). Layouts name their items freely
/// (`value`, `text`, `time`, `count`, `name`...); an item is a string, a number or `{value}`.
pub fn feedback_parts(f: &Value) -> (Option<String>, Option<String>, Option<f32>) {
    let text = |v: &Value| -> Option<String> {
        match v {
            Value::String(s) if !s.starts_with("data:") => Some(s.clone()),
            Value::Number(n) => Some(n.to_string()),
            Value::Object(o) => {
                o.get("value").and_then(|x| x.as_str().filter(|s| !s.starts_with("data:")).map(str::to_string).or_else(|| x.as_f64().filter(|_| !o.contains_key("range")).map(|n| n.to_string())))
            }
            _ => None,
        }
    };
    let bar = |v: &Value| -> Option<f32> {
        let (value, range) = match v {
            Value::Number(n) => (n.as_f64()?, None),
            Value::Object(o) => (o.get("value")?.as_f64()?, o.get("range").cloned()),
            _ => return None,
        };
        let (min, max) = range.map_or((0.0, 100.0), |r| (r["min"].as_f64().unwrap_or(0.0), r["max"].as_f64().unwrap_or(100.0)));
        (max > min).then(|| ((value - min) / (max - min)).clamp(0.0, 1.0) as f32)
    };
    let value = ["value", "text", "time", "count", "name", "extra"].iter().find_map(|k| f.get(*k).and_then(text));
    let title = f.get("title").and_then(text);
    let level = ["indicator", "bar", "gbar", "progress"].iter().find_map(|k| f.get(*k).and_then(bar));
    (value, title, level)
}

/// HubDeck's message to a plugin, in Stream Deck's words.
pub fn outgoing(m: &Value) -> Option<Value> {
    let event = m["event"].as_str()?;
    let context = m["context"].as_str().unwrap_or("");
    let action = m["action"].as_str().unwrap_or("");
    let settings = m.get("settings").cloned().filter(Value::is_object).unwrap_or_else(|| json!({}));
    let (controller, coordinates) = place(context);
    if matches!(event, "keyDown" | "keyUp" | "dialRotate") {
        with(&TOUCHED, |t| t.insert(context.to_string(), std::time::Instant::now()));
    }
    // a dial or knob held down while it turns: Stream Deck's "pressed" rotation (seek, fine steps)
    match event {
        "keyDown" => with(&HELD, |h| h.insert(context.to_string())),
        "keyUp" | "willDisappear" => with(&HELD, |h| h.remove(context)),
        _ => false,
    };
    let mut payload = json!({"settings": settings, "coordinates": coordinates, "controller": controller, "isInMultiAction": false});
    let states = action_states(action);
    if states > 1 {
        payload["state"] = json!(state_of(context));
    }
    let event = match (event, controller) {
        ("willAppear", _) => {
            with(&SETTINGS, |s| s.insert(context.to_string(), settings.clone()));
            "willAppear"
        }
        ("willDisappear", _) => "willDisappear",
        ("keyDown", "Encoder") => "dialDown",
        ("keyUp", "Encoder") => "dialUp",
        ("keyDown", _) => "keyDown",
        ("keyUp", _) => "keyUp",
        ("dialRotate", _) => {
            payload["ticks"] = m["ticks"].clone();
            let held = with(&HELD, |h| h.contains(context));
            payload["pressed"] = json!(held);
            "dialRotate"
        }
        _ => return None,
    };
    if controller == "Encoder" {
        payload.as_object_mut()?.remove("isInMultiAction");
    }
    log::debug!("to Stream Deck plugin: {event} {context} {}", payload.get("pressed").map(|p| format!("pressed={p}")).unwrap_or_default());
    Some(json!({"event": event, "action": action, "context": context, "device": DEVICE, "payload": payload}))
}

fn action_states(uuid: &str) -> usize {
    action(uuid).map_or(1, |a| a.states.len())
}

/// What a plugin's message asks for.
#[derive(Debug, PartialEq)]
pub enum Incoming {
    /// For the engine, in HubDeck's plugin words.
    Engine(Value),
    /// An answer straight back to the plugin.
    Reply(Value),
    /// For the settings page of a context.
    Inspector(String, Value),
    Nothing,
}

/// A plugin's message, translated. `dir` is its folder (for pictures by path).
pub fn incoming(plugin: &str, dir: &Path, m: &Value) -> Vec<Incoming> {
    let context = m["context"].as_str().unwrap_or("").to_string();
    let p = &m["payload"];
    match m["event"].as_str().unwrap_or("") {
        "setTitle" => vec![Incoming::Engine(json!({"event": "setTitle", "context": context, "title": p["title"]}))],
        "setImage" => {
            let image = p["image"].as_str().and_then(|i| normalize_image(dir, i));
            vec![Incoming::Engine(json!({"event": "setImage", "context": context, "image": image}))]
        }
        "setState" => {
            let state = p["state"].as_u64().unwrap_or(0).min(1) as u8;
            with(&STATES, |s| s.insert(context.clone(), state));
            vec![Incoming::Engine(json!({"event": "setState", "context": context, "state": state == 1}))]
        }
        "showOk" | "showAlert" => vec![Incoming::Engine(json!({"event": "flash", "context": context, "ok": m["event"] == "showOk"}))],
        "openUrl" => vec![Incoming::Engine(json!({"event": "openUrl", "url": p["url"]}))],
        "setSettings" => {
            let settings = if p.is_object() { p.clone() } else { json!({}) };
            with(&SETTINGS, |s| s.insert(context.clone(), settings.clone()));
            vec![
                Incoming::Engine(json!({"event": "sdSettings", "context": context, "settings": settings})),
                Incoming::Inspector(context.clone(), json!({"event": "didReceiveSettings", "action": m["action"], "context": context, "device": DEVICE, "payload": {"settings": settings}})),
            ]
        }
        "getSettings" => {
            let settings = with(&SETTINGS, |s| s.get(&context).cloned()).unwrap_or_else(|| json!({}));
            let (controller, coordinates) = place(&context);
            vec![Incoming::Reply(
                json!({"event": "didReceiveSettings", "action": m["action"], "context": context, "device": DEVICE, "id": m["id"], "payload": {"settings": settings, "coordinates": coordinates, "controller": controller, "isInMultiAction": false}}),
            )]
        }
        "setGlobalSettings" => {
            save_global(plugin, p);
            // the plugin's open settings pages hear it
            with(&INSPECTORS, |i| i.iter().filter(|(_, (pl, _, _))| pl == plugin).map(|(_, (_, _, c))| c.clone()).collect::<Vec<_>>())
                .into_iter()
                .map(|c| Incoming::Inspector(c, json!({"event": "didReceiveGlobalSettings", "payload": {"settings": p}})))
                .collect()
        }
        "getGlobalSettings" => vec![Incoming::Reply(json!({"event": "didReceiveGlobalSettings", "id": m["id"], "payload": {"settings": global(plugin)}}))],
        "setFeedback" => {
            // pictures in a layout (`icon`, a cover...) as data URIs HubDeck can draw
            let mut f = p.clone();
            if let Some(o) = f.as_object_mut() {
                for (_, v) in o.iter_mut() {
                    let pic = v.as_str().map(str::to_string).or_else(|| v.get("value").and_then(Value::as_str).map(str::to_string));
                    if let Some(pic) = pic.filter(|p| p.starts_with("data:image") || [".png", ".jpg", ".jpeg", ".svg"].iter().any(|e| p.ends_with(e)) || image_uri(dir, p).is_some()) {
                        let uri = normalize_image(dir, &pic);
                        match v {
                            Value::Object(o) => {
                                o.insert("value".into(), json!(uri));
                            }
                            _ => *v = json!(uri),
                        }
                    }
                }
            }
            log::debug!(
                "feedback {context}: {}",
                f.as_object().map(|o| o.iter().map(|(k, v)| format!("{k}={}", v.to_string().chars().take(40).collect::<String>())).collect::<Vec<_>>().join(" ")).unwrap_or_default()
            );
            vec![Incoming::Engine(json!({"event": "sdFeedback", "context": context, "feedback": f}))]
        }
        "sendToPropertyInspector" => vec![Incoming::Inspector(context.clone(), json!({"event": "sendToPropertyInspector", "action": m["action"], "context": context, "payload": p}))],
        "logMessage" => vec![Incoming::Engine(json!({"event": "log", "message": p["message"]}))],
        // profiles bundled with plugins, trigger descriptions, resources: not used by HubDeck
        _ => vec![Incoming::Nothing],
    }
}

fn global_path(plugin: &str) -> Option<PathBuf> {
    Some(DATA.get()?.join("plugin-settings").join(format!("{plugin}.json")))
}

pub fn global(plugin: &str) -> Value {
    global_path(plugin).and_then(|p| std::fs::read(p).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_else(|| json!({}))
}

fn save_global(plugin: &str, settings: &Value) {
    if let Some(p) = global_path(plugin) {
        let _ = std::fs::create_dir_all(p.parent().unwrap_or(Path::new(".")));
        let _ = super::profile::write_atomic(&p, settings.to_string().as_bytes());
    }
}

// ---------- starting a plugin ----------

/// How a plugin runs here.
pub enum Launch {
    Process(std::process::Command),
    /// An HTML plugin: this address in a hidden window.
    Html(String),
}

/// Whether the plugin is made for this system (a plugin without a list of systems is tried).
pub fn runs_here(sd: &SdManifest) -> Result<(), String> {
    if !sd.platforms.is_empty() && !sd.platforms.iter().any(|p| p == platform()) {
        return Err(format!("made for {} only", sd.platforms.join(" and ")));
    }
    Ok(())
}

pub fn launch(sd: &SdManifest, dir: &Path, port: u16, uuid: &str) -> Result<Launch, String> {
    runs_here(sd)?;
    let info = info(sd).to_string();
    let args = ["-port".to_string(), port.to_string(), "-pluginUUID".into(), uuid.to_string(), "-registerEvent".into(), "registerPlugin".into(), "-info".into(), info.clone()];
    let entry = inside(dir, &sd.code_path).ok_or("entry outside the plugin folder")?;
    if sd.code_path.ends_with(".html") || sd.code_path.ends_with(".htm") {
        let port_files = FILES_PORT.get().ok_or("the plugin file server is not running")?;
        let q = |s: &str| percent_encode(s);
        return Ok(Launch::Html(format!(
            "http://127.0.0.1:{port_files}/{}/{}?hubdeck=1&port={port}&uuid={}&event=registerPlugin&info={}",
            q(&sd.uuid),
            sd.code_path.trim_start_matches('/'),
            q(uuid),
            q(&info)
        )));
    }
    if sd.node {
        let node = find_node().ok_or("needs Node.js 20 or newer (nodejs.org); none found on this computer")?;
        let mut cmd = std::process::Command::new(node);
        cmd.arg(&entry).args(&args).current_dir(dir);
        return Ok(Launch::Process(cmd));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // unzipped programs lose their executable bit
        if let Ok(meta) = std::fs::metadata(&entry) {
            let mut perm = meta.permissions();
            perm.set_mode(perm.mode() | 0o755);
            let _ = std::fs::set_permissions(&entry, perm);
        }
    }
    let mut cmd = std::process::Command::new(&entry);
    cmd.args(&args).current_dir(entry.parent().unwrap_or(dir));
    Ok(Launch::Process(cmd))
}

/// Open an HTML plugin in its hidden window.
pub fn open_html(plugin: &str, url: &str) -> Result<(), String> {
    HTML.get().ok_or("no window for HTML plugins")?(plugin, url);
    Ok(())
}

fn percent_encode(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

/// Node.js for plugins: on the PATH, or where installers and version managers put it (an app
/// started from the Dock or Finder has a short PATH).
pub fn find_node() -> Option<PathBuf> {
    let exe = if cfg!(windows) { "node.exe" } else { "node" };
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    if cfg!(windows) {
        for var in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
            if let Some(base) = std::env::var_os(var) {
                dirs.push(PathBuf::from(&base).join("nodejs"));
                dirs.push(PathBuf::from(&base).join("Programs").join("nodejs"));
            }
        }
    } else {
        dirs.extend(["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"].map(PathBuf::from));
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            dirs.push(home.join(".volta/bin"));
            dirs.push(home.join(".local/bin"));
            // nvm: the newest installed version
            if let Ok(rd) = std::fs::read_dir(home.join(".nvm/versions/node")) {
                let mut v: Vec<PathBuf> = rd.flatten().map(|e| e.path().join("bin")).collect();
                v.sort();
                dirs.extend(v.into_iter().rev());
            }
        }
    }
    if let Some(data) = DATA.get() {
        dirs.push(data.join("node").join("bin"));
        dirs.push(data.join("node"));
    }
    dirs.into_iter().map(|d| d.join(exe)).find(|p| p.is_file())
}

// ---------- settings pages (property inspectors) ----------

/// The address of a control's settings page, registered for one connection; None when the action
/// has none.
pub fn inspector_url(plugin: &str, dir: &Path, action_id: &str, context: &str, settings: &Value, sd: &SdManifest) -> Option<String> {
    let (_, uuid) = action_id.split_once('/')?;
    let a = sd.actions.iter().find(|a| a.uuid == uuid)?;
    let page = a.inspector.clone().or_else(|| sd.inspector.clone())?;
    inside(dir, &page).filter(|p| p.is_file())?;
    let port_files = FILES_PORT.get()?;
    let token = uuid::Uuid::new_v4().simple().to_string();
    with(&INSPECTORS, |i| i.insert(token.clone(), (plugin.to_string(), uuid.to_string(), context.to_string())));
    let (controller, coordinates) = place(context);
    let action_info = json!({"action": uuid, "context": context, "device": DEVICE, "payload": {"settings": settings, "coordinates": coordinates, "controller": controller, "isInMultiAction": false}});
    with(&SETTINGS, |s| s.insert(context.to_string(), settings.clone()));
    let q = percent_encode;
    Some(format!(
        "http://127.0.0.1:{port_files}/{}/{}?hubdeck=1&port={}&uuid={}&event=registerPropertyInspector&info={}&actionInfo={}",
        q(plugin),
        page.trim_start_matches('/'),
        super::plugins::port(),
        q(&token),
        q(&info(sd).to_string()),
        q(&action_info.to_string())
    ))
}

/// Who registered with this uuid as a settings page: (plugin, action, context).
pub fn inspector_for(uuid: &str) -> Option<(String, String, String)> {
    with(&INSPECTORS, |i| i.get(uuid).cloned())
}

pub fn inspector_connected(context: &str, tx: Option<Sender<String>>) {
    with(&INSPECTOR_OUT, |o| match tx {
        Some(tx) => o.insert(context.to_string(), tx),
        None => o.remove(context),
    });
}

pub fn to_inspector(context: &str, m: &Value) {
    with(&INSPECTOR_OUT, |o| {
        if let Some(tx) = o.get(context) {
            let _ = tx.send(m.to_string());
        }
    });
}

/// A settings page's message: what goes to the plugin, the engine or back to the page.
pub fn from_inspector(plugin: &str, action: &str, context: &str, m: &Value) -> Vec<(Target, Value)> {
    let p = &m["payload"];
    match m["event"].as_str().unwrap_or("") {
        "setSettings" => {
            let settings = if p.is_object() { p.clone() } else { json!({}) };
            with(&SETTINGS, |s| s.insert(context.to_string(), settings.clone()));
            let (controller, coordinates) = place(context);
            vec![
                (Target::Engine, json!({"event": "sdSettings", "context": context, "settings": settings})),
                (
                    Target::Plugin,
                    json!({"event": "didReceiveSettings", "action": action, "context": context, "device": DEVICE, "payload": {"settings": settings, "coordinates": coordinates, "controller": controller, "isInMultiAction": false}}),
                ),
            ]
        }
        "getSettings" => {
            let settings = with(&SETTINGS, |s| s.get(context).cloned()).unwrap_or_else(|| json!({}));
            vec![(Target::Page, json!({"event": "didReceiveSettings", "action": action, "context": context, "device": DEVICE, "id": m["id"], "payload": {"settings": settings}}))]
        }
        "setGlobalSettings" => {
            save_global(plugin, p);
            vec![(Target::Plugin, json!({"event": "didReceiveGlobalSettings", "payload": {"settings": p}}))]
        }
        "getGlobalSettings" => vec![(Target::Page, json!({"event": "didReceiveGlobalSettings", "id": m["id"], "payload": {"settings": global(plugin)}}))],
        "sendToPlugin" => vec![(Target::Plugin, json!({"event": "sendToPlugin", "action": action, "context": context, "payload": p}))],
        "openUrl" => vec![(Target::Engine, json!({"event": "openUrl", "url": p["url"]}))],
        "logMessage" => vec![(Target::Engine, json!({"event": "log", "message": p["message"]}))],
        _ => Vec::new(),
    }
}

#[derive(Debug, PartialEq)]
pub enum Target {
    Plugin,
    Engine,
    Page,
}

// ---------- the file server (settings pages and HTML plugins) ----------

/// Serves `<plugins>/<uuid>.sdPlugin/...` at `http://127.0.0.1:<port>/<uuid>/...`; an HTML page
/// opened with `?hubdeck=1` gets a script that connects it the Stream Deck way after it loads.
pub fn start_files(data_dir: &Path, plugins_dir: PathBuf) {
    let _ = DATA.set(data_dir.to_path_buf());
    let server = match tiny_http::Server::http("127.0.0.1:0") {
        Ok(s) => s,
        Err(e) => {
            log::warn!("plugin file server: {e}");
            return;
        }
    };
    let port = server.server_addr().to_ip().map(|a| a.port()).unwrap_or(0);
    let _ = FILES_PORT.set(port);
    std::thread::Builder::new()
        .name("plugin-files".into())
        .spawn(move || {
            for req in server.incoming_requests() {
                // only pages of this computer, by this address: a web page that points its own name
                // at 127.0.0.1 (DNS rebinding) sends another Host
                let host_ok = req.headers().iter().find(|h| h.field.equiv("Host")).is_some_and(|h| {
                    let v = h.value.as_str();
                    v == format!("127.0.0.1:{port}") || v == format!("localhost:{port}")
                });
                if !host_ok {
                    let _ = req.respond(tiny_http::Response::from_data(b"forbidden".to_vec()).with_status_code(403));
                    continue;
                }
                let url = req.url().to_string();
                let (path, query) = url.split_once('?').unwrap_or((&url, ""));
                let path = percent_decode(path);
                let mut parts = path.trim_start_matches('/').splitn(2, '/');
                let (plugin, rel) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
                let plugin = if plain_id(plugin) { plugin } else { "" };
                let file = (!plugin.is_empty())
                    .then(|| super::plugins::folder_of(plugin))
                    .flatten()
                    .and_then(|d| inside(&d, rel))
                    .filter(|p| p.is_file())
                    .or_else(|| (!plugin.is_empty()).then(|| inside(&plugins_dir.join(format!("{plugin}.sdPlugin")), rel)).flatten().filter(|p| p.is_file()));
                let response = match file.and_then(|f| std::fs::read(&f).ok().map(|b| (f, b))) {
                    Some((f, mut body)) => {
                        let ext = f.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
                        if matches!(ext.as_str(), "html" | "htm") && query.contains("hubdeck=1") {
                            body.extend_from_slice(CONNECT_SCRIPT.as_bytes());
                        }
                        tiny_http::Response::from_data(body).with_header(format!("Content-Type: {}", mime(&ext)).parse::<tiny_http::Header>().expect("header"))
                    }
                    None => tiny_http::Response::from_data(b"not found".to_vec()).with_status_code(404),
                };
                let _ = req.respond(response);
            }
        })
        .expect("spawn plugin files");
    log::info!("plugin files on 127.0.0.1:{port}");
}

/// After the page loads, connect it as Stream Deck would (`connectElgatoStreamDeckSocket`, or the
/// OpenAction and older names).
const CONNECT_SCRIPT: &str = r#"
<script>
(function () {
  var q = new URLSearchParams(location.search);
  function go() {
    var f = window.connectElgatoStreamDeckSocket || window.connectOpenActionSocket || window.connectSocket;
    if (typeof f !== "function") return;
    if (q.get("actionInfo")) f(q.get("port"), q.get("uuid"), q.get("event"), q.get("info"), q.get("actionInfo"));
    else f(q.get("port"), q.get("uuid"), q.get("event"), q.get("info"));
  }
  if (document.readyState === "complete") setTimeout(go, 0); else window.addEventListener("load", go);
  // a settings page in HubDeck's narrow inspector: shrink it to fit the width, tell the window its height
  if (!q.get("actionInfo") || window.parent === window) return;
  // fields no wider than the panel, and no page stretched to the box: Stream Deck's styles give
  // fields fixed widths and the page the box's full height, which ran past the panel and grew the
  // box to its limit (owner's Toggle Audio Device, 2026-10-18; plugin-inspectors.mjs measures it)
  function narrow() {
    if (document.getElementById("hubdeck-fit")) return;
    var s = document.createElement("style");
    s.id = "hubdeck-fit";
    // Stream Deck's sdpi.css also sizes fields by --sdpi-width (224 px beside a 100 px label) and
    // its wrapper hides what runs past it and takes the box's height (HWiNFO, Audio Mute)
    s.textContent = "html,body{overflow-x:hidden!important;min-height:0!important;height:auto!important;scrollbar-width:none}" +
      "::-webkit-scrollbar{display:none}" +
      ":root{--sdpi-width:min(224px,calc(100vw - 132px))!important}" +
      ".sdpi-wrapper,.localbody{overflow:visible!important;height:auto!important;min-height:0!important;width:auto!important;max-width:100%!important}" +
      "select,input,textarea,.sdpi-item-value{max-width:100%!important;min-width:0!important;box-sizing:border-box}" +
      ".sdpi-item{max-width:100%}";
    (document.head || document.documentElement).appendChild(s);
  }

  // where what is drawn ends: fields and the innermost elements (text, icons), not the boxes
  // around them, which can be as tall as the frame or cut what runs past them
  var DRAWN = { INPUT: 1, SELECT: 1, TEXTAREA: 1, BUTTON: 1, IMG: 1, CANVAS: 1, SVG: 1, HR: 1, VIDEO: 1 };
  function drawnEdges() {
    var right = 0, bottom = 0;
    var all = [];
    // web components (sdpi-components: Essentials for Spotify) draw in their own shadow roots
    (function collect(root) {
      var els = root.querySelectorAll("*");
      for (var k = 0; k < els.length; k++) {
        all.push(els[k]);
        if (els[k].shadowRoot) collect(els[k].shadowRoot);
      }
    })(document.body);
    for (var i = 0; i < all.length; i++) {
      var el = all[i];
      if (!DRAWN[el.tagName.toUpperCase()] && el.children.length) continue;
      if (el.parentElement && el.parentElement.closest("select,svg")) continue;
      // what a closed <details> holds keeps its boxes but is not shown (HWiNFO's help texts)
      if (el.checkVisibility && !el.checkVisibility({ contentVisibilityAuto: true, visibilityProperty: true })) continue;
      var r = el.getBoundingClientRect();
      if (r.height > 0 && r.width > 0 && getComputedStyle(el).visibility !== "hidden") {
        right = Math.max(right, r.right + window.scrollX);
        bottom = Math.max(bottom, r.bottom + window.scrollY);
      }
    }
    return { right: right, bottom: bottom };
  }
  function fit() {
    narrow();
    document.body.style.zoom = "";
    document.body.style.width = "";
    var edges = drawnEdges();
    var w = Math.ceil(Math.max(document.documentElement.scrollWidth, edges.right));
    if (w > window.innerWidth + 2) {
      // laid out as wide as it needs, then made smaller: a zoomed page laid out anew in the room
      // the zoom gives grows again (sdpi-components)
      document.body.style.width = w + "px";
      edges = drawnEdges();
      document.body.style.zoom = String(window.innerWidth / w);
    }
    var z = parseFloat(document.body.style.zoom || "1");
    window.parent.postMessage({ hubdeckInspectorHeight: Math.ceil(edges.bottom * z) + 12 }, "*");
  }
  window.addEventListener("load", function () {
    fit();
    if (window.ResizeObserver) new ResizeObserver(function () { fit(); }).observe(document.body);
  });
})();
</script>
"#;

fn mime(ext: &str) -> &'static str {
    match ext {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}

// ---------- installing ----------

/// Download a plugin file (https only, at most 300 MB).
pub fn download(url: &str) -> Result<Vec<u8>, String> {
    if !url.starts_with("https://") {
        return Err("plugins are downloaded over https only".into());
    }
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(300))).build().into();
    let mut res = agent.get(url).header("User-Agent", "HubDeck").call().map_err(|e| format!("download failed: {e}"))?;
    res.body_mut().with_config().limit(300 * 1024 * 1024).read_to_vec().map_err(|e| format!("download failed: {e}"))
}

/// JSON bytes without the UTF-8 byte order mark that Windows tools write (Flight Tracker's
/// manifest, 2026-10-17): serde_json refuses it.
pub fn without_bom(bytes: &[u8]) -> &[u8] {
    bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes)
}

/// The `.streamDeckPlugin` inside a zip that holds only that file (DevOps for StreamDeck releases
/// `<uuid>.streamDeckPlugin.zip`, 2026-10-17); any other file as it is.
fn unwrap_zipped(bytes: &[u8]) -> std::borrow::Cow<'_, [u8]> {
    use std::io::Read as _;
    let Ok(mut zip) = zip::ZipArchive::new(std::io::Cursor::new(bytes)) else { return bytes.into() };
    let names: Vec<String> = (0..zip.len()).filter_map(|i| zip.by_index(i).ok().map(|f| f.name().unwrap_or_default().replace('\\', "/"))).collect();
    if names.iter().any(|n| n.split('/').any(|p| p.ends_with(".sdPlugin"))) {
        return bytes.into();
    }
    let inner: Vec<&String> = names.iter().filter(|n| n.to_lowercase().ends_with(".streamdeckplugin")).collect();
    let [inner] = inner[..] else { return bytes.into() };
    let mut data = Vec::new();
    match zip.by_name(inner).map(|mut f| f.read_to_end(&mut data)) {
        Ok(Ok(_)) => data.into(),
        _ => bytes.into(),
    }
}

/// Why a plugin Elgato encrypted cannot run here.
pub const ENCRYPTED: &str = "this plugin is encrypted for Elgato's Stream Deck app; other apps cannot read it";

/// Files that start with "ELGATO" are encrypted for Elgato's app (RME TotalMix 5.5.1, 2026-10-17).
pub fn is_encrypted(bytes: &[u8]) -> bool {
    bytes.starts_with(b"ELGATO")
}

/// The plugin uuid a `.streamDeckPlugin` file holds (its `<uuid>.sdPlugin` folder).
pub fn install_root(bytes: &[u8]) -> Result<String, String> {
    let bytes = &*unwrap_zipped(bytes);
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| format!("not a Stream Deck plugin file: {e}"))?;
    (0..zip.len())
        .filter_map(|i| zip.by_index(i).ok().map(|f| f.name().unwrap_or_default().replace('\\', "/")))
        .find_map(|n| n.split('/').find(|p| p.ends_with(".sdPlugin")).map(|p| p.trim_end_matches(".sdPlugin").to_string()))
        .ok_or_else(|| "no <uuid>.sdPlugin folder inside".to_string())
}

/// Install a `.streamDeckPlugin` (a zip holding `<uuid>.sdPlugin/`) into the plugins folder;
/// returns the plugin's uuid. An installed version is replaced.
pub fn install(bytes: &[u8], plugins_dir: &Path) -> Result<String, String> {
    use std::io::Read as _;
    let bytes = &*unwrap_zipped(bytes);
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| format!("not a Stream Deck plugin file: {e}"))?;
    // a zip made on Windows may separate folders with backslashes (Weather, 2026-10-16)
    let root = (0..zip.len())
        .filter_map(|i| zip.by_index(i).ok().map(|f| f.name().unwrap_or_default().replace('\\', "/")))
        .find_map(|n| n.split('/').find(|p| p.ends_with(".sdPlugin")).map(str::to_string))
        .ok_or("no <uuid>.sdPlugin folder inside")?;
    let target = plugins_dir.join(&root);
    let tmp = plugins_dir.join(format!(".{root}.part"));
    let _ = std::fs::remove_dir_all(&tmp);
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).map_err(|e| e.to_string())?;
        // the path after `<uuid>.sdPlugin/`, as plain parts: nothing may climb out (`..`, a drive)
        let name = f.name().unwrap_or_default().replace('\\', "/");
        let parts: Vec<&str> = name.split('/').filter(|p| !p.is_empty()).collect();
        let Some(at) = parts.iter().position(|p| *p == root) else { continue };
        let rel = &parts[at + 1..];
        if rel.iter().any(|p| *p == ".." || *p == "." || p.contains(':')) {
            continue;
        }
        let out = rel.iter().fold(tmp.clone(), |acc, p| acc.join(p));
        if f.is_dir() || name.ends_with('/') {
            std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
            continue;
        }
        std::fs::create_dir_all(out.parent().unwrap_or(&tmp)).map_err(|e| e.to_string())?;
        let mut data = Vec::new();
        f.read_to_end(&mut data).map_err(|e| e.to_string())?;
        std::fs::write(&out, data).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        if let Some(mode) = f.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&out, std::fs::Permissions::from_mode(mode & 0o777));
        }
    }
    let raw = std::fs::read(tmp.join("manifest.json")).unwrap_or_default();
    let Some(manifest) = serde_json::from_slice::<Value>(without_bom(&raw)).ok() else {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(if is_encrypted(&raw) { ENCRYPTED } else { "the plugin has no readable manifest.json" }.into());
    };
    // a readable manifest with its program encrypted (the folder is still `.<uuid>.sdPlugin.part`)
    let mut named = manifest.clone();
    if named.get("UUID").and_then(Value::as_str).is_none_or(str::is_empty) {
        named["UUID"] = Value::from(root.trim_end_matches(".sdPlugin"));
    }
    if read(&named, &tmp).is_ok_and(|sd| std::fs::read(tmp.join(&sd.code_path)).is_ok_and(|b| is_encrypted(&b))) {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(ENCRYPTED.into());
    }
    if !is_sd_manifest(&manifest) {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err("not a Stream Deck plugin manifest".into());
    }
    // an OpenAction device plugin drives hardware of its own and has nothing for the deck (Touchy-Pad)
    if manifest.get("DeviceNamespace").is_some() && manifest["Actions"].as_array().is_some_and(Vec::is_empty) {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err("this plugin adds a device of its own to OpenDeck and has no actions for the deck".into());
    }
    // made for other systems only (Volume Controller is Linux only): say so, not "broken"
    let systems: Vec<String> = manifest.get("OS").and_then(Value::as_array).map(|a| a.iter().filter_map(|o| o["Platform"].as_str().map(str::to_string)).collect()).unwrap_or_default();
    if !systems.is_empty() && !systems.iter().any(|p| p == platform()) {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(format!("this plugin is made for {} only", systems.join(", ")));
    }
    let _ = std::fs::remove_dir_all(&target);
    std::fs::rename(&tmp, &target).map_err(|e| e.to_string())?;
    Ok(root.trim_end_matches(".sdPlugin").to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// .NET plugins read it as a Version: dotted numbers, three or four (Windows Display Settings).
    #[test]
    fn the_system_version_is_dotted_numbers() {
        let v = platform_version();
        let parts: Vec<&str> = v.split('.').collect();
        assert!((3..=4).contains(&parts.len()) && parts.iter().all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit())), "{v}");
        if cfg!(windows) {
            assert!(v.starts_with("10.0.") && v != "10.0.0", "{v}");
        }
        assert_eq!(info(&SdManifest::default())["application"]["platformVersion"], v);
    }

    #[test]
    fn openaction_manifests_and_windows_zips_install() {
        // CodePaths by target, no CodePath, no UUID (the catalog's OpenAction plugins)
        let m = serde_json::json!({"Name": "System Information", "Actions": [{"UUID": "x.cpu"}], "CodePaths": {"x86_64-pc-windows-msvc": "a.exe", "aarch64-apple-darwin": "a-arm", "x86_64-apple-darwin": "a-mac"}, "CodePathWin": "w.exe", "CodePathMac": "m"});
        assert!(is_sd_manifest(&m));
        let dir = std::env::temp_dir().join("x.sys.sdPlugin");
        let sd = read(&m, &dir).unwrap();
        assert_eq!(sd.uuid, "x.sys", "the folder names it");
        assert_eq!(sd.code_path, m["CodePaths"][target_triple()].as_str().unwrap(), "the native build for this machine");
        // a zip made on Windows: folders with backslashes, and a plugin for Linux only says so
        let zip = |manifest: &Value| {
            let mut buf = std::io::Cursor::new(Vec::new());
            {
                let mut z = zip::ZipWriter::new(&mut buf);
                let o = zip::write::SimpleFileOptions::default();
                z.start_file(r"x.w.sdPlugin\manifest.json", o).unwrap();
                std::io::Write::write_all(&mut z, manifest.to_string().as_bytes()).unwrap();
                z.start_file(r"x.w.sdPlugin\bin\..\..\evil.txt", o).unwrap();
                z.finish().unwrap();
            }
            buf.into_inner()
        };
        let plugins = std::env::temp_dir().join(format!("hubdeck-sd-test-{}", std::process::id()));
        std::fs::create_dir_all(&plugins).unwrap();
        let ok = serde_json::json!({"Actions": [], "CodePath": "a.js"});
        assert_eq!(install(&zip(&ok), &plugins).unwrap(), "x.w");
        assert!(plugins.join("x.w.sdPlugin").join("manifest.json").exists());
        assert!(!plugins.join("evil.txt").exists() && !std::env::temp_dir().join("evil.txt").exists(), "nothing climbs out");
        let linux = serde_json::json!({"Actions": [], "CodePath": "a", "OS": [{"Platform": "linux"}]});
        assert_eq!(install(&zip(&linux), &plugins).unwrap_err(), "this plugin is made for linux only");
        let device = serde_json::json!({"Actions": [], "CodePath": "a", "DeviceNamespace": "tp"});
        assert!(install(&zip(&device), &plugins).unwrap_err().contains("device of its own"));
        // a .streamDeckPlugin zipped once more (DevOps for StreamDeck)
        let wrapped = {
            let mut buf = std::io::Cursor::new(Vec::new());
            {
                let mut z = zip::ZipWriter::new(&mut buf);
                let o = zip::write::SimpleFileOptions::default();
                z.start_file(".gitkeep", o).unwrap();
                z.start_file("x.w.streamDeckPlugin", o).unwrap();
                std::io::Write::write_all(&mut z, &zip(&ok)).unwrap();
                z.finish().unwrap();
            }
            buf.into_inner()
        };
        assert_eq!(install_root(&wrapped).unwrap(), "x.w");
        assert_eq!(install(&wrapped, &plugins).unwrap(), "x.w");
        assert_eq!(without_bom(b"\xEF\xBB\xBF{}"), b"{}", "Flight Tracker's manifest");
        assert_eq!(without_bom(b"{}"), b"{}");
        // Elgato's encrypted files
        let encrypted = {
            let mut buf = std::io::Cursor::new(Vec::new());
            {
                let mut z = zip::ZipWriter::new(&mut buf);
                z.start_file("x.e.sdPlugin/manifest.json", zip::write::SimpleFileOptions::default()).unwrap();
                std::io::Write::write_all(&mut z, b"ELGATO\x01\x00\xb0^").unwrap();
                z.finish().unwrap();
            }
            buf.into_inner()
        };
        assert!(install(&encrypted, &plugins).unwrap_err().contains("encrypted"));
        assert!(!plugins.join(".x.e.sdPlugin.part").exists(), "nothing left behind");
        let _ = std::fs::remove_dir_all(&plugins);
    }

    fn manifest() -> Value {
        json!({
            "UUID": "com.example.sd", "Name": "Example", "Version": "1.0.0.0", "Author": "Someone",
            "CodePath": "bin/plugin.js", "Nodejs": {"Version": "20"},
            "OS": [{"Platform": "mac", "MinimumVersion": "12"}, {"Platform": "windows", "MinimumVersion": "10"}],
            "SDKVersion": 2, "Software": {"MinimumVersion": "6.4"},
            "Actions": [
                {"UUID": "com.example.sd.toggle", "Name": "Toggle", "Icon": "imgs/none", "States": [{"Image": "a"}, {"Image": "b", "Title": "On"}], "PropertyInspectorPath": "pi.html"},
                {"UUID": "com.example.sd.volume", "Name": "Volume", "Controllers": ["Encoder"], "States": [{"Image": "c"}]}
            ]
        })
    }

    #[test]
    fn a_stream_deck_manifest_becomes_library_actions() {
        let dir = std::env::temp_dir().join("com.example.sd.sdPlugin");
        assert!(is_sd_manifest(&manifest()));
        let sd = read(&manifest(), &dir).unwrap();
        assert!(sd.node);
        assert_eq!(sd.code_path, "bin/plugin.js");
        assert_eq!(sd.actions[0].states.len(), 2);
        assert_eq!(sd.actions[0].states[1].1.as_deref(), Some("On"));
        assert!(sd.actions[1].encoder && !sd.actions[1].keypad);
        let m = to_manifest(sd);
        assert_eq!(m.id, "com.example.sd");
        let defs = m.action_defs();
        assert_eq!(defs[0].id, "com.example.sd/com.example.sd.toggle");
        assert_eq!(defs[1].inputs, vec![InputKind::Press, InputKind::Turn]);
    }

    #[test]
    fn messages_go_both_ways_in_stream_deck_words() {
        let dir = std::env::temp_dir().join("com.example.sd.sdPlugin");
        let _ = to_manifest(read(&manifest(), &dir).unwrap());
        // a key press on keys.7: column 2, row 1, with its state
        let down = outgoing(&json!({"event": "keyDown", "action": "com.example.sd.toggle", "context": "keys.7", "settings": {"a": 1}})).unwrap();
        assert_eq!(down["event"], "keyDown");
        assert_eq!(down["payload"]["coordinates"], json!({"column": 2, "row": 1}));
        assert_eq!(down["payload"]["settings"]["a"], 1);
        assert_eq!(down["payload"]["state"], 0);
        // the dial's press and turn are an encoder's
        let press = outgoing(&json!({"event": "keyDown", "action": "com.example.sd.volume", "context": "dial", "settings": {}})).unwrap();
        assert_eq!(press["event"], "dialDown");
        let turn = outgoing(&json!({"event": "dialRotate", "action": "com.example.sd.volume", "context": "knobs.1", "settings": {}, "ticks": -2})).unwrap();
        assert_eq!(turn["payload"]["pressed"], false);
        outgoing(&json!({"event": "keyDown", "action": "com.example.sd.volume", "context": "knobs.1", "settings": {}}));
        let held = outgoing(&json!({"event": "dialRotate", "action": "com.example.sd.volume", "context": "knobs.1", "settings": {}, "ticks": 1})).unwrap();
        assert_eq!(held["payload"]["pressed"], true);
        outgoing(&json!({"event": "keyUp", "action": "com.example.sd.volume", "context": "knobs.1", "settings": {}}));
        assert_eq!((turn["payload"]["ticks"].as_i64(), turn["payload"]["controller"].as_str()), (Some(-2), Some("Encoder")));
        // a key-up flips a two-state action
        assert_eq!(toggle_after_key_up("com.example.sd/com.example.sd.toggle", "keys.7"), Some(true));
        assert_eq!(toggle_after_key_up("com.example.sd/com.example.sd.volume", "dial"), None);
        // back: titles, pictures, settings
        let title = incoming("com.example.sd", &dir, &json!({"event": "setTitle", "context": "keys.7", "payload": {"title": "Hi"}}));
        assert_eq!(title, vec![Incoming::Engine(json!({"event": "setTitle", "context": "keys.7", "title": "Hi"}))]);
        let svg = incoming("com.example.sd", &dir, &json!({"event": "setImage", "context": "keys.7", "payload": {"image": "data:image/svg+xml;charset=utf8,%3Csvg%3E%3C/svg%3E"}}));
        let Incoming::Engine(v) = &svg[0] else { panic!() };
        assert!(super::super::images::safe_data_uri(v["image"].as_str().unwrap()));
        let set = incoming("com.example.sd", &dir, &json!({"event": "setSettings", "context": "keys.7", "payload": {"b": 2}}));
        assert!(matches!(&set[0], Incoming::Engine(v) if v["event"] == "sdSettings"));
        let get = incoming("com.example.sd", &dir, &json!({"event": "getSettings", "context": "keys.7"}));
        assert!(matches!(&get[0], Incoming::Reply(v) if v["payload"]["settings"]["b"] == 2));
    }

    #[test]
    fn feedback_items_by_any_name() {
        let (v, t, b) = feedback_parts(&json!({"text": {"value": "45%", "opacity": 1.0}, "indicator": {"value": 45}}));
        assert_eq!((v.as_deref(), t, b), (Some("45%"), None, Some(0.45)));
        let (v, t, _) = feedback_parts(&json!({"title": "Playback Control", "time": "1:02 / 3:30"}));
        assert_eq!((v.as_deref(), t.as_deref()), (Some("1:02 / 3:30"), Some("Playback Control")));
        let (_, _, b) = feedback_parts(&json!({"indicator": {"value": 5, "range": {"min": 0, "max": 10}}}));
        assert_eq!(b, Some(0.5));
    }

    #[test]
    fn files_stay_inside_the_plugin_folder() {
        let d = Path::new("/p/x.sdPlugin");
        assert!(inside(d, "../secret").is_none());
        // a drive or an absolute path never replaces the plugin folder
        assert!(inside(d, "C:/Users/me/.ssh/id_rsa").is_none());
        assert!(inside(d, "x/C:/Windows/win.ini").is_none());
        assert!(inside(d, "pi/index.html").is_some_and(|p| p.starts_with(d)));
        assert!(plain_id("com.example.plugin") && !plain_id("..") && !plain_id("a\\b") && !plain_id(""));
        assert!(inside(d, "pi/../../x").is_none());
        assert_eq!(inside(d, "/pi/a.html"), Some(d.join("pi/a.html")));
    }

    #[test]
    fn percent_decoding() {
        assert_eq!(percent_decode("%3Csvg%20a%3D%221%22%3E"), "<svg a=\"1\">");
        assert_eq!(percent_decode("100%"), "100%");
    }
}
