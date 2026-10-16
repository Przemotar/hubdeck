//! OBS Studio over obs-websocket 5 (built into OBS 28 and later): scenes, recording (and pause),
//! streaming, replay buffer, virtual camera, studio mode and transitions, scene collections, OBS
//! profiles, filters, screenshots, mute, mixer volume and source visibility. One worker thread owns the connection;
//! it connects while the active profile uses OBS, reconnects every few seconds, and reports state
//! changes (recording, current scene, mutes, visible sources) so keys show the truth.

use std::collections::HashMap;
use std::net::TcpStream;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use base64::Engine as _;
use crossbeam_channel::{Receiver, Sender, unbounded};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

use super::Msg;

pub const DEFAULT_URL: &str = "ws://127.0.0.1:4455";
const RETRY: Duration = Duration::from_secs(5);
/// Events: Config (2) | Scenes (4) | Inputs (8) | Filters (32) | Outputs (64) | SceneItems (128) |
/// Ui (1024, studio mode).
const EVENTS: u32 = 2 | 4 | 8 | 32 | 64 | 128 | 1024;

#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    Scene(String),
    ToggleRecord,
    ToggleStream,
    SaveReplay,
    ToggleMute(String),
    /// Push-to-talk / push-to-mute.
    SetMute {
        input: String,
        muted: bool,
    },
    /// 0..=1 on the OBS fader's own curve (faders).
    SetVolume {
        input: String,
        level: f32,
    },
    /// Decibels up or down (knobs, the dial).
    AdjustVolume {
        input: String,
        db: f32,
    },
    /// Show or hide a source; `scene` empty = the current scene.
    ToggleSource {
        scene: String,
        source: String,
    },
    ToggleRecordPause,
    ToggleVirtualCam,
    ToggleReplayBuffer,
    ToggleStudioMode,
    /// Studio mode: preview to program, with a named transition if not empty.
    Transition(String),
    SceneCollection(String),
    Profile(String),
    ToggleFilter {
        source: String,
        filter: String,
    },
    /// PNG of a source (empty = the live scene) into a folder (empty = the recording folder).
    Screenshot {
        source: String,
        folder: String,
    },
}

/// What a status answer or an event changes on the keys.
#[derive(Debug, Clone, PartialEq)]
pub enum Update {
    State(String, bool),
    Value(String, String),
}

/// The key states an event changes (scene items and volumes are handled by the worker).
pub fn event_updates(kind: &str, data: &Value) -> Vec<Update> {
    let active = data["outputActive"] == true;
    let text = |k: &str| data[k].as_str().unwrap_or("").to_string();
    match kind {
        "RecordStateChanged" => {
            let paused = data["outputState"] == "OBS_WEBSOCKET_OUTPUT_PAUSED";
            vec![Update::State("obs:record".into(), active || paused), Update::State("obs:recordPaused".into(), paused)]
        }
        "StreamStateChanged" => vec![Update::State("obs:stream".into(), active)],
        "VirtualcamStateChanged" => vec![Update::State("obs:virtualCam".into(), active)],
        "ReplayBufferStateChanged" => vec![Update::State("obs:replayBuffer".into(), active)],
        "StudioModeStateChanged" => vec![Update::State("obs:studio".into(), data["studioModeEnabled"] == true)],
        "CurrentSceneCollectionChanged" => vec![Update::Value("obs:collection".into(), text("sceneCollectionName"))],
        "CurrentProfileChanged" => vec![Update::Value("obs:profile".into(), text("profileName"))],
        "InputMuteStateChanged" => vec![Update::State(format!("obs:mute:{}", text("inputName")), data["inputMuted"] == true)],
        "SourceFilterEnableStateChanged" => vec![Update::State(format!("obs:filter:{}/{}", text("sourceName"), text("filterName")), data["filterEnabled"] == true)],
        _ => Vec::new(),
    }
}

