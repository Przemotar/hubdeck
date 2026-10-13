//! `hubdeck --mcp`: a Model Context Protocol server on stdin/stdout (JSON-RPC 2.0, one message a
//! line) so the user's own AI assistant (Claude Desktop, Claude Code, any MCP client) can design
//! and build profiles. It is a thin client of the running app's local HTTP API (`api.rs`, token
//! and port in the app data folder): no keys, no model, nothing leaves the computer through
//! HubDeck. Also here: connecting Claude Desktop and Claude Code to it (Settings > Connections).
//! See `docs/MCP.md`.

use std::io::{BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;
use serde_json::{Value, json};

/// Protocol versions this server speaks, newest first; a client asking for another gets the newest.
pub const PROTOCOL_VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

/// Name of the server in the clients' configuration.
pub const SERVER_NAME: &str = "hubdeck";

/// Rules for good profiles; served as a resource and by the `design_guide` tool.
pub const DESIGN_GUIDE: &str = include_str!("../../../../docs/PROFILE-DESIGN.md");
const GUIDE_URI: &str = "hubdeck://design-guide";

const TOO_OLD: &str = "The HubDeck that is running is too old for the assistant tools. Ask the user to update HubDeck (or restart it after an update).";
const NOT_RUNNING: &str = "HubDeck is not running, so its profiles cannot be read or changed. Ask the user to start HubDeck (it lives in the system tray / menu bar) and try again.";

/// How the server reaches the app: `(method, path, body)` -> the JSON answer or an error text.
pub trait Api {
    fn call(&self, method: &str, path: &str, body: Option<&Value>) -> Result<Value, String>;
}

/// The running app over its local HTTP API; the token and port are read on every call, so a
/// restarted app (new port) is found again.
pub struct Http {
    pub dir: PathBuf,
}

impl Api for Http {
    fn call(&self, method: &str, path: &str, body: Option<&Value>) -> Result<Value, String> {
        let token = std::fs::read_to_string(self.dir.join("api-token")).map_err(|_| NOT_RUNNING.to_string())?;
        let port: u16 = std::fs::read_to_string(self.dir.join("api-http-port")).ok().and_then(|p| p.trim().parse().ok()).ok_or_else(|| NOT_RUNNING.to_string())?;
        let (status, answer) = request(port, token.trim(), method, path, body).map_err(|_| NOT_RUNNING.to_string())?;
        // an older HubDeck takes unknown POSTs as plugin events (202) and answers other paths with
        // its usage line (404)
        let usage = answer.get("error").and_then(Value::as_str).is_some_and(|e| e.starts_with("use GET /api/state"));
        match status {
            202 => Err(TOO_OLD.into()),
            404 if usage || answer.get("error").is_none() => Err(TOO_OLD.into()),
            200..=299 => Ok(answer),
            401 => Err("HubDeck refused the request (the API token changed); restart HubDeck and try again.".into()),
            _ => Err(answer.get("error").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| format!("HubDeck answered {status}"))),
        }
    }
}

/// One HTTP/1.1 request to 127.0.0.1 (`Connection: close`): the status and the JSON body.
pub fn request(port: u16, token: &str, method: &str, path: &str, body: Option<&Value>) -> std::io::Result<(u16, Value)> {
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let mut stream = std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(800))?;
    // pictures and animations take a few seconds to make
    stream.set_read_timeout(Some(Duration::from_secs(90)))?;
    let body = body.map(Value::to_string).unwrap_or_default();
    let head = format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
    stream.write_all(head.as_bytes())?;
    stream.write_all(body.as_bytes())?;
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw)?;
    let text = String::from_utf8_lossy(&raw);
    let (head, body) = text.split_once("\r\n\r\n").ok_or_else(|| std::io::Error::other("no HTTP answer"))?;
    let status = head.split_whitespace().nth(1).and_then(|s| s.parse().ok()).ok_or_else(|| std::io::Error::other("no HTTP status"))?;
    // tiny_http sends bigger answers (the catalog, 40 KB+) in chunks: without decoding them the
    // JSON did not parse and list_actions / list_icons gave the assistant empty lists
    let chunked = head.lines().any(|l| l.to_ascii_lowercase().replace(' ', "") == "transfer-encoding:chunked");
    let body = if chunked { dechunk(body) } else { body.to_string() };
    Ok((status, serde_json::from_str(&body).unwrap_or(Value::Null)))
}

/// The body of a `Transfer-Encoding: chunked` answer: hex size, CRLF, data, CRLF ... `0`.
fn dechunk(mut rest: &str) -> String {
    let mut out = String::new();
    while let Some((size, after)) = rest.split_once("\r\n") {
        let Ok(n) = usize::from_str_radix(size.split(';').next().unwrap_or("").trim(), 16) else { break };
        if n == 0 || after.len() < n {
            if n > 0 {
                out.push_str(after);
            }
            break;
        }
        out.push_str(&after[..n]);
        rest = after[n..].strip_prefix("\r\n").unwrap_or(&after[n..]);
    }
    out
}

/// Which tools a server offers: the profile builder (`--mcp`) or the deck itself (`--mcp-deck`:
/// notify, status, questions on the keys, control values; meant to be switched on per project).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    Builder,
    Deck,
    /// HubDeck's own deck assistant (`--mcp-assistant`): the builder's tools limited to drafts,
    /// plus ideas, answer buttons, taste notes and the checked shortcut packs.
    Assistant,
}

/// Entry point of `hubdeck --mcp`: serve until stdin closes. Only protocol messages go to stdout.
pub fn run() {
    serve(Kind::Builder)
}

/// Entry point of `hubdeck --mcp-deck`.
pub fn run_deck() {
    serve(Kind::Deck)
}

/// Entry point of `hubdeck --mcp-assistant` (started by the deck assistant's Claude Code).
pub fn run_assistant() {
    super::guard::set_power(std::env::var_os("HUBDECK_ASSISTANT_POWER").is_some());
    serve(Kind::Assistant)
}

fn serve(kind: Kind) {
    let Some(dir) = super::agents::data_dir() else {
        eprintln!("hubdeck --mcp: no app data folder");
        return;
    };
    let api = Http { dir };
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(msg) => handle_message_for(kind, &api, &msg),
            Err(e) => Some(error(Value::Null, -32700, &format!("parse error: {e}"))),
        };
        if let Some(reply) = reply
            && writeln!(stdout, "{reply}").and_then(|_| stdout.flush()).is_err()
        {
            break;
        }
    }
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

fn result(id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

/// Answer one JSON-RPC message for the profile builder; None for notifications and responses.
#[cfg(test)]
pub fn handle_message(api: &dyn Api, msg: &Value) -> Option<Value> {
    handle_message_for(Kind::Builder, api, msg)
}

/// Answer one JSON-RPC message for a server of `kind`.
pub fn handle_message_for(kind: Kind, api: &dyn Api, msg: &Value) -> Option<Value> {
    let (tools, server_name, instructions): (fn() -> Vec<Value>, &str, &str) = match kind {
        Kind::Builder => (tools, SERVER_NAME, INSTRUCTIONS),
        Kind::Deck => (super::deck_mcp::tools, super::deck_mcp::SERVER_NAME, super::deck_mcp::INSTRUCTIONS),
        Kind::Assistant => (assistant_tools, SERVER_NAME, ASSISTANT_INSTRUCTIONS),
    };
    let method = msg.get("method").and_then(Value::as_str);
    let id = msg.get("id").cloned();
    let (Some(method), Some(id)) = (method, id) else {
        // a notification (`notifications/initialized`, `notifications/cancelled`) or a response
        return (msg.get("method").is_none() && msg.get("id").is_none()).then(|| error(Value::Null, -32600, "invalid request"));
    };
    let params = msg.get("params").cloned().unwrap_or(json!({}));
    Some(match method {
        "initialize" => {
            let asked = params.get("protocolVersion").and_then(Value::as_str).unwrap_or("");
            let version = PROTOCOL_VERSIONS.iter().find(|v| **v == asked).unwrap_or(&PROTOCOL_VERSIONS[0]);
            result(
                id,
                json!({
                    "protocolVersion": version,
                    "capabilities": {"tools": {"listChanged": false}, "resources": {"listChanged": false}},
                    "serverInfo": {"name": server_name, "title": "HubDeck", "version": env!("CARGO_PKG_VERSION")},
                    "instructions": instructions,
                }),
            )
        }
        "ping" => result(id, json!({})),
        "tools/list" => result(id, json!({ "tools": tools() })),
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            if !tools().iter().any(|t| t["name"] == name) {
                return Some(error(id, -32602, &format!("unknown tool {name:?}")));
            }
            let args = params.get("arguments").cloned().filter(Value::is_object).unwrap_or(json!({}));
            result(
                id,
                match match kind {
                    Kind::Builder => call_tool(api, name, &args),
                    Kind::Deck => super::deck_mcp::call_tool(api, name, &args),
                    Kind::Assistant => assistant_call(api, name, &args),
                } {
                    Ok(content) => json!({ "content": content, "isError": false }),
                    Err(e) => json!({ "content": [{"type": "text", "text": e}], "isError": true }),
                },
            )
        }
        "resources/list" => result(id, json!({"resources": [{"uri": GUIDE_URI, "name": "design-guide", "title": "How to design great HubDeck profiles", "mimeType": "text/markdown"}]})),
        "resources/read" => match params.get("uri").and_then(Value::as_str) {
            Some(GUIDE_URI) => result(id, json!({"contents": [{"uri": GUIDE_URI, "mimeType": "text/markdown", "text": DESIGN_GUIDE}]})),
            _ => error(id, -32002, "resource not found"),
        },
        "resources/templates/list" => result(id, json!({"resourceTemplates": []})),
        "prompts/list" => result(id, json!({"prompts": []})),
        _ => error(id, -32601, &format!("method not found: {method}")),
    })
}

