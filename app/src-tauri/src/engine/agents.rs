//! Claude Code sessions on the deck (owner's idea, `private/IDEAS.md` 1): which session works,
//! which waits for the user, which finished its turn.
//!
//! Claude Code hooks run `hubdeck --claude-hook` (see [`run_hook`]), which posts a few fields of the
//! hook's input to the local HTTP API as `POST /api/agent`. Only the state travels: the session id,
//! the project folder, the event and, for a permission, the tool name. Never prompts, tool input or
//! messages.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use serde_json::Value;

use super::profile::Look;

/// Sessions not heard from this long are dropped (a closed terminal sends no SessionEnd).
const FORGET_AFTER: Duration = Duration::from_secs(6 * 3600);
/// A waiting key blinks between its two colors this often.
pub const PULSE: Duration = Duration::from_millis(600);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Started or resumed, nothing asked yet.
    Ready,
    Working,
    /// Needs a permission decision.
    Permission,
    /// Finished its turn: the user's move.
    YourTurn,
}

impl State {
    pub fn word(self) -> &'static str {
        match self {
            State::Ready => "ready",
            State::Working => "working",
            State::Permission => "needs you",
            State::YourTurn => "your turn",
        }
    }

    /// Background color of a session key; a waiting one blinks to `PULSE_ALT`.
    fn color(self) -> &'static str {
        match self {
            State::Ready => "#2a2933",
            State::Working => "#1e3a8a",
            State::Permission => "#b45309",
            State::YourTurn => "#166534",
        }
    }

    pub fn waits(self) -> bool {
        matches!(self, State::Permission | State::YourTurn)
    }
}

const PULSE_ALT: &str = "#f59e0b";

#[derive(Debug, Clone)]
pub struct Session {
    /// The project folder's name, e.g. `masterhub`.
    pub project: String,
    pub cwd: String,
    pub state: State,
    /// The tool waiting for permission, e.g. `Bash`.
    pub tool: Option<String>,
    /// A key slot (0-based) the session keeps while it lives, so keys do not shuffle.
    pub slot: usize,
    pub since: Instant,
    pub heard: Instant,
}

/// Every live session, by id.
#[derive(Debug, Default)]
pub struct Fleet {
    sessions: BTreeMap<String, Session>,
    /// The session the dial shows (index into [`Fleet::list`]).
    pub selected: usize,
}

/// Last folder name of a path, `C:\work\masterhub` -> `masterhub`.
pub fn project_name(cwd: &str) -> String {
    cwd.trim_end_matches(['\\', '/']).rsplit(['\\', '/']).next().filter(|s| !s.is_empty()).unwrap_or("session").to_string()
}

impl Fleet {
    /// Apply one hook report; true when something the deck shows changed.
    pub fn report(&mut self, m: &Value, now: Instant) -> bool {
        let Some(id) = m["session"].as_str().filter(|s| !s.is_empty()) else { return false };
        let hook = m["hook"].as_str().unwrap_or("");
        if hook == "SessionEnd" {
            return self.sessions.remove(id).is_some();
        }
        let state = match (hook, m["kind"].as_str()) {
            ("SessionStart", _) => State::Ready,
            ("UserPromptSubmit" | "PreToolUse" | "PostToolUse" | "PostToolUseFailure" | "SubagentStop", _) => State::Working,
            ("PermissionRequest", _) | ("Notification", Some("permission_prompt")) => State::Permission,
            ("Stop", _) | ("Notification", Some("idle_prompt")) => State::YourTurn,
            _ => return false,
        };
        let cwd = m["cwd"].as_str().unwrap_or("").to_string();
        let free_slot = (0..).find(|s| !self.sessions.values().any(|x| x.slot == *s)).unwrap_or(0);
        let new = !self.sessions.contains_key(id);
        let entry = self.sessions.entry(id.to_string()).or_insert_with(|| Session { project: project_name(&cwd), cwd: cwd.clone(), state, tool: None, slot: free_slot, since: now, heard: now });
        entry.heard = now;
        if !cwd.is_empty() && entry.cwd != cwd {
            entry.project = project_name(&cwd);
            entry.cwd = cwd;
        }
        let tool = (state == State::Permission).then(|| m["tool"].as_str().map(str::to_string)).flatten();
        let changed = new || entry.state != state || entry.tool != tool;
        if entry.state != state {
            entry.since = now;
        }
        entry.state = state;
        entry.tool = tool;
        changed
    }

