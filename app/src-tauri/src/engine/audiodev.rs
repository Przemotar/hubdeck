//! The default speakers and microphone: list them and make one the default. Windows has no public
//! call for setting the default; every tool (SoundSwitch, EarTrumpet, NirCmd) uses the same
//! undocumented `IPolicyConfig` interface of the audio service, and so do we.

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub id: String,
    pub name: String,
    pub default: bool,
}

/// Active output (`input` false) or input devices.
pub fn list(input: bool) -> Result<Vec<Device>, String> {
    platform::list(input)
}

/// The device a saved value names: its system id (what Stream Deck's Audio Switcher keeps, and
/// what a converted key carries), else the same name, else the same name without Windows'
/// numbering (see [`plain`]), else a name that contains it (all case-insensitive).
pub fn find<'a>(devices: &'a [Device], wanted: &str) -> Option<&'a Device> {
    let wanted = wanted.trim().to_lowercase();
    if wanted.is_empty() {
        return None;
    }
    let bare = plain(&wanted);
    devices
        .iter()
        .find(|d| d.id.to_lowercase() == wanted)
        .or_else(|| devices.iter().find(|d| d.name.to_lowercase() == wanted))
        .or_else(|| devices.iter().find(|d| plain(&d.name) == bare))
        .or_else(|| devices.iter().find(|d| d.name.to_lowercase().contains(&wanted)))
        .or_else(|| devices.iter().find(|d| plain(&d.name).contains(&bare)))
}

/// Does a saved device value name one of these names or system ids (as [`find`] would)?
pub fn known(names_or_ids: &[String], wanted: &str) -> bool {
    let devices: Vec<Device> = names_or_ids.iter().map(|n| Device { id: n.clone(), name: n.clone(), default: false }).collect();
    find(&devices, wanted).is_some()
}

/// A device name lowercased and without the number Windows puts before a name it has seen twice:
/// "Głośniki (2 — Realtek(R) Audio)", "1 - BenQ EW277HDR (...)". The number comes and goes when
/// Windows installs the devices again (the owner's upgrade to Windows 11, 2026-11-01: "2 — "
/// was gone and his key found no speakers).
pub fn plain(name: &str) -> String {
    let c: Vec<char> = name.trim().to_lowercase().chars().collect();
    let mut out = String::with_capacity(name.len());
    let mut i = 0;
    while i < c.len() {
        if i == 0 || c[i - 1] == '(' {
            let mut j = i;
            while j < c.len() && c[j].is_ascii_digit() {
                j += 1;
            }
            let digits = j > i;
            while j < c.len() && c[j] == ' ' {
                j += 1;
            }
            if digits && j < c.len() && matches!(c[j], '—' | '–' | '-') {
                j += 1;
                while j < c.len() && c[j] == ' ' {
                    j += 1;
                }
                i = j;
                continue;
            }
        }
        out.push(c[i]);
        i += 1;
    }
    out
}

/// Make the device `name` names (see [`find`]) the default for every role.
pub fn set_default(input: bool, name: &str) -> Result<String, String> {
    let devices = list(input)?;
    let d = find(&devices, name).ok_or_else(|| format!("no {} device named {name:?}", if input { "input" } else { "output" }))?;
    platform::set_default(&d.id)?;
    Ok(d.name.clone())
}

/// Of two devices, the one to make the default: `b` while `a` is the default now, else `a`.
pub fn pick_between<'a>(devices: &'a [Device], a: &str, b: &str) -> Result<&'a Device, String> {
    let first = find(devices, a).ok_or_else(|| format!("no device named {a:?}"))?;
    let second = find(devices, b).ok_or_else(|| format!("no device named {b:?}"))?;
    Ok(if first.default { second } else { first })
}

/// A press of a key with two devices: switch to the other one; returns its name.
pub fn switch_between(input: bool, a: &str, b: &str) -> Result<String, String> {
    let devices = list(input)?;
    let d = pick_between(&devices, a, b)?;
    platform::set_default(&d.id)?;
    Ok(d.name.clone())
}

/// Make the next (or previous) device the default; returns its name.
pub fn cycle(input: bool, step: i32) -> Result<String, String> {
    let devices = list(input)?;
    if devices.is_empty() {
        return Err(if input {
            "no microphone found: connect one, or check it in the system's sound settings"
        } else {
            "no speakers or headphones found: connect some, or check them in the system's sound settings"
        }
        .into());
    }
    let now = devices.iter().position(|d| d.default).unwrap_or(0) as i32;
    let next = &devices[(now + step).rem_euclid(devices.len() as i32) as usize];
    platform::set_default(&next.id)?;
    Ok(next.name.clone())
}

