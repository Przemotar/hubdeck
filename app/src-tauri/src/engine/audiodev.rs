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

/// Make the device whose name contains `name` (case-insensitive) the default for every role.
pub fn set_default(input: bool, name: &str) -> Result<String, String> {
    let devices = list(input)?;
    let wanted = name.trim().to_lowercase();
    let d = devices
        .iter()
        .find(|d| d.name.to_lowercase() == wanted)
        .or_else(|| devices.iter().find(|d| d.name.to_lowercase().contains(&wanted)))
        .ok_or_else(|| format!("no {} device named {name:?}", if input { "input" } else { "output" }))?;
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