    /// Drop sessions silent for hours; true when one went.
    pub fn forget_stale(&mut self, now: Instant) -> bool {
        let before = self.sessions.len();
        self.sessions.retain(|_, s| now.duration_since(s.heard) < FORGET_AFTER);
        before != self.sessions.len()
    }

    /// Sessions in slot order.
    pub fn list(&self) -> Vec<&Session> {
        let mut v: Vec<&Session> = self.sessions.values().collect();
        v.sort_by_key(|s| s.slot);
        v
    }

    pub fn by_slot(&self, slot: usize) -> Option<&Session> {
        self.sessions.values().find(|s| s.slot == slot)
    }

    pub fn waiting(&self) -> usize {
        self.sessions.values().filter(|s| s.state.waits()).count()
    }

    /// Something waits, so keys and the edge blink.
    pub fn pulsing(&self) -> bool {
        self.sessions.values().any(|s| s.state == State::Permission)
    }

    /// Look of a session key: project name, state, color; a session needing permission blinks.
    pub fn key_look(&self, slot: usize, base: &Look, now: Instant, epoch: Instant) -> Look {
        let Some(s) = self.by_slot(slot) else {
            return Look { label: Some(format!("Agent {}\n–", slot + 1)), icon: Some("bot".into()), bg: Some("#141418".into()), fg: Some("#6c6b7b".into()), ..base.clone() };
        };
        let blink = s.state == State::Permission && (now.duration_since(epoch).as_millis() / PULSE.as_millis()) % 2 == 1;
        let second = match (&s.tool, s.state) {
            (Some(tool), State::Permission) => format!("{} · {}", s.state.word(), tool),
            _ => s.state.word().to_string(),
        };
        Look {
            label: Some(format!("{}\n{}", short(&s.project, 12), second)),
            icon: Some(if s.state == State::Working {
                "bot".into()
            } else if s.state.waits() {
                "bell".into()
            } else {
                "check".into()
            }),
            bg: Some(if blink { PULSE_ALT } else { s.state.color() }.into()),
            fg: Some("#ffffff".into()),
            image: None,
            face: true,
            ..base.clone()
        }
    }

    /// What the dial shows: the selected session and how many wait.
    pub fn dial_text(&self) -> (String, String) {
        let list = self.list();
        if list.is_empty() {
            return ("No sessions".into(), "Claude Code is quiet".into());
        }
        let s = list[self.selected.min(list.len() - 1)];
        let detail = match (&s.tool, s.state) {
            (Some(tool), State::Permission) => format!("{} · {}", s.state.word(), tool),
            _ => s.state.word().to_string(),
        };
        (short(&s.project, 14), format!("{detail} · {}/{} · {} waiting", self.selected.min(list.len() - 1) + 1, list.len(), self.waiting()))
    }

    /// Turning the dial moves through the sessions.
    pub fn scroll(&mut self, delta: i32) {
        let n = self.sessions.len();
        if n > 0 {
            self.selected = (self.selected as i32 + delta).rem_euclid(n as i32) as usize;
        }
    }

    pub fn selected_session(&self) -> Option<&Session> {
        let list = self.list();
        list.get(self.selected.min(list.len().saturating_sub(1))).copied()
    }
}

fn short(s: &str, max: usize) -> String {
    if s.chars().count() <= max { s.to_string() } else { format!("{}…", s.chars().take(max - 1).collect::<String>()) }
}

// ---------- the hook side: `hubdeck --claude-hook` ----------

/// Where the app keeps `api-token` and `api-http-port` (Tauri's app data dir for `dev.hubdeck.app`).
pub(crate) fn data_dir() -> Option<std::path::PathBuf> {
    const ID: &str = "dev.hubdeck.app";
    // started by a test copy (Build with AI, a hook): talk to that copy, not the real app
    if let Some(a) = crate::automation::get() {
        return Some(a.data_dir.clone());
    }
    #[cfg(windows)]
    return std::env::var_os("APPDATA").map(|d| std::path::PathBuf::from(d).join(ID));
    #[cfg(target_os = "macos")]
    return std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join("Library/Application Support").join(ID));
    #[cfg(all(unix, not(target_os = "macos")))]
    return std::env::var_os("XDG_DATA_HOME").map(std::path::PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/share"))).map(|d| d.join(ID));
}

