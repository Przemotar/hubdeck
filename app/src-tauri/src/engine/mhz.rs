//! Import of profiles exported by the official MasterHUB app (`.mhz`), so switching to HubDeck
//! costs nothing. The format is described in `docs/OFFICIAL-APP.md` > "The .mhz file": a zip with
//! one JSON file per profile plus the pictures it uses. The official profile is keyed by module
//! serial numbers; HubDeck binds control kinds, so the first module of each kind is taken.
//!
//! Actions are translated where HubDeck has an equivalent (hotkeys, open, text, media, volume,
//! scroll, multi-task, pages and folders, OBS). Anything else becomes an empty binding whose label
//! says what it was ("Not supported: Clear chat"), so nothing disappears silently.

use std::collections::HashMap;
use std::io::{Cursor, Read};

use serde_json::{Value, json};

use super::profile::{Binding, Look, Page, Profile};

/// Delay the official app puts between the steps of a multi-task.
const MULTI_TASK_DELAY_MS: u64 = 280;
/// Step of the official volume controls (fixed 5 % per detent).
const VOLUME_STEP: u64 = 5;

/// Read a `.mhz` file: one profile, or several when it holds other `.mhz` files (multi-export).
/// `store_image(bytes, extension)` keeps a picture and returns its name for a look.
pub fn import(bytes: &[u8], file_name: &str, store_image: &mut dyn FnMut(&[u8], &str) -> Option<String>) -> Result<Vec<Profile>, String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| format!("not a MasterHUB profile (.mhz): {e}"))?;
    let mut json_entry: Option<Vec<u8>> = None;
    let mut images: HashMap<String, Vec<u8>> = HashMap::new();
    let mut inner: Vec<(String, Vec<u8>)> = Vec::new();
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().replace('\\', "/");
        let mut data = Vec::new();
        entry.read_to_end(&mut data).map_err(|e| e.to_string())?;
        let base = name.rsplit('/').next().unwrap_or(&name).to_string();
        if name.starts_with("uploadImages/") {
            images.insert(base, data);
        } else if name.starts_with("gifPools/") {
            // pre-cut animation frames of the official app; the original picture is in uploadImages
        } else if base.to_lowercase().ends_with(".mhz") {
            inner.push((base, data));
        } else if json_entry.is_none() {
            json_entry = Some(data);
        }
    }
    if !inner.is_empty() {
        let mut all = Vec::new();
        for (name, data) in inner {
            all.extend(import(&data, &name, store_image)?);
        }
        return Ok(all);
    }
    let json = json_entry.ok_or("the .mhz file holds no profile")?;
    let value: Value = serde_json::from_slice(&json).map_err(|e| format!("the profile inside the .mhz file is not readable: {e}"))?;
    if !value.is_object() || (value.get("keypad").is_none() && value.get("knobsList").is_none() && value.get("dialEncoder").is_none()) {
        return Err("the .mhz file holds no MasterHUB profile".into());
    }
    let mut pictures = |name: &str| -> Option<String> {
        let base = name.replace('\\', "/");
        let base = base.rsplit('/').next()?;
        let bytes = images.get(base)?;
        let ext = base.rsplit('.').next().unwrap_or("png").to_lowercase();
        let ext = if ext == "jpeg" { "jpg".to_string() } else { ext };
        store_image(bytes, &ext)
    };
    Ok(vec![convert(&value, &profile_name(file_name), &mut pictures)])
}

/// `masterHubConfig_profile_Streaming.mhz` -> `Streaming`.
fn profile_name(file_name: &str) -> String {
    let stem = file_name.rsplit(['/', '\\']).next().unwrap_or(file_name);
    let stem = stem.strip_suffix(".mhz").or_else(|| stem.strip_suffix(".MHZ")).unwrap_or(stem);
    let name = stem.rsplit_once("_profile_").map(|(_, n)| n).unwrap_or(stem).trim();
    if name.is_empty() { "MasterHUB profile".into() } else { format!("{name} (MasterHUB)") }
}

/// The first module of a kind: `{serial: data}` -> data (sorted by serial so the choice is stable).
fn first_module(v: &Value, key: &str) -> Option<Value> {
    let map = v.get(key)?.as_object()?;
    let mut serials: Vec<&String> = map.keys().collect();
    serials.sort();
    serials.into_iter().map(|s| &map[s]).find(|m| !m.is_null()).cloned()
}

