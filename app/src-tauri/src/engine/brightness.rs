//! Monitor brightness over DDC/CI (the monitor's own setting, not a dimmed picture). Talking to a
//! monitor takes 50-200 ms, so a worker thread does it; the engine keeps the value it asked for
//! and shows that right away. Monitors without DDC/CI (most laptop panels, some TVs) are skipped.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crossbeam_channel::{Sender, unbounded};

/// Which monitors to change: all, or the n-th one (1-based, in the order Windows lists them).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Which {
    All,
    One(usize),
}

impl Which {
    pub fn from_param(s: &str) -> Which {
        s.parse::<usize>().ok().filter(|n| *n >= 1).map(Which::One).unwrap_or(Which::All)
    }
}

static WORKER: OnceLock<Sender<(Which, u32)>> = OnceLock::new();
/// Last value we set (or read) per target, 0..=100.
static KNOWN: Mutex<Option<HashMap<Which, f32>>> = Mutex::new(None);

fn worker() -> &'static Sender<(Which, u32)> {
    WORKER.get_or_init(|| {
        let (tx, rx) = unbounded::<(Which, u32)>();
        std::thread::Builder::new()
            .name("brightness".into())
            .spawn(move || {
                while let Ok(mut job) = rx.recv() {
                    // only the newest value per target matters when the dial spins
                    for next in rx.try_iter() {
                        if next.0 == job.0 {
                            job = next;
                        } else if let Err(e) = platform::set(job.0, job.1) {
                            log::warn!("brightness: {e}");
                        }
                    }
                    if let Err(e) = platform::set(job.0, job.1) {
                        log::warn!("brightness: {e}");
                    }
                }
            })
            .expect("spawn brightness thread");
        tx
    })
}

/// The brightness we believe the target has now, reading it from the monitor the first time.
pub fn get(which: Which) -> Result<f32, String> {
    if let Some(v) = KNOWN.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|k| k.get(&which)) {
        return Ok(*v);
    }
    let v = platform::get(which)? as f32;
    KNOWN.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).insert(which, v);
    Ok(v)
}

/// Ask for a new brightness (0..=100); returns at once, the worker talks to the monitor.
pub fn set(which: Which, value: f32) {
    let value = value.clamp(0.0, 100.0);
    KNOWN.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).insert(which, value);
    let _ = worker().send((which, value.round() as u32));
}

#[cfg(windows)]
mod platform {
    use windows::Win32::Devices::Display::{
        DestroyPhysicalMonitors, GetMonitorBrightness, GetNumberOfPhysicalMonitorsFromHMONITOR, GetPhysicalMonitorsFromHMONITOR, PHYSICAL_MONITOR, SetMonitorBrightness,
    };
    use windows::Win32::Foundation::{LPARAM, RECT};
    use windows::Win32::Graphics::Gdi::{EnumDisplayMonitors, HDC, HMONITOR};
    use windows::core::BOOL;

    use super::Which;

    unsafe extern "system" fn collect(monitor: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> BOOL {
        // SAFETY: `data` is the Vec passed by `with_monitors`, alive for the whole enumeration.
        let list = unsafe { &mut *(data.0 as *mut Vec<HMONITOR>) };
        list.push(monitor);
        BOOL(1)
    }

    /// Run `f` on every physical monitor of the target with its 1-based number; handles are freed after.
    fn with_monitors<T>(which: Which, mut f: impl FnMut(&PHYSICAL_MONITOR) -> Option<T>) -> Result<Vec<T>, String> {
        let mut displays: Vec<HMONITOR> = Vec::new();
        // SAFETY: the callback only pushes into `displays`, which outlives the call.
        unsafe {
            let _ = EnumDisplayMonitors(None, None, Some(collect), LPARAM(&mut displays as *mut _ as isize));
        }
        let mut out = Vec::new();
        let mut number = 0usize;
        for display in displays {
            let mut count = 0u32;
            // SAFETY: plain dxva2 calls; the array is sized from the count and destroyed below.
            unsafe {
                if GetNumberOfPhysicalMonitorsFromHMONITOR(display, &mut count).is_err() || count == 0 {
                    continue;
                }
                let mut physical = vec![PHYSICAL_MONITOR::default(); count as usize];
                if GetPhysicalMonitorsFromHMONITOR(display, &mut physical).is_err() {
                    continue;
                }
                for m in &physical {
                    number += 1;
                    if which == Which::All || which == Which::One(number) {
                        out.extend(f(m));
                    }
                }
                let _ = DestroyPhysicalMonitors(&physical);
            }
        }
        Ok(out)
    }

    pub fn get(which: Which) -> Result<u32, String> {
        let values = with_monitors(which, |m| {
            let (mut min, mut cur, mut max) = (0u32, 0u32, 0u32);
            // SAFETY: the handle is valid inside `with_monitors`.
            (unsafe { GetMonitorBrightness(m.hPhysicalMonitor, &mut min, &mut cur, &mut max) } != 0 && max > min).then(|| (cur - min) * 100 / (max - min))
        })?;
        values.first().copied().ok_or_else(|| "no monitor answers brightness over DDC/CI (laptop screens and some monitors do not)".into())
    }

    pub fn set(which: Which, value: u32) -> Result<(), String> {
        let done = with_monitors(which, |m| {
            let (mut min, mut cur, mut max) = (0u32, 0u32, 0u32);
            // SAFETY: as above.
            unsafe {
                if GetMonitorBrightness(m.hPhysicalMonitor, &mut min, &mut cur, &mut max) == 0 || max <= min {
                    return None;
                }
                (SetMonitorBrightness(m.hPhysicalMonitor, min + (max - min) * value / 100) != 0).then_some(())
            }
        })?;
        if done.is_empty() { Err("no monitor took the brightness over DDC/CI".into()) } else { Ok(()) }
    }
}

#[cfg(not(windows))]
mod platform {
    use super::Which;

    pub fn get(_: Which) -> Result<u32, String> {
        Err("monitor brightness is not available on this system yet".into())
    }

    pub fn set(_: Which, _: u32) -> Result<(), String> {
        Err("monitor brightness is not available on this system yet".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reads (never sets) the brightness of the attached monitors:
    /// `cargo test -p hubdeck read_brightness -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn read_brightness() {
        for which in [Which::All, Which::One(1), Which::One(2)] {
            println!("{which:?}: {:?}", platform::get(which));
        }
    }

    #[test]
    fn monitor_choice() {
        assert_eq!(Which::from_param("all"), Which::All);
        assert_eq!(Which::from_param("2"), Which::One(2));
        assert_eq!(Which::from_param("0"), Which::All);
    }
}
