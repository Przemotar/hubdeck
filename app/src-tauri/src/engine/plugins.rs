//! Plugins: separate programs in `<app data>/plugins/<folder>/` with a `manifest.json`. HubDeck
//! starts each one and talks to it over a WebSocket on 127.0.0.1, much like the Stream Deck SDK:
//! the plugin hears presses, turns and slides of the controls bound to its actions and can set
//! their title, picture and on/off state. The same server is the local API for scripts.
//!
//! Manifest:
//! ```json
//! { "id": "com.example.counter", "name": "Counter", "command": "node plugin.js",
//!   "actions": [ { "id": "count", "name": "Count presses", "description": "...", "icon": "zap",
//!                  "inputs": ["press"], "params": [ { "key": "start", "label": "Start at",
//!                  "type": "number", "default": 0 } ] } ] }
//! ```
//! The plugin gets `HUBDECK_PORT`, `HUBDECK_PLUGIN` and `HUBDECK_TOKEN` in its environment and
//! first sends `{"event": "register", "plugin": ..., "token": ...}`.
//!
//! Host -> plugin: `willAppear` / `willDisappear` {action, context, settings}, `keyDown` / `keyUp`
//! {action, context, settings}, `dialRotate` {action, context, settings, ticks}, `slide` {..., value}.
//! Plugin -> host: `setTitle` {context, title}, `setImage` {context, image: data URI}, `setState`
//! {context, state}, `showFeedback` {value 0..1, caption}, `log` {message}. A script using the local
//! API registers as `{"event": "register", "plugin": "api", "token": <contents of api-token>}` and
//! may also send `switchProfile` {profile} and `getState`.

use std::collections::HashMap;
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::{Mutex, OnceLock, RwLock};
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, unbounded};
use serde::Deserialize;
use serde_json::{Value, json};
use tungstenite::{Message, WebSocket};

use super::Msg;
use super::actions::{ActionDef, InputKind, ParamDef, ParamKind};

/// Id prefix of plugin actions: `<plugin id>/<action id>`.
pub fn is_plugin_action(id: &str) -> bool {
    id.contains('/')
}

#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub version: String,
    /// Program and arguments, run in the plugin's folder, e.g. `node plugin.js` or `plugin.exe`.
    pub command: String,
    #[serde(default)]
    pub actions: Vec<ManifestAction>,
    /// What the plugin says it needs: "network", "files", "processes", "input"...; shown to the user.
    #[serde(default)]
    pub permissions: Vec<String>,
    /// A Stream Deck plugin (`sdplugins.rs`): its own manifest; None for HubDeck's own plugins.
    #[serde(skip)]
    pub sd: Option<std::sync::Arc<super::sdplugins::SdManifest>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ManifestAction {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub inputs: Vec<InputKind>,
    #[serde(default)]
    pub params: Vec<ManifestParam>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ManifestParam {
    pub key: String,
    pub label: String,
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub default: Value,
    #[serde(default)]
    pub help: Option<String>,
    #[serde(default)]
    pub options: Vec<(String, String)>,
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
}

/// The catalog holds `&'static str`; plugin texts are loaded once at start and kept for good.
fn leak(s: &str) -> &'static str {
    Box::leak(s.to_string().into_boxed_str())
}

fn safe_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= 80 && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

impl Manifest {
    pub fn action_defs(&self) -> Vec<ActionDef> {
        self.actions
            .iter()
            .filter(|a| safe_id(&a.id))
            .map(|a| ActionDef {
                id: leak(&format!("{}/{}", self.id, a.id)),
                name: leak(&a.name),
                group: leak(&self.name),
                description: leak(&a.description),
                inputs: if a.inputs.is_empty() { vec![InputKind::Press] } else { a.inputs.clone() },
                params: a
                    .params
                    .iter()
                    .map(|p| ParamDef {
                        key: leak(&p.key),
                        label: leak(&p.label),
                        kind: match p.kind.as_str() {
                            "number" => ParamKind::Number { min: p.min.unwrap_or(f64::MIN), max: p.max.unwrap_or(f64::MAX), step: 1.0 },
                            "select" => ParamKind::Select { options: p.options.clone() },
                            "multiline" => ParamKind::Text { multiline: true },
                            "app" => ParamKind::App,
                            "target" => ParamKind::Target,
                            "hotkey" => ParamKind::Hotkey,
                            _ => ParamKind::Text { multiline: false },
                        },
                        default: p.default.clone(),
                        help: p.help.as_deref().map(leak),
                        when: None,
                    })
                    .collect(),
                icon: if a.icon.is_empty() { "zap" } else { leak(&a.icon) },
            })
            .collect()
    }
}

/// Plugin actions, added to the built-in catalog.
static ACTIONS: RwLock<Vec<ActionDef>> = RwLock::new(Vec::new());

pub fn actions() -> Vec<ActionDef> {
    ACTIONS.read().unwrap_or_else(|e| e.into_inner()).clone()
}

struct Plugin {
    manifest: Manifest,
    dir: PathBuf,
    token: String,
}

/// A plugin's running program. On Windows it and everything it starts sit in a job object that
/// ends them all when this is dropped, or when HubDeck itself ends however it ends: a killed or
/// crashed HubDeck left Flight Tracker running, and stopping ScriptDeck ended its `cmd.exe` but
/// not the PowerShell under it (2026-10-17). Chrome and VS Code keep their helpers the same way.
struct Running {
    child: Child,
    #[cfg(windows)]
    _job: Option<job::Job>,
}