#[cfg(windows)]
#[allow(non_snake_case)] // COM method names as Windows defines them
mod platform {
    use std::ffi::c_void;

    use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
    use windows::Win32::Media::Audio::{DEVICE_STATE_ACTIVE, ERole, IMMDeviceEnumerator, MMDeviceEnumerator, eCapture, eCommunications, eConsole, eMultimedia, eRender};
    use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance, CoTaskMemFree, STGM_READ};
    use windows::core::{GUID, HRESULT, IUnknown, IUnknown_Vtbl, PCWSTR, interface};

    use super::Device;

    /// CPolicyConfigClient, the audio service's policy object.
    const POLICY_CONFIG_CLIENT: GUID = GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);

    // Only SetDefaultEndpoint is called; the slots before it keep the vtable in order.
    #[interface("f8679f50-850a-41cf-9c72-430f290290c8")]
    unsafe trait IPolicyConfig: IUnknown {
        fn GetMixFormat(&self, a: PCWSTR, b: *mut *mut c_void) -> HRESULT;
        fn GetDeviceFormat(&self, a: PCWSTR, b: i32, c: *mut *mut c_void) -> HRESULT;
        fn ResetDeviceFormat(&self, a: PCWSTR) -> HRESULT;
        fn SetDeviceFormat(&self, a: PCWSTR, b: *mut c_void, c: *mut c_void) -> HRESULT;
        fn GetProcessingPeriod(&self, a: PCWSTR, b: i32, c: *mut i64, d: *mut i64) -> HRESULT;
        fn SetProcessingPeriod(&self, a: PCWSTR, b: *mut i64) -> HRESULT;
        fn GetShareMode(&self, a: PCWSTR, b: *mut c_void) -> HRESULT;
        fn SetShareMode(&self, a: PCWSTR, b: *mut c_void) -> HRESULT;
        fn GetPropertyValue(&self, a: PCWSTR, b: *const c_void, c: *mut c_void) -> HRESULT;
        fn SetPropertyValue(&self, a: PCWSTR, b: *const c_void, c: *mut c_void) -> HRESULT;
        fn SetDefaultEndpoint(&self, device: PCWSTR, role: ERole) -> HRESULT;
        fn SetEndpointVisibility(&self, a: PCWSTR, b: i32) -> HRESULT;
    }

    fn err(e: windows::core::Error) -> String {
        e.message().to_string()
    }

    pub fn list(input: bool) -> Result<Vec<Device>, String> {
        let flow = if input { eCapture } else { eRender };
        // SAFETY: COM calls on a thread the engine initialised for COM; strings are freed.
        unsafe {
            let e: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(err)?;
            let default_id = e.GetDefaultAudioEndpoint(flow, eConsole).ok().and_then(|d| d.GetId().ok()).map(|p| {
                let s = p.to_string().unwrap_or_default();
                CoTaskMemFree(Some(p.0 as *const c_void));
                s
            });
            let all = e.EnumAudioEndpoints(flow, DEVICE_STATE_ACTIVE).map_err(err)?;
            let mut out = Vec::new();
            for i in 0..all.GetCount().map_err(err)? {
                let Ok(d) = all.Item(i) else { continue };
                let Ok(pid) = d.GetId() else { continue };
                let id = pid.to_string().unwrap_or_default();
                CoTaskMemFree(Some(pid.0 as *const c_void));
                let name = d.OpenPropertyStore(STGM_READ).ok().and_then(|store| store.GetValue(&PKEY_Device_FriendlyName).ok()).map(|v| v.to_string()).unwrap_or_else(|| id.clone());
                out.push(Device { default: Some(&id) == default_id.as_ref(), id, name });
            }
            Ok(out)
        }
    }

    pub fn set_default(id: &str) -> Result<(), String> {
        let wide: Vec<u16> = id.encode_utf16().chain([0]).collect();
        // SAFETY: COM on an initialised thread; the id buffer outlives the calls.
        unsafe {
            let policy: IPolicyConfig = CoCreateInstance(&POLICY_CONFIG_CLIENT, None, CLSCTX_ALL).map_err(err)?;
            for role in [eConsole, eMultimedia, eCommunications] {
                policy.SetDefaultEndpoint(PCWSTR(wide.as_ptr()), role).ok().map_err(err)?;
            }
        }
        Ok(())
    }
}

#[cfg(not(windows))]
mod platform {
    use super::Device;

    pub fn list(_input: bool) -> Result<Vec<Device>, String> {
        Err("switching audio devices is not available on this system yet".into())
    }

