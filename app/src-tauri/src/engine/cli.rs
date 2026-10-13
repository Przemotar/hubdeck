//! `hubdeck cli ...`: HubDeck from a terminal or a script, through the running app's local API
//! (roadmap phase 2, "CLI on top of the local API"). It changes nothing the app's own window
//! could not, and prints plain lines.
//!
//! ```text
//! hubdeck cli status                     the active and home profile, the modules
//! hubdeck cli profiles                   every profile (* active, ^ home)
//! hubdeck cli profile <name or id>       show that profile
//! hubdeck cli key <1-15> [--label T] [--icon NAME] [--page ID]   change a key's look in the active profile
//! hubdeck cli lights <rainbow|color|profile|off> [#rrggbb]        the deck-wide light style
//! ```

use serde_json::{Value, json};

use super::mcp::{Api, Http};

const HELP: &str = "hubdeck cli status | profiles | profile <name or id> | key <1-15> [--label TEXT] [--icon NAME] [--page ID] | lights <rainbow|color|profile|off> [#rrggbb]";

/// Run one command; the text to print, or an error.
pub fn command(api: &dyn Api, args: &[String]) -> Result<String, String> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(String::as_str);
    match arg(0) {
        Some("status") => {
            let p = api.call("GET", "/api/profiles", None)?;
            let m = api.call("GET", "/api/modules", None)?;
            let name = |id: &Value| p["profiles"].as_array().and_then(|l| l.iter().find(|x| x["id"] == *id)).and_then(|x| x["name"].as_str()).unwrap_or("-").to_string();
            let attached: Vec<String> = m["attached"].as_array().map(|a| a.iter().filter_map(|k| k.as_str().map(str::to_string)).collect()).unwrap_or_default();
            Ok(format!("active: {}\nhome: {}\nmodules: {}", name(&p["active"]), name(&p["home"]), if attached.is_empty() { "none attached".into() } else { attached.join(", ") }))
        }
        Some("profiles") => {
            let p = api.call("GET", "/api/profiles", None)?;
            let list = p["profiles"].as_array().cloned().unwrap_or_default();
            Ok(list
                .iter()
                .map(|x| {
                    let mark = if x["id"] == p["active"] {
                        "*"
                    } else if x["id"] == p["home"] {
                        "^"
                    } else {
                        " "
                    };
                    format!("{mark} {}  ({})", x["name"].as_str().unwrap_or(""), x["id"].as_str().unwrap_or(""))
                })
                .collect::<Vec<_>>()
                .join("\n"))
        }
        Some("profile") => {
            let wanted = args[1..].join(" ");
            if wanted.trim().is_empty() {
                return Err("which profile? hubdeck cli profile <name or id>".into());
            }
            let p = api.call("GET", "/api/profiles", None)?;
            let list = p["profiles"].as_array().cloned().unwrap_or_default();
            let found = list
                .iter()
                .find(|x| x["id"].as_str() == Some(wanted.as_str()))
                .or_else(|| list.iter().find(|x| x["name"].as_str().is_some_and(|n| n.eq_ignore_ascii_case(wanted.trim()))))
                .ok_or_else(|| format!("no profile named {wanted:?} (hubdeck cli profiles lists them)"))?;
            let id = found["id"].as_str().unwrap_or_default();
            api.call("POST", &format!("/api/profiles/{id}/activate"), Some(&json!({})))?;
            Ok(format!("showing {}", found["name"].as_str().unwrap_or(id)))
        }
        Some("key") => {
            let n: usize = arg(1).and_then(|n| n.parse().ok()).filter(|n| (1..=15).contains(n)).ok_or("which key? 1 to 15, left to right, top to bottom")?;
            let (label, icon) = (flag("--label"), flag("--icon"));
            if label.is_none() && icon.is_none() {
                return Err("nothing to change: give --label and/or --icon".into());
            }
            let p = api.call("GET", "/api/profiles", None)?;
            let id = p["active"].as_str().ok_or("no profile is active")?.to_string();
            let control = match flag("--page") {
                Some(page) => format!("{page}/keys.{}", n - 1),
                None => format!("keys.{}", n - 1),
            };
            let profile = api.call("GET", &format!("/api/profiles/{id}"), None)?;
            let mut binding = profile["controls"].get(&control).cloned().unwrap_or_else(|| json!({"action": "", "params": {}, "look": {}}));
            if let Some(l) = label {
                binding["look"]["label"] = json!(l);
            }
            if let Some(i) = icon {
                binding["look"]["icon"] = json!(i);
                binding["look"]["image"] = Value::Null;
            }
            let answer = api.call("POST", &format!("/api/profiles/{id}/controls"), Some(&json!({"control": control, "binding": binding})))?;
            let warnings = answer["warnings"].as_array().map(|w| w.iter().filter_map(Value::as_str).collect::<Vec<_>>().join("; ")).unwrap_or_default();
            Ok(if warnings.is_empty() { format!("key {n} changed") } else { format!("key {n} changed ({warnings})") })
        }
        Some("lights") => {
            let style = arg(1).filter(|s| matches!(*s, "rainbow" | "color" | "profile" | "off")).ok_or("lights rainbow, color, profile or off")?;
            let mut body = json!({ "style": style });
            if let Some(c) = arg(2) {
                body["color"] = json!(c);
            }
            api.call("POST", "/api/lights", Some(&body))?;
            Ok(format!("lights: {style}"))
        }
        _ => Ok(HELP.into()),
    }
}

