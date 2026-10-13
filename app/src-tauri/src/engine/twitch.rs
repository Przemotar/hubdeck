//! Twitch through the Helix API: chat message, clear chat, chat modes (followers, subscribers,
//! emotes, slow), Shield Mode, ads, stream title and game, stream markers, clips. Sign-in is
//! Twitch's device code flow with ONE public HubDeck app (no secret ships with HubDeck): the browser
//! opens Twitch's activation page, the user approves, HubDeck receives the tokens. The refresh token
//! goes to the system's credential store. A worker thread does every call.
//!
//! `TWITCH_CLIENT_ID` is empty until the owner registers the HubDeck app on dev.twitch.tv; until
//! then the actions say that sign-in is not set up in this build.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, unbounded};
use serde_json::{Value, json};

use super::{Msg, secrets};

/// Client id of the public HubDeck app on dev.twitch.tv (category "Application Integration",
/// client type "Public"). Empty = Twitch sign-in is not set up in this build.
pub const TWITCH_CLIENT_ID: &str = "";

const NOT_SET_UP: &str = "Twitch sign-in is not set up in this build";
const API: &str = "https://api.twitch.tv/helix";
const SCOPES: &str = "user:write:chat moderator:manage:chat_settings moderator:manage:chat_messages moderator:manage:shield_mode channel:edit:commercial channel:manage:broadcast clips:edit";
const SECRET: &str = "twitch";

#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    Chat(String),
    ClearChat,
    /// Turn a chat mode on or off: `followers` (with minutes), `subscribers`, `emote`, `slow` (with seconds).
    ChatMode {
        mode: String,
        duration: u32,
    },
    Shield,
    Ad(u32),
    /// Empty fields stay as they are.
    TitleGame {
        title: String,
        game: String,
    },
    Marker(String),
    Clip,
    OpenLastClip,
}

static WORKER: OnceLock<Sender<Request>> = OnceLock::new();

pub fn start(engine: Sender<Msg>) {
    let (tx, rx) = unbounded();
    if WORKER.set(tx).is_ok() {
        std::thread::Builder::new().name("twitch".into()).spawn(move || Worker::new(engine).run(rx)).expect("spawn twitch thread");
    }
}

pub fn request(r: Request) -> Result<(), String> {
    if TWITCH_CLIENT_ID.is_empty() {
        return Err(NOT_SET_UP.into());
    }
    if let Some(tx) = WORKER.get() {
        let _ = tx.send(r);
    }
    Ok(())
}

/// Whether this build can sign in, and whether it is signed in.
pub fn status() -> (bool, bool) {
    (!TWITCH_CLIENT_ID.is_empty(), secrets::get(SECRET).is_some())
}

pub fn disconnect() -> Result<(), String> {
    secrets::set(SECRET, "")
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(10))).http_status_as_error(false).build().into()
}

fn post_form(url: &str, form: &[(&str, &str)]) -> Result<(u16, Value), String> {
    let mut resp = agent().post(url).send_form(form.iter().copied()).map_err(|e| e.to_string())?;
    let status = resp.status().as_u16();
    let body: Value = resp.body_mut().read_json().unwrap_or_default();
    Ok((status, body))
}

/// Sign in with the device code flow: open Twitch's page in the browser, then wait (up to five
/// minutes) until the user approved. Blocking; run it on a blocking thread.
pub fn connect(open_url: impl Fn(&str)) -> Result<(), String> {
    if TWITCH_CLIENT_ID.is_empty() {
        return Err(NOT_SET_UP.into());
    }
    let (status, start) = post_form("https://id.twitch.tv/oauth2/device", &[("client_id", TWITCH_CLIENT_ID), ("scopes", SCOPES)])?;
    let (device_code, uri) = match (start["device_code"].as_str(), start["verification_uri"].as_str()) {
        (Some(d), Some(u)) if status < 300 => (d.to_string(), u.to_string()),
        _ => return Err(format!("Twitch did not start the sign-in: {}", start["message"].as_str().unwrap_or("unknown error"))),
    };
    open_url(&uri);
    let interval = Duration::from_secs(start["interval"].as_u64().unwrap_or(5).max(1));
    let deadline = Instant::now() + Duration::from_secs(start["expires_in"].as_u64().unwrap_or(300).min(300));
    while Instant::now() < deadline {
        std::thread::sleep(interval);
        let (status, body) = post_form(
            "https://id.twitch.tv/oauth2/token",
            &[("client_id", TWITCH_CLIENT_ID), ("scopes", SCOPES), ("device_code", device_code.as_str()), ("grant_type", "urn:ietf:params:oauth:grant-type:device_code")],
        )?;
        if status < 300 {
            let refresh = body["refresh_token"].as_str().ok_or("no token from Twitch")?;
            return secrets::set(SECRET, refresh);
        }
        match body["message"].as_str().unwrap_or("") {
            "authorization_pending" => continue,
            "slow_down" => std::thread::sleep(interval),
            other => return Err(format!("Twitch did not allow it ({other})")),
        }
    }
    Err("no approval on Twitch within five minutes".into())
}

