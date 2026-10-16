//! System and per-app volume. Windows: WASAPI (endpoint volume and audio sessions).
//! macOS: system volume through Core Audio; per-app volume through process taps (macOS 14.2+).

#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    System,
    /// The default recording device (microphone), for the whole system.
    Mic,
    /// Executable name, e.g. `spotify.exe` (case-insensitive, `.exe` optional).
    App(String),
    /// Every app playing sound except these (the apps other controls of the profile set): one
    /// knob or fader for "everything else", as in deej.
    Others(Vec<String>),
}

/// The end of the error for an app that has no sound open now (WASAPI lists only apps with an
/// audio session): the dial says so quietly instead of a red banner (owner, 2026-10-14).
pub const SILENT: &str = "is not playing any sound right now";

/// The programs (exe names) that are recording from the default microphone right now.
pub fn mic_users() -> Vec<String> {
    platform::mic_users()
}

/// The apps sending sound out right now, as the window's app lists name them (`Spotify.app` on a
/// Mac); empty where the system does not say (Windows: not yet).
pub fn playing() -> Vec<String> {
    #[cfg(target_os = "macos")]
    {
        unsafe extern "C" {
            fn mh_audio_playing() -> *mut std::ffi::c_char;
            fn mh_free(p: *mut std::ffi::c_char);
        }
        // SAFETY: a malloc'd, NUL-ended string from the helper, copied, then freed by it
        unsafe {
            let p = mh_audio_playing();
            if p.is_null() {
                return Vec::new();
            }
            let text = std::ffi::CStr::from_ptr(p).to_string_lossy().to_string();
            mh_free(p);
            text.lines().filter(|l| !l.is_empty()).map(str::to_string).collect()
        }
    }
    #[cfg(not(target_os = "macos"))]
    Vec::new()
}

/// Call once on the thread that will use this module (COM on Windows).
/// HubDeck quits: macOS apps whose volume it set (process taps) play straight again.
#[cfg(target_os = "macos")]
pub fn release() {
    platform::release();
}

pub fn init_thread() {
    platform::init_thread();
}

/// Current volume 0.0..=1.0.
pub fn get(target: &Target) -> Result<f32, String> {
    platform::get(target)
}

pub fn set(target: &Target, level: f32) -> Result<(), String> {
    platform::set(target, level.clamp(0.0, 1.0))
}

/// Toggle mute; returns the new state.
pub fn toggle_mute(target: &Target) -> Result<bool, String> {
    platform::toggle_mute(target)
}

/// Whether the target is muted now (stateful keys show it).
pub fn is_muted(target: &Target) -> Result<bool, String> {
    platform::is_muted(target)
}

/// Mute or unmute (push-to-talk / push-to-mute); nothing happens when it is so already.
pub fn set_mute(target: &Target, muted: bool) -> Result<(), String> {
    if platform::is_muted(target)? != muted {
        platform::toggle_mute(target)?;
    }
    Ok(())
}

/// Whether `with_popup` can show the system's volume window here.
pub const POPUP_AVAILABLE: bool = cfg!(windows);

/// Set the system volume and show the Windows volume popup. Windows has no API for the popup;
/// only the volume keys show it. So a worker thread sets the volume a step short of `level`,
/// presses volume up (or down, `up` = false), and sets `level` exactly once the key went through
/// (the popup follows the change). Calls that come quicker than that are merged: the newest wins.
pub fn with_popup(level: f32, up: bool) {
    platform::with_popup(level.clamp(0.0, 1.0), up);
}

#[cfg(windows)]
mod platform {
    use std::sync::OnceLock;
    use std::time::Duration;

