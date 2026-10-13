//! A new profile for one app, made in one click ("+" > For an app...): the app's own preset when
//! there is one, else a starter for its kind of app (browser, editor, chat, media player, game,
//! other).
//! Either way it is linked to the app (switches in while it is in front), named after it, and
//! adapted to the modules the user has.

use serde_json::json;
use tauri::State;

use super::Engine;
use super::profile::{Binding, Look, Profile};

/// What kind of app an executable is, from its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppKind {
    Browser,
    Editor,
    Chat,
    Media,
    /// Any game: from a launcher's library or folder, or known to Windows as one.
    Game,
    Other,
}

const BROWSERS: [&str; 9] = ["chrome", "firefox", "msedge", "opera", "opera_gx", "brave", "vivaldi", "arc", "librewolf"];
const EDITORS: [&str; 16] =
    ["code", "cursor", "notepad", "notepad++", "sublime_text", "devenv", "idea64", "pycharm64", "webstorm64", "rider64", "clion64", "winword", "excel", "powerpnt", "obsidian", "zed"];
const CHATS: [&str; 9] = ["discord", "slack", "ms-teams", "teams", "zoom", "telegram", "whatsapp", "signal", "skype"];
const MEDIA: [&str; 9] = ["spotify", "vlc", "foobar2000", "musicbee", "itunes", "aimp", "winamp", "tidal", "deezer"];

fn stem(exe: &str) -> String {
    let base = exe.rsplit(['\\', '/']).next().unwrap_or(exe).to_lowercase();
    base.strip_suffix(".exe").unwrap_or(&base).to_string()
}

/// Launch links of game launchers.
const GAME_LINKS: [&str; 7] = ["steam://", "com.epicgames.launcher://", "goggalaxy://", "battlenet://", "uplay://", "origin://", "ea://"];
/// Folders games are installed in by their launchers.
const GAME_FOLDERS: [&str; 9] =
    ["\\steamapps\\common\\", "\\epic games\\", "\\gog galaxy\\games\\", "\\gog games\\", "\\riot games\\", "\\ubisoft game launcher\\games\\", "\\ea games\\", "\\xboxgames\\", "\\battle.net\\"];

/// Programs Windows knows to be games, read again at most once a minute (suggestions ask often).
fn windows_games() -> Vec<String> {
    use std::sync::Mutex;
    use std::time::{Duration, Instant};
    static CACHE: Mutex<Option<(Instant, Vec<String>)>> = Mutex::new(None);
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((at, list)) = cache.as_ref()
        && at.elapsed() < Duration::from_secs(60)
    {
        return list.clone();
    }
    let list = read_windows_games();
    *cache = Some((Instant::now(), list.clone()));
    list
}

/// Programs Windows knows to be games (the Game Bar keeps them under `GameConfigStore`).
#[cfg(windows)]
fn read_windows_games() -> Vec<String> {
    use std::os::windows::process::CommandExt;
    let out = std::process::Command::new("reg")
        .args(["query", r"HKCU\System\GameConfigStore\Children", "/s", "/v", "MatchedExeFullPath"])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
        .output();
    out.map(|o| reg_values(&String::from_utf8_lossy(&o.stdout))).unwrap_or_default()
}

#[cfg(not(windows))]
fn read_windows_games() -> Vec<String> {
    Vec::new()
}

/// The `REG_SZ` values in `reg query` output.
#[cfg_attr(not(windows), allow(dead_code))]
fn reg_values(out: &str) -> Vec<String> {
    out.lines().filter_map(|l| l.split_once("REG_SZ")).map(|(_, v)| v.trim().to_string()).filter(|v| !v.is_empty()).collect()
}

/// Whether `exe` (a name or a path), opened with `target`, is a game.
pub fn is_game(exe: &str, target: &str) -> bool {
    let (e, t) = (exe.to_lowercase().replace('/', "\\"), target.to_lowercase().replace('/', "\\"));
    if GAME_LINKS.iter().any(|l| target.to_lowercase().starts_with(l)) || GAME_FOLDERS.iter().any(|f| e.contains(f) || t.contains(f)) {
        return true;
    }
    let name = stem(exe);
    let same = |path: &str| !name.is_empty() && stem(path) == name;
    super::installed::games().iter().any(|g| same(&g.exe)) || windows_games().iter().any(|g| same(g))
}

