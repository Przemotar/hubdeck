//! Test mode, for checking the app from a script (Playwright over the web view's DevTools port)
//! without getting in the way of the person at the computer.
//!
//! `HUBDECK_AUTOMATION=<folder>` starts a separate copy that keeps its data in `<folder>` (copy the
//! real data there first to test on it), logs to `<folder>/logs`, has no tray icon, no single
//! instance and no autostart or update check, and opens its window off-screen without focus or a
//! taskbar button. The web view listens for DevTools on 127.0.0.1:`HUBDECK_CDP_PORT` (9333).
//! Instead of the deck it shows virtual modules (`HUBDECK_VIRTUAL_MODULES`, see [`virtual_modules`])
//! and draws nothing, so the real deck stays with the running app; `POST /api/test/input` presses,
//! turns and slides them. Never set outside tests: the DevTools port lets any local program drive
//! the window.

use std::path::PathBuf;
use std::sync::OnceLock;

use mh_protocol::ModuleKind;

pub struct Automation {
    pub data_dir: PathBuf,
    pub cdp_port: u16,
}

/// The film mode of the test mode (`HUBDECK_FILM_DECK=1` with `HUBDECK_AUTOMATION`): the README film
/// (scripts/film/) needs the window driven by a script AND the real deck, with its own clean data
/// (owner, 2026-10-15: "nie widziałem, żeby tam pokazywało jakieś klikanie, przełączanie"). The
/// window is on screen and the real deck is used, so the running HubDeck must be quit first.
pub fn real_deck() -> bool {
    get().is_some() && std::env::var_os("HUBDECK_FILM_DECK").is_some_and(|v| !v.is_empty())
}

/// The test mode, if this process was started in it.
pub fn get() -> Option<&'static Automation> {
    static MODE: OnceLock<Option<Automation>> = OnceLock::new();
    MODE.get_or_init(|| {
        let dir = std::env::var_os("HUBDECK_AUTOMATION").filter(|d| !d.is_empty())?;
        let cdp_port = std::env::var("HUBDECK_CDP_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(9333);
        Some(Automation { data_dir: PathBuf::from(dir), cdp_port })
    })
    .as_ref()
}

/// Before any window exists: the web view gets its own profile folder (two copies cannot share
/// one with different options) and the DevTools port; occlusion is off, so the off-screen window
/// keeps drawing and screenshots show it.
pub fn prepare_webview(a: &Automation) {
    let args = format!("--remote-debugging-port={} --disable-features=CalculateNativeWinOcclusion --disable-backgrounding-occluded-windows --disable-renderer-backgrounding", a.cdp_port);
    // SAFETY: called at the start of `run`, before the app starts any thread
    unsafe {
        std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", a.data_dir.join("webview"));
        std::env::set_var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", args);
    }
}

/// In a test copy: log instead of opening a link, file or folder (it would take the focus).
pub fn blocks_opening(what: &str) -> bool {
    if get().is_none() {
        return false;
    }
    log::info!("test copy: not opened: {what}");
    true
}

/// Marks the test copy's page so the system's dialogs answer from the walk instead of coming up on
/// the owner's screen (`app/src/confirm.ts`; a walk's Uninstall question did, 2026-10-23).
const NO_DIALOGS: &str = "window.__HUBDECK_TEST__ = { answers: [], asked: [] };";

/// The window, off-screen and never focused, without a taskbar button. Built hidden and shown
/// by Win32 without activation: Tauri's own position and focus options at build time were
/// ignored (the window came up on screen, focused).
pub fn open_window(app: &tauri::AppHandle) {
    let Some(cfg) = app.config().app.windows.iter().find(|w| w.label == "main") else { return };
    match tauri::WebviewWindowBuilder::from_config(app, cfg).and_then(|b| b.focused(false).skip_taskbar(true).visible(false).initialization_script(NO_DIALOGS).build()) {
        Ok(w) => show_off_screen(&w),
        Err(e) => log::warn!("automation: cannot open the window: {e}"),
    }
    watch_screen();
}

