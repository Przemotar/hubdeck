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
    /// The number of its first control: 0, or for a second module of the same kind the numbers
    /// after (`knobs.3` .. `knobs.5`, `keys.15` .. `keys.29`, `dial.1`). Each module keeps its numbers
    /// (remembered by serial number in `module-numbers.json`), so taking one off never hands its
    /// actions to another (owner, 2026-11-01: "co jak ktoś wepnie modul nr 1, a potem taki sam modul
    /// nr 2, a potem ten nr 1 odepnie?"); every kind works this way since 2026-11-02 (owner: "dodać
    /// te obsługę wielu modułów tego samego typu dla każdego z modułów").
    pub first: u16,
}

/// Kinds whose second module gets controls of its own: every kind with controls (the base has none).
/// The living deck's scene and the screensaver are shared by keys modules; their keys are not.
const OWN_CONTROLS: [ModuleKind; 5] = [ModuleKind::Keys, ModuleKind::Knobs, ModuleKind::Faders, ModuleKind::Rollers, ModuleKind::Dial];

/// The group of every attached module (by id; its first control is `group * control_count`): a
/// module keeps the group it had (`remembered`) unless an attached one took it first; a new one
/// gets the lowest group free. The base is always 0. Modules are taken in
/// the order of their port paths, so the result does not depend on the order they came in.
fn assign_groups<'a>(modules: impl Iterator<Item = &'a ModuleInfo>, remembered: &BTreeMap<String, u16>) -> BTreeMap<String, u16> {
    let mut by_kind: BTreeMap<&'static str, Vec<&ModuleInfo>> = BTreeMap::new();
    for m in modules {
        by_kind.entry(m.kind).or_default().push(m);
    }
    let mut out = BTreeMap::new();
    for (kind, mut ms) in by_kind {
        if !OWN_CONTROLS.iter().any(|k| k.name() == kind) {
            out.extend(ms.into_iter().map(|m| (m.id.clone(), 0)));
            continue;
        }
        ms.sort_by(|a, b| (&a.port_path, &a.serial).cmp(&(&b.port_path, &b.serial)));
        let mut taken = std::collections::BTreeSet::new();
        let mut later = Vec::new();
        for m in ms {
            match remembered.get(&m.id) {
                Some(g) if taken.insert(*g) => {
                    out.insert(m.id.clone(), *g);
                }
                _ => later.push(m),
            }
        }
        for m in later {
            let g = (0u16..).find(|g| !taken.contains(g)).unwrap_or(0);
            taken.insert(g);
            out.insert(m.id.clone(), g);
        }
    }
    out
}

/// The first control number of a module in `group`.
fn first_of(kind: &str, group: u16) -> u16 {
    OWN_CONTROLS.iter().find(|k| k.name() == kind).map_or(0, |k| group * k.control_count() as u16)
}

/// The event with its control numbered as the engine and the window know it (see `first`).
fn numbered(ui: UiEvent, first: u16) -> UiEvent {
    let at = |index: u8| (u16::from(index) + first).min(u16::from(u8::MAX)) as u8;
    match ui {
        UiEvent::Fader { module, index, value } => UiEvent::Fader { module, index: at(index), value },
        UiEvent::Knob { module, index, delta } => UiEvent::Knob { module, index: at(index), delta },
        UiEvent::KnobDown { module, index } => UiEvent::KnobDown { module, index: at(index) },
        UiEvent::KnobUp { module, index } => UiEvent::KnobUp { module, index: at(index) },
        UiEvent::Roller { module, index, delta } => UiEvent::Roller { module, index: at(index), delta },
        UiEvent::KeyDown { module, index } => UiEvent::KeyDown { module, index: at(index) },
        UiEvent::KeyUp { module, index } => UiEvent::KeyUp { module, index: at(index) },
        UiEvent::Dial { module, index, delta } => UiEvent::Dial { module, index: at(index), delta },
        UiEvent::DialDown { module, index } => UiEvent::DialDown { module, index: at(index) },
        UiEvent::DialUp { module, index } => UiEvent::DialUp { module, index: at(index) },
        other => other,
    }
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
    /// While no module is attached yet: how long ago the hub began looking (ms). A restart takes
    /// the deck back in about 6 s; until then the window says "Connecting", not "No MasterHUB".
    pub looking_ms: Option<u64>,
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
    Snapshot {
        deck: DeckSnapshot,
    },
    KeyDown {
        module: String,
        index: u8,
    },
    KeyUp {
        module: String,
        index: u8,
    },
    Fader {
        module: String,
        index: u8,
        value: u8,
    },
    Knob {
        module: String,
        index: u8,
        delta: i8,
    },
    KnobDown {
        module: String,
        index: u8,
    },
    KnobUp {
        module: String,
        index: u8,
    },
    Roller {
        module: String,
        index: u8,
        delta: i8,
    },
    /// `index` 0 is the first dial (`dial`), 1 a second one (`dial.1`).
    Dial {
        module: String,
        index: u8,
        delta: i8,
    },
    DialDown {
        module: String,
        index: u8,
    },
    DialUp {
        module: String,
        index: u8,
    },
}