pub fn kind_of(exe: &str) -> AppKind {
    let s = stem(exe);
    let is = |list: &[&str]| list.contains(&s.as_str());
    if is(&BROWSERS) {
        AppKind::Browser
    } else if is(&EDITORS) {
        AppKind::Editor
    } else if is(&CHATS) {
        AppKind::Chat
    } else if is(&MEDIA) {
        AppKind::Media
    } else {
        AppKind::Other
    }
}

const APP: &str = "#2b2733";
const NAV: &str = "#1f2a33";
const EDIT: &str = "#262333";
const MEDIA_BG: &str = "#1d2b3a";
const TALK: &str = "#2a1f33";

fn bind(action: &str, params: serde_json::Value, label: &str, icon: &str, bg: &str) -> Binding {
    Binding { action: action.into(), params, look: Look { label: Some(label.into()), icon: Some(icon.into()), bg: Some(bg.into()), ..Look::default() }, accel: false, look_on: None, swap_turns: false }
}

fn hotkey(keys: &str, label: &str, icon: &str, bg: &str) -> Binding {
    bind("core.hotkey", json!({"keys": keys}), label, icon, bg)
}

/// The keys that suit an app of this kind (after the app's own key, before the media row).
fn kind_keys(kind: AppKind, exe: &str) -> Vec<Binding> {
    let cmd = if cfg!(target_os = "macos") { "Cmd" } else { "Ctrl" };
    let s = stem(exe);
    match kind {
        AppKind::Browser => vec![
            hotkey(&format!("{cmd}+T"), "New tab", "plus", NAV),
            hotkey(&format!("{cmd}+W"), "Close tab", "x", NAV),
            hotkey(&format!("{cmd}+Shift+T"), "Reopen tab", "rotate-ccw", NAV),
            hotkey("Alt+Left", "Back", "arrow-left", NAV),
            hotkey("F5", "Reload", "refresh-cw", NAV),
            hotkey(&format!("{cmd}+F"), "Find", "search", NAV),
            hotkey(&format!("{cmd}+L"), "Address", "globe", NAV),
            hotkey(&format!("{cmd}+Shift+N"), "Private", "lock", NAV),
        ],
        AppKind::Editor => {
            let redo = if s == "code" || s == "cursor" || s == "zed" { format!("{cmd}+Shift+Z") } else { format!("{cmd}+Y") };
            vec![
                hotkey(&format!("{cmd}+S"), "Save", "save", EDIT),
                hotkey(&format!("{cmd}+Z"), "Undo", "undo-2", EDIT),
                hotkey(&redo, "Redo", "redo-2", EDIT),
                hotkey(&format!("{cmd}+F"), "Find", "search", EDIT),
                hotkey(&format!("{cmd}+H"), "Replace", "replace", EDIT),
                hotkey(&format!("{cmd}+C"), "Copy", "copy", EDIT),
                hotkey(&format!("{cmd}+V"), "Paste", "clipboard-paste", EDIT),
                hotkey(&format!("{cmd}+N"), "New", "file-plus", EDIT),
            ]
        }
        AppKind::Chat => {
            let (mute, camera, extra): (Option<&str>, Option<&str>, Vec<Binding>) = match s.as_str() {
                "discord" => (Some("Ctrl+Shift+M"), None, vec![hotkey("Ctrl+Shift+D", "Deafen", "headphones", TALK), hotkey("Ctrl+K", "Switch", "search", TALK)]),
                "ms-teams" | "teams" => (Some("Ctrl+Shift+M"), Some("Ctrl+Shift+O"), vec![hotkey("Ctrl+Shift+K", "Raise hand", "star", TALK), hotkey("Ctrl+Shift+H", "Leave", "x", TALK)]),
                "zoom" => (Some("Alt+A"), Some("Alt+V"), vec![hotkey("Alt+S", "Share", "monitor", TALK), hotkey("Alt+Q", "Leave", "x", TALK)]),
                "slack" => (None, None, vec![hotkey("Ctrl+K", "Switch", "search", TALK), hotkey("Ctrl+Shift+A", "Unread", "bell", TALK)]),
                _ => (None, None, Vec::new()),
            };
            let mut keys = vec![match mute {
                Some(k) => hotkey(k, "Mute", "mic-off", TALK),
                None => bind("core.mic", json!({}), "Mic", "mic", TALK),
            }];
            if let Some(c) = camera {
                keys.push(hotkey(c, "Camera", "video", TALK));
            }
            keys.extend(extra);
            keys
        }
        AppKind::Media => {
            let mut keys = vec![
                bind("core.media", json!({"command": "previous"}), "Prev", "skip-back", MEDIA_BG),
                bind("core.media", json!({"command": "playPause"}), "Play", "play", MEDIA_BG),
                bind("core.media", json!({"command": "next"}), "Next", "skip-forward", MEDIA_BG),
            ];
            if s == "spotify" {
                keys.push(bind("spotify.like", json!({}), "Like", "heart", MEDIA_BG));
            }
            keys.push(bind("core.media", json!({"command": "stop"}), "Stop", "square", MEDIA_BG));
            keys
        }
        AppKind::Game => Vec::new(), // see game_starter
        AppKind::Other => vec![
            hotkey(&format!("{cmd}+C"), "Copy", "copy", EDIT),
            hotkey(&format!("{cmd}+V"), "Paste", "clipboard-paste", EDIT),
            hotkey(&format!("{cmd}+Z"), "Undo", "undo-2", EDIT),
            hotkey(&format!("{cmd}+S"), "Save", "save", EDIT),
            hotkey(&format!("{cmd}+F"), "Find", "search", EDIT),
        ],
    }
}