impl Running {
    fn new(child: Child) -> Self {
        #[cfg(windows)]
        let _job = job::contain(&child);
        Self {
            child,
            #[cfg(windows)]
            _job,
        }
    }
}

impl std::ops::Deref for Running {
    type Target = Child;
    fn deref(&self) -> &Child {
        &self.child
    }
}

impl std::ops::DerefMut for Running {
    fn deref_mut(&mut self) -> &mut Child {
        &mut self.child
    }
}

#[cfg(windows)]
mod job {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation, SetInformationJobObject,
    };

    pub struct Job(HANDLE);

    // SAFETY: a job handle is a kernel handle, usable from any thread; only Drop closes it.
    unsafe impl Send for Job {}

    impl Drop for Job {
        fn drop(&mut self) {
            // SAFETY: the handle came from CreateJobObjectW and is closed once, here.
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }

    /// A job that ends the child's process tree when the last handle to it closes.
    pub fn contain(child: &std::process::Child) -> Option<Job> {
        // SAFETY: plain Win32 calls on a fresh job and the child's own process handle, which
        // `child` keeps open for the call; the info struct outlives the call.
        unsafe {
            let job = Job(CreateJobObjectW(None, None).ok()?);
            let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            SetInformationJobObject(job.0, JobObjectExtendedLimitInformation, &info as *const _ as *const _, std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32).ok()?;
            if let Err(e) = AssignProcessToJobObject(job.0, HANDLE(child.as_raw_handle())) {
                log::warn!("plugin process {} not in a job: {e}", child.id());
                return None;
            }
            Some(job)
        }
    }
}

struct Host {
    /// Installed plugins; Stream Deck plugins installed from the window join while HubDeck runs.
    plugins: RwLock<Vec<Plugin>>,
    api_token: String,
    /// plugin id -> messages to its connection.
    outbox: Mutex<HashMap<String, Sender<String>>>,
    /// plugin id -> its running program.
    children: Mutex<HashMap<String, Running>>,
    /// plugin id -> (restarts so far, what happened last), for the plugins page.
    status: Mutex<HashMap<String, (u32, String)>>,
    port: u16,
    dir: PathBuf,
    /// Controls currently shown with a plugin action: context -> (plugin, action, settings).
    visible: Mutex<HashMap<String, (String, String, Value)>>,
    /// Plugins the user stopped on the Plugins page: the watchdog leaves them stopped.
    held: Mutex<std::collections::HashSet<String>>,
}

static HOST: OnceLock<Host> = OnceLock::new();

/// Gives the engine state as JSON for `GET /api/state` (set by the engine at start).
static STATE: OnceLock<Box<dyn Fn() -> Value + Send + Sync>> = OnceLock::new();

pub fn set_state_source(f: Box<dyn Fn() -> Value + Send + Sync>) {
    let _ = STATE.set(f);
}

/// Answers the routes of [`super::api`] (profiles, catalog, pictures...): `(method, path, body)`
/// -> `Some((status, JSON))`, or None for a path it does not know. Set once the engine runs.
type ApiHandler = Box<dyn Fn(&str, &str, &str) -> Option<(u16, Value)> + Send + Sync>;
static API: OnceLock<ApiHandler> = OnceLock::new();

pub fn set_api_handler(handler: ApiHandler) {
    let _ = API.set(handler);
}

/// Largest request body: animation frames come as base64 pictures.
const MAX_BODY: u64 = 16_000_000;