/// The fields of a hook's input that leave the process: no prompt, no tool input, no messages.
pub fn hook_report(input: &Value) -> Value {
    serde_json::json!({
        "session": input["session_id"].as_str().unwrap_or(""),
        "hook": input["hook_event_name"].as_str().unwrap_or(""),
        "cwd": input["cwd"].as_str().unwrap_or(""),
        "kind": input["notification_type"].as_str(),
        "tool": input["tool_name"].as_str(),
    })
}

/// Entry point of `hubdeck --claude-hook`: read the hook input, post the state, exit quietly. Any
/// failure (HubDeck not running, no token) is silent: a hook must never disturb Claude Code.
pub fn run_hook() {
    use std::io::{Read, Write};
    let mut input = String::new();
    let _ = std::io::stdin().take(1_000_000).read_to_string(&mut input);
    let Ok(input) = serde_json::from_str::<Value>(&input) else { return };
    if std::env::args().any(|a| a == APPROVE_ARG) {
        // the answer goes to stdout; exit 2 without a decision leaves Claude Code's own question
        match approve(&input) {
            Some(answer) => println!("{answer}"),
            None => std::process::exit(2),
        }
        return;
    }
    let Some(dir) = data_dir() else { return };
    let (Ok(token), Ok(port)) = (std::fs::read_to_string(dir.join("api-token")), std::fs::read_to_string(dir.join("api-http-port"))) else { return };
    let Ok(port) = port.trim().parse::<u16>() else { return };
    let body = hook_report(&input).to_string();
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let Ok(mut stream) = std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(300)) else { return };
    let _ = stream.set_write_timeout(Some(Duration::from_millis(300)));
    let _ = stream.set_read_timeout(Some(Duration::from_millis(300)));
    let request = format!(
        "POST /api/agent HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        token.trim(),
        body.len()
    );
    if stream.write_all(request.as_bytes()).is_ok() {
        let mut reply = [0u8; 64];
        let _ = stream.read(&mut reply);
    }
}

/// What the deck shows for a permission question: the tool and what it touches, short. Stays on
/// this computer (it goes to the local HubDeck only).
pub fn approve_question(input: &Value) -> Option<String> {
    let tool = input["tool_name"].as_str()?;
    if !SAFE_TOOLS.contains(&tool) {
        return None;
    }
    let what = ["file_path", "pattern", "path", "query", "url"].iter().find_map(|k| input["tool_input"][*k].as_str()).unwrap_or("");
    let what = what.rsplit(['\\', '/']).find(|s| !s.is_empty()).unwrap_or(what);
    let short: String = what.chars().take(40).collect();
    Some(if short.is_empty() { format!("{tool}?") } else { format!("{tool} {short}?") })
}

/// The hook's answer to Claude Code from a deck answer.
pub fn approve_answer(allow: bool) -> Value {
    if allow {
        serde_json::json!({"hookSpecificOutput": {"hookEventName": "PermissionRequest", "decision": {"behavior": "allow"}}})
    } else {
        serde_json::json!({"hookSpecificOutput": {"hookEventName": "PermissionRequest", "decision": {"behavior": "deny", "message": "The user said no on the deck."}}})
    }
}

/// Ask on the deck and wait; None = no decision (not a safe tool, HubDeck not running, no press).
fn approve(input: &Value) -> Option<Value> {
    let question = approve_question(input)?;
    let dir = data_dir()?;
    // the keys in the window's language
    let polish = std::fs::read_to_string(dir.join("settings.json")).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).is_some_and(|v| v["language"] == "pl");
    let options = if polish { ["Zezwól", "Odmów"] } else { ["Allow", "Deny"] };
    let api = super::mcp::Http { dir };
    use super::mcp::Api as _;
    let asked = api.call("POST", "/api/ask", Some(&serde_json::json!({"question": question, "options": options, "seconds": APPROVE_WAIT_S}))).ok()?;
    let id = asked["id"].as_u64()?;
    let until = std::time::Instant::now() + Duration::from_secs(APPROVE_WAIT_S + 2);
    while std::time::Instant::now() < until {
        let a = api.call("GET", &format!("/api/ask/{id}"), None).ok()?;
        if a["waiting"].as_bool() != Some(true) {
            return match a["index"].as_u64() {
                Some(0) => Some(approve_answer(true)),
                Some(1) => Some(approve_answer(false)),
                _ => None,
            };
        }
        std::thread::sleep(Duration::from_millis(300));
    }
    None
}

