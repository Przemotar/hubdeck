//! System load for the dial, keys and full-panel graphs: CPU and memory from the OS, NVIDIA GPU
//! load, temperature and memory through NVML (part of the NVIDIA driver), and the CPU temperature
//! and other GPUs from LibreHardwareMonitor when the user runs it (`lhm.rs`, read only while a
//! control shows such a value). No kernel driver of our own: one froze this PC when the official
//! app shipped it.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crossbeam_channel::Sender;
use serde::{Deserialize, Serialize};

use super::Msg;

const SAMPLE_EVERY: Duration = Duration::from_millis(1000);
/// Samples kept for graphs: two minutes.
pub const HISTORY: usize = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Metric {
    Cpu,
    CpuTemp,
    Ram,
    Gpu,
    GpuTemp,
    Vram,
}

impl Metric {
    /// What turning a monitor flips through (VRAM only where a GPU tells it).
    pub const CYCLE: [Metric; 5] = [Metric::Cpu, Metric::Ram, Metric::Gpu, Metric::CpuTemp, Metric::GpuTemp];

    pub fn from_param(s: &str) -> Metric {
        match s {
            "ram" => Metric::Ram,
            "cpuTemp" => Metric::CpuTemp,
            "gpu" => Metric::Gpu,
            "gpuTemp" => Metric::GpuTemp,
            "vram" => Metric::Vram,
            _ => Metric::Cpu,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Metric::Cpu => "CPU",
            Metric::CpuTemp => "CPU temp",
            Metric::Ram => "RAM",
            Metric::Gpu => "GPU",
            Metric::GpuTemp => "GPU temp",
            Metric::Vram => "VRAM",
        }
    }

    /// `54%` or `61°C`.
    pub fn format(self, v: f32) -> String {
        match self {
            Metric::GpuTemp | Metric::CpuTemp => format!("{}°C", v.round() as i32),
            _ => format!("{}%", v.round() as i32),
        }
    }
}

/// One sample; GPU values are None without an NVIDIA GPU.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Sample {
    pub cpu: f32,
    /// From LibreHardwareMonitor; None when it does not run.
    pub cpu_temp: Option<f32>,
    pub ram: f32,
    pub gpu: Option<f32>,
    pub gpu_temp: Option<f32>,
    pub vram: Option<f32>,
}

impl Sample {
    /// 0..=100 (temperature in °C).
    pub fn get(&self, m: Metric) -> Option<f32> {
        match m {
            Metric::Cpu => Some(self.cpu),
            Metric::CpuTemp => self.cpu_temp,
            Metric::Ram => Some(self.ram),
            Metric::Gpu => self.gpu,
            Metric::GpuTemp => self.gpu_temp,
            Metric::Vram => self.vram,
        }
    }
}

/// Whole-machine CPU load from the change in the system's busy and idle times since last call.
/// Windows: `GetSystemTimes` (microseconds of work). sysinfo's `refresh_cpu_usage` took 1.1 s of
/// CPU per call on the owner's PC (2026-10-10) and made HubDeck use a quarter of a core when idle.
#[cfg(windows)]
struct CpuLoad {
    last: Option<(u64, u64)>,
}

#[cfg(windows)]
impl CpuLoad {
    fn new() -> Self {
        CpuLoad { last: None }
    }

    fn sample(&mut self, _sys: &mut sysinfo::System) -> f32 {
        use windows::Win32::Foundation::FILETIME;
        use windows::Win32::System::Threading::GetSystemTimes;
        let (mut idle, mut kernel, mut user) = (FILETIME::default(), FILETIME::default(), FILETIME::default());
        // SAFETY: three valid out-pointers.
        if unsafe { GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)) }.is_err() {
            return 0.0;
        }
        let ticks = |f: FILETIME| ((f.dwHighDateTime as u64) << 32) | f.dwLowDateTime as u64;
        // kernel time includes the idle time
        let (idle, total) = (ticks(idle), ticks(kernel) + ticks(user));
        let load = match self.last {
            Some((i0, t0)) if total > t0 => 100.0 * (1.0 - (idle.saturating_sub(i0)) as f32 / (total - t0) as f32),
            _ => 0.0,
        };
        self.last = Some((idle, total));
        load.clamp(0.0, 100.0)
    }
}

#[cfg(not(windows))]
struct CpuLoad;