fn convert(v: &Value, name: &str, pictures: &mut dyn FnMut(&str) -> Option<String>) -> Profile {
    let mut profile = Profile::new(name);
    if let Some(pages) = first_module(v, "keypad").and_then(|p| p.as_array().cloned()) {
        keys(&mut profile, &pages, pictures);
    }
    if let Some(knobs) = first_module(v, "knobsList") {
        // the official app reads knob events as data[2] = rotateB, [3] = rotateR, [4] = rotateG
        // (App.vue, replyM003IoStatus); HubDeck's knobs.N is data[2 + N]
        for (i, key) in ["rotateB", "rotateR", "rotateG"].iter().enumerate() {
            if let Some(b) = knobs.get(key).and_then(|k| turn_control(k, "right", "left")) {
                profile.controls.insert(format!("knobs.{i}"), b);
            }
        }
    }
    if let Some(dial) = first_module(v, "dialEncoder") {
        let mut b = turn_control(&dial, "right", "left");
        if str_of(&dial, "lcdDisplayMode") == "Image" {
            let file = [str_of(&dial, "gifPath"), str_of(&dial, "imageName")].into_iter().find(|s| !s.is_empty()).unwrap_or("");
            if let Some(image) = (!file.is_empty()).then(|| pictures(file)).flatten() {
                let binding = b.get_or_insert_with(|| empty(""));
                binding.look.image = Some(image);
            }
        }
        if let Some(b) = b {
            profile.controls.insert("dial".into(), b);
        }
    }
    if let Some(rollers) = first_module(v, "rollerBlock") {
        // official: data[2] = "right" roller, data[3] = "left"; HubDeck's rollers.N is data[2 + N]
        for (i, side) in ["right", "left"].iter().enumerate() {
            if let Some(b) = rollers.get(side).and_then(|r| {
                // the left roller's action is sometimes stored next to the two sides
                let mut r = r.clone();
                if str_of(&r, "btnFunc").is_empty() && *side == "left" {
                    r["btnFunc"] = json!(str_of(&rollers, "btnFunc"));
                }
                single_turn(&r, "up", "down")
            }) {
                profile.controls.insert(format!("rollers.{i}"), b);
            }
        }
    }
    if let Some(faders) = first_module(v, "sliders5List") {
        for i in 0..5 {
            if let Some(b) = faders.get(i.to_string()).and_then(fader) {
                profile.controls.insert(format!("faders.{i}"), b);
            }
        }
    }
    profile
}

fn str_of<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

fn int_of(v: &Value, key: &str) -> Option<i64> {
    v.get(key).and_then(|x| x.as_i64().or_else(|| x.as_f64().map(|f| f as i64)).or_else(|| x.as_str().and_then(|s| s.parse().ok())))
}

/// "chat-message" -> "Chat message".
fn readable(action: &str) -> String {
    let words = action.replace('-', " ");
    let mut chars = words.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => "Unknown action".into(),
    }
}

/// An empty binding that shows what the official app had there.
fn empty(what: &str) -> Binding {
    let look = if what.is_empty() {
        Look::default()
    } else {
        Look { label: Some(format!("Not supported: {what}")), icon: Some("x".into()), bg: Some("#3a3a40".into()), fg: Some("#b8b8c0".into()), ..Look::default() }
    };
    Binding { action: String::new(), params: json!({}), look, accel: false, look_on: None, swap_turns: false }
}

fn binding(action: &str, params: Value, label: Option<String>) -> Binding {
    Binding { action: action.into(), params, look: Look { label, ..Look::default() }, accel: false, look_on: None, swap_turns: false }
}

/// One key name of the official app (recorded or built in) -> HubDeck's name; None = unknown.
fn key_name(name: &str) -> Option<String> {
    let raw = name.trim();
    let n = raw.to_lowercase().replace("vk_", "").replace(['_', ' '], "");
    let mapped = match n.as_str() {
        "ctrl" | "control" | "lcontrol" | "rcontrol" | "leftcontrol" | "rightcontrol" | "lctrl" | "rctrl" => "Ctrl",
        "shift" | "lshift" | "rshift" | "leftshift" | "rightshift" => "Shift",
        "alt" | "menu" | "lmenu" | "rmenu" | "lalt" | "ralt" | "leftalt" | "rightalt" => "Alt",
        "win" | "lwin" | "rwin" | "super" | "leftsuper" | "rightsuper" | "meta" | "leftwin" | "rightwin" | "cmd" => "Win",
        "add" | "plus" | "oemplus" | "equal" | "=" => "=",
        "minus" | "subtract" | "oemminus" | "-" => "-",
        "prior" | "pageup" => "PageUp",
        "next" | "pagedown" => "PageDown",
        "return" | "enter" => "Enter",
        "escape" | "esc" => "Escape",
        "back" | "backspace" => "Backspace",
        "delete" | "del" => "Delete",
        "space" | "spacebar" => "Space",
        "tab" => "Tab",
        "home" => "Home",
        "end" => "End",
        "up" | "arrowup" => "Up",
        "down" | "arrowdown" => "Down",
        "left" | "arrowleft" => "Left",
        "right" | "arrowright" => "Right",
        "snapshot" | "print" | "printscreen" => "PrintScreen",
        "audioplay" | "audiopause" | "audioplaypause" | "mediaplaypause" | "playpause" => "PlayPause",
        "audionext" | "medianexttrack" | "nexttrack" => "NextTrack",
        "audioprev" | "mediaprevtrack" | "prevtrack" => "PrevTrack",
        "audiovolup" | "volumeup" => "VolumeUp",
        "audiovoldown" | "volumedown" => "VolumeDown",
        "audiomute" | "volumemute" => "VolumeMute",
        "oemcomma" | "comma" => ",",
        "oemperiod" | "period" => ".",
        "oem1" | "semicolon" => ";",
        "oem2" | "slash" => "/",
        "oem4" | "leftbracket" => "[",
        "oem6" | "rightbracket" => "]",
        "oem7" | "quote" => "'",
        "oem5" | "backslash" => "\\",
        "oem3" | "grave" => "`",
        _ => "",
    };
    if !mapped.is_empty() {
        return Some(mapped.to_string());
    }
    if let Some(d) = n.strip_prefix("numpad").filter(|d| d.len() == 1 && d.chars().all(|c| c.is_ascii_digit())) {
        return Some(d.to_string());
    }
    if let Some(f) = n.strip_prefix('f').and_then(|f| f.parse::<u8>().ok()).filter(|f| (1..=24).contains(f)) {
        return Some(format!("F{f}"));
    }
    let mut chars = raw.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if c != '+' => Some(c.to_ascii_uppercase().to_string()),
        _ => None,
    }
}

