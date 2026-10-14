use std::collections::{BTreeMap, HashMap};
use std::ffi::CString;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, unbounded};
use hidapi::HidApi;
use log::{debug, info, warn};
use mh_protocol::{ModuleKind, Query, VENDOR_ID, input, report};

use crate::events::EdgeState;
use crate::{Event, Module, ModuleId, Result};

/// A scan (HID enumeration) costs ~150 ms of CPU on Windows, so the hub does not poll every
/// second: it scans when the system reports a HID device arriving or leaving, every second for a
/// short while after that (modules attach one after another) and while a module is retrying, and
/// otherwise only every `IDLE_RESCAN` as a safety net. Without notifications it polls every second.
const RESCAN: Duration = Duration::from_secs(1);
const IDLE_RESCAN: Duration = Duration::from_secs(15);
/// Keep scanning every second this long after a device change.
const BUSY_FOR: Duration = Duration::from_secs(10);
/// After a failed open or init, wait this long before trying the same module again.
const RETRY_AFTER: Duration = Duration::from_secs(2);
/// macOS: after this many failed starts in a row, re-enumerate the module (as reseating it does),
/// at most once a minute per kind of module.
const RESET_AFTER_FAILURES: u32 = 3;
const RESET_AT_MOST_EVERY: Duration = Duration::from_secs(60);

/// Retry state per OS path: when to try again and how many starts failed in a row.
type Backoff = HashMap<CString, (Instant, u32)>;

struct Shared {
    modules: Mutex<BTreeMap<ModuleId, Arc<Module>>>,
    paths: Mutex<HashMap<CString, ModuleId>>,
    readers: Mutex<Vec<JoinHandle<()>>>,
    stop: AtomicBool,
    tx: Sender<Event>,
}

/// Owns every connected module. A scanner thread opens modules as they appear (every second),
/// one reader thread per module turns reports into [`Event`]s, and a read error closes the
/// module and emits [`Event::Disconnected`]. Dropping the hub hands keys and dial back to
/// hardware mode.
pub struct Hub {
    shared: Arc<Shared>,
    events: Receiver<Event>,
    scanner: Option<JoinHandle<()>>,
}

impl Hub {
    /// Start the hub; returns after the first scan, so already attached modules are ready.
    pub fn start() -> Result<Self> {
        let (tx, events) = unbounded();
        let shared = Arc::new(Shared { modules: Mutex::new(BTreeMap::new()), paths: Mutex::new(HashMap::new()), readers: Mutex::new(Vec::new()), stop: AtomicBool::new(false), tx });
        let (ready_tx, ready_rx) = crossbeam_channel::bounded::<Result<()>>(1);
        let scanner_shared = shared.clone();
        let scanner = std::thread::Builder::new().name("mh-scan".into()).spawn(move || scan_loop(scanner_shared, ready_tx)).expect("spawn scanner thread");
        ready_rx.recv().unwrap_or(Ok(()))?;
        Ok(Self { shared, events, scanner: Some(scanner) })
    }

    pub fn events(&self) -> &Receiver<Event> {
        &self.events
    }

    pub fn modules(&self) -> Vec<Arc<Module>> {
        lock(&self.shared.modules).values().cloned().collect()
    }

    pub fn module(&self, id: &ModuleId) -> Option<Arc<Module>> {
        lock(&self.shared.modules).get(id).cloned()
    }

    /// First connected module of a kind (most setups have one of each).
    pub fn first(&self, kind: ModuleKind) -> Option<Arc<Module>> {
        lock(&self.shared.modules).values().find(|m| m.kind() == kind).cloned()
    }

    /// Stop all threads and hand keys and dial back to hardware mode.
    pub fn close(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        if self.shared.stop.swap(true, Ordering::SeqCst) {
            return;
        }
        // each step timed (2026-10-14: an app that quit before the hand-back finished left the
        // dial refusing commands at the next start)
        let started = Instant::now();
        if let Some(s) = self.scanner.take() {
            let _ = s.join();
        }
        let scanner = started.elapsed();
        for r in std::mem::take(&mut *lock(&self.shared.readers)) {
            let _ = r.join();
        }
        let readers = started.elapsed();
        for (id, m) in std::mem::take(&mut *lock(&self.shared.modules)) {
            let t = Instant::now();
            if let Err(e) = m.release() {
                warn!("release {id}: {e} (after {:?})", t.elapsed());
            }
        }
        log::info!("hub closed: scanner {scanner:?}, readers {:?}, hand-back {:?}", readers - scanner, started.elapsed() - readers);
    }
}