const INSTRUCTIONS: &str = "HubDeck drives a modular desk controller (Cooler Master MasterHUB modules): a 15-key module with screens, a round dial screen, 3 knobs, 5 faders and 2 rollers, plus the base's edge light. These tools let you design and build profiles for the user. Start with design_guide (how to make a profile that feels polished) and list_modules (build for the modules the user owns). Then look up list_actions, list_icons, list_apps or list_games as needed, build with create_profile (whole profile in one call) and refine with set_control, set_lights, add_page, add_image and add_animation; check keys with preview_control. Never replace or delete one of the user's profiles without asking first. HubDeck must be running.";

/// The binding schema, shared by the tools that take one.
fn binding_schema() -> Value {
    let look = json!({
        "type": "object",
        "description": "How the control looks. Keys and the dial draw label, icon or image on bg in fg; knobs and faders only use led, ledEffect, ledSpeed and ledLevel.",
        "properties": {
            "label": {"type": "string", "description": "Short text, 10 characters a line or fewer; a newline makes a second line."},
            "icon": {"type": "string", "description": "A built-in icon name, or game:<name> for a game icon (list_icons)."},
            "image": {"type": "string", "description": "A picture name returned by add_image or add_animation; replaces the icon. Animated GIFs move."},
            "bg": {"type": "string", "description": "Background #rrggbb."},
            "fg": {"type": "string", "description": "Icon and text #rrggbb."},
            "style": {"type": "string", "enum": crate::engine::api::KEY_STYLES, "description": "Key background: flat, gradient, glass (a highlight like the hardware caps) or outline."},
            "led": {"type": "string", "description": "Knob or fader LED #rrggbb."},
            "ledEffect": {"type": "string", "description": "static, breathing, strobe, blink, cycling, random, wave, water, rainbow."},
            "ledSpeed": {"type": "integer", "minimum": 0, "maximum": 100},
            "ledLevel": {"type": "boolean", "description": "LED brightness follows the value (fader position, volume)."}
        }
    });
    json!({
        "type": "object",
        "required": ["action"],
        "description": "What one control does: an action from list_actions with its params, and its look.",
        "properties": {
            "action": {"type": "string", "description": "Action id, e.g. core.hotkey, core.volume, core.media, core.launch, core.page, core.modes. Empty string = a key that only shows something."},
            "params": {"type": "object", "description": "The action's params by key (list_actions with search gives them). Hotkeys are written like Ctrl+Shift+M, F13, VolumeUp; params.mode \"game\" sends them the way games read them. A turn action on a key needs params.turnOnPress 1 or -1."},
            "look": look.clone(),
            "lookOn": look,
            "accel": {"type": "boolean", "description": "Knobs, rollers, the dial: fast turns count double."}
        }
    })
}

const CONTROL_HELP: &str = "Control id: keys.0 .. keys.14 (5 per row, 3 rows, from the top left), dial, knobs.0 .. knobs.2, faders.0 .. faders.4, rollers.0 .. rollers.1. Add :hold or :double for a hold (500 ms) or double press of keys, knobs and the dial (keys.3:hold). Keys on an extra page: <page id>/keys.N (add_page first).";

fn tool(name: &str, title: &str, description: &str, properties: Value, required: &[&str], read_only: bool) -> Value {
    json!({
        "name": name,
        "title": title,
        "description": description,
        "inputSchema": {"type": "object", "properties": properties, "required": required},
        "annotations": {"readOnlyHint": read_only, "destructiveHint": false, "openWorldHint": false}
    })
}

/// Every tool with its description and input schema.
pub fn tools() -> Vec<Value> {
    let search = json!({"type": "string", "description": "Only entries containing this text (case does not matter)."});
    let profile_fields = json!({
        "name": {"type": "string", "description": "Short and clear, e.g. \"Photoshop\", \"Streaming\"."},
        "apps": {"type": "array", "items": {"type": "string"}, "description": "Executable names that switch to this profile while in front (Photoshop.exe, obs64.exe); see list_apps."},
        "controls": {"type": "object", "additionalProperties": binding_schema(), "description": format!("Control id -> binding. {CONTROL_HELP}")},
        "pages": {"type": "array", "items": {"type": "object", "properties": {"id": {"type": "string"}, "name": {"type": "string"}}, "required": ["id", "name"]}, "description": "Extra key pages {id, name}: ids are short words (\"tools\"); a core.page key with params.page = id opens one, params.page \"\" goes back."},
        "color": {"type": "string", "description": "The profile's color #rrggbb (used by the \"profile color\" light style)."},
        "edge": {"type": "string", "description": "Base edge light #rrggbb while the profile is active."},
        "edgeEffect": {"type": "string", "description": "Edge light effect (see ledEffect)."},
        "edgeSpeed": {"type": "integer", "minimum": 0, "maximum": 100},
        "panel": {"type": "object", "description": "One picture or a clock across all 15 keys (keys still act): {kind: \"picture\", image: <name>} (480x272), {kind: \"clock\"} or {kind: \"graph\", metric: \"cpu\"|\"ram\"|\"gpu\"|\"gpuTemp\"|\"vram\"}."}
    });
    let mut create = profile_fields.clone();
    create["replace"] = json!({"type": "string", "description": "Id of an existing profile to overwrite completely (a backup is made). Only after the user said yes."});
    create["draft"] = json!({"type": "boolean", "description": "A proposal for the user to try: not in the top bar, never switched to by its app, until the user picks it (Use this) or throws it away. The deck assistant makes its versions as drafts."});
    let mut update = profile_fields;
    if let Some(o) = update.as_object_mut() {
        o.remove("controls");
        o.remove("pages");
        o.insert("id".into(), json!({"type": "string"}));
    }
    vec![
        tool(
            "design_guide",
            "Profile design guide",
            "The rules for a polished HubDeck profile: layout by finger reach and frequency, color groups, icon style, short labels, contrast, one accent color, LED meanings, turn vs press vs hold, pictures and animations. Read it before designing.",
            json!({}),
            &[],
            true,
        ),
        tool(
            "list_modules",
            "List modules",
            "Module kinds the user has attached now and owns (attached or not), and the controls of each kind. Design for the owned kinds; a profile may also fill kinds the user does not own, but they will not be seen.",
            json!({}),
            &[],
            true,
        ),
        tool(
            "list_actions",
            "List actions",
            "What controls can do. Without search: every action's id, name, group, inputs (press, turn, slide) and description, plus hotkey special keys, key styles and LED effects. With search (an id, a group like OBS, or a word): those actions with their params (key, type, options, default, help). Fit actions to inputs: keys press, knobs and the dial turn and press, faders slide, rollers turn.",
            json!({"search": search, "details": {"type": "boolean", "description": "Params of every action even without search (long)."}}),
            &[],
            true,
        ),
        tool(
            "list_icons",
            "List icons",
            "Icon names for look.icon: built-in line icons (Lucide style) and game icons (game:<name>). Keep one icon family per profile.",
            json!({"search": search}),
            &[],
            true,
        ),
        tool(
            "list_apps",
            "List apps",
            "Apps installed on this computer (name, target to open with core.launch, exe for profile apps and per-app volume) and apps seen in front recently.",
            json!({"search": search}),
            &[],
            true,
        ),
        tool(
            "list_games",
            "List games",
            "Games installed through Steam, Epic or GOG: name and target (use core.game with params.game = target; HubDeck shows the cover on the key).",
            json!({"search": search}),
            &[],
            true,
        ),
        tool("list_profiles", "List profiles", "The user's profiles (id, name, linked apps), the active one and the home one.", json!({}), &[], true),
        tool("get_profile", "Get profile", "One profile with all its controls, pages and lights.", json!({"id": {"type": "string"}}), &["id"], true),
        tool(
            "create_profile",
            "Create profile",
            &format!(
                "Create a whole profile in one call: name, apps, controls, pages, colors. Everything is checked first (unknown actions, icons, pictures, colors, control ids or inputs that do not fit are refused with the reason); warnings come back for long labels and the like. Returns the profile with its id. Never set replace without asking the user. {CONTROL_HELP}"
            ),
            create,
            &["name"],
            false,
        ),
        tool("update_profile", "Update profile", "Change a profile's name, apps, colors, edge light or panel; its controls stay.", update, &["id"], false),
        tool(
            "set_control",
            "Set control",
            &format!(
                "Set what one control does and how it looks (replaces that control's binding). {CONTROL_HELP} Knobs and the dial can hold several turn jobs with core.modes (params.modes: [{{label, action, params}}], a press goes to the next). Multi-steps: core.multi (params.steps: [{{action, params, delay}}])."
            ),
            json!({"profile": {"type": "string", "description": "Profile id."}, "control": {"type": "string"}, "binding": binding_schema()}),
            &["profile", "control", "binding"],
            false,
        ),
        tool(
            "clear_control",
            "Clear control",
            "Remove what one control does (the control becomes empty; its hold and double press stay).",
            json!({"profile": {"type": "string"}, "control": {"type": "string"}}),
            &["profile", "control"],
            false,
        ),
        tool(
            "add_page",
            "Add page",
            "Add a page of 15 more keys to a profile (only keys change with the page). Then put keys on <page id>/keys.N, a core.page key on the main page with params.page = the page id, and a way back on the page (core.page with params.page \"\"), usually keys.14.",
            json!({"profile": {"type": "string"}, "name": {"type": "string"}, "id": {"type": "string", "description": "Optional short id (letters, digits, - _)."}}),
            &["profile", "name"],
            false,
        ),
        tool(
            "delete_page",
            "Delete page",
            "Remove a page and its keys. Ask the user first unless you made it in this conversation.",
            json!({"profile": {"type": "string"}, "page": {"type": "string"}}),
            &["profile", "page"],
            false,
        ),
        tool(
            "set_lights",
            "Set profile lights",
            "A profile's lights: edge (base edge #rrggbb), edgeEffect, edgeSpeed, color (the profile's color), and controls: {\"knobs.0\": {led, ledEffect, ledSpeed, ledLevel}} for knob and fader LEDs. Keep meanings consistent: red = live or muted, orange = needs attention, green = ok.",
            json!({"profile": {"type": "string"}, "edge": {"type": "string"}, "edgeEffect": {"type": "string"}, "edgeSpeed": {"type": "integer"}, "color": {"type": "string"}, "controls": {"type": "object"}}),
            &["profile"],
            false,
        ),
        tool(
            "get_deck_lights",
            "Get deck lights",
            "The deck-wide lighting: style (rainbow, color, profile, off), color, brightness 0-100 and reactions (mic muted, live recording or streaming, an agent needs the user, timer done; each {on, color}; levels = volume LEDs show their level).",
            json!({}),
            &[],
            true,
        ),
        tool(
            "set_deck_lights",
            "Set deck lights",
            "Change the deck-wide lighting for every profile (only the fields given). This affects all profiles: ask the user first.",
            json!({"style": {"type": "string", "enum": ["rainbow", "color", "profile", "off"]}, "color": {"type": "string"}, "brightness": {"type": "integer", "minimum": 0, "maximum": 100}, "react": {"type": "object"}}),
            &[],
            false,
        ),
        tool(
            "add_image",
            "Add image",
            "Store a picture for a key (72x72 screen, shown in a ~79x75 window), the dial (240x240, round: corners are cut off) or the full key panel (480x272). Give svg (SVG text, up to 2 MB; prefer it: crisp, small, and you can draw it) or base64 (PNG, JPEG, GIF or WebP, up to 15 MB). Returns the name to put in look.image (or panel.image).",
            json!({"svg": {"type": "string"}, "base64": {"type": "string"}}),
            &[],
            false,
        ),
        tool(
            "add_animation",
            "Add animation",
            "Make an animated GIF from frames and store it: frames [{svg} or {base64}] (up to 120), fps (1-15, default 10; the deck shows at most ~15), target key (80x80), dial (240x240, round) or panel (480x272), loop (default true). Frames are drawn at the target size on black. Keep animations subtle and short; returns the name for look.image.",
            json!({"frames": {"type": "array", "items": {"type": "object", "properties": {"svg": {"type": "string"}, "base64": {"type": "string"}}}}, "fps": {"type": "integer"}, "target": {"type": "string", "enum": ["key", "dial", "panel"]}, "loop": {"type": "boolean"}}),
            &["frames"],
            false,
        ),
        tool(
            "preview_control",
            "Preview control",
            "See what a key or the dial shows, as a PNG (exactly what the hardware gets): {profile, control} for a stored control (on: true for its \"on\" look), or {look, screen: key|dial} to try a look before saving. svg: true also returns the SVG text. Use it to check contrast and that labels fit.",
            json!({"profile": {"type": "string"}, "control": {"type": "string"}, "on": {"type": "boolean"}, "look": {"type": "object"}, "screen": {"type": "string", "enum": ["key", "dial"]}, "svg": {"type": "boolean"}}),
            &[],
            true,
        ),
        tool("activate_profile", "Activate profile", "Show a profile on the deck now (it becomes the home profile until HubDeck restarts).", json!({"id": {"type": "string"}}), &["id"], false),
        tool(
            "app_picture",
            "App or game picture",
            "The real icon of one of the user's apps or the cover of one of their games (by its name or program from list_apps / list_games), stored for look.image. Returns the name. Use it so their keys show their own apps and games.",
            json!({"name": {"type": "string", "description": "App or game name, or its program (Spotify.exe)."}}),
            &["name"],
            false,
        ),
        tool(
            "try_on_deck",
            "Try on the deck",
            "Show a profile (a draft) on the user's deck for a try, with its keys, dial and lights, without making it their home profile; apps do not switch it away meanwhile. Without id: back to what was showing. Use it to show the user what you built right away (tell them to look at the deck and touch it).",
            json!({"id": {"type": "string", "description": "Profile id; leave out to go back."}}),
            &[],
            false,
        ),
    ]
}