/// `{key: ["Control", "Z"], code: [..]}` -> `Ctrl+Z`, modifiers first; None when empty or unknown.
fn combo(hotkey: Option<&Value>) -> Option<String> {
    let keys = hotkey?.get("key")?.as_array()?;
    let names: Vec<String> = keys.iter().map(|k| k.as_str().and_then(key_name)).collect::<Option<_>>()?;
    if names.is_empty() {
        return None;
    }
    let is_mod = |k: &String| ["Ctrl", "Shift", "Alt", "Win"].contains(&k.as_str());
    let (mods, rest): (Vec<String>, Vec<String>) = names.into_iter().partition(is_mod);
    let mut out = mods;
    out.dedup();
    match rest.as_slice() {
        [] => {}
        [one] => out.push(one.clone()),
        _ => return None, // two plain keys at once cannot be sent as one combination
    }
    Some(out.join("+"))
}

/// Actions whose official meaning is "send the stored hotkey".
fn is_hotkey_action(f: &str) -> bool {
    matches!(f, "hot-key" | "undo-redo" | "switch-desktop" | "zoom-in-out" | "mh-zoom-in-out" | "screen-capture") || f.starts_with("ps-") || f.starts_with("ai-") || f.starts_with("pr-")
}

/// The official key's label, if it shows one.
fn title(btn: &Value) -> Option<String> {
    if btn.get("enableTitle").and_then(Value::as_bool) == Some(false) {
        return None;
    }
    [str_of(btn, "title"), str_of(btn, "btnName")].into_iter().map(str::trim).find(|t| !t.is_empty()).map(str::to_string)
}

fn rgb_hex(v: Option<&Value>) -> Option<String> {
    let v = v?;
    let c = |k: &str| v.get(k).and_then(Value::as_f64).map(|x| x.clamp(0.0, 255.0) as u8);
    Some(format!("#{:02x}{:02x}{:02x}", c("r")?, c("g")?, c("b")?))
}

