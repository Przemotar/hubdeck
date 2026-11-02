//! Home Assistant with a long-lived access token: toggle devices, run scenes and scripts, set light
//! brightness, call any service (REST, from a worker thread). The states of the entities shown on
//! keys come pushed over the WebSocket API (`ha_watch.rs`), so a key lights up when a device is on.

use std::sync::OnceLock;
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender, unbounded};
use serde_json::{Value, json};

use super::Msg;

const TIMEOUT: Duration = Duration::from_secs(4);

#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    /// `homeassistant.toggle` works for lights, switches, fans, covers, media players...
    Toggle(String),
    /// Scenes and scripts: `<domain>.turn_on`.
    Activate(String),
    /// Light brightness 0..=100.
    Brightness { entity: String, percent: f32 },
    /// Light brightness up or down by percent.
    BrightnessStep { entity: String, step: f32 },
    /// Any service with JSON data, e.g. `climate.set_temperature`.
    Service { service: String, data: Value },
}

enum Cmd {
    Request(Request),
    /// Entities shown on screen; their states are polled.
    Watch(Vec<String>),
    /// Read every entity for the window's lists (`names`).
    List,
    Settings {
        url: String,
        token: Option<String>,
    },
}

static WORKER: OnceLock<Sender<Cmd>> = OnceLock::new();

