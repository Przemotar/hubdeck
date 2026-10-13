//! Apps the user can link to a profile: the ones with a window open now and the ones seen in
//! front recently (remembered in `<app data>/apps.json`: exe name, path, last window title), each
//! with its real icon, so linking a profile to an app is a pick from a list, not typing `exe` names.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use base64::Engine as _;
use serde::{Deserialize, Serialize};

/// Apps remembered from the foreground, newest first.
const MAX_RECENT: usize = 40;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    /// `Photoshop.exe`
    pub exe: String,
    /// Full path of the program, for its icon.
    pub path: String,
    /// The last window title seen.
    pub title: String,
    /// Seconds since 1970 when it was last in front (0 = never).
    #[serde(default)]
    pub last_seen: i64,
    /// Has a window open now.
    #[serde(default, skip_deserializing)]
    pub running: bool,
    /// The program's icon as a small PNG data URI.
    #[serde(default, skip_deserializing, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// The program's own name ("Google Chrome"), from its version info; None when it has none.
    #[serde(default, skip_deserializing, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

static FILE: OnceLock<PathBuf> = OnceLock::new();
static RECENT: Mutex<Option<Vec<AppInfo>>> = Mutex::new(None);
static ICONS: Mutex<Option<HashMap<String, Option<String>>>> = Mutex::new(None);
static NAMES: Mutex<Option<HashMap<String, Option<String>>>> = Mutex::new(None);
static BY_EXE: Mutex<Option<HashMap<String, Option<String>>>> = Mutex::new(None);

pub fn init(data_dir: &std::path::Path) {
    let _ = FILE.set(data_dir.join("apps.json"));
}

fn with_recent<T>(f: impl FnOnce(&mut Vec<AppInfo>) -> T) -> T {
    let mut guard = RECENT.lock().unwrap_or_else(|e| e.into_inner());
    let list = guard.get_or_insert_with(|| FILE.get().and_then(|p| std::fs::read(p).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default());
    f(list)
}

/// Programs that are part of Windows itself and never a useful profile target.
fn ignored(exe: &str) -> bool {
    let e = exe.to_lowercase();
    ["applicationframehost.exe", "textinputhost.exe", "shellexperiencehost.exe", "lockapp.exe", "searchhost.exe", "startmenuexperiencehost.exe", "searchapp.exe"].contains(&e.as_str())
}

/// The app in front changed: remember it. Written to disk only when a new app (or a new path)
/// appears, so switching windows does not write a file every time.
pub fn seen(exe: &str, path: &str, title: &str) {
    if exe.is_empty() || ignored(exe) {
        return;
    }
    let now = chrono::Utc::now().timestamp();
    let changed = with_recent(|list| {
        let pos = list.iter().position(|a| a.exe.eq_ignore_ascii_case(exe));
        let new = pos.is_none_or(|i| list[i].path != path);
        if let Some(i) = pos {
            list.remove(i);
        }
        list.insert(0, AppInfo { exe: exe.into(), path: path.into(), title: title.into(), last_seen: now, running: false, icon: None, name: None });
        list.truncate(MAX_RECENT);
        new.then(|| list.clone())
    });
    if let (Some(list), Some(file)) = (changed, FILE.get())
        && let Ok(json) = serde_json::to_vec_pretty(&list)
        && let Err(e) = super::profile::write_atomic(file, &json)
    {
        log::warn!("apps.json: {e}");
    }
}

/// The full path of a program seen in front recently (`exe` is its file name).
pub fn path_of_seen(exe: &str) -> Option<String> {
    with_recent(|l| l.iter().find(|a| a.exe.eq_ignore_ascii_case(exe)).map(|a| a.path.clone())).filter(|p| !p.is_empty())
}

/// The program of an app seen in front (or open now) called `name`: its own name, its window
/// title ("Habit Tracker v2") or its program; for keys that name an app the user's own tools.
pub fn seen_named(name: &str) -> Option<String> {
    let n = name.trim().to_lowercase();
    if n.is_empty() {
        return None;
    }
    let stem = |e: &str| e.to_lowercase().trim_end_matches(".exe").to_string();
    list()
        .into_iter()
        .filter(|a| !a.path.is_empty())
        .find(|a| a.name.as_deref().is_some_and(|x| x.to_lowercase() == n) || a.title.to_lowercase() == n || stem(&a.exe) == n.trim_end_matches(".exe"))
        .map(|a| a.path)
}

/// Apps open now first (by name), then the ones seen recently; one entry per program, with icons.
pub fn list() -> Vec<AppInfo> {
    let mut out: Vec<AppInfo> = platform::running();
    out.sort_by_key(|a| a.exe.to_lowercase());
    let recent = with_recent(|l| l.clone());
    for mut r in recent {
        if let Some(open) = out.iter_mut().find(|a| a.exe.eq_ignore_ascii_case(&r.exe)) {
            open.last_seen = r.last_seen;
            continue;
        }
        r.running = false;
        out.push(r);
    }
    out.dedup_by(|a, b| a.exe.eq_ignore_ascii_case(&b.exe));
    for a in out.iter_mut() {
        a.icon = icon(&a.path);
        a.name = friendly_name(&a.path);
    }
    out
}

/// The icon of a program a profile is linked to (`PathOfExile.exe`), also when it was never in
/// front since HubDeck runs: from where it was seen, else the installed program or game of that
/// file name (or one in a game's install folder), else the game's cover. Cached, None included.
pub fn icon_for_exe(exe: &str) -> Option<String> {
    let key = exe.trim().to_lowercase();
    if key.is_empty() {
        return None;
    }
    if let Some(hit) = BY_EXE.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).get(&key) {
        return hit.clone();
    }
    let file = |p: &str| p.rsplit(['\\', '/']).next().unwrap_or(p).to_lowercase();
    let found = path_of_seen(exe).and_then(|p| icon(&p)).or_else(|| {
        let items = super::installed::cached_items();
        items.iter().find(|i| !i.exe.is_empty() && file(&i.exe) == key).and_then(|i| icon(&i.exe)).or_else(|| {
            items.iter().filter(|i| !i.dir.is_empty()).find_map(|i| {
                let candidate = std::path::Path::new(&i.dir).join(exe.trim());
                candidate.is_file().then(|| icon(&candidate.to_string_lossy())).flatten().or_else(|| {
                    // a game whose folder holds no such file but is named after it: its cover
                    let stem = key.trim_end_matches(".exe").replace([' ', '_', '-'], "");
                    let name = i.name.to_lowercase().replace([' ', '_', '-', ':'], "");
                    (!stem.is_empty() && (name.starts_with(&stem) || stem.starts_with(&name))).then(|| i.cover.as_deref().and_then(super::installed::png_icon)).flatten()
                })
            })
        })
    });
    BY_EXE.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).insert(key, found.clone());
    found
}