/// A key press action (also the steps of a multi-task). Err = not supported, with a readable name.
fn press_action(btn: &Value) -> Result<(String, Value), String> {
    let f = str_of(btn, "btnFunc");
    let option = int_of(btn, "dropdownOption").unwrap_or(0);
    let ok = |action: &str, params: Value| Ok((action.to_string(), params));
    match f {
        "" => Err(String::new()),
        "web-url" => {
            let url = str_of(btn, "url").trim();
            if url.is_empty() {
                return Err("Web URL (empty)".into());
            }
            let url = if url.contains("://") { url.to_string() } else { format!("https://{url}") };
            ok("core.launch", json!({"target": url}))
        }
        "app-files" if str_of(btn, "actionAppMode") != "Close" => ok("core.launch", json!({"target": str_of(btn, "appFilePath")})),
        "text" => {
            let mut text = str_of(btn, "content").to_string();
            if btn.get("afterEnter").and_then(Value::as_bool) == Some(true) {
                text.push('\n');
            }
            ok("core.text", json!({"text": text}))
        }
        "hot-key" if btn.get("hotKeySettingMode").and_then(Value::as_str) == Some("macro") => Err("Recorded macro".into()),
        "media-track-control" => {
            let command = match option {
                2 => "previous",
                3 => "next",
                5 => "stop",
                _ => "playPause",
            };
            ok("core.media", json!({"command": command}))
        }
        "volume-control" => {
            let app = int_of(btn, "volumeControlProcessId").unwrap_or(0) != 0;
            match (option, app) {
                (0, false) => ok("core.hotkey", json!({"keys": "VolumeUp"})),
                (1, false) => ok("core.hotkey", json!({"keys": "VolumeDown"})),
                (_, false) => ok("core.volume", json!({"target": "system"})),
                // the official app stores a process id, which means nothing after a restart
                (0 | 1, true) => Err("App volume up / down".into()),
                (_, true) => ok("core.volume", json!({"target": "app", "app": ""})),
            }
        }
        "scroll-up-down" => ok("core.hotkey", json!({"keys": if option == 0 { "PageUp" } else { "PageDown" }})),
        "media-fast-back-forward" => ok("core.seek", json!({"player": super::seek::from_official(str_of(btn, "mediaPlayerId")), "press": if option == 1 { "back" } else { "forward" }})),
        f if is_hotkey_action(f) => match combo(btn.get("hotkey")) {
            Some(keys) => ok("core.hotkey", json!({"keys": keys})),
            None => Err(readable(f)),
        },
        "record-start-stop" => ok("obs.record", json!({})),
        "stream" => ok("obs.stream", json!({})),
        "replay-buffer-save" => ok("obs.replay", json!({})),
        "scenes" => ok("obs.scene", json!({"scene": str_of(btn, "selectSceneId")})),
        // OBS sources are stored by OBS's internal id; the name has to be picked again
        "source-visibility" => ok("obs.source", json!({"source": "", "scene": ""})),
        "audio-mixer" if str_of(btn, "TypeID") == "Mute" => ok("obs.mute", json!({"input": ""})),
        "brightness" => ok("core.deckBrightness", json!({"press": if option == 1 { "darker" } else { "brighter" }, "step": 10})),
        // official options: 0 off, 1 static, 2 breathing, 3 strobe, 5 cycling ... 9 rainbow; 10/11 = next/previous
        "lighting-effect" if option <= 9 => {
            let effect = ["off", "static", "breathing", "strobe", "blink", "cycling", "random", "wave", "water", "rainbow"][option.max(0) as usize];
            ok("core.lighting", json!({"target": "all", "effect": effect, "color": "#ffffff", "speed": 50}))
        }
        "timer" => ok(super::actions::TIMER, timer_params(btn)),
        // "Play Audio": a sound file, play/pause or play/stop; HubDeck stops on the second press
        "play-pause" if !str_of(btn, "appFilePath").is_empty() => ok("core.sound", json!({"mode": "playStop", "file": str_of(btn, "appFilePath"), "volume": 80, "device": ""})),
        "clock" => ok(super::actions::CLOCK, json!({"show": "timeDate", "hours": "24"})),
        // the official key names one of its own profiles by number: pick the HubDeck one again
        "profile" => ok("core.profile", json!({"which": "named", "profile": ""})),
        "record-continue-pause" => ok("obs.recordPause", json!({})),
        "virtual-camera" => ok("obs.virtualCam", json!({})),
        "replay-buffer" => ok("obs.replayBuffer", json!({})),
        "studio-mode" => ok("obs.studioMode", json!({})),
        "transition" => ok("obs.transition", json!({"transition": ""})),
        "scene-collection" => ok("obs.sceneCollection", json!({"collection": str_of(btn, "SceneCollectionTitle")})),
        "obs-profile" => ok("obs.profile", json!({"profile": str_of(btn, "profileTitle")})),
        // the filter's source is stored by OBS's internal id; its name has to be typed again
        "filter" => ok("obs.filter", json!({"source": "", "filter": str_of(btn, "filterTitle")})),
        "screenshot" => ok("obs.screenshot", json!({"source": "", "folder": ""})),
        "chat-message" => ok("twitch.chat", json!({"message": str_of(btn, "twitchMessage")})),
        "clear-chat" => ok("twitch.clearChat", json!({})),
        "followers-chat" => ok("twitch.chatMode", json!({"mode": "followers", "duration": int_of(btn, "followersChatDuration").unwrap_or(0)})),
        "sub-chat" => ok("twitch.chatMode", json!({"mode": "subscribers"})),
        "emote-chat" => ok("twitch.chatMode", json!({"mode": "emote"})),
        "slow-chat" => ok("twitch.chatMode", json!({"mode": "slow", "duration": int_of(btn, "slowChatDuration").unwrap_or(30)})),
        "shield-mode" => ok("twitch.shield", json!({})),
        "play-ad" => ok("twitch.ad", json!({"length": "30"})),
        "stream-game-title" => ok("twitch.title", json!({"title": "", "game": ""})),
        "create-stream-marker" => ok("twitch.marker", json!({"description": ""})),
        "create-clip" => ok("twitch.clip", json!({})),
        "open-last-clip" => ok("twitch.openClip", json!({})),
        "streamlabs-record" => ok("streamlabs.record", json!({})),
        "streamlabs-stream" => ok("streamlabs.stream", json!({})),
        "streamlabs-studio-mode" => ok("streamlabs.studioMode", json!({})),
        "streamlabs-transition" => ok("streamlabs.transition", json!({})),
        // Streamlabs scenes and sources are stored by Streamlabs' internal ids: names to fill in
        "streamlabs-scenes" => ok("streamlabs.scene", json!({"scene": ""})),
        "streamlabs-source-visibility" => ok("streamlabs.source", json!({"source": "", "scene": ""})),
        "streamlabs-audio-mixer" if str_of(btn, "streamlabsAudioTypeId") == "Mute" => ok("streamlabs.mute", json!({"input": ""})),
        other => Err(readable(other)),
    }
}

