//! Profiles that make themselves, so nobody has to click a profile together for every app (owner,
//! 2026-10-12): at start for the installed apps HubDeck has a preset for, and for a game the first
//! time it comes to the front (it then switches in at once). Each program gets one only once: a
//! profile the user deleted is not made again (`settings.offered`). The window lists the new ones
//! until the user has looked (`settings.auto_made`). Off in Settings with `auto_profiles`.

use std::collections::HashSet;
use std::sync::Mutex;

use tauri::{AppHandle, Manager};

use super::Engine;
use super::installed::{self, Item};
use super::profile::Profile;
use super::starter;

/// Programs looked at in this run that need nothing (no preset, not a game).
static SKIPPED: Mutex<Option<HashSet<String>>> = Mutex::new(None);

/// Folder names that say nothing about the program inside them.
const GENERIC_DIRS: [&str; 14] = ["bin", "bin64", "x64", "x86", "win64", "win32", "binaries", "retail", "client", "game", "launcher", "build", "release", "_retail_"];

/// A readable name for a program from its path: the folder a launcher installed it in, else the
/// nearest folder that is not a generic one, else the file name.
fn name_from_path(path: &str) -> Option<String> {
    let parts: Vec<&str> = path.split(['\\', '/']).filter(|p| !p.is_empty()).collect();
    let lower: Vec<String> = parts.iter().map(|p| p.to_lowercase()).collect();
    for (i, w) in lower.windows(2).enumerate() {
        if matches!((w[0].as_str(), w[1].as_str()), ("steamapps", "common") | ("gog galaxy", "games")) {
            return parts.get(i + 2).map(|s| s.to_string());
        }
    }
    if let Some(i) = lower.iter().position(|p| p == "epic games" || p == "gog games" || p == "riot games" || p == "xboxgames") {
        return parts.get(i + 1).map(|s| s.to_string());
    }
    parts.iter().rev().skip(1).find(|p| !GENERIC_DIRS.contains(&p.to_lowercase().as_str()) && p.len() > 2).map(|s| s.to_string())
}

/// What to make for a program: its name, how to open it and the picture for its key.
struct Found {
    exe: String,
    name: String,
    target: String,
    picture: Option<Item>,
}

/// Whether `exe` should get a profile now: it has none, it never got one, and it has a preset or
/// is a game. `title`: its window title (a game's name when nothing better is known).
fn consider(engine: &Engine, exe: &str, title: &str, games_too: bool) -> Option<Found> {
    let key = exe.to_lowercase();
    {
        let s = engine.lock();
        let settings = &s.store.settings;
        if !settings.auto_profiles || settings.offered.contains(&key) || s.store.profiles.values().any(|p| p.app_match(exe, "").is_some()) {
            return None;
        }
    }
    if SKIPPED.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashSet::new).contains(&key) {
        return None;
    }
    let path = super::apps::path_of_seen(exe).unwrap_or_default();
    let found = if let Some(preset) = starter::preset_for(exe) {
        Some(Found { exe: exe.into(), name: preset.name, target: path.clone(), picture: None })
    } else if games_too && starter::is_game(if path.is_empty() { exe } else { &path }, "") {
        let library = installed::games().into_iter().find(|g| same_game(g, exe, &path));
        let title = title.trim();
        let name = library
            .as_ref()
            .map(|g| g.name.clone())
            .or_else(|| name_from_path(&path))
            .or_else(|| (!title.is_empty() && title.chars().count() <= 40).then(|| title.to_string()))
            .unwrap_or_else(|| exe.trim_end_matches(".exe").to_string());
        let target = library.as_ref().map(|g| g.target.clone()).unwrap_or_else(|| path.clone());
        let picture = library.or_else(|| {
            (!path.is_empty()).then(|| Item {
                name: name.clone(),
                target: path.clone(),
                exe: path.clone(),
                source: "startMenu".into(),
                cover: None,
                logo: String::new(),
                icon: None,
                dir: String::new(),
            })
        });
        Some(Found { exe: exe.into(), name, target, picture })
    } else {
        None
    };
    if found.is_none() {
        SKIPPED.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashSet::new).insert(key);
    }
    found
}

/// The library game this running program belongs to: its own program, or one run from inside its
/// install folder (Steam games have no program in the library, launchers start another one).
fn same_game(game: &Item, exe: &str, path: &str) -> bool {
    if !game.exe.is_empty() && game.exe.rsplit(['\\', '/']).next().is_some_and(|n| n.eq_ignore_ascii_case(exe)) {
        return true;
    }
    let norm = |p: &str| p.replace('\\', "/").trim_end_matches('/').to_lowercase();
    let (dir, path) = (norm(&game.dir), norm(path));
    !dir.is_empty() && path.starts_with(&format!("{dir}/"))
}

