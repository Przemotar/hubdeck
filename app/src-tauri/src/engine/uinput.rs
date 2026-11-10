//! Keystrokes, typing and scrolling through a `/dev/uinput` virtual keyboard: a real input
//! device, so it reaches Wayland windows and games the way a physical keyboard does (enigo's
//! X11 backend only reaches X11 and XWayland windows).
//!
//! The device is created on first use and lives until the process exits; its key codes are the
//! Linux input event codes. Text is typed on the US layout.

use std::ffi::CString;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

const EV_SYN: u16 = 0;
const EV_KEY: u16 = 1;
const EV_REL: u16 = 2;
const SYN_REPORT: u16 = 0;
const REL_HWHEEL: u16 = 6;
const REL_WHEEL: u16 = 8;

// /usr/include/linux/uinput.h
const UI_SET_EVBIT: libc::c_ulong = 0x4004_5564;
const UI_SET_KEYBIT: libc::c_ulong = 0x4004_5565;
const UI_SET_RELBIT: libc::c_ulong = 0x4004_5566;
const UI_DEV_SETUP: libc::c_ulong = 0x405c_5503;
const UI_DEV_CREATE: libc::c_ulong = 0x5501;
const UI_DEV_DESTROY: libc::c_ulong = 0x5502;

const KEY_DELAY: Duration = Duration::from_millis(8);
const ENTER_DELAY: Duration = Duration::from_millis(150);

#[repr(C)]
struct InputEvent {
    time: libc::timeval,
    kind: u16,
    code: u16,
    value: i32,
}

#[repr(C)]
struct InputId {
    bustype: u16,
    vendor: u16,
    product: u16,
    version: u16,
}

#[repr(C)]
struct UinputSetup {
    id: InputId,
    name: [u8; 80],
    ff_effects_max: u32,
}

struct Device {
    fd: libc::c_int,
}

impl Device {
    fn open() -> Result<Self, String> {
        let path = CString::new("/dev/uinput").expect("no NUL in the path");
        // SAFETY: plain open(2) with a NUL-terminated path.
        let fd = unsafe { libc::open(path.as_ptr(), libc::O_RDWR | libc::O_CLOEXEC) };
        if fd < 0 {
            return Err(format!("cannot open /dev/uinput: {} (install packaging/linux/99-masterhub.rules, which grants /dev/uinput to the user at the desk)", std::io::Error::last_os_error()));
        }
        let device = Device { fd };
        if let Err(e) = device.setup() {
            // SAFETY: the descriptor is ours and not used after this.
            unsafe { libc::close(fd) };
            return Err(e);
        }
        Ok(device)
    }

    fn setup(&self) -> Result<(), String> {
        self.ioctl(UI_SET_EVBIT, u64::from(EV_KEY))?;
        self.ioctl(UI_SET_EVBIT, u64::from(EV_REL))?;
        for code in KEY_CODES.iter().map(|(_, code)| *code) {
            self.ioctl(UI_SET_KEYBIT, u64::from(code))?;
        }
        self.ioctl(UI_SET_RELBIT, u64::from(REL_WHEEL))?;
        self.ioctl(UI_SET_RELBIT, u64::from(REL_HWHEEL))?;
        let mut setup = UinputSetup { id: InputId { bustype: 0x06, vendor: 0x766d, product: 1, version: 1 }, name: [0; 80], ff_effects_max: 0 };
        let name = b"hubdeck keyboard";
        setup.name[..name.len()].copy_from_slice(name);
        // SAFETY: UI_DEV_SETUP takes a pointer to a uinput_setup that outlives the call.
        let rc = unsafe { libc::ioctl(self.fd, UI_DEV_SETUP, &setup) };
        if rc < 0 {
            return Err(format!("uinput setup failed: {}", std::io::Error::last_os_error()));
        }
        self.ioctl(UI_DEV_CREATE, 0)
    }