    use crossbeam_channel::{Receiver, Sender};
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};

    use super::Target;

    /// How long the shell takes to handle a volume key before the exact level is set.
    const KEY_SETTLES: Duration = Duration::from_millis(50);
    /// What one volume key press changes (Windows: 2 of 100).
    const KEY_STEP: f32 = 0.02;

    static POPUP: OnceLock<Sender<(f32, bool)>> = OnceLock::new();

    pub fn with_popup(level: f32, up: bool) {
        let tx = POPUP.get_or_init(|| {
            let (tx, rx) = crossbeam_channel::unbounded();
            if let Err(e) = std::thread::Builder::new().name("volume-popup".into()).spawn(move || popup_worker(rx)) {
                log::warn!("volume popup thread: {e}");
            }
            tx
        });
        let _ = tx.send((level, up));
    }

    fn popup_worker(rx: Receiver<(f32, bool)>) {
        init_thread();
        let mut enigo = match Enigo::new(&Settings::default()) {
            Ok(e) => e,
            Err(e) => {
                log::warn!("volume popup unavailable: {e}");
                // still set the volume, just without the popup
                while let Ok((level, _)) = rx.recv() {
                    let _ = set(&Target::System, level);
                }
                return;
            }
        };
        let newest = |mut req: (f32, bool)| {
            while let Ok(newer) = rx.try_recv() {
                req = newer;
            }
            req
        };
        while let Ok(first) = rx.recv() {
            let mut req = newest(first);
            loop {
                let (level, up) = req;
                let short = if up { level - KEY_STEP } else { level + KEY_STEP };
                if let Err(e) = set(&Target::System, short.clamp(0.0, 1.0)) {
                    log::warn!("volume: {e}");
                }
                let key = if up { Key::VolumeUp } else { Key::VolumeDown };
                if let Err(e) = enigo.key(key, Direction::Click) {
                    log::warn!("volume key: {e}");
                }
                std::thread::sleep(KEY_SETTLES);
                match rx.try_recv() {
                    // still turning: another round keeps the popup up; the exact level comes last
                    Ok(next) => req = newest(next),
                    Err(_) => {
                        if let Err(e) = set(&Target::System, level) {
                            log::warn!("volume: {e}");
                        }
                        break;
                    }
                }
            }
        }
    }

    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
    use windows::Win32::Media::Audio::{
        AudioSessionStateActive, EDataFlow, IAudioSessionControl2, IAudioSessionManager2, IMMDevice, IMMDeviceEnumerator, ISimpleAudioVolume, MMDeviceEnumerator, eCapture, eConsole, eRender,
    };
    use windows::Win32::System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx};
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW};
    use windows::core::{Interface, PWSTR};

    pub fn init_thread() {
        // SAFETY: plain COM initialisation for this thread; a second call just returns S_FALSE.
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
    }

    fn err(e: windows::core::Error) -> String {
        e.message().to_string()
    }

    fn device_for(flow: EDataFlow) -> Result<IMMDevice, String> {
        // SAFETY: COM calls on an initialised thread; results are checked.
        unsafe {
            let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(err)?;
            enumerator.GetDefaultAudioEndpoint(flow, eConsole).map_err(|e| if flow == eCapture { "no microphone found".to_string() } else { err(e) })
        }
    }

    fn device() -> Result<IMMDevice, String> {
        device_for(eRender)
    }

    pub fn mic_users() -> Vec<String> {
        // SAFETY: COM calls on an initialised thread; every result is checked.
        unsafe {
            let Ok(device) = device_for(eCapture) else { return Vec::new() };
            let Ok(manager) = device.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) else { return Vec::new() };
            let Ok(list) = manager.GetSessionEnumerator() else { return Vec::new() };
            let mut out = Vec::new();
            for i in 0..list.GetCount().unwrap_or(0) {
                let Ok(control) = list.GetSession(i) else { continue };
                if control.GetState().ok() != Some(AudioSessionStateActive) {
                    continue;
                }
                let Ok(control2) = control.cast::<IAudioSessionControl2>() else { continue };
                if let Some(exe) = control2.GetProcessId().ok().and_then(process_name) {
                    out.push(exe);
                }
            }
            out
        }
    }

    thread_local! {
        /// Created once per thread; re-created when a call fails (e.g. the default device changed).
        static ENDPOINT: std::cell::RefCell<Option<IAudioEndpointVolume>> = const { std::cell::RefCell::new(None) };
        static MIC: std::cell::RefCell<Option<IAudioEndpointVolume>> = const { std::cell::RefCell::new(None) };
    }

    fn cached(slot: &'static std::thread::LocalKey<std::cell::RefCell<Option<IAudioEndpointVolume>>>, flow: EDataFlow) -> Result<IAudioEndpointVolume, String> {
        if let Some(ep) = slot.with(|e| e.borrow().clone()) {
            // SAFETY: COM call on an initialised thread; a failure means the object went stale.
            if unsafe { ep.GetMasterVolumeLevelScalar() }.is_ok() {
                return Ok(ep);
            }
        }
        // SAFETY: see device().
        let ep: IAudioEndpointVolume = unsafe { device_for(flow)?.Activate(CLSCTX_ALL, None).map_err(err)? };
        slot.with(|e| *e.borrow_mut() = Some(ep.clone()));
        Ok(ep)
    }

    /// The whole-device volume of the speakers (System) or the microphone (Mic).
    fn endpoint_of(target: &Target) -> Option<Result<IAudioEndpointVolume, String>> {
        match target {
            Target::System => Some(cached(&ENDPOINT, eRender)),
            Target::Mic => Some(cached(&MIC, eCapture)),
            Target::App(_) | Target::Others(_) => None,
        }
    }

    fn process_name(pid: u32) -> Option<String> {
        // SAFETY: the handle is closed below; the buffer outlives the call.
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut buf = [0u16; 1024];
            let mut len = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len).is_ok();
            let _ = CloseHandle(handle);
            ok.then(|| {
                let full = String::from_utf16_lossy(&buf[..len as usize]);
                full.rsplit(['\\', '/']).next().unwrap_or(&full).to_string()
            })
        }
    }

    fn same_app(exe: &str, wanted: &str) -> bool {
        let norm = |s: &str| s.trim().to_lowercase().trim_end_matches(".exe").to_string();
        norm(exe) == norm(wanted)
    }

    /// The audio sessions of a per-app target: the app's (it can have several, e.g. one per tab),
    /// or for `Others` every app's but the listed ones (and HubDeck's own).
    fn sessions_of(target: &Target) -> Result<Vec<ISimpleAudioVolume>, String> {
        match target {
            Target::App(app) => sessions(app, &|exe| same_app(exe, app)),
            Target::Others(skip) => {
                let all = sessions("", &|exe| !exe.to_lowercase().starts_with("hubdeck") && !skip.iter().any(|s| same_app(exe, s)));
                all.map_err(|_| "no other app is playing sound right now".to_string())
            }
            _ => Err("not an app target".into()),
        }
    }

    fn sessions(app: &str, wanted: &dyn Fn(&str) -> bool) -> Result<Vec<ISimpleAudioVolume>, String> {
        // SAFETY: see device().
        unsafe {
            let manager: IAudioSessionManager2 = device()?.Activate(CLSCTX_ALL, None).map_err(err)?;
            let list = manager.GetSessionEnumerator().map_err(err)?;
            let mut out = Vec::new();
            for i in 0..list.GetCount().map_err(err)? {
                let Ok(control) = list.GetSession(i) else { continue };
                let Ok(control2) = control.cast::<IAudioSessionControl2>() else { continue };
                let Ok(pid) = control2.GetProcessId() else { continue };
                if process_name(pid).is_some_and(|exe| wanted(&exe))
                    && let Ok(simple) = control.cast::<ISimpleAudioVolume>()
                {
                    out.push(simple);
                }
            }
            if out.is_empty() {
                return Err(format!("{app} {}", super::SILENT));
            }
            Ok(out)
        }
    }

    pub fn get(target: &Target) -> Result<f32, String> {
        // SAFETY: see device().
        unsafe {
            match (target, endpoint_of(target)) {
                (_, Some(ep)) => ep?.GetMasterVolumeLevelScalar().map_err(err),
                (t, None) => sessions_of(t)?[0].GetMasterVolume().map_err(err),
            }
        }
    }

    pub fn is_muted(target: &Target) -> Result<bool, String> {
        // SAFETY: see device().
        unsafe {
            match (target, endpoint_of(target)) {
                (_, Some(ep)) => Ok(ep?.GetMute().map_err(err)?.as_bool()),
                (t, None) => Ok(sessions_of(t)?[0].GetMute().map_err(err)?.as_bool()),
            }
        }
    }

    pub fn set(target: &Target, level: f32) -> Result<(), String> {
        // SAFETY: see device().
        unsafe {
            match (target, endpoint_of(target)) {
                (_, Some(ep)) => ep?.SetMasterVolumeLevelScalar(level, std::ptr::null()).map_err(err),
                (t, None) => {
                    for s in sessions_of(t)? {
                        s.SetMasterVolume(level, std::ptr::null()).map_err(err)?;
                    }
                    Ok(())
                }
            }
        }
    }

    pub fn toggle_mute(target: &Target) -> Result<bool, String> {
        // SAFETY: see device().
        unsafe {
            match (target, endpoint_of(target)) {
                (_, Some(ep)) => {
                    let ep = ep?;
                    let muted = !ep.GetMute().map_err(err)?.as_bool();
                    ep.SetMute(muted, std::ptr::null()).map_err(err)?;
                    Ok(muted)
                }
                (t, None) => {
                    let all = sessions_of(t)?;
                    let muted = !all[0].GetMute().map_err(err)?.as_bool();
                    for s in all {
                        s.SetMute(muted, std::ptr::null()).map_err(err)?;
                    }
                    Ok(muted)
                }
            }
        }
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use super::Target;
    use std::process::Command;

    pub fn init_thread() {}

    /// macOS does not say which program records the microphone, only whether one does: then the
    /// call apps that run (their usual names, `zoom`, `ms-teams`, `discord`, browsers last) stand in
    /// for "who records it", which is what call detection needs.
    pub fn mic_users() -> Vec<String> {
        if !coreaudio::input_in_use() {
            return Vec::new();
        }
        let mut names: Vec<String> = running_apps().into_iter().map(|n| crate::engine::profile::app_stem(&n)).collect();
        let browser = |n: &String| ["chrome", "msedge", "firefox", "brave", "opera", "safari", "arc"].contains(&n.as_str());
        names.sort_by_key(|n| browser(n));
        names
    }

    /// The names of the apps running now (NSWorkspace).
    fn running_apps() -> Vec<String> {
        use objc2_app_kit::NSWorkspace;
        // its own pool: the list and its names are autoreleased (a thread without one keeps them)
        objc2::rc::autoreleasepool(|_| NSWorkspace::sharedWorkspace().runningApplications().iter().filter_map(|a| a.localizedName().map(|n| n.to_string())).collect())
    }

    pub fn with_popup(level: f32, _up: bool) {
        // macOS shows its own volume overlay only for the media keys; the volume is still set
        if let Err(e) = set(&Target::System, level) {
            log::warn!("volume: {e}");
        }
    }

    fn osascript(script: &str) -> Result<String, String> {
        let out = Command::new("osascript").args(["-e", script]).output().map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    /// Per-app volume through Core Audio process taps (`src/macos/appvolume.m`, macOS 14.2+).
    mod appvol {
        use std::collections::HashMap;
        use std::ffi::{CString, c_char};
        use std::sync::Mutex;

        use super::Target;

        unsafe extern "C" {
            fn mh_appvol_set(key: *const c_char, needles: *const c_char, others: i32, gain: f32) -> i32;
            fn mh_appvol_get(key: *const c_char) -> f32;
            fn mh_appvol_reset();
        }

        /// What to look for in an app's bundle id or name: its usual name ("spotify") and how the
        /// Mac version is called where that differs ("chrome" plays from com.google.Chrome.helper).
        pub fn needles(app: &str) -> Vec<String> {
            let stem = crate::engine::profile::app_stem(app);
            let mut out = vec![stem.clone()];
            let extra: &[&str] = match stem.as_str() {
                "msedge" => &["edgemac", "microsoft edge"],
                "ms-teams" | "teams" => &["teams"],
                "obs64" => &["obs"],
                "zoom" => &["zoom.us"],
                "code" => &["vscode"],
                "windowsterminal" => &["terminal", "iterm"],
                _ => &[],
            };
            out.extend(extra.iter().map(|e| e.to_string()));
            out
        }

        fn key_and_needles(target: &Target) -> (String, Vec<String>, bool) {
            match target {
                Target::App(app) => (format!("app:{}", crate::engine::profile::app_stem(app)), needles(app), false),
                Target::Others(apps) => ("others".to_string(), apps.iter().flat_map(|a| needles(a)).collect(), true),
                _ => (String::new(), Vec::new(), false),
            }
        }

        /// Levels set for apps that were not playing yet: applied as soon as they play (the owner's
        /// test, session 9: a knob for Zoom showed an error in the tour while Zoom was quiet).
        /// key -> (what to look for, "every other app", level)
        type Waiting = HashMap<String, (Vec<String>, bool, f32)>;
        static PENDING: Mutex<Option<Waiting>> = Mutex::new(None);

        fn apply(key: &str, needles: &[String], others: bool, level: f32) -> i32 {
            let (Ok(k), Ok(n)) = (CString::new(key), CString::new(needles.join("\n"))) else { return -1 };
            // SAFETY: NUL-terminated strings we own, alive for the call.
            unsafe { mh_appvol_set(k.as_ptr(), n.as_ptr(), i32::from(others), level.clamp(0.0, 1.0)) }
        }

        /// Keep the level and try again every two seconds until the app plays (one thread while any wait).
        fn wait_for_sound(key: String, needles: Vec<String>, others: bool, level: f32) {
            let mut pending = PENDING.lock().unwrap_or_else(|e| e.into_inner());
            let map = pending.get_or_insert_with(HashMap::new);
            let start = map.is_empty();
            map.insert(key, (needles, others, level));
            if start {
                std::thread::spawn(|| {
                    loop {
                        std::thread::sleep(std::time::Duration::from_secs(2));
                        let mut pending = PENDING.lock().unwrap_or_else(|e| e.into_inner());
                        let Some(map) = pending.as_mut() else { return };
                        map.retain(|k, (n, o, l)| apply(k, n, *o, *l) == 1);
                        if map.is_empty() {
                            return;
                        }
                    }
                });
            }
        }

        pub fn set(target: &Target, level: f32) -> Result<(), String> {
            let (key, needles, others) = key_and_needles(target);
            if let Some(map) = PENDING.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
                map.remove(&key);
            }
            match apply(&key, &needles, others, level) {
                0 => Ok(()),
                // not playing yet: the level waits for its sound
                1 => {
                    wait_for_sound(key, needles, others, level);
                    Ok(())
                }
                2 => Err("per-app volume needs macOS 14.2 or later".into()),
                3 => Err("allow HubDeck in System Settings > Privacy & Security > Screen & System Audio Recording (System Audio Recording only)".into()),
                _ => Err("the sound output would not start for per-app volume".into()),
            }
        }

        pub fn get(target: &Target) -> f32 {
            let (key, _, _) = key_and_needles(target);
            if let Some((_, _, level)) = PENDING.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|m| m.get(&key)) {
                return *level;
            }
            let Ok(key) = CString::new(key) else { return 1.0 };
            // SAFETY: as above.
            unsafe { mh_appvol_get(key.as_ptr()) }
        }

        /// The level before a mute, to go back to.
        static BEFORE: Mutex<Option<HashMap<String, f32>>> = Mutex::new(None);

        pub fn toggle_mute(target: &Target) -> Result<bool, String> {
            let (key, _, _) = key_and_needles(target);
            let now = get(target);
            let mut before = BEFORE.lock().unwrap_or_else(|e| e.into_inner());
            let map = before.get_or_insert_with(HashMap::new);
            if now > 0.001 {
                map.insert(key, now);
                set(target, 0.0)?;
                Ok(true)
            } else {
                set(target, map.remove(&key).unwrap_or(1.0))?;
                Ok(false)
            }
        }

        /// Every app back to playing straight (HubDeck quits).
        pub fn reset() {
            // SAFETY: no arguments; it stops and removes what it made.
            unsafe { mh_appvol_reset() }
        }
    }

    /// HubDeck quits: apps whose volume it set play straight again.
    pub fn release() {
        appvol::reset();
    }

    fn mic_level() -> Result<f32, String> {
        let v: f32 = osascript("input volume of (get volume settings)")?.parse().map_err(|_| "unexpected input volume".to_string())?;
        Ok(v / 100.0)
    }

    /// Microphone input volume before we muted it, to restore on unmute.
    static MIC_BEFORE: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

    pub fn is_muted(target: &Target) -> Result<bool, String> {
        match target {
            Target::Mic => Ok(osascript("input volume of (get volume settings)")? == "0"),
            Target::App(_) | Target::Others(_) => Ok(appvol::get(target) <= 0.001),
            Target::System => Ok(osascript("output muted of (get volume settings)")? == "true"),
        }
    }

    pub fn get(target: &Target) -> Result<f32, String> {
        match target {
            Target::App(_) | Target::Others(_) => return Ok(appvol::get(target)),
            Target::Mic => return mic_level(),
            Target::System => {}
        }
        if let Some(v) = coreaudio::volume() {
            return Ok(v);
        }
        let v: f32 = osascript("output volume of (get volume settings)")?.parse().map_err(|_| "unexpected volume value".to_string())?;
        Ok(v / 100.0)
    }

    pub fn set(target: &Target, level: f32) -> Result<(), String> {
        match target {
            Target::App(_) | Target::Others(_) => return appvol::set(target, level),
            Target::Mic => return osascript(&format!("set volume input volume {}", (level.clamp(0.0, 1.0) * 100.0).round() as i32)).map(|_| ()),
            Target::System => {}
        }
        if coreaudio::set_volume(level.clamp(0.0, 1.0)) {
            return Ok(());
        }
        osascript(&format!("set volume output volume {}", (level * 100.0).round() as i32)).map(|_| ())
    }

    /// The default output's main volume straight from Core Audio: well under a millisecond, where
    /// AppleScript took ~0.2 s a detent and a fast turn lagged behind (owner, 2026-10-13). AppleScript
    /// stays the fallback for devices without a main volume.
    mod coreaudio {
        use std::ffi::c_void;

        #[repr(C)]
        struct Address {
            selector: u32,
            scope: u32,
            element: u32,
        }

        const fn code(s: &[u8; 4]) -> u32 {
            u32::from_be_bytes(*s)
        }
        const SYSTEM_OBJECT: u32 = 1;
        const DEFAULT_OUTPUT: u32 = code(b"dOut");
        const DEFAULT_INPUT: u32 = code(b"dIn ");
        const RUNNING_SOMEWHERE: u32 = code(b"gone");
        const SCOPE_GLOBAL: u32 = code(b"glob");
        const SCOPE_OUTPUT: u32 = code(b"outp");
        const VIRTUAL_MAIN_VOLUME: u32 = code(b"vmvc");

        #[link(name = "CoreAudio", kind = "framework")]
        unsafe extern "C" {
            fn AudioObjectGetPropertyData(object: u32, address: *const Address, qualifier_size: u32, qualifier: *const c_void, size: *mut u32, data: *mut c_void) -> i32;
        }
        #[link(name = "AudioToolbox", kind = "framework")]
        unsafe extern "C" {
            fn AudioHardwareServiceGetPropertyData(object: u32, address: *const Address, qualifier_size: u32, qualifier: *const c_void, size: *mut u32, data: *mut c_void) -> i32;
            fn AudioHardwareServiceSetPropertyData(object: u32, address: *const Address, qualifier_size: u32, qualifier: *const c_void, size: u32, data: *const c_void) -> i32;
        }

        fn output_device() -> Option<u32> {
            let address = Address { selector: DEFAULT_OUTPUT, scope: SCOPE_GLOBAL, element: 0 };
            let mut device = 0u32;
            let mut size = std::mem::size_of::<u32>() as u32;
            // SAFETY: a plain property read into a u32 we own, with its size.
            let status = unsafe { AudioObjectGetPropertyData(SYSTEM_OBJECT, &address, 0, std::ptr::null(), &mut size, (&mut device as *mut u32).cast()) };
            (status == 0 && device != 0).then_some(device)
        }

        /// Some program records the default microphone now.
        pub fn input_in_use() -> bool {
            let address = Address { selector: DEFAULT_INPUT, scope: SCOPE_GLOBAL, element: 0 };
            let mut device = 0u32;
            let mut size = std::mem::size_of::<u32>() as u32;
            // SAFETY: plain property reads into u32s we own, with their sizes.
            let status = unsafe { AudioObjectGetPropertyData(SYSTEM_OBJECT, &address, 0, std::ptr::null(), &mut size, (&mut device as *mut u32).cast()) };
            if status != 0 || device == 0 {
                return false;
            }
            let address = Address { selector: RUNNING_SOMEWHERE, scope: SCOPE_GLOBAL, element: 0 };
            let mut running = 0u32;
            let mut size = std::mem::size_of::<u32>() as u32;
            let status = unsafe { AudioObjectGetPropertyData(device, &address, 0, std::ptr::null(), &mut size, (&mut running as *mut u32).cast()) };
            status == 0 && running != 0
        }

        pub fn volume() -> Option<f32> {
            let device = output_device()?;
            let address = Address { selector: VIRTUAL_MAIN_VOLUME, scope: SCOPE_OUTPUT, element: 0 };
            let mut v = 0f32;
            let mut size = std::mem::size_of::<f32>() as u32;
            // SAFETY: as above, into an f32.
            let status = unsafe { AudioHardwareServiceGetPropertyData(device, &address, 0, std::ptr::null(), &mut size, (&mut v as *mut f32).cast()) };
            (status == 0).then_some(v)
        }

        pub fn set_volume(level: f32) -> bool {
            let Some(device) = output_device() else { return false };
            let address = Address { selector: VIRTUAL_MAIN_VOLUME, scope: SCOPE_OUTPUT, element: 0 };
            // SAFETY: writes an f32 we own, with its size.
            let status = unsafe { AudioHardwareServiceSetPropertyData(device, &address, 0, std::ptr::null(), std::mem::size_of::<f32>() as u32, (&level as *const f32).cast()) };
            status == 0
        }

        #[cfg(test)]
        mod tests {
            /// Reads only; run by hand on a Mac: `cargo test -p hubdeck coreaudio -- --ignored`.
            #[test]
            #[ignore]
            fn reads_the_output_volume() {
                let v = super::volume().expect("a default output with a main volume");
                assert!((0.0..=1.0).contains(&v));
                println!("output volume {v}");
            }
        }
    }

    pub fn toggle_mute(target: &Target) -> Result<bool, String> {
        if *target == Target::Mic {
            // macOS has no input mute: set the input volume to 0 and back
            let now = osascript("input volume of (get volume settings)")?;
            let mut before = MIC_BEFORE.lock().unwrap_or_else(|e| e.into_inner());
            if now != "0" {
                *before = Some(now);
                osascript("set volume input volume 0")?;
                return Ok(true);
            }
            osascript(&format!("set volume input volume {}", before.take().unwrap_or_else(|| "75".into())))?;
            return Ok(false);
        }
        if matches!(target, Target::App(_) | Target::Others(_)) {
            return appvol::toggle_mute(target);
        }
        let muted = osascript("output muted of (get volume settings)")? != "true";
        osascript(&format!("set volume output muted {muted}"))?;
        Ok(muted)
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod platform {
    use super::Target;
    pub fn init_thread() {}
    pub fn mic_users() -> Vec<String> {
        Vec::new()
    }
    pub fn with_popup(_: f32, _: bool) {}
    pub fn get(_: &Target) -> Result<f32, String> {
        Err("volume control is not implemented on this platform yet".into())
    }
    pub fn set(_: &Target, _: f32) -> Result<(), String> {
        Err("volume control is not implemented on this platform yet".into())
    }
    pub fn toggle_mute(_: &Target) -> Result<bool, String> {
        Err("volume control is not implemented on this platform yet".into())
    }
    pub fn is_muted(_: &Target) -> Result<bool, String> {
        Err("volume control is not implemented on this platform yet".into())
    }
}