/// The control id of dial `index`: the first is `dial` (every profile made before a second dial
/// was possible names it so), a second `dial.1`.
pub fn dial_id(index: u8) -> String {
    if index == 0 { "dial".into() } else { format!("dial.{index}") }
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
        UiEvent::Dial { index, delta, .. } => (dial_id(*index), InputEvent::Turn(*delta)),
        UiEvent::DialDown { index, .. } => (dial_id(*index), InputEvent::Press(true)),
        UiEvent::DialUp { index, .. } => (dial_id(*index), InputEvent::Press(false)),
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
    /// When the hub last began looking for modules (start, resume).
    looking_since: Option<Instant>,
    /// Each attached module's group (see [`assign_groups`]), by id.
    groups: BTreeMap<String, u16>,
    /// Every module's group as last given, attached or not (`module-numbers.json`).
    remembered: BTreeMap<String, u16>,
    numbers_file: Option<std::path::PathBuf>,
}

impl Inner {
    /// Give the attached modules their groups after one came or went; new ones are remembered.
    fn renumber(&mut self) {
        self.groups = assign_groups(self.modules.values(), &self.remembered);
        let mut changed = false;
        for (id, g) in &self.groups {
            if self.modules.get(id).is_some_and(|m| OWN_CONTROLS.iter().any(|k| k.name() == m.kind)) && self.remembered.get(id) != Some(g) {
                self.remembered.insert(id.clone(), *g);
                changed = true;
            }
        }
        if changed && let Some(path) = &self.numbers_file {
            match serde_json::to_string_pretty(&self.remembered) {
                Ok(text) => {
                    if let Err(e) = std::fs::write(path, text) {
                        log::warn!("module numbers not saved: {e}");
                    }
                }
                Err(e) => log::warn!("module numbers not saved: {e}"),
            }
        }
        let second: Vec<String> = self.groups.iter().filter(|(_, g)| **g > 0).map(|(id, g)| format!("{id} -> {}", self.modules.get(id).map_or(0, |m| first_of(m.kind, *g)))).collect();
        if !second.is_empty() {
            log::info!("more than one module of a kind: {} (first control number)", second.join(", "));
        }
    }

