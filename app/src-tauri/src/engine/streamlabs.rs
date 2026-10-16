//! Streamlabs Desktop over its remote-control API: JSON-RPC 2.0 over SockJS, here its raw
//! WebSocket endpoint (`ws://127.0.0.1:59650/api/websocket`), authenticated with the API token
//! from Streamlabs' Settings > Remote Control (kept in the credential store). Scenes, recording,
//! streaming, replay buffer, studio mode, source visibility, mute and mixer volume, with key states
//! from its event subscriptions. Written from the published API reference; not yet tried against
//! Streamlabs Desktop (not installed on the development PC).
//!
//! One worker thread owns the connection, like `obs.rs`: it connects while the active profile uses
//! Streamlabs, reconnects every few seconds, and resolves names (scenes, sources) to Streamlabs ids.

use std::collections::HashMap;
use std::net::TcpStream;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, unbounded};
use serde_json::{Value, json};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

use super::Msg;

pub const DEFAULT_URL: &str = "ws://127.0.0.1:59650/api/websocket";
const RETRY: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    Scene(String),
    ToggleRecord,
    ToggleStream,
    SaveReplay,
    ToggleReplayBuffer,
    ToggleStudioMode,
    Transition,
    /// Show or hide a source; `scene` empty = the live scene.
    ToggleSource {
        scene: String,
        source: String,
    },
    ToggleMute(String),
    /// Fader position 0..=1 (Streamlabs' own "deflection").
    SetVolume {
        source: String,
        level: f32,
    },
    /// Up or down by a fraction of the fader (knobs, the dial).
    AdjustVolume {
        source: String,
        by: f32,
    },
}

enum Cmd {
    Request(Request),
    Want(bool),
    Settings { url: String, token: Option<String> },
}

static WORKER: OnceLock<Sender<Cmd>> = OnceLock::new();

