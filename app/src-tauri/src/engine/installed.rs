//! What is installed on this PC, for the "Open app" picker, "Open game" and the games presets:
//! apps from the Start menu (shortcuts of the user and of all users) and Microsoft Store apps (on
//! macOS the Applications folders), and
//! games from Steam, Epic and GOG Galaxy with their launch links. Scanned on demand on a blocking
//! thread and cached in `<app data>/installed.json` (Refresh scans again). Game covers are read
//! from the launcher's own local cache on this PC: never bundled, never put into shared profiles.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub name: String,
    /// What to open: a program path, a `shell:AppsFolder\...` id or a launcher link
    /// (`steam://rungameid/...`).
    pub target: String,
    /// The program whose icon to show (empty when unknown).
    #[serde(default)]
    pub exe: String,
    /// `startMenu`, `store`, `steam`, `epic`, `gog`.
    pub source: String,
    /// A cover picture in the launcher's cache on this PC (games).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover: Option<String>,
    /// A PNG logo of a Microsoft Store app (they have no .exe icon).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub logo: String,
    /// Small icon as a data URI (filled when listing, not stored).
    #[serde(default, skip_deserializing, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// The game's install folder (Steam, Epic): the program that runs is often not `exe` (a
    /// launcher starts it), so a running game is matched by the folder it runs from.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub dir: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Installed {
    pub apps: Vec<Item>,
    pub games: Vec<Item>,
    /// [`CACHE_VERSION`] when written; an older cache is scanned again.
    #[serde(default)]
    pub version: u32,
}

/// 2: games carry their install folder (`Item::dir`). 3: macOS lists its Applications folders.
const CACHE_VERSION: u32 = 3;

static FILE: OnceLock<PathBuf> = OnceLock::new();

fn cached() -> Option<Installed> {
    FILE.get().and_then(|f| std::fs::read(f).ok()).and_then(|b| serde_json::from_slice::<Installed>(&b).ok()).filter(|c| c.version >= CACHE_VERSION)
}

pub fn init(data_dir: &Path) {
    let _ = FILE.set(data_dir.join("installed.json"));
}

/// The cached list with each item's icon, for the window's lists. Blocking: call it from a
/// blocking thread. Reading icons is slow and, on a Mac, heavy (every size up to 1024 px), so
/// nothing else asks for them (2026-10-14: icons of every app at start took gigabytes).
pub fn list_with_icons(refresh: bool) -> Installed {
    let mut found = list(refresh);
    for item in found.apps.iter_mut().chain(found.games.iter_mut()) {
        item.icon = super::apps::icon(&item.exe).or_else(|| png_icon(&item.logo));
    }
    found
}

/// The cached list, or a fresh scan when there is none or `refresh` is set; no icons. Blocking:
/// call it from a blocking thread.
pub fn list(refresh: bool) -> Installed {
    (!refresh).then(cached).flatten().unwrap_or_else(|| {
        let fresh = scan();
        if let Some(f) = FILE.get()
            && let Ok(json) = serde_json::to_vec_pretty(&fresh)
            && let Err(e) = super::profile::write_atomic(f, &json)
        {
            log::warn!("installed.json: {e}");
        }
        fresh
    })
}

/// Just the games: from the cache when there is one, else a quick scan of the launchers (no apps).
pub fn games() -> Vec<Item> {
    cached().map(|c| c.games).unwrap_or_else(|| {
        let mut games = steam_games();
        games.extend(epic_games());
        games.extend(gog_games());
        games.sort_by_key(|g| g.name.to_lowercase());
        games
    })
}

