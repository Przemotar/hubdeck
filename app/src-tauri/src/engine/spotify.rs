//! Spotify through its Web API, with the user's own app id (developer.spotify.com, free): like or
//! unlike the playing track, remove it from the playlist it plays from, add it to a playlist. The
//! sign-in is PKCE in the browser (no secret in HubDeck); the refresh token goes to the system's
//! credential store. A worker thread does every call, so the deck never waits for the network.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use base64::Engine as _;
use crossbeam_channel::{Receiver, Sender, unbounded};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::{Msg, secrets};

/// Register exactly this as the app's redirect URI on developer.spotify.com.
pub const REDIRECT: &str = "http://127.0.0.1:43123/callback";
// playlist-read-private: the window lists the user's playlists (a sign-in from before asks again)
const SCOPES: &str =
    "user-read-private user-library-read user-library-modify user-read-currently-playing user-read-playback-state playlist-modify-public playlist-modify-private playlist-read-private";
const API: &str = "https://api.spotify.com/v1";
const POLL: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    ToggleLike,
    RemoveFromPlaylist,
    AddToPlaylist(String),
    /// The user's playlists for the window's list (`names`); quiet.
    ListPlaylists,
}

/// The Liked Songs path for one track: `/me/library` by URI (Spotify, February 2026: apps in
/// Development Mode, as every HubDeck user's own is, got 403 Forbidden on the old `/me/tracks`;
/// issue #4).
fn library_path(suffix: &str, uri: &str) -> String {
    format!("/me/library{suffix}?uris={}", urlencode(uri))
}

/// The playlists in a `/me/playlists` reply: the name shown, its `spotify:playlist:` id kept.
pub fn playlist_names(v: &Value) -> Vec<super::names::Name> {
    v["items"].as_array().into_iter().flatten().filter_map(|p| Some(super::names::Name { value: format!("spotify:playlist:{}", p["id"].as_str()?), label: p["name"].as_str()?.to_string() })).collect()
}

enum Cmd {
    Request(Request),
    /// Keys show whether the playing track is liked: keep checking.
    Watch(bool),
    ClientId(String),
}

static WORKER: OnceLock<Sender<Cmd>> = OnceLock::new();