pub fn start(engine: Sender<Msg>, url: String, token: Option<String>) {
    let (tx, rx) = unbounded();
    if WORKER.set(tx).is_ok() {
        std::thread::Builder::new().name("streamlabs".into()).spawn(move || Worker::new(engine, url, token).run(rx)).expect("spawn streamlabs thread");
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

pub fn configure(url: String, token: Option<String>) {
    send(Cmd::Settings { url, token });
}

/// A JSON-RPC call: `method` on `resource` (e.g. `ScenesService`, `AudioSource["id"]`).
pub fn rpc(id: u64, method: &str, resource: &str, args: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": method, "params": {"resource": resource, "args": args}})
}

/// The resource name of an object with ids, e.g. `SceneItem["scene","item","source"]`.
pub fn resource(kind: &str, ids: &[&str]) -> String {
    let quoted: Vec<String> = ids.iter().map(|i| serde_json::to_string(i).unwrap_or_default()).collect();
    format!("{kind}[{}]", quoted.join(","))
}

/// Status strings of Streamlabs -> "on" for a key: live or about to be, recording, buffer running.
pub fn status_on(status: &str) -> bool {
    matches!(status, "live" | "starting" | "reconnecting" | "recording" | "running" | "saving")
}

/// A scene and the items in it (name -> (item id, source id, visible)).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SceneInfo {
    pub id: String,
    pub name: String,
    pub items: Vec<(String, String, String, bool)>,
}

/// Every source's name, and the ones with sound (they carry `muted`), for the window's lists.
pub fn source_names(result: &Value) -> (Vec<super::names::Name>, Vec<super::names::Name>) {
    let items = || result.as_array().into_iter().flatten();
    let all = items().filter_map(|s| s["name"].as_str()).map(super::names::Name::same).collect();
    let audio = items().filter(|s| s["muted"].is_boolean() || s["audio"] == true).filter_map(|s| s["name"].as_str()).map(super::names::Name::same).collect();
    (all, audio)
}

/// Read a scene model as the API returns it (`id`, `name`, `nodes` with `sceneItemId`,
/// `sourceId`, `name`, `visible`); folders have no source and are skipped.
pub fn scene_from(v: &Value) -> Option<SceneInfo> {
    let id = v["id"].as_str()?.to_string();
    let name = v["name"].as_str().unwrap_or("").to_string();
    let items = v["nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|n| Some((n["name"].as_str()?.to_string(), n["sceneItemId"].as_str()?.to_string(), n["sourceId"].as_str()?.to_string(), n["visible"].as_bool().unwrap_or(true))))
        .collect();
    Some(SceneInfo { id, name, items })
}

/// What to do when an answer arrives.
enum Then {
    Nothing,
    Auth,
    Scenes,
    ActiveScene,
    Sources,
    StreamingModel,
    TransitionsModel,
    FlipMute(String),
    Volume { source_id: String, by: f32 },
}

type Socket = WebSocket<MaybeTlsStream<TcpStream>>;

struct Worker {
    engine: Sender<Msg>,
    url: String,
    token: Option<String>,
    wanted: bool,
    socket: Option<Socket>,
    ready: bool,
    last_try: Option<Instant>,
    next_id: u64,
    pending: HashMap<u64, Then>,
    queued: Vec<Request>,
    scenes: Vec<SceneInfo>,
    active_scene: Option<String>,
    /// Source name -> id.
    sources: HashMap<String, String>,
    /// Fader positions we know, by source id.
    deflection: HashMap<String, f32>,
    studio: bool,
    replay_running: bool,
}

impl Worker {
    fn new(engine: Sender<Msg>, url: String, token: Option<String>) -> Self {
        Self {
            engine,
            url,
            token,
            wanted: false,
            socket: None,
            ready: false,
            last_try: None,
            next_id: 0,
            pending: HashMap::new(),
            queued: Vec::new(),
            scenes: Vec::new(),
            active_scene: None,
            sources: HashMap::new(),
            deflection: HashMap::new(),
            studio: false,
            replay_running: false,
        }
    }

    fn run(mut self, rx: Receiver<Cmd>) {
        loop {
            // while nothing uses Streamlabs, sleep until a command comes
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
                if self.ready {
                    self.perform(r);
                } else {
                    self.queued.push(r);
                    self.last_try = None;
                }
            }
            Cmd::Want(yes) => self.wanted = yes,
            Cmd::Settings { url, token } => {
                self.url = url;
                self.token = token;
                self.drop_connection();
                self.last_try = None;
            }
        }
    }

    fn drop_connection(&mut self) {
        self.socket = None;
        self.ready = false;
        self.pending.clear();
    }

    fn error(&self, message: String) {
        let _ = self.engine.send(Msg::IntegrationError { source: "Streamlabs".into(), message });
    }

    fn state(&self, key: &str, on: bool) {
        let _ = self.engine.send(Msg::IntegrationState { key: key.to_string(), on });
    }

    fn connect(&mut self) {
        self.last_try = Some(Instant::now());
        let Some(token) = self.token.clone() else {
            if !self.queued.is_empty() {
                self.queued.clear();
                self.error("set the Streamlabs API token first (Streamlabs: Settings > Remote Control)".into());
            }
            return;
        };
        match tungstenite::connect(self.url.as_str()) {
            Ok((mut socket, _)) => {
                if let MaybeTlsStream::Plain(s) = socket.get_mut() {
                    let _ = s.set_read_timeout(Some(Duration::from_millis(20)));
                }
                log::info!("Streamlabs connected at {}", self.url);
                self.socket = Some(socket);
                self.call("auth", "TcpServerService", json!([token]), Then::Auth);
            }
            Err(e) => {
                if !self.queued.is_empty() {
                    self.queued.clear();
                    self.error(format!("cannot reach Streamlabs Desktop at {} ({e}). Is it running with remote control on (Settings > Remote Control)?", self.url));
                }
                log::debug!("Streamlabs: {e}");
            }
        }
    }

    fn call(&mut self, method: &str, resource: &str, args: Value, then: Then) {
        self.next_id += 1;
        let id = self.next_id;
        let msg = rpc(id, method, resource, args);
        let Some(socket) = self.socket.as_mut() else { return };
        if let Err(e) = socket.send(Message::text(msg.to_string())) {
            log::info!("Streamlabs disconnected ({e})");
            self.drop_connection();
            return;
        }
        self.pending.insert(id, then);
    }

    /// Authenticated: subscribe to the events the keys show, then read the current state.
    fn on_auth(&mut self) {
        for (event, service) in [
            ("streamingStatusChange", "StreamingService"),
            ("recordingStatusChange", "StreamingService"),
            ("replayBufferStatusChange", "StreamingService"),
            ("sceneSwitched", "ScenesService"),
            ("itemUpdated", "ScenesService"),
            ("studioModeChanged", "TransitionsService"),
        ] {
            self.call(event, service, json!([]), Then::Nothing);
        }
        self.call("getModel", "StreamingService", json!([]), Then::StreamingModel);
        self.call("getModel", "TransitionsService", json!([]), Then::TransitionsModel);
        self.call("activeSceneId", "ScenesService", json!([]), Then::ActiveScene);
        self.call("getScenes", "ScenesService", json!([]), Then::Scenes);
        // the last answer makes the worker ready for the queued requests
        self.call("getSources", "SourcesService", json!([]), Then::Sources);
    }

    fn scene_named(&self, name: &str) -> Option<&SceneInfo> {
        if name.trim().is_empty() {
            let active = self.active_scene.as_deref()?;
            return self.scenes.iter().find(|s| s.id == active);
        }
        self.scenes.iter().find(|s| s.name == name.trim()).or_else(|| self.scenes.iter().find(|s| s.name.eq_ignore_ascii_case(name.trim())))
    }

    fn source_id(&self, name: &str) -> Option<String> {
        self.sources.get(name.trim()).cloned().or_else(|| self.sources.iter().find(|(n, _)| n.eq_ignore_ascii_case(name.trim())).map(|(_, id)| id.clone()))
    }

    fn perform(&mut self, r: Request) {
        match r {
            Request::Scene(name) => match self.scene_named(&name).map(|s| s.id.clone()) {
                Some(id) if !name.trim().is_empty() => self.call("makeSceneActive", "ScenesService", json!([id]), Then::Nothing),
                _ => self.error(format!("no scene named {name:?} in Streamlabs")),
            },
            Request::ToggleRecord => self.call("toggleRecording", "StreamingService", json!([]), Then::Nothing),
            Request::ToggleStream => self.call("toggleStreaming", "StreamingService", json!([]), Then::Nothing),
            Request::SaveReplay => self.call("saveReplay", "StreamingService", json!([]), Then::Nothing),
            Request::ToggleReplayBuffer => {
                let method = if self.replay_running { "stopReplayBuffer" } else { "startReplayBuffer" };
                self.call(method, "StreamingService", json!([]), Then::Nothing);
            }
            Request::ToggleStudioMode => {
                let method = if self.studio { "disableStudioMode" } else { "enableStudioMode" };
                self.call(method, "TransitionsService", json!([]), Then::Nothing);
            }
            Request::Transition => self.call("executeStudioModeTransition", "TransitionsService", json!([]), Then::Nothing),
            Request::ToggleSource { scene, source } => {
                let found = self.scene_named(&scene).and_then(|s| s.items.iter().find(|i| i.0.eq_ignore_ascii_case(source.trim())).map(|i| (s.id.clone(), s.name.clone(), i.clone())));
                match found {
                    Some((scene_id, scene_name, (name, item, source_id, visible))) => {
                        let res = resource("SceneItem", &[&scene_id, &item, &source_id]);
                        self.call("setVisibility", &res, json!([!visible]), Then::Nothing);
                        self.set_visible(&scene_id, &item, !visible);
                        self.source_state(&scene_name, &name, !visible);
                    }
                    None => self.error(format!("no source named {source:?} in that Streamlabs scene")),
                }
            }
            Request::ToggleMute(name) => match self.source_id(&name) {
                Some(id) => self.call("getModel", &resource("AudioSource", &[&id]), json!([]), Then::FlipMute(id)),
                None => self.error(format!("no source named {name:?} in Streamlabs")),
            },
            Request::SetVolume { source, level } => match self.source_id(&source) {
                Some(id) => {
                    let level = level.clamp(0.0, 1.0);
                    self.deflection.insert(id.clone(), level);
                    self.call("setDeflection", &resource("AudioSource", &[&id]), json!([level]), Then::Nothing);
                }
                None => self.error(format!("no source named {source:?} in Streamlabs")),
            },
            Request::AdjustVolume { source, by } => match self.source_id(&source) {
                Some(id) => match self.deflection.get(&id).copied() {
                    Some(now) => {
                        let next = (now + by).clamp(0.0, 1.0);
                        self.deflection.insert(id.clone(), next);
                        self.call("setDeflection", &resource("AudioSource", &[&id]), json!([next]), Then::Nothing);
                    }
                    None => self.call("getModel", &resource("AudioSource", &[&id]), json!([]), Then::Volume { source_id: id, by }),
                },
                None => self.error(format!("no source named {source:?} in Streamlabs")),
            },
        }
    }

    fn set_visible(&mut self, scene_id: &str, item: &str, visible: bool) {
        if let Some(s) = self.scenes.iter_mut().find(|s| s.id == scene_id)
            && let Some(i) = s.items.iter_mut().find(|i| i.1 == item)
        {
            i.3 = visible;
        }
    }

    /// A key names its scene, or none for "the live scene": report both.
    fn source_state(&self, scene: &str, source: &str, on: bool) {
        self.state(&format!("sl:source:{scene}/{source}"), on);
        let live = self.active_scene.as_deref().and_then(|id| self.scenes.iter().find(|s| s.id == id)).map(|s| s.name.as_str());
        if live == Some(scene) {
            self.state(&format!("sl:source:/{source}"), on);
        }
    }

    fn scene_switched(&mut self, id: &str) {
        self.active_scene = Some(id.to_string());
        let name = self.scenes.iter().find(|s| s.id == id).map(|s| s.name.clone()).unwrap_or_default();
        let _ = self.engine.send(Msg::IntegrationValue { key: "sl:scene".into(), value: name.clone() });
        if let Some(scene) = self.scenes.iter().find(|s| s.id == id) {
            for (n, _, _, visible) in &scene.items {
                self.state(&format!("sl:source:/{n}"), *visible);
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
                    log::info!("Streamlabs closed the connection");
                    self.drop_connection();
                    return;
                }
                Ok(_) => {}
                Err(tungstenite::Error::Io(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => return,
                Err(e) => {
                    log::info!("Streamlabs disconnected ({e})");
                    self.drop_connection();
                    return;
                }
            }
        }
    }

    fn message(&mut self, v: Value) {
        let result = &v["result"];
        if result["_type"] == "EVENT" {
            self.event(result["resourceId"].as_str().unwrap_or(""), &result["data"]);
            return;
        }
        let Some(id) = v["id"].as_u64() else { return };
        let then = self.pending.remove(&id).unwrap_or(Then::Nothing);
        if let Some(err) = v.get("error") {
            let message = err["message"].as_str().unwrap_or("request failed").to_string();
            if matches!(then, Then::Auth) {
                self.error(format!("Streamlabs refused the API token ({message})"));
                self.drop_connection();
                self.queued.clear();
            } else {
                self.error(message);
            }
            return;
        }
        match then {
            Then::Nothing => {}
            Then::Auth => self.on_auth(),
            Then::StreamingModel => {
                self.state("sl:stream", status_on(result["streamingStatus"].as_str().unwrap_or("")));
                self.state("sl:record", status_on(result["recordingStatus"].as_str().unwrap_or("")));
                self.replay_running = status_on(result["replayBufferStatus"].as_str().unwrap_or(""));
                self.state("sl:replayBuffer", self.replay_running);
            }
            Then::TransitionsModel => {
                self.studio = result["studioMode"] == true;
                self.state("sl:studio", self.studio);
            }
            Then::ActiveScene => self.active_scene = result.as_str().map(str::to_string),
            Then::Scenes => {
                self.scenes = result.as_array().into_iter().flatten().filter_map(scene_from).collect();
                super::names::set("streamlabs.scene", self.scenes.iter().map(|s| super::names::Name::same(&s.name)).collect());
                if let Some(id) = self.active_scene.clone() {
                    self.scene_switched(&id);
                }
            }
            Then::Sources => {
                self.sources = result.as_array().into_iter().flatten().filter_map(|s| Some((s["name"].as_str()?.to_string(), s["sourceId"].as_str().or(s["id"].as_str())?.to_string()))).collect();
                let (all, audio) = source_names(result);
                super::names::set("streamlabs.source", all);
                super::names::set("streamlabs.audio", audio);
                for s in result.as_array().into_iter().flatten() {
                    if let (Some(name), Some(muted)) = (s["name"].as_str(), s["muted"].as_bool()) {
                        self.state(&format!("sl:mute:{name}"), muted);
                    }
                }
                self.ready = true;
                for r in std::mem::take(&mut self.queued) {
                    self.perform(r);
                }
            }
            Then::FlipMute(id) => {
                let muted = result["muted"] == true;
                let name = result["name"].as_str().map(str::to_string).or_else(|| self.sources.iter().find(|(_, i)| **i == id).map(|(n, _)| n.clone())).unwrap_or_default();
                self.call("setMuted", &resource("AudioSource", &[&id]), json!([!muted]), Then::Nothing);
                self.state(&format!("sl:mute:{name}"), !muted);
            }
            Then::Volume { source_id, by } => {
                let now = result["fader"]["deflection"].as_f64().unwrap_or(1.0) as f32;
                let next = (now + by).clamp(0.0, 1.0);
                self.deflection.insert(source_id.clone(), next);
                self.call("setDeflection", &resource("AudioSource", &[&source_id]), json!([next]), Then::Nothing);
            }
        }
    }

    fn event(&mut self, resource_id: &str, data: &Value) {
        match resource_id {
            "StreamingService.streamingStatusChange" => self.state("sl:stream", status_on(data.as_str().unwrap_or(""))),
            "StreamingService.recordingStatusChange" => self.state("sl:record", status_on(data.as_str().unwrap_or(""))),
            "StreamingService.replayBufferStatusChange" => {
                self.replay_running = status_on(data.as_str().unwrap_or(""));
                self.state("sl:replayBuffer", self.replay_running);
            }
            "TransitionsService.studioModeChanged" => {
                self.studio = data.as_bool().unwrap_or(false);
                self.state("sl:studio", self.studio);
            }
            "ScenesService.sceneSwitched" => {
                if let Some(scene) = scene_from(data) {
                    let id = scene.id.clone();
                    match self.scenes.iter_mut().find(|s| s.id == id) {
                        Some(s) => *s = scene,
                        None => self.scenes.push(scene),
                    }
                    self.scene_switched(&id);
                }
            }
            "ScenesService.itemUpdated" => {
                if let (Some(scene_id), Some(item), Some(visible)) = (data["sceneId"].as_str(), data["sceneItemId"].as_str(), data["visible"].as_bool()) {
                    self.set_visible(scene_id, item, visible);
                    let names = self.scenes.iter().find(|s| s.id == scene_id).and_then(|s| s.items.iter().find(|i| i.1 == item).map(|i| (s.name.clone(), i.0.clone())));
                    if let Some((scene, source)) = names {
                        self.source_state(&scene, &source, visible);
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_follow_the_json_rpc_shape() {
        let msg = rpc(8, "auth", "TcpServerService", json!(["secret"]));
        assert_eq!(msg, json!({"jsonrpc": "2.0", "id": 8, "method": "auth", "params": {"resource": "TcpServerService", "args": ["secret"]}}));
        assert_eq!(resource("SceneItem", &["s1", "i2", "src\"3"]), r#"SceneItem["s1","i2","src\"3"]"#);
        assert_eq!(resource("AudioSource", &["a"]), r#"AudioSource["a"]"#);
    }

    #[test]
    fn statuses_and_scenes_are_read() {
        assert!(status_on("live") && status_on("recording") && status_on("running"));
        assert!(!status_on("offline") && !status_on("stopping") && !status_on("ending"));
        let model = json!({"id": "scene1", "name": "Gaming", "nodes": [
            {"sceneItemId": "i1", "sourceId": "cam", "name": "Camera", "visible": false},
            {"id": "folder", "name": "Group", "sceneNodeType": "folder"}
        ]});
        let s = scene_from(&model).unwrap();
        assert_eq!(s.name, "Gaming");
        assert_eq!(s.items, vec![("Camera".into(), "i1".into(), "cam".into(), false)], "folders are skipped");
        assert!(scene_from(&json!({"name": "no id"})).is_none());
        // SourcesService.getSources: an audio source carries `muted` (and `audio`), an image does not
        let (all, audio) = source_names(
            &json!([{"sourceId": "a", "name": "Mic/Aux", "type": "wasapi_input_capture", "audio": true, "muted": false}, {"sourceId": "b", "name": "Logo", "type": "image_source", "audio": false}]),
        );
        assert_eq!(all.len(), 2);
        assert_eq!(audio, [super::super::names::Name::same("Mic/Aux")]);
    }
}
