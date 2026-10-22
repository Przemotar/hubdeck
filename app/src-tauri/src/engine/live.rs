//! "Live value": a key or the dial shows a value that comes from somewhere and refreshes by
//! itself: a web address (JSON with a path, or plain text, with an optional token from the
//! credential store), a Home Assistant entity, or a local command's output. Thresholds color the
//! key, the dial draws a ring when a maximum is set, a press opens a page (or refreshes).
//!
//! One worker thread fetches what is on screen now (the engine tells it on every profile or page
//! change); it starts the first time a live value is shown, so the deck costs nothing while none
//! is. Each result goes to the engine as `IntegrationValue { key: "live:<source>", value: JSON }`
//! and the engine draws it with [`key_look`] / [`dial_face`].

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, unbounded};
use serde_json::Value;

use super::Msg;
use super::actions::{param_f64, param_str};
use super::profile::Look;

/// The action id.
pub const ACTION: &str = "core.live";
/// Shortest refresh: nothing is fetched more often than this.
pub const MIN_EVERY: Duration = Duration::from_secs(5);
const TIMEOUT: Duration = Duration::from_secs(8);
/// Largest answer read from a web address.
const MAX_BODY: u64 = 1024 * 1024;
/// Prefix of a credential store entry holding a token for a web address.
pub const SECRET_PREFIX: &str = "live:";

/// Where a value comes from.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Kind {
    /// A web address; `path` picks a value from JSON (`data.items.0.price`), empty = the whole
    /// answer; `secret` names a token in the credential store, sent as `Authorization: Bearer`.
    Url { url: String, path: String, secret: String },
    /// A Home Assistant entity's state (the address and token of Settings > Connections).
    Ha { entity: String, base: String },
    /// A command line; its output's first line. Runs only once the user allowed it on this computer.
    Command { command: String },
    /// The weather now from Open-Meteo (no key): `place` is `Name|lat|lon` from the city search,
    /// or just a name (looked up once).
    Weather { place: String, fahrenheit: bool },
    /// The next event of a calendar published as an ICS address (`webcal://` works too).
    Calendar { url: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Source {
    pub kind: Kind,
    pub every: Duration,
}

impl Source {
    /// The key of its value in the engine (`live:<hash>`).
    pub fn key(&self) -> String {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.kind.hash(&mut h);
        format!("live:{:016x}", h.finish())
    }
}

/// The source a binding's params describe; None when it is not set up yet. `ha_base`: Home
/// Assistant's address from the settings.
pub fn source(params: &Value, ha_base: &str) -> Option<Source> {
    let text = |k: &str| param_str(params, k).trim().to_string();
    let every = Duration::from_secs_f64(param_f64(params, "every", 60.0).max(MIN_EVERY.as_secs_f64()));
    let every = match param_str(params, "source") {
        "weather" => every.max(WEATHER_EVERY),
        "calendar" => every.max(CALENDAR_EVERY),
        _ => every,
    };
    let kind = match param_str(params, "source") {
        "ha" => Kind::Ha { entity: Some(text("entity")).filter(|e| e.contains('.'))?, base: ha_base.trim_end_matches('/').to_string() },
        "command" => Kind::Command { command: Some(text("command")).filter(|c| !c.is_empty())? },
        "calendar" => Kind::Calendar { url: Some(text("url").replacen("webcal://", "https://", 1)).filter(|u| u.starts_with("http://") || u.starts_with("https://"))? },
        "weather" => Kind::Weather { place: Some(text("place")).filter(|p| !p.is_empty())?, fahrenheit: param_str(params, "units") == "imperial" },
        _ => Kind::Url { url: Some(text("url")).filter(|u| u.starts_with("http://") || u.starts_with("https://"))?, path: text("path"), secret: text("secret") },
    };
    Some(Source { kind, every })
}

// ---------- what the worker reports ----------

/// One reading, as the engine stores it (JSON text in `IntegrationValue`).
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Reading {
    /// The value as text (a number keeps its own digits).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    /// The value as a number, when it is one: formatted with the binding's decimals, compared
    /// with its thresholds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num: Option<f64>,
    /// A second line the source suggests (a unit, a place, an event's title).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub sub: String,
    /// Draw the key in the warning color (an event about to start pulses with it).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub attention: bool,
    /// What went wrong; the key shows a short form.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub error: String,
    /// A command that waits for the user's yes (a press allows it).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub locked: bool,
    /// Shown after the number when the binding has no suffix of its own (`°`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub unit: String,
    /// Decimals when the binding leaves them automatic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub places: Option<u8>,
    /// The weather's sky and the hours and days ahead (the dial's weather scene).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forecast: Option<Forecast>,
}

/// The weather for the dial's scene: now, the next hours and the next days (Open-Meteo).
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Forecast {
    pub place: String,
    pub code: i64,
    pub day: bool,
    pub lat: f64,
    pub lon: f64,
    /// The next hours from now: ("14:00", temperature, code).
    pub hours: Vec<(String, f64, i64)>,
    /// Today and the next days: (date `2026-10-14`, low, high, code).
    pub days: Vec<(String, f64, f64, i64)>,
}

impl Reading {
    fn error(e: impl Into<String>) -> Self {
        Self { error: e.into(), ..Self::default() }
    }

    /// A value from text: a number when it reads as one (`42`, `-3.5`, `21.4 °C` -> 21.4).
    pub fn from_text(text: &str) -> Self {
        let line = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
        let line: String = line.chars().take(200).collect();
        let lead: String = line.chars().take_while(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '+')).collect();
        // a short unit may follow (`%`, `°C`, `ms`); anything longer is text
        let rest = line[lead.len()..].trim();
        let num = lead.parse::<f64>().ok().filter(|n| n.is_finite() && rest.chars().count() <= 4);
        Self { num, text: line, ..Self::default() }
    }
}

/// Pick `path` (`a.b.0.c`, `a.b[0].c`) out of JSON.
pub fn pick<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    let path = path.replace('[', ".").replace(']', "");
    path.split('.').filter(|s| !s.is_empty()).try_fold(v, |v, seg| match v {
        Value::Array(a) => seg.parse::<usize>().ok().and_then(|i| a.get(i)),
        Value::Object(o) => o.get(seg),
        _ => None,
    })
}

/// A reading from a web answer: JSON with `path`, or plain text.
pub fn reading_from_body(body: &str, path: &str) -> Reading {
    match serde_json::from_str::<Value>(body) {
        Ok(json) => match pick(&json, path) {
            Some(Value::Number(n)) => Reading { num: n.as_f64(), text: n.to_string(), ..Reading::default() },
            Some(Value::String(s)) => Reading::from_text(s),
            Some(Value::Bool(b)) => Reading { text: if *b { "on".into() } else { "off".into() }, num: Some(if *b { 1.0 } else { 0.0 }), ..Reading::default() },
            Some(Value::Null) | None if !path.trim().is_empty() => Reading::error(format!("no {path} in the answer")),
            Some(other) => Reading::from_text(&other.to_string()),
            None => Reading::from_text(&json.to_string()),
        },
        Err(_) => Reading::from_text(body),
    }
}

