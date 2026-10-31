//! A frame counter for the camera (2026-10-26: does the key screen show every picture, and does it
//! show them whole when reports go two at a time?): every picture carries its number as ten
//! squares on the top row (two a key, white = 1), a white bar that moves 17 px a picture on the
//! middle row, and busy snow on the bottom row so a picture weighs what a snowy deck does (q98).
//! First a calibration picture (all ten squares white) for 1.5 s, then pictures as fast as they
//! go for the given seconds, then black.
//!
//!     cargo run --release -p mh-device --example counter -- [seconds]
//!     (HUBDECK_PIPELINE=0 for one report at a time)
//!
//! Quit HubDeck first. The keys go back to their own animation at the end.

use std::io::Cursor;
use std::time::{Duration, Instant};

use image::codecs::jpeg::JpegEncoder;
use image::{Rgb, RgbImage};
use mh_device::Hub;
use mh_protocol::{ModuleKind, report};

fn picture(n: u32, calibrate: bool) -> Vec<u8> {
    let mut img = RgbImage::from_pixel(480, 272, Rgb([0, 0, 0]));
    let mut fill = |x0: u32, y0: u32, w: u32, h: u32, c: [u8; 3]| {
        for y in y0..(y0 + h).min(272) {
            for x in x0..(x0 + w).min(480) {
                img.put_pixel(x, y, Rgb(c));
            }
        }
    };
    for bit in 0..10u32 {
        let (key, half) = (bit / 2, bit % 2);
        let on = calibrate || (n >> (9 - bit)) & 1 == 1;
        fill(key * 100 + 8 + half * 35, 18, 28, 40, if on { [255, 255, 255] } else { [0, 0, 0] });
    }
    if !calibrate {
        fill((n * 17) % 470, 104, 10, 66, [255, 220, 60]);
        let mut seed = 0x9e37_79b9u32 ^ n.wrapping_mul(2_654_435_761);
        let mut rnd = || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed
        };
        for _ in 0..260 {
            let (x, y, r) = (rnd() % 480, 198 + rnd() % 74, 1 + rnd() % 3);
            fill(x.saturating_sub(r), y.saturating_sub(r), 2 * r, 2 * r, [235, 242, 255]);
        }
    }
    let mut out = Cursor::new(Vec::new());
    JpegEncoder::new_with_quality(&mut out, 98).encode_image(&img).expect("jpeg");
    out.into_inner()
}

fn main() {
    let seconds: f64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(6.0);
    let pipelined = std::env::var("HUBDECK_PIPELINE").map_or(true, |v| v != "0");
    let hub = Hub::start().expect("hub");
    std::thread::sleep(Duration::from_millis(800));
    let keys = hub.first(ModuleKind::Keys).expect("the keys module");
    let pictures: Vec<Vec<u8>> = (0..512).map(|n| picture(n, false)).collect();
    let avg = pictures.iter().map(Vec::len).sum::<usize>() / pictures.len();
    keys.send_all(&report::panel_image(&picture(0, true), keys.output_len)).expect("send");
    std::thread::sleep(Duration::from_millis(1500));
    let start = Instant::now();
    let mut n = 0usize;
    while start.elapsed().as_secs_f64() < seconds {
        keys.send_all(&report::panel_image(&pictures[n % 512], keys.output_len)).expect("send");
        n += 1;
    }
    let rate = n as f64 / start.elapsed().as_secs_f64();
    let black = {
        let mut out = Cursor::new(Vec::new());
        JpegEncoder::new_with_quality(&mut out, 98).encode_image(&RgbImage::new(480, 272)).expect("jpeg");
        out.into_inner()
    };
    keys.send_all(&report::panel_image(&black, keys.output_len)).expect("send");
    println!("{}: {n} pictures (~{} KB) in {seconds} s = {rate:.1} a second sent", if pipelined { "two at a time" } else { "one at a time" }, avg / 1024);
    std::thread::sleep(Duration::from_millis(1000));
    drop(hub);
}