/// Make and save the profile; returns it.
fn make(app: &AppHandle, engine: &Engine, found: Found) -> Option<Profile> {
    let picture = found.picture.as_ref().and_then(|item| installed::picture(item).ok());
    let profile = starter::for_app(&found.exe, &found.name, &found.target, picture, None);
    let (profile, _) = super::api::adapt_to_user(app, profile);
    let mut s = engine.lock();
    let key = found.exe.to_lowercase();
    if s.store.settings.offered.contains(&key) {
        return None; // made meanwhile by another call
    }
    if let Err(e) = s.store.save_profile(profile.clone()) {
        log::warn!("auto profile for {}: {e}", found.exe);
        return None;
    }
    s.store.settings.offered.push(key);
    s.store.settings.auto_made.push(profile.id.clone());
    if let Err(e) = s.store.save_settings() {
        log::warn!("settings: {e}");
    }
    log::info!("made a profile by itself: \"{}\" for {}", profile.name, found.exe);
    Some(profile)
}

/// An app came to the front and no profile is linked to it: make one when it is a game or has a
/// preset, then switch to it. Runs on its own thread (it may scan the registry and draw a picture).
pub fn on_foreground(app: &AppHandle, exe: &str, title: &str) {
    if exe.is_empty() || exe.to_lowercase().starts_with("hubdeck") {
        return;
    }
    let (app, exe, title) = (app.clone(), exe.to_string(), title.to_string());
    std::thread::spawn(move || {
        let engine = app.state::<Engine>();
        if let Some(found) = consider(&engine, &exe, &title, true)
            && make(&app, &engine, found).is_some()
        {
            engine.changed();
            engine.refresh_foreground();
        }
    });
}

/// At start: profiles for the installed apps that have a preset (games wait until they are played).
pub fn at_start(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        // let the deck and the window come up first
        std::thread::sleep(std::time::Duration::from_secs(8));
        let engine = app.state::<Engine>();
        if !engine.lock().store.settings.auto_profiles {
            return;
        }
        let mut made = 0;
        for item in installed::list(false).apps {
            // the program: `Discord.exe` on Windows, `Discord.app` on a Mac (the preset matches both)
            let Some(exe) = item.exe.rsplit(['\\', '/']).find(|e| !e.is_empty()).filter(|e| e.to_lowercase().ends_with(".exe") || e.to_lowercase().ends_with(".app")).map(str::to_string) else {
                continue;
            };
            if let Some(mut found) = consider(&engine, &exe, "", false) {
                found.target = item.target.clone();
                found.picture = Some(item.clone());
                made += usize::from(make(&app, &engine, found).is_some());
            }
        }
        if made > 0 {
            engine.changed();
        }
    });
}

/// The user looked at the profiles that made themselves.
#[tauri::command]
pub fn auto_seen(engine: tauri::State<'_, Engine>) -> Result<(), String> {
    {
        let mut s = engine.lock();
        s.store.settings.auto_made.clear();
        s.store.save_settings().map_err(|e| e.to_string())?;
    }
    engine.changed();
    Ok(())
}

/// Settings: profiles make themselves (on) or not.
#[tauri::command]
pub fn auto_set(engine: tauri::State<'_, Engine>, on: bool) -> Result<(), String> {
    {
        let mut s = engine.lock();
        s.store.settings.auto_profiles = on;
        s.store.save_settings().map_err(|e| e.to_string())?;
    }
    engine.changed();
    if on {
        engine.refresh_foreground();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_running_game_is_found_by_its_program_or_install_folder() {
        let steam = Item {
            name: "DREDGE".into(),
            target: "steam://rungameid/1562430".into(),
            exe: String::new(),
            source: "steam".into(),
            cover: None,
            logo: String::new(),
            icon: None,
            dir: r"D:\SteamLibrary\steamapps\common\DREDGE".into(),
        };
        assert!(same_game(&steam, "DREDGE.exe", r"d:\steamlibrary\steamapps\common\dredge\DREDGE.exe"));
        assert!(same_game(&steam, "DREDGE.exe", "D:/SteamLibrary/steamapps/common/DREDGE/bin/DREDGE.exe"));
        assert!(!same_game(&steam, "DREDGE.exe", r"D:\SteamLibrary\steamapps\common\DREDGE2\DREDGE.exe"), "a folder that only starts the same");
        assert!(!same_game(&steam, "DREDGE.exe", ""));
        let epic = Item { exe: r"G:\Fortnite\FortniteLauncher.exe".into(), dir: String::new(), ..steam.clone() };
        assert!(same_game(&epic, "fortnitelauncher.exe", ""));
    }

    #[test]
    fn games_are_named_after_their_folder() {
        assert_eq!(name_from_path(r"G:\Battlenet gry\Diablo II Resurrected\D2R.exe").as_deref(), Some("Diablo II Resurrected"));
        assert_eq!(name_from_path(r"C:\Program Files (x86)\Steam\steamapps\common\Dying Light 2\ph\work\bin\x64\DyingLightGame_x64_rwdi.exe").as_deref(), Some("Dying Light 2"));
        assert_eq!(name_from_path(r"D:\Epic Games\WorldWarZ\en_us\client\bin\pc\wwzRetailEgs.exe").as_deref(), Some("WorldWarZ"));
        assert_eq!(name_from_path(r"C:\Program Files (x86)\GOG Galaxy\Games\The Witcher 3 Wild Hunt\bin\x64\witcher3.exe").as_deref(), Some("The Witcher 3 Wild Hunt"));
        assert_eq!(name_from_path(r"C:\Games\Hades\x64\Hades.exe").as_deref(), Some("Hades"));
    }
}