/// Keep the entries of `list` whose JSON contains `search`.
fn filter(list: &Value, search: Option<&str>) -> Value {
    let Some(s) = search.map(str::to_lowercase).filter(|s| !s.trim().is_empty()) else { return list.clone() };
    json!(list.as_array().map(|a| a.iter().filter(|v| v.to_string().to_lowercase().contains(&s)).cloned().collect::<Vec<_>>()).unwrap_or_default())
}

fn text(v: &Value) -> Vec<Value> {
    vec![json!({"type": "text", "text": serde_json::to_string_pretty(v).unwrap_or_default()})]
}

fn arg<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    args.get(key).and_then(Value::as_str).filter(|s| !s.is_empty()).ok_or_else(|| format!("{key} is missing"))
}

/// A path segment from an id (ids are uuids or short words; anything else is refused).
fn seg(id: &str) -> Result<&str, String> {
    if !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') { Ok(id) } else { Err(format!("{id:?} is not a valid id")) }
}

/// Run one tool against the app; the content blocks of the result.
pub fn call_tool(api: &dyn Api, name: &str, args: &Value) -> Result<Vec<Value>, String> {
    let search = args.get("search").and_then(Value::as_str);
    let with = |keys: &[&str]| {
        let mut body = json!({});
        for k in keys {
            if let Some(v) = args.get(*k) {
                body[*k] = v.clone();
            }
        }
        body
    };
    match name {
        "design_guide" => Ok(vec![json!({"type": "text", "text": DESIGN_GUIDE})]),
        "list_modules" => Ok(text(&api.call("GET", "/api/modules", None)?)),
        "list_actions" => {
            let catalog = api.call("GET", "/api/catalog", None)?;
            let actions = filter(&catalog["actions"], search);
            let detailed = search.is_some() || args.get("details").and_then(Value::as_bool).unwrap_or(false);
            let actions = if detailed {
                actions
            } else {
                json!(
                    actions
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|a| json!({"id": a["id"], "name": a["name"], "group": a["group"], "inputs": a["inputs"], "description": a["description"]}))
                        .collect::<Vec<_>>()
                )
            };
            Ok(text(&json!({
                "actions": actions,
                "specialKeys": catalog["specialKeys"],
                "keyStyles": catalog["keyStyles"],
                "ledEffects": catalog["ledEffects"],
                "note": if detailed { "" } else { "Call list_actions with search set to an action id or group for its params." },
            })))
        }
        "list_icons" => {
            let catalog = api.call("GET", "/api/catalog", None)?;
            Ok(text(&json!({"icons": filter(&catalog["icons"], search), "gameIcons": filter(&catalog["gameIcons"], search)})))
        }
        "list_apps" => {
            let apps = api.call("GET", "/api/apps", None)?;
            Ok(text(&json!({"installed": filter(&apps["installed"], search), "recent": filter(&apps["recent"], search)})))
        }
        "list_games" => Ok(text(&filter(&api.call("GET", "/api/games", None)?, search))),
        "list_profiles" => Ok(text(&api.call("GET", "/api/profiles", None)?)),
        "get_profile" => Ok(text(&api.call("GET", &format!("/api/profiles/{}", seg(arg(args, "id")?)?), None)?)),
        "create_profile" => Ok(text(&api.call("POST", "/api/profiles", Some(args))?)),
        "update_profile" => {
            let id = seg(arg(args, "id")?)?;
            Ok(text(&api.call("POST", &format!("/api/profiles/{id}/update"), Some(&with(&["name", "apps", "color", "edge", "edgeEffect", "edgeSpeed", "panel"])))?))
        }
        "set_control" | "clear_control" => {
            let id = seg(arg(args, "profile")?)?;
            let binding = if name == "set_control" { args.get("binding").cloned().ok_or("binding is missing")? } else { Value::Null };
            Ok(text(&api.call("POST", &format!("/api/profiles/{id}/controls"), Some(&json!({"control": arg(args, "control")?, "binding": binding})))?))
        }
        "add_page" => Ok(text(&api.call("POST", &format!("/api/profiles/{}/pages", seg(arg(args, "profile")?)?), Some(&with(&["name", "id"])))?)),
        "delete_page" => Ok(text(&api.call("POST", &format!("/api/profiles/{}/pages/{}/delete", seg(arg(args, "profile")?)?, seg(arg(args, "page")?)?), None)?)),
        "set_lights" => {
            let id = seg(arg(args, "profile")?)?;
            Ok(text(&api.call("POST", &format!("/api/profiles/{id}/lights"), Some(&with(&["edge", "edgeEffect", "edgeSpeed", "color", "controls"])))?))
        }
        "get_deck_lights" => Ok(text(&api.call("GET", "/api/lights", None)?)),
        "set_deck_lights" => Ok(text(&api.call("POST", "/api/lights", Some(&with(&["style", "color", "brightness", "react"])))?)),
        "add_image" => Ok(text(&api.call("POST", "/api/images", Some(&with(&["svg", "base64"])))?)),
        "add_animation" => Ok(text(&api.call("POST", "/api/animations", Some(&with(&["frames", "fps", "target", "loop"])))?)),
        "preview_control" => {
            let answer = api.call("POST", "/api/preview", Some(&with(&["profile", "control", "on", "look", "screen"])))?;
            let mut content = vec![json!({"type": "image", "data": answer["png"], "mimeType": "image/png"})];
            let note = format!("{} preview ({}).", answer["screen"].as_str().unwrap_or("key"), if answer["screen"] == "dial" { "240x240, round" } else { "72x72" });
            content.push(json!({"type": "text", "text": note}));
            if args.get("svg").and_then(Value::as_bool).unwrap_or(false) {
                content.push(json!({"type": "text", "text": answer["svg"]}));
            }
            Ok(content)
        }
        "activate_profile" => Ok(text(&api.call("POST", &format!("/api/profiles/{}/activate", seg(arg(args, "id")?)?), None)?)),
        "app_picture" => Ok(text(&api.call("POST", "/api/app-picture", Some(&json!({"name": arg(args, "name")?})))?)),
        "try_on_deck" => match args.get("id").and_then(Value::as_str).filter(|i| !i.is_empty()) {
            Some(id) => Ok(text(&api.call("POST", &format!("/api/profiles/{}/try", seg(id)?), None)?)),
            None => Ok(text(&api.call("POST", "/api/try/end", None)?)),
        },
        _ => Err(format!("unknown tool {name}")),
    }
}

