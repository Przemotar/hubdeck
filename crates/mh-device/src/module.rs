use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use hidapi::HidDevice;
use mh_protocol::report::{self, FEATURE_LEN};
use mh_protocol::{ModuleKind, Query, Report, descriptor};
use parking_lot::FairMutex;

use crate::{Error, ModuleId, Result};

/// Time a module gets to switch to software mode before it is read (see `initialize`).
const MODE_SETTLE: Duration = Duration::from_millis(600);

/// Read timeout while holding the device lock; bounds how long a write waits for a reader.
pub(crate) const READ_SLICE: Duration = Duration::from_millis(20);

/// An open module. Reads and writes share one handle behind a mutex: the reader holds it for
/// at most [`READ_SLICE`], so writers interleave between reads.
///
/// Firmware bug (keys and dial, verified 2026-10-09): reading any feature report while the
/// module is in hardware mode wedges it until its power is cycled (the module is pulled off
/// the base): every later feature read comes back empty and its USB strings turn to garbage.
/// [`Module::query`] therefore refuses to read keys or dial outside software mode.
pub struct Module {
    pub id: ModuleId,
    /// Output report length including the report id, read from the HID descriptor.
    pub output_len: usize,
    /// USB port numbers from the root hub, e.g. `[6, 1, 4, 3]`; see [`crate::location`].
    pub port_path: Option<Vec<u8>>,
    /// Fair: the reader re-locks right after every 20 ms read, an unfair lock starves writers.
    device: FairMutex<HidDevice>,
    /// Handed back to its firmware at exit: nothing more is sent (a picture after that, while the
    /// app was closing, may be what left a dial refusing commands on the next start, 2026-10-13).
    released: AtomicBool,
    software_mode: AtomicBool,
}

impl Module {
    pub(crate) fn new(kind: ModuleKind, serial: String, device: HidDevice) -> Self {
        let mut buf = [0u8; 4096];
        let output_len = device.get_report_descriptor(&mut buf).ok().and_then(|n| descriptor::output_report_len(&buf[..n], 0x02)).unwrap_or(64);
        Self { id: ModuleId { kind, serial }, output_len, port_path: None, device: FairMutex::new(device), software_mode: AtomicBool::new(false), released: AtomicBool::new(false) }
    }

    /// Whether the module currently accepts commands and queries (always true for the M0 family).
    pub fn is_ready(&self) -> bool {
        !self.kind().needs_software_mode() || self.software_mode.load(Ordering::SeqCst)
    }

    fn track_mode(&self, report: &Report) {
        for on in [true, false] {
            if report::software_mode(self.kind(), on).as_ref() == Some(report) {
                self.software_mode.store(on, Ordering::SeqCst);
            }
        }
    }

    pub fn kind(&self) -> ModuleKind {
        self.id.kind
    }

    /// Send one report. Refuses forbidden commands even when built by hand.
    pub fn send(&self, report: &Report) -> Result<()> {
        self.send_all(std::slice::from_ref(report))
    }

    /// Send reports back to back under one lock. An image is ~100 reports; releasing the lock
    /// between them let the reader take it for a full read slice each time (2026-10-09: 3.3 s
    /// for a 33 KB panel image instead of tens of milliseconds).
    pub fn send_all(&self, reports: &[Report]) -> Result<()> {
        if reports.iter().any(Report::is_forbidden) {
            return Err(Error::Forbidden);
        }
        let device = self.device.lock();
        if self.released.load(Ordering::SeqCst) {
            return Ok(());
        }
        for report in reports {
            match report {
                Report::Output(bytes) => {
                    let mut buf = bytes.clone();
                    buf.resize(self.output_len.max(bytes.len()), 0);
                    device.write(&buf)?;
                }
                Report::Feature(bytes) => {
                    let mut buf = bytes.clone();
                    buf.resize(FEATURE_LEN.max(bytes.len()), 0);
                    device.send_feature_report(&buf)?;
                }
            }
        }
        drop(device);
        reports.iter().for_each(|r| self.track_mode(r));
        Ok(())
    }