/// The installed app or game called `name` (its name, "Visual Studio Code", or its program,
/// "Code.exe" / "code"); else one whose name holds it as a word ("Terminal" -> "Windows Terminal").
/// Keys the assistant sketched name apps and games so (session 6).
pub fn item_named(name: &str) -> Option<Item> {
    let n = name.trim().to_lowercase();
    if n.is_empty() {
        return None;
    }
    let stem = |p: &str| p.rsplit(['\\', '/']).next().unwrap_or(p).to_lowercase().trim_end_matches(".exe").to_string();
    let items = cached_items();
    let exact = items.iter().find(|i| i.name.to_lowercase() == n || (!i.exe.is_empty() && stem(&i.exe) == n.trim_end_matches(".exe")));
    exact.or_else(|| items.iter().find(|i| i.name.to_lowercase().split(|c: char| !c.is_alphanumeric()).any(|w| w == n))).cloned()
}

/// What to open for an installed app or game called `name`.
pub fn target_named(name: &str) -> Option<String> {
    item_named(name).map(|i| i.target)
}

/// Installed apps and games from the cache (no icons filled, so it is quick); empty before a scan.
pub fn cached_items() -> Vec<Item> {
    cached().map(|c| c.apps.into_iter().chain(c.games).collect()).unwrap_or_default()
}

/// A small data URI of a picture (a Store logo, a game cover), like `apps::icon` gives for programs.
pub(crate) fn png_icon(path: &str) -> Option<String> {
    use base64::Engine as _;
    if path.is_empty() {
        return None;
    }
    let img = image::open(path).ok()?.resize(32, 32, image::imageops::FilterType::Triangle);
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(out.into_inner())))
}

/// The best file of a Store app's logo: `Assets\X.png` exists as `X.targetsize-48.png`,
/// `X.scale-200.png`... (high-contrast and light-theme variants are skipped).
#[cfg_attr(not(windows), allow(dead_code))]
pub fn store_logo(dir: &Path, logo: &str) -> Option<String> {
    let logo = Path::new(logo);
    let folder = dir.join(logo.parent().unwrap_or(Path::new("")));
    let stem = logo.file_stem()?.to_string_lossy().to_lowercase();
    let exact = dir.join(logo);
    let score = |name: &str| -> Option<u32> {
        let n = name.to_lowercase();
        let rest = n.strip_prefix(&format!("{stem}."))?.strip_suffix(".png")?;
        if rest.contains("contrast") || rest.contains("light") {
            return None;
        }
        let num = |key: &str| rest.split(key).nth(1).and_then(|t| t.split(|c: char| !c.is_ascii_digit()).next()).and_then(|d| d.parse::<u32>().ok());
        Some(match (num("targetsize-"), num("scale-")) {
            (Some(t), _) => t.min(256) * 2 + u32::from(rest.contains("unplated")),
            (None, Some(sc)) => sc * 44 / 100,
            _ => 1,
        })
    };
    let best = std::fs::read_dir(&folder).ok()?.flatten().filter_map(|e| Some((score(&e.file_name().to_string_lossy())?, e.path()))).max_by_key(|(sc, _)| *sc).map(|(_, p)| p);
    best.or_else(|| exact.is_file().then_some(exact)).map(|p| p.to_string_lossy().to_string())
}

/// The key picture for an item: its local cover (marked so shared profiles leave it out), else
/// the program's icon.
pub fn picture(item: &Item) -> Result<String, String> {
    // the same app's picture is made once per run: the tour rebuilds its deck at every click and
    // reading an app's icon from its bundle is slow (owner: the looks switched "mega wolno")
    static MADE: std::sync::Mutex<Option<std::collections::HashMap<String, String>>> = std::sync::Mutex::new(None);
    let key = format!("{}|{}|{}|{}", item.target, item.exe, item.logo, item.cover.as_deref().unwrap_or_default());
    if let Some(name) = MADE.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|m| m.get(&key)).cloned() {
        return Ok(name);
    }
    let made = picture_uncached(item)?;
    MADE.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(Default::default).insert(key, made.clone());
    Ok(made)
}

