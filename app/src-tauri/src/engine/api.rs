//! What the user has, for adapting profiles, and the routes of the local HTTP API that let an
//! assistant build profiles (`hubdeck --mcp` is a client of them, see `mcp.rs` and `docs/MCP.md`):
//! the catalog, modules, apps, games, profiles, controls, lights, pages, pictures, animations and
//! previews. Everything an assistant sends is checked before it is saved.

use base64::Engine as _;
use serde::Serialize;
use serde_json::{Value, json};
use tauri::{AppHandle, Manager};

use crate::deck::Deck;
use crate::placements::Placements;

use super::Engine;
use super::actions::InputKind;
use super::presets::KINDS;
use super::profile::{Binding, Profile};

/// Module kinds the user has, in the order of [`KINDS`] (the base is not one of them).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ModuleKinds {
    /// Attached to the base now.
    pub attached: Vec<String>,
    /// Attached now or owned but not attached: what profiles are adapted to.
    pub owned: Vec<String>,
}

/// Kinds from the attached modules and the keys of the placements table (`"<kind>@<slot>"`).
pub fn kinds_from<'a>(attached: impl IntoIterator<Item = &'a str>, placement_keys: impl IntoIterator<Item = &'a str>) -> ModuleKinds {
    let attached: Vec<&str> = attached.into_iter().collect();
    let remembered: Vec<&str> = placement_keys.into_iter().map(|k| k.split('@').next().unwrap_or("")).collect();
    let pick = |from: &dyn Fn(&str) -> bool| KINDS.iter().filter(|k| from(k)).map(|k| k.to_string()).collect::<Vec<_>>();
    ModuleKinds { attached: pick(&|k| attached.contains(&k)), owned: pick(&|k| attached.contains(&k) || remembered.contains(&k)) }
}

/// The user's module kinds now.
pub fn module_kinds(app: &AppHandle) -> ModuleKinds {
    let attached: Vec<&'static str> = app.try_state::<Deck>().map(|d| d.snapshot().modules.iter().map(|m| m.kind).collect()).unwrap_or_default();
    let keys: Vec<String> = app.try_state::<Placements>().map(|p| p.all().into_keys().collect()).unwrap_or_default();
    kinds_from(attached, keys.iter().map(String::as_str))
}

/// Adapt `profile` to the user's modules (see [`super::presets::adapt_noted`]). With no module
/// known at all (nothing attached, nothing remembered) it stays as it is.
pub fn adapt_to_user(app: &AppHandle, profile: super::profile::Profile) -> (super::profile::Profile, Vec<super::presets::Moved>) {
    let kinds = module_kinds(app).owned;
    if kinds.is_empty() {
        return (profile, Vec::new());
    }
    super::presets::adapt_noted(profile, &kinds)
}

// ---------- routes of the local HTTP API for assistants (`hubdeck --mcp`, scripts) ----------

/// Connect the routes below to the local HTTP API (after the engine is managed).
pub fn install(app: AppHandle) {
    super::plugins::set_api_handler(Box::new(move |method, path, body| handle(&app, method, path, body)));
}

type Answer = Result<Value, (u16, String)>;

fn bad(msg: impl Into<String>) -> (u16, String) {
    (400, msg.into())
}

fn not_found(msg: impl Into<String>) -> (u16, String) {
    (404, msg.into())
}

fn body_json(body: &str) -> Result<Value, (u16, String)> {
    let v: Value = serde_json::from_str(if body.trim().is_empty() { "{}" } else { body }).map_err(|e| bad(format!("not JSON: {e}")))?;
    if v.is_object() { Ok(v) } else { Err(bad("the body must be a JSON object")) }
}

/// Test mode only: `{"kind": "keys", "index": 4, "gesture": "press" | "turn" | "slide", "value": 1}`.
fn test_input(app: &AppHandle, b: &Value) -> Answer {
    let kind = b["kind"].as_str().ok_or_else(|| bad("kind is missing"))?;
    let index = b["index"].as_u64().unwrap_or(0).min(255) as u8;
    let gesture = b["gesture"].as_str().unwrap_or("press");
    let value = b["value"].as_i64().unwrap_or(1);
    app.state::<Deck>().virtual_input(kind, index, gesture, value).map_err(bad)?;
    Ok(json!({"ok": true}))
}