/// The name a program gives itself (FileDescription, else ProductName), cached by path.
pub fn friendly_name(path: &str) -> Option<String> {
    if path.is_empty() {
        return None;
    }
    let mut guard = NAMES.lock().unwrap_or_else(|e| e.into_inner());
    let cache = guard.get_or_insert_with(HashMap::new);
    if let Some(hit) = cache.get(path) {
        return hit.clone();
    }
    let name = platform::description(path).map(|n| n.trim().to_string()).filter(|n| !n.is_empty() && n.chars().count() <= 40);
    cache.insert(path.to_string(), name.clone());
    name
}

/// A 32 px icon of the program as a data URI, cached.
pub fn icon(path: &str) -> Option<String> {
    if path.is_empty() {
        return None;
    }
    let mut guard = ICONS.lock().unwrap_or_else(|e| e.into_inner());
    let cache = guard.get_or_insert_with(HashMap::new);
    if let Some(hit) = cache.get(path) {
        return hit.clone();
    }
    let uri = super::appicon::png(path).ok().and_then(|png| {
        let img = image::load_from_memory(&png).ok()?.resize(32, 32, image::imageops::FilterType::Triangle);
        let mut small = std::io::Cursor::new(Vec::new());
        img.write_to(&mut small, image::ImageFormat::Png).ok()?;
        Some(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(small.into_inner())))
    });
    cache.insert(path.to_string(), uri.clone());
    uri
}

