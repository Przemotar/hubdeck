//! Settings > Diagnostics: check one module (owner, 2026-11-02, after a faders user's "is it
//! HubDeck or the module?": "panel do debugowania, w którym można zaznaczyć wybrany moduł i
//! zdebugować czy działa").
//!
//! - Reads every known feature report of the module (`mh_protocol::report::diagnostic_reads`), with
//!   how long each took. Reads only: nothing is written to the module's memory.
//! - Holds the module while the window tests it: the engine's pictures and LED changes for it are
//!   dropped, so a test colour stays until the test changes it. The hold ends when the window lets
//!   go or stops renewing it (`HOLD_FOR`), and the engine then draws and lights everything again.
//! - Keeps what happened to every module since the start (attached, gone, not answering, a picture
//!   or LED batch that failed): the window shows the last hour, a report carries it.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::State;

use crate::deck::Deck;
use crate::engine::Engine;

/// A hold not renewed for this long ends by itself (a window closed mid-test leaves no module
/// stuck on a test colour).
const HOLD_FOR: Duration = Duration::from_secs(45);
/// Notes kept (the oldest go first).
const KEEP: usize = 300;

static HELD: Mutex<Option<(String, Instant)>> = Mutex::new(None);
static WATCHING: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    /// Unix time in ms.
    pub at: u64,
    pub module: String,
    pub what: String,
}