/// `(method, path, body)` -> `(status, JSON)`; None for a path this module does not serve.
pub fn handle(app: &AppHandle, method: &str, path: &str, body: &str) -> Option<(u16, Value)> {
    let parts: Vec<&str> = path.strip_prefix("/api/")?.split('/').collect();
    let engine = || app.try_state::<Engine>().ok_or((503, "HubDeck is still starting".to_string()));
    let result: Answer = match (method, parts.as_slice()) {
        ("GET", ["catalog"]) => Ok(catalog()),
        ("GET", ["modules"]) => Ok(modules(app)),
        ("GET", ["apps"]) => Ok(apps()),
        ("GET", ["games"]) => Ok(games()),
        ("GET", ["spotify", "playlists"]) => Ok(match super::spotify::playlists(std::time::Duration::from_secs(6)) {
            Some(list) => json!({"connected": true, "playlists": list.iter().map(|n| json!({"name": n.label, "playlist": n.value})).collect::<Vec<_>>()}),
            None => json!({"connected": false, "playlists": []}),
        }),
        ("GET", ["profiles"]) => engine().map(|e| profiles(&e)),
        ("GET", ["profiles", id]) => engine().and_then(|e| e.lock().store.profiles.get(*id).map(|p| json!(p)).ok_or_else(|| not_found(format!("no profile {id}")))),
        ("POST", ["profiles"]) => engine().and_then(|e| create_profile(&e, &body_json(body)?)),
        ("POST", ["profiles", id, "update"]) => engine().and_then(|e| update_profile(&e, id, &body_json(body)?)),
        ("POST", ["profiles", id, "controls"]) => engine().and_then(|e| set_control(&e, id, &body_json(body)?)),
        ("POST", ["profiles", id, "lights"]) => engine().and_then(|e| profile_lights(&e, id, &body_json(body)?)),
        ("POST", ["profiles", id, "pages"]) => engine().and_then(|e| add_page(&e, id, &body_json(body)?)),
        ("POST", ["profiles", id, "pages", page, "delete"]) => engine().and_then(|e| delete_page(&e, id, page)),
        ("POST", ["profiles", id, "activate"]) => engine().and_then(|e| e.activate(id).map(|_| json!({"ok": true})).map_err(not_found)),
        ("POST", ["profiles", id, "try"]) => engine().and_then(|e| e.try_profile(Some(id)).map(|_| json!({"ok": true, "showing": id})).map_err(not_found)),
        // throw a draft away (only drafts: the user's own profiles are never deleted here)
        ("POST", ["drafts", id, "discard"]) => engine().and_then(|e| {
            if !e.lock().store.settings.drafts.iter().any(|d| d == id) {
                return Err(not_found(format!("no draft {id}")));
            }
            e.settle_draft(id, false).map(|_| json!({"ok": true})).map_err(bad)
        }),
        ("POST", ["try", "end"]) => engine().and_then(|e| e.try_profile(None).map(|_| json!({"ok": true})).map_err(not_found)),
        ("GET", ["lights"]) => engine().map(|e| json!(e.lock().store.settings.lights)),
        ("POST", ["lights"]) => engine().and_then(|e| set_lights(&e, &body_json(body)?)),
        ("POST", ["images"]) => body_json(body).and_then(|b| add_image(&b)),
        ("POST", ["app-picture"]) => body_json(body).and_then(|b| app_picture(&b)),
        ("POST", ["animations"]) => body_json(body).and_then(|b| add_animation(&b)),
        // the deck assistant's ideas, answer buttons and taste notes go to its window
        // the user's own apps (docs/APP-CONTRACT.md)
        ("POST", ["state"]) => body_json(body).and_then(|b| super::states::push(&b).map(|n| json!({"ok": true, "items": n})).map_err(bad)),
        ("GET", ["states"]) => engine().map(|e| e.app_states()),
        ("GET", ["hubdeck"]) => engine().map(|e| hubdeck_state(app, &e)),
        ("GET", ["presses", id]) => engine().map(|e| {
            let on = e.lock().store.settings.count_presses;
            json!({"counting": on, "controls": super::presses::counts(id).into_iter().map(|(c, n)| json!({"control": c, "count": n.count, "lastDay": n.last_day})).collect::<Vec<_>>()})
        }),
        ("GET", ["try", "touched"]) => Ok(super::presses::touched().map_or(json!({"draft": null, "touched": []}), |(d, t)| json!({"draft": d, "touched": t}))),
        ("POST", ["apps"]) => engine().and_then(|e| body_json(body).and_then(|b| e.connect_app(b["url"].as_str().unwrap_or_default()).map(|a| json!(a)).map_err(bad))),
        // quit cleanly (the deck is handed back), for scripts and the dev loop: killing the
        // process can leave the keys module mid-picture (docs/KNOWLEDGE.md)
        ("POST", ["quit"]) => {
            let app = app.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(200));
                app.exit(0);
            });
            Ok(json!({"ok": true}))
        }
        // install a Stream Deck plugin from a file on this computer or an https address
        ("POST", ["plugins", "install"]) => body_json(body).and_then(|b| {
            let bytes = match (b["path"].as_str(), b["url"].as_str()) {
                (Some(path), _) => std::fs::read(path).map_err(|e| bad(e.to_string()))?,
                (None, Some(url)) => super::sdplugins::download(url).map_err(bad)?,
                _ => return Err(bad("give a path or a url")),
            };
            let id = super::plugins::install_sd(&bytes).map_err(bad)?;
            use tauri::Emitter as _;
            let _ = app.emit("engine://catalog", ());
            Ok(json!({"ok": true, "id": id}))
        }),
        ("POST", ["apps", "announce"]) => engine().and_then(|e| body_json(body).and_then(|b| e.announce_app(b["url"].as_str().unwrap_or_default()).map_err(bad))),
        ("POST", ["apps", id, "remove"]) => engine().and_then(|e| e.remove_app(id).map(|_| json!({"ok": true})).map_err(not_found)),
        ("POST", ["assistant", "idea"]) => body_json(body).and_then(|b| super::assistant::api_idea(app, &b).map_err(bad)),
        ("POST", ["assistant", "choices"]) => body_json(body).and_then(|b| super::assistant::api_choices(app, &b).map_err(bad)),
        ("POST", ["assistant", "tour"]) => body_json(body).and_then(|b| super::assistant::api_tour(app, &b).map_err(bad)),
        ("POST", ["assistant", "permission"]) => body_json(body).and_then(|b| super::assistant::api_permission(app, &b).map_err(bad)),
        ("GET", ["assistant", "permission", id]) => id.parse().map(super::assistant::api_permission_answer).map_err(|_| bad("not an id")),
        ("POST", ["assistant", "remember"]) => body_json(body).and_then(|b| super::assistant::api_remember(app, &b).map_err(bad)),
        ("POST", ["preview"]) => engine().and_then(|e| preview(&e, &body_json(body)?)),
        ("POST", ["notify"]) => engine().and_then(|e| notify(&e, &body_json(body)?)),
        ("POST", ["status"]) => engine().and_then(|e| status(&e, &body_json(body)?)),
        ("POST", ["ask"]) => engine().and_then(|e| ask(&e, &body_json(body)?)),
        ("GET", ["ask", id]) => engine().and_then(|e| ask_answer(&e, id)),
        ("GET", ["controls"]) => engine().map(|e| controls(&e)),
        ("POST", ["test", "input"]) if crate::automation::get().is_some() => body_json(body).and_then(|b| test_input(app, &b)),
        ("POST", ["test", "foreground"]) if crate::automation::get().is_some() => engine().and_then(|e| {
            let b = body_json(body)?;
            let exe = b["exe"].as_str().ok_or_else(|| bad("exe is missing"))?.to_string();
            e.test_foreground(exe, b["path"].as_str().unwrap_or(""), b["title"].as_str().unwrap_or("").to_string());
            Ok(json!({"ok": true}))
        }),
        _ => return None,
    };
    Some(match result {
        Ok(v) => (200, v),
        Err((code, error)) => (code, json!({ "error": error })),
    })
}

/// Controls of each kind, as the API and the profile file name them.
pub const CONTROLS: [(&str, usize); 5] = [("keys", 15), ("dial", 1), ("knobs", 3), ("faders", 5), ("rollers", 2)];

/// Key looks (`look.style`).
pub const KEY_STYLES: [&str; 4] = ["flat", "gradient", "glass", "outline"];

fn catalog() -> Value {
    let actions: Vec<_> = super::actions::all().into_iter().filter(|a| super::games::GAMES_ENABLED || a.id != super::actions::GAMES).collect();
    json!({
        "actions": actions,
        "specialKeys": super::hotkey::SPECIAL_KEYS,
        "icons": super::icons::ICONS.iter().map(|(n, _)| *n).collect::<Vec<_>>(),
        "gameIcons": super::game_icons::GAME_ICONS.iter().map(|(n, _)| format!("{}{n}", super::render::GAME_PREFIX)).collect::<Vec<_>>(),
        "keyStyles": KEY_STYLES,
        "ledEffects": super::leds::EFFECTS.iter().map(|(n, _)| *n).collect::<Vec<_>>(),
    })
}

fn modules(app: &AppHandle) -> Value {
    let kinds = module_kinds(app);
    json!({
        "attached": kinds.attached,
        "owned": kinds.owned,
        "controls": {
            "keys": "keys.0 .. keys.14: 15 keys with 72x72 screens (a ~79x75 window each), 5 per row, 3 rows, numbered from the top left",
            "dial": "dial: a 240x240 round screen that turns and presses",
            "knobs": "knobs.0 .. knobs.2: 3 knobs that turn and press, each with an LED ring",
            "faders": "faders.0 .. faders.4: 5 faders (absolute 0..1), each with an LED",
            "rollers": "rollers.0 .. rollers.1: 2 rollers that turn (no press, no LED)",
        },
    })
}

