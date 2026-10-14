//! Does handing the dial back to its firmware right after streaming pictures leave it stuck for the
//! next start? (2026-10-14, macOS: the dial refused every command after HubDeck restarted while
//! Mochi was animating on it, twice; restarts without an animation went fine.)
//!
//! Each cycle starts the hub, waits for the dial, streams pictures for a few seconds (or not),
//! hands everything back (drops the hub), waits, and starts again.
//!
//!     cargo run --release -p mh-device --example handback -- <cycles> <stream: yes|no> <gap seconds>
//!
//! Quit HubDeck first. A cycle that never sees the dial again means it got stuck.

use std::io::Cursor;
use std::time::{Duration, Instant};

use image::codecs::jpeg::JpegEncoder;
use image::{Rgb, RgbImage};
use mh_device::Hub;
use mh_protocol::ModuleKind;
use mh_protocol::report;

fn jpeg(i: u32) -> Vec<u8> {
    let x0 = (i * 9) % 240;
    let img = RgbImage::from_fn(240, 240, |x, y| if x.abs_diff(x0) < 14 { Rgb([180, 160, 255]) } else { Rgb([20, (y / 4) as u8, 50]) });
    let mut out = Cursor::new(Vec::new());
    JpegEncoder::new_with_quality(&mut out, 90).encode_image(&img).expect("encode jpeg");
    out.into_inner()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cycles: u32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(4);
    let stream = args.get(2).is_none_or(|s| s == "yes");
    let gap: f64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(6.0);
    let frames: Vec<Vec<u8>> = (0..40).map(jpeg).collect();
    for cycle in 1..=cycles {
        let started = Instant::now();
        let hub = Hub::start().expect("hub");
        let mut dial = None;
        while started.elapsed() < Duration::from_secs(25) {
            dial = hub.first(ModuleKind::Dial);
            if dial.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        let Some(d) = dial else {
            println!("cycle {cycle}: the dial did not come up in 25 s (stuck)");
            hub.close();
            std::thread::sleep(Duration::from_secs_f64(gap));
            continue;
        };
        println!("cycle {cycle}: dial up after {:.1} s", started.elapsed().as_secs_f64());
        if stream {
            let s = Instant::now();
            let mut n = 0;
            while s.elapsed() < Duration::from_secs(3) {
                if let Err(e) = d.send_all(&report::dial_image(&frames[n % frames.len()], d.output_len)) {
                    println!("  send: {e}");
                    break;
                }
                n += 1;
                std::thread::sleep(Duration::from_millis(30));
            }
            println!("  streamed {n} pictures");
        }
        drop(d);
        hub.close();
        println!("  handed back; waiting {gap} s");
        std::thread::sleep(Duration::from_secs_f64(gap));
    }
}
