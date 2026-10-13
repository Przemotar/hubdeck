//! What the deck assistant may put on a control, enforced in code rather than in its prompt (both
//! design reviews, 2026-10-12): no commands, and shortcuts or actions that close, quit, delete or
//! clear things only on a hold, never on a plain press or a turn.

use serde_json::Value;

use super::profile::Binding;

/// Actions the assistant never puts on a control (the user can, in the editor).
const REFUSED: [(&str, &str); 2] = [("core.run", "running commands"), ("ha.service", "calling any Home Assistant service")];

/// Actions that destroy or clear something on their own.
const DESTRUCTIVE_ACTIONS: [&str; 1] = ["twitch.clearChat"];

/// The user let the assistant reach further (Settings: more power): then it may also put commands
/// on keys (`core.run`, for the user's own tools); set in the app and in the assistant's MCP server
/// (`HUBDECK_ASSISTANT_POWER`).
static POWER: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_power(on: bool) {
    POWER.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// Why the assistant may not put this action and params on `control` (None: fine). `apps` are
/// the profile's programs (their shortcut packs say which shortcuts close or delete).
fn refusal(control: &str, action: &str, params: &Value, apps: &[String], depth: usize) -> Option<String> {
    let power = POWER.load(std::sync::atomic::Ordering::Relaxed);
    if let Some((_, what)) = REFUSED.iter().find(|(a, _)| *a == action && !(power && *a == "core.run")) {
        return Some(format!("{control}: {action} is not for the assistant ({what}); leave it to the user"));
    }
    if action == "core.launch"
        && !power
        && let Some(why) = risky_target(super::actions::param_str(params, "target"))
    {
        return Some(format!("{control}: opening {why} is not for the assistant; leave it to the user"));
    }
    let on_hold = control.ends_with(":hold");
    if DESTRUCTIVE_ACTIONS.contains(&action) && !on_hold {
        return Some(format!("{control}: {action} clears or deletes something, so it goes on a hold ({control}:hold), never a plain press"));
    }
    if action == "core.hotkey" {
        for key in ["keys", "clockwise", "counterClockwise"] {
            if let Some(combo) = params.get(key).and_then(Value::as_str).filter(|c| !c.is_empty())
                && super::shortcuts::is_destructive(combo, apps)
                && (!on_hold || key != "keys")
            {
                return Some(format!("{control}: {combo} closes, quits or deletes something, so it goes on a hold ({}:hold), never a press or a turn", control.split(':').next().unwrap_or(control)));
            }
        }
    }
    if action == "core.text" && super::actions::param_str(params, "text").contains('\n') && !on_hold {
        return Some(format!("{control}: text with Enter in it sends or submits things, so it goes on a hold"));
    }
    // steps of a multi-action and jobs of a dial or knob with modes are checked the same way
    if depth < 4 {
        for list in ["steps", "modes"] {
            for step in params.get(list).and_then(Value::as_array).into_iter().flatten() {
                let a = step.get("action").and_then(Value::as_str).unwrap_or_default();
                let p = step.get("params").cloned().unwrap_or(Value::Null);
                if let Some(r) = refusal(control, a, &p, apps, depth + 1) {
                    return Some(r);
                }
            }
        }
    }
    None
}

/// What the assistant may not open with "Open app, file or website" (release audit, 2026-10-13: on
/// Windows it starts anything): scripts and installers, and links of schemes that are not web pages
/// or the common apps' own. Apps, documents, folders and web pages are fine.
pub fn risky_target(target: &str) -> Option<&'static str> {
    let t = target.trim().trim_matches('"').to_lowercase();
    const SCRIPTS: [&str; 16] = [".bat", ".cmd", ".ps1", ".vbs", ".vbe", ".js", ".jse", ".wsf", ".wsh", ".hta", ".msi", ".scr", ".sh", ".command", ".scpt", ".py"];
    if SCRIPTS.iter().any(|e| t.ends_with(e)) {
        return Some("a script or an installer");
    }
    // `C:\...` is a drive, not a scheme
    if let Some((scheme, _)) = t.split_once(':')
        && scheme.len() > 1
        && scheme.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        && !matches!(scheme, "http" | "https" | "mailto" | "steam" | "spotify" | "discord" | "zoommtg" | "msteams" | "slack" | "notion" | "figma" | "obsidian" | "ms-settings")
    {
        return Some("a link of that kind");
    }
    None
}