#[derive(Default)]
struct Notes {
    last: VecDeque<Note>,
    /// How many notes each module had since the start, by kind of note (a flood of failed
    /// pictures pushes the older notes out of `last`, not out of the count).
    counts: HashMap<(String, &'static str), u64>,
}

static NOTES: Mutex<Option<Notes>> = Mutex::new(None);

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Whether the engine leaves module `id` alone now (pictures and LEDs).
pub fn held(id: &str) -> bool {
    let held = HELD.lock().unwrap_or_else(|e| e.into_inner());
    held.as_ref().is_some_and(|(h, at)| h == id && at.elapsed() < HOLD_FOR)
}

/// Something happened to a module. `kind` groups the count: "attached", "gone", "not answering",
/// "picture failed", "lights failed".
pub fn note(module: &str, kind: &'static str, detail: impl std::fmt::Display) {
    let detail = detail.to_string();
    let what = if detail.is_empty() { kind.to_string() } else { format!("{kind}: {detail}") };
    let mut notes = NOTES.lock().unwrap_or_else(|e| e.into_inner());
    let notes = notes.get_or_insert_with(Notes::default);
    *notes.counts.entry((module.to_string(), kind)).or_insert(0) += 1;
    // the same failure again and again: one line with the time of the last (also when other modules'
    // notes came between; at the start every module is announced twice within milliseconds)
    let now = now_ms();
    if let Some(same) = notes.last.iter_mut().rev().find(|n| n.module == module).filter(|n| n.what == what && (kind != "attached" || now.saturating_sub(n.at) < 2000)) {
        same.at = now;
        return;
    }
    notes.last.push_back(Note { at: now_ms(), module: module.to_string(), what });
    while notes.last.len() > KEEP {
        notes.last.pop_front();
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Read {
    pub name: &'static str,
    pub id: u8,
    /// The answer's payload in hex (`feature_payload`), when it came.
    pub hex: Option<String>,
    /// The payload as text when it is readable (a firmware version).
    pub text: Option<String>,
    pub error: Option<String>,
    pub ms: f32,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")
}

/// The answers known to be words: firmware versions (after 4 bytes that differ per module, likely a
/// checksum; read on the owner's deck 2026-11-02) and the base's position sensor (its value sits after
/// a length byte of 0, so the payload alone is empty).
fn as_text(id: u8, answer: &[u8]) -> Option<String> {
    use mh_protocol::report;
    match id {
        0x2D | 0x2E | 0x2F | 0x38 | 0x48 => report::parse_firmware(answer),
        0x35 => report::parse_orientation(answer).map(|o| format!("{o:?}").to_lowercase()),
        _ => None,
    }
}

/// Every known feature report of an open module, read now (blocking: HID I/O).
pub fn read_all(m: &mh_device::Module) -> Vec<Read> {
    mh_protocol::report::diagnostic_reads(m.kind())
        .iter()
        .map(|(name, id)| {
            let started = Instant::now();
            let answer = m.query_raw(*id);
            let ms = started.elapsed().as_secs_f32() * 1000.0;
            match answer {
                Ok(bytes) => {
                    let payload = mh_protocol::report::feature_payload(&bytes);
                    Read { name, id: *id, hex: Some(hex(payload)), text: as_text(*id, &bytes), error: None, ms }
                }
                Err(e) => Read { name, id: *id, hex: None, text: None, error: Some(e.to_string()), ms },
            }
        })
        .collect()
}

/// The notes of one module (oldest first) and its counts by kind.
pub fn notes_of(module: &str) -> (Vec<Note>, Vec<(&'static str, u64)>) {
    let notes = NOTES.lock().unwrap_or_else(|e| e.into_inner());
    let Some(notes) = notes.as_ref() else { return (Vec::new(), Vec::new()) };
    let last = notes.last.iter().filter(|n| n.module == module).cloned().collect();
    let counts = notes.counts.iter().filter(|((m, _), _)| m == module).map(|((_, k), n)| (*k, *n)).collect();
    (last, counts)
}

/// Read every known feature report of a module (by its window id, `kind:serial`).
#[tauri::command]
pub async fn diag_read(deck: State<'_, Deck>, module: String) -> Result<Vec<Read>, String> {
    let m = deck.module(&module).ok_or_else(|| format!("{module} is not connected"))?;
    tauri::async_runtime::spawn_blocking(move || read_all(&m)).await.map_err(|e| e.to_string())
}

/// Hold a module for a test (`Some(id)`, renewed by calling again), or let it go (`None`).
#[tauri::command]
pub fn diag_hold(engine: State<'_, Engine>, module: Option<String>) {
    *HELD.lock().unwrap_or_else(|e| e.into_inner()) = module.map(|id| (id, Instant::now()));
    if WATCHING.swap(true, Ordering::SeqCst) {
        return;
    }
    // when the hold ends (let go, or not renewed), the engine draws and lights everything again
    let tx = engine.sender();
    let spawned = std::thread::Builder::new().name("diagnostics hold".into()).spawn(move || {
        loop {
            std::thread::sleep(Duration::from_millis(250));
            let over = {
                let mut held = HELD.lock().unwrap_or_else(|e| e.into_inner());
                if held.as_ref().is_some_and(|(_, at)| at.elapsed() >= HOLD_FOR) {
                    *held = None;
                }
                held.is_none()
            };
            if over {
                WATCHING.store(false, Ordering::SeqCst);
                let _ = tx.send(crate::engine::Msg::DeckChanged);
                return;
            }
        }
    });
    if spawned.is_err() {
        WATCHING.store(false, Ordering::SeqCst);
    }
}

/// What happened to the modules since the start: the notes (oldest first) and the counts by
/// module and kind.
#[tauri::command]
pub fn diag_notes() -> serde_json::Value {
    let notes = NOTES.lock().unwrap_or_else(|e| e.into_inner());
    let Some(notes) = notes.as_ref() else { return serde_json::json!({"notes": [], "counts": []}) };
    let counts: Vec<_> = notes.counts.iter().map(|((m, k), n)| serde_json::json!({"module": m, "kind": k, "count": n})).collect();
    serde_json::json!({"notes": notes.last, "counts": counts})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_firmware_and_the_sensor_are_read_as_words() {
        // the owner's faders, 2026-11-02: 0x2E and 0x32 as they answered
        let firmware = [0x2E, 15, 0x54, 0xe2, 0x61, 0x51, b'0', b'1', b'.', b'0', b'0', b'.', b'0', b'5', b'.', b'0', b'0'];
        assert_eq!(as_text(0x2E, &firmware).as_deref(), Some("01.00.05.00"));
        assert_eq!(as_text(0x32, &[0x32, 5, 0x64, 0x64, 0x64, 0x64, 0x64]), None, "brightness 100 is not the text dddd");
    }

    #[test]
    fn a_hold_is_for_one_module_and_ends_unrenewed() {
        *HELD.lock().unwrap() = Some(("faders:A".into(), Instant::now()));
        assert!(held("faders:A"));
        assert!(!held("faders:B"));
        *HELD.lock().unwrap() = Some(("faders:A".into(), Instant::now() - HOLD_FOR));
        assert!(!held("faders:A"));
        *HELD.lock().unwrap() = None;
    }

    #[test]
    fn the_same_failure_again_is_one_line_but_counted() {
        note("keys:T", "picture failed", "timeout");
        note("keys:T", "picture failed", "timeout");
        let notes = NOTES.lock().unwrap();
        let n = notes.as_ref().unwrap();
        assert_eq!(n.last.iter().filter(|x| x.module == "keys:T").count(), 1);
        assert_eq!(n.counts[&("keys:T".to_string(), "picture failed")], 2);
    }
}
