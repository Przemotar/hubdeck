//! The guard: a small second process (`hubdeck --guard <data dir> <log dir>`) that waits for
//! HubDeck to end. A clean exit says "bye" first; any other end (a crash, Task Manager) leaves
//! keys and dial in software mode showing the last picture forever, because the firmware has no
//! watchdog (2026-10-15: frozen from 00:27 to the morning). Then the guard hands the deck back
//! and starts HubDeck again in the tray (release builds; at most 3 times in 10 minutes).
//!
//! The guard learns of the end through its stdin: HubDeck holds the pipe's other end, and the
//! system closes it however HubDeck ends. No polling, nothing to read while HubDeck runs.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{ChildStdin, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const ARG: &str = "--guard";
const BYE: &[u8] = b"bye\n";
const RESTARTS: &str = "guard-restarts";
const MAX_RESTARTS: usize = 3;
const RESTART_WINDOW_MS: i64 = 10 * 60 * 1000;
const RESTART_DELAY: Duration = Duration::from_secs(3);
/// What the window shows at the next start (owner, 2026-10-15: a crash report in the app).
pub const REPORT: &str = "last-crash.json";
const REPORT_LINES: usize = 60;

static PIPE: Mutex<Option<ChildStdin>> = Mutex::new(None);

/// Start the guard for this HubDeck (once at start; again if an update could not install).
pub fn spawn(data_dir: &Path, log_dir: &Path) {
    let Ok(exe) = std::env::current_exe() else { return };
    let mut cmd = Command::new(exe);
    cmd.arg(ARG).arg(data_dir).arg(log_dir).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    match cmd.spawn() {
        Ok(mut child) => {
            *PIPE.lock().unwrap_or_else(|e| e.into_inner()) = child.stdin.take();
            log::info!("guard running (pid {})", child.id());
        }
        Err(e) => log::warn!("guard did not start: {e}"),
    }
}

/// HubDeck ends on purpose (Quit, an update): the guard goes away without touching anything.
pub fn leaving() {
    if let Some(mut pipe) = PIPE.lock().unwrap_or_else(|e| e.into_inner()).take() {
        let _ = pipe.write_all(BYE).and_then(|_| pipe.flush());
    }
}

/// `hubdeck --guard <data dir> <log dir>`: runs until HubDeck ends.
pub fn run(args: &[String]) {
    let (Some(data_dir), Some(log_dir)) = (args.first().map(PathBuf::from), args.get(1).map(PathBuf::from)) else { return };
    let log = |msg: &str| line(&log_dir, msg);
    let mut got = Vec::new();
    let _ = std::io::stdin().lock().read_to_end(&mut got);
    if got.starts_with(BYE) {
        return;
    }
    log("HubDeck ended without quitting: handing the deck back");
    let ended = chrono::Local::now();
    let tail = log_tail(&log_dir);
    hand_back(&log);
    crate::deck::mark_released_in(&data_dir);
    let report = |restarted: bool| write_report(&data_dir, ended, &tail, restarted);
    if cfg!(debug_assertions) {
        log("dev build: not starting HubDeck again");
        report(false);
        return;
    }
    std::thread::sleep(RESTART_DELAY);
    if shutting_down() {
        report(false);
        return;
    }
    let now = chrono::Utc::now().timestamp_millis();
    let path = data_dir.join(RESTARTS);
    let mut recent: Vec<i64> = std::fs::read_to_string(&path).unwrap_or_default().lines().filter_map(|l| l.trim().parse().ok()).filter(|t| now - t < RESTART_WINDOW_MS).collect();
    if recent.len() >= MAX_RESTARTS {
        log(&format!("HubDeck ended {MAX_RESTARTS} times within 10 minutes: not starting it again (the deck stays handed back)"));
        report(false);
        return;
    }
    recent.push(now);
    let _ = std::fs::write(&path, recent.iter().map(|t| format!("{t}\n")).collect::<String>());
    report(true);
    let Ok(exe) = std::env::current_exe() else { return };
    match Command::new(&exe).arg(crate::HIDDEN_ARG).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
        Ok(child) => log(&format!("HubDeck started again in the tray (pid {}, restart {} of {MAX_RESTARTS} in 10 minutes)", child.id(), recent.len())),
        Err(e) => log(&format!("could not start HubDeck again: {e}")),
    }
}