/// A game without a preset of its own: keys that mean the same in most games (menu, map,
/// inventory), capture with the Windows Game Bar, Discord's mute and deafen, the media row; the
/// game, Discord and music volumes on the turning controls.
fn game_starter(exe: &str, name: &str, target: &str, picture: Option<String>) -> Profile {
    use super::presets::{game_key, game_preset};
    const MENU: &str = "#2a1f33";
    const CAPTURE: &str = "#1f2a33";
    let mut p = game_preset(name, &[exe], exe, &[]);
    let mut app_key = bind("core.launch", json!({"target": target}), name, "gamepad-2", APP);
    if let Some(image) = picture {
        app_key.look.image = Some(image);
        app_key.look.icon = None;
    }
    let keys = [
        app_key,
        game_key("Escape", "Menu", "game:cog", MENU),
        game_key("Tab", "Tab", "game:compass", MENU),
        game_key("M", "Map", "game:treasure-map", MENU),
        game_key("I", "Inventory", "game:backpack", MENU),
        // Windows: the Game Bar; macOS: its own screenshot and screen recording keys
        hotkey(if cfg!(target_os = "macos") { "Cmd+Shift+3" } else { "Win+Alt+PrintScreen" }, "Screenshot", "game:photo-camera", CAPTURE),
        hotkey(if cfg!(target_os = "macos") { "Cmd+Shift+5" } else { "Win+Alt+G" }, if cfg!(target_os = "macos") { "Capture" } else { "Clip 30 s" }, "game:film-strip", CAPTURE),
        hotkey(if cfg!(target_os = "macos") { "Cmd+Shift+5" } else { "Win+Alt+R" }, "Record", "game:video-camera", CAPTURE),
        hotkey("Ctrl+Shift+M", "Discord mute", "mic-off", TALK),
        hotkey("Ctrl+Shift+D", "Deafen", "game:headphones", TALK),
        bind("core.media", json!({"command": "previous"}), "Prev", "skip-back", MEDIA_BG),
        bind("core.media", json!({"command": "playPause"}), "Play", "play", MEDIA_BG),
        bind("core.media", json!({"command": "next"}), "Next", "skip-forward", MEDIA_BG),
        hotkey("VolumeMute", "Mute", "volume-x", MEDIA_BG),
        bind("core.mic", json!({}), "Mic", "mic", MEDIA_BG),
    ];
    for (i, b) in keys.into_iter().enumerate() {
        p.controls.insert(format!("keys.{i}"), b);
    }
    p
}

/// A starter profile for `exe` of `kind`: its own key, keys for its kind, a media row, volumes on
/// the turning controls. `target` opens the app; `picture` (a picture name) shows on its key.
pub fn starter(kind: AppKind, exe: &str, name: &str, target: &str, picture: Option<String>) -> Profile {
    let mut p = starter_profile(kind, exe, name, target, picture);
    super::house::apply(&mut p);
    p
}