// ---------- the deck assistant's own server (`--mcp-assistant`) ----------

const ASSISTANT_INSTRUCTIONS: &str = "HubDeck's deck assistant tools. You work in drafts only: create_profile always makes a draft, and the other changing tools work on drafts, never on the user's own profiles or deck-wide settings. Propose ideas with propose_idea (one call per idea; HubDeck makes a draft of its sketch at once), offer answers with offer_choices, remember the user's taste with remember. Put only checked shortcuts on keys (list_shortcuts); shortcuts that close, quit or delete go on a hold.";

/// Tools the assistant gets: the builder's, without the ones that change the user's own deck,
/// and its own (ideas, answers, taste, shortcut packs).
pub fn assistant_tools() -> Vec<Value> {
    let mut list: Vec<Value> = tools()
        .into_iter()
        .filter(|t| !matches!(t["name"].as_str(), Some("activate_profile" | "set_deck_lights")))
        .map(|mut t| {
            if t["name"] == "create_profile" {
                if let Some(props) = t["inputSchema"]["properties"].as_object_mut() {
                    props.remove("replace");
                    props.remove("draft");
                }
                t["description"] = json!(format!(
                    "Create a draft profile in one call (always a draft: the user tries it and picks it with Use this). Everything is checked first; the reasons come back. {CONTROL_HELP}"
                ));
            }
            t
        })
        .collect();
    let binding =
        json!({"type": "object", "properties": {"action": {"type": "string"}, "params": {"type": "object"}, "label": {"type": "string"}, "icon": {"type": "string"}}, "required": ["action"]});
    let deck = json!({
        "type": "object",
        "description": "The whole deck sketched for the attached modules: keys (up to 15, from the top left, row by row), dial, knobs (up to 3), rollers (up to 2), faders (up to 5), edge (#rrggbb).",
        "properties": {
            "keys": {"type": "array", "items": binding.clone()},
            "dial": binding.clone(),
            "knobs": {"type": "array", "items": binding.clone()},
            "rollers": {"type": "array", "items": binding.clone()},
            "faders": {"type": "array", "items": binding},
            "edge": {"type": "string"}
        }
    });
    list.push(tool(
        "propose_idea",
        "Propose an idea",
        "Propose one idea for this person, as soon as you have it (one call per idea; do not wait to have them all). HubDeck makes a draft of its deck sketch at once and shows it as a card with a picture of their deck, Try and Pick. Returns the draft id and the controls left out, with why: fix those in the draft with set_control if they matter.",
        json!({
            "title": {"type": "string", "description": "Short, specific: \"Spotify mixer\", \"Rider debug desk\"."},
            "app": {"type": "string", "description": "The program it is for (Spotify.exe), or \"\" for a whole-desk idea."},
            "pitch": {"type": "string", "description": "One line: what it does for them."},
            "extras": {"type": "array", "items": {"type": "string"}, "description": "One or two things that make it special."},
            "deck": deck
        }),
        &["title", "pitch", "deck"],
        false,
    ));
    list.push(tool(
        "offer_choices",
        "Offer answers",
        "Show short answers as buttons under your message (the user can still write their own). multiple: true when several can be right (what to put on the deck, which apps): they tick any number, or all, and send them together.",
        json!({"choices": {"type": "array", "items": {"type": "string"}, "minItems": 2, "maxItems": 8}, "multiple": {"type": "boolean"}}),
        &["choices"],
        false,
    ));
    list.push(tool(
        "remember",
        "Remember the user's taste",
        "Remember one short thing about this user's taste for later talks (\"likes few animations\", \"Spotify always on the first knob\"). The user sees it and can remove it. Only what they said or clearly chose, never guesses about their work or life.",
        json!({"text": {"type": "string"}}),
        &["text"],
        false,
    ));
    list.push(tool(
        "tried",
        "What they touched",
        "Which controls the user pressed or turned while the last draft was on their deck for a try, in order: what they reached for (and what they did not) tells you what to keep, move or drop.",
        json!({}),
        &[],
        true,
    ));
    list.push(tool(
        "key_usage",
        "Key usage",
        "How often each control of a profile was pressed, most first, with the last day (only if the user switched counting on: counting false means nothing is counted). Arrange keys by what is used; keys never pressed are candidates to replace.",
        json!({"profile": {"type": "string"}}),
        &["profile"],
        true,
    ));
    list.push(tool(
        "hubdeck_state",
        "What is set up",
        "What is set up in HubDeck now: Claude Code sessions on keys (connected or not), the light style and which reactions are on, the user's connected apps, the screensaver, screens off after, modules attached and owned, whether more power is on. Check it before you promise a reaction or an integration.",
        json!({}),
        &[],
        true,
    ));
    list.push(tool(
        "list_app_states",
        "The user's own apps",
        "The apps the user connected (Settings > Connections > Your apps) with the actions their manifests offer, and what each reports now (by name, like launcher.1). Show an item on a key with core.state (params.name = app.id, or app.* on the dial to count what waits), call an app action with core.http or core.state's url, open a link with core.launch.",
        json!({}),
        &[],
        true,
    ));
    list.push(tool(
        "connect_app",
        "Connect an app",
        "Connect one of the user's apps by the address of its HubDeck manifest or status on this computer (http://127.0.0.1:.../hubdeck/manifest, docs/APP-CONTRACT.md); HubDeck then listens to it. Only addresses on this computer.",
        json!({"url": {"type": "string"}}),
        &["url"],
        false,
    ));
    list.push(tool(
        "test_address",
        "Test an address",
        "One GET to an app's local API on this computer (http://127.0.0.1:... or http://localhost:...), to see what it answers before you build a key on it (core.live, core.http). Returns the status and the start of the answer. Other addresses need more power (WebFetch).",
        json!({"url": {"type": "string"}}),
        &["url"],
        true,
    ));
    list.push(tool(
        "permission_prompt",
        "Ask the user",
        "Used by Claude Code itself to ask the user before a command or a web fetch; never call it yourself.",
        json!({"tool_name": {"type": "string"}, "input": {"type": "object"}, "tool_use_id": {"type": "string"}}),
        &["tool_name", "input"],
        false,
    ));
    list.push(tool(
        "make_tour",
        "Make a personal tour",
        "Only when they ask what HubDeck can do: HubDeck makes a tour draft at once from their own apps (real icons) and games (covers), with pages for live values, looks, control, animations, a picture across the keys and integrations. Then show it with try_on_deck and walk them through it.",
        json!({"language": {"type": "string", "description": "pl or en: the language of its labels (the user's)."}}),
        &[],
        false,
    ));
    list.push(tool(
        "list_shortcuts",
        "Checked shortcuts",
        "The checked default Windows shortcuts of an app (read from its vendor's pages) and Windows' own, with what each does; destructive ones close, quit or delete and go on a hold (keys.N:hold). Put only these on keys: any other shortcut shows to the user as not checked. Without app: which apps have packs.",
        json!({"app": {"type": "string", "description": "App name or program (Spotify, Code.exe)."}}),
        &[],
        true,
    ));
    list
}