pub fn start(engine: Sender<Msg>, client_id: String) {
    let (tx, rx) = unbounded();
    if WORKER.set(tx).is_ok() {
        std::thread::Builder::new().name("spotify".into()).spawn(move || Worker::new(engine, client_id).run(rx)).expect("spawn spotify thread");
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

/// The user's playlists, asked of Spotify now (up to `wait`); None when Spotify is not connected.
/// For the deck assistant: playlist keys with the user's real playlists instead of "Playlist 1"
/// and a request for links (owner, 2026-10-19).
pub fn playlists(wait: Duration) -> Option<Vec<super::names::Name>> {
    secrets::get("spotify")?;
    super::names::forget("spotify.playlist");
    request(Request::ListPlaylists);
    let until = Instant::now() + wait;
    loop {
        if let Some(list) = super::names::get("spotify.playlist") {
            return Some(list);
        }
        if Instant::now() >= until {
            return Some(Vec::new());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

pub fn watch(on: bool) {
    send(Cmd::Watch(on));
}

pub fn set_client_id(id: String) {
    send(Cmd::ClientId(id));
}

/// A playlist id from an id, a `spotify:playlist:` URI or an open.spotify.com link.
pub fn playlist_id(text: &str) -> Option<String> {
    let t = text.trim();
    let id = if let Some(rest) = t.strip_prefix("spotify:playlist:") {
        rest
    } else if let Some(i) = t.find("/playlist/") {
        t[i + 10..].split(['?', '/']).next().unwrap_or("")
    } else {
        t
    };
    (!id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric())).then(|| id.to_string())
}

fn b64url(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// PKCE: (verifier, challenge).
pub fn pkce() -> (String, String) {
    let verifier = b64url(format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple()).as_bytes());
    let challenge = b64url(&Sha256::digest(verifier.as_bytes()));
    (verifier, challenge)
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(8))).http_status_as_error(false).build().into()
}

/// Who signed in, for the window: their Spotify name and whether they have Premium (None: Spotify
/// did not say). Apps like HubDeck's (Development Mode) stop working without Premium.
#[derive(Debug, Clone, serde::Serialize, PartialEq)]
pub struct Account {
    pub name: String,
    pub premium: Option<bool>,
}

/// The account in a `/me` reply (`product` needs the `user-read-private` scope).
fn account_of(me: &Value) -> Account {
    let name = me["display_name"].as_str().or_else(|| me["id"].as_str()).unwrap_or("").to_string();
    Account { name, premium: me["product"].as_str().map(|p| p == "premium") }
}

/// Sign in: open the browser, wait for Spotify to send the browser back to the loopback address,
/// trade the code for tokens. Blocks up to three minutes; run it on a blocking thread.
pub fn connect(client_id: &str, open_url: impl Fn(&str)) -> Result<Account, String> {
    let client_id = client_id.trim();
    if client_id.len() < 16 {
        return Err("paste the Client ID of your app from developer.spotify.com".into());
    }
    let server = tiny_http::Server::http("127.0.0.1:43123").map_err(|e| format!("port 43123 is busy ({e}); close what uses it and try again"))?;
    let (verifier, challenge) = pkce();
    let state = uuid::Uuid::new_v4().simple().to_string();
    let url = format!(
        "https://accounts.spotify.com/authorize?client_id={client_id}&response_type=code&redirect_uri={}&code_challenge_method=S256&code_challenge={challenge}&scope={}&state={state}",
        urlencode(REDIRECT),
        urlencode(SCOPES)
    );
    open_url(&url);
    let deadline = Instant::now() + Duration::from_secs(180);
    let code = loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err("no answer from the browser within three minutes".into());
        }
        let Ok(Some(req)) = server.recv_timeout(left) else { continue };
        let query = req.url().split_once('?').map(|(_, q)| q.to_string()).unwrap_or_default();
        let param = |k: &str| query.split('&').find_map(|kv| kv.split_once('=').filter(|(a, _)| *a == k).map(|(_, v)| v.to_string()));
        let page = |text: &str| {
            tiny_http::Response::from_string(format!("<html><body style=\"font-family:sans-serif;padding:2em\"><h2>HubDeck</h2><p>{text}</p></body></html>"))
                .with_header("Content-Type: text/html; charset=utf-8".parse::<tiny_http::Header>().expect("static header"))
        };
        if param("state").as_deref() != Some(state.as_str()) {
            let _ = req.respond(page("This answer does not belong to the sign-in HubDeck started."));
            continue;
        }
        if let Some(err) = param("error") {
            let _ = req.respond(page("Spotify did not allow it. You can close this tab."));
            return Err(format!("Spotify said: {err}"));
        }
        let Some(code) = param("code") else { continue };
        let _ = req.respond(page("Connected to Spotify. You can close this tab."));
        break code;
    };
    let mut resp = agent()
        .post("https://accounts.spotify.com/api/token")
        .send_form([("grant_type", "authorization_code"), ("code", code.as_str()), ("redirect_uri", REDIRECT), ("client_id", client_id), ("code_verifier", verifier.as_str())])
        .map_err(|e| e.to_string())?;
    let body: Value = resp.body_mut().read_json().map_err(|e| e.to_string())?;
    let refresh = body["refresh_token"].as_str().ok_or_else(|| format!("no token from Spotify: {}", body["error_description"].as_str().unwrap_or("unknown error")))?;
    secrets::set("spotify", refresh)?;
    set_client_id(client_id.to_string());
    // who it is and whether Premium: the window says so at once instead of a key failing later
    let me = body["access_token"]
        .as_str()
        .and_then(|token| agent().get(&format!("{API}/me")).header("Authorization", &format!("Bearer {token}")).call().ok())
        .and_then(|mut r| r.body_mut().read_json::<Value>().ok())
        .unwrap_or_default();
    Ok(account_of(&me))
}

