//! The deck as a virtual joystick through vJoy (Windows): keys press its buttons, faders, knobs,
//! the dial and rollers move its axes, so any sim binds them in its own control settings
//! (`docs/SIM-PANEL.md`).
//!
//! The user installs vJoy (we never bundle a driver); `vJoyInterface.dll` is loaded from its
//! install folder at the first joystick action, and every path without it ends in a clear
//! message. Devices are acquired lazily and handed back (all buttons up, axes reset) when the
//! deck is released (pause, sleep, exit).
#![cfg_attr(not(windows), allow(dead_code))]

use std::collections::{BTreeSet, HashMap};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use serde_json::Value;

use super::actions::{param_f64, param_str};
use super::exec::{Feedback, InputEvent};

/// Where to get vJoy: the signed fork for Windows 10 and 11.
pub const INSTALL_URL: &str = "https://github.com/BrunnerInnovation/vJoy/releases";

pub fn not_installed() -> String {
    format!("Install vJoy to use the deck as a joystick: {INSTALL_URL}")
}

/// Axes by param value: (param, label, HID usage as vJoy numbers them).
pub const AXES: [(&str, &str, u32); 8] =
    [("x", "X", 0x30), ("y", "Y", 0x31), ("z", "Z", 0x32), ("rx", "RX", 0x33), ("ry", "RY", 0x34), ("rz", "RZ", 0x35), ("slider", "Slider", 0x36), ("dial", "Dial", 0x37)];

/// Hat directions by param value; the index times 45 degrees is the angle (0 = up).
pub const HAT_DIRECTIONS: [(&str, &str); 8] =
    [("up", "Up"), ("upRight", "Up-right"), ("right", "Right"), ("downRight", "Down-right"), ("down", "Down"), ("downLeft", "Down-left"), ("left", "Left"), ("upLeft", "Up-left")];

/// The vJoy calls the joystick logic needs; [`VJoy`] on Windows, a fake in tests.
pub trait Driver {
    /// Take device `dev` (1..=16) for HubDeck, with every control reset.
    fn acquire(&mut self, dev: u32) -> Result<(), String>;
    /// Reset the device's controls and hand it back.
    fn relinquish(&mut self, dev: u32);
    fn buttons(&mut self, dev: u32) -> u32;
    fn set_button(&mut self, dev: u32, n: u8, down: bool) -> Result<(), String>;
    /// (min, max) of an axis, None when the device does not have it.
    fn axis_range(&mut self, dev: u32, usage: u32) -> Option<(i32, i32)>;
    fn set_axis(&mut self, dev: u32, usage: u32, raw: i32) -> Result<(), String>;
    /// Number of (continuous, four-way) hats.
    fn hats(&mut self, dev: u32) -> (u32, u32);
    /// Continuous hat: angle in 1/100 degree, None = centered.
    fn set_hat(&mut self, dev: u32, hat: u8, angle: Option<u32>) -> Result<(), String>;
    /// Four-way hat: 0 up, 1 right, 2 down, 3 left, None = centered.
    fn set_hat4(&mut self, dev: u32, hat: u8, dir: Option<u8>) -> Result<(), String>;
}

/// The devices HubDeck holds and the axis values it set (knobs move from there).
pub struct Joystick<D> {
    driver: D,
    owned: BTreeSet<u32>,
    /// (device, usage) -> 0..=1.
    axes: HashMap<(u32, u32), f64>,
}

impl<D: Driver> Joystick<D> {
    pub fn new(driver: D) -> Self {
        Joystick { driver, owned: BTreeSet::new(), axes: HashMap::new() }
    }

    fn device(&mut self, dev: u32) -> Result<(), String> {
        if !self.owned.contains(&dev) {
            self.driver.acquire(dev)?;
            self.owned.insert(dev);
            self.axes.retain(|(d, _), _| *d != dev);
        }
        Ok(())
    }

