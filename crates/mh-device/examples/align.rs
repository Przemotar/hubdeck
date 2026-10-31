//! Do a key's own picture and the whole panel picture line up on the key screen? (2026-10-26: the
//! owner saw whole keys jump with slow effects, where the app switches between the whole panel and
//! the keys' squares.) One still picture of coarse noise goes out as the whole panel, then as the
//! fifteen 72x72 squares cut from it where `key_slot` says the deck shows them, in turns, for the
//! camera: a square that lands a pixel off shows as the picture jumping on that key.
//!
//!     cargo run --release -p mh-device --example align -- [rounds] [dx dy]
//!
//! `dx dy` move every cut by whole pixels (to try a correction). Quit HubDeck first; the keys go
//! black at the end.

use std::io::Cursor;
use std::time::Duration;

use image::codecs::jpeg::JpegEncoder;
use image::{Rgb, RgbImage};
use mh_device::Hub;
use mh_protocol::{ModuleKind, report};

const SLOT: u32 = 72;

/// As `render::key_slot` in the app.
fn key_slot(i: u32) -> (u32, u32) {
    ([5, 104, 203, 303, 402][(i % 5) as usize], [1, 101, 199][(i / 5) as usize])
}

fn jpeg(img: &RgbImage) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    JpegEncoder::new_with_quality(&mut out, 98).encode_image(img).expect("jpeg");
    out.into_inner()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let rounds: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(4);
    let dx: i32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    let dy: i32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(0);
    // noise in 3 px cells: sharp in every direction, so a shift of one pixel shows
    let mut seed = 0x2545_f491u32;
    let mut rnd = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed
    };
    let cells: Vec<u8> = (0..160 * 91).map(|_| if rnd() % 2 == 0 { 30 } else { 230 }).collect();
    let panel = RgbImage::from_fn(480, 272, |x, y| {
        let v = cells[(y / 3 * 160 + x / 3) as usize];
        Rgb([v, v, v])
    });
    let whole = jpeg(&panel);
    let squares: Vec<Vec<u8>> = (0..15)
        .map(|i| {
            let (sx, sy) = key_slot(i);
            let (sx, sy) = ((sx as i32 + dx).max(0) as u32, (sy as i32 + dy).max(0) as u32);
            jpeg(&image::imageops::crop_imm(&panel, sx, sy, SLOT, SLOT).to_image())
        })
        .collect();
    let hub = Hub::start().expect("hub");
    std::thread::sleep(Duration::from_millis(800));
    let keys = hub.first(ModuleKind::Keys).expect("the keys module");
    // a black start the film can find
    keys.send_all(&report::panel_image(&jpeg(&RgbImage::new(480, 272)), keys.output_len)).expect("send");
    std::thread::sleep(Duration::from_millis(1500));
    for _ in 0..rounds {
        keys.send_all(&report::panel_image(&whole, keys.output_len)).expect("send");
        std::thread::sleep(Duration::from_millis(1500));
        let reports: Vec<_> = squares.iter().enumerate().flat_map(|(i, j)| report::key_image(i as u8, j, keys.output_len)).collect();
        keys.send_all(&reports).expect("send");
        std::thread::sleep(Duration::from_millis(1500));
    }
    keys.send_all(&report::panel_image(&jpeg(&RgbImage::new(480, 272)), keys.output_len)).expect("send");
    println!("{rounds} rounds of whole / squares, cut moved by ({dx}, {dy})");
    std::thread::sleep(Duration::from_millis(500));
    drop(hub);
}
