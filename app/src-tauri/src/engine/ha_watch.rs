//! The states of the Home Assistant entities shown on keys, pushed by Home Assistant over its
//! WebSocket API (`subscribe_entities`, Home Assistant 2022.4 and later) instead of a GET per entity
//! every two seconds (a user's report, 2026-10-20: "cos wysyla zapytania za czesto"; up to 1800 an
//! hour per entity; ROADMAP > Now, 2026-11-01). One connection, one subscription for the entities on
//! screen; a key changes when the device does. When the WebSocket cannot be used (an older Home
//! Assistant, a proxy that refuses it) the states are read over REST every two seconds as before,
//! and the WebSocket is tried again every few minutes.

use std::collections::{HashMap, HashSet};
use std::io::ErrorKind;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender};
use serde_json::{Value, json};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

use super::Msg;

const POLL: Duration = Duration::from_secs(2);
const CONNECT: Duration = Duration::from_secs(4);
/// While connected, how long a read waits before looking at new settings.
const TICK: Duration = Duration::from_millis(250);
/// A ping when nothing came for this long, so a dead connection is noticed.
const PING_AFTER: Duration = Duration::from_secs(30);
/// After a failure: wait, doubling up to a minute.
const RETRY_FIRST: Duration = Duration::from_secs(5);
const RETRY_MAX: Duration = Duration::from_secs(60);
/// Polling instead: how often the WebSocket is tried again.
const WS_AGAIN: Duration = Duration::from_secs(300);

/// What the watcher works with; a new one replaces the old.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Watch {
    pub url: String,
    pub token: Option<String>,
    pub entities: Vec<String>,
}

type Socket = WebSocket<MaybeTlsStream<TcpStream>>;
/// Each entity's last on/off told to the engine.
type Known = HashMap<String, bool>;
/// Tells the engine an entity's new state when its on/off changed.
type Tell<'a> = &'a dyn Fn(&mut Known, &str, &str);

/// `http://homeassistant.local:8123/` -> `ws://homeassistant.local:8123/api/websocket`.
pub fn ws_url(url: &str) -> Option<String> {
    let url = url.trim().trim_end_matches('/');
    let rest = url.strip_prefix("https://").map(|r| ("wss://", r)).or_else(|| url.strip_prefix("http://").map(|r| ("ws://", r)))?;
    Some(format!("{}{}/api/websocket", rest.0, rest.1))
}

/// One event of `subscribe_entities` applied to the known states: `a` adds entities (full state
/// `s`), `c` changes them (`+` holds the new `s` when the state changed, not only attributes), `r`
/// removes them. Returns the entities whose state changed, with the new state.
pub fn apply(event: &Value, states: &mut HashMap<String, String>) -> Vec<(String, String)> {
    let mut changed = Vec::new();
    if let Some(added) = event["a"].as_object() {
        for (id, v) in added {
            if let Some(s) = v["s"].as_str()
                && states.insert(id.clone(), s.to_string()).as_deref() != Some(s)
            {
                changed.push((id.clone(), s.to_string()));
            }
        }
    }
    if let Some(changes) = event["c"].as_object() {
        for (id, v) in changes {
            if let Some(s) = v["+"]["s"].as_str()
                && states.insert(id.clone(), s.to_string()).as_deref() != Some(s)
            {
                changed.push((id.clone(), s.to_string()));
            }
        }
    }
    if let Some(removed) = event["r"].as_array() {
        for id in removed.iter().filter_map(Value::as_str) {
            if states.remove(id).is_some() {
                changed.push((id.to_string(), "unavailable".to_string()));
            }
        }
    }
    changed
}