    /// Acquire the device and make sure it has button `n`.
    pub fn check_button(&mut self, dev: u32, n: u8) -> Result<(), String> {
        self.device(dev)?;
        let count = self.driver.buttons(dev);
        if n as u32 > count {
            return Err(format!("vJoy device {dev} has {count} buttons, button {n} needs more: raise the number in Configure vJoy"));
        }
        Ok(())
    }

    pub fn button(&mut self, dev: u32, n: u8, down: bool) -> Result<(), String> {
        self.check_button(dev, n)?;
        self.driver.set_button(dev, n, down)
    }

    fn axis_range(&mut self, dev: u32, usage: u32) -> Result<(i32, i32), String> {
        self.device(dev)?;
        let name = AXES.iter().find(|a| a.2 == usage).map(|a| a.1).unwrap_or("?");
        self.driver.axis_range(dev, usage).ok_or_else(|| format!("vJoy device {dev} has no {name} axis: tick it in Configure vJoy"))
    }

    /// Set an axis to `v` (0..=1 of its range); returns the value set.
    pub fn axis_set(&mut self, dev: u32, usage: u32, v: f64) -> Result<f64, String> {
        let (min, max) = self.axis_range(dev, usage)?;
        let v = v.clamp(0.0, 1.0);
        self.driver.set_axis(dev, usage, to_raw(v, min, max))?;
        self.axes.insert((dev, usage), v);
        Ok(v)
    }

    /// Move an axis by `delta` (a share of its range) from the value HubDeck set last; a device
    /// just taken starts centered, as vJoy resets it.
    pub fn axis_by(&mut self, dev: u32, usage: u32, delta: f64) -> Result<f64, String> {
        self.device(dev)?;
        let now = self.axes.get(&(dev, usage)).copied().unwrap_or(0.5);
        self.axis_set(dev, usage, now + delta)
    }

    /// Point hat `hat` (1-based) at `dir` (an index of [`HAT_DIRECTIONS`]), or center it.
    pub fn hat(&mut self, dev: u32, hat: u8, dir: Option<usize>) -> Result<(), String> {
        self.device(dev)?;
        let (cont, four) = self.driver.hats(dev);
        if cont >= hat as u32 {
            self.driver.set_hat(dev, hat, dir.map(|d| d as u32 * 4500))
        } else if four >= hat as u32 {
            match dir {
                None => self.driver.set_hat4(dev, hat, None),
                Some(d) if d % 2 == 0 => self.driver.set_hat4(dev, hat, Some(d as u8 / 2)),
                Some(_) => Err(format!("hat {hat} of vJoy device {dev} has four directions only: make it continuous in Configure vJoy")),
            }
        } else {
            Err(format!("vJoy device {dev} has no hat {hat}: add POV hats in Configure vJoy"))
        }
    }

    /// Hand every device back with its buttons up.
    pub fn release_all(&mut self) {
        for dev in std::mem::take(&mut self.owned) {
            self.driver.relinquish(dev);
        }
        self.axes.clear();
    }
}

/// 0..=1 of an axis range in the axis's own numbers.
pub fn to_raw(v: f64, min: i32, max: i32) -> i32 {
    (min as f64 + v.clamp(0.0, 1.0) * (max as f64 - min as f64)).round() as i32
}

/// The `device` param, 1..=16.
pub fn device(p: &Value) -> u32 {
    param_f64(p, "device", 1.0).round().clamp(1.0, 16.0) as u32
}

fn button_param(p: &Value, key: &str, default: f64) -> u8 {
    param_f64(p, key, default).round().clamp(0.0, 128.0) as u8
}

fn axis_param(p: &Value) -> (u32, &'static str) {
    let name = param_str(p, "axis");
    AXES.iter().find(|a| a.0 == name).map(|a| (a.2, a.1)).unwrap_or((AXES[0].2, AXES[0].1))
}

/// A button tap queued for a turn: (device, button, how many).
pub type Taps = Option<(u32, u8, u8)>;

