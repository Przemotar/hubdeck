//! Device service: runs the hot-plug hub on its own thread (HID never touches the UI or async
//! runtime), keeps a snapshot of what is attached, and forwards every change to the window.
//!
//! The hub can be let go and started again: while the deck is paused (tray menu) or the computer
//! sleeps, keys and dial are back in hardware mode and nothing is read or drawn.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use mh_device::{Event, Hub, ModuleId};
use mh_protocol::{ModuleKind, Orientation, Query, report};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::engine::{InputEvent, Msg as EngineMsg};

/// Name of the event the window listens to.
pub const DECK_EVENT: &str = "deck://event";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleInfo {
    pub id: String,
    pub kind: &'static str,
    pub serial: String,
    pub firmware: Option<String>,
    /// Slot inside the base, e.g. "1-4-3" (USB port path below the base's hub tree).
    pub slot: Option<String>,
    #[serde(skip)]
    pub port_path: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckSnapshot {
    pub modules: Vec<ModuleInfo>,
    /// "horizontal" | "vertical", from the base sensor; None until a base answers.
    pub orientation: Option<&'static str>,
    /// Clockwise degrees the base is turned on the desk (0, 90, 180, 270): the user's choice if
    /// set, otherwise from the sensor (horizontal = 270, vertical = 0).
    pub turn: i32,
    /// The user's choice of `turn`; None = follow the sensor.
    pub turn_override: Option<i32>,
    /// Modules that need to be restarted or reseated (firmware wedged).
    pub unresponsive: Vec<String>,
    /// Paused from the tray: keys and dial are handed back to their firmware.
    pub paused: bool,
    /// Where each fader stands now, by `<module>#<index>`: a fader reports its place once when
    /// attached, so a window opened later showed every cap in the middle until it moved
    /// (owner, 2026-10-15).
    pub faders: BTreeMap<String, u8>,
}

/// The sensor only tells one upright position (vertical) from the rest (`PROTOCOL.md`); the rest
/// is taken as lying flat, turned left. Upright, the drawings stood upside down at 0 (owner's deck,
/// 2026-10-14: "obracanie decku przekręca rzeczy do góry nogami"): that position is a half turn
/// from 0, 90 degrees on from lying flat at 270 the other way.
pub fn base_turn(orientation: Option<&str>, turn_override: Option<i32>) -> i32 {
    match (turn_override, orientation) {
        (Some(t), _) => t.rem_euclid(360) / 90 * 90,
        (None, Some("horizontal")) => 270,
        (None, Some("vertical")) => 180,
        _ => 0,
    }
}

/// What the window receives. Tagged so TypeScript can switch on `type`.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum UiEvent {
    Snapshot { deck: DeckSnapshot },
    KeyDown { module: String, index: u8 },
    KeyUp { module: String, index: u8 },
    Fader { module: String, index: u8, value: u8 },
    Knob { module: String, index: u8, delta: i8 },
    KnobDown { module: String, index: u8 },
    KnobUp { module: String, index: u8 },
    Roller { module: String, index: u8, delta: i8 },
    Dial { module: String, delta: i8 },
    DialDown { module: String },
    DialUp { module: String },
}

/// UI event -> (control id, engine input), e.g. ("keys.4", Press(true)).
fn engine_input(ev: &UiEvent) -> Option<(String, InputEvent)> {
    Some(match ev {
        UiEvent::KeyDown { index, .. } => (format!("keys.{index}"), InputEvent::Press(true)),
        UiEvent::KeyUp { index, .. } => (format!("keys.{index}"), InputEvent::Press(false)),
        UiEvent::Fader { index, value, .. } => (format!("faders.{index}"), InputEvent::Slide(*value)),
        UiEvent::Knob { index, delta, .. } => (format!("knobs.{index}"), InputEvent::Turn(*delta)),
        UiEvent::KnobDown { index, .. } => (format!("knobs.{index}"), InputEvent::Press(true)),
        UiEvent::KnobUp { index, .. } => (format!("knobs.{index}"), InputEvent::Press(false)),
        UiEvent::Roller { index, delta, .. } => (format!("rollers.{index}"), InputEvent::Turn(*delta)),
        UiEvent::Dial { delta, .. } => ("dial".into(), InputEvent::Turn(*delta)),
        UiEvent::DialDown { .. } => ("dial".into(), InputEvent::Press(true)),
        UiEvent::DialUp { .. } => ("dial".into(), InputEvent::Press(false)),
        UiEvent::Snapshot { .. } => return None,
    })
}