fn urlencode(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

/// The track playing now: its URI, the playlist it plays from (if one), its name.
struct Playing {
    uri: String,
    playlist: Option<String>,
    name: String,
}

struct Worker {
    engine: Sender<Msg>,
    client_id: String,
    access: Option<(String, Instant)>,
    watching: bool,
    last_poll: Instant,
    liked: Option<bool>,
    /// Spotify said "Too Many Requests": nothing is asked before this (its Retry-After).
    wait_until: Option<Instant>,
}

impl Worker {
    fn new(engine: Sender<Msg>, client_id: String) -> Self {
        Worker { engine, client_id, access: None, watching: false, last_poll: Instant::now() - POLL, liked: None, wait_until: None }
    }

    fn error(&self, message: String) {
        let _ = self.engine.send(Msg::IntegrationError { source: "Spotify".into(), message });
    }

    fn token(&mut self) -> Result<String, String> {
        if let Some((t, until)) = &self.access
            && Instant::now() < *until
        {
            return Ok(t.clone());
        }
        let refresh = secrets::get("spotify").ok_or("connect Spotify first (profile panel > Connections)")?;
        if self.client_id.is_empty() {
            return Err("set your Spotify Client ID first (profile panel > Connections)".into());
        }
        let mut resp = agent()
            .post("https://accounts.spotify.com/api/token")
            .send_form([("grant_type", "refresh_token"), ("refresh_token", refresh.as_str()), ("client_id", self.client_id.as_str())])
            .map_err(|e| e.to_string())?;
        let body: Value = resp.body_mut().read_json().map_err(|e| e.to_string())?;
        let access = body["access_token"].as_str().ok_or("Spotify refused the saved sign-in; connect again")?.to_string();
        // Spotify may hand out a new refresh token; the old one stops working then
        if let Some(r) = body["refresh_token"].as_str() {
            let _ = secrets::set("spotify", r);
        }
        let ttl = body["expires_in"].as_u64().unwrap_or(3600).saturating_sub(60);
        self.access = Some((access.clone(), Instant::now() + Duration::from_secs(ttl)));
        Ok(access)
    }

    fn call(&mut self, method: &str, path: &str, body: Option<Value>) -> Result<Value, String> {
        if let Some(until) = self.wait_until.filter(|u| Instant::now() < *u) {
            return Err(format!("Spotify asked to wait; try again in {} s", until.saturating_duration_since(Instant::now()).as_secs() + 1));
        }
        let token = self.token()?;
        let url = format!("{API}{path}");
        let auth = format!("Bearer {token}");
        let a = agent();
        let mut resp = match (method, body) {
            ("GET", _) => a.get(&url).header("Authorization", &auth).call(),
            ("PUT", Some(b)) => a.put(&url).header("Authorization", &auth).send_json(&b),
            ("PUT", None) => a.put(&url).header("Authorization", &auth).send_empty(),
            ("POST", Some(b)) => a.post(&url).header("Authorization", &auth).send_json(&b),
            ("POST", None) => a.post(&url).header("Authorization", &auth).send_empty(),
            ("DELETE", Some(b)) => a.delete(&url).header("Authorization", &auth).force_send_body().send_json(&b),
            (_, _) => a.delete(&url).header("Authorization", &auth).call(),
        }
        .map_err(|e| e.to_string())?;
        let status = resp.status().as_u16();
        if status == 429 {
            let secs = resp.headers().get("retry-after").and_then(|v| v.to_str().ok()).and_then(|v| v.trim().parse::<u64>().ok()).unwrap_or(30);
            self.wait_until = Some(Instant::now() + Duration::from_secs(secs.clamp(1, 3600)));
        }
        let text = resp.body_mut().read_to_string().unwrap_or_default();
        if status == 401 {
            self.access = None;
        }
        if status >= 300 {
            let v: Value = serde_json::from_str(&text).unwrap_or_default();
            return Err(v["error"]["message"].as_str().map(str::to_string).unwrap_or_else(|| format!("Spotify answered {status}")));
        }
        Ok(serde_json::from_str(&text).unwrap_or(Value::Null))
    }

    /// The playing track, if any.
    fn playing(&mut self) -> Result<Option<Playing>, String> {
        let v = self.call("GET", "/me/player/currently-playing", None)?;
        let item = &v["item"];
        let Some(uri) = item["uri"].as_str().filter(|u| u.starts_with("spotify:track:")) else { return Ok(None) };
        let playlist = (v["context"]["type"] == "playlist").then(|| v["context"]["uri"].as_str().and_then(playlist_id)).flatten();
        Ok(Some(Playing { uri: uri.to_string(), playlist, name: item["name"].as_str().unwrap_or("").to_string() }))
    }

    fn feedback(&self, caption: String) {
        let _ = self.engine.send(Msg::IntegrationFeedback(caption));
    }

    fn perform(&mut self, r: Request) -> Result<(), String> {
        if r == Request::ListPlaylists {
            let v = self.call("GET", "/me/playlists?limit=50", None)?;
            super::names::set("spotify.playlist", playlist_names(&v));
            return Ok(());
        }
        let Playing { uri, playlist, name, .. } = self.playing()?.ok_or("nothing is playing on Spotify")?;
        match r {
            Request::ToggleLike => {
                let saved = self.call("GET", &library_path("/contains", &uri), None)?.get(0).and_then(Value::as_bool).unwrap_or(false);
                let method = if saved { "DELETE" } else { "PUT" };
                self.call(method, &library_path("", &uri), None)?;
                self.liked = Some(!saved);
                let _ = self.engine.send(Msg::IntegrationState { key: "spotify:liked".into(), on: !saved });
                self.feedback(if saved { format!("Unliked {name}") } else { format!("Liked {name}") });
            }
            Request::RemoveFromPlaylist => {
                let playlist = playlist.ok_or("this track is not playing from a playlist")?;
                self.call("DELETE", &format!("/playlists/{playlist}/items"), Some(json!({"items": [{"uri": uri}]})))?;
                self.feedback(format!("Removed {name}"));
                // going to the next track needs Premium; without it the track just stays playing
                let _ = self.call("POST", "/me/player/next", None);
            }
            Request::AddToPlaylist(target) => {
                let target = playlist_id(&target).ok_or("set the playlist (its link or id)")?;
                self.call("POST", &format!("/playlists/{target}/items"), Some(json!({"uris": [uri]})))?;
                self.feedback(format!("Added {name}"));
            }
            // answered above, before anything has to play
            Request::ListPlaylists => {}
        }
        Ok(())
    }

    fn poll_liked(&mut self) {
        let Ok(Some(Playing { uri, .. })) = self.playing() else { return };
        if let Ok(v) = self.call("GET", &library_path("/contains", &uri), None) {
            let liked = v.get(0).and_then(Value::as_bool).unwrap_or(false);
            if self.liked != Some(liked) {
                self.liked = Some(liked);
                let _ = self.engine.send(Msg::IntegrationState { key: "spotify:liked".into(), on: liked });
            }
        }
    }

    fn run(mut self, rx: Receiver<Cmd>) {
        loop {
            // nothing to watch: sleep until a command comes
            let next = if self.watching { rx.recv_timeout(Duration::from_millis(500)) } else { rx.recv().map_err(|_| crossbeam_channel::RecvTimeoutError::Disconnected) };
            match next {
                Ok(Cmd::Request(r)) => {
                    // a list the window asked for fails quietly: the field stays typed
                    let quiet = r == Request::ListPlaylists;
                    if let Err(e) = self.perform(r)
                        && !quiet
                    {
                        self.error(e);
                    }
                }
                Ok(Cmd::Watch(on)) => self.watching = on,
                Ok(Cmd::ClientId(id)) => {
                    self.client_id = id;
                    self.access = None;
                }
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => return,
            }
            if self.watching && self.last_poll.elapsed() >= POLL && secrets::get("spotify").is_some() {
                self.last_poll = Instant::now();
                self.poll_liked();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_account_says_whether_it_has_premium() {
        assert_eq!(account_of(&json!({"display_name": "Ola", "product": "premium"})), Account { name: "Ola".into(), premium: Some(true) });
        assert_eq!(account_of(&json!({"id": "ola123", "product": "free"})), Account { name: "ola123".into(), premium: Some(false) });
        // an older sign-in without user-read-private: not known
        assert_eq!(account_of(&json!({"display_name": "Ola"})).premium, None);
    }

    #[test]
    fn liked_songs_go_through_the_library_by_uri() {
        assert_eq!(library_path("/contains", "spotify:track:7a3LWj5xSFhFRYmztS8wgK"), "/me/library/contains?uris=spotify%3Atrack%3A7a3LWj5xSFhFRYmztS8wgK");
        assert_eq!(library_path("", "spotify:track:7a3LWj5xSFhFRYmztS8wgK"), "/me/library?uris=spotify%3Atrack%3A7a3LWj5xSFhFRYmztS8wgK");
    }

    #[test]
    fn playlists_from_the_reply_keep_their_ids() {
        // GET /v1/me/playlists: {"items": [{"id", "name", ...}]}
        let v = json!({"items": [{"id": "37i9dQZF1DXcBWIGoYBM5M", "name": "Today's Top Hits"}, {"name": "no id"}]});
        let names = playlist_names(&v);
        assert_eq!(names.len(), 1);
        assert_eq!(names[0].label, "Today's Top Hits");
        assert_eq!(playlist_id(&names[0].value).as_deref(), Some("37i9dQZF1DXcBWIGoYBM5M"));
    }

    #[test]
    fn playlist_ids_from_links_and_uris() {
        assert_eq!(playlist_id("37i9dQZF1DXcBWIGoYBM5M").as_deref(), Some("37i9dQZF1DXcBWIGoYBM5M"));
        assert_eq!(playlist_id("spotify:playlist:37i9dQZF1DXcBWIGoYBM5M").as_deref(), Some("37i9dQZF1DXcBWIGoYBM5M"));
        assert_eq!(playlist_id("https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M?si=abc").as_deref(), Some("37i9dQZF1DXcBWIGoYBM5M"));
        assert_eq!(playlist_id("not a playlist!"), None);
    }

    #[test]
    fn pkce_challenge_is_the_sha256_of_the_verifier() {
        let (v, c) = pkce();
        assert!(v.len() >= 43 && v.len() <= 128, "RFC 7636 length");
        assert_eq!(c, b64url(&Sha256::digest(v.as_bytes())));
        assert!(!c.contains('=') && !c.contains('+') && !c.contains('/'));
        assert_eq!(urlencode("a b:/"), "a%20b%3A%2F");
    }
}