/// Claude Code asks before one of its own tools (more power): the window asks the user and this
/// waits for the answer (ten minutes at most). The answer is Claude Code's permission JSON.
fn permission(api: &dyn Api, args: &Value) -> Value {
    let input = args.get("input").cloned().unwrap_or(json!({}));
    let deny = |why: &str| json!({"behavior": "deny", "message": why});
    let asked = match api.call("POST", "/api/assistant/permission", Some(&json!({"tool": args["tool_name"], "input": input}))) {
        Ok(a) => a,
        Err(e) => return deny(&format!("HubDeck could not ask the user: {e}")),
    };
    let allow = json!({"behavior": "allow", "updatedInput": input});
    if asked["allow"] == true {
        return allow;
    }
    let Some(id) = asked["id"].as_u64() else { return deny("HubDeck could not ask the user") };
    let until = std::time::Instant::now() + std::time::Duration::from_secs(600);
    while std::time::Instant::now() < until {
        if let Ok(a) = api.call("GET", &format!("/api/assistant/permission/{id}"), None)
            && a["answered"] == true
        {
            return if a["allow"] == true { allow } else { deny("The user said no in HubDeck's window. Ask what they want instead, or do it another way.") };
        }
        std::thread::sleep(std::time::Duration::from_millis(300));
    }
    deny("The user did not answer in time.")
}

/// An `http://` address on this computer: the whole authority is read (a `user@` part, as in
/// `http://127.0.0.1:1@example.com/`, would send the request elsewhere; release audit, 2026-10-13).
fn on_this_computer(url: &str) -> Result<(), String> {
    let rest = url.strip_prefix("http://").ok_or("only http:// addresses on this computer (127.0.0.1 or localhost)")?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if authority.contains(['@', '\\']) || authority.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("an address with a user name or odd characters is refused".into());
    }
    let host = match authority.strip_prefix('[') {
        Some(v6) => v6.split_once(']').map_or("", |(h, _)| h),
        None => authority.split(':').next().unwrap_or_default(),
    };
    if matches!(host, "127.0.0.1" | "localhost" | "::1") { Ok(()) } else { Err(format!("{host} is not this computer: only 127.0.0.1 or localhost (other addresses need more power)")) }
}

/// `test_address`: one GET to a local address (this computer only: nothing leaves it).
fn test_address(url: &str) -> Result<Vec<Value>, String> {
    on_this_computer(url.trim())?;
    let (code, answer) = super::exec::http_request("GET", url, "", "").map_err(|e| format!("no answer: {e}"))?;
    let start: String = answer.chars().take(2000).collect();
    Ok(text(&json!({"status": code, "answer": start, "cut": answer.chars().count() > 2000})))
}

/// The draft `id` (refused for the user's own profiles), with its apps.
fn draft_apps(api: &dyn Api, id: &str) -> Result<Vec<String>, String> {
    let id = seg(id)?;
    let listed = api.call("GET", "/api/profiles", None)?;
    let draft = listed["profiles"].as_array().into_iter().flatten().find(|p| p["id"] == id).map(|p| p["draft"].as_bool() == Some(true));
    match draft {
        None => Err(format!("no profile {id}")),
        Some(false) => Err(format!(
            "{id} is one of the user's own profiles: in this talk you change only drafts. Make a draft with create_profile (you can copy what you need from get_profile); the user picks it with Use this."
        )),
        Some(true) => Ok(api.call("GET", &format!("/api/profiles/{id}"), None)?["apps"].as_array().into_iter().flatten().filter_map(|a| a.as_str()).map(str::to_string).collect()),
    }
}

/// The packs that fit `app` (a name or a program), or the list of apps with packs.
fn shortcut_packs(app: Option<&str>) -> Value {
    let packs = super::shortcuts::packs();
    let Some(app) = app.map(|a| a.trim().to_lowercase()).filter(|a| !a.is_empty()) else {
        return json!({"apps": packs.iter().map(|p| json!({"app": p.app, "programs": p.exes})).collect::<Vec<_>>()});
    };
    let fits = |p: &super::shortcuts::Pack| {
        p.exes.iter().any(|e| e == "*") || p.app.to_lowercase().contains(&app) || p.exes.iter().any(|e| e.to_lowercase().trim_end_matches(".exe") == app.trim_end_matches(".exe"))
    };
    let found: Vec<&super::shortcuts::Pack> = packs.iter().filter(|p| fits(p)).collect();
    let own = found.iter().any(|p| !p.exes.iter().any(|e| e == "*"));
    json!({
        "packs": found,
        "note": if own { "Only these are checked for this app." } else { "No checked pack for this app yet: use Windows' own, media, volume and launch actions, or ask the user which shortcut they use; any other shortcut shows as not checked." },
    })
}

/// Run one of the assistant's tools: changes only in drafts, closing shortcuts only on a hold.
pub fn assistant_call(api: &dyn Api, name: &str, args: &Value) -> Result<Vec<Value>, String> {
    let apps_of = |v: &Value| v["apps"].as_array().into_iter().flatten().filter_map(|a| a.as_str()).map(str::to_string).collect::<Vec<_>>();
    match name {
        "activate_profile" | "set_deck_lights" => Err(format!("{name} is not available to the deck assistant")),
        "create_profile" => {
            let mut a = args.clone();
            if let Some(o) = a.as_object_mut() {
                o.remove("replace");
                o.insert("draft".into(), json!(true));
                o.insert("assistant".into(), json!(true));
            }
            let apps = apps_of(&a);
            for (control, b) in a["controls"].as_object().into_iter().flatten() {
                super::guard::check_json(control, b, &apps)?;
            }
            call_tool(api, name, &a)
        }
        "update_profile" | "set_control" | "clear_control" | "add_page" | "delete_page" | "set_lights" => {
            let id = args.get("id").or_else(|| args.get("profile")).and_then(Value::as_str).ok_or("profile is missing")?;
            let apps = draft_apps(api, id)?;
            if name == "set_control" {
                super::guard::check_json(arg(args, "control")?, args.get("binding").unwrap_or(&Value::Null), &apps)?;
            }
            call_tool(api, name, args)
        }
        "try_on_deck" => {
            if let Some(id) = args.get("id").and_then(Value::as_str).filter(|i| !i.is_empty()) {
                draft_apps(api, id)?;
            }
            call_tool(api, name, args)
        }
        "propose_idea" => Ok(text(&api.call("POST", "/api/assistant/idea", Some(args))?)),
        "offer_choices" => Ok(text(&api.call("POST", "/api/assistant/choices", Some(args))?)),
        "remember" => Ok(text(&api.call("POST", "/api/assistant/remember", Some(args))?)),
        "make_tour" => Ok(text(&api.call("POST", "/api/assistant/tour", Some(args))?)),
        "permission_prompt" => Ok(vec![json!({"type": "text", "text": permission(api, args).to_string()})]),
        "list_shortcuts" => Ok(text(&shortcut_packs(args.get("app").and_then(Value::as_str)))),
        "test_address" => test_address(arg(args, "url")?),
        "list_app_states" => Ok(text(&api.call("GET", "/api/states", None)?)),
        "hubdeck_state" => Ok(text(&api.call("GET", "/api/hubdeck", None)?)),
        "tried" => Ok(text(&api.call("GET", "/api/try/touched", None)?)),
        "key_usage" => Ok(text(&api.call("GET", &format!("/api/presses/{}", seg(arg(args, "profile")?)?), None)?)),
        "connect_app" => {
            let url = arg(args, "url")?;
            on_this_computer(url)?;
            Ok(text(&api.call("POST", "/api/apps", Some(&json!({"url": url})))?))
        }
        _ => call_tool(api, name, args),
    }
}

// ---------- connecting assistants (Settings > Connections > AI assistant) ----------

/// This program, as clients should start it.
pub(crate) fn exe() -> Result<String, String> {
    std::env::current_exe().map_err(|e| e.to_string())?.to_str().map(str::to_string).ok_or_else(|| "the program path is not valid text".into())
}

/// The `mcpServers` entry of the deck assistant's own server.
pub fn assistant_entry(exe: &str) -> Value {
    json!({"command": exe, "args": ["--mcp-assistant"]})
}

/// The `mcpServers` entry for HubDeck.
pub fn server_entry(exe: &str) -> Value {
    json!({"command": exe, "args": ["--mcp"]})
}

/// JSON to paste into any MCP client's configuration.
pub fn snippet(exe: &str) -> String {
    serde_json::to_string_pretty(&json!({"mcpServers": {SERVER_NAME: server_entry(exe)}})).unwrap_or_default()
}

/// The Claude Code command that adds HubDeck for every project of the user.
pub fn code_command(exe: &str) -> String {
    format!("claude mcp add --scope user {SERVER_NAME} -- \"{exe}\" --mcp")
}

/// The deck tools for one project: run in that project's folder.
pub fn deck_command(exe: &str) -> String {
    format!("claude mcp add --scope project {} -- \"{exe}\" --mcp-deck", super::deck_mcp::SERVER_NAME)
}

/// Claude Desktop's configuration file.
pub fn desktop_config_path() -> Option<PathBuf> {
    #[cfg(windows)]
    return std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("Claude").join("claude_desktop_config.json"));
    #[cfg(target_os = "macos")]
    return std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support/Claude/claude_desktop_config.json"));
    #[cfg(all(unix, not(target_os = "macos")))]
    return std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config/Claude/claude_desktop_config.json"));
}

