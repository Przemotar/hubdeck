//! Start and stop the hub again and again, as HubDeck does at each restart, and count the cycles
//! where keys or dial do not come back (macOS wedges, 2026-10-13). The app must not be running.
//!
//!     cargo run -p mh-device --example cycle -- [cycles] [pause_ms]
//!     MH_HID_SHARED=1 cargo run ...     # open the devices shared instead of exclusively
//!
//! Each cycle: start, wait until keys and dial are connected (10 s at most), draw a few frames on
//! both (as the app does while it runs), close (which hands them back), pause.
use std::time::{Duration, Instant};

use mh_device::Hub;
use mh_protocol::module::ModuleKind;
use mh_protocol::report;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let mut args = std::env::args().skip(1);
    let cycles: usize = args.next().and_then(|a| a.parse().ok()).unwrap_or(20);
    let pause = Duration::from_millis(args.next().and_then(|a| a.parse().ok()).unwrap_or(1000));
    let frame = black_jpeg();
    for i in 1..=cycles {
        let hub = Hub::start().expect("hub");
        let started = Instant::now();
        let (mut keys, mut dial) = (None, None);
        while started.elapsed() < Duration::from_secs(10) && (keys.is_none() || dial.is_none()) {
            keys = hub.first(ModuleKind::Keys);
            dial = hub.first(ModuleKind::Dial);
            std::thread::sleep(Duration::from_millis(100));
        }
        let ok = keys.is_some() && dial.is_some();
        println!("cycle {i}: keys {} dial {} after {} ms", keys.is_some(), dial.is_some(), started.elapsed().as_millis());
        for _ in 0..10 {
            if let Some(k) = &keys {
                let _ = k.send_all(&report::panel_image(&frame, k.output_len));
            }
            if let Some(d) = &dial {
                let _ = d.send_all(&report::dial_image(&frame, d.output_len));
            }
            std::thread::sleep(Duration::from_millis(30));
        }
        hub.close();
        if !ok {
            println!("WEDGED in cycle {i}");
            return;
        }
        std::thread::sleep(pause);
    }
    println!("all {cycles} cycles fine");
}

fn black_jpeg() -> Vec<u8> {
    let img = image::RgbImage::new(240, 240);
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(img).write_to(&mut out, image::ImageFormat::Jpeg).expect("jpeg");
    out.into_inner()
}