/// The chat settings change that turns `mode` on or off, given the current settings.
pub fn chat_mode_body(mode: &str, current: &Value, duration: u32) -> Result<(Value, bool), String> {
    let (field, extra) = match mode {
        "followers" => ("follower_mode", Some(("follower_mode_duration", duration.min(129_600)))),
        "subscribers" => ("subscriber_mode", None),
        "emote" => ("emote_mode", None),
        "slow" => ("slow_mode", Some(("slow_mode_wait_time", duration.clamp(3, 120)))),
        other => return Err(format!("unknown chat mode {other:?}")),
    };
    let on = !(current[field] == true);
    let mut body = json!({});
    body[field] = json!(on);
    if let (true, Some((k, v))) = (on, extra) {
        body[k] = json!(v);
    }
    Ok((body, on))
}

fn urlencode(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

struct Worker {
    engine: Sender<Msg>,
    access: Option<(String, Instant)>,
    /// (user id, login) of the signed-in broadcaster.
    me: Option<(String, String)>,
    last_clip: Option<String>,
}

impl Worker {
    fn new(engine: Sender<Msg>) -> Self {
        Self { engine, access: None, me: None, last_clip: None }
    }

    fn token(&mut self) -> Result<String, String> {
        if let Some((t, until)) = &self.access
            && Instant::now() < *until
        {
            return Ok(t.clone());
        }
        let refresh = secrets::get(SECRET).ok_or("connect Twitch first (profile panel > Connections)")?;
        let (status, body) = post_form("https://id.twitch.tv/oauth2/token", &[("grant_type", "refresh_token"), ("refresh_token", refresh.as_str()), ("client_id", TWITCH_CLIENT_ID)])?;
        if status >= 300 {
            self.me = None;
            return Err("Twitch refused the saved sign-in; connect again".into());
        }
        let access = body["access_token"].as_str().ok_or("no token from Twitch")?.to_string();
        // a public client's refresh token works once: keep the new one
        if let Some(r) = body["refresh_token"].as_str() {
            let _ = secrets::set(SECRET, r);
        }
        let ttl = body["expires_in"].as_u64().unwrap_or(3600).saturating_sub(60);
        self.access = Some((access.clone(), Instant::now() + Duration::from_secs(ttl)));
        Ok(access)
    }

    fn call(&mut self, method: &str, path: &str, body: Option<Value>) -> Result<Value, String> {
        let token = self.token()?;
        let url = format!("{API}{path}");
        let auth = format!("Bearer {token}");
        let a = agent();
        let mut resp = match (method, body) {
            ("GET", _) => a.get(&url).header("Authorization", &auth).header("Client-Id", TWITCH_CLIENT_ID).call(),
            ("POST", Some(b)) => a.post(&url).header("Authorization", &auth).header("Client-Id", TWITCH_CLIENT_ID).send_json(&b),
            ("POST", None) => a.post(&url).header("Authorization", &auth).header("Client-Id", TWITCH_CLIENT_ID).send_empty(),
            ("PATCH", Some(b)) => a.patch(&url).header("Authorization", &auth).header("Client-Id", TWITCH_CLIENT_ID).send_json(&b),
            ("PUT", Some(b)) => a.put(&url).header("Authorization", &auth).header("Client-Id", TWITCH_CLIENT_ID).send_json(&b),
            (_, _) => a.delete(&url).header("Authorization", &auth).header("Client-Id", TWITCH_CLIENT_ID).call(),
        }
        .map_err(|e| e.to_string())?;
        let status = resp.status().as_u16();
        let text = resp.body_mut().read_to_string().unwrap_or_default();
        if status == 401 {
            self.access = None;
        }
        if status >= 300 {
            let v: Value = serde_json::from_str(&text).unwrap_or_default();
            return Err(v["message"].as_str().map(str::to_string).unwrap_or_else(|| format!("Twitch answered {status}")));
        }
        Ok(serde_json::from_str(&text).unwrap_or(Value::Null))
    }

    fn me(&mut self) -> Result<(String, String), String> {
        if let Some(me) = &self.me {
            return Ok(me.clone());
        }
        let v = self.call("GET", "/users", None)?;
        let user = &v["data"][0];
        let me = (user["id"].as_str().ok_or("Twitch did not say who is signed in")?.to_string(), user["login"].as_str().unwrap_or("").to_string());
        self.me = Some(me.clone());
        Ok(me)
    }

    fn feedback(&self, caption: &str) {
        let _ = self.engine.send(Msg::IntegrationFeedback(caption.to_string()));
    }

    fn perform(&mut self, r: Request) -> Result<(), String> {
        let (id, login) = self.me()?;
        let ids = format!("broadcaster_id={id}&moderator_id={id}");
        match r {
            Request::Chat(text) => {
                self.call("POST", "/chat/messages", Some(json!({"broadcaster_id": id, "sender_id": id, "message": text})))?;
            }
            Request::ClearChat => {
                self.call("DELETE", &format!("/moderation/chat?{ids}"), None)?;
                self.feedback("Chat cleared");
            }
            Request::ChatMode { mode, duration } => {
                let current = self.call("GET", &format!("/chat/settings?{ids}"), None)?;
                let (body, on) = chat_mode_body(&mode, &current["data"][0], duration)?;
                self.call("PATCH", &format!("/chat/settings?{ids}"), Some(body))?;
                let _ = self.engine.send(Msg::IntegrationState { key: format!("twitch:mode:{mode}"), on });
            }
            Request::Shield => {
                let current = self.call("GET", &format!("/moderation/shield_mode?{ids}"), None)?;
                let on = current["data"][0]["is_active"] != true;
                self.call("PUT", &format!("/moderation/shield_mode?{ids}"), Some(json!({"is_active": on})))?;
                let _ = self.engine.send(Msg::IntegrationState { key: "twitch:shield".into(), on });
            }
            Request::Ad(length) => {
                self.call("POST", "/channels/commercial", Some(json!({"broadcaster_id": id, "length": length.clamp(30, 180)})))?;
                self.feedback("Ad started");
            }
            Request::TitleGame { title, game } => {
                let mut body = json!({});
                if !title.trim().is_empty() {
                    body["title"] = json!(title.trim());
                }
                if !game.trim().is_empty() {
                    let g = self.call("GET", &format!("/games?name={}", urlencode(game.trim())), None)?;
                    body["game_id"] = json!(g["data"][0]["id"].as_str().ok_or_else(|| format!("Twitch knows no game named {:?}", game.trim()))?);
                }
                self.call("PATCH", &format!("/channels?broadcaster_id={id}"), Some(body))?;
                self.feedback("Stream info set");
            }
            Request::Marker(description) => {
                let mut body = json!({"user_id": id});
                if !description.trim().is_empty() {
                    body["description"] = json!(description.trim().chars().take(140).collect::<String>());
                }
                self.call("POST", "/streams/markers", Some(body))?;
                self.feedback("Marker added");
            }
            Request::Clip => {
                let v = self.call("POST", &format!("/clips?broadcaster_id={id}"), None)?;
                let clip = v["data"][0]["id"].as_str().ok_or("Twitch made no clip (are you live?)")?;
                self.last_clip = Some(format!("https://clips.twitch.tv/{clip}"));
                self.feedback("Clip created");
            }
            Request::OpenLastClip => {
                let url = self.last_clip.clone().unwrap_or_else(|| format!("https://www.twitch.tv/{login}/clips"));
                crate::system::shell_open(&url)?;
            }
        }
        Ok(())
    }

    fn run(mut self, rx: Receiver<Request>) {
        while let Ok(r) = rx.recv() {
            if let Err(e) = self.perform(r) {
                let _ = self.engine.send(Msg::IntegrationError { source: "Twitch".into(), message: e });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_modes_toggle_from_the_current_settings() {
        let off = json!({"follower_mode": false, "slow_mode": false});
        assert_eq!(chat_mode_body("followers", &off, 10).unwrap(), (json!({"follower_mode": true, "follower_mode_duration": 10}), true));
        assert_eq!(chat_mode_body("slow", &off, 1).unwrap(), (json!({"slow_mode": true, "slow_mode_wait_time": 3}), true), "Twitch's minimum");
        let on = json!({"emote_mode": true});
        assert_eq!(chat_mode_body("emote", &on, 0).unwrap(), (json!({"emote_mode": false}), false));
        assert!(chat_mode_body("nope", &off, 0).is_err());
    }

    #[test]
    fn without_a_client_id_twitch_explains() {
        if TWITCH_CLIENT_ID.is_empty() {
            assert_eq!(request(Request::ClearChat).unwrap_err(), NOT_SET_UP);
            assert_eq!(connect(|_| {}).unwrap_err(), NOT_SET_UP);
            assert!(!status().0);
        }
        assert_eq!(urlencode("Just Chatting"), "Just%20Chatting");
    }
}