/// Run a joystick action on `j`: the dial feedback, and button taps for the caller to play out
/// over time (a sim must see each one down, then up).
pub fn run_on<D: Driver>(j: &mut Joystick<D>, action: &str, p: &Value, input: InputEvent) -> Result<(Option<Feedback>, Taps), String> {
    let dev = device(p);
    let (usage, axis) = axis_param(p);
    let caption = || format!("{axis} axis");
    let fb = |v: f64| Some(Feedback { value: Some(v as f32), caption: caption() });
    Ok(match (action, input) {
        ("vjoy.button", InputEvent::Press(down)) => {
            let n = button_param(p, "button", 1.0);
            if n == 0 {
                return if down { Err("set the button number (1-128)".into()) } else { Ok((None, None)) };
            }
            j.button(dev, n, down)?;
            (None, None)
        }
        ("vjoy.button", InputEvent::Turn(d)) => {
            let n = button_param(p, if d > 0 { "clockwise" } else { "counterClockwise" }, 0.0);
            if n == 0 {
                return Ok((None, None));
            }
            j.check_button(dev, n)?;
            (None, Some((dev, n, d.unsigned_abs())))
        }
        ("vjoy.axis", InputEvent::Slide(v)) => (fb(j.axis_set(dev, usage, v as f64 / 100.0)?), None),
        ("vjoy.axis", InputEvent::Turn(d)) => (fb(j.axis_by(dev, usage, d as f64 * param_f64(p, "step", 2.0) / 100.0)?), None),
        ("vjoy.axis", InputEvent::Press(true)) => match param_str(p, "press") {
            "zero" => (fb(j.axis_set(dev, usage, 0.0)?), None),
            "max" => (fb(j.axis_set(dev, usage, 1.0)?), None),
            "nothing" => (None, None),
            _ => (fb(j.axis_set(dev, usage, 0.5)?), None),
        },
        ("vjoy.hat", InputEvent::Press(down)) => {
            let hat = param_f64(p, "hat", 1.0).round().clamp(1.0, 4.0) as u8;
            let dir = HAT_DIRECTIONS.iter().position(|d| d.0 == param_str(p, "direction")).unwrap_or(0);
            j.hat(dev, hat, down.then_some(dir))?;
            (None, None)
        }
        _ => (None, None),
    })
}

#[cfg(windows)]
type Global = Joystick<win::VJoy>;
#[cfg(not(windows))]
type Global = Joystick<NoDriver>;

/// The loaded joystick; None until the first action (or after a failed load).
static JOY: Mutex<Option<Global>> = Mutex::new(None);
/// The last failed load, so a missing vJoy is not searched for on every press.
static LOAD_FAILED: Mutex<Option<(std::time::Instant, String)>> = Mutex::new(None);
const RETRY_LOAD: std::time::Duration = std::time::Duration::from_secs(10);

fn load() -> Result<Global, String> {
    #[cfg(windows)]
    {
        win::VJoy::load().map(Joystick::new)
    }
    #[cfg(not(windows))]
    {
        Err("The joystick actions work on Windows only: they use the vJoy driver.".into())
    }
}

/// Run `f` on the joystick, loading vJoy first if needed.
fn with<R>(f: impl FnOnce(&mut Global) -> Result<R, String>) -> Result<R, String> {
    let mut joy = JOY.lock().unwrap_or_else(|e| e.into_inner());
    if joy.is_none() {
        let mut failed = LOAD_FAILED.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((_, msg)) = failed.as_ref().filter(|(at, _)| at.elapsed() < RETRY_LOAD) {
            return Err(msg.clone());
        }
        match load() {
            Ok(j) => {
                log::info!("vJoy loaded");
                *joy = Some(j);
                *failed = None;
            }
            Err(e) => {
                *failed = Some((std::time::Instant::now(), e.clone()));
                return Err(e);
            }
        }
    }
    f(joy.as_mut().expect("loaded above"))
}