/// The key states a status answer reports (asked right after connecting).
pub fn status_updates(kind: &str, data: &Value) -> Vec<Update> {
    let active = data["outputActive"] == true;
    match kind {
        "GetRecordStatus" => {
            let paused = data["outputPaused"] == true;
            vec![Update::State("obs:record".into(), active), Update::State("obs:recordPaused".into(), paused)]
        }
        "GetStreamStatus" => vec![Update::State("obs:stream".into(), active)],
        "GetVirtualCamStatus" => vec![Update::State("obs:virtualCam".into(), active)],
        "GetReplayBufferStatus" => vec![Update::State("obs:replayBuffer".into(), active)],
        "GetStudioModeEnabled" => vec![Update::State("obs:studio".into(), data["studioModeEnabled"] == true)],
        "GetSceneCollectionList" => vec![Update::Value("obs:collection".into(), data["currentSceneCollectionName"].as_str().unwrap_or("").to_string())],
        "GetProfileList" => vec![Update::Value("obs:profile".into(), data["currentProfileName"].as_str().unwrap_or("").to_string())],
        _ => Vec::new(),
    }
}

/// Where a screenshot goes: `<folder>/<source> 2026-10-11 21-05-09.png` (no characters Windows forbids).
pub fn screenshot_path(folder: &str, source: &str, now: chrono::NaiveDateTime) -> String {
    let safe: String = source.chars().map(|c| if "<>:\"/\\|?*".contains(c) || c.is_control() { '_' } else { c }).collect();
    let folder = folder.trim_end_matches(['/', '\\']);
    format!("{folder}/{} {}.png", safe.trim(), now.format("%Y-%m-%d %H-%M-%S"))
}

enum Cmd {
    Request(Request),
    /// The active profile uses OBS (connect and stay connected) or not.
    Want(bool),
    Settings {
        url: String,
        password: Option<String>,
    },
}

static WORKER: OnceLock<Sender<Cmd>> = OnceLock::new();

/// What OBS named (read when connecting), by kind: `scene`, `input`, `transition`, `collection`,
/// `profile`, `filter:<source>`; for the window's lists and the import check.
static KNOWN: std::sync::Mutex<std::collections::BTreeMap<String, Vec<String>>> = std::sync::Mutex::new(std::collections::BTreeMap::new());
static CONNECTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// (scenes, audio inputs) while connected to OBS; None otherwise.
pub fn known() -> Option<(Vec<String>, Vec<String>)> {
    let all = names()?;
    let get = |k: &str| all.get(k).cloned().unwrap_or_default();
    Some((get("scene"), get("input")))
}

/// Every list OBS gave, while connected (the window picks from them; owner, 2026-10-15: lists, not
/// names typed from memory).
pub fn names() -> Option<std::collections::BTreeMap<String, Vec<String>>> {
    CONNECTED.load(std::sync::atomic::Ordering::Relaxed).then(|| KNOWN.lock().map(|k| k.clone()).unwrap_or_default())
}

/// The names in an OBS reply, as (kind, names); `source` is the source a filter list was asked for.
fn names_in(kind: &str, data: &Value, source: Option<&str>) -> Option<(String, Vec<String>)> {
    let list = |key: &str, field: Option<&str>| -> Vec<String> {
        data[key]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|x| match field {
                Some(f) => x[f].as_str().map(str::to_string),
                None => x.as_str().map(str::to_string),
            })
            .collect()
    };
    Some(match kind {
        "GetSceneList" => ("scene".into(), list("scenes", Some("sceneName"))),
        "GetInputList" => ("input".into(), list("inputs", Some("inputName"))),
        "GetSceneTransitionList" => ("transition".into(), list("transitions", Some("transitionName"))),
        "GetSceneCollectionList" => ("collection".into(), list("sceneCollections", None)),
        "GetProfileList" => ("profile".into(), list("profiles", None)),
        "GetSourceFilterList" => (format!("filter:{}", source?), list("filters", Some("filterName"))),
        _ => return None,
    })
}

fn remember(kind: &str, data: &Value, source: Option<&str>) {
    if let Some((k, v)) = names_in(kind, data, source)
        && let Ok(mut known) = KNOWN.lock()
    {
        known.insert(k, v);
    }
}