// ---------- drawing ----------

const WARN_BG: &str = "#8a5a00";
const ALERT_BG: &str = "#9b1c1c";
const ERROR_BG: &str = "#3a3a40";

/// How a threshold param reads: empty = none.
fn threshold(params: &Value, key: &str) -> Option<f64> {
    match params.get(key) {
        Some(Value::Number(n)) => n.as_f64(),
        Some(Value::String(s)) => s.trim().parse().ok(),
        _ => None,
    }
}

/// 0 = normal, 1 = warning, 2 = alert, by the binding's thresholds (`below`: lower is worse).
pub fn level(params: &Value, num: Option<f64>) -> u8 {
    let Some(n) = num else { return 0 };
    let below = params.get("below").and_then(Value::as_bool).unwrap_or(false) || param_str(params, "below") == "true";
    let crossed = |t: Option<f64>| t.is_some_and(|t| if below { n <= t } else { n >= t });
    if crossed(threshold(params, "alert")) {
        2
    } else if crossed(threshold(params, "warn")) {
        1
    } else {
        0
    }
}

/// The value as the binding wants it: prefix, the number with its decimals (or the text), suffix.
pub fn format_value(params: &Value, r: &Reading) -> String {
    let decimals = match (param_f64(params, "decimals", -1.0), r.places) {
        (d, Some(p)) if d < 0.0 => p as f64,
        (d, _) => d,
    };
    let value = match r.num {
        Some(n) if decimals >= 0.0 => format!("{n:.*}", decimals.min(6.0) as usize),
        Some(n) if n.fract() == 0.0 && n.abs() < 1e15 => format!("{}", n as i64),
        Some(n) => {
            let s = format!("{n:.2}");
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        }
        None => r.text.clone(),
    };
    let suffix = param_str(params, "suffix");
    let suffix = if suffix.is_empty() && r.num.is_some() { r.unit.as_str() } else { suffix };
    format!("{}{value}{suffix}", param_str(params, "prefix"))
}

/// What a live control shows: the main text, a second line, and its colors.
#[derive(Debug, Clone, PartialEq)]
pub struct Face {
    pub main: String,
    pub second: Option<String>,
    pub bg: Option<String>,
    /// 0..=1 for the dial's ring (only with a maximum).
    pub ring: Option<f32>,
}

/// The face of a live control from its latest reading (`raw`: the JSON the worker sent).
pub fn face(params: &Value, look: &Look, raw: Option<&str>) -> Face {
    let caption = look.label.clone().filter(|l| !l.trim().is_empty());
    // a field the user still has to fill in (a preset's place, calendar or address)
    if source(params, &HA_BASE.lock().unwrap_or_else(|e| e.into_inner())).is_none() {
        let what = match param_str(params, "source") {
            "weather" => "Weather",
            "calendar" => "Calendar",
            "ha" => "Sensor",
            "command" => "Command",
            _ => "Address",
        };
        return Face { main: "Set up".into(), second: caption.or(Some(what.into())), bg: None, ring: None };
    }
    let Some(r) = raw.and_then(|s| serde_json::from_str::<Reading>(s).ok()) else {
        return Face { main: "…".into(), second: caption, bg: None, ring: None };
    };
    if r.locked {
        return Face { main: "Allow?".into(), second: Some("press".into()), bg: Some(WARN_BG.into()), ring: None };
    }
    if !r.error.is_empty() {
        log::debug!("live value: {}", r.error);
        return Face { main: "–".into(), second: caption.or(Some("offline".into())), bg: Some(ERROR_BG.into()), ring: None };
    }
    let bg = match (level(params, r.num), r.attention) {
        (2, _) => Some(ALERT_BG.to_string()),
        (1, _) | (_, true) => Some(WARN_BG.to_string()),
        _ => None,
    };
    let max = param_f64(params, "max", 0.0);
    let ring = r.num.filter(|_| max > 0.0).map(|n| (n / max).clamp(0.0, 1.0) as f32);
    let second = caption.or_else(|| Some(r.sub.clone()).filter(|s| !s.is_empty()));
    Face { main: format_value(params, &r), second, bg, ring }
}

/// A key's look for a live value: big value, small caption, the threshold color.
pub fn key_look(params: &Value, look: Look, raw: Option<&str>) -> Look {
    let f = face(params, &look, raw);
    let label = match f.second {
        Some(s) => format!("{}\n{s}", f.main),
        None => f.main,
    };
    Look { label: Some(label), icon: None, image: None, face: true, bg: f.bg.or(look.bg.clone()), ..look }
}

// ---------- the worker ----------

enum Cmd {
    Watch(Vec<Source>),
    /// Fetch this source again now (a press).
    Refresh(String),
}

static ENGINE: OnceLock<Sender<Msg>> = OnceLock::new();
static WORKER: OnceLock<Sender<Cmd>> = OnceLock::new();
/// Home Assistant's address as last shown (a press finds its source with it).
static HA_BASE: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

/// Where readings go; no thread starts until a live value is shown.
pub fn init(engine: Sender<Msg>) {
    let _ = ENGINE.set(engine);
}

/// The sources on screen now (empty: nothing is fetched).
pub fn watch(sources: Vec<Source>) {
    let mut sources = sources;
    sources.sort_by_key(Source::key);
    sources.dedup();
    if sources.is_empty() && WORKER.get().is_none() {
        return;
    }
    let worker = WORKER.get_or_init(|| {
        let (tx, rx) = unbounded();
        let engine = ENGINE.get().cloned();
        std::thread::Builder::new().name("live-values".into()).spawn(move || run(engine, rx)).expect("spawn live values thread");
        tx
    });
    let _ = worker.send(Cmd::Watch(sources));
}

/// A press on a live control: allow its command if it waits for that, refresh it, or open its
/// page. Returns the caption to show on the dial.
pub fn press(params: &Value) -> Result<Option<String>, String> {
    let open = param_str(params, "open").trim();
    let ha_base = HA_BASE.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if let Some(src) = source(params, &ha_base) {
        if let Kind::Command { command } = &src.kind
            && !allowed(command)
        {
            allow(command)?;
            refresh(&src.key());
            return Ok(Some("Allowed".into()));
        }
        if open.is_empty() {
            refresh(&src.key());
            return Ok(Some("Refreshing".into()));
        }
    }
    if !open.is_empty() {
        crate::system::shell_open(open)?;
    }
    Ok(None)
}