    pub fn set_default(_id: &str) -> Result<(), String> {
        Err("switching audio devices is not available on this system yet".into())
    }
}

#[cfg(test)]
mod find_tests {
    use super::{Device, find, pick_between};

    #[test]
    fn two_devices_take_turns() {
        let dev = |id: &str, name: &str, default: bool| Device { id: id.into(), name: name.into(), default };
        let now_speakers = [dev("{1}", "Speakers (Realtek)", true), dev("{2}", "Headphones (USB)", false)];
        assert_eq!(pick_between(&now_speakers, "Speakers", "Headphones").unwrap().name, "Headphones (USB)");
        let now_headphones = [dev("{1}", "Speakers (Realtek)", false), dev("{2}", "Headphones (USB)", true)];
        assert_eq!(pick_between(&now_headphones, "Speakers", "Headphones").unwrap().name, "Speakers (Realtek)");
        // a third device is the default: the first one comes
        let other = [dev("{1}", "Speakers (Realtek)", false), dev("{2}", "Headphones (USB)", false), dev("{3}", "Monitor", true)];
        assert_eq!(pick_between(&other, "Speakers", "Headphones").unwrap().name, "Speakers (Realtek)");
        assert!(pick_between(&now_speakers, "Speakers", "TV").is_err());
    }

    #[test]
    fn a_device_is_found_by_id_name_or_part_of_the_name() {
        let d = |id: &str, name: &str| Device { id: id.into(), name: name.into(), default: false };
        let devices = [d("{0.0.0.00000000}.{5306ea6f}", "LG ULTRAGEAR (NVIDIA High Definition Audio)"), d("{0.0.0.00000000}.{9c79ccff-8f51}", "Głośniki (Realtek(R) Audio)")];
        // the owner's Key 15 after its Audio Switcher key became a built-in one (2026-10-23)
        assert_eq!(find(&devices, "{0.0.0.00000000}.{9C79CCFF-8f51}").unwrap().name, "Głośniki (Realtek(R) Audio)");
        assert_eq!(find(&devices, "głośniki (realtek(r) audio)").unwrap().name, "Głośniki (Realtek(R) Audio)");
        assert_eq!(find(&devices, "ultragear").unwrap().name, "LG ULTRAGEAR (NVIDIA High Definition Audio)");
        assert!(find(&devices, "fifine").is_none());
        assert!(find(&devices, " ").is_none());
    }

    #[test]
    fn windows_numbering_does_not_lose_the_device() {
        let d = |id: &str, name: &str| Device { id: id.into(), name: name.into(), default: false };
        // the owner's speakers before and after the upgrade to Windows 11 (2026-11-01), new ids
        let after = [d("{0ddeae63}", "Głośniki (Realtek(R) Audio)"), d("{b378d7b1}", "Interfejs SPDIF (FiiO USB DAC-E10)")];
        assert_eq!(find(&after, "Głośniki (2 — Realtek(R) Audio)").unwrap().id, "{0ddeae63}");
        let before = [d("{9c79ccff}", "Głośniki (2 — Realtek(R) Audio)"), d("{2a748d9b}", "1 - BenQ EW277HDR (NVIDIA High Definition Audio)")];
        assert_eq!(find(&before, "Głośniki (Realtek(R) Audio)").unwrap().id, "{9c79ccff}");
        assert_eq!(find(&before, "BenQ EW277HDR (NVIDIA High Definition Audio)").unwrap().id, "{2a748d9b}");
        // a number that is part of the name stays
        assert_eq!(super::plain("Realtek HD Audio 2nd output (Realtek(R) Audio)"), "realtek hd audio 2nd output (realtek(r) audio)");
        assert_eq!(super::plain("Speakers (2- USB Audio)"), "speakers (usb audio)");
        assert!(super::known(&["Głośniki (Realtek(R) Audio)".into()], "Głośniki (2 — Realtek(R) Audio)"));
        assert!(!super::known(&["Głośniki (Realtek(R) Audio)".into()], "Słuchawki (USB)"));
    }
}

#[cfg(all(test, windows))]
mod tests {
    /// Lists (never changes) the audio devices: `cargo test -p hubdeck list_audio -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn list_audio_devices() {
        crate::engine::volume::init_thread();
        println!("outputs: {:?}", super::list(false));
        println!("inputs: {:?}", super::list(true));
        // setting the device that already is the default changes nothing but proves the call works
        let current = super::list(false).unwrap().into_iter().find(|d| d.default).expect("a default output");
        super::platform::set_default(&current.id).expect("IPolicyConfig works");
        assert!(super::list(false).unwrap().iter().any(|d| d.default && d.id == current.id));
    }
}