/// What is set up in HubDeck, for the assistant (so it does not promise what is off): Claude Code
/// sessions on keys, the light reactions, connected apps, the screensaver, modules, more power.
fn hubdeck_state(app: &AppHandle, engine: &Engine) -> Value {
    let settings = engine.lock().store.settings.clone();
    let kinds = module_kinds(app);
    json!({
        "claudeCodeSessionsOnKeys": super::agents::connected(),
        "lights": {"style": settings.lights.style, "color": settings.lights.color, "reactions": settings.lights.react},
        "apps": settings.apps.iter().map(|a| json!({"id": a.id, "name": a.name, "actions": a.actions.len()})).collect::<Vec<_>>(),
        "screensaver": if settings.saver.is_empty() { json!(null) } else { json!({"effect": settings.saver, "afterMinutes": settings.saver_after_minutes}) },
        "screensOffAfterMinutes": settings.blank_after_minutes,
        "modulesAttached": kinds.attached,
        "modulesOwned": kinds.owned,
        "assistantMorePower": settings.assistant_power,
        "appIcons": settings.app_icons,
    })
}

fn apps() -> Value {
    let installed = super::installed::list(false);
    let apps: Vec<Value> = installed.apps.iter().map(|i| json!({"name": i.name, "target": i.target, "exe": i.exe, "source": i.source})).collect();
    let recent: Vec<Value> = super::apps::list().into_iter().map(|a| json!({"exe": a.exe, "title": a.title, "running": a.running})).collect();
    json!({ "installed": apps, "recent": recent })
}

fn games() -> Value {
    let installed = super::installed::list(false);
    json!(installed.games.iter().map(|g| json!({"name": g.name, "target": g.target, "exe": g.exe, "source": g.source})).collect::<Vec<_>>())
}

fn profiles(engine: &Engine) -> Value {
    let state = serde_json::to_value(engine.lock().state()).unwrap_or_default();
    json!({ "profiles": state["profiles"], "active": state["active"], "home": state["home"] })
}

fn is_hex(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// Inputs each control kind gives.
fn inputs_of(kind: &str) -> &'static [InputKind] {
    match kind {
        "keys" => &[InputKind::Press],
        "dial" | "knobs" => &[InputKind::Press, InputKind::Turn],
        "faders" => &[InputKind::Slide],
        "rollers" => &[InputKind::Turn],
        _ => &[],
    }
}

/// Check a control id (`keys.3`, `dial`, `knobs.0:hold`, `<page>/keys.2:double`); returns its kind.
pub fn control_kind(id: &str, pages: &[super::profile::Page]) -> Result<&'static str, String> {
    let (page, rest) = match id.split_once('/') {
        Some((page, rest)) => (Some(page), rest),
        None => (None, id),
    };
    let (base, gesture) = match rest.split_once(':') {
        Some((base, g)) => (base, Some(g)),
        None => (rest, None),
    };
    let (kind, index) = match base.split_once('.') {
        Some((kind, index)) => (kind, Some(index)),
        None => (base, None),
    };
    let (name, count) = CONTROLS.iter().find(|(k, _)| *k == kind).copied().ok_or_else(|| format!("{id}: unknown control (keys.N, dial, knobs.N, faders.N, rollers.N)"))?;
    match (name, index) {
        ("dial", None) => {}
        ("dial", Some(_)) => return Err(format!("{id}: the dial is just `dial`")),
        (_, Some(i)) if i.parse::<usize>().is_ok_and(|i| i < count) && !i.starts_with('+') => {}
        _ => return Err(format!("{id}: {name} are numbered {name}.0 .. {name}.{}", count - 1)),
    }
    if let Some(g) = gesture {
        if !matches!(g, "hold" | "double") {
            return Err(format!("{id}: the gesture is `:hold` or `:double`"));
        }
        if !inputs_of(name).contains(&InputKind::Press) {
            return Err(format!("{id}: {name} cannot be pressed, so they have no hold or double press"));
        }
    }
    if let Some(page) = page {
        if name != "keys" {
            return Err(format!("{id}: only keys belong to pages; the other controls are the same on every page"));
        }
        if !pages.iter().any(|p| p.id == page) {
            return Err(format!("{id}: no page {page:?} in this profile (add_page first)"));
        }
    }
    Ok(name)
}

/// Problems that make a binding wrong (errors) and ones worth a second look (warnings).
pub fn check_binding(id: &str, b: &Binding, pages: &[super::profile::Page], errors: &mut Vec<String>, warnings: &mut Vec<String>) {
    let kind = match control_kind(id, pages) {
        Ok(k) => k,
        Err(e) => return errors.push(e),
    };
    if !b.action.is_empty() {
        match super::actions::find(&b.action) {
            None => errors.push(format!("{id}: unknown action {:?} (see list_actions)", b.action)),
            Some(def) => {
                let gives = if id.contains(':') { &[InputKind::Press][..] } else { inputs_of(kind) };
                let on_press = kind == "keys" && def.inputs.contains(&InputKind::Turn) && b.params.get("turnOnPress").and_then(Value::as_i64).is_some_and(|v| v != 0);
                if !on_press && !def.inputs.iter().any(|i| gives.contains(i)) {
                    errors.push(format!(
                        "{id}: {} reacts to {:?}, but {kind} give {:?}{}",
                        b.action,
                        def.inputs,
                        gives,
                        if kind == "keys" && def.inputs.contains(&InputKind::Turn) { " (on a key, set params.turnOnPress to 1 or -1)" } else { "" }
                    ));
                }
                if b.action == "core.hotkey" {
                    for key in ["keys", "clockwise", "counterClockwise"] {
                        if let Some(combo) = b.params.get(key).and_then(Value::as_str).filter(|c| !c.is_empty())
                            && super::hotkey::parse(combo).is_none()
                        {
                            errors.push(format!("{id}: hotkey {combo:?} does not parse (like Ctrl+Shift+M, F13, VolumeUp, Space)"));
                        }
                    }
                }
                for p in &def.params {
                    if let (super::actions::ParamKind::Select { options }, Some(v)) = (&p.kind, b.params.get(p.key).and_then(Value::as_str))
                        && !options.iter().any(|(o, _)| o == v)
                    {
                        errors.push(format!("{id}: {}.{} must be one of {:?}", b.action, p.key, options.iter().map(|(o, _)| o.as_str()).collect::<Vec<_>>()));
                    }
                }
            }
        }
    }
    for look in std::iter::once(&b.look).chain(b.look_on.as_ref()) {
        if let Some(icon) = look.icon.as_deref().filter(|i| !i.is_empty())
            && super::render::icon_svg(icon).is_none()
        {
            errors.push(format!("{id}: unknown icon {icon:?} (see list_icons)"));
        }
        if look.fit.as_deref().is_some_and(|f| f != "icon" && f != "fill") {
            errors.push(format!("{id}: look.fit is icon or fill"));
        }
        for (field, value) in [("bg", &look.bg), ("fg", &look.fg), ("iconColor", &look.icon_color), ("led", &look.led)] {
            if let Some(v) = value.as_deref().filter(|v| !is_hex(v)) {
                errors.push(format!("{id}: look.{field} {v:?} is not a #rrggbb color"));
            }
        }
        if let Some(style) = look.style.as_deref().filter(|s| !KEY_STYLES.contains(s)) {
            errors.push(format!("{id}: look.style {style:?} is not one of {KEY_STYLES:?}"));
        }
        if let Some(effect) = look.led_effect.as_deref().filter(|e| !super::leds::EFFECTS.iter().any(|(n, _)| n == e)) {
            errors.push(format!("{id}: look.ledEffect {effect:?} is not a known effect (see list_actions: ledEffects)"));
        }
        // "cover": the cover of what plays now
        if let Some(image) = look.image.as_deref().filter(|i| *i != super::images::COVER_REF && !super::images::exists(i)) {
            errors.push(format!("{id}: look.image {image:?} is not stored (add_image or add_animation return the name)"));
        }
        if let Some(label) = look.label.as_deref() {
            let longest = label.lines().map(|l| l.chars().count()).max().unwrap_or(0);
            if longest > 10 && matches!(kind, "keys") {
                warnings.push(format!("{id}: label {label:?} is long for a 72 px key (10 characters a line or fewer read well)"));
            }
        }
        if (look.led.is_some() || look.led_effect.is_some()) && !matches!(kind, "knobs" | "faders") {
            warnings.push(format!("{id}: only knobs and faders have LEDs; look.led does nothing here"));
        }
    }
}