fn picture_uncached(item: &Item) -> Result<String, String> {
    if let Some(cover) = item.cover.as_deref().filter(|c| Path::new(c).is_file()) {
        let name = super::images::import(Path::new(cover))?;
        super::images::mark_local_only(&name);
        return Ok(name);
    }
    if !item.logo.is_empty() && Path::new(&item.logo).is_file() {
        return super::images::import(Path::new(&item.logo));
    }
    if item.exe.is_empty() {
        return Err("no picture for this one".into());
    }
    super::appicon::png(&item.exe).and_then(|png| super::images::import_bytes(&png, "png"))
}

fn scan() -> Installed {
    let mut apps = platform::start_menu_apps();
    apps.extend(platform::store_apps());
    apps.sort_by_key(|a| a.name.to_lowercase());
    apps.dedup_by(|a, b| a.name.eq_ignore_ascii_case(&b.name));
    let mut games = steam_games();
    games.extend(epic_games());
    games.extend(gog_games());
    games.sort_by_key(|g| g.name.to_lowercase());
    log::info!("installed: {} apps, {} games", apps.len(), games.len());
    Installed { apps, games, version: CACHE_VERSION }
}

fn env_path(var: &str) -> Option<PathBuf> {
    std::env::var_os(var).map(PathBuf::from)
}

// ---- Steam ----

/// `"key"  "value"` pairs of a Valve KeyValues text file (nesting ignored: enough for the keys we read).
pub fn vdf_values(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split('"').collect();
            // `\t"key"\t\t"value"` splits into ["\t", key, "\t\t", value, ""]
            (parts.len() >= 5).then(|| (parts[1].to_string(), parts[3].replace("\\\\", "\\")))
        })
        .collect()
}

/// Steam's own tools that are not games.
fn steam_tool(name: &str, appid: &str) -> bool {
    let n = name.to_lowercase();
    appid == "228980" || ["steamworks", "redistributable", "proton", "steam linux runtime", "dedicated server", "sdk"].iter().any(|w| n.contains(w))
}

fn steam_root() -> Option<PathBuf> {
    [env_path("ProgramFiles(x86)").map(|p| p.join("Steam")), env_path("ProgramFiles").map(|p| p.join("Steam")), env_path("HOME").map(|h| h.join("Library/Application Support/Steam"))]
        .into_iter()
        .flatten()
        .find(|p| p.join("steamapps").is_dir())
}

/// A cover from Steam's local library cache (newer and older layouts).
fn steam_cover(root: &Path, appid: &str) -> Option<String> {
    let cache = root.join("appcache").join("librarycache");
    let dir = cache.join(appid);
    // newer Steam keeps the pictures in hashed subfolders of the game's folder
    let mut folders = vec![dir.clone()];
    folders.extend(std::fs::read_dir(&dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir()));
    let names = ["library_600x900.jpg", "library_capsule.jpg", "header.jpg", "library_header.jpg"];
    names
        .iter()
        .flat_map(|n| folders.iter().map(move |f| f.join(n)))
        .chain([cache.join(format!("{appid}_library_600x900.jpg")), cache.join(format!("{appid}_header.jpg"))])
        .find(|p| p.is_file())
        .map(|p| p.to_string_lossy().to_string())
}

fn steam_games() -> Vec<Item> {
    let Some(root) = steam_root() else { return Vec::new() };
    let folders = std::fs::read_to_string(root.join("steamapps").join("libraryfolders.vdf")).unwrap_or_default();
    let mut libraries: Vec<PathBuf> = vdf_values(&folders).into_iter().filter(|(k, _)| k == "path").map(|(_, v)| PathBuf::from(v)).collect();
    if libraries.is_empty() {
        libraries.push(root.clone());
    }
    let mut out = Vec::new();
    for lib in libraries {
        let Ok(entries) = std::fs::read_dir(lib.join("steamapps")) else { continue };
        for e in entries.flatten() {
            let file = e.file_name().to_string_lossy().to_string();
            if !(file.starts_with("appmanifest_") && file.ends_with(".acf")) {
                continue;
            }
            let values = vdf_values(&std::fs::read_to_string(e.path()).unwrap_or_default());
            let get = |k: &str| values.iter().find(|(key, _)| key.eq_ignore_ascii_case(k)).map(|(_, v)| v.clone()).unwrap_or_default();
            let (appid, name) = (get("appid"), get("name"));
            if appid.is_empty() || name.is_empty() || steam_tool(&name, &appid) {
                continue;
            }
            let dir = match get("installdir") {
                d if d.is_empty() => String::new(),
                d => lib.join("steamapps").join("common").join(d).to_string_lossy().to_string(),
            };
            out.push(Item { name, target: format!("steam://rungameid/{appid}"), exe: String::new(), source: "steam".into(), cover: steam_cover(&root, &appid), logo: String::new(), icon: None, dir });
        }
    }
    out
}