/// Save the token a live value sends to its web address (`name` is the binding's `secret`
/// param); an empty token deletes it. Tokens stay in the system's credential store.
#[tauri::command]
pub fn live_secret_set(name: String, token: String) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() || name.len() > 64 || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err("the token name may use letters, digits, - and _".into());
    }
    super::secrets::set(&format!("{SECRET_PREFIX}{name}"), token.trim())
}

/// Whether a token with this name is saved.
#[tauri::command]
pub fn live_secret_saved(name: String) -> bool {
    !name.trim().is_empty() && super::secrets::get(&format!("{SECRET_PREFIX}{}", name.trim())).is_some()
}

fn refresh(key: &str) {
    if let Some(w) = WORKER.get() {
        let _ = w.send(Cmd::Refresh(key.to_string()));
    }
}

/// Commands the user allowed on this computer (a press on the key), in `<data>/live-commands.json`.
fn allowed_file() -> Option<std::path::PathBuf> {
    super::agents::data_dir().map(|d| d.join("live-commands.json"))
}

fn allowed_list() -> Vec<String> {
    allowed_file().and_then(|f| std::fs::read(f).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub fn allowed(command: &str) -> bool {
    allowed_list().iter().any(|c| c == command)
}

fn allow(command: &str) -> Result<(), String> {
    let mut list = allowed_list();
    if !list.iter().any(|c| c == command) {
        list.push(command.to_string());
    }
    let file = allowed_file().ok_or("no app data folder")?;
    super::profile::write_atomic(&file, &serde_json::to_vec_pretty(&list).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

fn run(engine: Option<Sender<Msg>>, rx: Receiver<Cmd>) {
    let agent: ureq::Agent = crate::engine::calls::builder().timeout_global(Some(TIMEOUT)).http_status_as_error(false).build().into();
    let mut watched: Vec<Source> = Vec::new();
    let mut due: HashMap<String, Instant> = HashMap::new();
    let mut last: HashMap<String, String> = HashMap::new();
    // calendars: fetched every `every`, the time until the next event counted between fetches
    let mut calendars: HashMap<String, (Instant, Result<Vec<Event>, String>)> = HashMap::new();
    loop {
        // nothing on screen: sleep until the engine says otherwise
        let wait = due.values().min().map(|t| t.saturating_duration_since(Instant::now()).min(Duration::from_secs(60))).unwrap_or(Duration::from_secs(86_400));
        match rx.recv_timeout(wait) {
            Ok(Cmd::Watch(list)) => {
                let keys: Vec<String> = list.iter().map(Source::key).collect();
                due.retain(|k, _| keys.contains(k));
                for k in &keys {
                    // new on screen: now; already shown: keep its rhythm
                    due.entry(k.clone()).or_insert_with(Instant::now);
                }
                watched = list;
            }
            Ok(Cmd::Refresh(key)) => {
                calendars.remove(&key);
                if due.contains_key(&key) {
                    due.insert(key, Instant::now());
                }
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => return,
        }
        let now = Instant::now();
        for src in &watched {
            let key = src.key();
            if due.get(&key).is_none_or(|t| *t > now) {
                continue;
            }
            let reading = match &src.kind {
                Kind::Calendar { url } => {
                    if calendars.get(&key).is_none_or(|(at, _)| at.elapsed() >= src.every) {
                        calendars.insert(key.clone(), (Instant::now(), fetch_calendar(&agent, url)));
                    }
                    let (reading, next) = match &calendars[&key].1 {
                        Ok(events) => calendar_reading(events, chrono::Local::now().naive_local()),
                        Err(e) => (Reading::error(e.clone()), src.every),
                    };
                    due.insert(key.clone(), now + next);
                    reading
                }
                kind => {
                    due.insert(key.clone(), now + src.every);
                    fetch(&agent, kind)
                }
            };
            let text = serde_json::to_string(&reading).unwrap_or_default();
            if last.get(&key) != Some(&text) {
                last.insert(key.clone(), text.clone());
                if let Some(engine) = &engine {
                    let _ = engine.send(Msg::IntegrationValue { key, value: text });
                }
            }
        }
    }
}

fn fetch(agent: &ureq::Agent, kind: &Kind) -> Reading {
    match kind {
        Kind::Url { url, path, secret } => {
            let mut req = agent.get(url).header("User-Agent", concat!("HubDeck/", env!("CARGO_PKG_VERSION")));
            if !secret.is_empty() {
                match super::secrets::get(&format!("{SECRET_PREFIX}{secret}")) {
                    Some(token) => req = req.header("Authorization", format!("Bearer {token}")),
                    None => return Reading::error("the token is not saved on this computer"),
                }
            }
            match req.call() {
                Ok(mut res) if res.status().is_success() => match res.body_mut().with_config().limit(MAX_BODY).read_to_string() {
                    Ok(body) => reading_from_body(&body, path),
                    Err(e) => Reading::error(e.to_string()),
                },
                Ok(res) => Reading::error(format!("{url} answered {}", res.status())),
                Err(e) => Reading::error(e.to_string()),
            }
        }
        Kind::Ha { entity, base } => {
            let Some(token) = super::secrets::get("homeassistant").filter(|_| !base.is_empty()) else {
                return Reading::error("set up Home Assistant in Settings > Connections");
            };
            match agent.get(format!("{base}/api/states/{entity}")).header("Authorization", format!("Bearer {token}")).call() {
                Ok(mut res) if res.status().is_success() => match res.body_mut().read_json::<Value>() {
                    Ok(v) => ha_reading(&v),
                    Err(e) => Reading::error(e.to_string()),
                },
                Ok(res) => Reading::error(format!("Home Assistant answered {} for {entity}", res.status())),
                Err(e) => Reading::error(e.to_string()),
            }
        }
        Kind::Weather { place, fahrenheit } => weather(agent, place, *fahrenheit),
        Kind::Calendar { .. } => Reading::error("calendars are read by the worker"),
        Kind::Command { command } => {
            if !allowed(command) {
                return Reading { locked: true, ..Reading::default() };
            }
            match run_command(command) {
                Ok(out) => Reading::from_text(&out),
                Err(e) => Reading::error(e),
            }
        }
    }
}

// ---------- weather (Open-Meteo, no key) ----------

/// Weather is fetched at most this often (Open-Meteo updates every 15 minutes).
pub const WEATHER_EVERY: Duration = Duration::from_secs(600);
const GEOCODE: &str = "https://geocoding-api.open-meteo.com/v1/search";
const FORECAST: &str = "https://api.open-meteo.com/v1/forecast";

/// A place found by the city search.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Place {
    pub name: String,
    /// Region and country, to tell places of the same name apart.
    pub detail: String,
    pub lat: f64,
    pub lon: f64,
    /// The param value to store: `Name|lat|lon`.
    pub param: String,
}

fn encode(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

/// Places from Open-Meteo's geocoding answer.
pub fn parse_places(v: &Value) -> Vec<Place> {
    v["results"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| {
            let detail = [r["admin1"].as_str(), r["country"].as_str()].into_iter().flatten().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(", ");
            let (name, lat, lon) = (r["name"].as_str()?.to_string(), r["latitude"].as_f64()?, r["longitude"].as_f64()?);
            Some(Place { param: format!("{name}|{lat:.4}|{lon:.4}"), name, detail, lat, lon })
        })
        .collect()
}

fn search_places(agent: &ureq::Agent, name: &str) -> Result<Vec<Place>, String> {
    let url = format!("{GEOCODE}?name={}&count=6&language=en&format=json", encode(name.trim()));
    let mut res = agent.get(&url).call().map_err(|e| e.to_string())?;
    let v: Value = res.body_mut().read_json().map_err(|e| e.to_string())?;
    Ok(parse_places(&v))
}

/// The city search of the weather param.
#[tauri::command]
pub async fn weather_search(name: String) -> Result<Vec<Place>, String> {
    if name.trim().len() < 2 {
        return Ok(Vec::new());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let agent: ureq::Agent = crate::engine::calls::builder().timeout_global(Some(TIMEOUT)).build().into();
        search_places(&agent, &name)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// `Name|lat|lon` -> (name, lat, lon); a bare name has no coordinates.
pub fn parse_place(place: &str) -> (String, Option<(f64, f64)>) {
    let mut parts = place.split('|');
    let name = parts.next().unwrap_or("").trim().to_string();
    let lat = parts.next().and_then(|s| s.trim().parse::<f64>().ok());
    let lon = parts.next().and_then(|s| s.trim().parse::<f64>().ok());
    (name, lat.zip(lon))
}

/// A short word for a WMO weather code (Open-Meteo's `weather_code`), short enough for a key.
pub fn condition(code: i64) -> &'static str {
    match code {
        0 => "Clear",
        1 => "Fair",
        2 => "Some sun",
        3 => "Cloudy",
        45 | 48 => "Fog",
        51..=57 => "Drizzle",
        61..=65 | 80..=82 => "Rain",
        66 | 67 => "Ice rain",
        71..=77 | 85 | 86 => "Snow",
        95..=99 => "Storm",
        _ => "",
    }
}

/// The same in Polish (the dial's weather scene).
pub fn condition_pl(code: i64) -> &'static str {
    match code {
        0 => "Bezchmurnie",
        1 => "Pogodnie",
        2 => "Trochę słońca",
        3 => "Pochmurno",
        45 | 48 => "Mgła",
        51..=57 => "Mżawka",
        61..=65 | 80..=82 => "Deszcz",
        66 | 67 => "Marznący deszcz",
        71..=77 | 85 | 86 => "Śnieg",
        95..=99 => "Burza",
        _ => "",
    }
}

/// A reading from Open-Meteo's answer: `current`, and `hourly` / `daily` when asked for.
pub fn weather_reading(v: &Value, place: &str) -> Reading {
    let Some(t) = v["current"]["temperature_2m"].as_f64() else {
        return Reading::error(v["reason"].as_str().unwrap_or("no weather in the answer").to_string());
    };
    let code = v["current"]["weather_code"].as_i64().unwrap_or(-1);
    let word = condition(code);
    let sub = if word.is_empty() { place.to_string() } else { word.to_string() };
    // the hours from the current one on (the answer starts at midnight)
    let now = v["current"]["time"].as_str().unwrap_or("");
    let times = v["hourly"]["time"].as_array().cloned().unwrap_or_default();
    let start = times.iter().position(|x| x.as_str().is_some_and(|x| x.get(..13) >= now.get(..13))).unwrap_or(0);
    let hours = times
        .iter()
        .enumerate()
        .skip(start)
        .take(24)
        .filter_map(|(i, x)| Some((x.as_str()?.get(11..16)?.to_string(), v["hourly"]["temperature_2m"][i].as_f64()?, v["hourly"]["weather_code"][i].as_i64().unwrap_or(-1))))
        .collect();
    let days = v["daily"]["time"]
        .as_array()
        .map(|d| {
            d.iter()
                .enumerate()
                .filter_map(|(i, x)| {
                    Some((x.as_str()?.to_string(), v["daily"]["temperature_2m_min"][i].as_f64()?, v["daily"]["temperature_2m_max"][i].as_f64()?, v["daily"]["weather_code"][i].as_i64().unwrap_or(-1)))
                })
                .collect()
        })
        .unwrap_or_default();
    let forecast = Forecast {
        place: place.to_string(),
        code,
        day: v["current"]["is_day"].as_i64().is_none_or(|d| d == 1),
        lat: v["latitude"].as_f64().unwrap_or_default(),
        lon: v["longitude"].as_f64().unwrap_or_default(),
        hours,
        days,
    };
    Reading { num: Some(t), text: format!("{t}"), sub, unit: "°".into(), places: Some(0), forecast: Some(forecast), ..Reading::default() }
}

/// The city of this computer's time zone (`Europe/Warsaw` -> `Warsaw`), for a weather place
/// set to `auto` (the dial gallery and the tour have no city to ask for).
fn local_city() -> Option<String> {
    let zone = iana_time_zone::get_timezone().ok()?;
    let city = zone.rsplit('/').next()?.replace('_', " ");
    (zone.contains('/') && !city.is_empty()).then_some(city)
}

/// Coordinates of places named without them, looked up once per run.
static PLACES: std::sync::Mutex<Vec<(String, (f64, f64))>> = std::sync::Mutex::new(Vec::new());

fn weather(agent: &ureq::Agent, place: &str, fahrenheit: bool) -> Reading {
    let auto = place.trim().eq_ignore_ascii_case("auto");
    let local = if auto { local_city() } else { None };
    if auto && local.is_none() {
        return Reading::error("set the city (this computer's time zone names none)");
    }
    let (name, coords) = parse_place(local.as_deref().unwrap_or(place));
    let coords = match coords {
        Some(c) => c,
        None => {
            let known = PLACES.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|(n, _)| *n == name).map(|(_, c)| *c);
            match known {
                Some(c) => c,
                None => match search_places(agent, &name) {
                    Ok(found) if !found.is_empty() => {
                        let c = (found[0].lat, found[0].lon);
                        PLACES.lock().unwrap_or_else(|e| e.into_inner()).push((name.clone(), c));
                        c
                    }
                    Ok(_) => return Reading::error(format!("no place called {name}")),
                    Err(e) => return Reading::error(e),
                },
            }
        }
    };
    let unit = if fahrenheit { "&temperature_unit=fahrenheit" } else { "" };
    let url = format!(
        "{FORECAST}?latitude={:.4}&longitude={:.4}&current=temperature_2m,weather_code,is_day&hourly=temperature_2m,weather_code&daily=weather_code,temperature_2m_max,temperature_2m_min&forecast_days=3&timezone=auto{unit}",
        coords.0, coords.1
    );
    match agent.get(&url).call() {
        Ok(mut res) => match res.body_mut().read_json::<Value>() {
            Ok(v) => weather_reading(&v, &name),
            Err(e) => Reading::error(e.to_string()),
        },
        Err(e) => Reading::error(e.to_string()),
    }
}

// ---------- calendar (ICS) ----------

/// A calendar is downloaded at most this often; the time until its next event is counted between.
pub const CALENDAR_EVERY: Duration = Duration::from_secs(300);
/// The key pulses this long before an event starts.
pub const SOON: chrono::Duration = chrono::Duration::minutes(5);
/// How far ahead recurring events are expanded.
const AHEAD_DAYS: i64 = 8;
/// Largest calendar file read.
const MAX_ICS: u64 = 8 * 1024 * 1024;

/// One occurrence of an event, in local time.
#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub start: chrono::NaiveDateTime,
    pub end: chrono::NaiveDateTime,
    pub title: String,
}

fn fetch_calendar(agent: &ureq::Agent, url: &str) -> Result<Vec<Event>, String> {
    let mut res = agent.get(url).call().map_err(|e| e.to_string())?;
    if !res.status().is_success() {
        return Err(format!("the calendar address answered {}", res.status()));
    }
    let text = res.body_mut().with_config().limit(MAX_ICS).read_to_string().map_err(|e| e.to_string())?;
    if !text.contains("BEGIN:VCALENDAR") {
        return Err("not a calendar (ICS) file".into());
    }
    Ok(parse_ics(&text, chrono::Local::now().naive_local()))
}

/// A date-time of an ICS property: UTC (`...Z`) is turned into local time; a time with a TZID or
/// none is taken as local; a date alone is None (all-day events are not "next meetings").
fn ics_time(value: &str) -> Option<chrono::NaiveDateTime> {
    let value = value.trim();
    if value.len() == 8 {
        return None;
    }
    let utc = value.ends_with('Z');
    let t = chrono::NaiveDateTime::parse_from_str(value.trim_end_matches('Z'), "%Y%m%dT%H%M%S").ok()?;
    Some(if utc { chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(t, chrono::Utc).with_timezone(&chrono::Local).naive_local() } else { t })
}

/// `PT1H30M`, `P1D`, `PT45M` -> a duration.
fn ics_duration(value: &str) -> Option<chrono::Duration> {
    let v = value.trim().trim_start_matches('+').strip_prefix('P')?;
    let (mut total, mut num) = (0i64, String::new());
    for c in v.chars() {
        match c {
            '0'..='9' => num.push(c),
            'T' => {}
            'W' | 'D' | 'H' | 'M' | 'S' => {
                let n: i64 = num.parse().ok()?;
                num.clear();
                total += n * match c {
                    'W' => 604_800,
                    'D' => 86_400,
                    'H' => 3600,
                    'M' => 60,
                    _ => 1,
                };
            }
            _ => return None,
        }
    }
    Some(chrono::Duration::seconds(total))
}

/// Occurrences of the events of an ICS file from a day before `now` to [`AHEAD_DAYS`] after:
/// single events, and repeating ones (`RRULE` daily, weekly with days, monthly, yearly, with
/// `INTERVAL`, `COUNT`, `UNTIL`; `EXDATE` and moved occurrences left out). Cancelled ones are
/// skipped.
pub fn parse_ics(text: &str, now: chrono::NaiveDateTime) -> Vec<Event> {
    let unfolded = text.replace("\r\n ", "").replace("\r\n\t", "").replace("\n ", "").replace("\n\t", "");
    let (from, to) = (now - chrono::Duration::days(1), now + chrono::Duration::days(AHEAD_DAYS));
    struct Raw {
        uid: String,
        start: Option<chrono::NaiveDateTime>,
        end: Option<chrono::NaiveDateTime>,
        duration: Option<chrono::Duration>,
        title: String,
        rrule: String,
        exdates: Vec<chrono::NaiveDateTime>,
        recurrence_id: Option<chrono::NaiveDateTime>,
        cancelled: bool,
    }
    let mut raws: Vec<Raw> = Vec::new();
    let mut cur: Option<Raw> = None;
    for line in unfolded.lines() {
        let line = line.trim_end_matches('\r');
        match line {
            "BEGIN:VEVENT" => {
                cur = Some(Raw { uid: String::new(), start: None, end: None, duration: None, title: String::new(), rrule: String::new(), exdates: Vec::new(), recurrence_id: None, cancelled: false })
            }
            "END:VEVENT" => raws.extend(cur.take()),
            _ => {
                let Some(ev) = cur.as_mut() else { continue };
                let Some((head, value)) = line.split_once(':') else { continue };
                let name = head.split(';').next().unwrap_or("");
                match name {
                    "UID" => ev.uid = value.to_string(),
                    "DTSTART" => ev.start = ics_time(value),
                    "DTEND" => ev.end = ics_time(value),
                    "DURATION" => ev.duration = ics_duration(value),
                    "SUMMARY" => ev.title = value.replace("\\,", ",").replace("\\;", ";").replace("\\n", " ").replace("\\\\", "\\"),
                    "RRULE" => ev.rrule = value.to_string(),
                    "EXDATE" => ev.exdates.extend(value.split(',').filter_map(ics_time)),
                    "RECURRENCE-ID" => ev.recurrence_id = ics_time(value),
                    "STATUS" => ev.cancelled = value.eq_ignore_ascii_case("CANCELLED"),
                    _ => {}
                }
            }
        }
    }
    // occurrences moved or cancelled on their own: left out of their series
    let moved: Vec<(String, chrono::NaiveDateTime)> = raws.iter().filter_map(|r| r.recurrence_id.map(|t| (r.uid.clone(), t))).collect();
    let mut out = Vec::new();
    for r in raws.iter().filter(|r| !r.cancelled) {
        let Some(start) = r.start else { continue };
        let length = r.end.map(|e| e - start).or(r.duration).unwrap_or_else(chrono::Duration::zero);
        let mut push = |s: chrono::NaiveDateTime| {
            if s + length >= from && s <= to {
                out.push(Event { start: s, end: s + length, title: r.title.clone() });
            }
        };
        if r.rrule.is_empty() || r.recurrence_id.is_some() {
            push(start);
            continue;
        }
        for s in occurrences(start, &r.rrule, to) {
            if !r.exdates.contains(&s) && !moved.iter().any(|(uid, t)| *uid == r.uid && *t == s) {
                push(s);
            }
        }
    }
    out.sort_by_key(|e| e.start);
    out
}

/// Starts of a repeating event up to `to`.
fn occurrences(start: chrono::NaiveDateTime, rrule: &str, to: chrono::NaiveDateTime) -> Vec<chrono::NaiveDateTime> {
    use chrono::{Datelike, Months};
    let part = |k: &str| rrule.split(';').find_map(|p| p.strip_prefix(&format!("{k}="))).unwrap_or("");
    let interval = part("INTERVAL").parse::<u32>().unwrap_or(1).max(1);
    let count = part("COUNT").parse::<usize>().ok();
    let until = ics_time(part("UNTIL")).or_else(|| chrono::NaiveDate::parse_from_str(part("UNTIL"), "%Y%m%d").ok().and_then(|d| d.and_hms_opt(23, 59, 59)));
    let end = until.map_or(to, |u| u.min(to));
    let days: Vec<chrono::Weekday> = part("BYDAY")
        .split(',')
        .filter_map(|d| match d.trim_start_matches(|c: char| c == '-' || c == '+' || c.is_ascii_digit()) {
            "MO" => Some(chrono::Weekday::Mon),
            "TU" => Some(chrono::Weekday::Tue),
            "WE" => Some(chrono::Weekday::Wed),
            "TH" => Some(chrono::Weekday::Thu),
            "FR" => Some(chrono::Weekday::Fri),
            "SA" => Some(chrono::Weekday::Sat),
            "SU" => Some(chrono::Weekday::Sun),
            _ => None,
        })
        .collect();
    let mut out = Vec::new();
    let mut n = 0usize;
    let done = |n: usize, s: chrono::NaiveDateTime| count.is_some_and(|c| n >= c) || s > end;
    match part("FREQ") {
        "DAILY" => {
            let mut s = start;
            while !done(n, s) && out.len() < 5000 {
                out.push(s);
                n += 1;
                s += chrono::Duration::days(interval as i64);
            }
        }
        "WEEKLY" => {
            let days = if days.is_empty() { vec![start.weekday()] } else { days };
            // the Monday of the first week, then every `interval` weeks
            let mut week = start - chrono::Duration::days(start.weekday().num_days_from_monday() as i64);
            'weeks: while week <= end && out.len() < 5000 {
                for d in 0..7 {
                    let s = week + chrono::Duration::days(d);
                    if s < start || !days.contains(&s.weekday()) {
                        continue;
                    }
                    if done(n, s) {
                        break 'weeks;
                    }
                    out.push(s);
                    n += 1;
                }
                week += chrono::Duration::weeks(interval as i64);
            }
        }
        "MONTHLY" | "YEARLY" => {
            let step = if part("FREQ") == "MONTHLY" { interval } else { interval * 12 };
            let mut k = 0u32;
            while out.len() < 5000 {
                let Some(s) = start.checked_add_months(Months::new(k * step)) else { break };
                if done(n, s) {
                    break;
                }
                // the 31st in a month without one: that month has no occurrence
                if s.day() == start.day() {
                    out.push(s);
                    n += 1;
                }
                k += 1;
            }
        }
        _ => out.push(start),
    }
    out
}

/// What the calendar key shows at `now`, and when to look again (every second while an event is
/// about to start, so the key pulses).
pub fn calendar_reading(events: &[Event], now: chrono::NaiveDateTime) -> (Reading, Duration) {
    let title = |e: &Event| e.title.chars().take(16).collect::<String>();
    let calm = Duration::from_secs(20);
    if let Some(e) = events.iter().find(|e| e.start <= now && now < e.end) {
        let left = (e.end - now).num_minutes();
        return (Reading { text: "Now".into(), sub: title(e), num: Some(0.0), ..Reading::default() }, calm.min(Duration::from_secs(((left.max(0) + 1) * 60) as u64)));
    }
    let Some(e) = events.iter().find(|e| e.start > now) else {
        return (Reading { text: "Free".into(), sub: "no events".into(), ..Reading::default() }, calm);
    };
    let until = e.start - now;
    let minutes = until.num_minutes();
    let text = if minutes < 1 {
        "Now".to_string()
    } else if minutes < 60 {
        format!("{minutes} min")
    } else if minutes < 12 * 60 && e.start.date() == now.date() {
        format!("{} h", (minutes as f64 / 60.0).round())
    } else if e.start.date() == now.date() {
        e.start.format("%H:%M").to_string()
    } else if e.start.date() == now.date() + chrono::Duration::days(1) {
        e.start.format("Tmrw").to_string()
    } else {
        e.start.format("%a").to_string()
    };
    let soon = until <= SOON;
    let reading = Reading {
        text,
        sub: title(e),
        num: Some(minutes as f64),
        // the key blinks in the warning color, once a second
        attention: soon && now.and_utc().timestamp() % 2 == 0,
        ..Reading::default()
    };
    (reading, if soon { Duration::from_secs(1) } else { calm })
}

/// A Home Assistant state: the value, its unit as the second line.
pub fn ha_reading(v: &Value) -> Reading {
    let state = v["state"].as_str().unwrap_or("");
    if matches!(state, "unavailable" | "unknown" | "") {
        return Reading::error(format!("{} is {state}", v["entity_id"].as_str().unwrap_or("the entity")));
    }
    let mut r = Reading::from_text(state);
    r.sub = v["attributes"]["unit_of_measurement"].as_str().unwrap_or("").to_string();
    r
}

/// Run a command line through the shell; its output (stdout), at most 10 s.
fn run_command(command: &str) -> Result<String, String> {
    #[cfg(windows)]
    let mut cmd = {
        use std::os::windows::process::CommandExt;
        let mut c = std::process::Command::new("cmd.exe");
        c.arg("/C").raw_arg(command);
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        c
    };
    #[cfg(not(windows))]
    let mut cmd = {
        let mut c = std::process::Command::new("sh");
        c.args(["-c", command]);
        c
    };
    let mut child = cmd.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null()).spawn().map_err(|e| e.to_string())?;
    let started = Instant::now();
    loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(_) => break,
            None if started.elapsed() > Duration::from_secs(10) => {
                let _ = child.kill();
                return Err("the command took longer than 10 s".into());
            }
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    }
    let mut out = String::new();
    if let Some(mut stdout) = child.stdout.take() {
        let _ = std::io::Read::read_to_string(&mut std::io::Read::take(&mut stdout, 64 * 1024), &mut out);
    }
    Ok(out)
}