/// Run a joystick action (`vjoy.*`) for one input.
pub fn run(action: &str, p: &Value, input: InputEvent) -> Result<Option<Feedback>, String> {
    // a release when nothing was loaded: the press already explained why
    if input == InputEvent::Press(false) && JOY.lock().unwrap_or_else(|e| e.into_inner()).is_none() {
        return Ok(None);
    }
    let (feedback, taps) = with(|j| run_on(j, action, p, input))?;
    if let Some((dev, n, count)) = taps {
        tap(dev, n, count);
    }
    Ok(feedback)
}

/// Bumped whenever the devices are handed back, so queued taps do not take them again.
static EPOCH: AtomicU64 = AtomicU64::new(0);
static QUEUED: AtomicUsize = AtomicUsize::new(0);
/// Taps waiting at most; a fast spin past this is dropped rather than played for seconds.
const MAX_QUEUED: usize = 12;
const TAP_DOWN: std::time::Duration = std::time::Duration::from_millis(40);
const TAP_GAP: std::time::Duration = std::time::Duration::from_millis(30);

static TAPPER: Mutex<Option<std::sync::mpsc::Sender<(u64, u32, u8)>>> = Mutex::new(None);

/// Press and release a button `count` times on a thread of its own: down long enough for a sim
/// that polls once a frame, then up, then the next.
fn tap(dev: u32, n: u8, count: u8) {
    let mut tx = TAPPER.lock().unwrap_or_else(|e| e.into_inner());
    let sender = tx.get_or_insert_with(|| {
        let (tx, rx) = std::sync::mpsc::channel::<(u64, u32, u8)>();
        let spawned = std::thread::Builder::new().name("vjoy-taps".into()).spawn(move || {
            for (epoch, dev, n) in rx {
                QUEUED.fetch_sub(1, Ordering::SeqCst);
                if epoch != EPOCH.load(Ordering::SeqCst) {
                    continue;
                }
                let down = with(|j| j.button(dev, n, true));
                std::thread::sleep(TAP_DOWN);
                // after a release the device was reset already
                if down.is_ok()
                    && epoch == EPOCH.load(Ordering::SeqCst)
                    && let Err(e) = with(|j| j.button(dev, n, false))
                {
                    log::warn!("vJoy button {n} up: {e}");
                }
                std::thread::sleep(TAP_GAP);
            }
        });
        if let Err(e) = spawned {
            log::warn!("vJoy tap thread: {e}");
        }
        tx
    });
    let epoch = EPOCH.load(Ordering::SeqCst);
    for _ in 0..count {
        if QUEUED.load(Ordering::SeqCst) >= MAX_QUEUED {
            break;
        }
        QUEUED.fetch_add(1, Ordering::SeqCst);
        if sender.send((epoch, dev, n)).is_err() {
            QUEUED.fetch_sub(1, Ordering::SeqCst);
            *tx = None;
            break;
        }
    }
}

/// Hand every vJoy device back (deck paused, computer asleep, app quitting). Loads nothing.
pub fn release_all() {
    EPOCH.fetch_add(1, Ordering::SeqCst);
    if let Some(j) = JOY.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        j.release_all();
    }
}

/// Stand-in driver where vJoy cannot exist; never constructed.
#[cfg(not(windows))]
pub struct NoDriver;

#[cfg(not(windows))]
impl Driver for NoDriver {
    fn acquire(&mut self, _: u32) -> Result<(), String> {
        Err("Windows only".into())
    }
    fn relinquish(&mut self, _: u32) {}
    fn buttons(&mut self, _: u32) -> u32 {
        0
    }
    fn set_button(&mut self, _: u32, _: u8, _: bool) -> Result<(), String> {
        Err("Windows only".into())
    }
    fn axis_range(&mut self, _: u32, _: u32) -> Option<(i32, i32)> {
        None
    }
    fn set_axis(&mut self, _: u32, _: u32, _: i32) -> Result<(), String> {
        Err("Windows only".into())
    }
    fn hats(&mut self, _: u32) -> (u32, u32) {
        (0, 0)
    }
    fn set_hat(&mut self, _: u32, _: u8, _: Option<u32>) -> Result<(), String> {
        Err("Windows only".into())
    }
    fn set_hat4(&mut self, _: u32, _: u8, _: Option<u8>) -> Result<(), String> {
        Err("Windows only".into())
    }
}