/// Errors and warnings of a whole profile.
pub fn check_profile(p: &Profile) -> (Vec<String>, Vec<String>) {
    let (mut errors, mut warnings) = (Vec::new(), Vec::new());
    if p.name.trim().is_empty() {
        errors.push("a profile needs a name".into());
    }
    for (field, value) in [("edge", &p.edge), ("color", &p.color)] {
        if let Some(v) = value.as_deref().filter(|v| !is_hex(v)) {
            errors.push(format!("{field} {v:?} is not a #rrggbb color"));
        }
    }
    if let Some(effect) = p.edge_effect.as_deref().filter(|e| !super::leds::EFFECTS.iter().any(|(n, _)| n == e)) {
        errors.push(format!("edgeEffect {effect:?} is not a known effect"));
    }
    for page in &p.pages {
        if page.id.is_empty() || page.id.contains(['/', ':', '.']) {
            errors.push(format!("page id {:?}: use a short word without / : .", page.id));
        }
    }
    for (id, b) in &p.controls {
        check_binding(id, b, &p.pages, &mut errors, &mut warnings);
    }
    (errors, warnings)
}

/// Fields of a profile an assistant may set besides its controls.
const PROFILE_FIELDS: [&str; 9] = ["name", "apps", "controls", "pages", "edge", "edgeEffect", "edgeSpeed", "panel", "color"];

/// Overlay `fields` of `body` on `base` (a profile as JSON) and read it back as a profile.
fn merged(base: Value, body: &Value, fields: &[&str]) -> Result<Profile, (u16, String)> {
    let mut v = base;
    for f in fields {
        if let Some(x) = body.get(*f) {
            v[*f] = x.clone();
        }
    }
    serde_json::from_value::<Profile>(v).map_err(|e| bad(format!("not a valid profile: {e}")))
}

fn save_checked(engine: &Engine, mut p: Profile) -> Answer {
    if p.assistant {
        give_bare_keys_an_icon(&mut p);
    }
    let (errors, warnings) = check_profile(&p);
    if !errors.is_empty() {
        return Err(bad(format!("nothing saved; fix these: {}", errors.join("; "))));
    }
    engine.lock().store.save_profile(p.clone()).map_err(|e| (500, e.to_string()))?;
    engine.changed();
    Ok(json!({ "profile": p, "warnings": warnings }))
}

/// A new profile, or with `replace` the profile of that id rewritten (a backup is made first).
fn create_profile(engine: &Engine, body: &Value) -> Answer {
    let replace = body.get("replace").and_then(Value::as_str).filter(|r| !r.is_empty());
    let id = match replace {
        Some(r) => {
            if !engine.lock().store.profiles.contains_key(r) {
                return Err(not_found(format!("no profile {r} to replace")));
            }
            super::backup::create(engine.data_dir(), "before an assistant replaced a profile").map_err(|e| (500, e.to_string()))?;
            r.to_string()
        }
        None => uuid::Uuid::new_v4().to_string(),
    };
    let base = json!({ "version": super::profile::PROFILE_VERSION, "id": id, "name": "" });
    let mut p = merged(base, body, &PROFILE_FIELDS)?;
    // made by the deck assistant: its shortcuts outside the checked packs get a mark
    p.assistant = body.get("assistant").and_then(Value::as_bool) == Some(true);
    // a draft that is a new version of one of the user's profiles: "Use this" replaces that one
    p.version_of = body.get("versionOf").and_then(Value::as_str).filter(|v| !v.is_empty() && *v != id).map(str::to_string);
    // the assistant's limits hold here too, not only in its own tool process (release audit)
    if p.assistant {
        for (control, b) in &p.controls {
            super::guard::check(control, b, &p.apps).map_err(bad)?;
        }
    }
    let answer = save_checked(engine, p)?;
    // a proposal of the deck assistant: kept out of the bar and of switching until picked
    if body.get("draft").and_then(Value::as_bool) == Some(true) {
        let mut s = engine.lock();
        if !s.store.settings.drafts.contains(&id) {
            s.store.settings.drafts.push(id.clone());
        }
        s.store.save_settings().map_err(|e| (500, e.to_string()))?;
        drop(s);
        engine.changed();
    }
    Ok(answer)
}

/// An assistant's key that opens an app or a game but got no picture (the app's icon could not be
/// taken from its program) shows the action's own icon instead of a bare label (persona check
/// before the release, 2026-10-16: five such keys in three personas' drafts).
fn give_bare_keys_an_icon(p: &mut Profile) {
    for (id, b) in p.controls.iter_mut() {
        let key = id.rsplit('/').next().is_some_and(|c| c.starts_with("keys.")) && !id.contains(':');
        let icon = match b.action.as_str() {
            "core.launch" => "app-window",
            "core.game" => "gamepad-2",
            _ => continue,
        };
        if key && b.look.icon.is_none() && b.look.image.is_none() {
            b.look.icon = Some(icon.to_string());
        }
    }
}

fn stored(engine: &Engine, id: &str) -> Result<Profile, (u16, String)> {
    engine.lock().store.profiles.get(id).cloned().ok_or_else(|| not_found(format!("no profile {id}")))
}

/// Change some fields of a profile (name, apps, colors, panel...); its controls stay.
fn update_profile(engine: &Engine, id: &str, body: &Value) -> Answer {
    let p = stored(engine, id)?;
    let fields: Vec<&str> = PROFILE_FIELDS.iter().copied().filter(|f| *f != "controls" && *f != "pages").collect();
    save_checked(engine, merged(json!(p), body, &fields)?)
}