/// The configuration with HubDeck's server added (or updated); everything else kept as it is.
pub fn with_server(mut config: Value, exe: &str) -> Value {
    if !config.is_object() {
        config = json!({});
    }
    let servers = config.as_object_mut().expect("object").entry("mcpServers").or_insert_with(|| json!({}));
    if !servers.is_object() {
        *servers = json!({});
    }
    servers[SERVER_NAME] = server_entry(exe);
    config
}

/// Whether a Claude Desktop configuration starts HubDeck.
pub fn has_server(config: &Value) -> bool {
    config["mcpServers"][SERVER_NAME]["args"].as_array().is_some_and(|a| a.iter().any(|x| x == "--mcp"))
}

/// Add HubDeck to the Claude Desktop configuration at `path`; the old file is kept as
/// `<name>.hubdeck-backup`. Returns the backup's path when there was a file.
pub fn connect_desktop_at(path: &Path, exe: &str) -> Result<Option<PathBuf>, String> {
    let old = std::fs::read_to_string(path).ok();
    let config: Value = match &old {
        Some(text) if !text.trim().is_empty() => serde_json::from_str(text).map_err(|e| format!("{} is not valid JSON ({e}); nothing changed", path.display()))?,
        _ => json!({}),
    };
    let next = with_server(config, exe);
    let mut backup = None;
    if let Some(text) = &old {
        let b = path.with_extension("json.hubdeck-backup");
        std::fs::write(&b, text).map_err(|e| e.to_string())?;
        backup = Some(b);
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    super::profile::write_atomic(path, serde_json::to_string_pretty(&next).map_err(|e| e.to_string())?.as_bytes()).map_err(|e| e.to_string())?;
    Ok(backup)
}

/// The `claude` command line, if installed: on the PATH, or where its installers put it.
pub fn find_claude_cli() -> Option<PathBuf> {
    #[cfg(windows)]
    let (finder, names) = ("where", ["claude.exe", "claude.cmd", "claude"]);
    #[cfg(not(windows))]
    let (finder, names) = ("which", ["claude", "claude", "claude"]);
    let mut cmd = std::process::Command::new(finder);
    cmd.arg("claude").stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    if let Some(found) = cmd.output().ok().filter(|o| o.status.success()).and_then(|o| String::from_utf8_lossy(&o.stdout).lines().map(str::trim).find(|l| !l.is_empty()).map(PathBuf::from)) {
        return Some(found);
    }
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)?;
    let mut places = vec![home.join(".local/bin"), home.join(".claude/local")];
    if cfg!(windows) {
        places.extend(std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("npm")));
    } else {
        places.extend([PathBuf::from("/opt/homebrew/bin"), PathBuf::from("/usr/local/bin")]);
    }
    places.iter().flat_map(|dir| names.iter().map(move |n| dir.join(n))).find(|p| p.is_file())
}

/// What Settings > Connections shows for the assistant.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpInfo {
    pub exe: String,
    /// Claude Desktop's configuration starts HubDeck.
    pub desktop: bool,
    /// The `claude` command line was found.
    pub code_cli: bool,
    pub code_command: String,
    /// The deck tools for one project (Claude Code), to copy.
    pub deck_command: String,
    pub snippet: String,
}

#[tauri::command]
pub async fn mcp_info() -> Result<McpInfo, String> {
    let exe = exe()?;
    tauri::async_runtime::spawn_blocking(move || {
        let desktop = desktop_config_path().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|t| serde_json::from_str::<Value>(&t).ok()).is_some_and(|c| has_server(&c));
        McpInfo { desktop, code_cli: find_claude_cli().is_some(), code_command: code_command(&exe), deck_command: deck_command(&exe), snippet: snippet(&exe), exe }
    })
    .await
    .map_err(|e| e.to_string())
}

/// Add HubDeck to Claude Desktop (the user restarts Claude Desktop); returns the backup's path.
#[tauri::command]
pub fn mcp_connect_desktop() -> Result<Option<String>, String> {
    let path = desktop_config_path().ok_or("no folder for Claude Desktop's settings")?;
    Ok(connect_desktop_at(&path, &exe()?)?.map(|b| b.display().to_string()))
}