/// The official timer's length (`countDownSecondDuration`, seconds) and sound switch.
fn timer_params(v: &Value) -> Value {
    let secs = int_of(v, "countDownSecondDuration").unwrap_or(60).clamp(1, 359_999);
    let sound = [v.get("isTimerSoundOn"), v.get("dialEncoderIsTimerSoundOn")].into_iter().flatten().find_map(Value::as_bool).unwrap_or(true);
    json!({"minutes": secs / 60, "seconds": secs % 60, "press": "startPause", "sound": if sound { "on" } else { "off" }})
}

/// A key: its action and look. None = an empty key.
fn key_binding(btn: &Value, pictures: &mut dyn FnMut(&str) -> Option<String>) -> Option<Binding> {
    let f = str_of(btn, "btnFunc");
    let mut b = if f == "multi-tasking" {
        let rows = btn.get("actionList").and_then(Value::as_array).cloned().unwrap_or_default();
        let steps: Vec<Value> = rows
            .iter()
            .filter_map(|row| press_action(row).ok())
            .enumerate()
            .map(|(i, (action, params))| json!({"action": action, "params": params, "delay": if i == 0 { 0 } else { MULTI_TASK_DELAY_MS }}))
            .collect();
        if steps.is_empty() { empty("Multi-task") } else { binding(super::actions::MULTI, json!({"steps": steps}), None) }
    } else {
        match press_action(btn) {
            Ok((action, params)) => binding(&action, params, None),
            Err(what) if what.is_empty() => empty(""),
            Err(what) => empty(&what),
        }
    };
    let image = Some(str_of(btn, "gifPath")).filter(|s| !s.is_empty()).and_then(&mut *pictures);
    if b.look.label.is_none() {
        b.look.label = title(btn);
    }
    if image.is_some() {
        b.look.image = image;
    }
    // colors only when they differ from the official defaults (dark key, white text)
    if let Some(bg) = rgb_hex(btn.get("btnColor")).filter(|c| c != "#222223") {
        b.look.bg = Some(bg);
    }
    if let Some(fg) = rgb_hex(btn.get("btnTextColor")).filter(|c| c != "#ffffff") {
        b.look.fg = Some(fg);
    }
    let blank = b.action.is_empty() && b.look == Look::default();
    (!blank).then_some(b)
}

/// The keys: main page, extra pages, and folders (which become pages with a back key).
fn keys(profile: &mut Profile, pages: &[Value], pictures: &mut dyn FnMut(&str) -> Option<String>) {
    let page_id = |i: usize| if i == 0 { String::new() } else { format!("mhz-page-{}", i + 1) };
    let count = pages.len().max(1);
    for i in 1..pages.len() {
        profile.pages.push(Page { id: page_id(i), name: format!("Page {}", i + 1) });
    }
    for (p, page) in pages.iter().enumerate() {
        let here = page_id(p);
        let on_page = |control: String| if here.is_empty() { control } else { format!("{here}/{control}") };
        for (slot, btn) in page.as_array().map(Vec::as_slice).unwrap_or_default().iter().enumerate() {
            let index = int_of(btn, "id").map(|i| i as usize).filter(|i| *i < 15).unwrap_or(slot);
            let control = on_page(format!("keys.{index}"));
            let f = str_of(btn, "btnFunc");
            let nav = match f {
                "next-page" => Some(page_id((p + 1) % count)),
                "previous-page" => Some(page_id((p + count - 1) % count)),
                _ => None,
            };
            if let Some(target) = nav {
                let mut b = binding("core.page", json!({"page": target}), title(btn));
                b.look.icon = Some(if f == "next-page" { "skip-forward" } else { "arrow-left" }.into());
                profile.controls.insert(control, b);
                continue;
            }
            if f == "new-folder" {
                let folder_id = format!("mhz-folder-{}-{index}", p + 1);
                let name = title(btn).unwrap_or_else(|| format!("Folder {}", index + 1));
                profile.pages.push(Page { id: folder_id.clone(), name: name.clone() });
                let mut b = binding("core.page", json!({"page": folder_id}), Some(name));
                b.look.icon = Some("folder".into());
                profile.controls.insert(control, b);
                for (fslot, item) in btn.get("folder").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default().iter().enumerate() {
                    let findex = int_of(item, "id").map(|i| i as usize).filter(|i| *i < 15).unwrap_or(fslot);
                    let fcontrol = format!("{folder_id}/keys.{findex}");
                    if matches!(str_of(item, "btnFunc"), "new-folder" | "previous-page") {
                        let mut back = binding("core.page", json!({"page": here}), Some("Back".into()));
                        back.look.icon = Some("arrow-left".into());
                        profile.controls.insert(fcontrol, back);
                    } else if let Some(b) = key_binding(item, pictures) {
                        profile.controls.insert(fcontrol, b);
                    }
                }
                continue;
            }
            if f == "create-new-page" {
                profile.controls.insert(control, empty("Create new page"));
                continue;
            }
            if let Some(b) = key_binding(btn, pictures) {
                profile.controls.insert(control, b);
            }
        }
    }
}