/// The key under which a binding's latest reading is stored (None: not set up).
pub fn value_key(params: &Value) -> Option<String> {
    source(params, &HA_BASE.lock().unwrap_or_else(|e| e.into_inner())).map(|s| s.key())
}

/// The engine shows `profile` at `page`: fetch its live values (nothing when it has none).
pub fn show(profile: Option<&super::profile::Profile>, page: Option<&str>, ha_base: &str) {
    *HA_BASE.lock().unwrap_or_else(|e| e.into_inner()) = ha_base.to_string();
    watch(profile.map(|p| shown_sources(p, page, ha_base)).unwrap_or_default());
}

/// The sources of the live controls on screen: the keys of `page` and the dial.
pub fn shown_sources(profile: &super::profile::Profile, page: Option<&str>, ha_base: &str) -> Vec<Source> {
    (0..15)
        .map(|i| super::profile::control_id(page, &format!("keys.{i}")))
        .chain(std::iter::once("dial".to_string()))
        .filter_map(|c| profile.controls.get(&c))
        .filter(|b| b.action == ACTION)
        .filter_map(|b| source(&b.params, ha_base))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn values_are_picked_from_json_and_text() {
        let body = r#"{"data": {"items": [{"price": 41.5}, {"price": 7}], "name": "BTC", "ok": true}}"#;
        assert_eq!(reading_from_body(body, "data.items.0.price").num, Some(41.5));
        assert_eq!(reading_from_body(body, "data.items[1].price").num, Some(7.0));
        assert_eq!(reading_from_body(body, "data.name").text, "BTC");
        assert_eq!(reading_from_body(body, "data.ok").text, "on");
        assert!(!reading_from_body(body, "data.missing").error.is_empty());
        assert_eq!(reading_from_body("  23.5\nmore", "").num, Some(23.5));
        assert_eq!(reading_from_body("21.4 °C", "").num, Some(21.4));
        assert_eq!(reading_from_body("build passing", "").num, None);
        assert_eq!(reading_from_body("2026-10-12", "").num, None, "a date is not a number");
    }

    #[test]
    fn faces_format_and_color_by_thresholds() {
        let params = json!({"url": "https://x.example", "prefix": "", "suffix": "%", "decimals": 0, "warn": "60", "alert": 80, "max": 100});
        let r = |n: f64| serde_json::to_string(&Reading { num: Some(n), text: n.to_string(), ..Reading::default() }).unwrap();
        let look = Look { label: Some("CPU".into()), bg: Some("#101010".into()), ..Look::default() };
        let calm = face(&params, &look, Some(&r(42.4)));
        assert_eq!((calm.main.as_str(), calm.second.as_deref(), calm.bg), ("42%", Some("CPU"), None));
        assert_eq!(calm.ring, Some(0.424));
        assert_eq!(face(&params, &look, Some(&r(65.0))).bg.as_deref(), Some(WARN_BG));
        assert_eq!(face(&params, &look, Some(&r(95.0))).bg.as_deref(), Some(ALERT_BG));
        // lower is worse (battery)
        let battery = json!({"warn": 30, "alert": 10, "below": true});
        assert_eq!(level(&battery, Some(8.0)), 2);
        assert_eq!(level(&battery, Some(25.0)), 1);
        assert_eq!(level(&battery, Some(90.0)), 0);
        assert_eq!(level(&json!({"warn": ""}), Some(90.0)), 0, "an empty threshold is none");
        // automatic decimals, prefix, text values
        assert_eq!(format_value(&json!({"prefix": "$"}), &Reading { num: Some(1234.5), ..Reading::default() }), "$1234.5");
        assert_eq!(format_value(&json!({}), &Reading { num: Some(3.0), ..Reading::default() }), "3");
        assert_eq!(format_value(&json!({}), &Reading { text: "green".into(), ..Reading::default() }), "green");
        // waiting, errors, a command waiting for the user's yes
        assert_eq!(face(&json!({"source": "weather", "place": ""}), &Look::default(), None).main, "Set up", "no place yet");
        assert_eq!(face(&params, &look, None).main, "…", "waiting for the first reading");
        let err = serde_json::to_string(&Reading::error("timeout")).unwrap();
        assert_eq!(face(&params, &look, Some(&err)).bg.as_deref(), Some(ERROR_BG));
        let locked = serde_json::to_string(&Reading { locked: true, ..Reading::default() }).unwrap();
        assert_eq!(face(&params, &look, Some(&locked)).main, "Allow?");
        let key = key_look(&params, look.clone(), Some(&r(95.0)));
        assert!(key.face && key.icon.is_none());
        assert_eq!(key.label.as_deref(), Some("95%\nCPU"));
    }

    #[test]
    fn sources_come_from_params_and_hide_nothing_secret() {
        assert_eq!(source(&json!({"source": "url", "url": "ftp://x"}), ""), None, "web addresses only");
        let url = source(&json!({"source": "url", "url": "https://api.example.com/v", "path": "a.b", "every": 1}), "").unwrap();
        assert_eq!(url.every, MIN_EVERY, "never faster than 5 s");
        assert!(url.key().starts_with("live:"));
        assert_eq!(source(&json!({"source": "ha", "entity": "sensor.temp"}), "http://ha:8123/").unwrap().kind, Kind::Ha { entity: "sensor.temp".into(), base: "http://ha:8123".into() });
        assert_eq!(source(&json!({"source": "ha", "entity": "temp"}), "http://ha"), None);
        assert!(matches!(source(&json!({"source": "command", "command": "echo 5"}), "").unwrap().kind, Kind::Command { .. }));
        let r = ha_reading(&json!({"entity_id": "sensor.t", "state": "21.5", "attributes": {"unit_of_measurement": "°C"}}));
        assert_eq!((r.num, r.sub.as_str()), (Some(21.5), "°C"));
        assert!(!ha_reading(&json!({"entity_id": "sensor.t", "state": "unavailable"})).error.is_empty());
    }

    #[test]
    fn shown_sources_are_the_page_keys_and_the_dial() {
        let mut p = super::super::profile::Profile::new("Board");
        let live =
            |url: &str| super::super::profile::Binding { action: ACTION.into(), params: json!({"source": "url", "url": url}), look: Look::default(), accel: false, look_on: None, swap_turns: false };
        p.controls.insert("keys.0".into(), live("https://a.example"));
        p.controls.insert("dial".into(), live("https://b.example"));
        p.controls.insert("pg/keys.1".into(), live("https://c.example"));
        p.controls.insert("knobs.0".into(), live("https://d.example"));
        let main: Vec<String> = shown_sources(&p, None, "").iter().map(|s| format!("{:?}", s.kind)).collect();
        assert_eq!(main.len(), 2);
        assert!(main[0].contains("a.example") && main[1].contains("b.example"));
        assert!(format!("{:?}", shown_sources(&p, Some("pg"), "")).contains("c.example"));
    }

    #[test]
    fn weather_from_open_meteo() {
        let places = parse_places(&json!({"results": [
            {"name": "Warsaw", "latitude": 52.22977, "longitude": 21.01178, "country": "Poland", "admin1": "Masovia"},
            {"name": "Warsaw", "latitude": 41.2381, "longitude": -85.8530, "country": "United States", "admin1": "Indiana"},
            {"name": "Broken"}
        ]}));
        assert_eq!(places.len(), 2);
        assert_eq!(places[0].detail, "Masovia, Poland");
        assert_eq!(places[0].param, "Warsaw|52.2298|21.0118");
        assert_eq!(parse_place("Warsaw|52.2298|21.0118"), ("Warsaw".to_string(), Some((52.2298, 21.0118))));
        assert_eq!(parse_place("Kraków"), ("Kraków".to_string(), None));
        assert_eq!(encode("Kraków city"), "Krak%C3%B3w%20city");
        let r = weather_reading(&json!({"current": {"temperature_2m": 12.6, "weather_code": 61}}), "Warsaw");
        assert_eq!((r.num, r.sub.as_str()), (Some(12.6), "Rain"));
        assert_eq!(format_value(&json!({}), &r), "13°", "whole degrees with a degree sign by default");
        assert_eq!(format_value(&json!({"decimals": 1, "suffix": " C"}), &r), "12.6 C", "the binding's own format wins");
        assert!(!weather_reading(&json!({"error": true, "reason": "bad latitude"}), "x").error.is_empty());
        // the scene's hours start at the current one, the days come as they are
        let v = json!({
            "latitude": 50.06, "longitude": 19.94,
            "current": {"time": "2026-10-14T11:30", "temperature_2m": 8.4, "weather_code": 3, "is_day": 1},
            "hourly": {"time": ["2026-10-14T10:00", "2026-10-14T11:00", "2026-10-14T12:00"], "temperature_2m": [7.0, 8.0, 9.0], "weather_code": [3, 3, 61]},
            "daily": {"time": ["2026-10-14", "2026-10-15"], "weather_code": [51, 3], "temperature_2m_max": [11.0, 13.7], "temperature_2m_min": [4.3, 7.5]}
        });
        let f = weather_reading(&v, "Kraków").forecast.expect("a forecast");
        assert_eq!((f.code, f.day, f.place.as_str()), (3, true, "Kraków"));
        assert_eq!(f.hours, vec![("11:00".to_string(), 8.0, 3), ("12:00".to_string(), 9.0, 61)]);
        assert_eq!(f.days[1], ("2026-10-15".to_string(), 7.5, 13.7, 3));
        let src = source(&json!({"source": "weather", "place": "Warsaw|52.2|21.0", "every": 5}), "").unwrap();
        assert_eq!(src.every, WEATHER_EVERY, "weather is not fetched more often than every 10 minutes");
        assert_eq!(src.kind, Kind::Weather { place: "Warsaw|52.2|21.0".into(), fahrenheit: false });
        for code in [0, 1, 2, 3, 45, 51, 61, 66, 71, 80, 95] {
            assert!(condition(code).chars().count() <= 9, "{code} fits a key");
        }
    }

    fn at(s: &str) -> chrono::NaiveDateTime {
        chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
    }

    #[test]
    fn calendars_give_the_next_event() {
        let ics = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:a\r\nDTSTART;TZID=Europe/Warsaw:20261012T093000\r\nDTEND;TZID=Europe/Warsaw:20261012T100000\r\nSUMMARY:Standup\\, team\r\nRRULE:FREQ=WEEKLY;BYDAY=MO,WE,FR\r\nEXDATE;TZID=Europe/Warsaw:20261014T093000\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nUID:b\r\nDTSTART:20261012T140000\r\nDURATION:PT1H\r\nSUMMARY:Design re\r\n view\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nUID:c\r\nDTSTART;VALUE=DATE:20261012\r\nSUMMARY:Holiday\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nUID:d\r\nDTSTART:20261012T120000\r\nSUMMARY:Cancelled\r\nSTATUS:CANCELLED\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nUID:a\r\nRECURRENCE-ID;TZID=Europe/Warsaw:20261016T093000\r\nDTSTART;TZID=Europe/Warsaw:20261016T110000\r\nDTEND;TZID=Europe/Warsaw:20261016T113000\r\nSUMMARY:Standup moved\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let now = at("2026-10-12 09:00");
        let events = parse_ics(ics, now);
        let starts: Vec<String> = events.iter().map(|e| format!("{} {}", e.start.format("%a %H:%M"), e.title)).collect();
        assert_eq!(
            starts,
            ["Mon 09:30 Standup, team", "Mon 14:00 Design review", "Fri 11:00 Standup moved", "Mon 09:30 Standup, team"],
            "Wednesday left out, Friday moved, all-day and cancelled skipped"
        );
        let (r, next) = calendar_reading(&events, now);
        assert_eq!((r.text.as_str(), r.sub.as_str(), r.num), ("30 min", "Standup, team", Some(30.0)));
        assert_eq!(next, Duration::from_secs(20));
        let (soon, next) = calendar_reading(&events, at("2026-10-12 09:26"));
        assert_eq!(next, Duration::from_secs(1), "counts every second before it starts");
        assert_eq!(soon.text, "4 min");
        let pulses = (0..2).map(|s| calendar_reading(&events, at("2026-10-12 09:26") + chrono::Duration::seconds(s)).0.attention).collect::<Vec<_>>();
        assert_ne!(pulses[0], pulses[1], "the key pulses");
        assert_eq!(calendar_reading(&events, at("2026-10-12 09:45")).0.text, "Now");
        assert_eq!(calendar_reading(&events, at("2026-10-12 10:30")).0.text, "4 h");
        assert_eq!(calendar_reading(&events, at("2026-10-12 16:00")).0.text, "Fri");
        assert_eq!(calendar_reading(&[], now).0.text, "Free");
        assert_eq!(ics_duration("PT1H30M"), Some(chrono::Duration::minutes(90)));
        let daily = occurrences(at("2026-10-01 08:00"), "FREQ=DAILY;INTERVAL=2;COUNT=3", at("2026-12-01 00:00"));
        assert_eq!(daily.len(), 3);
        assert_eq!(daily[2], at("2026-10-05 08:00"));
        let monthly = occurrences(at("2026-01-31 08:00"), "FREQ=MONTHLY;UNTIL=20260601T000000Z", at("2026-12-01 00:00"));
        assert_eq!(monthly.len(), 3, "January, March, May have a 31st");
        let src = source(&json!({"source": "calendar", "url": "webcal://cal.example/x.ics"}), "").unwrap();
        assert_eq!(src.kind, Kind::Calendar { url: "https://cal.example/x.ics".into() });
        assert_eq!(src.every, CALENDAR_EVERY);
    }

    #[cfg(windows)]
    #[test]
    fn commands_report_their_first_line() {
        assert_eq!(Reading::from_text(&run_command("echo 42").unwrap()).num, Some(42.0));
    }
}