/// Start the worker; `url` and `password` come from the settings and the credential store.
pub fn start(engine: Sender<Msg>, url: String, password: Option<String>) {
    let (tx, rx) = unbounded();
    if WORKER.set(tx).is_ok() {
        std::thread::Builder::new().name("obs".into()).spawn(move || Worker::new(engine, url, password).run(rx)).expect("spawn obs thread");
    }
}

fn send(cmd: Cmd) {
    if let Some(tx) = WORKER.get() {
        let _ = tx.send(cmd);
    }
}

pub fn request(r: Request) {
    send(Cmd::Request(r));
}

pub fn want(yes: bool) {
    send(Cmd::Want(yes));
}

pub fn configure(url: String, password: Option<String>) {
    send(Cmd::Settings { url, password });
}

/// The `authentication` string of obs-websocket 5:
/// base64(sha256(base64(sha256(password + salt)) + challenge)).
pub fn auth_string(password: &str, salt: &str, challenge: &str) -> String {
    let b64 = base64::engine::general_purpose::STANDARD;
    let secret = b64.encode(Sha256::digest(format!("{password}{salt}").as_bytes()));
    b64.encode(Sha256::digest(format!("{secret}{challenge}").as_bytes()))
}

/// OBS fader position (0..=1) to a volume multiplier: OBS's own faders are close to cubic.
pub fn level_to_mul(level: f32) -> f32 {
    level.clamp(0.0, 1.0).powi(3)
}

/// What to do when the answer to a request arrives.
enum Then {
    Nothing,
    /// GetSceneItemId answered: read whether it is visible.
    ItemFound {
        scene: String,
        source: String,
    },
    /// GetSceneItemEnabled answered: flip it.
    ItemState {
        scene: String,
        source: String,
        id: i64,
    },
    /// GetCurrentProgramScene answered for a source toggle in the current scene.
    SceneForSource {
        source: String,
    },
    /// GetInputVolume answered: apply the pending change.
    Volume {
        input: String,
        db: f32,
    },
    /// GetInputList answered: ask each input whether it is muted.
    Inputs,
    /// GetSourceFilterList answered for this source or scene (a source without filters is fine).
    FiltersOf(String),
    MuteOf(String),
    /// A status asked at connect time: a failure (e.g. no replay buffer configured) is not an error.
    Quiet,
    /// GetStudioModeEnabled answered for a toggle: flip it.
    FlipStudio,
    /// GetSourceFilter answered: flip it.
    FlipFilter {
        source: String,
        filter: String,
    },
    /// GetRecordDirectory answered: take the screenshot there.
    ShotFolder {
        source: String,
    },
    /// GetCurrentProgramScene answered: screenshot of the live scene.
    ShotScene {
        folder: String,
    },
    /// SetCurrentSceneTransition answered: now transition.
    TransitionSet,
}

type Socket = WebSocket<MaybeTlsStream<TcpStream>>;

struct Worker {
    engine: Sender<Msg>,
    url: String,
    password: Option<String>,
    wanted: bool,
    socket: Option<Socket>,
    last_try: Option<Instant>,
    next_id: u64,
    pending: HashMap<String, Then>,
    /// Requests that arrived while not connected; sent once connected.
    queued: Vec<Request>,
    /// Known input volumes in dB.
    volume_db: HashMap<String, f32>,
    /// scene item id -> (scene, source), to follow visibility events.
    items: HashMap<(String, i64), String>,
    live_scene: Option<String>,
}

impl Worker {
    fn new(engine: Sender<Msg>, url: String, password: Option<String>) -> Self {
        Self {
            engine,
            url,
            password,
            wanted: false,
            socket: None,
            last_try: None,
            next_id: 0,
            pending: HashMap::new(),
            queued: Vec::new(),
            volume_db: HashMap::new(),
            items: HashMap::new(),
            live_scene: None,
        }
    }