/// `{control, binding}`: set what one control does; `binding: null` clears it.
fn set_control(engine: &Engine, id: &str, body: &Value) -> Answer {
    let control = body.get("control").and_then(Value::as_str).ok_or_else(|| bad("control is missing"))?;
    let mut p = stored(engine, id)?;
    control_kind(control, &p.pages).map_err(bad)?;
    match body.get("binding").filter(|b| !b.is_null()) {
        Some(b) => {
            let binding: Binding = serde_json::from_value(b.clone()).map_err(|e| bad(format!("not a valid binding: {e}")))?;
            let (mut errors, mut warnings) = (Vec::new(), Vec::new());
            check_binding(control, &binding, &p.pages, &mut errors, &mut warnings);
            if !errors.is_empty() {
                return Err(bad(format!("nothing saved; fix these: {}", errors.join("; "))));
            }
            p.controls.insert(control.to_string(), binding);
            if p.assistant {
                give_bare_keys_an_icon(&mut p);
                // a key the assistant fixes afterwards gets the app's real icon too, as the sketch's
                // keys do (2026-10-19: its Spotify key, set again, kept a plain window icon)
                super::assistant::with_real_apps(&mut p);
            }
            engine.lock().store.save_profile(p).map_err(|e| (500, e.to_string()))?;
            engine.changed();
            Ok(json!({ "ok": true, "warnings": warnings }))
        }
        None => {
            let removed = p.controls.remove(control).is_some();
            engine.lock().store.save_profile(p).map_err(|e| (500, e.to_string()))?;
            engine.changed();
            Ok(json!({ "ok": true, "removed": removed }))
        }
    }
}

/// The profile's edge light and color, and the LEDs of knobs and faders: `{edge, edgeEffect,
/// edgeSpeed, color, controls: {"knobs.0": {led, ledEffect, ledSpeed, ledLevel}}}`. A control
/// without a binding gets one that only lights.
fn profile_lights(engine: &Engine, id: &str, body: &Value) -> Answer {
    let mut p = merged(json!(stored(engine, id)?), body, &["edge", "edgeEffect", "edgeSpeed", "color"])?;
    if let Some(controls) = body.get("controls").and_then(Value::as_object) {
        for (control, led) in controls {
            let kind = control_kind(control, &p.pages).map_err(bad)?;
            if !matches!(kind, "knobs" | "faders") || control.contains(':') {
                return Err(bad(format!("{control}: only knobs.N and faders.N have LEDs; the base edge is `edge`")));
            }
            let b = p.controls.entry(control.clone()).or_insert_with(|| Binding { action: String::new(), params: json!({}), look: Default::default(), accel: false, look_on: None, swap_turns: false });
            let mut look = json!(b.look);
            for f in ["led", "ledEffect", "ledSpeed", "ledLevel"] {
                if let Some(v) = led.get(f) {
                    look[f] = v.clone();
                }
            }
            b.look = serde_json::from_value(look).map_err(|e| bad(format!("{control}: {e}")))?;
        }
    }
    save_checked(engine, p)
}

/// Add a key page: `{name, id?}`; returns the page.
fn add_page(engine: &Engine, id: &str, body: &Value) -> Answer {
    let name = body.get("name").and_then(Value::as_str).map(str::trim).filter(|n| !n.is_empty()).ok_or_else(|| bad("a page needs a name"))?;
    let mut p = stored(engine, id)?;
    let page_id = match body.get("id").and_then(Value::as_str).filter(|i| !i.is_empty()) {
        Some(i) if i.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') && i.len() <= 32 => i.to_string(),
        Some(i) => return Err(bad(format!("page id {i:?}: letters, digits, - and _ only"))),
        None => format!("p{}", &uuid::Uuid::new_v4().simple().to_string()[..8]),
    };
    if p.pages.iter().any(|pg| pg.id == page_id) {
        return Err(bad(format!("the profile has a page {page_id} already")));
    }
    let page = super::profile::Page { id: page_id, name: name.to_string() };
    p.pages.push(page.clone());
    engine.lock().store.save_profile(p).map_err(|e| (500, e.to_string()))?;
    engine.changed();
    Ok(json!(page))
}

/// Remove a page and its keys.
fn delete_page(engine: &Engine, id: &str, page: &str) -> Answer {
    let mut p = stored(engine, id)?;
    if !p.pages.iter().any(|pg| pg.id == page) {
        return Err(not_found(format!("no page {page}")));
    }
    p.pages.retain(|pg| pg.id != page);
    let prefix = format!("{page}/");
    p.controls.retain(|c, _| !c.starts_with(&prefix));
    engine.lock().store.save_profile(p).map_err(|e| (500, e.to_string()))?;
    engine.changed();
    Ok(json!({ "ok": true }))
}

/// A short message on the dial and the base edge lit for a while: `{text, detail?, color?, seconds?}`.
fn notify(engine: &Engine, body: &Value) -> Answer {
    let text = body.get("text").and_then(Value::as_str).map(str::trim).filter(|t| !t.is_empty()).ok_or_else(|| bad("text is missing"))?;
    let seconds = body.get("seconds").and_then(Value::as_u64).unwrap_or(8).clamp(1, 120);
    let rgb = body.get("color").and_then(Value::as_str).map(|c| super::leds::parse_hex(c).ok_or_else(|| bad("color is #rrggbb"))).transpose()?.unwrap_or([139, 124, 246]);
    engine.lock().status = Some(super::DeckStatus {
        main: text.chars().take(40).collect(),
        detail: body.get("detail").and_then(Value::as_str).map(|d| d.chars().take(60).collect()),
        until: Some(std::time::Instant::now() + std::time::Duration::from_secs(seconds)),
    });
    engine.notify_light(rgb, seconds);
    engine.deck_ui();
    Ok(json!({"ok": true}))
}

/// A status on the dial until it is cleared: `{text, detail?}`; an empty or missing text clears it.
fn status(engine: &Engine, body: &Value) -> Answer {
    let text = body.get("text").and_then(Value::as_str).map(str::trim).unwrap_or("");
    engine.lock().status =
        (!text.is_empty()).then(|| super::DeckStatus { main: text.chars().take(40).collect(), detail: body.get("detail").and_then(Value::as_str).map(|d| d.chars().take(60).collect()), until: None });
    engine.deck_ui();
    Ok(json!({"ok": true, "cleared": text.is_empty()}))
}

