//! macOS: can the key panel take a full-quality picture faster when its reports are written
//! several at a time? (2026-10-30: on the Mac a q98 panel of Tokyo Rain took 121-133 ms, ~8
//! pictures a second; hidapi waits for each 320-byte report.) Sends the same busy pictures (as
//! `pipeline.rs` on Windows: a snowy panel, q98, and a denser one) for a few seconds each way:
//! hidapi one by one as HubDeck does, then `mh_device::pictures` with 1, 2, 4 and 8 in flight on a
//! device object of its own; prints the time a picture takes, pictures a second and CPU.
//!
//!     cargo run --release -p mh-device --example pipeline_mac -- [seconds per test]
//!
//! Quit HubDeck first (it holds the modules exclusively); the keys go back to their own animation
//! at the end. macOS only.

#[cfg(target_os = "macos")]
fn main() {
    mac::main();
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("macOS only");
}

#[cfg(target_os = "macos")]
mod mac {
    use std::io::Cursor;
    use std::time::{Duration, Instant};

    use image::codecs::jpeg::JpegEncoder;
    use image::{Rgb, RgbImage};
    use mh_device::Hub;
    use mh_device::pictures::Pipelined;
    use mh_protocol::{ModuleKind, Report, report};

    /// This process's CPU time so far (user + system), ms.
    fn cpu_ms() -> f64 {
        // SAFETY: getrusage fills the struct it is given.
        let mut u: Rusage = unsafe { std::mem::zeroed() };
        unsafe { getrusage(0, &mut u) };
        let t = |v: Timeval| v.sec as f64 * 1000.0 + v.usec as f64 / 1000.0;
        t(u.ru_utime) + t(u.ru_stime)
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Timeval {
        sec: i64,
        usec: i32,
    }
    #[repr(C)]
    struct Rusage {
        ru_utime: Timeval,
        ru_stime: Timeval,
        _rest: [i64; 14],
    }
    unsafe extern "C" {
        fn getrusage(who: i32, usage: *mut Rusage) -> i32;
    }

    /// A busy picture like snow on the owner's deck (`flakes` of them): dark keys, light flakes.
    fn frame(i: u32, flakes: u32) -> RgbImage {
        let mut img = RgbImage::from_fn(480, 272, |x, y| {
            let key = (x % 100) < 79 && (y % 99) < 75;
            if key { Rgb([27, 26, 32]) } else { Rgb([0, 0, 0]) }
        });
        let mut seed = 0x9e37_79b9u32 ^ i.wrapping_mul(2_654_435_761);
        let mut rnd = || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed
        };
        for _ in 0..flakes {
            let (cx, cy, r) = ((rnd() % 480) as i32, (rnd() % 272) as i32, 1 + (rnd() % 4) as i32);
            let c = Rgb([120 + (rnd() % 135) as u8, 140 + (rnd() % 115) as u8, 255]);
            for y in cy - r..=cy + r {
                for x in cx - r..=cx + r {
                    if (0..480).contains(&x) && (0..272).contains(&y) && (x - cx).pow(2) + (y - cy).pow(2) <= r * r {
                        img.put_pixel(x as u32, y as u32, c);
                    }
                }
            }
        }
        img
    }

    fn jpeg(img: &RgbImage) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        JpegEncoder::new_with_quality(&mut out, 98).encode_image(img).expect("encode jpeg");
        out.into_inner()
    }

    fn bytes(r: &Report) -> &[u8] {
        let Report::Output(b) = r else { panic!("an output report") };
        b
    }

    pub fn main() {
        let seconds: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(4);
        let api = hidapi::HidApi::new().expect("hidapi");
        let path = api.device_list().find(|d| d.vendor_id() == 0x2516 && ModuleKind::from_product_id(d.product_id()) == Some(ModuleKind::Keys)).expect("keys in the list").path().to_owned();
        // `MH_FIRST=1`: a device object of our own opened before the hub takes the modules
        // exclusively; does it still write?
        let first = std::env::var_os("MH_FIRST").map(|_| Pipelined::open_with(&path, 8));
        if let Some(Err(e)) = &first {
            println!("opened first: {e}");
        }
        let hub = Hub::start().expect("hub");
        std::thread::sleep(Duration::from_millis(1500));
        let keys = hub.first(ModuleKind::Keys).expect("the keys module");
        if let Some(Ok(mut p)) = first {
            let frames: Vec<Vec<Report>> = (0..30).map(|i| report::panel_image(&jpeg(&frame(i, 140)), keys.output_len)).collect();
            let cpu0 = cpu_ms();
            let start = Instant::now();
            let mut n = 0usize;
            while start.elapsed() < Duration::from_secs(seconds) {
                if let Err(e) = p.write_all(frames[n % frames.len()].iter().map(bytes), keys.output_len) {
                    println!("opened first, after the hub: {e}");
                    break;
                }
                n += 1;
            }
            let ms = start.elapsed().as_secs_f64() * 1000.0 / n.max(1) as f64;
            println!("opened first, 8 in flight beside the exclusive hub: {n} pictures, {ms:6.1} ms a picture, CPU {:.2} ms", (cpu_ms() - cpu0) / n.max(1) as f64);
            // the hub's own reads still come (a press on the keys while this runs)
            drop(hub);
            return;
        }
        for flakes in [140, 900] {
            let frames: Vec<Vec<u8>> = (0..30).map(|i| jpeg(&frame(i, flakes))).collect();
            let avg = frames.iter().map(Vec::len).sum::<usize>() / frames.len();
            let reports: Vec<Vec<Report>> = frames.iter().map(|f| report::panel_image(f, keys.output_len)).collect();
            println!("pictures: q98, ~{} KB, {} reports of {} B each", avg / 1024, reports[0].len(), keys.output_len);

            // hidapi, one report at a time (HubDeck today)
            let cpu0 = cpu_ms();
            let start = Instant::now();
            let mut n = 0usize;
            while start.elapsed() < Duration::from_secs(seconds) {
                keys.send_all(&reports[n % reports.len()]).expect("send");
                n += 1;
            }
            let ms = start.elapsed().as_secs_f64() * 1000.0 / n as f64;
            println!("  hidapi one by one: {ms:6.1} ms a picture, {:5.1} a second, CPU {:.2} ms a picture", 1000.0 / ms, (cpu_ms() - cpu0) / n as f64);

            for depth in [1, 2, 4, 8, 16] {
                let mut p = match Pipelined::open_with(&path, depth) {
                    Ok(p) => p,
                    Err(e) => {
                        println!("  {depth} in flight: {e}");
                        continue;
                    }
                };
                let cpu0 = cpu_ms();
                let start = Instant::now();
                let mut n = 0usize;
                while start.elapsed() < Duration::from_secs(seconds) {
                    p.write_all(reports[n % reports.len()].iter().map(bytes), keys.output_len).expect("write");
                    n += 1;
                }
                let ms = start.elapsed().as_secs_f64() * 1000.0 / n as f64;
                println!("  {depth:2} in flight:      {ms:6.1} ms a picture, {:5.1} a second, CPU {:.2} ms a picture", 1000.0 / ms, (cpu_ms() - cpu0) / n as f64);
            }
        }
        drop(hub); // the keys back to their own animation
    }
}