/// The value of `DllX64Location` (or `DllX86Location`) in `reg query` output.
pub fn dll_dir_from_reg(output: &str, value: &str) -> Option<String> {
    output.lines().map(str::trim).filter(|l| l.starts_with(value)).find_map(|l| l.split_once("REG_SZ").map(|(_, dir)| dir.trim().to_string())).filter(|d| !d.is_empty())
}

#[cfg(windows)]
mod win {
    use std::path::PathBuf;

    use libloading::Library;

    use super::{Driver, dll_dir_from_reg, not_installed};

    const DLL: &str = "vJoyInterface.dll";
    const ARCH: &str = if cfg!(target_pointer_width = "64") { "x64" } else { "x86" };

    // vJoyInterface.h: every call is __cdecl (the only convention on x64)
    type NoArgs = unsafe extern "C" fn() -> i32;
    type ById = unsafe extern "C" fn(u32) -> i32;
    type Relinquish = unsafe extern "C" fn(u32);
    type SetBtn = unsafe extern "C" fn(i32, u32, u8) -> i32;
    type SetAxis = unsafe extern "C" fn(i32, u32, u32) -> i32;
    type AxisExist = unsafe extern "C" fn(u32, u32) -> i32;
    type AxisLimit = unsafe extern "C" fn(u32, u32, *mut i32) -> i32;
    type SetDiscPov = unsafe extern "C" fn(i32, u32, u8) -> i32;
    type SetContPov = unsafe extern "C" fn(u32, u32, u8) -> i32;

    // VjdStat
    const OWN: i32 = 0;
    const FREE: i32 = 1;
    const BUSY: i32 = 2;
    const MISS: i32 = 3;

    pub struct VJoy {
        status: ById,
        acquire: ById,
        relinquish: Relinquish,
        reset: ById,
        set_btn: SetBtn,
        set_axis: SetAxis,
        axis_exist: AxisExist,
        axis_min: AxisLimit,
        axis_max: AxisLimit,
        buttons: ById,
        disc_povs: ById,
        cont_povs: ById,
        set_disc_pov: SetDiscPov,
        set_cont_pov: SetContPov,
        /// Keeps the function pointers above valid.
        _lib: Library,
    }

