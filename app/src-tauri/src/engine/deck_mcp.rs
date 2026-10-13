//! `hubdeck --mcp-deck`: tools that let an assistant use the deck while it works (roadmap phase 2):
//! a short notice with the base edge lit, a status on the dial, a question with its answers on the
//! keys (the user presses one), and the values the faders and knobs are set to. A separate server
//! from the profile builder, meant to be switched on per project (each tool costs context):
//! `claude mcp add --scope project hubdeck-deck -- <hubdeck> --mcp-deck`.

use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::mcp::Api;

pub const SERVER_NAME: &str = "hubdeck-deck";

pub const INSTRUCTIONS: &str = "The user's desk controller (HubDeck): a round dial screen, 15 keys with screens and a light strip on the base. Use notify for things the user should see when they look up (a long task finished, a test failed), show_status for what you are doing during a long task (clear it at the end), ask_physical to let the user answer a short question with one key press instead of typing, and read_controls to read faders and knobs the user set. Keep texts short: they show on small screens.";

pub fn tools() -> Vec<Value> {
    vec![
        json!({
            "name": "notify",
            "description": "Show a short message on the dial for some seconds and light the base edge, so the user notices it at a glance.",
            "inputSchema": {"type": "object", "properties": {
                "text": {"type": "string", "description": "Up to about 20 characters, e.g. \"Tests passed\"."},
                "detail": {"type": "string", "description": "An optional second line, short."},
                "color": {"type": "string", "description": "Edge color #rrggbb: green for good news, red for a failure, amber for attention."},
                "seconds": {"type": "integer", "description": "How long, 1-120 (default 8)."}
            }, "required": ["text"]}
        }),
        json!({
            "name": "show_status",
            "description": "Keep a status on the dial (what you are doing) until you change or clear it. An empty text clears it.",
            "inputSchema": {"type": "object", "properties": {
                "text": {"type": "string", "description": "Short, e.g. \"Refactoring 3/7\"; empty clears."},
                "detail": {"type": "string", "description": "An optional second line."}
            }}
        }),
        json!({
            "name": "ask_physical",
            "description": "Ask the user a short question on the deck: the question shows on the dial, each answer on a key; returns the answer the user pressed, or that nobody answered. Use for quick decisions (\"Deploy now?\" Yes / Later), never for anything destructive without a typed confirmation too.",
            "inputSchema": {"type": "object", "properties": {
                "question": {"type": "string", "description": "Short, it shows on the round dial."},
                "options": {"type": "array", "items": {"type": "string"}, "minItems": 1, "maxItems": 14, "description": "The answers, one per key, a word or two each."},
                "timeout_seconds": {"type": "integer", "description": "How long to wait, 5-600 (default 120)."}
            }, "required": ["question", "options"]}
        }),
        json!({
            "name": "read_controls",
            "description": "The active profile and page and the values (0-100) the faders, knobs and dial last set.",
            "inputSchema": {"type": "object", "properties": {}}
        }),
    ]
}

fn text(t: &str) -> Vec<Value> {
    vec![json!({"type": "text", "text": t})]
}

/// Run one deck tool.
pub fn call_tool(api: &dyn Api, name: &str, args: &Value) -> Result<Vec<Value>, String> {
    let pick = |keys: &[&str]| {
        let mut body = json!({});
        for k in keys {
            if let Some(v) = args.get(*k) {
                body[*k] = v.clone();
            }
        }
        body
    };
    match name {
        "notify" => {
            api.call("POST", "/api/notify", Some(&pick(&["text", "detail", "color", "seconds"])))?;
            Ok(text("shown on the deck"))
        }
        "show_status" => {
            let answer = api.call("POST", "/api/status", Some(&pick(&["text", "detail"])))?;
            Ok(text(if answer["cleared"].as_bool() == Some(true) { "status cleared" } else { "status shown on the dial" }))
        }
        "ask_physical" => {
            let seconds = args.get("timeout_seconds").and_then(Value::as_u64).unwrap_or(120).clamp(5, 600);
            let asked = api.call("POST", "/api/ask", Some(&json!({"question": args.get("question"), "options": args.get("options"), "seconds": seconds})))?;
            let id = asked["id"].as_u64().ok_or("HubDeck did not take the question")?;
            wait_for_answer(api, id, Duration::from_secs(seconds + 2), Duration::from_millis(400))
        }
        "read_controls" => Ok(vec![json!({"type": "text", "text": serde_json::to_string_pretty(&api.call("GET", "/api/controls", None)?).unwrap_or_default()})]),
        _ => Err(format!("unknown tool {name}")),
    }
}

/// Look at the question every `every` until it is answered or `limit` passes.
fn wait_for_answer(api: &dyn Api, id: u64, limit: Duration, every: Duration) -> Result<Vec<Value>, String> {
    let start = Instant::now();
    loop {
        let a = api.call("GET", &format!("/api/ask/{id}"), None)?;
        if a["waiting"].as_bool() != Some(true) {
            return Ok(text(&match (a["answered"].as_bool(), a["choice"].as_str()) {
                (Some(true), Some(choice)) => format!("The user pressed: {choice}"),
                _ => "No answer: the user dismissed the question or did not answer in time.".to_string(),
            }));
        }
        if start.elapsed() > limit {
            return Ok(text("No answer: the user did not answer in time."));
        }
        std::thread::sleep(every);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    /// A fake app whose question is answered on the third look.
    struct Fake {
        looks: Cell<u32>,
        sent: RefCell<Vec<(String, Value)>>,
    }

    impl Api for Fake {
        fn call(&self, method: &str, path: &str, body: Option<&Value>) -> Result<Value, String> {
            self.sent.borrow_mut().push((format!("{method} {path}"), body.cloned().unwrap_or(Value::Null)));
            Ok(match (method, path) {
                ("POST", "/api/ask") => json!({"id": 7}),
                ("GET", "/api/ask/7") => {
                    self.looks.set(self.looks.get() + 1);
                    if self.looks.get() < 3 { json!({"waiting": true}) } else { json!({"waiting": false, "answered": true, "index": 1, "choice": "Later"}) }
                }
                ("POST", "/api/status") => json!({"ok": true, "cleared": body.and_then(|b| b["text"].as_str()).is_none_or(str::is_empty)}),
                _ => json!({"ok": true}),
            })
        }
    }

    #[test]
    fn a_question_waits_for_the_press() {
        let f = Fake { looks: Cell::new(0), sent: RefCell::new(Vec::new()) };
        let out = wait_for_answer(&f, 7, Duration::from_secs(5), Duration::from_millis(1)).unwrap();
        assert_eq!(out[0]["text"], "The user pressed: Later");
        assert_eq!(f.looks.get(), 3);
    }

    #[test]
    fn tools_reach_their_routes() {
        let f = Fake { looks: Cell::new(0), sent: RefCell::new(Vec::new()) };
        call_tool(&f, "notify", &json!({"text": "Tests passed", "color": "#22c55e"})).unwrap();
        assert_eq!(f.sent.borrow()[0].0, "POST /api/notify");
        assert_eq!(f.sent.borrow()[0].1["color"], "#22c55e");
        assert_eq!(call_tool(&f, "show_status", &json!({"text": ""})).unwrap()[0]["text"], "status cleared");
        assert!(call_tool(&f, "nope", &json!({})).is_err());
        let names: Vec<String> = tools().iter().map(|t| t["name"].as_str().unwrap().to_string()).collect();
        assert_eq!(names, ["notify", "show_status", "ask_physical", "read_controls"]);
    }
}