/// One turn function (`cw` / `ccw` are the official direction keys: right/left or up/down).
fn turn_action(func_row: &Value, cw: &str, ccw: &str) -> Result<(String, Value), String> {
    let func = str_of(func_row, "btnFunc");
    let app = int_of(func_row, "volumeControlProcessId").unwrap_or(0) != 0;
    let ok = |action: &str, params: Value| Ok((action.to_string(), params));
    match func {
        "" => Err(String::new()),
        "volume-control" if app => ok("core.volume", json!({"target": "app", "app": "", "step": VOLUME_STEP})),
        "volume-control" => ok("core.volume", json!({"target": "system", "step": VOLUME_STEP})),
        "scroll-up-down" => ok("core.scroll", json!({"axis": "vertical", "amount": 3})),
        "media-fast-back-forward" => ok("core.seek", json!({"player": super::seek::from_official(str_of(func_row, "mediaPlayerId")), "press": "forward"})),
        "media-track-control" => ok("core.media", json!({"command": "playPause"})),
        "audio-mixer" => ok("obs.volume", json!({"input": "", "step": 1})),
        "streamlabs-audio-mixer" => ok("streamlabs.volume", json!({"input": "", "step": 2})),
        "brightness" => ok("core.deckBrightness", json!({"step": 10})),
        "timer" => ok(super::actions::TIMER, timer_params(func_row)),
        "profile" => ok("core.profile", json!({"which": "next"})),
        name if is_hotkey_action(name) => {
            let side = |k: &str| func_row.get(k).and_then(|s| combo(s.get("hotkey")));
            match (side(cw), side(ccw)) {
                (None, None) => Err(readable(func)),
                (a, b) => ok("core.hotkey", json!({"keys": "", "clockwise": a.unwrap_or_default(), "counterClockwise": b.unwrap_or_default()})),
            }
        }
        other => Err(readable(other)),
    }
}

/// A roller: one function with `up` / `down`.
fn single_turn(r: &Value, cw: &str, ccw: &str) -> Option<Binding> {
    match turn_action(r, cw, ccw) {
        Ok((action, params)) => Some(binding(&action, params, title(r))),
        Err(what) if what.is_empty() => None,
        Err(what) => Some(empty(&what)),
    }
}

/// A knob or the dial: a list of functions, a press goes to the next one (HubDeck's Modes).
fn turn_control(k: &Value, cw: &str, ccw: &str) -> Option<Binding> {
    let list = k.get("functionList").and_then(Value::as_array).cloned().unwrap_or_default();
    if list.is_empty() {
        return single_turn(k, cw, ccw);
    }
    let mut modes = Vec::new();
    let mut skipped = Vec::new();
    for f in &list {
        match turn_action(f, cw, ccw) {
            Ok((action, params)) => {
                let label = [str_of(f, "btnName"), str_of(f, "label")].into_iter().find(|s| !s.trim().is_empty()).map(str::to_string).unwrap_or_else(|| readable(str_of(f, "btnFunc")));
                modes.push(json!({"label": label, "action": action, "params": params}));
            }
            Err(what) if !what.is_empty() => skipped.push(what),
            Err(_) => {}
        }
    }
    match modes.len() {
        0 if skipped.is_empty() => None,
        0 => Some(empty(&skipped.join(", "))),
        1 => {
            let m = &modes[0];
            Some(binding(str_of(m, "action"), m["params"].clone(), None))
        }
        _ => Some(binding(super::actions::MODES, json!({"modes": modes}), None)),
    }
}