/// The same API over plain HTTP, for `curl` and hooks: `POST /api/<event>` with a JSON body (the
/// fields of the WebSocket message), `GET /api/state`, and the routes of [`super::api`]. The token
/// goes in `Authorization: Bearer`.
fn serve_http(server: tiny_http::Server, token: String, engine: Sender<Msg>) {
    for mut req in server.incoming_requests() {
        let authorized = req.headers().iter().any(|h| h.field.equiv("Authorization") && h.value.as_str().trim() == format!("Bearer {token}"));
        let path = req.url().split('?').next().unwrap_or("").to_string();
        let reply =
            |code: u16, body: String| tiny_http::Response::from_string(body).with_status_code(code).with_header("Content-Type: application/json".parse::<tiny_http::Header>().expect("static header"));
        let mut body = String::new();
        if authorized && req.method() == &tiny_http::Method::Post {
            let _ = std::io::Read::read_to_string(&mut std::io::Read::take(req.as_reader(), MAX_BODY), &mut body);
        }
        let routed = if authorized { API.get().and_then(|api| api(req.method().as_str(), &path, &body)) } else { None };
        let response = if !authorized {
            reply(401, r#"{"error":"missing or wrong token (Authorization: Bearer <contents of api-token>)"}"#.into())
        } else if let Some((code, value)) = routed {
            reply(code, value.to_string())
        } else if req.method() == &tiny_http::Method::Get && path == "/api/state" {
            reply(200, STATE.get().map(|f| f().to_string()).unwrap_or_else(|| "{}".into()))
        } else if let (tiny_http::Method::Post, Some(event)) = (req.method(), path.strip_prefix("/api/")) {
            match serde_json::from_str::<Value>(if body.trim().is_empty() { "{}" } else { &body }) {
                Ok(Value::Object(mut map)) if !event.is_empty() && event.chars().all(|c| c.is_ascii_alphanumeric()) => {
                    map.insert("event".into(), Value::String(event.to_string()));
                    let _ = engine.send(Msg::FromPlugin { plugin: "http".into(), message: Value::Object(map) });
                    reply(202, r#"{"ok":true}"#.into())
                }
                Ok(_) => reply(400, r#"{"error":"the body must be a JSON object and the path /api/<event>"}"#.into()),
                Err(e) => reply(400, json!({"error": format!("not JSON: {e}")}).to_string()),
            }
        } else {
            reply(404, r#"{"error":"use GET /api/state or POST /api/<event>"}"#.into())
        };
        let _ = req.respond(response);
    }
}

fn random_token() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

/// Read every manifest under `dir`; broken ones are skipped with a log line.
pub fn discover(dir: &Path) -> Vec<(Manifest, PathBuf)> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return out };
    for e in entries.flatten().filter(|e| e.path().is_dir()) {
        let path = e.path().join("manifest.json");
        if e.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        let read = std::fs::read(&path).map_err(|e| e.to_string()).and_then(|b| serde_json::from_slice::<Value>(super::sdplugins::without_bom(&b)).map_err(|e| e.to_string())).and_then(|v| {
            if super::sdplugins::is_sd_manifest(&v) {
                super::sdplugins::read(&v, &e.path()).map(super::sdplugins::to_manifest)
            } else {
                serde_json::from_value::<Manifest>(v).map_err(|e| e.to_string())
            }
        });
        match read {
            Ok(m) if safe_id(&m.id) => out.push((m, e.path())),
            Ok(m) => log::warn!("plugin {}: id {:?} may use only letters, digits, . - _", path.display(), m.id),
            Err(err) => log::warn!("plugin {}: {err}", path.display()),
        }
    }
    out
}

/// Load plugins, start the server and the plugin programs. The API token is written to
/// `<data>/api-token` for local scripts.
pub fn start(data_dir: &Path, engine: Sender<Msg>) {
    stop_orphans(&data_dir.join("plugins"));
    super::sdplugins::start_files(data_dir, data_dir.join("plugins"));
    let found = discover(&data_dir.join("plugins"));
    *ACTIONS.write().unwrap_or_else(|e| e.into_inner()) = found.iter().flat_map(|(m, _)| m.action_defs()).collect();
    let api_token = std::fs::read_to_string(data_dir.join("api-token")).ok().map(|s| s.trim().to_string()).filter(|s| s.len() >= 16).unwrap_or_else(|| {
        let t = random_token();
        let _ = super::profile::write_atomic(&data_dir.join("api-token"), t.as_bytes());
        t
    });
    let listener = match TcpListener::bind("127.0.0.1:0") {
        Ok(l) => l,
        Err(e) => {
            log::error!("plugin server: {e}");
            return;
        }
    };
    let port = listener.local_addr().map(|a| a.port()).unwrap_or(0);
    let _ = super::profile::write_atomic(&data_dir.join("api-port"), port.to_string().as_bytes());
    match tiny_http::Server::http("127.0.0.1:0") {
        Ok(server) => {
            let http_port = server.server_addr().to_ip().map(|a| a.port()).unwrap_or(0);
            let _ = super::profile::write_atomic(&data_dir.join("api-http-port"), http_port.to_string().as_bytes());
            let (token, tx) = (api_token.clone(), engine.clone());
            std::thread::Builder::new().name("api-http".into()).spawn(move || serve_http(server, token, tx)).expect("spawn api http");
            log::info!("local API over HTTP on 127.0.0.1:{http_port}");
        }
        Err(e) => log::warn!("local API over HTTP: {e}"),
    }
    let plugins = found.into_iter().map(|(manifest, dir)| Plugin { manifest, dir, token: random_token() }).collect();
    let host = Host {
        plugins: RwLock::new(plugins),
        api_token,
        outbox: Mutex::new(HashMap::new()),
        children: Mutex::new(HashMap::new()),
        status: Mutex::new(HashMap::new()),
        port,
        dir: data_dir.join("plugins"),
        visible: Mutex::new(HashMap::new()),
        held: Mutex::new(std::collections::HashSet::new()),
    };
    if HOST.set(host).is_err() {
        return;
    }
    std::thread::Builder::new().name("plugin-server".into()).spawn(move || serve(listener, engine)).expect("spawn plugin server");
    let host = HOST.get().expect("just set");
    for p in host.plugins.read().unwrap_or_else(|e| e.into_inner()).iter() {
        launch(p, port);
    }
    std::thread::Builder::new().name("plugin-watch".into()).spawn(watch).expect("spawn plugin watchdog");
    log::info!("plugin server on 127.0.0.1:{port}, {} plugin(s)", host.plugins.read().unwrap_or_else(|e| e.into_inner()).len());
}

fn launch(p: &Plugin, port: u16) {
    if let Some(sd) = &p.manifest.sd {
        return launch_sd(p, sd, port);
    }
    let mut parts = p.manifest.command.split_whitespace();
    let Some(program) = parts.next() else { return };
    // a program inside the plugin folder wins over one on the PATH
    let local = p.dir.join(program);
    let program = if local.is_file() { local.into_os_string() } else { program.into() };
    let mut cmd = Command::new(program);
    cmd.args(parts).current_dir(&p.dir).env("HUBDECK_PORT", port.to_string()).env("HUBDECK_PLUGIN", &p.manifest.id).env("HUBDECK_TOKEN", &p.token);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: no console window per plugin
    }
    let Some(host) = HOST.get() else { return };
    let mut status = host.status.lock().unwrap_or_else(|e| e.into_inner());
    let entry = status.entry(p.manifest.id.clone()).or_insert((0, String::new()));
    match cmd.spawn() {
        Ok(child) => {
            log::info!("plugin {} {} started", p.manifest.id, p.manifest.version);
            entry.1 = "running".into();
            host.children.lock().unwrap_or_else(|e| e.into_inner()).insert(p.manifest.id.clone(), Running::new(child));
        }
        Err(e) => {
            log::warn!("plugin {}: cannot start {:?}: {e}", p.manifest.id, p.manifest.command);
            entry.1 = format!("cannot start {:?}: {e}", p.manifest.command);
        }
    }
}

/// A Stream Deck plugin: Node, a native program or an HTML page in a hidden window.
fn launch_sd(p: &Plugin, sd: &super::sdplugins::SdManifest, port: u16) {
    let Some(host) = HOST.get() else { return };
    let result = super::sdplugins::launch(sd, &p.dir, port, &p.token).and_then(|l| match l {
        super::sdplugins::Launch::Html(url) => super::sdplugins::open_html(&p.manifest.id, &url).map(|_| None),
        super::sdplugins::Launch::Process(mut cmd) => {
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x0800_0000);
            }
            cmd.spawn().map(Some).map_err(|e| format!("cannot start {}: {e}", sd.code_path))
        }
    });
    let mut status = host.status.lock().unwrap_or_else(|e| e.into_inner());
    let entry = status.entry(p.manifest.id.clone()).or_insert((0, String::new()));
    match result {
        Ok(child) => {
            log::info!("Stream Deck plugin {} {} started{}", p.manifest.id, p.manifest.version, if child.is_none() { " (HTML)" } else { "" });
            entry.1 = "running".into();
            if let Some(child) = child {
                host.children.lock().unwrap_or_else(|e| e.into_inner()).insert(p.manifest.id.clone(), Running::new(child));
            }
        }
        Err(e) => {
            log::warn!("Stream Deck plugin {}: {e}", p.manifest.id);
            entry.1 = e;
        }
    }
}

