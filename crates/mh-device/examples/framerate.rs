//! How fast the key panel and the dial screen take new pictures: the budget for animations and
//! games. Sends moving frames for a few seconds and prints frames per second.
//!
//!     cargo run --release -p mh-device --example framerate -- [seconds per test] [jpeg quality]
//!
//! Quit HubDeck first: it redraws the deck and would mix its pictures in.

use std::io::Cursor;
use std::time::{Duration, Instant};

use image::codecs::jpeg::JpegEncoder;
use image::{Rgb, RgbImage};
use mh_device::{Hub, Module};
use mh_protocol::ModuleKind;
use mh_protocol::report;

fn jpeg(img: &RgbImage, quality: u8) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    JpegEncoder::new_with_quality(&mut out, quality).encode_image(img).expect("encode jpeg");
    out.into_inner()
}

/// A bar sweeping across a dark background, different in every frame.
fn frame(w: u32, h: u32, i: u32) -> RgbImage {
    let x0 = (i * 7) % w;
    RgbImage::from_fn(w, h, |x, y| if x.abs_diff(x0) < 12 { Rgb([240, 200, 40]) } else { Rgb([(x * 255 / w) as u8 / 4, (y * 255 / h) as u8 / 4, 40]) })
}

/// Frames are encoded before timing, so only the transfer is measured.
fn run(m: &Module, w: u32, h: u32, quality: u8, seconds: u64, panel: bool) -> (f64, usize) {
    let frames: Vec<Vec<u8>> = (0..60).map(|i| jpeg(&frame(w, h, i), quality)).collect();
    let avg = frames.iter().map(Vec::len).sum::<usize>() / frames.len();
    let start = Instant::now();
    let mut n = 0u32;
    while start.elapsed() < Duration::from_secs(seconds) {
        let data = &frames[(n % 60) as usize];
        let reports = if panel { report::panel_image(data, m.output_len) } else { report::dial_image(data, m.output_len) };
        m.send_all(&reports).expect("send");
        n += 1;
    }
    (n as f64 / start.elapsed().as_secs_f64(), avg)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seconds: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(4);
    let quality: u8 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(80);
    let hub = Hub::start().expect("hub");
    std::thread::sleep(Duration::from_millis(500));
    let keys = hub.first(ModuleKind::Keys);
    let dial = hub.first(ModuleKind::Dial);
    if let Some(k) = &keys {
        let (fps, size) = run(k, 480, 272, quality, seconds, true);
        println!("key panel 480x272 JPEG q{quality} (~{} KB): {fps:.1} fps", size / 1024);
    }
    if let Some(d) = &dial {
        let (fps, size) = run(d, 240, 240, quality, seconds, false);
        println!("dial 240x240 JPEG q{quality} (~{} KB): {fps:.1} fps", size / 1024);
    }
    if let (Some(k), Some(d)) = (keys, dial) {
        // both at once from two threads: separate USB devices behind the base's hub
        let t = std::thread::spawn(move || run(&d, 240, 240, quality, seconds, false));
        let (kf, _) = run(&k, 480, 272, quality, seconds, true);
        let (df, _) = t.join().expect("dial thread");
        println!("together: key panel {kf:.1} fps + dial {df:.1} fps");
    }
    drop(hub); // keys and dial back to their own animation
}