#[cfg(not(windows))]
impl CpuLoad {
    fn new() -> Self {
        CpuLoad
    }

    fn sample(&mut self, sys: &mut sysinfo::System) -> f32 {
        sys.refresh_cpu_usage();
        sys.global_cpu_usage()
    }
}

/// How much of the RAM is in use, %. On Windows from `GlobalMemoryStatusEx` (microseconds):
/// sysinfo's `refresh_memory` took 29.6 ms there, every second, 3 % of a core on its own
/// (session 33, `scripts/thread-cpu.py`: the "monitor" thread at 3.8 % with the deck still).
#[cfg(windows)]
fn ram_used(_sys: &mut sysinfo::System) -> f32 {
    use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    let mut m = MEMORYSTATUSEX { dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
    // SAFETY: a valid out-pointer with its length set.
    if unsafe { GlobalMemoryStatusEx(&mut m) }.is_err() || m.ullTotalPhys == 0 {
        return 0.0;
    }
    (m.ullTotalPhys - m.ullAvailPhys) as f32 / m.ullTotalPhys as f32 * 100.0
}

#[cfg(not(windows))]
fn ram_used(sys: &mut sysinfo::System) -> f32 {
    sys.refresh_memory();
    if sys.total_memory() > 0 { sys.used_memory() as f32 / sys.total_memory() as f32 * 100.0 } else { 0.0 }
}

/// A control shows a value only LibreHardwareMonitor gives (CPU temperature, a non-NVIDIA GPU).
static WANT_LHM: AtomicBool = AtomicBool::new(false);

pub fn want_lhm(yes: bool) {
    WANT_LHM.store(yes, Ordering::Relaxed);
}

/// Try to reach LibreHardwareMonitor again this long after it was not there.
#[cfg_attr(not(windows), allow(dead_code))]
const LHM_RETRY: Duration = Duration::from_secs(30);

/// Sample once a second, forever.
pub fn watch(tx: Sender<Msg>) {
    use sysinfo::System;
    let mut sys = System::new();
    let mut cpu = CpuLoad::new();
    let nvml = nvml_wrapper::Nvml::init().map_err(|e| log::info!("no NVIDIA GPU readings ({e})")).ok();
    let mut first = true;
    /// Where LibreHardwareMonitor's readings come from.
    #[cfg(windows)]
    enum Source {
        Web,
        Wmi(super::lhm::Lhm),
    }
    #[cfg(windows)]
    let mut lhm: Option<Source> = None;
    let mut lhm_tried: Option<Instant> = None;
    loop {
        let cpu_load = cpu.sample(&mut sys);
        let ram = ram_used(&mut sys);
        let mut sample = Sample { cpu: cpu_load, ram, ..Sample::default() };
        if let Some(device) = nvml.as_ref().and_then(|n| n.device_by_index(0).ok()) {
            sample.gpu = device.utilization_rates().ok().map(|u| u.gpu as f32);
            sample.gpu_temp = device.temperature(nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu).ok().map(|t| t as f32);
            sample.vram = device.memory_info().ok().filter(|m| m.total > 0).map(|m| m.used as f32 / m.total as f32 * 100.0);
        }
        // macOS: the CPU and GPU temperatures and the GPU load from the system's own sensors
        #[cfg(target_os = "macos")]
        {
            sample.cpu_temp = super::machw::cpu_temp();
            sample.gpu_temp = super::machw::gpu_temp();
            sample.gpu = sample.gpu.or_else(super::machw::gpu_load);
        }
        if WANT_LHM.load(Ordering::Relaxed) {
            #[cfg(windows)]
            {
                // its web server first (0.9.5 and later have no WMI), then WMI (up to 0.9.4)
                if lhm.is_none() && lhm_tried.is_none_or(|t| t.elapsed() >= LHM_RETRY) {
                    lhm_tried = Some(Instant::now());
                    // why the web server gave nothing goes in the log, so a user's diagnostics tell
                    // "off" from "answers without these sensors" (an AMD user still saw "-" after 0.1.6)
                    let web = super::lhm::from_web();
                    let why = match &web {
                        Ok(s) => format!("{} temperature and load sensors", s.len()),
                        Err(e) => e.clone(),
                    };
                    lhm = match web {
                        Ok(s) if !s.is_empty() => Some(Source::Web),
                        // its old WMI namespace may stay registered and empty after an update
                        _ => super::lhm::Lhm::connect().filter(|l| l.sensors().is_ok_and(|s| !s.is_empty())).map(Source::Wmi),
                    };
                    let picked = match &lhm {
                        Some(Source::Web) => super::lhm::from_web().ok(),
                        Some(Source::Wmi(w)) => w.sensors().ok(),
                        None => None,
                    }
                    .map(|s| super::lhm::pick(&s));
                    log::info!(
                        "LibreHardwareMonitor {}",
                        match &lhm {
                            Some(Source::Web) => format!("found (web server: {why}; picked {picked:?})"),
                            Some(Source::Wmi(_)) => format!("found (WMI; picked {picked:?})"),
                            None => format!("not running, or its web server is off ({}: {why})", super::lhm::WEB),
                        }
                    );
                }
                let reading = lhm.as_ref().map(|l| match l {
                    Source::Web => super::lhm::from_web(),
                    Source::Wmi(w) => w.sensors(),
                });
                match reading {
                    Some(Ok(sensors)) => {
                        let r = super::lhm::pick(&sensors);
                        sample.cpu_temp = r.cpu_temp;
                        sample.gpu = sample.gpu.or(r.gpu_load);
                        sample.gpu_temp = sample.gpu_temp.or(r.gpu_temp);
                    }
                    Some(Err(e)) => {
                        log::info!("LibreHardwareMonitor gone ({e})");
                        lhm = None;
                    }
                    None => {}
                }
            }
            #[cfg(not(windows))]
            let _ = &mut lhm_tried;
        }
        if std::mem::take(&mut first) {
            log::info!("system monitor running: {sample:?}");
            // the sensor names differ by chip: the log says what this Mac has
            #[cfg(target_os = "macos")]
            log::info!("temperature sensors: {}", super::machw::sensor_names().join(", "));
        }
        if tx.send(Msg::Metrics(sample)).is_err() {
            return;
        }
        std::thread::sleep(SAMPLE_EVERY);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What each reading costs: `cargo test --release -p hubdeck sample_cost -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn sample_cost() {
        use std::time::Instant;
        let mut sys = sysinfo::System::new();
        let mut cpu = CpuLoad::new();
        let t = Instant::now();
        for _ in 0..5 {
            cpu.sample(&mut sys);
        }
        println!("cpu sample: {:?}", t.elapsed() / 5);
        std::thread::sleep(std::time::Duration::from_millis(500));
        println!("cpu load now: {:.1}%", cpu.sample(&mut sys));
        let t = Instant::now();
        for _ in 0..5 {
            sys.refresh_memory();
        }
        println!("refresh_memory: {:?}", t.elapsed() / 5);
        let t = Instant::now();
        for _ in 0..5 {
            ram_used(&mut sys);
        }
        println!("ram_used: {:?} ({:.1} %)", t.elapsed() / 5, ram_used(&mut sys));
        let t = Instant::now();
        let nvml = nvml_wrapper::Nvml::init();
        println!("nvml init: {:?} ({})", t.elapsed(), nvml.is_ok());
        if let Ok(n) = nvml {
            let t = Instant::now();
            for _ in 0..5 {
                let d = n.device_by_index(0).unwrap();
                let _ = d.utilization_rates();
                let _ = d.temperature(nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu);
                let _ = d.memory_info();
            }
            println!("nvml readings: {:?}", t.elapsed() / 5);
        }
    }

    #[test]
    fn metrics_read_and_format() {
        let s = Sample { cpu: 12.4, cpu_temp: None, ram: 50.0, gpu: None, gpu_temp: Some(61.2), vram: None };
        assert_eq!(s.get(Metric::CpuTemp), None);
        assert_eq!(Metric::CpuTemp.format(70.4), "70°C");
        assert_eq!(Metric::from_param("cpuTemp"), Metric::CpuTemp);
        assert_eq!(s.get(Metric::Cpu), Some(12.4));
        assert_eq!(s.get(Metric::Gpu), None);
        assert_eq!(Metric::GpuTemp.format(61.2), "61°C");
        assert_eq!(Metric::from_param("vram"), Metric::Vram);
        assert_eq!(Metric::from_param("nonsense"), Metric::Cpu);
    }
}