fn starter_profile(kind: AppKind, exe: &str, name: &str, target: &str, picture: Option<String>) -> Profile {
    if kind == AppKind::Game {
        return game_starter(exe, name, target, picture);
    }
    let mut p = Profile::new(name);
    p.apps = vec![exe.to_string()];
    let mut app_key = bind("core.launch", json!({"target": target}), name, "app-window", APP);
    if let Some(image) = picture {
        app_key.look.image = Some(image);
        app_key.look.icon = None;
    }
    let mut keys = vec![app_key];
    keys.extend(kind_keys(kind, exe));
    // the bottom row: media and sound, unless the app is a media player (it has them already)
    if kind != AppKind::Media {
        keys.truncate(10);
        while keys.len() < 10 {
            keys.push(Binding { action: String::new(), params: json!({}), look: Look::default(), accel: false, look_on: None, swap_turns: false });
        }
        keys.extend([
            bind("core.media", json!({"command": "previous"}), "Prev", "skip-back", MEDIA_BG),
            bind("core.media", json!({"command": "playPause"}), "Play", "play", MEDIA_BG),
            bind("core.media", json!({"command": "next"}), "Next", "skip-forward", MEDIA_BG),
            hotkey("VolumeMute", "Mute", "volume-x", MEDIA_BG),
            bind("core.mic", json!({}), "Mic", "mic", MEDIA_BG),
        ]);
    }
    for (i, b) in keys.into_iter().enumerate().take(15) {
        // empty slots stay free for the user's own keys
        if !b.action.is_empty() {
            p.controls.insert(format!("keys.{i}"), b);
        }
    }
    let volume = |target: &str, app: &str, label: &str| bind("core.volume", json!({"target": target, "app": app, "step": 2}), label, "volume-2", "#1f1d26");
    p.controls.insert("dial".into(), volume("app", exe, name));
    p.controls.insert("knobs.0".into(), volume("app", exe, name));
    p.controls.insert("knobs.1".into(), volume("system", "", "System"));
    p.controls.insert("knobs.2".into(), bind("core.scroll", json!({"axis": "horizontal", "amount": 3}), "Scroll ↔", "arrow-left-right", "#1f1d26"));
    for (i, (target, app, label)) in
        [("system", "", "System"), ("app", exe, name), ("app", "Discord.exe", "Discord"), ("app", "Spotify.exe", "Spotify"), ("app", "chrome.exe", "Browser")].into_iter().enumerate()
    {
        p.controls.insert(format!("faders.{i}"), volume(target, app, label));
    }
    p.controls.insert("rollers.0".into(), bind("core.scroll", json!({"axis": "vertical", "amount": 3}), "Scroll", "arrow-up-down", "#1f1d26"));
    p.controls.insert("rollers.1".into(), bind("core.scroll", json!({"axis": "horizontal", "amount": 3}), "Scroll ↔", "arrow-left-right", "#1f1d26"));
    p
}

/// The preset made for this app, if any (its apps list names the executable).
pub fn preset_for(exe: &str) -> Option<Profile> {
    let name = exe.rsplit(['\\', '/']).next().unwrap_or(exe);
    super::presets::catalog().into_iter().filter(|p| !p.test).map(|p| p.make()).find(|p| p.app_match(name, "").is_some())
}

/// The profile for one app: its preset or a starter, named after it, linked to it, adapted to
/// `kinds` (the module kinds the user has; None = as built).
pub fn for_app(exe: &str, name: &str, target: &str, picture: Option<String>, kinds: Option<&[String]>) -> Profile {
    let exe_name = exe.rsplit(['\\', '/']).next().unwrap_or(exe).to_string();
    let mut p = match preset_for(&exe_name) {
        Some(mut preset) => {
            preset.id = uuid::Uuid::new_v4().to_string();
            preset
        }
        None => {
            let kind = if is_game(exe, target) { AppKind::Game } else { kind_of(&exe_name) };
            super::localize::profile(starter(kind, &exe_name, name, target, picture))
        }
    };
    // the app's own name, never translated
    p.name = name.to_string();
    if !p.apps.iter().any(|a| a.eq_ignore_ascii_case(&exe_name)) {
        p.apps.insert(0, exe_name);
    }
    match kinds {
        Some(k) if !k.is_empty() => super::presets::adapt(p, k),
        _ => p,
    }
}