    /// The first control number of attached module `id`.
    fn first(&self, id: &str) -> u16 {
        self.modules.get(id).map_or(0, |m| first_of(m.kind, self.groups.get(id).copied().unwrap_or(0)))
    }

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
                ModuleInfo { slot, first: self.first(&m.id), ..m.clone() }
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
            looking_ms: self.looking_since.filter(|_| self.modules.is_empty()).map(|t| t.elapsed().as_millis() as u64),
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
    /// Where each module's control numbers are kept (`module-numbers.json`), read now.
    pub fn set_numbers_file(&self, path: std::path::PathBuf) {
        let remembered = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        let mut inner = self.lock();
        inner.remembered = remembered;
        inner.numbers_file = Some(path);
        inner.renumber();
    }

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
                inner.modules.insert(id.clone(), ModuleInfo { id, kind: kind.name(), serial, firmware: None, slot: None, port_path: Some(path), first: 0 });
            }
            inner.renumber();
            inner.orientation = Some("horizontal");
        }
        log::info!("automation: {} virtual module(s)", self.lock().modules.len());
        self.publish_snapshot(&app);
        self.to_engine(EngineMsg::DeckChanged);
    }

    /// Test mode: a control of a virtual module used as if by hand (`keys` 4 `press`, `dial` `turn`
    /// -1, `faders` 2 `slide` 60, `knobs` 4 `turn` 1 on a second knobs module); the window and the
    /// engine get what the hardware would send.
    pub fn virtual_input(&self, kind: &str, index: u8, gesture: &str, value: i64) -> Result<(), String> {
        let app = self.app.lock().unwrap_or_else(|e| e.into_inner()).clone().ok_or("not started")?;
        // the module whose numbers hold `index` (the last one starting at or below it)
        let (module, first) = {
            let inner = self.lock();
            inner
                .modules
                .values()
                .filter(|m| m.kind == kind)
                .map(|m| (m.id.clone(), inner.first(&m.id)))
                .filter(|(_, f)| *f <= u16::from(index))
                .max_by_key(|(_, f)| *f)
                .ok_or_else(|| format!("no {kind} module"))?
        };
        let index = (u16::from(index) - first) as u8;
        let delta = value.clamp(-10, 10) as i8;
        let events = match (kind, gesture) {
            ("keys", "press") => vec![UiEvent::KeyDown { module: module.clone(), index }, UiEvent::KeyUp { module, index }],
            ("knobs", "press") => vec![UiEvent::KnobDown { module: module.clone(), index }, UiEvent::KnobUp { module, index }],
            ("dial", "press") => vec![UiEvent::DialDown { module: module.clone(), index }, UiEvent::DialUp { module, index }],
            ("knobs", "turn") => vec![UiEvent::Knob { module, index, delta }],
            ("rollers", "turn") => vec![UiEvent::Roller { module, index, delta }],
            ("dial", "turn") => vec![UiEvent::Dial { module, index, delta }],
            ("faders", "slide") => vec![UiEvent::Fader { module, index, value: value.clamp(0, 100) as u8 }],
            _ => return Err(format!("{kind} cannot {gesture}")),
        };
        for ui in events.into_iter().map(|ui| numbered(ui, first)) {
            if let Some((control, input)) = engine_input(&ui) {
                self.to_engine(EngineMsg::Input { control, event: input });
            }
            let _ = app.emit(DECK_EVENT, ui);
        }
        Ok(())
    }

    fn run(&self, app: AppHandle) {
        self.lock().looking_since = Some(Instant::now());
        settle_after_last_release(&app);
        let mut first = true;
        while !self.stop.load(Ordering::SeqCst) {
            if self.held() {
                std::thread::sleep(Duration::from_millis(50));
                continue;
            }
            if !std::mem::take(&mut first) {
                self.lock().looking_since = Some(Instant::now());
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
                    crate::module_check::note(&key, "gone", "");
                    fader_seen.retain(|k| !k.starts_with(&format!("{key}#")));
                    let mut inner = self.lock();
                    inner.faders.retain(|k, _| !k.starts_with(&format!("{key}#")));
                    inner.modules.remove(&key);
                    inner.renumber();
                    drop(inner);
                    self.publish_snapshot(app);
                    self.to_engine(EngineMsg::DeckChanged);
                    continue;
                }
                Event::Unresponsive(id) => {
                    crate::module_check::note(&module_key(&id), "not answering", "");
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
                Event::Dial { module, delta } => UiEvent::Dial { module: module_key(&module), index: 0, delta },
                Event::DialDown { module } => UiEvent::DialDown { module: module_key(&module), index: 0 },
                Event::DialUp { module } => UiEvent::DialUp { module: module_key(&module), index: 0 },
            };
            // a second module of a kind: its controls numbered after the first's
            let ui = {
                let inner = self.lock();
                let module = match &ui {
                    UiEvent::Fader { module, .. }
                    | UiEvent::Knob { module, .. }
                    | UiEvent::KnobDown { module, .. }
                    | UiEvent::KnobUp { module, .. }
                    | UiEvent::Roller { module, .. }
                    | UiEvent::KeyDown { module, .. }
                    | UiEvent::KeyUp { module, .. }
                    | UiEvent::Dial { module, .. }
                    | UiEvent::DialDown { module, .. }
                    | UiEvent::DialUp { module, .. } => Some(module.clone()),
                    UiEvent::Snapshot { .. } => None,
                };
                let first = module.map_or(0, |m| inner.first(&m));
                numbered(ui, first)
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
        crate::module_check::note(&module_key(id), "attached", firmware.as_deref().map(|f| format!("firmware {f}")).unwrap_or_else(|| "no firmware answer".into()));
        let info = ModuleInfo { id: module_key(id), kind: id.kind.name(), serial: id.serial.clone(), firmware, slot: None, port_path: m.port_path.clone(), first: 0 };
        let mut inner = self.lock();
        inner.unresponsive.remove(id.kind.name());
        inner.modules.insert(info.id.clone(), info);
        inner.renumber();
    }

    fn publish_snapshot(&self, app: &AppHandle) {
        let _ = app.emit(DECK_EVENT, UiEvent::Snapshot { deck: self.snapshot() });
    }
}

#[cfg(test)]
mod tests {
    fn module(kind: ModuleKind, serial: &str, path: &[u8]) -> ModuleInfo {
        ModuleInfo { id: format!("{}:{serial}", kind.name()), kind: kind.name(), serial: serial.into(), firmware: None, slot: None, port_path: Some(path.to_vec()), first: 0 }
    }

    #[test]
    fn a_second_module_of_a_kind_keeps_its_numbers_when_the_first_goes() {
        // the owner's case (2026-11-01): knobs 1 in, then knobs 2, then knobs 1 out
        let one = module(ModuleKind::Knobs, "A", &[1, 4, 3]);
        let two = module(ModuleKind::Knobs, "B", &[1, 1, 3]);
        let mut remembered = BTreeMap::new();
        let g = assign_groups([&one].into_iter(), &remembered);
        assert_eq!(g["knobs:A"], 0);
        remembered.extend(g);
        // the second one sits in a lower slot, but the first keeps its numbers
        let g = assign_groups([&one, &two].into_iter(), &remembered);
        assert_eq!((g["knobs:A"], g["knobs:B"]), (0, 1));
        assert_eq!(first_of("knobs", g["knobs:B"]), 3);
        remembered.extend(g);
        let g = assign_groups([&two].into_iter(), &remembered);
        assert_eq!(g["knobs:B"], 1, "knobs 2 keeps knobs.3 .. knobs.5 with knobs 1 taken off");
        // a new module alone (one swapped for another) takes the lowest free numbers: the profile's
        // knobs.0 .. knobs.2 still work for someone who owns one
        let three = module(ModuleKind::Knobs, "C", &[1, 4, 3]);
        assert_eq!(assign_groups([&three].into_iter(), &remembered)["knobs:C"], 0);
        // keys and the dial too (2026-11-02): a second keys module is keys.15 .. keys.29, a second dial dial.1
        let k1 = module(ModuleKind::Keys, "K1", &[2, 3]);
        let k2 = module(ModuleKind::Keys, "K2", &[3, 3]);
        let g = assign_groups([&k1, &k2].into_iter(), &BTreeMap::new());
        assert_eq!((g["keys:K1"], g["keys:K2"]), (0, 1));
        assert_eq!(first_of("keys", 1), 15);
        assert_eq!(first_of("dial", 1), 1);
        let base = module(ModuleKind::Base, "B", &[1]);
        assert_eq!(assign_groups([&base].into_iter(), &BTreeMap::new())["base:B"], 0);
        assert_eq!(first_of("faders", 1), 5);
        assert_eq!(first_of("rollers", 2), 4);
    }

    #[test]
    fn events_of_a_second_module_go_on_from_the_firsts_numbers() {
        let ui = numbered(UiEvent::Knob { module: "knobs:B".into(), index: 1, delta: 1 }, 3);
        assert_eq!(engine_input(&ui).map(|(c, _)| c).as_deref(), Some("knobs.4"));
        let ui = numbered(UiEvent::KeyDown { module: "keys:K2".into(), index: 4 }, 15);
        assert_eq!(engine_input(&ui).map(|(c, _)| c).as_deref(), Some("keys.19"), "a second keys module's keys go on from 15");
        let ui = numbered(UiEvent::Dial { module: "dial:D2".into(), index: 0, delta: 1 }, 1);
        assert_eq!(engine_input(&ui).map(|(c, _)| c).as_deref(), Some("dial.1"));
        let ui = numbered(UiEvent::DialDown { module: "dial:D1".into(), index: 0 }, 0);
        assert_eq!(engine_input(&ui).map(|(c, _)| c).as_deref(), Some("dial"), "the first dial keeps its old name");
    }

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