/// Every half second: a window of this test copy that came up on the screen (a system dialog, a
/// window built without the off-screen placement) is logged as `test copy: on the screen: <title>`
/// and moved off it. Every walk fails on that line (`scripts/ui/screen.mjs`; owner's pick 1a,
/// 2026-10-23, after a walk's question came up on his screen at 14:24).
#[cfg(windows)]
fn watch_screen() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static STARTED: AtomicBool = AtomicBool::new(false);
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(|| {
        log::info!("test copy: watching the screen");
        let mut seen = std::collections::HashSet::new();
        loop {
            for (hwnd, title) in on_screen() {
                if seen.insert(hwnd) {
                    log::warn!("test copy: on the screen: {title}");
                }
                hide(hwnd);
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    });
}

#[cfg(not(windows))]
fn watch_screen() {}

/// This process's visible top-level windows that lie on the screen: over the monitors' area, in the
/// same (DPI-scaled) units the window's rectangle comes in for this thread; off-screen windows sit
/// at -20000. `HUBDECK_SCREEN_FROM=<x>` moves the screen's left edge, so `scripts/ui/screen-check.mjs`
/// can test the watch with a window still far off every monitor.
#[cfg(windows)]
fn on_screen() -> Vec<(isize, String)> {
    use windows::Win32::Foundation::{HWND, LPARAM, RECT};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetClassNameW, GetSystemMetrics, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
        SM_YVIRTUALSCREEN,
    };
    use windows::core::BOOL;
    // SAFETY: plain system queries
    let mut screen = unsafe {
        let (x, y) = (GetSystemMetrics(SM_XVIRTUALSCREEN), GetSystemMetrics(SM_YVIRTUALSCREEN));
        RECT { left: x, top: y, right: x + GetSystemMetrics(SM_CXVIRTUALSCREEN), bottom: y + GetSystemMetrics(SM_CYVIRTUALSCREEN) }
    };
    if let Some(from) = std::env::var("HUBDECK_SCREEN_FROM").ok().and_then(|v| v.parse::<i32>().ok()) {
        (screen.left, screen.top) = (from, from);
    }
    struct Search {
        screen: RECT,
        found: Vec<(isize, String)>,
    }
    unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> BOOL {
        // SAFETY: lparam is the Search passed below, alive for the whole enumeration.
        let search = unsafe { &mut *(lparam.0 as *mut Search) };
        let screen = search.screen;
        // SAFETY: plain window queries on a window handle EnumWindows gave us.
        unsafe {
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            let mut r = RECT::default();
            let mut class = [0u16; 64];
            let n = GetClassNameW(hwnd, &mut class);
            // Tao's message window for its event loop: "visible", 14 px, never drawn (2026-10-23)
            let event_target = String::from_utf16_lossy(&class[..n.max(0) as usize]) == "Tao Thread Event Target";
            let over = |r: &RECT| r.left < screen.right && r.right > screen.left && r.top < screen.bottom && r.bottom > screen.top && r.right > r.left;
            if pid == std::process::id() && !event_target && IsWindowVisible(hwnd).as_bool() && GetWindowRect(hwnd, &mut r).is_ok() && over(&r) {
                let mut buf = [0u16; 256];
                let n = GetWindowTextW(hwnd, &mut buf);
                search.found.push((hwnd.0 as isize, String::from_utf16_lossy(&buf[..n.max(0) as usize])));
            }
        }
        BOOL(1)
    }
    let mut search = Search { screen, found: Vec::new() };
    // SAFETY: the callback only uses the Search while EnumWindows runs.
    let _ = unsafe { EnumWindows(Some(collect), LPARAM(&mut search as *mut _ as isize)) };
    search.found
}

#[cfg(windows)]
fn hide(hwnd: isize) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{HWND_BOTTOM, SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE, SWP_NOSIZE, SetWindowPos};
    // SAFETY: a handle EnumWindows just gave for a window of this process; a stale one only fails.
    // Asynchronous: a window whose thread is busy (a modal dialog's) must not stop the watch.
    let _ = unsafe { SetWindowPos(HWND(hwnd as *mut _), Some(HWND_BOTTOM), -20000, -20000, 0, 0, SWP_NOSIZE | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS) };
}

#[cfg(windows)]
fn show_off_screen(w: &tauri::WebviewWindow) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{HWND_BOTTOM, SWP_NOACTIVATE, SWP_NOSIZE, SWP_SHOWWINDOW, SetWindowPos};
    let Ok(raw) = w.hwnd() else { return };
    // SAFETY: the window handle is alive (just built) and owned by this thread's app
    if let Err(e) = unsafe { SetWindowPos(HWND(raw.0), Some(HWND_BOTTOM), -20000, -20000, 0, 0, SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW) } {
        log::warn!("automation: cannot place the window: {e}");
    }
}

#[cfg(not(windows))]
fn show_off_screen(w: &tauri::WebviewWindow) {
    let _ = w.set_position(tauri::PhysicalPosition::new(-20000, -20000));
    let _ = w.show();
}

/// `kind@port.path` entries, comma separated (`keys@6.2.3`); the port path gives the slot in the
/// base, so the copied placements apply. Without the variable: the owner's Creator Kit set.
pub fn virtual_modules() -> Vec<(ModuleKind, Vec<u8>)> {
    let spec = std::env::var("HUBDECK_VIRTUAL_MODULES").unwrap_or_else(|_| "base@6.1.4.4,keys@6.2.3,dial@6.1.3.4,knobs@6.1.4.3,rollers@6.3.3".into());
    parse_modules(&spec)
}

fn parse_modules(spec: &str) -> Vec<(ModuleKind, Vec<u8>)> {
    spec.split(',')
        .filter_map(|entry| {
            let (kind, path) = entry.trim().split_once('@')?;
            let kind = ModuleKind::from_name(kind)?;
            let path = path.split('.').map(|p| p.parse().ok()).collect::<Option<Vec<u8>>>()?;
            Some((kind, path))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modules_spec_is_read_and_bad_entries_are_skipped() {
        let m = parse_modules("keys@6.2.3, dial@6.1.3.4,wheel@1.2,knobs@x");
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].0.name(), "keys");
        assert_eq!(m[1].1, vec![6, 1, 3, 4]);
    }
}