/// Add HubDeck to Claude Code with its command line (`claude mcp add --scope user`); an older
/// entry is replaced, so a moved HubDeck is found again.
#[tauri::command]
pub async fn mcp_connect_code() -> Result<String, String> {
    let exe = exe()?;
    tauri::async_runtime::spawn_blocking(move || {
        let cli = find_claude_cli().ok_or_else(|| format!("Claude Code's command line was not found. Run this in a terminal: {}", code_command(&exe)))?;
        let run = |args: &[&str]| {
            let mut cmd = std::process::Command::new(&cli);
            cmd.args(args).stdin(std::process::Stdio::null());
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
            }
            cmd.output()
        };
        let _ = run(&["mcp", "remove", "--scope", "user", SERVER_NAME]);
        let out = run(&["mcp", "add", "--scope", "user", SERVER_NAME, "--", &exe, "--mcp"]).map_err(|e| e.to_string())?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        } else {
            Err(format!("claude mcp add failed: {}{}", String::from_utf8_lossy(&out.stderr).trim(), String::from_utf8_lossy(&out.stdout).trim()))
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

/// What "Build with AI" asks the assistant for: a whole profile for one app or game, built with
/// HubDeck's tools for the user's modules, linked to the program and checked before it is shown.
pub fn build_prompt(name: &str, exe: &str, target: &str, picture: Option<&str>, game: bool, language: &str) -> String {
    let what = if game { "the game" } else { "the app" };
    let opened = if target.trim().is_empty() || target == exe { String::new() } else { format!(", opened with {target}") };
    let mut rules = vec![
        "First read design_guide and list_modules; build only for the module kinds the user owns. Look up list_actions and list_icons as you need them.".to_string(),
        format!("Name the profile \"{name}\" and link it to {exe} (its apps list), so it switches in while {what} is in front."),
        format!("Use the real default shortcuts of {name}. Only use ones you are sure of: leave a key empty rather than guess."),
        "Make every control useful: keys (group them by use, with icons and short labels), the dial, knobs, faders and rollers, and set the lights.".to_string(),
        format!("Write labels in {language}; keep them short."),
        house_rule(),
        "Create it with create_profile in one call, check two or three keys with preview_control, fix what looks wrong, then activate_profile.".to_string(),
        "Do not change or delete any other profile.".to_string(),
    ];
    if game {
        rules.insert(3, "Keys that press game keys use the hotkey action with \"mode\": \"game\" (scan codes, which games read).".into());
    }
    if let Some(pic) = picture.filter(|p| !p.is_empty()) {
        rules.insert(3, format!("Put the picture \"{pic}\" (already stored) on the key that opens {what}: look.image."));
    }
    let list: String = rules.iter().map(|r| format!("- {r}\n")).collect();
    format!("Build a complete, polished HubDeck profile for {what} \"{name}\" (program {exe}{opened}) with the hubdeck tools.\n{list}When done, answer with one short sentence saying what you built.")
}

/// HubDeck's own key look (owner's pick, `house.rs`), so a built profile sits next to the others:
/// without it the builder made glass keys on colored backgrounds.
pub(crate) fn house_rule() -> String {
    use super::house::{BG, LABEL, hue, icon_color};
    let groups = [
        ("media", hue::MEDIA),
        ("sound", hue::SOUND),
        ("microphone", hue::MIC),
        ("navigation and pages", hue::NAV),
        ("shortcuts and tools", hue::TOOL),
        ("live and streaming", hue::LIVE),
        ("opening apps", hue::APP),
        ("lights", hue::SHOW),
        ("game keys", hue::GAME),
    ];
    let colors: Vec<String> = groups.iter().map(|(name, h)| format!("{name} {}", icon_color(*h))).collect();
    format!(
        "Keep HubDeck's house look on keys and the dial: look.bg \"{BG}\", look.fg \"{LABEL}\", no look.style, and the icon colored by its group in look.iconColor ({}). Pictures (look.image) only for app or game covers.",
        colors.join(", ")
    )
}

/// The request for "Build with AI" (also copied for Claude Desktop when Claude Code is not here).
#[tauri::command]
pub fn ai_prompt(name: String, exe: String, target: String, picture: Option<String>, language: String) -> String {
    let file = exe.rsplit(['\\', '/']).next().unwrap_or(&exe).to_string();
    let language = if language == "pl" { "Polish" } else { "English" };
    build_prompt(&name, &file, &target, picture.as_deref(), super::starter::is_game(&exe, &target), language)
}

/// The text of Claude Code's `--output-format json` answer, or its error.
fn answer_of(stdout: &str) -> Result<String, String> {
    let v: Value = serde_json::from_str(stdout.trim()).map_err(|_| format!("Claude Code answered: {}", stdout.trim().chars().take(400).collect::<String>()))?;
    let text = v["result"].as_str().unwrap_or_default().trim().to_string();
    if v["is_error"].as_bool().unwrap_or(false) { Err(if text.is_empty() { "Claude Code stopped with an error".into() } else { text }) } else { Ok(text) }
}

/// "Build with AI": run Claude Code without a window (`claude -p`) with only HubDeck's MCP tools
/// (no files, no commands, no other servers), give it the request and wait for it (up to 15
/// minutes). Returns its last sentence. `Err("no-cli")` when Claude Code is not installed.
#[tauri::command]
pub async fn ai_build(prompt: String) -> Result<String, String> {
    let exe = exe()?;
    tauri::async_runtime::spawn_blocking(move || {
        use std::io::{Read, Write};
        let cli = find_claude_cli().ok_or("no-cli")?;
        let dir = std::env::temp_dir().join("hubdeck-ai");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let config = dir.join("mcp.json");
        let mut entry = server_entry(&exe);
        // a test copy's builder must reach that copy, not the real app
        if let Some(a) = crate::automation::get() {
            entry["env"] = json!({"HUBDECK_AUTOMATION": a.data_dir});
        }
        std::fs::write(&config, json!({"mcpServers": {SERVER_NAME: entry}}).to_string()).map_err(|e| e.to_string())?;
        let mut cmd = std::process::Command::new(&cli);
        cmd.args(["-p", "--tools", "", "--mcp-config"])
            .arg(&config)
            .args(["--strict-mcp-config", "--allowedTools", &format!("mcp__{SERVER_NAME}"), "--no-session-persistence", "--output-format", "json"])
            .current_dir(&dir)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let mut child = cmd.spawn().map_err(|e| format!("could not start Claude Code: {e}"))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(prompt.as_bytes()).map_err(|e| e.to_string())?;
        }
        // read while it runs, so a long answer never fills the pipe and stalls it
        let mut stdout = child.stdout.take().ok_or("no output")?;
        let reader = std::thread::spawn(move || {
            let mut out = String::new();
            let _ = stdout.read_to_string(&mut out);
            out
        });
        let started = std::time::Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                break status;
            }
            if started.elapsed() > std::time::Duration::from_secs(15 * 60) {
                let _ = child.kill();
                return Err("Claude Code took longer than 15 minutes; stopped".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        };
        let out = reader.join().unwrap_or_default();
        log::info!("build with AI: Claude Code finished ({status}) in {:?}", started.elapsed());
        answer_of(&out)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    #[test]
    fn only_addresses_of_this_computer() {
        for ok in ["http://127.0.0.1:8080/x", "http://localhost/", "http://[::1]:5/a?b"] {
            assert!(on_this_computer(ok).is_ok(), "{ok}");
        }
        for bad in ["http://127.0.0.1:1@example.com/", "http://localhost@evil.com", "http://example.com/127.0.0.1", "https://127.0.0.1/", "http://127.0.0.1\\@evil.com/", "http://127.0.0.1.evil.com/"]
        {
            assert!(on_this_computer(bad).is_err(), "{bad}");
        }
    }

    use super::*;
    use std::sync::Mutex;

    #[test]
    fn test_address_asks_only_this_computer() {
        // a one-answer server on this computer
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = std::io::Read::read(&mut s, &mut buf);
                let _ = std::io::Write::write_all(&mut s, b"HTTP/1.1 200 OK\r\nContent-Length: 15\r\nConnection: close\r\n\r\n{\"agents\":[1]}\n");
            }
        });
        let got = test_address(&format!("http://127.0.0.1:{port}/agents")).unwrap();
        let v: Value = serde_json::from_str(got[0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(v["status"], 200);
        assert!(v["answer"].as_str().unwrap().contains("agents"));
        assert!(test_address("https://example.com/x").is_err());
        assert!(test_address("http://example.com:80/x").unwrap_err().contains("not this computer"));
        assert!(test_address("http://localhost.evil.com/x").is_err());
    }

    #[test]
    fn chunked_answers_are_put_together() {
        assert_eq!(dechunk("5\r\nhello\r\n7\r\n world!\r\n0\r\n\r\n"), "hello world!");
        assert_eq!(dechunk("3;x=1\r\nabc\r\n0\r\n\r\n"), "abc");
        let json = r#"{"actions":[1,2,3]}"#;
        let body = format!("{:x}\r\n{json}\r\n0\r\n\r\n", json.len());
        assert_eq!(serde_json::from_str::<Value>(&dechunk(&body)).unwrap()["actions"][2], 3);
    }

    #[test]
    fn the_ai_request_names_the_program_and_the_rules() {
        let p = build_prompt("Diablo II Resurrected", "D2R.exe", "battlenet://", Some("cover.png"), true, "Polish");
        assert!(p.contains("\"Diablo II Resurrected\"") && p.contains("link it to D2R.exe") && p.contains("\"mode\": \"game\""));
        assert!(p.contains("\"cover.png\"") && p.contains("in Polish") && p.contains("Do not change or delete any other profile"));
        let app = build_prompt("Blender", "blender.exe", "blender.exe", None, false, "English");
        assert!(!app.contains("mode") && !app.contains("opened with") && !app.contains("picture"));
        assert_eq!(answer_of(r#"{"type":"result","is_error":false,"result":" Built a Blender profile. "}"#), Ok("Built a Blender profile.".into()));
        assert!(answer_of(r#"{"type":"result","is_error":true,"result":"Not logged in"}"#).unwrap_err().contains("Not logged in"));
        assert!(answer_of("not json").is_err());
    }

    /// The app with one of the user's profiles and one draft.
    struct Drafts {
        calls: Mutex<Vec<(String, String, Option<Value>)>>,
    }

    impl Api for Drafts {
        fn call(&self, method: &str, path: &str, body: Option<&Value>) -> Result<Value, String> {
            self.calls.lock().unwrap().push((method.into(), path.into(), body.cloned()));
            match (method, path) {
                ("GET", "/api/profiles") => Ok(json!({"profiles": [{"id": "mine", "draft": false}, {"id": "d1", "draft": true}]})),
                ("GET", "/api/profiles/d1") => Ok(json!({"id": "d1", "apps": ["Zoom.exe"]})),
                (_, p) if p.starts_with("/api/profiles") => Ok(json!({"ok": true})),
                _ => Ok(json!({"ok": true})),
            }
        }
    }

    #[test]
    fn the_deck_assistant_changes_only_drafts_and_holds_closing_shortcuts() {
        let names: Vec<String> = assistant_tools().iter().filter_map(|t| t["name"].as_str().map(str::to_string)).collect();
        assert!(!names.iter().any(|n| n == "activate_profile" || n == "set_deck_lights"));
        assert!(names.iter().any(|n| n == "propose_idea") && names.iter().any(|n| n == "list_shortcuts"));
        let create = assistant_tools().into_iter().find(|t| t["name"] == "create_profile").unwrap();
        assert!(create["inputSchema"]["properties"].get("replace").is_none());
        let api = Drafts { calls: Mutex::new(Vec::new()) };
        let media = json!({"action": "core.media", "params": {"command": "playPause"}});
        assert!(assistant_call(&api, "set_control", &json!({"profile": "mine", "control": "keys.0", "binding": media})).unwrap_err().contains("own profiles"));
        assert!(assistant_call(&api, "set_control", &json!({"profile": "d1", "control": "keys.0", "binding": media})).is_ok());
        let end = json!({"action": "core.hotkey", "params": {"keys": "Alt+Q"}});
        assert!(assistant_call(&api, "set_control", &json!({"profile": "d1", "control": "keys.1", "binding": end})).unwrap_err().contains("hold"), "Zoom's end meeting on a press");
        assert!(assistant_call(&api, "set_control", &json!({"profile": "d1", "control": "keys.1:hold", "binding": end})).is_ok());
        assert!(assistant_call(&api, "try_on_deck", &json!({"id": "mine"})).is_err());
        assert!(assistant_call(&api, "activate_profile", &json!({"id": "d1"})).is_err());
        assistant_call(&api, "create_profile", &json!({"name": "X", "replace": "mine"})).unwrap();
        let sent = api.calls.lock().unwrap().iter().rev().find(|(m, p, _)| m == "POST" && p == "/api/profiles").and_then(|c| c.2.clone()).unwrap();
        assert!(sent.get("replace").is_none() && sent["draft"] == true && sent["assistant"] == true, "{sent}");
        assert!(assistant_call(&api, "create_profile", &json!({"name": "X", "controls": {"keys.0": {"action": "core.run", "params": {"command": "x"}}}})).is_err());
        let spotify = assistant_call(&api, "list_shortcuts", &json!({"app": "Spotify.exe"})).unwrap();
        assert!(spotify[0]["text"].as_str().unwrap().contains("Like song"));
    }

    /// Answers like the app, and remembers what was asked.
    struct Fake {
        calls: Mutex<Vec<(String, String, Option<Value>)>>,
    }

    impl Api for Fake {
        fn call(&self, method: &str, path: &str, body: Option<&Value>) -> Result<Value, String> {
            self.calls.lock().unwrap().push((method.into(), path.into(), body.cloned()));
            match (method, path) {
                ("GET", "/api/catalog") => Ok(json!({
                    "actions": [{"id": "core.media", "name": "Media control", "group": "Media", "inputs": ["press"], "description": "Play", "params": [{"key": "command"}]},
                                {"id": "obs.record", "name": "Record", "group": "OBS", "inputs": ["press"], "description": "Rec", "params": []}],
                    "icons": ["play", "mic"], "gameIcons": ["game:sword"], "specialKeys": ["F13"], "keyStyles": ["flat"], "ledEffects": ["static"]
                })),
                ("POST", "/api/preview") => Ok(json!({"screen": "key", "svg": "<svg/>", "png": "iVBORw0KGgo="})),
                ("POST", "/api/profiles") => Ok(json!({"profile": {"id": "new", "name": body.unwrap()["name"]}, "warnings": []})),
                _ => Err(NOT_RUNNING.into()),
            }
        }
    }

    fn fake() -> Fake {
        Fake { calls: Mutex::new(Vec::new()) }
    }

    fn rpc(api: &dyn Api, method: &str, params: Value) -> Value {
        handle_message(api, &json!({"jsonrpc": "2.0", "id": 7, "method": method, "params": params})).expect("an answer")
    }

    #[test]
    fn initialize_negotiates_the_version_and_notifications_get_no_answer() {
        let f = fake();
        let r = rpc(&f, "initialize", json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "t", "version": "1"}}));
        assert_eq!(r["id"], 7);
        assert_eq!(r["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(r["result"]["serverInfo"]["name"], "hubdeck");
        assert!(r["result"]["capabilities"]["tools"].is_object());
        let newest = rpc(&f, "initialize", json!({"protocolVersion": "1999-01-01"}));
        assert_eq!(newest["result"]["protocolVersion"], PROTOCOL_VERSIONS[0]);
        assert!(handle_message(&f, &json!({"jsonrpc": "2.0", "method": "notifications/initialized"})).is_none());
        assert_eq!(rpc(&f, "nope", json!({}))["error"]["code"], -32601);
        assert_eq!(rpc(&f, "ping", json!({}))["result"], json!({}));
    }

    #[test]
    fn tools_list_has_every_tool_with_an_object_schema() {
        let f = fake();
        let tools = rpc(&f, "tools/list", json!({}))["result"]["tools"].as_array().unwrap().clone();
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        for want in [
            "list_actions",
            "list_modules",
            "list_apps",
            "list_games",
            "list_profiles",
            "get_profile",
            "create_profile",
            "set_control",
            "clear_control",
            "activate_profile",
            "try_on_deck",
            "app_picture",
            "add_image",
            "add_animation",
            "preview_control",
            "set_lights",
            "add_page",
            "design_guide",
            "list_icons",
            "get_deck_lights",
            "set_deck_lights",
        ] {
            assert!(names.contains(&want), "{want} is listed");
        }
        let mut seen = std::collections::HashSet::new();
        for t in &tools {
            assert!(seen.insert(t["name"].as_str().unwrap()), "unique names");
            assert!(t["description"].as_str().unwrap().len() > 30, "{} is described", t["name"]);
            assert_eq!(t["inputSchema"]["type"], "object", "{}", t["name"]);
            for r in t["inputSchema"]["required"].as_array().unwrap() {
                assert!(t["inputSchema"]["properties"].get(r.as_str().unwrap()).is_some(), "{}: required {r} is a property", t["name"]);
            }
        }
        // the binding schema teaches the control ids, gestures and pages
        let set = tools.iter().find(|t| t["name"] == "set_control").unwrap().to_string();
        for word in ["keys.0", ":hold", "knobs.0", "<page id>/keys.N", "game:<name>", "turnOnPress"] {
            assert!(set.contains(word), "set_control mentions {word}");
        }
        assert!(DESIGN_GUIDE.len() > 1000, "the guide is bundled");
    }

    #[test]
    fn tool_calls_go_to_the_api_and_errors_come_back_as_tool_errors() {
        let f = fake();
        let r = rpc(&f, "tools/call", json!({"name": "list_actions", "arguments": {"search": "obs"}}));
        let out: Value = serde_json::from_str(r["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(out["actions"].as_array().unwrap().len(), 1);
        assert_eq!(out["actions"][0]["id"], "obs.record");
        let compact = rpc(&f, "tools/call", json!({"name": "list_actions", "arguments": {}}));
        let compact: Value = serde_json::from_str(compact["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
        assert!(compact["actions"][0].get("params").is_none(), "no params without search");
        let preview = rpc(&f, "tools/call", json!({"name": "preview_control", "arguments": {"profile": "p", "control": "keys.0"}}));
        assert_eq!(preview["result"]["content"][0]["type"], "image");
        assert_eq!(preview["result"]["content"][0]["mimeType"], "image/png");
        let created = rpc(&f, "tools/call", json!({"name": "create_profile", "arguments": {"name": "Edit"}}));
        assert_eq!(created["result"]["isError"], false);
        // the app is "not running" for everything else
        let down = rpc(&f, "tools/call", json!({"name": "list_profiles", "arguments": {}}));
        assert_eq!(down["result"]["isError"], true);
        assert!(down["result"]["content"][0]["text"].as_str().unwrap().contains("not running"));
        let bad_id = rpc(&f, "tools/call", json!({"name": "get_profile", "arguments": {"id": "../x"}}));
        assert_eq!(bad_id["result"]["isError"], true, "ids cannot leave the API path");
        assert_eq!(rpc(&f, "tools/call", json!({"name": "rm_rf", "arguments": {}}))["error"]["code"], -32602);
        let set = rpc(&f, "tools/call", json!({"name": "clear_control", "arguments": {"profile": "p1", "control": "keys.3"}}));
        assert_eq!(set["result"]["isError"], true);
        let calls = f.calls.lock().unwrap();
        let last = calls.last().unwrap();
        assert_eq!((last.0.as_str(), last.1.as_str()), ("POST", "/api/profiles/p1/controls"));
        assert_eq!(last.2.as_ref().unwrap(), &json!({"control": "keys.3", "binding": null}));
    }

    #[test]
    fn http_round_trip_against_a_fake_app() {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let port = server.server_addr().to_ip().unwrap().port();
        let dir = std::env::temp_dir().join(format!("hubdeck-mcp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("api-token"), "secret-token-123456\n").unwrap();
        std::fs::write(dir.join("api-http-port"), port.to_string()).unwrap();
        let app = std::thread::spawn(move || {
            for _ in 0..2 {
                let mut req = server.recv().unwrap();
                let ok = req.headers().iter().any(|h| h.field.equiv("Authorization") && h.value.as_str() == "Bearer secret-token-123456");
                let mut body = String::new();
                req.as_reader().read_to_string(&mut body).unwrap();
                let (code, answer) = match (ok, req.url()) {
                    (true, "/api/profiles") => (200, json!({"profiles": [{"id": "a", "name": "Desk"}], "active": "a", "home": "a"})),
                    (true, url) => (400, json!({"error": format!("no {url} with {body}")})),
                    _ => (401, json!({"error": "token"})),
                };
                req.respond(tiny_http::Response::from_string(answer.to_string()).with_status_code(code)).unwrap();
            }
        });
        let api = Http { dir: dir.clone() };
        let r = handle_message(&api, &json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "list_profiles", "arguments": {}}})).unwrap();
        assert!(r["result"]["content"][0]["text"].as_str().unwrap().contains("Desk"), "{r}");
        let err = api.call("POST", "/api/profiles/a/activate", Some(&json!({"x": 1}))).unwrap_err();
        assert!(err.contains("/api/profiles/a/activate") && err.contains("\"x\":1"), "the app's error is passed on: {err}");
        app.join().unwrap();
        // nothing listens any more: a clear message
        let gone = api.call("GET", "/api/profiles", None).unwrap_err();
        assert!(gone.contains("not running"), "{gone}");
        std::fs::remove_dir_all(&dir).ok();
        assert!(Http { dir }.call("GET", "/api/profiles", None).unwrap_err().contains("not running"), "no token file");
    }

    #[test]
    fn desktop_config_keeps_other_servers_and_makes_a_backup() {
        let dir = std::env::temp_dir().join(format!("hubdeck-claude-desktop-{}", std::process::id()));
        let path = dir.join("Claude").join("claude_desktop_config.json");
        // no file yet: created, no backup
        assert_eq!(connect_desktop_at(&path, "C:\\HubDeck\\hubdeck.exe").unwrap(), None);
        let made: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(made["mcpServers"]["hubdeck"], json!({"command": "C:\\HubDeck\\hubdeck.exe", "args": ["--mcp"]}));
        // an existing file: other servers and settings stay, the old text is kept
        std::fs::write(&path, r#"{"mcpServers": {"files": {"command": "npx", "args": ["fs"]}, "hubdeck": {"command": "old.exe", "args": ["--mcp"]}}, "theme": "dark"}"#).unwrap();
        let backup = connect_desktop_at(&path, "D:\\new\\hubdeck.exe").unwrap().unwrap();
        let next: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(next["mcpServers"]["files"]["command"], "npx");
        assert_eq!(next["theme"], "dark");
        assert_eq!(next["mcpServers"]["hubdeck"]["command"], "D:\\new\\hubdeck.exe");
        assert!(has_server(&next));
        assert!(std::fs::read_to_string(backup).unwrap().contains("old.exe"));
        // broken JSON is never overwritten
        std::fs::write(&path, "{ broken").unwrap();
        assert!(connect_desktop_at(&path, "x").unwrap_err().contains("nothing changed"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ broken");
        std::fs::remove_dir_all(&dir).ok();
        assert!(snippet("C:\\a b\\hubdeck.exe").contains("\"--mcp\""));
        assert_eq!(code_command("C:\\a b\\h.exe"), "claude mcp add --scope user hubdeck -- \"C:\\a b\\h.exe\" --mcp");
    }
}