/// Run the watcher (its own thread): settings and the entities on screen come on `rx`.
pub fn run(engine: Sender<Msg>, rx: Receiver<Watch>, poll_agent: ureq::Agent) {
    let mut watch = Watch::default();
    let mut known: HashMap<String, bool> = HashMap::new();
    let mut retry = RETRY_FIRST;
    let mut polling_since: Option<Instant> = None;
    let mut last_poll = Instant::now() - POLL;
    let tell = |known: &mut HashMap<String, bool>, entity: &str, state: &str| {
        let on = super::homeassistant::is_on(state);
        if known.insert(entity.to_string(), on) != Some(on) {
            let _ = engine.send(Msg::IntegrationState { key: format!("ha:{entity}"), on });
        }
    };
    let error = |message: String| {
        let _ = engine.send(Msg::IntegrationError { source: "Home Assistant".into(), message });
    };
    loop {
        // the newest settings; nothing to watch: wait for some
        while let Ok(w) = rx.try_recv() {
            watch = w;
        }
        known.retain(|k, _| watch.entities.contains(k));
        let Some(token) = watch.token.clone().filter(|_| !watch.entities.is_empty() && !watch.url.is_empty()) else {
            match rx.recv() {
                Ok(w) => watch = w,
                Err(_) => return,
            }
            continue;
        };
        let use_ws = polling_since.is_none_or(|since| since.elapsed() >= WS_AGAIN);
        if use_ws {
            match connect(&watch.url, &token) {
                Ok(mut socket) => {
                    if polling_since.take().is_some() {
                        log::info!("Home Assistant: back on the WebSocket");
                    }
                    log::info!("Home Assistant: watching {} entities over the WebSocket", watch.entities.len());
                    retry = RETRY_FIRST;
                    match follow(&mut socket, &watch, &rx, &mut known, &tell) {
                        Follow::NewSettings(w) => watch = w,
                        Follow::Closed(why) => {
                            log::info!("Home Assistant: the WebSocket closed ({why}); again in {} s", retry.as_secs());
                            if let Ok(w) = rx.recv_timeout(retry) {
                                watch = w;
                            }
                            retry = (retry * 2).min(RETRY_MAX);
                        }
                        Follow::Unsupported(why) => {
                            log::info!("Home Assistant: no entity subscription ({why}); reading the states every 2 s");
                            polling_since = Some(Instant::now());
                        }
                        Follow::Gone => return,
                    }
                    let _ = socket.close(None);
                    continue;
                }
                Err(Connect::Refused) => {
                    error("Home Assistant refused the token".into());
                    if let Ok(w) = rx.recv_timeout(RETRY_MAX) {
                        watch = w;
                    }
                    continue;
                }
                Err(Connect::NoWebSocket(why)) => {
                    log::info!("Home Assistant: no WebSocket ({why}); reading the states every 2 s");
                    polling_since = Some(Instant::now());
                }
                Err(Connect::Unreachable(why)) => {
                    log::info!("Home Assistant: cannot connect ({why}); again in {} s", retry.as_secs());
                    if let Ok(w) = rx.recv_timeout(retry) {
                        watch = w;
                    }
                    retry = (retry * 2).min(RETRY_MAX);
                    continue;
                }
            }
        }
        // polling, as before the WebSocket
        if last_poll.elapsed() < POLL {
            if let Ok(w) = rx.recv_timeout(POLL - last_poll.elapsed()) {
                watch = w;
            }
            continue;
        }
        last_poll = Instant::now();
        let mut missing = Vec::new();
        for entity in &watch.entities {
            let response = poll_agent.get(format!("{}/api/states/{entity}", watch.url.trim().trim_end_matches('/'))).header("Authorization", format!("Bearer {token}")).call();
            if matches!(response, Err(ureq::Error::StatusCode(404))) {
                missing.push(entity.clone());
            }
            let Ok(mut response) = response else { continue };
            let Ok(v) = response.body_mut().read_json::<Value>() else { continue };
            tell(&mut known, entity, v["state"].as_str().unwrap_or(""));
        }
        super::homeassistant::set_missing(&watch.entities, &missing);
    }
}

enum Connect {
    /// The token was refused (`auth_invalid`).
    Refused,
    /// The server answered but not as Home Assistant's WebSocket API.
    NoWebSocket(String),
    /// Nothing answered (down, no network): try again later.
    Unreachable(String),
}

