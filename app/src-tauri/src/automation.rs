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

/// The window, off-screen and never focused, without a taskbar button. Built hidden and shown
/// by Win32 without activation: Tauri's own position and focus options at build time were
/// ignored (the window came up on screen, focused).
pub fn open_window(app: &tauri::AppHandle) {
    let Some(cfg) = app.config().app.windows.iter().find(|w| w.label == "main") else { return };
    match tauri::WebviewWindowBuilder::from_config(app, cfg).and_then(|b| b.focused(false).skip_taskbar(true).visible(false).build()) {
        Ok(w) => show_off_screen(&w),
        Err(e) => log::warn!("automation: cannot open the window: {e}"),
    }
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