// ---- Epic ----

fn epic_games() -> Vec<Item> {
    let Some(dir) = env_path("ProgramData").map(|p| p.join(r"Epic\EpicGamesLauncher\Data\Manifests")) else { return Vec::new() };
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "item"))
        .filter_map(|e| serde_json::from_slice::<serde_json::Value>(&std::fs::read(e.path()).ok()?).ok())
        .filter(|m| m["bIsApplication"] != false && !m["AppCategories"].as_array().is_some_and(|c| c.iter().any(|x| x == "plugins" || x == "engines")))
        .filter_map(|m| {
            let s = |k: &str| m[k].as_str().unwrap_or("").to_string();
            let (ns, item, app) = (s("CatalogNamespace"), s("CatalogItemId"), s("AppName"));
            (!s("DisplayName").is_empty() && !app.is_empty()).then(|| Item {
                name: s("DisplayName"),
                target: format!("com.epicgames.launcher://apps/{ns}%3A{item}%3A{app}?action=launch&silent=true"),
                exe: Path::new(&s("InstallLocation")).join(s("LaunchExecutable")).to_string_lossy().to_string(),
                source: "epic".into(),
                cover: None,
                logo: String::new(),
                icon: None,
                dir: s("InstallLocation"),
            })
        })
        .collect()
}

// ---- GOG Galaxy ----

fn gog_games() -> Vec<Item> {
    let Some(dir) = env_path("ProgramFiles(x86)").map(|p| p.join(r"GOG Galaxy\Games")) else { return Vec::new() };
    let Ok(games) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out = Vec::new();
    for g in games.flatten() {
        let Ok(files) = std::fs::read_dir(g.path()) else { continue };
        for f in files.flatten() {
            let name = f.file_name().to_string_lossy().to_string();
            if !(name.starts_with("goggame-") && name.ends_with(".info")) {
                continue;
            }
            let Some(info) = std::fs::read(f.path()).ok().and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok()) else { continue };
            let task = info["playTasks"].as_array().and_then(|t| t.iter().find(|x| x["isPrimary"] == true).or(t.first())).cloned().unwrap_or_default();
            let exe = g.path().join(task["path"].as_str().unwrap_or("")).to_string_lossy().to_string();
            if let Some(title) = info["name"].as_str()
                && Path::new(&exe).is_file()
            {
                out.push(Item { name: title.into(), target: exe.clone(), exe, source: "gog".into(), cover: None, logo: String::new(), icon: None, dir: String::new() });
            }
        }
    }
    out
}

#[cfg(windows)]
mod platform {
    use std::path::Path;