/// Connect, with a time limit, and sign in with the token.
fn connect(url: &str, token: &str) -> Result<Socket, Connect> {
    let ws = ws_url(url).ok_or_else(|| Connect::NoWebSocket(format!("not an http(s) address: {url}")))?;
    let parsed: tungstenite::http::Uri = ws.parse().map_err(|e| Connect::NoWebSocket(format!("{e}")))?;
    let host = parsed.host().ok_or_else(|| Connect::NoWebSocket("no host".into()))?;
    let tls = parsed.scheme_str() == Some("wss");
    let port = parsed.port_u16().unwrap_or(if tls { 443 } else { 80 });
    let addr = (host, port).to_socket_addrs().map_err(|e| Connect::Unreachable(e.to_string()))?.next().ok_or_else(|| Connect::Unreachable("no address".into()))?;
    let stream = TcpStream::connect_timeout(&addr, CONNECT).map_err(|e| Connect::Unreachable(e.to_string()))?;
    let _ = stream.set_read_timeout(Some(CONNECT));
    let _ = stream.set_nodelay(true);
    let (mut socket, _) = if tls {
        tungstenite::client_tls(ws.as_str(), stream).map_err(|e| Connect::NoWebSocket(e.to_string()))?
    } else {
        tungstenite::client(ws.as_str(), MaybeTlsStream::Plain(stream)).map_err(|e| Connect::NoWebSocket(e.to_string()))?
    };
    let first = read_json(&mut socket).map_err(Connect::NoWebSocket)?;
    if first["type"] != "auth_required" {
        return Err(Connect::NoWebSocket(format!("first message {}", first["type"])));
    }
    socket.send(Message::text(json!({"type": "auth", "access_token": token}).to_string())).map_err(|e| Connect::Unreachable(e.to_string()))?;
    let answer = read_json(&mut socket).map_err(Connect::Unreachable)?;
    match answer["type"].as_str() {
        Some("auth_ok") => {}
        Some("auth_invalid") => return Err(Connect::Refused),
        other => return Err(Connect::NoWebSocket(format!("auth answered {other:?}"))),
    }
    set_timeout(&mut socket, TICK);
    Ok(socket)
}

fn set_timeout(socket: &mut Socket, d: Duration) {
    match socket.get_mut() {
        MaybeTlsStream::Plain(s) => {
            let _ = s.set_read_timeout(Some(d));
        }
        MaybeTlsStream::Rustls(s) => {
            let _ = s.get_mut().set_read_timeout(Some(d));
        }
        _ => {}
    }
}

/// The next JSON message (the stream's read timeout bounds the wait).
fn read_json(socket: &mut Socket) -> Result<Value, String> {
    loop {
        match socket.read().map_err(|e| e.to_string())? {
            Message::Text(t) => return serde_json::from_str(&t).map_err(|e| e.to_string()),
            Message::Close(_) => return Err("closed".into()),
            _ => {}
        }
    }
}

enum Follow {
    NewSettings(Watch),
    Closed(String),
    Unsupported(String),
    Gone,
}