    fn run(mut self, rx: Receiver<Cmd>) {
        loop {
            CONNECTED.store(self.socket.is_some(), std::sync::atomic::Ordering::Relaxed);
            // while nothing uses OBS, sleep until a command comes; connected, wake often to read
            // events; wanted but not reachable, try again every few seconds
            if self.socket.is_none() && !self.wanted && self.queued.is_empty() {
                match rx.recv() {
                    Ok(cmd) => self.command(cmd),
                    Err(_) => return,
                }
            }
            let timeout = if self.socket.is_some() { Duration::from_millis(30) } else { Duration::from_millis(500) };
            match rx.recv_timeout(timeout) {
                Ok(cmd) => self.command(cmd),
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => return,
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
            }
            for cmd in rx.try_iter().collect::<Vec<_>>() {
                self.command(cmd);
            }
            if self.socket.is_none() && (self.wanted || !self.queued.is_empty()) && self.last_try.is_none_or(|t| t.elapsed() >= RETRY) {
                self.connect();
            }
            if self.socket.is_some() {
                self.read();
            }
        }
    }

    fn command(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Request(r) => {
                if self.socket.is_some() {
                    self.perform(r);
                } else {
                    self.queued.push(r);
                    self.last_try = None; // try now, not in five seconds
                }
            }
            Cmd::Want(yes) => self.wanted = yes,
            Cmd::Settings { url, password } => {
                self.url = url;
                self.password = password;
                self.socket = None;
                self.last_try = None;
            }
        }
    }

    fn error(&self, message: String) {
        let _ = self.engine.send(Msg::IntegrationError { source: "OBS".into(), message });
    }

    fn state(&self, key: String, on: bool) {
        let _ = self.engine.send(Msg::IntegrationState { key, on });
    }

    fn connect(&mut self) {
        self.last_try = Some(Instant::now());
        match self.handshake() {
            Ok(socket) => {
                log::info!("OBS connected at {}", self.url);
                self.socket = Some(socket);
                // the state keys show right after connecting
                self.call("GetSceneList", json!({}), Then::Quiet);
                for kind in
                    ["GetRecordStatus", "GetStreamStatus", "GetVirtualCamStatus", "GetReplayBufferStatus", "GetStudioModeEnabled", "GetSceneCollectionList", "GetProfileList", "GetSceneTransitionList"]
                {
                    self.call(kind, json!({}), Then::Quiet);
                }
                self.call("GetCurrentProgramScene", json!({}), Then::Nothing);
                self.call("GetInputList", json!({}), Then::Inputs);
                for r in std::mem::take(&mut self.queued) {
                    self.perform(r);
                }
            }
            Err(e) => {
                if !self.queued.is_empty() {
                    self.queued.clear();
                    self.error(format!("cannot reach OBS at {} ({e}). Is OBS running with its WebSocket server on (Tools > WebSocket Server Settings)?", self.url));
                }
                log::debug!("OBS: {e}");
            }
        }
    }

    fn handshake(&self) -> Result<Socket, String> {
        let (mut socket, _) = tungstenite::connect(self.url.as_str()).map_err(|e| e.to_string())?;
        if let MaybeTlsStream::Plain(s) = socket.get_mut() {
            let _ = s.set_read_timeout(Some(Duration::from_secs(3)));
        }
        let hello = read_json(&mut socket)?;
        if hello["op"] != 0 {
            return Err("unexpected first message".into());
        }
        let mut identify = json!({"rpcVersion": 1, "eventSubscriptions": EVENTS});
        if let Some(auth) = hello["d"].get("authentication") {
            let password = self.password.as_deref().ok_or("OBS asks for a password; set it in the profile panel under OBS")?;
            identify["authentication"] = json!(auth_string(password, auth["salt"].as_str().unwrap_or(""), auth["challenge"].as_str().unwrap_or("")));
        }
        socket.send(Message::text(json!({"op": 1, "d": identify}).to_string())).map_err(|e| e.to_string())?;
        let identified = read_json(&mut socket).map_err(|e| if e.contains("closed") { "OBS refused the password".to_string() } else { e })?;
        if identified["op"] != 2 {
            return Err("OBS did not accept the connection".into());
        }
        if let MaybeTlsStream::Plain(s) = socket.get_mut() {
            let _ = s.set_read_timeout(Some(Duration::from_millis(20)));
        }
        Ok(socket)
    }

    fn call(&mut self, kind: &str, data: Value, then: Then) {
        self.next_id += 1;
        let id = format!("hd{}", self.next_id);
        let msg = json!({"op": 6, "d": {"requestType": kind, "requestId": id, "requestData": data}});
        let Some(socket) = self.socket.as_mut() else { return };
        if let Err(e) = socket.send(Message::text(msg.to_string())) {
            log::info!("OBS disconnected ({e})");
            self.socket = None;
            return;
        }
        self.pending.insert(id, then);
    }

    fn perform(&mut self, r: Request) {
        match r {
            Request::Scene(scene) => self.call("SetCurrentProgramScene", json!({"sceneName": scene}), Then::Nothing),
            Request::ToggleRecord => self.call("ToggleRecord", json!({}), Then::Nothing),
            Request::ToggleStream => self.call("ToggleStream", json!({}), Then::Nothing),
            Request::SaveReplay => self.call("SaveReplayBuffer", json!({}), Then::Nothing),
            Request::ToggleMute(input) => self.call("ToggleInputMute", json!({"inputName": input}), Then::Nothing),
            Request::SetMute { input, muted } => self.call("SetInputMute", json!({"inputName": input, "inputMuted": muted}), Then::Nothing),
            Request::SetVolume { input, level } => self.call("SetInputVolume", json!({"inputName": input, "inputVolumeMul": level_to_mul(level)}), Then::Nothing),
            Request::AdjustVolume { input, db } => match self.volume_db.get(&input) {
                Some(now) => {
                    let next = (now + db).clamp(-100.0, 26.0);
                    self.volume_db.insert(input.clone(), next);
                    self.call("SetInputVolume", json!({"inputName": input, "inputVolumeDb": next}), Then::Nothing);
                }
                None => self.call("GetInputVolume", json!({"inputName": input.clone()}), Then::Volume { input, db }),
            },
            Request::ToggleSource { scene, source } if scene.is_empty() => self.call("GetCurrentProgramScene", json!({}), Then::SceneForSource { source }),
            Request::ToggleSource { scene, source } => self.call("GetSceneItemId", json!({"sceneName": scene, "sourceName": source}), Then::ItemFound { scene, source }),
            Request::ToggleRecordPause => self.call("ToggleRecordPause", json!({}), Then::Nothing),
            Request::ToggleVirtualCam => self.call("ToggleVirtualCam", json!({}), Then::Nothing),
            Request::ToggleReplayBuffer => self.call("ToggleReplayBuffer", json!({}), Then::Nothing),
            Request::ToggleStudioMode => self.call("GetStudioModeEnabled", json!({}), Then::FlipStudio),
            Request::Transition(name) if name.trim().is_empty() => self.call("TriggerStudioModeTransition", json!({}), Then::Nothing),
            Request::Transition(name) => self.call("SetCurrentSceneTransition", json!({"transitionName": name.trim()}), Then::TransitionSet),
            Request::SceneCollection(name) => self.call("SetCurrentSceneCollection", json!({"sceneCollectionName": name}), Then::Nothing),
            Request::Profile(name) => self.call("SetCurrentProfile", json!({"profileName": name}), Then::Nothing),
            Request::ToggleFilter { source, filter } => self.call("GetSourceFilter", json!({"sourceName": source.clone(), "filterName": filter.clone()}), Then::FlipFilter { source, filter }),
            Request::Screenshot { source, folder } if folder.trim().is_empty() => self.call("GetRecordDirectory", json!({}), Then::ShotFolder { source }),
            Request::Screenshot { source, folder } => self.screenshot(source, folder),
        }
    }

    /// Screenshot once the folder is known; an empty source means the live scene.
    fn screenshot(&mut self, source: String, folder: String) {
        if source.trim().is_empty() {
            match self.live_scene.clone() {
                Some(scene) if !scene.is_empty() => self.screenshot(scene, folder),
                _ => self.call("GetCurrentProgramScene", json!({}), Then::ShotScene { folder }),
            }
            return;
        }
        let path = screenshot_path(&folder, &source, chrono::Local::now().naive_local());
        self.call("SaveSourceScreenshot", json!({"sourceName": source, "imageFormat": "png", "imageFilePath": path}), Then::Nothing);
    }

    fn apply(&self, updates: Vec<Update>) {
        for u in updates {
            match u {
                Update::State(key, on) => self.state(key, on),
                Update::Value(key, value) => {
                    let _ = self.engine.send(Msg::IntegrationValue { key, value });
                }
            }
        }
    }

    fn read(&mut self) {
        loop {
            let Some(socket) = self.socket.as_mut() else { return };
            match socket.read() {
                Ok(Message::Text(text)) => {
                    if let Ok(v) = serde_json::from_str::<Value>(&text) {
                        self.message(v);
                    }
                }
                Ok(Message::Close(_)) => {
                    log::info!("OBS closed the connection");
                    self.socket = None;
                    return;
                }
                Ok(_) => {}
                Err(tungstenite::Error::Io(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => return,
                Err(e) => {
                    log::info!("OBS disconnected ({e})");
                    self.socket = None;
                    return;
                }
            }
        }
    }

    fn message(&mut self, v: Value) {
        let d = &v["d"];
        match v["op"].as_u64() {
            Some(5) => self.event(d["eventType"].as_str().unwrap_or(""), &d["eventData"]),
            Some(7) => {
                let then = self.pending.remove(d["requestId"].as_str().unwrap_or("")).unwrap_or(Then::Nothing);
                let kind = d["requestType"].as_str().unwrap_or("");
                if d["requestStatus"]["result"] != true {
                    if matches!(then, Then::Quiet | Then::FiltersOf(_)) {
                        return;
                    }
                    let comment = d["requestStatus"]["comment"].as_str().unwrap_or("request failed");
                    self.error(format!("{kind}: {comment}"));
                    return;
                }
                self.response(kind, &d["responseData"], then);
            }
            _ => {}
        }
    }

    fn response(&mut self, kind: &str, data: &Value, then: Then) {
        let filters_of = if let Then::FiltersOf(s) = &then { Some(s.clone()) } else { None };
        remember(kind, data, filters_of.as_deref());
        match (kind, then) {
            ("GetStudioModeEnabled", Then::FlipStudio) => {
                let on = data["studioModeEnabled"] == true;
                self.call("SetStudioModeEnabled", json!({"studioModeEnabled": !on}), Then::Nothing);
            }
            ("GetSourceFilter", Then::FlipFilter { source, filter }) => {
                let on = data["filterEnabled"] == true;
                self.state(format!("obs:filter:{source}/{filter}"), !on);
                self.call("SetSourceFilterEnabled", json!({"sourceName": source, "filterName": filter, "filterEnabled": !on}), Then::Nothing);
            }
            ("GetRecordDirectory", Then::ShotFolder { source }) => {
                let folder = data["recordDirectory"].as_str().unwrap_or("").to_string();
                self.screenshot(source, folder);
            }
            ("GetCurrentProgramScene", Then::ShotScene { folder }) => {
                let scene = data["currentProgramSceneName"].as_str().unwrap_or("").to_string();
                if !scene.is_empty() {
                    self.screenshot(scene, folder);
                }
            }
            ("SetCurrentSceneTransition", Then::TransitionSet) => self.call("TriggerStudioModeTransition", json!({}), Then::Nothing),
            (k, _) if !status_updates(k, data).is_empty() => self.apply(status_updates(k, data)),
            ("GetCurrentProgramScene", Then::SceneForSource { source }) => {
                let scene = data["currentProgramSceneName"].as_str().unwrap_or("").to_string();
                self.call("GetSceneItemId", json!({"sceneName": scene.clone(), "sourceName": source.clone()}), Then::ItemFound { scene, source });
            }
            ("GetCurrentProgramScene", _) => self.scene(data["currentProgramSceneName"].as_str().unwrap_or("")),
            ("GetSceneList", _) => {
                // the filters of every scene too (a filter sits on a scene or a source)
                for name in names_in(kind, data, None).map(|(_, v)| v).unwrap_or_default() {
                    self.call("GetSourceFilterList", json!({"sourceName": name.clone()}), Then::FiltersOf(name));
                }
            }
            ("GetInputList", Then::Inputs) => {
                for name in names_in(kind, data, None).map(|(_, v)| v).unwrap_or_default() {
                    self.call("GetInputMute", json!({"inputName": name.clone()}), Then::MuteOf(name.clone()));
                    self.call("GetSourceFilterList", json!({"sourceName": name.clone()}), Then::FiltersOf(name));
                }
            }
            ("GetInputMute", Then::MuteOf(name)) => self.state(format!("obs:mute:{name}"), data["inputMuted"] == true),
            ("GetInputVolume", Then::Volume { input, db }) => {
                let now = data["inputVolumeDb"].as_f64().unwrap_or(0.0) as f32;
                self.volume_db.insert(input.clone(), now);
                self.perform(Request::AdjustVolume { input, db });
            }
            ("GetSceneItemId", Then::ItemFound { scene, source }) => {
                let Some(id) = data["sceneItemId"].as_i64() else { return };
                self.items.insert((scene.clone(), id), source.clone());
                self.call("GetSceneItemEnabled", json!({"sceneName": scene.clone(), "sceneItemId": id}), Then::ItemState { scene, source, id });
            }
            ("GetSceneItemEnabled", Then::ItemState { scene, source, id }) => {
                let enabled = data["sceneItemEnabled"] == true;
                self.source_state(&scene, &source, !enabled);
                self.call("SetSceneItemEnabled", json!({"sceneName": scene, "sceneItemId": id, "sceneItemEnabled": !enabled}), Then::Nothing);
            }
            _ => {}
        }
    }

    /// A key for a source names its scene, or none for "the live scene": report both.
    fn source_state(&self, scene: &str, source: &str, on: bool) {
        self.state(format!("obs:source:{scene}/{source}"), on);
        if self.live_scene.as_deref() == Some(scene) {
            self.state(format!("obs:source:/{source}"), on);
        }
    }

    fn scene(&mut self, name: &str) {
        self.live_scene = Some(name.to_string());
        let _ = self.engine.send(Msg::IntegrationState { key: "obs:scene".into(), on: true });
        let _ = self.engine.send(Msg::IntegrationValue { key: "obs:scene".into(), value: name.to_string() });
    }

    fn event(&mut self, kind: &str, data: &Value) {
        let updates = event_updates(kind, data);
        if !updates.is_empty() {
            self.apply(updates);
            return;
        }
        match kind {
            "CurrentProgramSceneChanged" => self.scene(data["sceneName"].as_str().unwrap_or("")),
            "InputVolumeChanged" => {
                if let (Some(name), Some(db)) = (data["inputName"].as_str(), data["inputVolumeDb"].as_f64()) {
                    self.volume_db.insert(name.to_string(), db as f32);
                }
            }
            "SceneItemEnableStateChanged" => {
                let scene = data["sceneName"].as_str().unwrap_or("").to_string();
                if let (Some(id), Some(on)) = (data["sceneItemId"].as_i64(), data["sceneItemEnabled"].as_bool())
                    && let Some(source) = self.items.get(&(scene.clone(), id)).cloned()
                {
                    self.source_state(&scene, &source, on);
                }
            }
            _ => {}
        }
    }
}

fn read_json(socket: &mut Socket) -> Result<Value, String> {
    loop {
        match socket.read().map_err(|e| e.to_string())? {
            Message::Text(t) => return serde_json::from_str(&t).map_err(|e| e.to_string()),
            Message::Close(_) => return Err("connection closed".into()),
            _ => continue,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lists_come_out_of_obs_replies() {
        // shapes from the obs-websocket 5 protocol (GetSceneTransitionList, GetSceneCollectionList...)
        assert_eq!(
            names_in(
                "GetSceneTransitionList",
                &json!({"currentSceneTransitionName": "Fade", "transitions": [{"transitionName": "Cut", "transitionKind": "cut_transition"}, {"transitionName": "Fade"}]}),
                None
            ),
            Some(("transition".into(), vec!["Cut".into(), "Fade".into()]))
        );
        assert_eq!(
            names_in("GetSceneCollectionList", &json!({"currentSceneCollectionName": "Main", "sceneCollections": ["Main", "Podcast"]}), None),
            Some(("collection".into(), vec!["Main".into(), "Podcast".into()]))
        );
        assert_eq!(names_in("GetProfileList", &json!({"currentProfileName": "Untitled", "profiles": ["Untitled", "1080p"]}), None), Some(("profile".into(), vec!["Untitled".into(), "1080p".into()])));
        assert_eq!(
            names_in("GetSourceFilterList", &json!({"filters": [{"filterName": "Color Correction", "filterEnabled": true}]}), Some("Camera")),
            Some(("filter:Camera".into(), vec!["Color Correction".into()]))
        );
        assert_eq!(names_in("GetSourceFilterList", &json!({"filters": []}), None), None);
        assert_eq!(names_in("GetSceneList", &json!({"scenes": [{"sceneName": "Live", "sceneIndex": 0}]}), None), Some(("scene".into(), vec!["Live".into()])));
        assert_eq!(names_in("GetRecordStatus", &json!({}), None), None);
    }

    #[test]
    fn authentication_matches_the_protocol_example() {
        // the worked example in the obs-websocket 5 protocol documentation
        assert_eq!(auth_string("supersecretpassword", "lM1GncleQOaCu9lT1yeUZhFYnqhsLLP1G5lAGo3ixaI=", "+IxH4CnCiqpX1rM9scsNynZzbOe4KhDeYcTNS3PDaeY="), "1Ct943GAT+6YQUUX47Ia/ncufilbe6+oD6lY+5kaCu4=");
    }

    #[test]
    fn events_and_statuses_update_the_keys() {
        let paused = event_updates("RecordStateChanged", &json!({"outputActive": false, "outputState": "OBS_WEBSOCKET_OUTPUT_PAUSED"}));
        assert_eq!(paused, [Update::State("obs:record".into(), true), Update::State("obs:recordPaused".into(), true)], "paused is still recording");
        assert_eq!(event_updates("VirtualcamStateChanged", &json!({"outputActive": true})), [Update::State("obs:virtualCam".into(), true)]);
        assert_eq!(event_updates("StudioModeStateChanged", &json!({"studioModeEnabled": true})), [Update::State("obs:studio".into(), true)]);
        assert_eq!(event_updates("CurrentProfileChanged", &json!({"profileName": "Twitch"})), [Update::Value("obs:profile".into(), "Twitch".into())]);
        assert_eq!(event_updates("SourceFilterEnableStateChanged", &json!({"sourceName": "Cam", "filterName": "Blur", "filterEnabled": false})), [Update::State("obs:filter:Cam/Blur".into(), false)]);
        assert!(event_updates("CurrentProgramSceneChanged", &json!({})).is_empty(), "scenes are handled by the worker");
        assert_eq!(status_updates("GetReplayBufferStatus", &json!({"outputActive": true})), [Update::State("obs:replayBuffer".into(), true)]);
        assert_eq!(status_updates("GetSceneCollectionList", &json!({"currentSceneCollectionName": "Main"})), [Update::Value("obs:collection".into(), "Main".into())]);
        assert!(status_updates("SetInputVolume", &json!({})).is_empty());
    }

    #[test]
    fn screenshot_names_are_safe() {
        let t = chrono::NaiveDate::from_ymd_opt(2026, 10, 11).unwrap().and_hms_opt(21, 5, 9).unwrap();
        assert_eq!(screenshot_path("C:\\Videos\\", "Game: 1/2", t), "C:\\Videos/Game_ 1_2 2026-10-11 21-05-09.png");
    }

    #[test]
    fn fader_curve() {
        assert_eq!(level_to_mul(0.0), 0.0);
        assert_eq!(level_to_mul(1.0), 1.0);
        assert!((level_to_mul(0.5) - 0.125).abs() < 1e-6);
    }
}