    use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, IPersistFile, STGM_READ};
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
    use windows::core::{HSTRING, Interface};

    use super::Item;

    /// Program a `.lnk` points at.
    fn link_target(link: &Path) -> Option<String> {
        // SAFETY: COM on this thread; the buffer outlives GetPath.
        unsafe {
            let shell: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
            let file: IPersistFile = shell.cast().ok()?;
            file.Load(&HSTRING::from(link.as_os_str()), STGM_READ).ok()?;
            let mut buf = [0u16; 520];
            shell.GetPath(&mut buf, std::ptr::null_mut(), 0).ok()?;
            let n = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
            Some(String::from_utf16_lossy(&buf[..n]))
        }
    }

    fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>, depth: u8) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() && depth < 4 {
                walk(&p, out, depth + 1);
            } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("lnk")) {
                out.push(p);
            }
        }
    }

    /// Programs in the Start menu (this user's and everyone's), not uninstallers or help files.
    pub fn start_menu_apps() -> Vec<Item> {
        // SAFETY: COM for the shell link objects on this (blocking) thread.
        let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        let mut links = Vec::new();
        for root in [
            std::env::var_os("ProgramData").map(|p| Path::new(&p).join(r"Microsoft\Windows\Start Menu\Programs")),
            std::env::var_os("APPDATA").map(|p| Path::new(&p).join(r"Microsoft\Windows\Start Menu\Programs")),
        ]
        .into_iter()
        .flatten()
        {
            walk(&root, &mut links, 0);
        }
        links
            .into_iter()
            .filter_map(|link| {
                let name = link.file_stem()?.to_string_lossy().to_string();
                let target = link_target(&link)?;
                let lower = format!("{} {}", name.to_lowercase(), target.to_lowercase());
                // tools nobody puts on a key: uninstallers, readmes, "About Java", system tools
                let skip = [
                    "uninstall",
                    "unins0",
                    "readme",
                    "help",
                    "documentation",
                    "website",
                    "release notes",
                    "license",
                    "about ",
                    "check for updates",
                    "administrative tools",
                    "character map",
                    "odbc",
                    "recovery drive",
                    "debug",
                    "crash report",
                ]
                .iter()
                .any(|w| lower.contains(w));
                (target.to_lowercase().ends_with(".exe") && Path::new(&target).is_file() && !skip).then(|| Item {
                    name,
                    target: target.clone(),
                    exe: target,
                    source: "startMenu".into(),
                    cover: None,
                    logo: String::new(),
                    icon: None,
                    dir: String::new(),
                })
            })
            .collect()
    }

    /// Microsoft Store apps (they have no .exe to start): `Get-StartApps`, ids with a `!`.
    pub fn store_apps() -> Vec<Item> {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let out = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "[Console]::OutputEncoding=[Text.Encoding]::UTF8; $p=@{}; Get-AppxPackage | % { $p[$_.PackageFamilyName]=$_.InstallLocation }; \
                 Get-StartApps | ? { $_.AppID -like '*!*' -and $p[$_.AppID.Split('!')[0]] } | \
                 % { [pscustomobject]@{Name=$_.Name; AppID=$_.AppID; Dir=$p[$_.AppID.Split('!')[0]]} } | ConvertTo-Json -Compress",
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        let Ok(out) = out else { return Vec::new() };
        let list: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap_or_default();
        list.into_iter()
            .filter_map(|a| {
                let id = a["AppID"].as_str()?.to_string();
                let name = a["Name"].as_str()?.to_string();
                let dir = std::path::PathBuf::from(a["Dir"].as_str()?);
                let manifest = std::fs::read_to_string(dir.join("AppxManifest.xml")).unwrap_or_default();
                let logo = manifest
                    .split("Square44x44Logo=\"")
                    .nth(1)
                    .and_then(|t| t.split('"').next())
                    .or_else(|| manifest.split("<Logo>").nth(1).and_then(|t| t.split('<').next()))
                    .and_then(|l| super::store_logo(&dir, l))
                    .unwrap_or_default();
                Some(Item { name, target: format!(r"shell:AppsFolder\{id}"), exe: String::new(), source: "store".into(), cover: None, logo, icon: None, dir: String::new() })
            })
            .collect()
    }
}

/// macOS: the apps in the Applications folders (the system's, the user's, one folder deep for
/// suites like Adobe's), named as Finder names them; `target` and `exe` are the `.app` bundle.
#[cfg(target_os = "macos")]
mod platform {
    use std::path::{Path, PathBuf};

