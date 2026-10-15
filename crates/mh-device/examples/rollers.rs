//! Why the rollers send nothing on the owner's Mac (2026-10-14: no input at all, the left roller
//! squealed; on the PC they work). Prints every raw report the rollers module sends, with the
//! time, and on macOS the module's USB power state from `ioreg` every few seconds. With
//! `--keepalive <ms>` it reads the module's firmware version that often (a feature read, safe on
//! the rollers: they are never in a hardware mode that wedges), to see whether a module that
//! hears from the computer keeps sending. Quit HubDeck first on macOS (it opens modules
//! exclusively); on Windows it can run alongside.
//!
//!     cargo run -p mh-device --example rollers -- [seconds] [--keepalive 1000]
//!
//! Turn both rollers both ways while it runs. Expected: `01 2a ...` reports (PROTOCOL.md, tag
//! 0x2A: `01 02 r1 r2`, 0 up, 1 down, ff none).

use std::time::{Duration, Instant};

use hidapi::HidApi;
use mh_protocol::{ModuleKind, Query, VENDOR_ID, report};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let seconds: u64 = args.iter().find_map(|a| a.parse().ok()).unwrap_or(20);
    let keepalive = args.iter().position(|a| a == "--keepalive").and_then(|i| args.get(i + 1)).and_then(|v| v.parse::<u64>().ok()).map(Duration::from_millis);
    let api = HidApi::new().expect("hidapi");
    let kind = ModuleKind::Rollers;
    let found: Vec<_> = api.device_list().filter(|d| d.vendor_id() == VENDOR_ID && d.product_id() == kind.product_id()).collect();
    if found.is_empty() {
        println!("no rollers module found (product id {:04x})", kind.product_id());
        return;
    }
    for d in &found {
        println!("found: path {:?} usage page {:#x} usage {:#x} interface {} serial {:?}", d.path(), d.usage_page(), d.usage(), d.interface_number(), d.serial_number());
    }
    let dev = match found[0].open_device(&api) {
        Ok(d) => d,
        Err(e) => {
            println!("open failed: {e} (on macOS quit HubDeck first: it opens modules exclusively)");
            return;
        }
    };
    let firmware_id = report::query_id(kind, Query::Firmware).expect("firmware query");
    let read_firmware = |label: &str| {
        let mut buf = [0u8; report::FEATURE_LEN + 1];
        buf[0] = firmware_id;
        match dev.get_feature_report(&mut buf) {
            Ok(n) => println!("{label}: firmware {:?} ({n} B)", report::parse_firmware(&buf[..n])),
            Err(e) => println!("{label}: firmware read failed: {e}"),
        }
    };
    read_firmware("start");
    power_state();
    println!("reading for {seconds} s{}; turn both rollers both ways", keepalive.map_or(String::new(), |k| format!(", a firmware read every {} ms", k.as_millis())));
    let started = Instant::now();
    let (mut last_alive, mut last_power) = (Instant::now(), Instant::now());
    let mut reports = 0u32;
    let mut buf = [0u8; 65];
    while started.elapsed() < Duration::from_secs(seconds) {
        match dev.read_timeout(&mut buf, 100) {
            Ok(0) => {}
            Ok(n) => {
                reports += 1;
                let shown = &buf[..n.min(8)];
                println!("{:7.3} s  {n:2} B  {shown:02x?}", started.elapsed().as_secs_f32());
            }
            Err(e) => {
                println!("{:7.3} s  read error: {e}", started.elapsed().as_secs_f32());
                break;
            }
        }
        if let Some(every) = keepalive
            && last_alive.elapsed() >= every
        {
            last_alive = Instant::now();
            read_firmware("keepalive");
        }
        if last_power.elapsed() >= Duration::from_secs(5) {
            last_power = Instant::now();
            power_state();
        }
    }
    println!("{reports} reports in {:.1} s", started.elapsed().as_secs_f32());
    power_state();
}

/// macOS: what IOKit says about the rollers' power (a suspended module sends nothing). Elsewhere: nothing.
fn power_state() {
    if !cfg!(target_os = "macos") {
        return;
    }
    let Ok(out) = std::process::Command::new("ioreg").args(["-r", "-c", "IOUSBHostDevice", "-l", "-w0"]).output() else { return };
    let text = String::from_utf8_lossy(&out.stdout);
    // the block of the rollers' USB device, from its name to the next device
    let Some(at) = text.find("2xRoller") else {
        println!("ioreg: no rollers device");
        return;
    };
    let block: String = text[at..]
        .lines()
        .take(80)
        .filter(|l| ["PowerState", "LPM", "Suspend", "kUSBCurrentConfiguration", "locationID", "IOPowerManagement"].iter().any(|k| l.contains(k)))
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("\n  ");
    println!("ioreg (rollers):\n  {block}");
}