    fn ioctl(&self, request: libc::c_ulong, arg: u64) -> Result<(), String> {
        // SAFETY: the request and argument are the ones uinput defines.
        let rc = unsafe { libc::ioctl(self.fd, request, arg) };
        if rc < 0 {
            return Err(format!("uinput ioctl {request:#x} failed: {}", std::io::Error::last_os_error()));
        }
        Ok(())
    }

    fn write(&self, events: &[(u16, u16, i32)]) {
        let mut buffer = Vec::with_capacity((events.len() + 1) * std::mem::size_of::<InputEvent>());
        for &(kind, code, value) in events {
            let event = InputEvent { time: libc::timeval { tv_sec: 0, tv_usec: 0 }, kind, code, value };
            // SAFETY: InputEvent is plain old data; the bytes are copied into the buffer.
            let bytes = unsafe { std::slice::from_raw_parts((&event as *const InputEvent).cast::<u8>(), std::mem::size_of::<InputEvent>()) };
            buffer.extend_from_slice(bytes);
        }
        let syn = InputEvent { time: libc::timeval { tv_sec: 0, tv_usec: 0 }, kind: EV_SYN, code: SYN_REPORT, value: 0 };
        // SAFETY: same as above, for the closing SYN_REPORT.
        let bytes = unsafe { std::slice::from_raw_parts((&syn as *const InputEvent).cast::<u8>(), std::mem::size_of::<InputEvent>()) };
        buffer.extend_from_slice(bytes);
        // SAFETY: the buffer is valid for its whole length; a short write is dropped like a lost key.
        unsafe { libc::write(self.fd, buffer.as_ptr().cast(), buffer.len()) };
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        let _ = self.ioctl(UI_DEV_DESTROY, 0);
        // SAFETY: the descriptor is ours and closed once.
        unsafe { libc::close(self.fd) };
    }
}

static DEVICE: OnceLock<Mutex<Option<Device>>> = OnceLock::new();

/// Run `f` with the virtual keyboard, creating it on first use.
fn with_device<T>(f: impl FnOnce(&Device) -> T) -> Result<T, String> {
    let cell = DEVICE.get_or_init(|| Mutex::new(None));
    let mut guard = cell.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        *guard = Some(Device::open()?);
    }
    Ok(f(guard.as_ref().expect("just created")))
}

/// Open `/dev/uinput` and create the virtual keyboard, as the first key would. The doctor calls
/// this to check the permission and the kernel side without sending a key anywhere.
pub fn probe() -> Result<(), String> {
    with_device(|_| ())
}

/// Check the key tables without touching the input stack: every common hotkey parses and every
/// printable character on the US layout has a key.
pub fn self_check() -> Result<usize, String> {
    let mut checked = 0;
    for combo in ["Ctrl+C", "Ctrl+Shift+M", "Alt+F4", "Super+L", "Ctrl+Alt+Delete", "Shift+Tab"] {
        combo_codes(combo)?;
        checked += 1;
    }
    for ch in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 ~!@#$%^&*()_+{}|:\"<>?-=[]\\;',./\n\t".chars() {
        if char_keys(ch).is_empty() {
            return Err(format!("no key for {ch:?} on the US layout"));
        }
        checked += 1;
    }
    Ok(checked)
}

/// Press and release the keys of `combo`, e.g. `Ctrl+Shift+M` (the names the hotkey field uses).
pub fn press(combo: &str) -> Result<(), String> {
    let codes = combo_codes(combo)?;
    with_device(|d| {
        d.write(&codes.iter().map(|c| (EV_KEY, *c, 1)).collect::<Vec<_>>());
        d.write(&codes.iter().rev().map(|c| (EV_KEY, *c, 0)).collect::<Vec<_>>());
    })
}