/// A plugin that crashed is started again, up to `MAX_RESTARTS` times, a few seconds later.
const MAX_RESTARTS: u32 = 5;

fn watch() {
    let mut exited_at: HashMap<String, Instant> = HashMap::new();
    loop {
        std::thread::sleep(Duration::from_secs(2));
        let Some(host) = HOST.get() else { return };
        let mut ended = Vec::new();
        for (id, child) in host.children.lock().unwrap_or_else(|e| e.into_inner()).iter_mut() {
            if let Ok(Some(code)) = child.try_wait() {
                ended.push((id.clone(), code.to_string()));
            }
        }
        for (id, code) in ended {
            host.children.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
            // stopped on the Plugins page: not a crash, nothing to start again
            if host.held.lock().unwrap_or_else(|e| e.into_inner()).contains(&id) {
                continue;
            }
            log::warn!("plugin {id} stopped ({code})");
            exited_at.insert(id.clone(), Instant::now());
            host.status.lock().unwrap_or_else(|e| e.into_inner()).entry(id).or_insert((0, String::new())).1 = format!("stopped ({code})");
        }
        let due: Vec<String> = exited_at.iter().filter(|(_, at)| at.elapsed() >= Duration::from_secs(3)).map(|(id, _)| id.clone()).collect();
        for id in due {
            exited_at.remove(&id);
            let restarts = {
                let mut status = host.status.lock().unwrap_or_else(|e| e.into_inner());
                let entry = status.entry(id.clone()).or_insert((0, String::new()));
                entry.0 += 1;
                entry.0
            };
            if restarts > MAX_RESTARTS {
                host.status.lock().unwrap_or_else(|e| e.into_inner()).entry(id.clone()).or_insert((0, String::new())).1 = format!("stopped {MAX_RESTARTS} times; see the log");
                continue;
            }
            if let Some(p) = host.plugins.read().unwrap_or_else(|e| e.into_inner()).iter().find(|p| p.manifest.id == id) {
                launch(p, host.port);
            }
        }
    }
}

/// Stop a plugin by hand (it eats the CPU, it misbehaves): its program ends and the watchdog leaves
/// it stopped until Start (owner's pick, 2026-10-16; PAIN-POINTS SD11: plugins that eat the CPU).
pub fn stop_one(id: &str) -> Result<(), String> {
    let host = HOST.get().ok_or("plugins are not running")?;
    host.held.lock().unwrap_or_else(|e| e.into_inner()).insert(id.to_string());
    if let Some(mut child) = host.children.lock().unwrap_or_else(|e| e.into_inner()).remove(id) {
        let _ = child.kill();
        let _ = child.wait();
    }
    host.outbox.lock().unwrap_or_else(|e| e.into_inner()).remove(id);
    host.status.lock().unwrap_or_else(|e| e.into_inner()).entry(id.to_string()).or_insert((0, String::new())).1 = "stopped by you".into();
    log::info!("plugin {id} stopped by the user");
    Ok(())
}

/// Start a plugin stopped by hand (or one the watchdog gave up on), with a fresh restart count.
pub fn start_one(id: &str) -> Result<(), String> {
    let host = HOST.get().ok_or("plugins are not running")?;
    host.held.lock().unwrap_or_else(|e| e.into_inner()).remove(id);
    if host.children.lock().unwrap_or_else(|e| e.into_inner()).contains_key(id) {
        return Ok(());
    }
    host.status.lock().unwrap_or_else(|e| e.into_inner()).insert(id.to_string(), (0, "starting".into()));
    let plugins = host.plugins.read().unwrap_or_else(|e| e.into_inner());
    let p = plugins.iter().find(|p| p.manifest.id == id).ok_or_else(|| format!("no plugin {id}"))?;
    launch(p, host.port);
    Ok(())
}