/// Open the deck, let every module connect, and close it: closing hands keys and dial back to
/// their firmware, as at a clean exit.
fn hand_back(log: &dyn Fn(&str)) {
    let started = Instant::now();
    let hub = match mh_device::Hub::start() {
        Ok(hub) => hub,
        Err(e) => return log(&format!("could not open the deck: {e}")),
    };
    // wait until no new module has shown up for 1.5 s (at most 8 s)
    let (mut count, mut since) = (0, Instant::now());
    while started.elapsed() < Duration::from_secs(8) {
        let n = hub.modules().len();
        if n != count {
            (count, since) = (n, Instant::now());
        } else if n > 0 && since.elapsed() > Duration::from_millis(1500) {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    hub.close();
    log(&format!("deck handed back: {count} module(s) in {} ms", started.elapsed().as_millis()));
}

/// The log's last lines before the end (the guard's own lines left out).
fn log_tail(log_dir: &Path) -> Vec<String> {
    let text = std::fs::read(log_dir.join("hubdeck.log")).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    let lines: Vec<&str> = text.lines().filter(|l| !l.contains("[hubdeck_lib::guard]")).collect();
    lines[lines.len().saturating_sub(REPORT_LINES)..].iter().map(|l| l.to_string()).collect()
}

/// The report the window shows at the next start: when, whether HubDeck came back, the panic (if
/// the log has one) and the last log lines to copy (nothing is sent anywhere).
fn write_report(data_dir: &Path, ended: chrono::DateTime<chrono::Local>, tail: &[String], restarted: bool) {
    let panic = tail.iter().rev().find(|l| l.contains("panic in thread")).cloned();
    let report = serde_json::json!({
        "at": ended.format("%Y-%m-%d %H:%M:%S").to_string(),
        "restarted": restarted,
        "panic": panic,
        "log": tail,
    });
    let _ = std::fs::write(data_dir.join(REPORT), serde_json::to_vec_pretty(&report).unwrap_or_default());
}

/// The last unexpected end, if the window has not shown it yet.
#[tauri::command]
pub fn crash_report(engine: tauri::State<'_, crate::engine::Engine>) -> Option<serde_json::Value> {
    let mut report: serde_json::Value = serde_json::from_slice(&std::fs::read(engine.data_dir().join(REPORT)).ok()?).ok()?;
    // it can go into a public bug report (Report this crash): private things out, as in diagnostics
    if let Some(lines) = report.get_mut("log").and_then(|l| l.as_array_mut()) {
        *lines = lines.iter().filter_map(|l| l.as_str().and_then(crate::engine::diagnostics::scrub)).map(serde_json::Value::String).collect();
    }
    if let Some(panic) = report.get("panic").and_then(|p| p.as_str()).and_then(crate::engine::diagnostics::scrub) {
        report["panic"] = serde_json::Value::String(panic);
    }
    Some(report)
}

/// The user has seen it: kept beside as the last one seen, so the banner does not come back.
#[tauri::command]
pub fn crash_seen(engine: tauri::State<'_, crate::engine::Engine>) {
    let dir = engine.data_dir();
    let _ = std::fs::rename(dir.join(REPORT), dir.join("last-crash.seen.json"));
}

#[cfg(windows)]
fn shutting_down() -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_SHUTTINGDOWN};
    // SAFETY: a plain query with no pointers.
    unsafe { GetSystemMetrics(SM_SHUTTINGDOWN) != 0 }
}

#[cfg(not(windows))]
fn shutting_down() -> bool {
    false
}

/// A line in HubDeck's own log, in its format (the app is not running to write it).
fn line(log_dir: &Path, msg: &str) {
    let at = chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]");
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(log_dir.join("hubdeck.log")) {
        let _ = writeln!(f, "{at}[INFO][hubdeck_lib::guard] {msg}");
    }
}