impl Drop for Hub {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn scan_loop(shared: Arc<Shared>, ready: Sender<Result<()>>) {
    let mut api = match HidApi::new() {
        Ok(api) => api,
        Err(e) => {
            let _ = ready.send(Err(e.into()));
            return;
        }
    };
    // macOS opens HID devices exclusively by default; `MH_HID_SHARED=1` opens them shared (the
    // system's own driver stays attached), to find out what wedges modules after a restart there
    #[cfg(target_os = "macos")]
    if std::env::var_os("MH_HID_SHARED").is_some() {
        api.set_open_exclusive(false);
        info!("HID devices opened shared");
    }
    let mut backoff: Backoff = HashMap::new();
    let mut last_reset: HashMap<ModuleKind, Instant> = HashMap::new();
    scan_once(&shared, &mut api, &mut backoff, &mut last_reset);
    let _ = ready.send(Ok(()));
    let notified = crate::notify::watch();
    let mut busy_until = Instant::now() + BUSY_FOR;
    let mut last_scan = Instant::now();
    while !shared.stop.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(50));
        if crate::notify::changed() {
            busy_until = Instant::now() + BUSY_FOR;
        }
        let busy = !notified || Instant::now() < busy_until || !backoff.is_empty();
        let every = if busy { RESCAN } else { IDLE_RESCAN };
        if last_scan.elapsed() >= every && !shared.stop.load(Ordering::SeqCst) {
            last_scan = Instant::now();
            scan_once(&shared, &mut api, &mut backoff, &mut last_reset);
            // a module that answered is done retrying
            backoff.retain(|_, (t, _)| Instant::now() < *t + RETRY_AFTER * 5);
        }
    }
}

fn scan_once(shared: &Arc<Shared>, api: &mut HidApi, backoff: &mut Backoff, last_reset: &mut HashMap<ModuleKind, Instant>) {
    if let Err(e) = api.reset_devices().and_then(|_| api.add_devices(VENDOR_ID, 0)) {
        warn!("enumerate: {e}");
        return;
    }
    // Keyed by OS path: the USB serial string of a wedged keys/dial module is garbage and
    // changes between scans, so it cannot tell "already open" apart from "new".
    let found: Vec<_> = api
        .device_list()
        .filter_map(|d| {
            let kind = ModuleKind::from_product_id(d.product_id())?;
            Some((kind, d.serial_number().unwrap_or_default().to_string(), d.path().to_owned()))
        })
        .collect();
    for (kind, descriptor_serial, path) in found {
        if lock(&shared.paths).contains_key(&path) || backoff.get(&path).is_some_and(|(t, _)| Instant::now() < *t) {
            continue;
        }
        let mut module = match api.open_path(&path) {
            Ok(device) => Module::new(kind, descriptor_serial, device),
            Err(e) => {
                debug!("open {}: {e} (module still attaching?)", kind.name());
                let failures = backoff.get(&path).map_or(0, |b| b.1);
                backoff.insert(path, (Instant::now() + RETRY_AFTER, failures));
                continue;
            }
        };
        if let Err(e) = module.initialize() {
            warn!("init {}: {e}", kind.name());
            let failures = backoff.get(&path).map_or(0, |b| b.1) + 1;
            backoff.insert(path.clone(), (Instant::now() + RETRY_AFTER, failures));
            if failures == 1 && matches!(e, crate::Error::NoAnswer { .. }) {
                let _ = shared.tx.send(Event::Unresponsive(module.id.clone()));
            }
            // a module that keeps refusing its start is stuck in its firmware; on macOS bring it
            // back the way reseating it does (the owner had to pull keys and dial off, 2026-10-13)
            if cfg!(target_os = "macos") && failures >= RESET_AFTER_FAILURES && last_reset.get(&kind).is_none_or(|t| t.elapsed() >= RESET_AT_MOST_EVERY) {
                last_reset.insert(kind, Instant::now());
                drop(module);
                match crate::recover::reenumerate(&path) {
                    Ok(()) => info!("re-enumerated {} after {failures} failed starts", kind.name()),
                    Err(e) => warn!("re-enumerate {}: {e}", kind.name()),
                }
            }
            continue;
        }
        // the serial from the module itself is reliable once it answers
        if let Ok(answer) = module.query(Query::SerialNumber)
            && let Some(serial) = report::parse_serial_number(&answer)
        {
            module.id.serial = serial;
        }
        module.port_path = crate::location::usb_port_path(&path);
        backoff.remove(&path);
        let module = Arc::new(module);
        let id = module.id.clone();
        info!("connected {id} (output report {} B, port path {:?})", module.output_len, module.port_path);
        lock(&shared.modules).insert(id.clone(), module.clone());
        lock(&shared.paths).insert(path.clone(), id.clone());
        let reader_shared = shared.clone();
        let reader = std::thread::Builder::new().name(format!("mh-read-{}", id.kind.name())).spawn(move || read_loop(reader_shared, module, path)).expect("spawn reader thread");
        let mut readers = lock(&shared.readers);
        readers.retain(|r| !r.is_finished());
        readers.push(reader);
        let _ = shared.tx.send(Event::Connected(id));
    }
}

fn read_loop(shared: Arc<Shared>, module: Arc<Module>, path: CString) {
    let id = module.id.clone();
    let mut edges = EdgeState::default();
    let mut buf = [0u8; 64];
    while !shared.stop.load(Ordering::SeqCst) {
        match module.read(&mut buf) {
            Ok(Some(n)) => {
                for inp in input::parse(id.kind, &buf[..n]) {
                    for ev in edges.apply(&id, inp) {
                        let _ = shared.tx.send(ev);
                    }
                }
            }
            Ok(None) => {}
            Err(e) => {
                info!("disconnected {id} ({e})");
                let mut modules = lock(&shared.modules);
                if modules.get(&id).is_some_and(|m| Arc::ptr_eq(m, &module)) {
                    modules.remove(&id);
                }
                drop(modules);
                lock(&shared.paths).remove(&path);
                let _ = shared.tx.send(Event::Disconnected(id));
                return;
            }
        }
    }
}