    use super::Item;

    fn add(dir: &Path, depth: u8, out: &mut Vec<Item>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for e in entries.flatten() {
            let p = e.path();
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
            if name.starts_with('.') {
                continue;
            }
            match name.strip_suffix(".app") {
                Some(stem) => {
                    let path = p.to_string_lossy().into_owned();
                    out.push(Item { name: stem.to_string(), target: path.clone(), exe: path, source: "applications".into(), cover: None, logo: String::new(), icon: None, dir: String::new() });
                }
                None if p.is_dir() && depth > 0 => add(&p, depth - 1, out),
                None => {}
            }
        }
    }

    pub fn start_menu_apps() -> Vec<Item> {
        let mut dirs: Vec<PathBuf> = ["/Applications", "/System/Applications", "/System/Applications/Utilities", "/Applications/Utilities"].map(PathBuf::from).to_vec();
        if let Some(home) = std::env::var_os("HOME") {
            dirs.push(PathBuf::from(home).join("Applications"));
        }
        let mut out = Vec::new();
        for d in dirs {
            add(&d, 1, &mut out);
        }
        out
    }

    pub fn store_apps() -> Vec<Item> {
        Vec::new()
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod platform {
    pub fn start_menu_apps() -> Vec<super::Item> {
        Vec::new()
    }
    pub fn store_apps() -> Vec<super::Item> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steam_files_are_read() {
        let folders = "\"libraryfolders\"\n{\n\t\"0\"\n\t{\n\t\t\"path\"\t\t\"C:\\\\Program Files (x86)\\\\Steam\"\n\t\t\"label\"\t\t\"\"\n\t}\n}\n";
        let v = vdf_values(folders);
        assert!(v.contains(&("path".into(), r"C:\Program Files (x86)\Steam".into())));
        let manifest = "\"AppState\"\n{\n\t\"appid\"\t\t\"1016920\"\n\t\"name\"\t\t\"Unrailed!\"\n}\n";
        let v = vdf_values(manifest);
        assert_eq!(v, [("appid".to_string(), "1016920".to_string()), ("name".to_string(), "Unrailed!".to_string())]);
        assert!(steam_tool("Steamworks Common Redistributables", "228980"));
        assert!(steam_tool("Proton Experimental", "1493710"));
        assert!(!steam_tool("Unrailed!", "1016920"));
    }

    // Microsoft Store apps: Windows only (their manifests use `\` paths)
    #[cfg(windows)]
    #[test]
    fn store_logo_picks_the_best_variant() {
        let dir = std::env::temp_dir().join(format!("hubdeck-logo-{}", std::process::id()));
        let assets = dir.join("Assets");
        std::fs::create_dir_all(&assets).unwrap();
        for f in ["AppList.scale-100.png", "AppList.targetsize-48.png", "AppList.targetsize-48_altform-unplated.png", "AppList.targetsize-256_contrast-black.png", "Other.png"] {
            std::fs::write(assets.join(f), b"png").unwrap();
        }
        let best = store_logo(&dir, r"Assets\AppList.png").unwrap();
        assert!(best.ends_with("AppList.targetsize-48_altform-unplated.png"), "{best}");
        assert_eq!(store_logo(&dir, r"Assets\Missing.png"), None);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// On a PC: `cargo test -p hubdeck installed_live -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn installed_live() {
        let found = scan();
        println!("{} apps, {} games", found.apps.len(), found.games.len());
        let store: Vec<&Item> = found.apps.iter().filter(|a| a.source == "store").collect();
        println!("store apps {}, with a logo {}", store.len(), store.iter().filter(|a| !a.logo.is_empty()).count());
        for g in found.games.iter().take(5) {
            println!("game {} -> {} cover {:?}", g.name, g.target, g.cover.is_some());
        }
        for a in found.apps.iter().take(5) {
            println!("app {} -> {}", a.name, a.target);
        }
    }
}