/// Subscribe to the entities and pass their changes on until the settings change or the
/// connection ends.
fn follow(socket: &mut Socket, watch: &Watch, rx: &Receiver<Watch>, known: &mut Known, tell: Tell) -> Follow {
    const SUB: u64 = 1;
    let mut next_id = SUB + 1;
    if let Err(e) = socket.send(Message::text(json!({"id": SUB, "type": "subscribe_entities", "entity_ids": watch.entities}).to_string())) {
        return Follow::Closed(e.to_string());
    }
    let mut states: HashMap<String, String> = HashMap::new();
    let mut first_event = true;
    let mut heard = Instant::now();
    let mut pinged: Option<Instant> = None;
    loop {
        match rx.try_recv() {
            Ok(w) if w != *watch => return Follow::NewSettings(w),
            Ok(_) => {}
            Err(crossbeam_channel::TryRecvError::Disconnected) => return Follow::Gone,
            Err(crossbeam_channel::TryRecvError::Empty) => {}
        }
        match socket.read() {
            Ok(Message::Text(text)) => {
                heard = Instant::now();
                pinged = None;
                let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
                match (v["type"].as_str(), v["id"].as_u64()) {
                    (Some("result"), Some(SUB)) if v["success"] == false => return Follow::Unsupported(v["error"]["message"].as_str().unwrap_or("refused").to_string()),
                    (Some("event"), Some(SUB)) => {
                        for (entity, state) in apply(&v["event"], &mut states) {
                            tell(known, &entity, &state);
                        }
                        // the first event holds every entity that exists: the rest are not found
                        if first_event {
                            first_event = false;
                            let have: HashSet<&String> = states.keys().collect();
                            let missing: Vec<String> = watch.entities.iter().filter(|e| !have.contains(e)).cloned().collect();
                            super::homeassistant::set_missing(&watch.entities, &missing);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Message::Close(_)) => return Follow::Closed("closed by Home Assistant".into()),
            Ok(_) => heard = Instant::now(),
            Err(tungstenite::Error::Io(e)) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(e) => return Follow::Closed(e.to_string()),
        }
        if heard.elapsed() >= PING_AFTER {
            match pinged {
                None => {
                    if socket.send(Message::text(json!({"id": next_id, "type": "ping"}).to_string())).is_err() {
                        return Follow::Closed("ping failed".into());
                    }
                    next_id += 1;
                    pinged = Some(Instant::now());
                }
                Some(at) if at.elapsed() >= PING_AFTER => return Follow::Closed("no answer to a ping".into()),
                Some(_) => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn addresses() {
        assert_eq!(ws_url("http://homeassistant.local:8123/").as_deref(), Some("ws://homeassistant.local:8123/api/websocket"));
        assert_eq!(ws_url(" https://x.ui.nabu.casa ").as_deref(), Some("wss://x.ui.nabu.casa/api/websocket"));
        assert_eq!(ws_url("homeassistant.local:8123"), None);
    }

    #[test]
    fn the_compressed_states() {
        let mut states = HashMap::new();
        let first = json!({"a": {"light.desk": {"s": "on", "a": {"brightness": 200}}, "switch.kettle": {"s": "off", "a": {}}}});
        let mut got = apply(&first, &mut states);
        got.sort();
        assert_eq!(got, [("light.desk".to_string(), "on".to_string()), ("switch.kettle".to_string(), "off".to_string())]);
        // an attribute alone changes nothing on the key
        assert!(apply(&json!({"c": {"light.desk": {"+": {"a": {"brightness": 90}}}}}), &mut states).is_empty());
        assert_eq!(apply(&json!({"c": {"light.desk": {"+": {"s": "off", "lc": 1.0}}}}), &mut states), [("light.desk".to_string(), "off".to_string())]);
        assert_eq!(apply(&json!({"r": ["switch.kettle"]}), &mut states), [("switch.kettle".to_string(), "unavailable".to_string())]);
    }

    /// A Home Assistant of our own on a local port, speaking its WebSocket API as documented
    /// (developers.home-assistant.io > WebSocket API): auth, `subscribe_entities`, events.
    fn fake_home_assistant(token: &'static str, subscribe_ok: bool) -> (String, std::thread::JoinHandle<Vec<Value>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let handle = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut ws = tungstenite::accept(stream).unwrap();
            let mut heard = Vec::new();
            ws.send(Message::text(json!({"type": "auth_required", "ha_version": "2025.10.0"}).to_string())).unwrap();
            let auth: Value = serde_json::from_str(&ws.read().unwrap().into_text().unwrap()).unwrap();
            heard.push(auth.clone());
            if auth["access_token"] != token {
                ws.send(Message::text(json!({"type": "auth_invalid", "message": "Invalid access token"}).to_string())).unwrap();
                return heard;
            }
            ws.send(Message::text(json!({"type": "auth_ok", "ha_version": "2025.10.0"}).to_string())).unwrap();
            let sub: Value = serde_json::from_str(&ws.read().unwrap().into_text().unwrap()).unwrap();
            heard.push(sub.clone());
            let id = sub["id"].clone();
            if !subscribe_ok {
                ws.send(Message::text(json!({"id": id, "type": "result", "success": false, "error": {"code": "unknown_command", "message": "Unknown command."}}).to_string())).unwrap();
                return heard;
            }
            ws.send(Message::text(json!({"id": id, "type": "result", "success": true, "result": null}).to_string())).unwrap();
            ws.send(Message::text(json!({"id": id, "type": "event", "event": {"a": {"light.desk": {"s": "off", "a": {"friendly_name": "Desk"}, "c": "x", "lc": 1.0}}}}).to_string())).unwrap();
            std::thread::sleep(Duration::from_millis(300));
            ws.send(Message::text(json!({"id": id, "type": "event", "event": {"c": {"light.desk": {"+": {"s": "on", "lc": 2.0}}}}}).to_string())).unwrap();
            std::thread::sleep(Duration::from_millis(500));
            let _ = ws.close(None);
            heard
        });
        (url, handle)
    }

    #[test]
    fn states_come_pushed_from_home_assistant() {
        let (url, server) = fake_home_assistant("good", true);
        let (engine_tx, engine_rx) = crossbeam_channel::unbounded();
        let (tx, rx) = crossbeam_channel::unbounded();
        let agent: ureq::Agent = ureq::Agent::config_builder().build().into();
        std::thread::spawn(move || run(engine_tx, rx, agent));
        tx.send(Watch { url, token: Some("good".into()), entities: vec!["light.desk".into(), "light.gone".into()] }).unwrap();
        let mut states = Vec::new();
        while let Ok(msg) = engine_rx.recv_timeout(Duration::from_secs(5)) {
            if let Msg::IntegrationState { key, on } = msg {
                states.push((key, on));
                if states.len() == 2 {
                    break;
                }
            }
        }
        assert_eq!(states, [("ha:light.desk".to_string(), false), ("ha:light.desk".to_string(), true)], "the first state, then the push");
        let heard = server.join().unwrap();
        assert_eq!(heard[0]["type"], "auth");
        assert_eq!(heard[1]["type"], "subscribe_entities");
        assert_eq!(heard[1]["entity_ids"], json!(["light.desk", "light.gone"]));
        assert_eq!(super::super::homeassistant::missing(), ["light.gone"], "an entity the first event does not hold is not found");
        drop(tx);
    }

    #[test]
    fn an_older_home_assistant_is_read_over_rest() {
        // the WebSocket answers, but without `subscribe_entities` (before 2022.4); then GET
        // /api/states/<entity> on the same address, as before the WebSocket
        let (url, server) = fake_home_assistant("good", false);
        let port = url.rsplit(':').next().unwrap().to_string();
        let (engine_tx, engine_rx) = crossbeam_channel::unbounded();
        let (tx, rx) = crossbeam_channel::unbounded();
        let agent: ureq::Agent = ureq::Agent::config_builder().build().into();
        std::thread::spawn(move || run(engine_tx, rx, agent));
        tx.send(Watch { url, token: Some("good".into()), entities: vec!["switch.kettle".into()] }).unwrap();
        server.join().unwrap();
        // the same port answers REST now
        let listener = TcpListener::bind(format!("127.0.0.1:{port}")).unwrap();
        let rest = std::thread::spawn(move || {
            use std::io::{Read, Write};
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 2048];
            let n = stream.read(&mut buf).unwrap();
            let request = String::from_utf8_lossy(&buf[..n]).to_string();
            let body = r#"{"entity_id":"switch.kettle","state":"on","attributes":{}}"#;
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK
content-type: application/json
content-length: {}
connection: close

{body}",
                body.len()
            );
            request
        });
        let state = loop {
            match engine_rx.recv_timeout(Duration::from_secs(8)) {
                Ok(Msg::IntegrationState { key, on }) => break (key, on),
                Ok(_) => continue,
                Err(e) => panic!("no state: {e}"),
            }
        };
        assert_eq!(state, ("ha:switch.kettle".to_string(), true));
        let request = rest.join().unwrap();
        assert!(request.starts_with("GET /api/states/switch.kettle "), "{request}");
        assert!(request.to_lowercase().contains("authorization: bearer good"), "{request}");
        drop(tx);
    }

    #[test]
    fn a_refused_token_is_said() {
        let (url, server) = fake_home_assistant("good", true);
        let (engine_tx, engine_rx) = crossbeam_channel::unbounded();
        let (tx, rx) = crossbeam_channel::unbounded();
        let agent: ureq::Agent = ureq::Agent::config_builder().build().into();
        std::thread::spawn(move || run(engine_tx, rx, agent));
        tx.send(Watch { url, token: Some("wrong".into()), entities: vec!["light.desk".into()] }).unwrap();
        let said = loop {
            match engine_rx.recv_timeout(Duration::from_secs(5)) {
                Ok(Msg::IntegrationError { message, .. }) => break message,
                Ok(_) => continue,
                Err(e) => panic!("no error: {e}"),
            }
        };
        assert_eq!(said, "Home Assistant refused the token");
        server.join().unwrap();
    }
}