/// Plugin programs left running by an earlier HubDeck that crashed (no Exit, so `stop` never ran):
/// a process started from the plugins folder whose parent is gone is stopped before new copies
/// start. A plugin of a HubDeck that still runs keeps its parent and is left alone.
fn stop_orphans(plugins_dir: &Path) {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
    if !plugins_dir.is_dir() {
        return;
    }
    // the path as the plugins were started with (`canonicalize` adds \\?\ on Windows)
    let dir = plugins_dir;
    let mut sys = System::new();
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing().with_cmd(UpdateKind::Always).with_exe(UpdateKind::Always).with_cwd(UpdateKind::Always));
    let me = sysinfo::Pid::from_u32(std::process::id());
    for (pid, p) in sys.processes() {
        // a plugin program, Node with a plugin's script, or a program run in a plugin's folder
        let from_plugins = p.exe().is_some_and(|e| e.starts_with(dir)) || p.cwd().is_some_and(|c| c.starts_with(dir)) || p.cmd().iter().any(|a| Path::new(a).starts_with(dir));
        let orphan = match p.parent() {
            None => true,
            Some(parent) => parent != me && (parent.as_u32() == 1 || sys.process(parent).is_none()),
        };
        if *pid != me && from_plugins && orphan && p.kill() {
            log::info!("stopped a plugin program left by an earlier run (pid {pid})");
        }
    }
}

/// Stop every plugin program (app exit).
pub fn stop() {
    if let Some(host) = HOST.get() {
        for (_, mut c) in std::mem::take(&mut *host.children.lock().unwrap_or_else(|e| e.into_inner())) {
            let _ = c.kill();
        }
    }
}

/// One installed plugin for the plugins page.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub actions: usize,
    /// "running", "stopped (...)", "cannot start ..."
    pub status: String,
    pub connected: bool,
    pub restarts: u32,
    pub permissions: Vec<String>,
    /// CPU use in percent of one core, and memory in MB, while it runs.
    pub cpu: Option<f32>,
    pub memory_mb: Option<u64>,
    /// Stopped on the Plugins page.
    pub held: bool,
}

/// CPU and memory of the plugin processes, refreshed on each call (the page asks every 2 s).
static USAGE: Mutex<Option<sysinfo::System>> = Mutex::new(None);

pub fn list() -> Vec<PluginInfo> {
    let Some(host) = HOST.get() else { return Vec::new() };
    let pids: HashMap<String, sysinfo::Pid> = host.children.lock().unwrap_or_else(|e| e.into_inner()).iter().map(|(id, c)| (id.clone(), sysinfo::Pid::from_u32(c.id()))).collect();
    let mut usage = USAGE.lock().unwrap_or_else(|e| e.into_inner());
    let sys = usage.get_or_insert_with(sysinfo::System::new);
    let wanted: Vec<sysinfo::Pid> = pids.values().copied().collect();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&wanted), true);
    let status = host.status.lock().unwrap_or_else(|e| e.into_inner());
    let outbox = host.outbox.lock().unwrap_or_else(|e| e.into_inner());
    let held = host.held.lock().unwrap_or_else(|e| e.into_inner());
    host.plugins
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .map(|p| {
            let (restarts, s) = status.get(&p.manifest.id).cloned().unwrap_or((0, "not started".into()));
            PluginInfo {
                id: p.manifest.id.clone(),
                name: p.manifest.name.clone(),
                version: p.manifest.version.clone(),
                actions: p.manifest.actions.len(),
                status: s,
                connected: outbox.contains_key(&p.manifest.id),
                restarts,
                permissions: p.manifest.permissions.clone(),
                cpu: pids.get(&p.manifest.id).and_then(|pid| sys.process(*pid)).map(|pr| pr.cpu_usage()),
                memory_mb: pids.get(&p.manifest.id).and_then(|pid| sys.process(*pid)).map(|pr| pr.memory() / 1_048_576),
                held: held.contains(&p.manifest.id),
            }
        })
        .collect()
}

/// The plugin server's WebSocket port (Stream Deck settings pages connect to it).
pub fn port() -> u16 {
    HOST.get().map_or(0, |h| h.port)
}

/// An installed plugin's folder.
pub fn folder_of(plugin: &str) -> Option<PathBuf> {
    HOST.get()?.plugins.read().unwrap_or_else(|e| e.into_inner()).iter().find(|p| p.manifest.id == plugin).map(|p| p.dir.clone())
}

/// A Stream Deck plugin's manifest and folder.
pub fn sd_of(plugin: &str) -> Option<(std::sync::Arc<super::sdplugins::SdManifest>, PathBuf)> {
    HOST.get()?.plugins.read().unwrap_or_else(|e| e.into_inner()).iter().find(|p| p.manifest.id == plugin).and_then(|p| Some((p.manifest.sd.clone()?, p.dir.clone())))
}