/// Ask the user on the deck: `{question, options: [up to 14], seconds?}` -> `{id}`; the answer comes
/// from `GET /api/ask/<id>` (the request does not wait, so the API stays free).
fn ask(engine: &Engine, body: &Value) -> Answer {
    let question = body.get("question").and_then(Value::as_str).map(str::trim).filter(|q| !q.is_empty()).ok_or_else(|| bad("question is missing"))?;
    let options: Vec<String> = body
        .get("options")
        .and_then(Value::as_array)
        .map(|o| o.iter().filter_map(Value::as_str).map(|s| s.trim().chars().take(24).collect::<String>()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default();
    if options.is_empty() || options.len() > 14 {
        return Err(bad("options: 1 to 14 short answers (they go on the keys)"));
    }
    let seconds = body.get("seconds").and_then(Value::as_u64).unwrap_or(120).clamp(5, 600);
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    engine.lock().prompt = Some(super::Prompt { id, question: question.chars().take(80).collect(), options, answer: None, until: std::time::Instant::now() + std::time::Duration::from_secs(seconds) });
    engine.deck_ui();
    Ok(json!({ "id": id }))
}

/// The answer to a question: `{waiting}` or `{answered, choice, index}` or `{answered: false}` (dismissed or timed out).
fn ask_answer(engine: &Engine, id: &str) -> Answer {
    let s = engine.lock();
    let p = s.prompt.as_ref().filter(|p| p.id.to_string() == id).ok_or_else(|| not_found(format!("no question {id} (a newer one replaced it)")))?;
    Ok(match p.answer {
        None if p.waiting() => json!({"waiting": true}),
        Some(Some(i)) => json!({"waiting": false, "answered": true, "index": i, "choice": p.options[i]}),
        _ => json!({"waiting": false, "answered": false}),
    })
}

/// What the controls are set to now: the active profile and page, the values the faders, knobs
/// and the dial last set (0-100).
fn controls(engine: &Engine) -> Value {
    let s = engine.lock();
    let profile = s.active.as_ref().and_then(|id| s.store.profiles.get(id));
    let mut values: Vec<(String, f32)> = s.levels.iter().map(|(k, v)| (k.clone(), (v * 100.0).round())).collect();
    values.sort_by(|a, b| a.0.cmp(&b.0));
    json!({
        "profile": profile.map(|p| p.name.clone()),
        "page": s.page.clone(),
        "values": values.into_iter().map(|(k, v)| json!({"control": k, "value": v})).collect::<Vec<_>>(),
    })
}

/// The deck-wide lights (style, color, brightness, reactions): the fields given change.
fn set_lights(engine: &Engine, body: &Value) -> Answer {
    let mut v = json!(engine.lock().store.settings.lights);
    merge_json(&mut v, body);
    let mut lights: super::profile::LightSettings = serde_json::from_value(v).map_err(|e| bad(format!("not valid light settings: {e}")))?;
    if !matches!(lights.style.as_str(), "rainbow" | "color" | "profile" | "off") {
        return Err(bad("style is rainbow, color, profile or off"));
    }
    let r = &lights.react;
    for c in [&lights.color, &r.mic.color, &r.live.color, &r.agents.color, &r.timer.color] {
        if !is_hex(c) {
            return Err(bad(format!("{c:?} is not a #rrggbb color")));
        }
    }
    lights.brightness = lights.brightness.min(100);
    {
        let mut s = engine.lock();
        s.store.settings.lights = lights.clone();
        s.store.save_settings().map_err(|e| (500, e.to_string()))?;
    }
    engine.changed();
    Ok(json!(lights))
}

/// Objects merge key by key; anything else replaces.
fn merge_json(base: &mut Value, patch: &Value) {
    match (base, patch) {
        (Value::Object(b), Value::Object(p)) => {
            for (k, v) in p {
                merge_json(b.entry(k.clone()).or_insert(Value::Null), v);
            }
        }
        (b, p) => *b = p.clone(),
    }
}

/// Largest picture an assistant may add (as stored).
const MAX_PICTURE: usize = 15 * 1024 * 1024;
/// Largest SVG text.
const MAX_SVG: usize = 2 * 1024 * 1024;

/// A picture from `{svg}` (text) or `{base64}` (PNG, JPEG, GIF or WebP): its bytes and extension.
fn picture_bytes(v: &Value) -> Result<(Vec<u8>, &'static str), String> {
    if let Some(svg) = v.get("svg").and_then(Value::as_str) {
        if svg.len() > MAX_SVG {
            return Err(format!("the SVG is {} KB; the limit is {} KB", svg.len() / 1024, MAX_SVG / 1024));
        }
        super::render::Renderer::default().rgba(svg, Some((8, 8))).map_err(|e| format!("not a readable SVG: {e}"))?;
        return Ok((svg.as_bytes().to_vec(), "svg"));
    }
    let b64 = v.get("base64").or_else(|| v.get("png")).and_then(Value::as_str).ok_or("give svg (text) or base64 (a PNG, JPEG, GIF or WebP file)")?;
    let b64 = b64.split_once("base64,").map_or(b64, |(_, data)| data).trim();
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64).map_err(|e| format!("not base64: {e}"))?;
    if bytes.len() > MAX_PICTURE {
        return Err(format!("the picture is {} MB; the limit is 15 MB", bytes.len() / 1_048_576));
    }
    let ext = match image::guess_format(&bytes).map_err(|e| format!("not a picture: {e}"))? {
        image::ImageFormat::Png => "png",
        image::ImageFormat::Jpeg => "jpg",
        image::ImageFormat::Gif => "gif",
        image::ImageFormat::WebP => "webp",
        other => return Err(format!("{other:?} is not supported (PNG, JPEG, GIF, WebP or SVG)")),
    };
    image::load_from_memory(&bytes).map_err(|e| format!("cannot read this picture: {e}"))?;
    Ok((bytes, ext))
}

/// Store a picture; returns the name for `look.image` (or a picture panel).
fn add_image(body: &Value) -> Answer {
    let (bytes, ext) = picture_bytes(body).map_err(bad)?;
    let name = super::images::import_bytes(&bytes, ext).map_err(|e| (500, e))?;
    Ok(json!({ "name": name }))
}

/// `{name}`: the icon of an app or the cover of a game HubDeck knows (installed, or seen in
/// front: by its name or program), stored for look.image. Only names from those lists, no paths.
fn app_picture(body: &Value) -> Answer {
    let name = body.get("name").and_then(Value::as_str).map(str::trim).filter(|n| !n.is_empty()).ok_or_else(|| bad("name is missing (an app or game from list_apps / list_games)"))?;
    picture_of(name).map(|n| json!({ "name": n }))
}

/// The real icon of one of the user's apps or the cover of a game (by name or program), stored
/// as a picture; its name.
pub fn picture_of(name: &str) -> Result<String, (u16, String)> {
    let lower = name.to_lowercase();
    let same = |s: &str| !s.is_empty() && (s.to_lowercase() == lower || s.rsplit(['\\', '/']).next().is_some_and(|f| f.to_lowercase() == lower));
    let found = super::installed::list(false);
    let installed = found.games.iter().chain(found.apps.iter()).find(|i| same(&i.name) || same(&i.exe));
    let from_installed = installed.map(super::installed::picture);
    if let Some(Ok(picture)) = from_installed {
        return Ok(picture);
    }
    // a Microsoft Store app has no program file to take the icon from (Spotify: a plain window
    // icon on its key); the program it runs as, seen in front, has one
    let exe = installed.map(|i| i.exe.as_str()).filter(|e| !e.is_empty()).map(str::to_lowercase).unwrap_or_else(|| format!("{lower}.exe"));
    let seen = super::apps::list().into_iter().find(|a| same(&a.exe) || a.name.as_deref().is_some_and(same) || a.exe.to_lowercase() == exe);
    let seen = match (seen, from_installed) {
        (Some(s), _) => s,
        (None, Some(Err(e))) => return Err((500, e)),
        (None, _) => return Err(not_found(format!("{name}: not an app or game HubDeck knows; see list_apps and list_games"))),
    };
    let png = super::appicon::png(&seen.path).map_err(|e| (500, e))?;
    super::images::import_bytes(&png, "png").map_err(|e| (500, e))
}

/// Size of animation frames for each target.
pub fn animation_size(target: &str) -> Option<(u32, u32)> {
    match target {
        "key" => Some((80, 80)),
        "dial" => Some((super::render::DIAL_PX, super::render::DIAL_PX)),
        "panel" => Some((super::render::PANEL_W, super::render::PANEL_H)),
        _ => None,
    }
}

/// Most frames in one animation, and the fastest rate the deck shows smoothly.
pub const MAX_ANIMATION_FRAMES: usize = 120;
pub const MAX_FPS: u32 = 15;

/// Frames -> an animated GIF: `{frames: [{svg} | {base64}], fps, target: key|dial|panel, loop}`.
/// SVG frames are drawn at the target size, pictures scaled to fit it, on black.
pub fn compose_gif(body: &Value) -> Result<(Vec<u8>, Value), String> {
    let frames = body.get("frames").and_then(Value::as_array).filter(|f| !f.is_empty()).ok_or("frames is missing (a list of {svg} or {base64})")?;
    if frames.len() > MAX_ANIMATION_FRAMES {
        return Err(format!("{} frames; the limit is {MAX_ANIMATION_FRAMES}", frames.len()));
    }
    let target = body.get("target").and_then(Value::as_str).unwrap_or("key");
    let (w, h) = animation_size(target).ok_or("target is key, dial or panel")?;
    let fps = body.get("fps").and_then(Value::as_u64).unwrap_or(10).clamp(1, MAX_FPS as u64) as u32;
    let looped = body.get("loop").and_then(Value::as_bool).unwrap_or(true);
    let renderer = super::render::Renderer::default();
    let mut out = Vec::new();
    {
        let mut encoder = image::codecs::gif::GifEncoder::new_with_speed(&mut out, 10);
        encoder.set_repeat(if looped { image::codecs::gif::Repeat::Infinite } else { image::codecs::gif::Repeat::Finite(0) }).map_err(|e| e.to_string())?;
        for (n, f) in frames.iter().enumerate() {
            let picture = match f.get("svg").and_then(Value::as_str) {
                Some(svg) if svg.len() <= MAX_SVG => renderer.rgba(svg, Some((w, h))).map_err(|e| format!("frame {n}: {e}"))?,
                Some(_) => return Err(format!("frame {n}: the SVG is too big")),
                None => {
                    let (bytes, _) = picture_bytes(f).map_err(|e| format!("frame {n}: {e}"))?;
                    image::load_from_memory(&bytes).map_err(|e| format!("frame {n}: {e}"))?.resize(w, h, image::imageops::FilterType::Lanczos3).to_rgba8()
                }
            };
            let mut canvas = image::RgbaImage::from_pixel(w, h, image::Rgba([0, 0, 0, 255]));
            let (x, y) = ((w - picture.width().min(w)) / 2, (h - picture.height().min(h)) / 2);
            image::imageops::overlay(&mut canvas, &picture, x as i64, y as i64);
            let delay = image::Delay::from_numer_denom_ms(1000, fps);
            encoder.encode_frame(image::Frame::from_parts(canvas, 0, 0, delay)).map_err(|e| format!("frame {n}: {e}"))?;
        }
    }
    if out.len() > MAX_PICTURE {
        return Err(format!("the animation is {} MB; the limit is 15 MB (fewer frames or a smaller target)", out.len() / 1_048_576));
    }
    let info = json!({ "frames": frames.len(), "fps": fps, "width": w, "height": h, "bytes": out.len() });
    Ok((out, info))
}

fn add_animation(body: &Value) -> Answer {
    let (gif, mut info) = compose_gif(body).map_err(bad)?;
    info["name"] = json!(super::images::import_bytes(&gif, "gif").map_err(|e| (500, e))?);
    Ok(info)
}

/// What a key or the dial shows: `{profile, control}` (a stored binding) or `{look, screen}`.
/// Returns the SVG and a PNG (base64) of it.
fn preview(engine: &Engine, body: &Value) -> Answer {
    let (look, screen) = match (body.get("profile").and_then(Value::as_str), body.get("control").and_then(Value::as_str)) {
        (Some(id), Some(control)) => {
            let p = stored(engine, id)?;
            let kind = control_kind(control, &p.pages).map_err(bad)?;
            if !matches!(kind, "keys" | "dial") {
                return Err(bad(format!("{control}: only keys and the dial have screens; knobs and faders show look.led")));
            }
            let b = p.controls.get(control).ok_or_else(|| not_found(format!("{control} has no binding in this profile")))?;
            let state = body.get("on").and_then(Value::as_bool);
            (b.look_for(state), kind)
        }
        _ => {
            let look: super::profile::Look = serde_json::from_value(body.get("look").cloned().unwrap_or(json!({}))).map_err(|e| bad(format!("not a valid look: {e}")))?;
            (look, if body.get("screen").and_then(Value::as_str) == Some("dial") { "dial" } else { "keys" })
        }
    };
    let svg = if screen == "dial" { super::render::dial_svg(&look, None, 0) } else { super::render::key_svg(&look, 0) };
    let png = super::render::Renderer::default().png(&svg).map_err(|e| (500, e))?;
    Ok(json!({ "screen": if screen == "dial" { "dial" } else { "key" }, "svg": svg, "png": base64::engine::general_purpose::STANDARD.encode(png) }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_assistants_app_key_without_a_picture_gets_an_icon() {
        let mut p: Profile = serde_json::from_value(json!({"id": "x", "name": "x", "controls": {
            "keys.0": {"action": "core.launch", "params": {"target": "spotify.exe"}, "look": {"label": "Spotify"}},
            "p1/keys.2": {"action": "core.game", "params": {"game": "steam://run/1"}, "look": {}},
            "keys.1": {"action": "core.launch", "params": {}, "look": {"image": "a.png"}},
            "keys.3": {"action": "core.hotkey", "params": {}, "look": {}},
            "keys.4:hold": {"action": "core.launch", "params": {}, "look": {}}
        }}))
        .unwrap();
        give_bare_keys_an_icon(&mut p);
        let icon = |c: &str| p.controls[c].look.icon.clone();
        assert_eq!(icon("keys.0").as_deref(), Some("app-window"));
        assert_eq!(icon("p1/keys.2").as_deref(), Some("gamepad-2"));
        assert_eq!(icon("keys.1"), None, "its own picture stays");
        assert_eq!(icon("keys.3"), None, "only apps and games");
        assert_eq!(icon("keys.4:hold"), None, "a hold shows no face");
    }

    #[test]
    fn kinds_join_attached_and_remembered_modules() {
        let k = kinds_from(["base", "keys", "dial"], ["faders@1-1-3", "keys@2-3", "rollers@owned", "base@x"]);
        assert_eq!(k.attached, ["keys", "dial"]);
        assert_eq!(k.owned, ["keys", "dial", "faders", "rollers"], "in catalog order, no base, once each");
        assert_eq!(kinds_from([], []), ModuleKinds::default());
    }

    fn page(id: &str) -> super::super::profile::Page {
        super::super::profile::Page { id: id.into(), name: id.into() }
    }

    #[test]
    fn control_ids_are_checked() {
        let pages = [page("tools")];
        for ok in ["keys.0", "keys.14", "dial", "knobs.2", "faders.4", "rollers.1", "keys.3:hold", "dial:double", "knobs.0:hold", "tools/keys.2", "tools/keys.2:double"] {
            assert!(control_kind(ok, &pages).is_ok(), "{ok}");
        }
        for bad in ["keys.15", "keys", "dial.0", "knobs.3", "faders.-1", "keys.+1", "rollers.0:hold", "faders.1:double", "keys.1:long", "other/keys.1", "tools/dial", "base", "keys.1.2"] {
            assert!(control_kind(bad, &pages).is_err(), "{bad}");
        }
        assert_eq!(control_kind("tools/keys.2:hold", &pages), Ok("keys"));
    }

    fn binding(v: Value) -> Binding {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn profiles_from_an_assistant_are_checked_before_saving() {
        let mut p = Profile::new("Edit");
        p.controls.insert("keys.0".into(), binding(json!({"action": "core.media", "params": {"command": "next"}, "look": {"label": "Next", "icon": "play", "bg": "#1d2b3a", "style": "glass"}})));
        p.controls.insert("faders.0".into(), binding(json!({"action": "core.volume", "params": {"target": "system"}, "look": {"led": "#20ff40", "ledLevel": true}})));
        p.controls.insert("keys.1".into(), binding(json!({"action": "core.volume", "params": {"target": "system", "turnOnPress": 1}, "look": {"label": "Vol +"}})));
        p.controls.insert("keys.2".into(), binding(json!({"action": "", "look": {"label": "Hi", "icon": "game:crowned-skull"}})));
        let (errors, warnings) = check_profile(&p);
        assert!(errors.is_empty(), "{errors:?}");
        assert!(warnings.is_empty(), "{warnings:?}");

        let mut wrong = p.clone();
        wrong.controls.insert("keys.3".into(), binding(json!({"action": "core.nothing"})));
        wrong.controls.insert("keys.4".into(), binding(json!({"action": "core.media", "look": {"icon": "no-such-icon", "bg": "red", "style": "shiny"}})));
        wrong.controls.insert("faders.1".into(), binding(json!({"action": "core.media"})));
        wrong.controls.insert("keys.5".into(), binding(json!({"action": "core.scroll"})));
        wrong.controls.insert("keys.6".into(), binding(json!({"action": "core.hotkey", "params": {"keys": "Ctrl+Nope"}})));
        wrong.controls.insert("keys.7".into(), binding(json!({"action": "core.media", "params": {"command": "rewind"}})));
        wrong.controls.insert("knobs.0".into(), binding(json!({"action": "core.volume", "look": {"ledEffect": "disco", "image": "0000000000000000.png"}})));
        wrong.controls.insert("keys.99".into(), binding(json!({"action": "core.media"})));
        wrong.edge = Some("#12345".into());
        let (errors, _) = check_profile(&wrong);
        let all = errors.join("\n");
        for needle in [
            "unknown action \"core.nothing\"",
            "unknown icon \"no-such-icon\"",
            "look.bg \"red\"",
            "look.style \"shiny\"",
            "faders.1: core.media reacts",
            "turnOnPress",
            "Ctrl+Nope",
            "core.media.command must be one of",
            "\"disco\"",
            "is not stored",
            "keys.99",
            "edge \"#12345\"",
        ] {
            assert!(all.contains(needle), "{needle} in:\n{all}");
        }

        let mut long = Profile::new("Long");
        long.controls.insert("keys.0".into(), binding(json!({"action": "core.media", "look": {"label": "Previous track please", "led": "#ff0000"}})));
        let (errors, warnings) = check_profile(&long);
        assert!(errors.is_empty());
        assert_eq!(warnings.len(), 2, "a long label and an LED on a key: {warnings:?}");
    }

    #[test]
    fn animations_become_gifs_of_the_target_size() {
        let frame = |c: &str| json!({"svg": format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><circle cx="5" cy="5" r="4" fill="{c}"/></svg>"#)});
        let (gif, info) = compose_gif(&json!({"frames": [frame("red"), frame("blue"), frame("lime")], "fps": 60, "target": "dial"})).unwrap();
        assert_eq!(info["fps"], MAX_FPS, "fps is capped");
        assert_eq!((info["width"].as_u64(), info["height"].as_u64()), (Some(240), Some(240)));
        let decoded = image::codecs::gif::GifDecoder::new(std::io::Cursor::new(&gif)).unwrap();
        let frames = image::AnimationDecoder::into_frames(decoded).collect_frames().unwrap();
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0].buffer().dimensions(), (240, 240));
        let centre = frames[0].buffer().get_pixel(120, 120);
        assert!(centre[0] > 200 && centre[2] < 60, "the first frame is red in the middle: {centre:?}");
        // a PNG frame is scaled to fit
        let mut png = std::io::Cursor::new(Vec::new());
        image::RgbaImage::from_pixel(4, 2, image::Rgba([0, 0, 255, 255])).write_to(&mut png, image::ImageFormat::Png).unwrap();
        let b64 = base64::engine::general_purpose::STANDARD.encode(png.into_inner());
        let (_, info) = compose_gif(&json!({"frames": [{"base64": b64}], "target": "key"})).unwrap();
        assert_eq!(info["width"], 80);
        assert!(compose_gif(&json!({"frames": []})).is_err());
        assert!(compose_gif(&json!({"frames": [frame("red")], "target": "wall"})).is_err());
        assert!(compose_gif(&json!({"frames": vec![frame("red"); MAX_ANIMATION_FRAMES + 1]})).is_err());
        assert!(compose_gif(&json!({"frames": [{"svg": "<svg"}]})).unwrap_err().contains("frame 0"));
    }

    #[test]
    fn pictures_are_validated_and_settings_merge() {
        assert_eq!(picture_bytes(&json!({"svg": r#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"/>"#})).unwrap().1, "svg");
        assert!(picture_bytes(&json!({"svg": "not svg"})).is_err());
        assert!(picture_bytes(&json!({"base64": "aGVsbG8="})).is_err(), "text is not a picture");
        assert!(picture_bytes(&json!({})).is_err());
        let mut png = std::io::Cursor::new(Vec::new());
        image::RgbaImage::new(2, 2).write_to(&mut png, image::ImageFormat::Png).unwrap();
        let uri = format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png.into_inner()));
        assert_eq!(picture_bytes(&json!({"base64": uri})).unwrap().1, "png", "a data URI works too");
        let mut lights = json!(super::super::profile::LightSettings::default());
        merge_json(&mut lights, &json!({"brightness": 40, "react": {"mic": {"color": "#00ff00"}}}));
        let merged: super::super::profile::LightSettings = serde_json::from_value(lights).unwrap();
        assert_eq!((merged.brightness, merged.react.mic.color.as_str(), merged.react.mic.on), (40, "#00ff00", true));
    }
}