/// "+" > For an app...: make the profile, save it and return it (the window activates it).
#[tauri::command]
pub fn profile_for_app(engine: State<'_, Engine>, exe: String, name: String, target: String, picture: Option<String>, kinds: Option<Vec<String>>) -> Result<Profile, String> {
    if exe.trim().is_empty() {
        return Err("this app has no program to link the profile to".into());
    }
    let name = if name.trim().is_empty() { stem(&exe) } else { name.trim().to_string() };
    let profile = for_app(&exe, &name, &target, picture, kinds.as_deref());
    engine.lock().store.save_profile(profile.clone()).map_err(|e| e.to_string())?;
    engine.changed();
    Ok(profile)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::render;

    #[test]
    fn knows_kinds_of_apps() {
        assert_eq!(kind_of("chrome.exe"), AppKind::Browser);
        assert_eq!(kind_of(r"C:\Program Files\Microsoft VS Code\Code.exe"), AppKind::Editor);
        assert_eq!(kind_of("Discord.exe"), AppKind::Chat);
        assert_eq!(kind_of("Spotify.exe"), AppKind::Media);
        assert_eq!(kind_of("blender.exe"), AppKind::Other);
    }

    #[test]
    fn starters_are_linked_named_and_drawable() {
        for (exe, name) in [("chrome.exe", "Chrome"), ("Code.exe", "VS Code"), ("Discord.exe", "Discord"), ("Spotify.exe", "Spotify"), ("blender.exe", "Blender"), ("Teams.exe", "Teams")] {
            let p = for_app(exe, name, "", None, None);
            assert_eq!(p.name, name);
            assert!(p.apps.iter().any(|a| a.eq_ignore_ascii_case(exe)), "{exe}");
            // a starter fills the turning controls; a preset brings its own
            assert!(preset_for(exe).is_some() || (p.controls.contains_key("dial") && p.controls.contains_key("knobs.0")), "{exe}");
            for (id, b) in &p.controls {
                if let Some(icon) = &b.look.icon {
                    assert!(render::icon_svg(icon).is_some(), "{exe} {id}: no icon {icon}");
                }
            }
        }
        // the app's own key opens it and shows its picture when there is one
        let p = starter(AppKind::Other, "blender.exe", "Blender", r"C:\Blender\blender.exe", Some("abc.png".into()));
        assert_eq!(p.controls["keys.0"].action, "core.launch");
        assert_eq!(p.controls["keys.0"].look.image.as_deref(), Some("abc.png"));
        // a chat app gets its mute, then free keys, then the media row from key 10
        let d = starter(AppKind::Chat, "Discord.exe", "Discord", "", None); // the starter itself (the meetings preset is used for Discord)
        assert_eq!(d.controls["keys.1"].params["keys"], "Ctrl+Shift+M");
        assert!(!d.controls.contains_key("keys.6"));
        assert_eq!(d.controls["keys.11"].params["command"], "playPause");
    }

    #[test]
    fn games_are_found_and_get_game_keys() {
        assert!(is_game("x.exe", "steam://rungameid/570"));
        assert!(is_game(r"D:\SteamLibrary\steamapps\common\Hades\Hades.exe", ""));
        assert!(!is_game("notepad.exe", r"C:\Windows\notepad.exe"));
        assert_eq!(reg_values("\r\nHKEY_CURRENT_USER\\System\\GameConfigStore\\Children\\abc\r\n    MatchedExeFullPath    REG_SZ    G:\\Games\\D2R.exe\r\n"), vec![r"G:\Games\D2R.exe"]);
        let p = starter(AppKind::Game, "D2R.exe", "Diablo II Resurrected", "battlenet://", Some("cover.png".into()));
        assert_eq!(p.apps, vec!["D2R.exe"]);
        assert_eq!(p.controls["keys.0"].look.image.as_deref(), Some("cover.png"));
        assert_eq!(p.controls["keys.1"].params["keys"], "Escape");
        assert_eq!(p.controls["keys.1"].params["mode"], "game", "game keys go as scan codes");
        assert_eq!(p.controls["dial"].params["app"], "D2R.exe", "the dial sets the game's volume");
        for (id, b) in &p.controls {
            if let Some(icon) = &b.look.icon {
                assert!(render::icon_svg(icon).is_some(), "{id}: no icon {icon}");
            }
            if b.action == "core.hotkey" {
                assert!(crate::engine::hotkey::parse(b.params["keys"].as_str().unwrap()).is_some(), "{id}");
            }
        }
    }

    #[test]
    fn an_app_with_a_preset_starts_from_it() {
        let p = for_app("obs64.exe", "OBS", "", None, None);
        assert_eq!(p.name, "OBS");
        assert_eq!(p.controls["keys.5"].action, "obs.record");
        let adapted = for_app("obs64.exe", "OBS", "", None, Some(&["keys".to_string(), "dial".to_string()]));
        assert!(adapted.controls.contains_key("dial"));
    }
}