/// A fader: what it sets.
fn fader(f: &Value) -> Option<Binding> {
    let func = str_of(f, "btnFunc");
    let app = int_of(f, "volumeControlProcessId").unwrap_or(0) != 0;
    let (action, params) = match func {
        "" => return None,
        "volume-control" if app => ("core.volume", json!({"target": "app", "app": ""})),
        "volume-control" => ("core.volume", json!({"target": "system"})),
        "audio-mixer" => ("obs.volume", json!({"input": ""})),
        "streamlabs-audio-mixer" => ("streamlabs.volume", json!({"input": ""})),
        "brightness" => ("core.deckBrightness", json!({})),
        other => return Some(empty(&readable(other))),
    };
    Some(binding(action, params, title(f)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// A synthetic `.mhz` (never an official file): the profile JSON plus pictures.
    fn mhz(profile: &Value, images: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        {
            let mut z = zip::ZipWriter::new(&mut buf);
            let opts = zip::write::SimpleFileOptions::default();
            for (name, bytes) in images {
                z.start_file(format!("uploadImages/{name}"), opts).unwrap();
                z.write_all(bytes).unwrap();
            }
            z.start_file("profile2_BASE123", opts).unwrap();
            z.write_all(&serde_json::to_vec(profile).unwrap()).unwrap();
            z.finish().unwrap();
        }
        buf.into_inner()
    }

    fn key(id: usize, func: &str, extra: Value) -> Value {
        let mut k = json!({"id": id, "title": "", "btnFunc": func, "enableTitle": true, "folder": [], "hotkey": {"key": [], "code": []},
            "btnColor": {"r": 34, "g": 34, "b": 35}, "btnTextColor": {"r": 255, "g": 255, "b": 255}, "gifPath": ""});
        for (k2, v) in extra.as_object().unwrap() {
            k[k2] = v.clone();
        }
        k
    }

    fn sample() -> Value {
        let page1 = vec![
            key(0, "hot-key", json!({"title": "Save", "hotkey": {"key": ["Ctrl", "S"], "code": [17, 83]}})),
            key(1, "web-url", json!({"url": "example.com"})),
            key(2, "media-track-control", json!({"dropdownOption": 3})),
            key(3, "chat-message", json!({})),
            key(14, "discord-mute", json!({})),
            key(
                4,
                "multi-tasking",
                json!({"actionList": [
                    key(0, "text", json!({"content": "hi", "afterEnter": true})),
                    key(0, "voice-mod", json!({})),
                    key(0, "undo-redo", json!({"hotkey": {"key": ["Control", "Z"], "code": [17, 90]}}))
                ]}),
            ),
            key(
                5,
                "new-folder",
                json!({"title": "Tools", "folder": [
                    key(0, "new-folder", json!({})),
                    key(1, "app-files", json!({"appFilePath": "C:\\Tools\\a.exe", "actionAppMode": "Open"}))
                ]}),
            ),
            key(6, "next-page", json!({})),
            key(7, "scenes", json!({"selectSceneId": "Gaming", "btnColor": {"r": 255, "g": 0, "b": 0}})),
            key(10, "obs-profile", json!({"profileTitle": "Twitch"})),
            key(11, "brightness", json!({"dropdownOption": 1})),
            key(13, "streamlabs-stream", json!({})),
            key(12, "timer", json!({"countDownSecondDuration": 125, "isTimerSoundOn": false})),
            key(8, "", json!({"gifPath": "C:/Users/x/AppData/Roaming/MasterHUB/uploadImages/cat.png"})),
            key(9, "", json!({})),
        ];
        let page2 = vec![key(0, "previous-page", json!({})), key(1, "screen-capture", json!({"hotkey": {"key": ["LeftSuper", "Shift", "S"], "code": [91, 56, 31]}}))];
        json!({
            "serialNumber": "BASE123",
            "keypad": {"KEYS1": [page1, page2]},
            "knobsList": {"KNOBS1": {
                "rotateR": {"functionList": [
                    {"btnFunc": "volume-control", "btnName": "Volume", "volumeControlProcessId": 0, "left": {}, "right": {}},
                    {"btnFunc": "hot-key", "btnName": "Zoom", "left": {"hotkey": {"key": ["Control", "Minus"]}}, "right": {"hotkey": {"key": ["Control", "Add"]}}}
                ]},
                "rotateG": {"functionList": [{"btnFunc": "twitch-thing", "left": {}, "right": {}}]},
                "rotateB": {"functionList": []}
            }},
            "dialEncoder": {"DIAL1": {"functionList": [{"btnFunc": "scroll-up-down", "left": {}, "right": {}}], "lcdDisplayMode": "Image", "gifPath": "dial.gif"}},
            "rollerBlock": {"ROLL1": {"btnFunc": "volume-control", "left": {"volumeControlProcessId": 0, "up": {}, "down": {}}, "right": {"btnFunc": "", "up": {}, "down": {}}}},
            "sliders5List": {"FAD1": {"0": {"btnFunc": "volume-control", "volumeControlProcessId": 0}, "1": {"btnFunc": "discord-input-volume-control"}, "2": {"btnFunc": ""}}}
        })
    }

    #[test]
    fn imports_a_synthetic_mhz() {
        let file = mhz(&sample(), &[("cat.png", b"PNGDATA"), ("dial.gif", b"GIFDATA")]);
        let mut stored = Vec::new();
        let mut store = |bytes: &[u8], ext: &str| {
            stored.push((bytes.to_vec(), ext.to_string()));
            Some(format!("img{}.{ext}", stored.len()))
        };
        let profiles = import(&file, "masterHubConfig_profile_Streaming.mhz", &mut store).unwrap();
        assert_eq!(profiles.len(), 1);
        let p = &profiles[0];
        assert_eq!(p.name, "Streaming (MasterHUB)");
        let c = |id: &str| p.controls.get(id).unwrap_or_else(|| panic!("{id} missing"));

        assert_eq!(c("keys.0").action, "core.hotkey");
        assert_eq!(c("keys.0").params["keys"], "Ctrl+S");
        assert_eq!(c("keys.0").look.label.as_deref(), Some("Save"));
        assert_eq!(c("keys.1").params["target"], "https://example.com");
        assert_eq!(c("keys.2").params["command"], "next");
        assert_eq!(c("keys.3").action, "twitch.chat");

        let steps = c("keys.4").params["steps"].as_array().unwrap();
        assert_eq!(c("keys.4").action, "core.multi");
        assert_eq!(steps.len(), 2, "the unsupported step is dropped");
        assert_eq!(steps[0]["params"]["text"], "hi\n");
        assert_eq!(steps[1]["params"]["keys"], "Ctrl+Z");
        assert_eq!(steps[1]["delay"], MULTI_TASK_DELAY_MS);

        let folder = c("keys.5").params["page"].as_str().unwrap().to_string();
        assert!(p.pages.iter().any(|pg| pg.id == folder && pg.name == "Tools"));
        assert_eq!(c(&format!("{folder}/keys.0")).params["page"], "", "back to the main page");
        assert_eq!(c(&format!("{folder}/keys.1")).params["target"], "C:\\Tools\\a.exe");

        assert_eq!(c("keys.6").params["page"], "mhz-page-2");
        assert_eq!(c("mhz-page-2/keys.0").params["page"], "", "previous of page 2 is the main page");
        assert_eq!(c("mhz-page-2/keys.1").params["keys"], "Win+Shift+S");

        assert_eq!(c("keys.7").params["scene"], "Gaming");
        assert_eq!(c("keys.10").action, "obs.profile");
        assert_eq!(c("keys.10").params["profile"], "Twitch");
        assert_eq!(c("keys.11").params["press"], "darker");
        assert_eq!(c("keys.13").action, "streamlabs.stream");
        assert_eq!(c("keys.14").action, "");
        assert_eq!(c("keys.14").look.label.as_deref(), Some("Not supported: Discord mute"));
        assert_eq!((c("keys.12").params["minutes"].clone(), c("keys.12").params["seconds"].clone(), c("keys.12").params["sound"].clone()), (json!(2), json!(5), json!("off")));
        assert_eq!(c("keys.7").look.bg.as_deref(), Some("#ff0000"));
        assert_eq!(c("keys.0").look.bg, None, "default colors are not copied");
        assert_eq!(c("keys.8").look.image.as_deref(), Some("img1.png"));
        assert!(!p.controls.contains_key("keys.9"), "empty keys stay empty");

        let knob = c("knobs.1");
        assert_eq!(knob.action, "core.modes");
        let modes = knob.params["modes"].as_array().unwrap();
        assert_eq!(modes[0]["action"], "core.volume");
        assert_eq!(modes[1]["params"]["clockwise"], "Ctrl+=");
        assert_eq!(modes[1]["params"]["counterClockwise"], "Ctrl+-");
        assert_eq!(c("knobs.2").look.label.as_deref(), Some("Not supported: Twitch thing"));
        assert!(!p.controls.contains_key("knobs.0"));

        assert_eq!(c("dial").action, "core.scroll");
        assert_eq!(c("dial").look.image.as_deref(), Some("img2.gif"));
        assert_eq!(c("rollers.1").action, "core.volume");
        assert!(!p.controls.contains_key("rollers.0"));
        assert_eq!(c("faders.0").params["target"], "system");
        assert_eq!(c("faders.1").action, "");
        assert!(!p.controls.contains_key("faders.2"));
    }

    #[test]
    fn multi_export_holds_several_profiles() {
        let one = mhz(&sample(), &[]);
        let mut buf = Cursor::new(Vec::new());
        {
            let mut z = zip::ZipWriter::new(&mut buf);
            let opts = zip::write::SimpleFileOptions::default();
            for name in ["masterHubConfig_profile_A.mhz", "masterHubConfig_profile_B.mhz"] {
                z.start_file(name, opts).unwrap();
                z.write_all(&one).unwrap();
            }
            z.finish().unwrap();
        }
        let profiles = import(&buf.into_inner(), "all.mhz", &mut |_, _| None).unwrap();
        assert_eq!(profiles.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["A (MasterHUB)", "B (MasterHUB)"]);
    }

    #[test]
    fn rejects_files_that_are_not_mhz() {
        assert!(import(b"{}", "x.mhz", &mut |_, _| None).is_err());
        let not_profile = mhz(&json!({"hello": 1}), &[]);
        assert!(import(&not_profile, "x.mhz", &mut |_, _| None).is_err());
    }

    #[test]
    fn key_names_follow_hubdeck() {
        assert_eq!(combo(Some(&json!({"key": ["Shift", "Control", "Z"]}))).as_deref(), Some("Shift+Ctrl+Z"));
        assert_eq!(combo(Some(&json!({"key": ["VK_CONTROL", "VK_NEXT"]}))).as_deref(), Some("Ctrl+PageDown"));
        assert_eq!(combo(Some(&json!({"key": ["F13"]}))).as_deref(), Some("F13"));
        assert_eq!(combo(Some(&json!({"key": ["NumPad7"]}))).as_deref(), Some("7"));
        assert_eq!(combo(Some(&json!({"key": ["AudioNext"]}))).as_deref(), Some("NextTrack"));
        assert_eq!(combo(Some(&json!({"key": ["Ctrl", "Hyper"]}))), None);
        assert_eq!(combo(Some(&json!({"key": []}))), None);
        for name in ["Ctrl+Z", "Win+Shift+S", "Ctrl+=", "Ctrl+-", "PageDown", "NextTrack", "F13"] {
            assert!(super::super::hotkey::parse(name).is_some(), "{name} is sendable");
        }
    }
}