// ---------- connecting Claude Code: hooks in the user's settings ----------

/// Hook events HubDeck listens to.
pub const HOOK_EVENTS: [&str; 9] = ["SessionStart", "UserPromptSubmit", "PreToolUse", "PostToolUse", "PermissionRequest", "Notification", "Stop", "SubagentStop", "SessionEnd"];
/// Marks our hook entries so they can be found and removed again.
pub const HOOK_ARG: &str = "--claude-hook";
/// Our synchronous `PermissionRequest` hook: the question goes to the deck (opt-in).
pub const APPROVE_ARG: &str = "--approve";
/// How long the deck waits for a press before Claude Code asks in the terminal as usual.
pub const APPROVE_WAIT_S: u64 = 30;

/// Tools whose permission the deck may answer: they only read (owner, 2026-10-12: "only safe tools").
pub const SAFE_TOOLS: [&str; 6] = ["Read", "Glob", "Grep", "LS", "WebSearch", "WebFetch"];

/// Our entries: the command string runs this program with `--claude-hook` (older entries kept it
/// in an `args` list).
fn is_ours(hook: &Value) -> bool {
    hook["command"].as_str().is_some_and(|c| c.contains(HOOK_ARG)) || hook["args"].as_array().is_some_and(|a| a.iter().any(|x| x.as_str() == Some(HOOK_ARG)))
}

/// The shell command of a hook: the program in quotes, then its arguments.
fn hook_command(exe: &str, args: &str) -> String {
    format!("\"{exe}\" {args}")
}

/// Settings with HubDeck's hooks added (`exe` = this program) or, with `exe` None, removed. Other
/// hooks and settings are kept as they are.
#[cfg(test)]
pub fn with_hooks(settings: Value, exe: Option<&str>) -> Value {
    with_hooks_approve(settings, exe, false)
}

/// The same, and with `approve` a synchronous `PermissionRequest` hook that asks on the deck.
pub fn with_hooks_approve(mut settings: Value, exe: Option<&str>, approve: bool) -> Value {
    if !settings.is_object() {
        settings = serde_json::json!({});
    }
    let hooks = settings.as_object_mut().expect("object").entry("hooks").or_insert_with(|| serde_json::json!({}));
    if !hooks.is_object() {
        *hooks = serde_json::json!({});
    }
    let hooks = hooks.as_object_mut().expect("object");
    // remove ours everywhere first, then add fresh entries
    for (_, groups) in hooks.iter_mut() {
        if let Some(groups) = groups.as_array_mut() {
            for g in groups.iter_mut() {
                if let Some(list) = g["hooks"].as_array_mut() {
                    list.retain(|h| !is_ours(h));
                }
            }
            groups.retain(|g| g["hooks"].as_array().is_none_or(|l| !l.is_empty()));
        }
    }
    hooks.retain(|_, g| g.as_array().is_none_or(|a| !a.is_empty()));
    if let Some(exe) = exe {
        for event in HOOK_EVENTS {
            let entry = serde_json::json!({ "hooks": [ { "type": "command", "command": hook_command(exe, HOOK_ARG), "async": true, "timeout": 5 } ] });
            if let Some(list) = hooks.entry(event).or_insert_with(|| Value::Array(Vec::new())).as_array_mut() {
                list.push(entry);
            }
        }
        if approve {
            // waits for a press (only for the safe tools; the others fall through at once)
            let entry = serde_json::json!({ "hooks": [ { "type": "command", "command": hook_command(exe, &format!("{HOOK_ARG} {APPROVE_ARG}")), "timeout": APPROVE_WAIT_S + 10 } ] });
            if let Some(list) = hooks.entry("PermissionRequest").or_insert_with(|| Value::Array(Vec::new())).as_array_mut() {
                list.push(entry);
            }
        }
    }
    if hooks.is_empty() {
        settings.as_object_mut().expect("object").remove("hooks");
    }
    settings
}

