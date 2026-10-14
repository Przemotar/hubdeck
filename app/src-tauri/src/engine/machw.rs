//! macOS sensors and monitor brightness through `src/macos/hw.m` (Apple has no public API for
//! them; each one is looked up at run time, so a Mac without it reads "not available").

unsafe extern "C" {
    fn mh_cpu_temp() -> f64;
    fn mh_gpu_temp() -> f64;
    fn mh_gpu_load() -> f64;
    fn mh_monitors() -> i32;
    fn mh_monitor_get(index: i32) -> i32;
    fn mh_monitor_set(index: i32, value: i32) -> i32;
    fn mh_sensor_names(out: *mut std::ffi::c_char, size: i32);
    fn mh_session_locked() -> i32;
    fn mh_idle_seconds() -> f64;
}

/// Whether the screen is locked (None = unknown).
pub fn session_locked() -> Option<bool> {
    // SAFETY: a plain C function without arguments; it releases what it copies.
    match unsafe { mh_session_locked() } {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

/// Seconds since the last keyboard or mouse input.
pub fn idle_seconds() -> Option<f64> {
    // SAFETY: as above.
    let s = unsafe { mh_idle_seconds() };
    (s.is_finite() && s >= 0.0).then_some(s)
}

/// The temperature sensors' names (diagnostics: they differ from chip to chip).
pub fn sensor_names() -> Vec<String> {
    let mut buf = vec![0u8; 8192];
    // SAFETY: the helper writes a NUL-terminated string within `size` bytes of our buffer.
    unsafe { mh_sensor_names(buf.as_mut_ptr().cast(), buf.len() as i32) };
    let end = buf.iter().position(|b| *b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..end]).lines().map(str::to_string).collect()
}

fn some(v: f64) -> Option<f32> {
    (v >= 0.0).then_some(v as f32)
}

/// The CPU clusters' temperature, °C.
pub fn cpu_temp() -> Option<f32> {
    // SAFETY: plain C functions without arguments; they own and release their objects.
    some(unsafe { mh_cpu_temp() })
}

pub fn gpu_temp() -> Option<f32> {
    // SAFETY: as above.
    some(unsafe { mh_gpu_temp() })
}

/// The GPU's load, 0..100.
pub fn gpu_load() -> Option<f32> {
    // SAFETY: as above.
    some(unsafe { mh_gpu_load() })
}

/// Monitors HubDeck can set: the built-in screen first, then external ones (DDC).
pub fn monitors() -> usize {
    // SAFETY: as above.
    unsafe { mh_monitors() }.max(0) as usize
}

/// Monitor `index` (0-based), 0..=100.
pub fn get(index: usize) -> Option<u32> {
    // SAFETY: a plain C function taking a number.
    let v = unsafe { mh_monitor_get(index as i32) };
    (v >= 0).then_some(v as u32)
}

/// Set monitor `index` (None = every one) to 0..=100.
pub fn set(index: Option<usize>, value: u32) -> bool {
    // SAFETY: a plain C function taking numbers.
    unsafe { mh_monitor_set(index.map_or(-1, |i| i as i32), value.min(100) as i32) == 0 }
}

#[cfg(test)]
mod tests {
    /// Reads only; on a Mac: `cargo test -p hubdeck machw -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn reads_this_mac() {
        println!("cpu {:?} °C, gpu {:?} °C, gpu load {:?} %", super::cpu_temp(), super::gpu_temp(), super::gpu_load());
        let mut names = super::sensor_names();
        names.sort();
        names.dedup();
        println!("sensors: {names:?}");
        let n = super::monitors();
        println!("monitors {n}: {:?}", (0..n).map(super::get).collect::<Vec<_>>());
    }

    /// Sets the built-in screen to the brightness it has (nothing visible changes).
    #[test]
    #[ignore]
    fn sets_the_same_brightness() {
        let now = super::get(0).expect("a built-in screen");
        assert!(super::set(Some(0), now));
        println!("set {now}, read back {:?}", super::get(0));
    }
}