#[cfg(windows)]
mod platform {
    use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM};
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW};
    use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, GW_OWNER, GWL_EXSTYLE, GetWindow, GetWindowLongW, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible, WS_EX_TOOLWINDOW};
    use windows::core::{BOOL, PWSTR};

    use super::AppInfo;

    /// FileDescription (else ProductName) from the program's version info, in its first language.
    pub fn description(path: &str) -> Option<String> {
        use windows::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};
        use windows::core::{HSTRING, PCWSTR};
        let file = HSTRING::from(path);
        // SAFETY: the buffer is sized by GetFileVersionInfoSizeW and outlives every pointer
        // VerQueryValueW hands out (they point into it); lengths are checked before reading.
        unsafe {
            let size = GetFileVersionInfoSizeW(&file, None);
            if size == 0 {
                return None;
            }
            let mut data = vec![0u8; size as usize];
            GetFileVersionInfoW(&file, None, size, data.as_mut_ptr().cast()).ok()?;
            let query = |sub: &str| -> Option<(*const u16, u32)> {
                let sub = HSTRING::from(sub);
                let mut ptr = std::ptr::null_mut();
                let mut len = 0u32;
                VerQueryValueW(data.as_ptr().cast(), PCWSTR(sub.as_ptr()), &mut ptr, &mut len).as_bool().then_some((ptr as *const u16, len))
            };
            let (tr, tr_len) = query(r"\VarFileInfo\Translation")?;
            let (lang, cp) = if tr_len >= 4 { (*tr, *tr.add(1)) } else { (0x0409, 0x04b0) };
            ["FileDescription", "ProductName"].iter().find_map(|field| {
                let (p, len) = query(&format!(r"\StringFileInfo\{lang:04x}{cp:04x}\{field}"))?;
                let text = String::from_utf16_lossy(std::slice::from_raw_parts(p, len as usize));
                // some programs report a length past the end of the string (Spotify: "Spotify\u{4}\0\0FileV...")
                let text: String = text.chars().take_while(|c| !c.is_control()).collect();
                let text = text.trim().to_string();
                (!text.is_empty()).then_some(text)
            })
        }
    }

    /// Full path of a process's program.
    pub fn path_of(pid: u32) -> Option<String> {
        // SAFETY: plain queries; the handle is closed before returning.
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut buf = [0u16; 1024];
            let mut len = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len).is_ok();
            let _ = CloseHandle(handle);
            ok.then(|| String::from_utf16_lossy(&buf[..len as usize]))
        }
    }

    unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> BOOL {
        // SAFETY: lparam is the Vec passed by `running` below, alive for the whole enumeration.
        let found = unsafe { &mut *(lparam.0 as *mut Vec<(u32, String)>) };
        // SAFETY: plain window queries on a window handle EnumWindows gave us.
        unsafe {
            let tool = (GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0) != 0;
            let owned = GetWindow(hwnd, GW_OWNER).is_ok_and(|o| !o.is_invalid());
            if IsWindowVisible(hwnd).as_bool() && !tool && !owned {
                let mut buf = [0u16; 256];
                let n = GetWindowTextW(hwnd, &mut buf);
                if n > 0 {
                    let mut pid = 0u32;
                    GetWindowThreadProcessId(hwnd, Some(&mut pid));
                    found.push((pid, String::from_utf16_lossy(&buf[..n as usize])));
                }
            }
        }
        BOOL(1)
    }

    /// Programs with a visible top-level window now.
    pub fn running() -> Vec<AppInfo> {
        let mut windows: Vec<(u32, String)> = Vec::new();
        // SAFETY: the callback only uses the Vec while EnumWindows runs.
        let _ = unsafe { EnumWindows(Some(collect), LPARAM(&mut windows as *mut _ as isize)) };
        let own = std::process::id();
        let mut out: Vec<AppInfo> = Vec::new();
        for (pid, title) in windows {
            if pid == own || out.iter().any(|a| a.title == title && a.path.is_empty()) {
                continue;
            }
            let Some(path) = path_of(pid) else { continue };
            let exe = path.rsplit(['\\', '/']).next().unwrap_or(&path).to_string();
            if super::ignored(&exe) || out.iter().any(|a| a.exe.eq_ignore_ascii_case(&exe)) {
                continue;
            }
            out.push(AppInfo { exe, path, title, last_seen: 0, running: true, icon: None, name: None });
        }
        out
    }
}

#[cfg(not(windows))]
mod platform {
    pub fn running() -> Vec<super::AppInfo> {
        Vec::new()
    }

    pub fn description(_path: &str) -> Option<String> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn a_program_names_itself() {
        let notepad = std::env::var("WINDIR").map(|w| format!(r"{w}\System32\notepad.exe")).unwrap_or_default();
        let name = friendly_name(&notepad).unwrap_or_default();
        assert!(!name.is_empty() && !name.ends_with(".exe"), "{name:?}");
        assert_eq!(friendly_name(r"C:\no\such.exe"), None);
    }

    #[test]
    fn system_hosts_are_not_offered() {
        assert!(ignored("ApplicationFrameHost.exe"));
        assert!(!ignored("Photoshop.exe"));
    }

    #[cfg(windows)]
    #[test]
    fn notepad_icon_is_a_small_data_uri() {
        let uri = icon(r"C:\Windows\System32\notepad.exe").expect("icon");
        assert!(uri.starts_with("data:image/png;base64,"));
        assert!(uri.len() < 12_000, "small enough to send for every app");
        assert_eq!(icon(""), None);
    }
}