/// Whether the settings carry HubDeck's hooks.
pub fn has_hooks(settings: &Value) -> bool {
    settings["hooks"].as_object().is_some_and(|h| h.values().any(|g| g.as_array().is_some_and(|a| a.iter().any(|x| x["hooks"].as_array().is_some_and(|l| l.iter().any(is_ours))))))
}

/// `~/.claude/settings.json`.
pub fn claude_settings_path() -> Option<std::path::PathBuf> {
    #[cfg(windows)]
    let home = std::env::var_os("USERPROFILE");
    #[cfg(not(windows))]
    let home = std::env::var_os("HOME");
    home.map(|h| std::path::PathBuf::from(h).join(".claude").join("settings.json"))
}

/// Add (connect) or remove HubDeck's hooks in the user's Claude Code settings. A copy of the old
/// file is kept next to it as `settings.json.hubdeck-backup`.
/// `approve` adds the deck's answer to permission prompts of safe tools.
pub fn set_connected_with(connect: bool, approve: bool) -> Result<bool, String> {
    let path = claude_settings_path().ok_or("no home folder")?;
    let old = std::fs::read_to_string(&path).ok();
    let settings: Value = match &old {
        Some(text) if !text.trim().is_empty() => serde_json::from_str(text).map_err(|e| format!("{} is not valid JSON ({e}); nothing changed", path.display()))?,
        _ => serde_json::json!({}),
    };
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let next = with_hooks_approve(settings, connect.then(|| exe.to_str()).flatten(), approve);
    if let Some(text) = &old {
        std::fs::write(path.with_extension("json.hubdeck-backup"), text).map_err(|e| e.to_string())?;
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    super::profile::write_atomic(&path, serde_json::to_string_pretty(&next).map_err(|e| e.to_string())?.as_bytes()).map_err(|e| e.to_string())?;
    Ok(has_hooks(&next))
}

/// Whether the settings carry our approve hook.
pub fn approving() -> bool {
    claude_settings_path().and_then(|p| std::fs::read_to_string(p).ok()).is_some_and(|t| t.contains(APPROVE_ARG))
}

/// Whether Claude Code is connected (our hooks are in its settings).
pub fn connected() -> bool {
    claude_settings_path().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|t| serde_json::from_str::<Value>(&t).ok()).is_some_and(|s| has_hooks(&s))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn hook(session: &str, hook: &str, kind: Option<&str>) -> Value {
        json!({"session": session, "hook": hook, "cwd": format!("C:\\work\\{session}"), "kind": kind, "tool": "Bash"})
    }

    #[test]
    fn states_follow_the_hooks() {
        let t = Instant::now();
        let mut f = Fleet::default();
        assert!(f.report(&hook("alpha", "SessionStart", None), t));
        assert_eq!(f.by_slot(0).unwrap().state, State::Ready);
        f.report(&hook("alpha", "UserPromptSubmit", None), t);
        assert_eq!(f.by_slot(0).unwrap().state, State::Working);
        assert!(!f.report(&hook("alpha", "PreToolUse", None), t), "still working: nothing to redraw");
        f.report(&hook("alpha", "PermissionRequest", None), t);
        let s = f.by_slot(0).unwrap();
        assert_eq!((s.state, s.tool.as_deref(), s.project.as_str()), (State::Permission, Some("Bash"), "alpha"));
        assert!(f.pulsing());
        f.report(&hook("alpha", "Stop", None), t);
        assert_eq!(f.by_slot(0).unwrap().state, State::YourTurn);
        assert_eq!(f.waiting(), 1);
        assert!(f.report(&hook("alpha", "SessionEnd", None), t));
        assert!(f.list().is_empty());
        assert!(!f.report(&json!({"hook": "Stop"}), t), "no session id: ignored");
    }

    #[test]
    fn slots_stay_put() {
        let t = Instant::now();
        let mut f = Fleet::default();
        for s in ["a", "b", "c"] {
            f.report(&hook(s, "SessionStart", None), t);
        }
        f.report(&hook("b", "SessionEnd", None), t);
        f.report(&hook("d", "SessionStart", None), t);
        let slots: Vec<(String, usize)> = f.list().iter().map(|s| (s.project.clone(), s.slot)).collect();
        assert_eq!(slots, vec![("a".into(), 0), ("d".into(), 1), ("c".into(), 2)], "a new session takes the freed slot");
        assert!(f.forget_stale(t + FORGET_AFTER + Duration::from_secs(1)));
        assert!(f.list().is_empty());
    }

    #[test]
    fn hook_report_keeps_only_the_state() {
        let input = json!({"session_id": "s1", "hook_event_name": "UserPromptSubmit", "cwd": "/home/u/proj", "prompt": "secret plans", "tool_input": {"command": "rm"}});
        let r = hook_report(&input);
        assert!(!r.to_string().contains("secret"), "{r}");
        assert!(!r.to_string().contains("rm"), "{r}");
        assert_eq!(r["session"], "s1");
        assert_eq!(project_name("C:\\work\\masterhub\\"), "masterhub");
    }

    #[test]
    fn hooks_merge_into_settings_and_come_out_again() {
        let mine = json!({"type": "command", "command": "my-linter"});
        let settings = json!({"model": "opus", "hooks": {"Stop": [{"hooks": [mine.clone()]}]}});
        let on = with_hooks(settings.clone(), Some("C:\\HubDeck\\hubdeck.exe"));
        assert!(has_hooks(&on));
        assert_eq!(on["model"], "opus");
        assert_eq!(on["hooks"]["Stop"].as_array().unwrap().len(), 2, "the user's own Stop hook stays");
        assert_eq!(on["hooks"]["PreToolUse"][0]["hooks"][0]["async"], true);
        assert_eq!(on["hooks"]["Stop"][1]["hooks"][0]["command"], "\"C:\\HubDeck\\hubdeck.exe\" --claude-hook");
        let twice = with_hooks(on.clone(), Some("C:\\HubDeck\\hubdeck.exe"));
        assert_eq!(twice, on, "connecting again changes nothing");
        let off = with_hooks(on, None);
        assert!(!has_hooks(&off));
        assert_eq!(off, settings, "disconnecting restores the settings");
    }

    #[test]
    fn the_deck_answers_permission_only_for_safe_tools() {
        let read = json!({"hook_event_name": "PermissionRequest", "tool_name": "Read", "tool_input": {"file_path": "C:\\work\\app\\src\\main.rs"}});
        assert_eq!(approve_question(&read).as_deref(), Some("Read main.rs?"));
        assert_eq!(approve_question(&json!({"tool_name": "Bash", "tool_input": {"command": "rm -rf /"}})), None, "never for commands");
        assert_eq!(approve_answer(true)["hookSpecificOutput"]["decision"]["behavior"], "allow");
        assert_eq!(approve_answer(false)["hookSpecificOutput"]["decision"]["behavior"], "deny");
        // the approve hook is added only when asked for, and goes away with the rest
        let on = with_hooks_approve(json!({}), Some("C:\\HubDeck\\hubdeck.exe"), true);
        let permission = on["hooks"]["PermissionRequest"].as_array().unwrap();
        assert_eq!(permission.len(), 2);
        assert!(permission[1]["hooks"][0]["command"].as_str().unwrap().ends_with("--claude-hook --approve"));
        assert!(permission[1]["hooks"][0].get("async").is_none(), "it waits for the answer");
        assert!(!has_hooks(&with_hooks(on, None)));
        // entries of the older format (args) are still found and removed
        let old = json!({"hooks": {"Stop": [{"hooks": [{"type": "command", "command": "C:\\x\\hubdeck.exe", "args": ["--claude-hook"]}]}]}});
        assert!(has_hooks(&old) && !has_hooks(&with_hooks(old, None)));
    }

    #[test]
    fn waiting_keys_blink() {
        let t = Instant::now();
        let mut f = Fleet::default();
        f.report(&hook("x", "PermissionRequest", None), t);
        let a = f.key_look(0, &Look::default(), t, t);
        let b = f.key_look(0, &Look::default(), t + PULSE, t);
        assert_ne!(a.bg, b.bg);
        assert!(f.key_look(5, &Look::default(), t, t).label.unwrap().contains("Agent 6"));
    }
}