fn orientation_name(o: Orientation) -> &'static str {
    match o {
        Orientation::Horizontal => "horizontal",
        Orientation::Vertical => "vertical",
    }
}

fn module_key(id: &ModuleId) -> String {
    format!("{}:{}", id.kind.name(), id.serial)
}

#[derive(Default)]
struct Inner {
    modules: BTreeMap<String, ModuleInfo>,
    orientation: Option<&'static str>,
    turn_override: Option<i32>,
    unresponsive: BTreeMap<String, ()>,
    paused: bool,
    faders: BTreeMap<String, u8>,
}

impl Inner {
    fn snapshot(&self) -> DeckSnapshot {
        let base_path = self.modules.values().find(|m| m.kind == "base").and_then(|m| m.port_path.clone());
        let modules = self
            .modules
            .values()
            .map(|m| {
                let slot = match (&m.port_path, &base_path) {
                    (Some(p), Some(b)) => mh_device::location::slot_in_base(p, b),
                    _ => None,
                };
                ModuleInfo { slot, ..m.clone() }
            })
            .collect();
        DeckSnapshot {
            modules,
            orientation: self.orientation,
            turn: base_turn(self.orientation, self.turn_override),
            turn_override: self.turn_override,
            unresponsive: self.unresponsive.keys().cloned().collect(),
            paused: self.paused,
            faders: self.faders.clone(),
        }
    }
}

/// Shared with Tauri commands through `State`.
#[derive(Clone, Default)]
pub struct Deck {
    inner: Arc<Mutex<Inner>>,
    stop: Arc<AtomicBool>,
    /// Paused by the user (tray menu).
    paused: Arc<AtomicBool>,
    /// The computer is going to sleep, or has just woken up and USB is settling.
    suspended: Arc<AtomicBool>,
    /// True while a hub owns the modules (keys and dial in software mode).
    running: Arc<AtomicBool>,
    hub: Arc<Mutex<Option<Arc<Hub>>>>,
    engine: Arc<Mutex<Option<crossbeam_channel::Sender<EngineMsg>>>>,
    app: Arc<Mutex<Option<AppHandle>>>,
}

/// How long to wait after resume before opening the modules again; USB devices come back one by
/// one and a module that is still attaching would be reported as not responding.
#[cfg_attr(not(windows), allow(dead_code))]
const RESUME_SETTLE: Duration = Duration::from_secs(2);

/// When HubDeck last handed the modules back (`<data>/released-at`, written at exit).
const RELEASED_AT: &str = "released-at";
/// How long keys and dial get after a hand-back before the next start takes them. On macOS a module
/// taken a second after the last app gave it back refused every command until it was reseated
/// (2026-10-13, three times in about fifteen restarts).
const SETTLE: Duration = Duration::from_secs(6);

/// Remember the hand-back time for the next start.
pub fn mark_released(app: &AppHandle) {
    if let Ok(dir) = app.path().app_data_dir() {
        mark_released_in(&dir);
    }
}

/// The same from the guard, which has no app handle.
pub fn mark_released_in(dir: &std::path::Path) {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let _ = std::fs::write(dir.join(RELEASED_AT), now.to_string());
}

/// Wait out the rest of `SETTLE` if the last app gave the modules back only moments ago.
fn settle_after_last_release(app: &AppHandle) {
    let Ok(dir) = app.path().app_data_dir() else { return };
    let Some(at) = std::fs::read_to_string(dir.join(RELEASED_AT)).ok().and_then(|s| s.trim().parse::<u128>().ok()) else { return };
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let since = Duration::from_millis(now.saturating_sub(at).min(u64::MAX as u128) as u64);
    if since < SETTLE {
        log::info!("modules were handed back {} ms ago; waiting before taking them", since.as_millis());
        std::thread::sleep(SETTLE - since);
    }
}