    /// Folder of the DLL as the vJoy installer records it in its uninstall entry.
    fn registry_dir() -> Option<PathBuf> {
        use std::os::windows::process::CommandExt;
        let value = format!("Dll{}Location", ARCH.to_uppercase());
        let out = std::process::Command::new("reg")
            .args(["query", r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall", "/s", "/v", &value])
            .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
            .output()
            .ok()?;
        dll_dir_from_reg(&String::from_utf8_lossy(&out.stdout), &value).map(PathBuf::from)
    }

    fn candidates() -> Vec<PathBuf> {
        let mut list: Vec<PathBuf> = registry_dir().into_iter().map(|d| d.join(DLL)).collect();
        for var in ["ProgramW6432", "ProgramFiles"] {
            if let Ok(pf) = std::env::var(var) {
                list.push(PathBuf::from(pf).join("vJoy").join(ARCH).join(DLL));
            }
        }
        list.dedup();
        list
    }

    impl VJoy {
        pub fn load() -> Result<VJoy, String> {
            let Some(path) = candidates().into_iter().find(|p| p.is_file()) else { return Err(not_installed()) };
            // SAFETY: vJoy's own DLL from its install folder; it runs no code we depend on at load
            let lib = unsafe { Library::new(&path) }.map_err(|e| format!("cannot load {}: {e}. {}", path.display(), not_installed()))?;
            macro_rules! sym {
                ($name:literal) => {
                    // SAFETY: the types follow vJoyInterface.h
                    *unsafe { lib.get(concat!($name, "\0").as_bytes()) }.map_err(|e| format!("{} lacks {}: {e}; reinstall vJoy", path.display(), $name))?
                };
            }
            let enabled: NoArgs = sym!("vJoyEnabled");
            let vjoy = VJoy {
                status: sym!("GetVJDStatus"),
                acquire: sym!("AcquireVJD"),
                relinquish: sym!("RelinquishVJD"),
                reset: sym!("ResetVJD"),
                set_btn: sym!("SetBtn"),
                set_axis: sym!("SetAxis"),
                axis_exist: sym!("GetVJDAxisExist"),
                axis_min: sym!("GetVJDAxisMin"),
                axis_max: sym!("GetVJDAxisMax"),
                buttons: sym!("GetVJDButtonNumber"),
                disc_povs: sym!("GetVJDDiscPovNumber"),
                cont_povs: sym!("GetVJDContPovNumber"),
                set_disc_pov: sym!("SetDiscPov"),
                set_cont_pov: sym!("SetContPov"),
                _lib: lib,
            };
            // SAFETY: a plain query of the loaded DLL
            if unsafe { enabled() } == 0 {
                return Err("vJoy is installed but switched off: open Configure vJoy and tick Enable vJoy".into());
            }
            log::info!("vJoy from {}", path.display());
            Ok(vjoy)
        }
    }

    fn ok(result: i32, what: impl FnOnce() -> String) -> Result<(), String> {
        if result != 0 { Ok(()) } else { Err(what()) }
    }

    // SAFETY (every unsafe call below): functions of the loaded vJoy DLL with the types from its header
    impl Driver for VJoy {
        fn acquire(&mut self, dev: u32) -> Result<(), String> {
            match unsafe { (self.status)(dev) } {
                OWN => {}
                FREE => ok(unsafe { (self.acquire)(dev) }, || format!("vJoy device {dev} could not be taken"))?,
                BUSY => return Err(format!("vJoy device {dev} is used by another program: close it or pick another device")),
                MISS => return Err(format!("vJoy device {dev} is not set up: add it in Configure vJoy")),
                _ => return Err(format!("vJoy device {dev} is in an unknown state: restart vJoy")),
            }
            unsafe { (self.reset)(dev) };
            Ok(())
        }

        fn relinquish(&mut self, dev: u32) {
            unsafe {
                (self.reset)(dev);
                (self.relinquish)(dev);
            }
        }

        fn buttons(&mut self, dev: u32) -> u32 {
            unsafe { (self.buttons)(dev) }.max(0) as u32
        }

        fn set_button(&mut self, dev: u32, n: u8, down: bool) -> Result<(), String> {
            ok(unsafe { (self.set_btn)(down as i32, dev, n) }, || format!("vJoy did not take button {n}"))
        }

        fn axis_range(&mut self, dev: u32, usage: u32) -> Option<(i32, i32)> {
            if unsafe { (self.axis_exist)(dev, usage) } == 0 {
                return None;
            }
            let (mut min, mut max) = (0i32, 0i32);
            let got = unsafe { (self.axis_min)(dev, usage, &mut min) != 0 && (self.axis_max)(dev, usage, &mut max) != 0 };
            // vJoy's axes run 0..32767 when it cannot tell
            Some(if got && max > min { (min, max) } else { (0, 0x7fff) })
        }

        fn set_axis(&mut self, dev: u32, usage: u32, raw: i32) -> Result<(), String> {
            ok(unsafe { (self.set_axis)(raw, dev, usage) }, || "vJoy did not take the axis value".into())
        }

        fn hats(&mut self, dev: u32) -> (u32, u32) {
            unsafe { ((self.cont_povs)(dev).max(0) as u32, (self.disc_povs)(dev).max(0) as u32) }
        }

        fn set_hat(&mut self, dev: u32, hat: u8, angle: Option<u32>) -> Result<(), String> {
            ok(unsafe { (self.set_cont_pov)(angle.unwrap_or(u32::MAX), dev, hat) }, || format!("vJoy did not take hat {hat}"))
        }

        fn set_hat4(&mut self, dev: u32, hat: u8, dir: Option<u8>) -> Result<(), String> {
            ok(unsafe { (self.set_disc_pov)(dir.map(i32::from).unwrap_or(-1), dev, hat) }, || format!("vJoy did not take hat {hat}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Records calls; device 1 has 8 buttons, X and Z axes (0..32767) and one four-way hat.
    #[derive(Default)]
    struct Fake {
        log: Vec<String>,
        busy: bool,
    }

    impl Driver for Fake {
        fn acquire(&mut self, dev: u32) -> Result<(), String> {
            if self.busy {
                return Err(format!("vJoy device {dev} is used by another program"));
            }
            self.log.push(format!("acquire {dev}"));
            Ok(())
        }
        fn relinquish(&mut self, dev: u32) {
            self.log.push(format!("release {dev}"));
        }
        fn buttons(&mut self, _: u32) -> u32 {
            8
        }
        fn set_button(&mut self, dev: u32, n: u8, down: bool) -> Result<(), String> {
            self.log.push(format!("btn {dev}/{n} {}", if down { "down" } else { "up" }));
            Ok(())
        }
        fn axis_range(&mut self, _: u32, usage: u32) -> Option<(i32, i32)> {
            [0x30, 0x32].contains(&usage).then_some((0, 32767))
        }
        fn set_axis(&mut self, dev: u32, usage: u32, raw: i32) -> Result<(), String> {
            self.log.push(format!("axis {dev}/{usage:x} {raw}"));
            Ok(())
        }
        fn hats(&mut self, _: u32) -> (u32, u32) {
            (0, 1)
        }
        fn set_hat(&mut self, _: u32, _: u8, _: Option<u32>) -> Result<(), String> {
            unreachable!("no continuous hats")
        }
        fn set_hat4(&mut self, dev: u32, hat: u8, dir: Option<u8>) -> Result<(), String> {
            self.log.push(format!("hat {dev}/{hat} {dir:?}"));
            Ok(())
        }
    }

    fn joy() -> Joystick<Fake> {
        Joystick::new(Fake::default())
    }

    #[test]
    fn buttons_go_down_and_up_and_the_device_is_taken_once() {
        let mut j = joy();
        let p = json!({"device": 2, "button": 5});
        run_on(&mut j, "vjoy.button", &p, InputEvent::Press(true)).unwrap();
        run_on(&mut j, "vjoy.button", &p, InputEvent::Press(false)).unwrap();
        assert_eq!(j.driver.log, ["acquire 2", "btn 2/5 down", "btn 2/5 up"]);
        assert!(run_on(&mut j, "vjoy.button", &json!({"button": 9}), InputEvent::Press(true)).unwrap_err().contains("8 buttons"));
        assert!(run_on(&mut j, "vjoy.button", &json!({"button": 0}), InputEvent::Press(true)).is_err());
        j.release_all();
        assert_eq!(j.driver.log.iter().filter(|l| l.starts_with("release")).count(), 2, "devices 2 and 1");
        assert!(j.owned.is_empty());
    }

    #[test]
    fn a_turn_queues_taps_of_its_direction_button() {
        let mut j = joy();
        let p = json!({"clockwise": 3, "counterClockwise": 4});
        assert_eq!(run_on(&mut j, "vjoy.button", &p, InputEvent::Turn(2)).unwrap().1, Some((1, 3, 2)));
        assert_eq!(run_on(&mut j, "vjoy.button", &p, InputEvent::Turn(-1)).unwrap().1, Some((1, 4, 1)));
        assert_eq!(run_on(&mut j, "vjoy.button", &json!({}), InputEvent::Turn(1)).unwrap().1, None, "no button for that way");
    }

    #[test]
    fn faders_set_axes_and_knobs_move_them_from_there() {
        let mut j = joy();
        let p = json!({"axis": "z", "step": 10, "press": "center"});
        let (fb, _) = run_on(&mut j, "vjoy.axis", &p, InputEvent::Slide(100)).unwrap();
        assert_eq!(fb.unwrap().caption, "Z axis");
        assert_eq!(j.driver.log.last().unwrap(), "axis 1/32 32767");
        let (fb, _) = run_on(&mut j, "vjoy.axis", &p, InputEvent::Turn(-3)).unwrap();
        assert!((fb.unwrap().value.unwrap() - 0.7).abs() < 1e-4);
        run_on(&mut j, "vjoy.axis", &p, InputEvent::Press(true)).unwrap();
        assert_eq!(j.driver.log.last().unwrap(), "axis 1/32 16384");
        run_on(&mut j, "vjoy.axis", &json!({"axis": "z", "press": "zero"}), InputEvent::Press(true)).unwrap();
        assert_eq!(j.driver.log.last().unwrap(), "axis 1/32 0");
        // the release of a press does nothing; a fresh device starts centered
        assert_eq!(run_on(&mut j, "vjoy.axis", &p, InputEvent::Press(false)).unwrap().0, None);
        let (fb, _) = run_on(&mut j, "vjoy.axis", &json!({"device": 3, "step": 5}), InputEvent::Turn(1)).unwrap();
        assert!((fb.unwrap().value.unwrap() - 0.55).abs() < 1e-4, "X of device 3 from the middle");
        assert!(run_on(&mut j, "vjoy.axis", &json!({"axis": "rz"}), InputEvent::Slide(5)).unwrap_err().contains("no RZ axis"));
    }

    #[test]
    fn hats_point_and_center_and_explain_diagonals() {
        let mut j = joy();
        run_on(&mut j, "vjoy.hat", &json!({"direction": "left"}), InputEvent::Press(true)).unwrap();
        run_on(&mut j, "vjoy.hat", &json!({"direction": "left"}), InputEvent::Press(false)).unwrap();
        assert_eq!(&j.driver.log[1..], ["hat 1/1 Some(3)", "hat 1/1 None"]);
        assert!(run_on(&mut j, "vjoy.hat", &json!({"direction": "upLeft"}), InputEvent::Press(true)).unwrap_err().contains("four directions"));
        assert!(run_on(&mut j, "vjoy.hat", &json!({"hat": 2}), InputEvent::Press(true)).unwrap_err().contains("no hat 2"));
    }

    #[test]
    fn a_busy_device_explains() {
        let mut j = Joystick::new(Fake { busy: true, ..Fake::default() });
        assert!(run_on(&mut j, "vjoy.button", &json!({"button": 1}), InputEvent::Press(true)).unwrap_err().contains("another program"));
    }

    #[test]
    fn params_and_ranges() {
        assert_eq!(device(&json!({"device": 40})), 16);
        assert_eq!(device(&json!({})), 1);
        assert_eq!(to_raw(0.5, 1, 32768), 16385);
        assert_eq!(to_raw(2.0, 0, 100), 100);
        let reg = "\r\nHKEY_LOCAL_MACHINE\\SOFTWARE\\...\\{8E31F76F-74C3-47F1-9550-E041EEDC5FBB}_is1\r\n    DllX64Location    REG_SZ    C:\\Program Files\\vJoy\\x64\r\n\r\nEnd of search: 1 match(es) found.\r\n";
        assert_eq!(dll_dir_from_reg(reg, "DllX64Location").as_deref(), Some("C:\\Program Files\\vJoy\\x64"));
        assert_eq!(dll_dir_from_reg("End of search: 0 match(es) found.", "DllX64Location"), None);
        assert!(not_installed().contains(INSTALL_URL));
    }
}