/// `hubdeck cli ...` from `main`: print the answer (on Windows the release build has no console
/// of its own, so it writes into the terminal it was started from).
pub fn run(args: &[String]) -> i32 {
    attach_console();
    let Some(dir) = super::agents::data_dir() else {
        eprintln!("hubdeck cli: no app data folder");
        return 1;
    };
    match command(&Http { dir }, args) {
        Ok(text) => {
            println!("{text}");
            0
        }
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}

#[cfg(windows)]
fn attach_console() {
    use windows::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};
    // SAFETY: no arguments to get wrong; failing (no parent console) just keeps the output away.
    let _ = unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
}

#[cfg(not(windows))]
fn attach_console() {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// A fake app: answers from a script and records what was sent.
    struct Fake {
        sent: RefCell<Vec<(String, String, Option<Value>)>>,
    }

    impl Api for Fake {
        fn call(&self, method: &str, path: &str, body: Option<&Value>) -> Result<Value, String> {
            self.sent.borrow_mut().push((method.into(), path.into(), body.cloned()));
            Ok(match (method, path) {
                ("GET", "/api/profiles") => json!({"active": "a", "home": "b", "profiles": [{"id": "a", "name": "Desk"}, {"id": "b", "name": "Home"}]}),
                ("GET", "/api/modules") => json!({"attached": ["keys", "dial"]}),
                ("GET", "/api/profiles/a") => json!({"controls": {"keys.2": {"action": "core.media", "params": {"command": "next"}, "look": {"label": "Next"}}}}),
                _ => json!({"ok": true}),
            })
        }
    }

    type Sent = Vec<(String, String, Option<Value>)>;

    fn run(args: &[&str]) -> (Result<String, String>, Sent) {
        let fake = Fake { sent: RefCell::new(Vec::new()) };
        let out = command(&fake, &args.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        (out, fake.sent.into_inner())
    }

    #[test]
    fn commands_talk_to_the_api() {
        assert_eq!(run(&["status"]).0.unwrap(), "active: Desk\nhome: Home\nmodules: keys, dial");
        assert!(run(&["profiles"]).0.unwrap().starts_with("* Desk"));
        let (out, sent) = run(&["profile", "home"]);
        assert_eq!(out.unwrap(), "showing Home");
        assert_eq!(sent.last().unwrap().1, "/api/profiles/b/activate");
        assert!(run(&["profile", "nope"]).0.is_err());
        // a key keeps its action, gets the new label
        let (_, sent) = run(&["key", "3", "--label", "Skip"]);
        let body = sent.last().unwrap().2.clone().unwrap();
        assert_eq!(body["control"], "keys.2");
        assert_eq!(body["binding"]["action"], "core.media");
        assert_eq!(body["binding"]["look"]["label"], "Skip");
        assert!(run(&["key", "16", "--label", "x"]).0.is_err());
        assert_eq!(run(&["lights", "off"]).0.unwrap(), "lights: off");
        assert_eq!(run(&[]).0.unwrap(), HELP);
    }
}