/// Install a Stream Deck plugin file and start it at once (an installed version is stopped and
/// replaced). Returns the plugin's id.
pub fn install_sd(bytes: &[u8]) -> Result<String, String> {
    let host = HOST.get().ok_or("plugins are not running")?;
    std::fs::create_dir_all(&host.dir).map_err(|e| e.to_string())?;
    // stop a running copy first: Windows cannot replace files in use
    let peek = super::sdplugins::install_root(bytes)?;
    if let Some(mut c) = host.children.lock().unwrap_or_else(|e| e.into_inner()).remove(&peek) {
        let _ = c.kill();
        let _ = c.wait();
    }
    let id = super::sdplugins::install(bytes, &host.dir)?;
    let dir = host.dir.join(format!("{id}.sdPlugin"));
    let v: Value = std::fs::read(dir.join("manifest.json")).ok().and_then(|b| serde_json::from_slice(super::sdplugins::without_bom(&b)).ok()).ok_or("no manifest")?;
    let manifest = super::sdplugins::to_manifest(super::sdplugins::read(&v, &dir)?);
    let plugin = Plugin { manifest, dir, token: random_token() };
    {
        let mut actions = ACTIONS.write().unwrap_or_else(|e| e.into_inner());
        actions.retain(|a| !a.id.starts_with(&format!("{id}/")));
        actions.extend(plugin.manifest.action_defs());
    }
    launch(&plugin, host.port);
    let mut plugins = host.plugins.write().unwrap_or_else(|e| e.into_inner());
    plugins.retain(|p| p.manifest.id != id);
    plugins.push(plugin);
    log::info!("installed Stream Deck plugin {id}");
    Ok(id)
}

/// Stop a plugin and take its actions out of the library (its folder stays; the caller removes it).
pub fn uninstall(id: &str) {
    let Some(host) = HOST.get() else { return };
    if let Some(mut c) = host.children.lock().unwrap_or_else(|e| e.into_inner()).remove(id) {
        let _ = c.kill();
        let _ = c.wait();
    }
    ACTIONS.write().unwrap_or_else(|e| e.into_inner()).retain(|a| !a.id.starts_with(&format!("{id}/")));
    host.plugins.write().unwrap_or_else(|e| e.into_inner()).retain(|p| p.manifest.id != id);
}

/// Where plugin folders go (created if missing).
pub fn folder() -> Option<PathBuf> {
    let dir = HOST.get()?.dir.clone();
    let _ = std::fs::create_dir_all(&dir);
    Some(dir)
}

fn serve(listener: TcpListener, engine: Sender<Msg>) {
    for stream in listener.incoming().flatten() {
        let engine = engine.clone();
        std::thread::spawn(move || {
            if let Err(e) = connection(stream, engine) {
                log::debug!("plugin connection: {e}");
            }
        });
    }
}

/// A message's JSON text: plugins send it as a text frame, some as a binary one (ScriptDeck's
/// PowerShell, 2026-10-17), and Elgato's app reads both.
fn json_text(m: &Message) -> Option<&str> {
    match m {
        Message::Text(t) => Some(t.as_str()),
        Message::Binary(b) => std::str::from_utf8(b).ok(),
        _ => None,
    }
}