/// Press and release `combo`, holding it `hold` long (games read the key state between frames).
pub fn press_hold(combo: &str, hold: Duration) -> Result<(), String> {
    let codes = combo_codes(combo)?;
    with_device(|d| {
        d.write(&codes.iter().map(|c| (EV_KEY, *c, 1)).collect::<Vec<_>>());
        std::thread::sleep(hold);
        d.write(&codes.iter().rev().map(|c| (EV_KEY, *c, 0)).collect::<Vec<_>>());
    })
}

/// Press and release one key by name (`PlayPause`, `VolumeUp`, `Escape`).
pub fn tap(name: &str) -> Result<(), String> {
    let code = code(name).ok_or_else(|| format!("no key for {name:?} on Linux"))?;
    with_device(|d| {
        d.write(&[(EV_KEY, code, 1)]);
        d.write(&[(EV_KEY, code, 0)]);
    })
}

/// Type `text` on the US layout, a key at a time: games see one key per frame, not a burst.
pub fn type_text(text: &str) -> Result<(), String> {
    with_device(|d| {
        for ch in text.chars() {
            let keys = char_keys(ch);
            if keys.is_empty() {
                log::warn!("uinput: no key for {ch:?} on the US layout");
                continue;
            }
            let pause = if ch == '\n' { ENTER_DELAY } else { KEY_DELAY };
            std::thread::sleep(pause);
            let mut events = Vec::with_capacity(4);
            for (code, shifted) in keys {
                if shifted {
                    events.push((EV_KEY, SHIFT, 1));
                }
                events.push((EV_KEY, code, 1));
                events.push((EV_KEY, code, 0));
                if shifted {
                    events.push((EV_KEY, SHIFT, 0));
                }
            }
            d.write(&events);
            std::thread::sleep(pause);
        }
    })
}

/// Scroll `amount` detents; positive is down (vertical) or right (horizontal).
pub fn scroll(amount: i32, horizontal: bool) -> Result<(), String> {
    let code = if horizontal { REL_HWHEEL } else { REL_WHEEL };
    with_device(|d| d.write(&[(EV_REL, code, amount)]))
}

fn combo_codes(combo: &str) -> Result<Vec<u16>, String> {
    let mut codes = Vec::new();
    for part in combo.split('+').map(str::trim).filter(|p| !p.is_empty()) {
        let code = match part.to_lowercase().as_str() {
            "ctrl" | "control" => KEY_LEFTCTRL,
            "shift" => SHIFT,
            "alt" | "option" => KEY_LEFTALT,
            "win" | "cmd" | "command" | "meta" | "super" => KEY_LEFTMETA,
            other => code(other).ok_or_else(|| format!("no key for {other:?} on Linux"))?,
        };
        codes.push(code);
    }
    if codes.is_empty() {
        return Err("empty hotkey".into());
    }
    Ok(codes)
}

fn char_keys(ch: char) -> Vec<(u16, bool)> {
    let shifted = |name: &str| vec![(code(name).expect("mapped"), true)];
    let plain = |name: &str| vec![(code(name).expect("mapped"), false)];
    match ch {
        '~' => shifted("`"),
        '!' => shifted("1"),
        '@' => shifted("2"),
        '#' => shifted("3"),
        '$' => shifted("4"),
        '%' => shifted("5"),
        '^' => shifted("6"),
        '&' => shifted("7"),
        '*' => shifted("8"),
        '(' => shifted("9"),
        ')' => shifted("0"),
        '_' => shifted("-"),
        '+' => shifted("="),
        '{' => shifted("["),
        '}' => shifted("]"),
        '|' => shifted("\\"),
        ':' => shifted(";"),
        '"' => shifted("'"),
        '<' => shifted(","),
        '>' => shifted("."),
        '?' => shifted("/"),
        '-' => plain("-"),
        '=' => plain("="),
        '[' => plain("["),
        ']' => plain("]"),
        '\\' => plain("\\"),
        ';' => plain(";"),
        '\'' => plain("'"),
        '`' => plain("`"),
        ',' => plain(","),
        '.' => plain("."),
        '/' => plain("/"),
        ' ' => plain("space"),
        '\n' => plain("enter"),
        '\t' => plain("tab"),
        c if c.is_ascii_alphabetic() => {
            let name = c.to_ascii_lowercase().to_string();
            vec![(code(&name).expect("letters are mapped"), c.is_ascii_uppercase())]
        }
        c if c.is_ascii_digit() => plain(&c.to_string()),
        _ => Vec::new(),
    }
}