pub fn start(engine: Sender<Msg>, url: String, token: Option<String>) {
    let (tx, rx) = unbounded();
    if WORKER.set(tx).is_ok() {
        std::thread::Builder::new().name("homeassistant".into()).spawn(move || run(engine, url, token, rx)).expect("spawn home assistant thread");
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

/// Ask for the entity lists (the window opened a Home Assistant field).
pub fn list() {
    send(Cmd::List);
}

/// The entities in `/api/states`, by the lists the window offers: `ha.any` (everything),
/// `ha.light`, `ha.scene` (scenes and scripts); the label is the entity's name in Home Assistant.
pub fn entity_lists(states: &Value) -> Vec<(&'static str, Vec<super::names::Name>)> {
    let all: Vec<super::names::Name> = states
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| {
            let id = s["entity_id"].as_str()?;
            let name = s["attributes"]["friendly_name"].as_str().unwrap_or(id);
            Some(super::names::Name { value: id.to_string(), label: if name == id { id.to_string() } else { format!("{name} ({id})") } })
        })
        .collect();
    let of = |domains: &[&str]| all.iter().filter(|n| domains.contains(&domain(&n.value))).cloned().collect::<Vec<_>>();
    vec![("ha.light", of(&["light"])), ("ha.scene", of(&["scene", "script"])), ("ha.any", all.clone())]
}

pub fn watch(entities: Vec<String>) {
    send(Cmd::Watch(entities));
}

/// Entities Home Assistant answered "not found" for (the import check shows them).
static MISSING: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

pub fn missing() -> Vec<String> {
    MISSING.lock().map(|m| m.clone()).unwrap_or_default()
}

/// Of the entities `watched`, the ones Home Assistant does not have now are `missing`.
pub fn set_missing(watched: &[String], missing: &[String]) {
    if let Ok(mut m) = MISSING.lock() {
        m.retain(|e| !watched.contains(e));
        m.extend(missing.iter().cloned());
    }
}

pub fn configure(url: String, token: Option<String>) {
    send(Cmd::Settings { url, token });
}

/// `light.desk` -> `light`.
fn domain(entity: &str) -> &str {
    entity.split('.').next().unwrap_or("")
}

/// The service call for a request: (`domain/service`, JSON body).
pub fn service_call(r: &Request) -> Result<(String, Value), String> {
    Ok(match r {
        Request::Toggle(e) => ("homeassistant/toggle".into(), json!({"entity_id": e})),
        Request::Activate(e) => (format!("{}/turn_on", domain(e)), json!({"entity_id": e})),
        Request::Brightness { entity, percent } if *percent <= 0.0 => ("light/turn_off".into(), json!({"entity_id": entity})),
        Request::Brightness { entity, percent } => ("light/turn_on".into(), json!({"entity_id": entity, "brightness_pct": percent.round().clamp(1.0, 100.0)})),
        Request::BrightnessStep { entity, step } => ("light/turn_on".into(), json!({"entity_id": entity, "brightness_step_pct": step.round()})),
        Request::Service { service, data } => {
            let (d, s) = service.trim().split_once('.').ok_or("write the service as domain.service, e.g. light.turn_on")?;
            (format!("{d}/{s}"), if data.is_null() { json!({}) } else { data.clone() })
        }
    })
}

/// Whether an entity state counts as "on" for a key.
pub fn is_on(state: &str) -> bool {
    matches!(state, "on" | "open" | "opening" | "playing" | "home" | "heat" | "cool" | "heat_cool" | "auto" | "unlocked")
}

fn run(engine: Sender<Msg>, mut url: String, mut token: Option<String>, rx: Receiver<Cmd>) {
    let agent: ureq::Agent = crate::engine::calls::builder().timeout_global(Some(TIMEOUT)).build().into();
    // the states of the entities on screen: pushed over the WebSocket (polled when it cannot be used)
    let (watch_tx, watch_rx) = unbounded();
    {
        let (engine, agent) = (engine.clone(), agent.clone());
        std::thread::Builder::new().name("homeassistant states".into()).spawn(move || super::ha_watch::run(engine, watch_rx, agent)).expect("spawn home assistant states thread");
    }
    let mut watched: Vec<String> = Vec::new();
    let tell_watcher = |url: &str, token: &Option<String>, watched: &[String]| {
        let _ = watch_tx.send(super::ha_watch::Watch { url: url.to_string(), token: token.clone(), entities: watched.to_vec() });
    };
    let error = |message: String| {
        let _ = engine.send(Msg::IntegrationError { source: "Home Assistant".into(), message });
    };
    loop {
        match rx.recv() {
            Ok(Cmd::Request(r)) => {
                let Some(tok) = token.clone() else {
                    error("set the Home Assistant address and token in the profile panel under Connections".into());
                    continue;
                };
                match service_call(&r) {
                    Ok((path, body)) => {
                        let result = agent.post(format!("{}/api/services/{path}", url.trim_end_matches('/'))).header("Authorization", format!("Bearer {tok}")).send_json(&body);
                        match result {
                            Ok(_) => {}
                            Err(ureq::Error::StatusCode(401)) => error("Home Assistant refused the token".into()),
                            Err(e) => error(format!("{path}: {e}")),
                        }
                    }
                    Err(e) => error(e),
                }
            }
            Ok(Cmd::Watch(list)) => {
                if list != watched {
                    watched = list;
                    tell_watcher(&url, &token, &watched);
                }
            }
            Ok(Cmd::Settings { url: u, token: t }) => {
                url = u;
                token = t;
                super::names::forget("ha.");
                tell_watcher(&url, &token, &watched);
            }
            Ok(Cmd::List) => {
                let Some(tok) = token.clone() else { continue };
                let response = agent.get(format!("{}/api/states", url.trim_end_matches('/'))).header("Authorization", format!("Bearer {tok}")).call();
                if let Ok(mut response) = response
                    && let Ok(v) = response.body_mut().read_json::<Value>()
                {
                    for (kind, names) in entity_lists(&v) {
                        super::names::set(kind, names);
                    }
                }
            }
            Err(_) => return,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_calls() {
        assert_eq!(service_call(&Request::Toggle("light.desk".into())).unwrap(), ("homeassistant/toggle".to_string(), json!({"entity_id": "light.desk"})));
        assert_eq!(service_call(&Request::Activate("scene.movie".into())).unwrap().0, "scene/turn_on");
        assert_eq!(service_call(&Request::Activate("script.goodnight".into())).unwrap().0, "script/turn_on");
        assert_eq!(service_call(&Request::Brightness { entity: "light.a".into(), percent: 0.0 }).unwrap().0, "light/turn_off");
        assert_eq!(service_call(&Request::Brightness { entity: "light.a".into(), percent: 42.4 }).unwrap().1["brightness_pct"], 42.0);
        assert_eq!(service_call(&Request::Service { service: "climate.set_temperature".into(), data: json!({"temperature": 21}) }).unwrap().0, "climate/set_temperature");
        assert!(service_call(&Request::Service { service: "nodot".into(), data: Value::Null }).is_err());
        assert!(is_on("on") && is_on("open") && !is_on("off") && !is_on("unavailable"));
    }

    #[test]
    fn entity_lists_from_the_states_reply() {
        // GET /api/states: an array of {entity_id, state, attributes: {friendly_name}}
        let states = json!([
            {"entity_id": "light.desk", "state": "on", "attributes": {"friendly_name": "Desk lamp"}},
            {"entity_id": "scene.movie", "state": "scening", "attributes": {}},
            {"entity_id": "script.good_night", "state": "off", "attributes": {"friendly_name": "Good night"}},
            {"entity_id": "switch.kettle", "state": "off", "attributes": {"friendly_name": "Kettle"}}
        ]);
        let lists = entity_lists(&states);
        let get = |k: &str| lists.iter().find(|(kind, _)| *kind == k).unwrap().1.clone();
        assert_eq!(get("ha.light"), [super::super::names::Name { value: "light.desk".into(), label: "Desk lamp (light.desk)".into() }]);
        assert_eq!(get("ha.scene").iter().map(|n| n.value.as_str()).collect::<Vec<_>>(), ["scene.movie", "script.good_night"]);
        assert_eq!(get("ha.scene")[0].label, "scene.movie");
        assert_eq!(get("ha.any").len(), 4);
    }
}