/// Check one binding the assistant wants on `control`.
pub fn check(control: &str, b: &Binding, apps: &[String]) -> Result<(), String> {
    refusal(control, &b.action, &b.params, apps, 0).map_or(Ok(()), Err)
}

/// The same for a binding as JSON (`{action, params}`), as tools and sketches send it.
pub fn check_json(control: &str, b: &Value, apps: &[String]) -> Result<(), String> {
    let action = b.get("action").and_then(Value::as_str).unwrap_or_default();
    refusal(control, action, b.get("params").unwrap_or(&Value::Null), apps, 0).map_or(Ok(()), Err)
}

/// Shortcuts on a binding that are not in a checked pack for `apps` (shown with a mark).
pub fn unchecked_shortcuts(action: &str, params: &Value, apps: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    if action == "core.hotkey" {
        for key in ["keys", "clockwise", "counterClockwise"] {
            if let Some(combo) = params.get(key).and_then(Value::as_str).filter(|c| !c.is_empty())
                && !super::shortcuts::is_checked(combo, apps)
            {
                out.push(combo.to_string());
            }
        }
    }
    for list in ["steps", "modes"] {
        for step in params.get(list).and_then(Value::as_array).into_iter().flatten() {
            out.extend(unchecked_shortcuts(step.get("action").and_then(Value::as_str).unwrap_or_default(), step.get("params").unwrap_or(&Value::Null), apps));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_assistant_opens_apps_and_pages_but_not_scripts() {
        let launch = |t: &str| check_json("keys.0", &json!({"action": "core.launch", "params": {"target": t}}), &[]);
        assert!(launch("C:\\Program Files\\Spotify\\Spotify.exe").is_ok());
        assert!(launch("/Applications/Spotify.app").is_ok());
        assert!(launch("https://example.com").is_ok());
        assert!(launch("steam://rungameid/1").is_ok());
        assert!(launch("C:\\Users\\me\\run.bat").is_err());
        assert!(launch("search-ms:query=x").is_err());
        assert!(launch("file:///C:/x.exe").is_err());
    }

    #[test]
    fn the_assistant_keeps_closing_shortcuts_and_commands_off_plain_presses() {
        let none: Vec<String> = Vec::new();
        assert!(check_json("keys.0", &json!({"action": "core.run", "params": {"command": "x"}}), &none).is_err());
        assert!(check_json("keys.0", &json!({"action": "core.hotkey", "params": {"keys": "Alt+F4"}}), &none).is_err());
        assert!(check_json("keys.0:hold", &json!({"action": "core.hotkey", "params": {"keys": "Alt+F4"}}), &none).is_ok());
        assert!(check_json("keys.0", &json!({"action": "core.hotkey", "params": {"keys": "Ctrl+Z"}}), &none).is_ok());
        assert!(check_json("knobs.0", &json!({"action": "core.hotkey", "params": {"clockwise": "Ctrl+W"}}), &none).is_err());
        let zoom = vec!["Zoom.exe".to_string()];
        assert!(check_json("keys.1", &json!({"action": "core.hotkey", "params": {"keys": "Alt+Q"}}), &zoom).is_err(), "Zoom's end meeting");
        assert!(check_json("keys.1", &json!({"action": "core.hotkey", "params": {"keys": "Alt+Q"}}), &none).is_ok());
        let nested = json!({"action": "core.multi", "params": {"steps": [{"action": "core.media", "params": {}}, {"action": "core.hotkey", "params": {"keys": "Ctrl+Q"}}]}});
        assert!(check_json("keys.2", &nested, &none).is_err(), "a step of a multi-action");
        assert!(check_json("keys.3", &json!({"action": "core.text", "params": {"text": "gg\n"}}), &none).is_err());
        assert!(check_json("keys.3", &json!({"action": "twitch.clearChat"}), &none).is_err());
    }

    #[test]
    fn shortcuts_outside_the_packs_are_listed() {
        let spotify = vec!["Spotify.exe".to_string()];
        assert!(unchecked_shortcuts("core.hotkey", &json!({"keys": "Alt+Shift+B"}), &spotify).is_empty());
        assert!(unchecked_shortcuts("core.hotkey", &json!({"keys": "Win+D"}), &spotify).is_empty(), "Windows' own work everywhere");
        assert_eq!(unchecked_shortcuts("core.hotkey", &json!({"keys": "Ctrl+Alt+P"}), &spotify), ["Ctrl+Alt+P"]);
    }
}