    /// Raw feature report answer (`[id, len, payload...]`). Refused on keys and dial in
    /// hardware mode, see the type-level docs.
    pub fn query_raw(&self, report_id: u8) -> Result<Vec<u8>> {
        if !self.is_ready() {
            return Err(Error::HardwareMode(self.kind().name()));
        }
        let mut buf = [0u8; FEATURE_LEN + 1];
        buf[0] = report_id;
        let device = self.device.lock();
        let n = device.get_feature_report(&mut buf)?;
        Ok(buf[..n].to_vec())
    }

    pub fn query(&self, query: Query) -> Result<Vec<u8>> {
        let id = report::query_id(self.kind(), query).ok_or(Error::Unsupported(self.kind().name()))?;
        self.query_raw(id)
    }

    pub fn firmware(&self) -> Result<Option<String>> {
        Ok(report::parse_firmware(&self.query(Query::Firmware)?))
    }

    /// One read slice; `Ok(None)` on timeout, `Err` when the module is gone.
    pub(crate) fn read(&self, buf: &mut [u8]) -> Result<Option<usize>> {
        let device = self.device.lock();
        let n = device.read_timeout(buf, READ_SLICE.as_millis() as i32)?;
        Ok((n > 0).then_some(n))
    }

    /// Keys and dial: enter software mode and confirm the module answers. On 2026-10-09 a keys
    /// module once took every command but answered nothing; toggling the mode is the retry.
    ///
    /// The read that confirms it is the dangerous part: a read that lands while the module is still
    /// in hardware mode wedges it (PROTOCOL.md). On macOS, modules taken right after another app
    /// gave them back wedged now and then with a 200 ms wait (2026-10-13), so the mode is sent
    /// twice with time to settle before the first read, and a failed send is never followed by a read.
    pub(crate) fn initialize(&self) -> Result<()> {
        let Some(on) = report::software_mode(self.kind(), true) else { return Ok(()) };
        let name = self.kind().name();
        for attempt in 0..3 {
            if attempt > 0 {
                if let Some(off) = report::software_mode(self.kind(), false) {
                    self.send(&off).inspect_err(|e| log::warn!("init {name}: attempt {attempt}: hardware mode: {e}"))?;
                }
                std::thread::sleep(Duration::from_millis(500));
            }
            self.send(&on).inspect_err(|e| log::warn!("init {name}: attempt {attempt}: software mode: {e}"))?;
            std::thread::sleep(MODE_SETTLE);
            self.send(&on).inspect_err(|e| log::warn!("init {name}: attempt {attempt}: software mode again: {e}"))?;
            std::thread::sleep(Duration::from_millis(200));
            match self.firmware() {
                Ok(Some(_)) => return Ok(()),
                Ok(None) => log::warn!("init {name}: attempt {attempt}: no firmware answer"),
                Err(e) => {
                    log::warn!("init {name}: attempt {attempt}: firmware read: {e}");
                    return Err(e);
                }
            }
        }
        Err(Error::NoAnswer { kind: self.kind().name(), serial: self.id.serial.clone() })
    }

    /// Hand keys and dial back to their firmware (idle animation). No-op for other modules.
    /// Under the device lock, so it never lands inside a picture, and nothing is sent after it.
    pub fn release(&self) -> Result<()> {
        let device = self.device.lock();
        if self.released.swap(true, Ordering::SeqCst) {
            return Ok(());
        }
        if let Some(Report::Feature(bytes)) = report::software_mode(self.kind(), false) {
            let mut buf = bytes.clone();
            buf.resize(FEATURE_LEN.max(bytes.len()), 0);
            device.send_feature_report(&buf)?;
            self.software_mode.store(false, Ordering::SeqCst);
        }
        Ok(())
    }
}