const SHIFT: u16 = 42;
const KEY_LEFTCTRL: u16 = 29;
const KEY_LEFTALT: u16 = 56;
const KEY_LEFTMETA: u16 = 125;

/// The Linux input event code of a key name (the names the hotkey field and the US layout use).
fn code(name: &str) -> Option<u16> {
    KEY_CODES.iter().find(|(n, _)| *n == name).map(|(_, code)| *code)
}

/// Every key the virtual keyboard knows, by name.
const KEY_CODES: &[(&str, u16)] = &[
    ("escape", 1),
    ("esc", 1),
    ("1", 2),
    ("2", 3),
    ("3", 4),
    ("4", 5),
    ("5", 6),
    ("6", 7),
    ("7", 8),
    ("8", 9),
    ("9", 10),
    ("0", 11),
    ("-", 12),
    ("=", 13),
    ("backspace", 14),
    ("tab", 15),
    ("q", 16),
    ("w", 17),
    ("e", 18),
    ("r", 19),
    ("t", 20),
    ("y", 21),
    ("u", 22),
    ("i", 23),
    ("o", 24),
    ("p", 25),
    ("[", 26),
    ("]", 27),
    ("enter", 28),
    ("return", 28),
    ("a", 30),
    ("s", 31),
    ("d", 32),
    ("f", 33),
    ("g", 34),
    ("h", 35),
    ("j", 36),
    ("k", 37),
    ("l", 38),
    (";", 39),
    ("'", 40),
    ("`", 41),
    ("\\", 43),
    ("z", 44),
    ("x", 45),
    ("c", 46),
    ("v", 47),
    ("b", 48),
    ("n", 49),
    ("m", 50),
    (",", 51),
    (".", 52),
    ("/", 53),
    ("space", 57),
    ("capslock", 58),
    ("f1", 59),
    ("f2", 60),
    ("f3", 61),
    ("f4", 62),
    ("f5", 63),
    ("f6", 64),
    ("f7", 65),
    ("f8", 66),
    ("f9", 67),
    ("f10", 68),
    ("numlock", 69),
    ("scrolllock", 70),
    ("num7", 71),
    ("num8", 72),
    ("num9", 73),
    ("numsubtract", 74),
    ("num4", 75),
    ("num5", 76),
    ("num6", 77),
    ("numadd", 78),
    ("num1", 79),
    ("num2", 80),
    ("num3", 81),
    ("num0", 82),
    ("numdecimal", 83),
    ("f11", 87),
    ("f12", 88),
    ("printscreen", 99),
    ("home", 102),
    ("up", 103),
    ("pageup", 104),
    ("left", 105),
    ("right", 106),
    ("end", 107),
    ("down", 108),
    ("pagedown", 109),
    ("insert", 110),
    ("delete", 111),
    ("mute", 113),
    ("volumemute", 113),
    ("volumedown", 114),
    ("volumeup", 115),
    ("pause", 119),
    ("break", 119),
    ("playpause", 164),
    ("mediaplaypause", 164),
    ("nexttrack", 163),
    ("medianext", 163),
    ("prevtrack", 165),
    ("mediaprev", 165),
    ("stop", 166),
    ("f13", 183),
    ("f14", 184),
    ("f15", 185),
    ("f16", 186),
    ("f17", 187),
    ("f18", 188),
    ("f19", 189),
    ("f20", 190),
    ("f21", 191),
    ("f22", 192),
    ("f23", 193),
    ("f24", 194),
];