fn connection(stream: TcpStream, engine: Sender<Msg>) -> Result<(), String> {
    let mut ws: WebSocket<TcpStream> = tungstenite::accept(stream).map_err(|e| e.to_string())?;
    let host = HOST.get().ok_or("no host")?;
    let first = match json_text(&ws.read().map_err(|e| e.to_string())?) {
        Some(t) => serde_json::from_str::<Value>(t).map_err(|e| e.to_string())?,
        None => return Err("expected a register message".into()),
    };
    // a Stream Deck plugin or one of its settings pages
    if first["event"] == "registerPropertyInspector" {
        return inspector_connection(ws, first["uuid"].as_str().unwrap_or(""), engine);
    }
    let sd_uuid = (first["event"] == "registerPlugin").then(|| first["uuid"].as_str().unwrap_or("").to_string());
    let (who, sd_dir) = match &sd_uuid {
        Some(uuid) => host
            .plugins
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .find(|p| p.manifest.sd.is_some() && p.token == *uuid)
            .map(|p| (p.manifest.id.clone(), Some(p.dir.clone())))
            .unwrap_or_default(),
        None => (first["plugin"].as_str().unwrap_or("").to_string(), None),
    };
    let token = first["token"].as_str().unwrap_or("");
    let ok = (sd_uuid.is_some() && !who.is_empty())
        || (first["event"] == "register"
            && ((who == "api" && token == host.api_token) || host.plugins.read().unwrap_or_else(|e| e.into_inner()).iter().any(|p| p.manifest.id == who && p.token == token)));
    if !ok {
        let _ = ws.send(Message::text(json!({"event": "error", "message": "unknown plugin or token"}).to_string()));
        return Err(format!("refused {who}"));
    }
    log::info!("plugin {who} connected");
    let (tx, rx): (Sender<String>, Receiver<String>) = unbounded();
    host.outbox.lock().unwrap_or_else(|e| e.into_inner()).insert(who.clone(), tx);
    if sd_dir.is_some() {
        let _ = ws
            .send(Message::text(json!({"event": "deviceDidConnect", "device": super::sdplugins::DEVICE, "deviceInfo": {"name": "HubDeck", "size": {"columns": 5, "rows": 3}, "type": 7}}).to_string()));
    }
    // tell it about the controls it already has on screen
    for (context, (plugin, action, settings)) in host.visible.lock().unwrap_or_else(|e| e.into_inner()).iter() {
        if *plugin == who {
            let m = json!({"event": "willAppear", "action": action, "context": context, "settings": settings});
            let m = if sd_dir.is_some() { super::sdplugins::outgoing(&m) } else { Some(m) };
            if let Some(m) = m {
                let _ = ws.send(Message::text(m.to_string()));
            }
        }
    }
    ws.get_mut().set_read_timeout(Some(Duration::from_millis(30))).map_err(|e| e.to_string())?;
    loop {
        for out in rx.try_iter() {
            ws.send(Message::text(out)).map_err(|e| e.to_string())?;
        }
        match ws.read() {
            Ok(m @ (Message::Text(_) | Message::Binary(_))) => {
                let t = json_text(&m).unwrap_or_default();
                if let (Ok(v), Some(dir)) = (serde_json::from_str::<Value>(t), &sd_dir) {
                    for out in super::sdplugins::incoming(&who, dir, &v) {
                        match out {
                            super::sdplugins::Incoming::Engine(m) => {
                                let _ = engine.send(Msg::FromPlugin { plugin: who.clone(), message: m });
                            }
                            super::sdplugins::Incoming::Reply(m) => ws.send(Message::text(m.to_string())).map_err(|e| e.to_string())?,
                            super::sdplugins::Incoming::Inspector(context, m) => super::sdplugins::to_inspector(&context, &m),
                            super::sdplugins::Incoming::Nothing => {}
                        }
                    }
                } else if let Ok(v) = serde_json::from_str::<Value>(t) {
                    if v["event"] == "getState" {
                        let _ = engine.send(Msg::ApiStateRequest(who.clone()));
                    } else {
                        let _ = engine.send(Msg::FromPlugin { plugin: who.clone(), message: v });
                    }
                }
            }
            Ok(Message::Close(_)) => break,
            Ok(_) => {}
            Err(tungstenite::Error::Io(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
            Err(e) => {
                log::info!("plugin {who} disconnected ({e})");
                break;
            }
        }
    }
    host.outbox.lock().unwrap_or_else(|e| e.into_inner()).remove(&who);
    Ok(())
}

/// Send a message to a plugin (or a local API client) if it is connected; a Stream Deck plugin
/// gets it in its own words (messages it has none for are dropped).
pub fn send_to(plugin: &str, message: Value) {
    let Some(host) = HOST.get() else { return };
    let sd = host.plugins.read().unwrap_or_else(|e| e.into_inner()).iter().any(|p| p.manifest.id == plugin && p.manifest.sd.is_some());
    let message = if sd { super::sdplugins::outgoing(&message) } else { Some(message) };
    if let (Some(message), Some(tx)) = (message, host.outbox.lock().unwrap_or_else(|e| e.into_inner()).get(plugin)) {
        let _ = tx.send(message.to_string());
    }
}

/// A message already in a Stream Deck plugin's words.
fn send_raw(plugin: &str, message: &Value) {
    if let Some(host) = HOST.get()
        && let Some(tx) = host.outbox.lock().unwrap_or_else(|e| e.into_inner()).get(plugin)
    {
        let _ = tx.send(message.to_string());
    }
}

/// A Stream Deck settings page: it registered with the uuid HubDeck put in its address.
fn inspector_connection(mut ws: WebSocket<TcpStream>, uuid: &str, engine: Sender<Msg>) -> Result<(), String> {
    let (plugin, action, context) = super::sdplugins::inspector_for(uuid).ok_or("unknown settings page")?;
    let (tx, rx): (Sender<String>, Receiver<String>) = unbounded();
    super::sdplugins::inspector_connected(&context, Some(tx));
    send_raw(&plugin, &json!({"event": "propertyInspectorDidAppear", "action": action, "context": context, "device": super::sdplugins::DEVICE}));
    ws.get_mut().set_read_timeout(Some(Duration::from_millis(30))).map_err(|e| e.to_string())?;
    let result = loop {
        let sent: Result<(), String> = rx.try_iter().try_for_each(|out| ws.send(Message::text(out)).map_err(|e| e.to_string()));
        if let Err(e) = sent {
            break Err(e);
        }
        match ws.read() {
            Ok(m @ (Message::Text(_) | Message::Binary(_))) => {
                let Ok(v) = serde_json::from_str::<Value>(json_text(&m).unwrap_or_default()) else { continue };
                for (target, m) in super::sdplugins::from_inspector(&plugin, &action, &context, &v) {
                    match target {
                        super::sdplugins::Target::Plugin => send_raw(&plugin, &m),
                        super::sdplugins::Target::Engine => {
                            let _ = engine.send(Msg::FromPlugin { plugin: plugin.clone(), message: m });
                        }
                        super::sdplugins::Target::Page => {
                            if let Err(e) = ws.send(Message::text(m.to_string())) {
                                log::debug!("settings page: {e}");
                            }
                        }
                    }
                }
            }
            Ok(Message::Close(_)) => break Ok(()),
            Ok(_) => {}
            Err(tungstenite::Error::Io(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
            Err(e) => break Err(e.to_string()),
        }
    };
    super::sdplugins::inspector_connected(&context, None);
    send_raw(&plugin, &json!({"event": "propertyInspectorDidDisappear", "action": action, "context": context, "device": super::sdplugins::DEVICE}));
    result
}

/// A control bound to a plugin action was pressed, turned or slid.
/// `swap_turns`: a plain turn goes out as a turn while pressed and the other way round (Stream Deck
/// dial actions; the binding's `swapTurns`).
pub fn input(action: &str, context: &str, settings: &Value, event: super::exec::InputEvent, swap_turns: bool) {
    let Some((plugin, action)) = action.split_once('/') else { return };
    use super::exec::InputEvent;
    if swap_turns {
        return swapped_input(plugin, action, context, settings, event);
    }
    let msg = match event {
        InputEvent::Press(true) => json!({"event": "keyDown", "action": action, "context": context, "settings": settings}),
        InputEvent::Press(false) => json!({"event": "keyUp", "action": action, "context": context, "settings": settings}),
        InputEvent::Turn(d) => json!({"event": "dialRotate", "action": action, "context": context, "settings": settings, "ticks": d}),
        InputEvent::Slide(v) => json!({"event": "slide", "action": action, "context": context, "settings": settings, "value": v}),
    };
    send_to(plugin, msg);
}

/// A dial action with turning and turning-while-pressed swapped. Plugins follow the order of
/// events (down, turns, up), so the order is played for them: a plain turn becomes down, the turns
/// and an up once the turning stops; turns while the dial is held go out plain, and a press without
/// turns is a press (down and up at release).
fn swapped_input(plugin: &str, action: &str, context: &str, settings: &Value, event: super::exec::InputEvent) {
    use super::exec::InputEvent;
    static HELD: Mutex<Option<HashMap<String, bool>>> = Mutex::new(None);
    static TURNING: Mutex<Option<HashMap<String, Instant>>> = Mutex::new(None);
    let msg = |event: &str| json!({"event": event, "action": action, "context": context, "settings": settings});
    let mut held = HELD.lock().unwrap_or_else(|e| e.into_inner());
    let held = held.get_or_insert_with(HashMap::new);
    match event {
        InputEvent::Press(true) => {
            held.insert(context.to_string(), false);
        }
        InputEvent::Press(false) => {
            if held.remove(context) == Some(false) {
                send_to(plugin, msg("keyDown"));
                send_to(plugin, msg("keyUp"));
            }
        }
        InputEvent::Turn(d) => {
            if let Some(turned) = held.get_mut(context) {
                *turned = true;
                send_to(plugin, json!({"event": "dialRotate", "action": action, "context": context, "settings": settings, "ticks": d}));
                return;
            }
            let mut turning = TURNING.lock().unwrap_or_else(|e| e.into_inner());
            let turning = turning.get_or_insert_with(HashMap::new);
            let first = !turning.contains_key(context);
            turning.insert(context.to_string(), Instant::now());
            if first {
                send_to(plugin, msg("keyDown"));
                // let go once the turning stops
                let (plugin, up) = (plugin.to_string(), msg("keyUp"));
                let context = context.to_string();
                std::thread::spawn(move || {
                    loop {
                        std::thread::sleep(Duration::from_millis(100));
                        let mut t = TURNING.lock().unwrap_or_else(|e| e.into_inner());
                        let t = t.get_or_insert_with(HashMap::new);
                        if t.get(&context).is_none_or(|at| at.elapsed() >= Duration::from_millis(450)) {
                            t.remove(&context);
                            send_to(&plugin, up);
                            return;
                        }
                    }
                });
            }
            send_to(plugin, json!({"event": "dialRotate", "action": action, "context": context, "settings": settings, "ticks": d}));
        }
        InputEvent::Slide(_) => {}
    }
}

/// The controls on screen that use plugin actions; tells plugins what appeared and disappeared.
/// Returns the controls whose plugin action went away or changed.
pub fn set_visible(now: HashMap<String, (String, String, Value)>) -> Vec<String> {
    let Some(host) = HOST.get() else { return Vec::new() };
    let mut visible = host.visible.lock().unwrap_or_else(|e| e.into_inner());
    let mut gone = Vec::new();
    for (context, (plugin, action, _)) in visible.iter() {
        let same = now.get(context).is_some_and(|(p, a, _)| p == plugin && a == action);
        if !same {
            send_to(plugin, json!({"event": "willDisappear", "action": action, "context": context}));
            gone.push(context.clone());
        }
    }
    for (context, (plugin, action, settings)) in &now {
        // settings a Stream Deck plugin or its page just saved: it knows them already
        let saved_by_it = visible.get(context).is_some_and(|(p, a, _)| p == plugin && a == action) && super::sdplugins::settings_known(context, settings);
        if !saved_by_it && visible.get(context) != Some(&(plugin.clone(), action.clone(), settings.clone())) {
            send_to(plugin, json!({"event": "willAppear", "action": action, "context": context, "settings": settings}));
        }
    }
    *visible = now;
    gone
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_becomes_catalog_entries() {
        let m: Manifest = serde_json::from_value(json!({
            "id": "com.example.counter", "name": "Counter", "command": "node plugin.js",
            "actions": [
                {"id": "count", "name": "Count presses", "inputs": ["press", "turn"],
                 "params": [{"key": "start", "label": "Start at", "type": "number", "default": 0, "min": 0, "max": 100},
                            {"key": "mode", "label": "Mode", "type": "select", "default": "a", "options": [["a", "A"], ["b", "B"]]}]},
                {"id": "bad id!", "name": "Skipped"}
            ]
        }))
        .unwrap();
        let defs = m.action_defs();
        assert_eq!(defs.len(), 1, "unsafe action ids are skipped");
        assert_eq!(defs[0].id, "com.example.counter/count");
        assert_eq!(defs[0].group, "Counter");
        assert_eq!(defs[0].inputs, vec![InputKind::Press, InputKind::Turn]);
        assert!(matches!(defs[0].params[0].kind, ParamKind::Number { min, max, .. } if min == 0.0 && max == 100.0));
        assert!(is_plugin_action(defs[0].id));
        assert!(!is_plugin_action("core.media"));
    }

    #[test]
    fn discovery_skips_broken_manifests() {
        let dir = std::env::temp_dir().join(format!("hubdeck-plugins-{}", std::process::id()));
        let _ = std::fs::create_dir_all(dir.join("good"));
        let _ = std::fs::create_dir_all(dir.join("broken"));
        std::fs::write(dir.join("good/manifest.json"), r#"{"id":"a.b","name":"A","command":"x"}"#).unwrap();
        std::fs::write(dir.join("broken/manifest.json"), "{").unwrap();
        let found = discover(&dir);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0.id, "a.b");
        let _ = std::fs::remove_dir_all(dir);
    }
}