impl Deck {
    pub fn snapshot(&self) -> DeckSnapshot {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).snapshot()
    }

    /// Where input and deck changes go besides the window. Set before [`Deck::spawn`].
    pub fn set_engine(&self, tx: crossbeam_channel::Sender<EngineMsg>) {
        *self.engine.lock().unwrap_or_else(|e| e.into_inner()) = Some(tx);
    }

    fn to_engine(&self, msg: EngineMsg) {
        if let Some(tx) = self.engine.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            let _ = tx.send(msg);
        }
    }

    /// Stop for good (app exit) and wait until keys and dial are handed back.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
        // the hand-back must finish before the app exits: a module left mid-way refused every
        // command at the next start (2026-10-14, macOS; the log missed "deck released" each time)
        let started = Instant::now();
        self.wait_released(Duration::from_secs(8));
        if self.running.load(Ordering::SeqCst) {
            log::warn!("the deck was not handed back within {:?}", started.elapsed());
        }
    }

    /// Hand the modules back to their firmware (`true`) or take them again (`false`).
    pub fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::SeqCst);
        self.lock().paused = paused;
        if let Some(app) = self.app() {
            self.publish_snapshot(&app);
        }
        log::info!("deck {}", if paused { "paused" } else { "resumed" });
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    /// The computer is about to sleep: release keys and dial before USB goes down. Blocks until
    /// they are released (Windows gives the callback about two seconds).
    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn suspend(&self) {
        log::info!("system suspend: releasing the deck");
        self.suspended.store(true, Ordering::SeqCst);
        self.wait_released(Duration::from_millis(1500));
    }

    /// The computer woke up: open the modules again once USB has settled.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn resume(&self) {
        if !self.suspended.load(Ordering::SeqCst) {
            return;
        }
        log::info!("system resume: opening the deck in {RESUME_SETTLE:?}");
        let suspended = self.suspended.clone();
        std::thread::spawn(move || {
            std::thread::sleep(RESUME_SETTLE);
            suspended.store(false, Ordering::SeqCst);
        });
    }

    /// Set (or with `None`, clear) the base turn chosen by the user.
    pub fn set_turn_override(&self, turn: Option<i32>) {
        self.lock().turn_override = turn.map(|t| t.rem_euclid(360) / 90 * 90);
        if let Some(app) = self.app() {
            self.publish_snapshot(&app);
        }
        self.to_engine(EngineMsg::DeckChanged);
    }

    fn held(&self) -> bool {
        self.paused.load(Ordering::SeqCst) || self.suspended.load(Ordering::SeqCst)
    }

    fn wait_released(&self, timeout: Duration) {
        let until = Instant::now() + timeout;
        while self.running.load(Ordering::SeqCst) && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn app(&self) -> Option<AppHandle> {
        self.app.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// An open module by its UI id (`"kind:serial"`), for commands that draw or light it.
    pub fn module(&self, id: &str) -> Option<Arc<mh_device::Module>> {
        // quitting: nothing more is drawn or lit, so the hand-back has the modules to itself
        if self.stop.load(Ordering::SeqCst) {
            return None;
        }
        let hub = self.hub.lock().unwrap_or_else(|e| e.into_inner()).clone()?;
        hub.modules().into_iter().find(|m| module_key(&m.id) == id)
    }

    /// Start the device thread. It lives until [`Deck::stop`]; whenever it lets the hub go, keys
    /// and dial are handed back to hardware mode.
    pub fn spawn(&self, app: AppHandle) {
        *self.app.lock().unwrap_or_else(|e| e.into_inner()) = Some(app.clone());
        let deck = self.clone();
        std::thread::Builder::new().name("deck".into()).spawn(move || deck.run(app)).expect("spawn deck thread");
    }

    /// Test mode (`automation`): these modules instead of the hardware; nothing is drawn.
    pub fn spawn_virtual(&self, app: AppHandle, modules: Vec<(ModuleKind, Vec<u8>)>) {
        *self.app.lock().unwrap_or_else(|e| e.into_inner()) = Some(app.clone());
        {
            let mut inner = self.lock();
            for (i, (kind, path)) in modules.into_iter().enumerate() {
                let serial = format!("VIRTUAL{i}");
                let id = format!("{}:{serial}", kind.name());
                inner.modules.insert(id.clone(), ModuleInfo { id, kind: kind.name(), serial, firmware: None, slot: None, port_path: Some(path) });
            }
            inner.orientation = Some("horizontal");
        }
        log::info!("automation: {} virtual module(s)", self.lock().modules.len());
        self.publish_snapshot(&app);
        self.to_engine(EngineMsg::DeckChanged);
    }

    /// Test mode: a control of a virtual module used as if by hand (`keys` 4 `press`, `dial` `turn`
    /// -1, `faders` 2 `slide` 60); the window and the engine get what the hardware would send.
    pub fn virtual_input(&self, kind: &str, index: u8, gesture: &str, value: i64) -> Result<(), String> {
        let app = self.app.lock().unwrap_or_else(|e| e.into_inner()).clone().ok_or("not started")?;
        let module = self.lock().modules.values().find(|m| m.kind == kind).map(|m| m.id.clone()).ok_or_else(|| format!("no {kind} module"))?;
        let delta = value.clamp(-10, 10) as i8;
        let events = match (kind, gesture) {
            ("keys", "press") => vec![UiEvent::KeyDown { module: module.clone(), index }, UiEvent::KeyUp { module, index }],
            ("knobs", "press") => vec![UiEvent::KnobDown { module: module.clone(), index }, UiEvent::KnobUp { module, index }],
            ("dial", "press") => vec![UiEvent::DialDown { module: module.clone() }, UiEvent::DialUp { module }],
            ("knobs", "turn") => vec![UiEvent::Knob { module, index, delta }],
            ("rollers", "turn") => vec![UiEvent::Roller { module, index, delta }],
            ("dial", "turn") => vec![UiEvent::Dial { module, delta }],
            ("faders", "slide") => vec![UiEvent::Fader { module, index, value: value.clamp(0, 100) as u8 }],
            _ => return Err(format!("{kind} cannot {gesture}")),
        };
        for ui in events {
            if let Some((control, input)) = engine_input(&ui) {
                self.to_engine(EngineMsg::Input { control, event: input });
            }
            let _ = app.emit(DECK_EVENT, ui);
        }
        Ok(())
    }

    fn run(&self, app: AppHandle) {
        settle_after_last_release(&app);
        while !self.stop.load(Ordering::SeqCst) {
            if self.held() {
                std::thread::sleep(Duration::from_millis(50));
                continue;
            }
            let hub = match Hub::start() {
                Ok(hub) => Arc::new(hub),
                Err(e) => {
                    log::error!("hub failed to start: {e}");
                    std::thread::sleep(Duration::from_secs(5));
                    continue;
                }
            };
            self.running.store(true, Ordering::SeqCst);
            *self.hub.lock().unwrap_or_else(|e| e.into_inner()) = Some(hub.clone());
            self.serve(&app, &hub);
            // the last Arc drops here: the hub hands keys and dial back to hardware mode
            self.hub.lock().unwrap_or_else(|e| e.into_inner()).take();
            drop(hub);
            self.running.store(false, Ordering::SeqCst);
            {
                let mut inner = self.lock();
                inner.modules.clear();
                inner.unresponsive.clear();
            }
            self.publish_snapshot(&app);
            self.to_engine(EngineMsg::DeckChanged);
            log::info!("deck released");
            crate::engine::vjoy::release_all();
        }
    }

    /// Forward hub events until the app stops or the deck is paused or suspended.
    fn serve(&self, app: &AppHandle, hub: &Hub) {
        for m in hub.modules() {
            self.on_connected(hub, &m.id);
        }
        self.publish_snapshot(app);
        self.to_engine(EngineMsg::DeckChanged);
        let mut fader_seen = std::collections::HashSet::<String>::new();

        while !self.stop.load(Ordering::SeqCst) && !self.held() {
            let Ok(event) = hub.events().recv_timeout(Duration::from_millis(50)) else { continue };
            let ui = match event {
                Event::Connected(id) => {
                    self.on_connected(hub, &id);
                    self.publish_snapshot(app);
                    self.to_engine(EngineMsg::DeckChanged);
                    continue;
                }
                Event::Disconnected(id) => {
                    let key = module_key(&id);
                    fader_seen.retain(|k| !k.starts_with(&format!("{key}#")));
                    let mut inner = self.lock();
                    inner.faders.retain(|k, _| !k.starts_with(&format!("{key}#")));
                    inner.modules.remove(&key);
                    drop(inner);
                    self.publish_snapshot(app);
                    self.to_engine(EngineMsg::DeckChanged);
                    continue;
                }
                Event::Unresponsive(id) => {
                    self.lock().unresponsive.insert(id.kind.name().to_string(), ());
                    self.publish_snapshot(app);
                    continue;
                }
                Event::Orientation { orientation, .. } => {
                    self.lock().orientation = Some(orientation_name(orientation));
                    self.publish_snapshot(app);
                    self.to_engine(EngineMsg::DeckChanged);
                    continue;
                }
                Event::KeyDown { module, index } => UiEvent::KeyDown { module: module_key(&module), index },
                Event::KeyUp { module, index } => UiEvent::KeyUp { module: module_key(&module), index },
                Event::Fader { module, index, value } => UiEvent::Fader { module: module_key(&module), index, value },
                Event::Knob { module, index, delta } => UiEvent::Knob { module: module_key(&module), index, delta },
                Event::KnobDown { module, index } => UiEvent::KnobDown { module: module_key(&module), index },
                Event::KnobUp { module, index } => UiEvent::KnobUp { module: module_key(&module), index },
                Event::Roller { module, index, delta } => UiEvent::Roller { module: module_key(&module), index, delta },
                Event::Dial { module, delta } => UiEvent::Dial { module: module_key(&module), delta },
                Event::DialDown { module } => UiEvent::DialDown { module: module_key(&module) },
                Event::DialUp { module } => UiEvent::DialUp { module: module_key(&module) },
            };
            // a fader reports its position once when attached; that only feeds the drawing,
            // actions run when it is moved (otherwise starting the app would set volumes)
            let baseline = match &ui {
                UiEvent::Fader { module, index, value } => {
                    self.lock().faders.insert(format!("{module}#{index}"), *value);
                    fader_seen.insert(format!("{module}#{index}"))
                }
                _ => false,
            };
            match (baseline, &ui) {
                (true, UiEvent::Fader { index, value, .. }) => self.to_engine(EngineMsg::Level { control: format!("faders.{index}"), value: *value }),
                (false, _) => {
                    if let Some((control, input)) = engine_input(&ui) {
                        self.to_engine(EngineMsg::Input { control, event: input });
                    }
                }
                _ => {}
            }
            let _ = app.emit(DECK_EVENT, ui);
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn on_connected(&self, hub: &Hub, id: &ModuleId) {
        let Some(m) = hub.module(id) else { return };
        let firmware = m.firmware().ok().flatten();
        if id.kind == ModuleKind::Base
            && let Some(o) = m.query(Query::Orientation).ok().and_then(|a| report::parse_orientation(&a))
        {
            self.lock().orientation = Some(orientation_name(o));
        }
        let info = ModuleInfo { id: module_key(id), kind: id.kind.name(), serial: id.serial.clone(), firmware, slot: None, port_path: m.port_path.clone() };
        let mut inner = self.lock();
        inner.unresponsive.remove(id.kind.name());
        inner.modules.insert(info.id.clone(), info);
    }

    fn publish_snapshot(&self, app: &AppHandle) {
        let _ = app.emit(DECK_EVENT, UiEvent::Snapshot { deck: self.snapshot() });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_turn_follows_the_sensor_unless_overridden() {
        assert_eq!(base_turn(None, None), 0);
        assert_eq!(base_turn(Some("vertical"), None), 180);
        assert_eq!(base_turn(Some("horizontal"), None), 270);
        assert_eq!(base_turn(Some("horizontal"), Some(90)), 90);
        assert_eq!(base_turn(None, Some(-90)), 270);
        assert_eq!(base_turn(None, Some(185)), 180);
    }
}
