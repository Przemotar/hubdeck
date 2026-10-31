//! What a press looks like on the key screen, two ways, for the camera (A4, owner's "4 a",
//! 2026-10-26): the owner's own key panel picture, then every 1.5 s key 5 ("Lubię to") "pressed":
//!   a) as HubDeck does it now: the key's picture brightened, sent as a picture (a JPEG);
//!   b) first the key's solid colour (`03 37`, 14 ms, no JPEG) in its own accent, then the
//!      brightened picture, then the plain picture again.
//!
//!     cargo run --release -p mh-device --example press -- <panel.jpg> <a|b> [presses]
//!
//! Quit HubDeck first; the keys go back to the panel picture at the end.

use std::io::Cursor;
use std::time::Duration;

use image::codecs::jpeg::JpegEncoder;
use mh_device::Hub;
use mh_protocol::{ModuleKind, report};

const SLOT: [(u32, u32); 15] = [(5, 1), (104, 1), (203, 1), (303, 1), (402, 1), (5, 101), (104, 101), (203, 101), (303, 101), (402, 101), (5, 199), (104, 199), (203, 199), (303, 199), (402, 199)];

fn jpeg(img: &image::RgbImage) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    JpegEncoder::new_with_quality(&mut out, 98).encode_image(img).expect("jpeg");
    out.into_inner()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let panel = image::open(&args[1]).expect("a panel picture").to_rgb8();
    let variant = args.get(2).map(String::as_str).unwrap_or("a");
    let presses: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(4);
    let key = 5usize;
    // the pressed key: its square brightened toward white, as the press flash draws it
    let mut pressed = panel.clone();
    let (sx, sy) = SLOT[key];
    for y in sy..sy + 72 {
        for x in sx..sx + 72 {
            let p = pressed.get_pixel_mut(x, y);
            for c in p.0.iter_mut() {
                *c = (*c as f32 + (255.0 - *c as f32) * 0.28) as u8;
            }
        }
    }
    // the key's accent: its brightest colourful pixel
    let accent = (sy..sy + 72)
        .flat_map(|y| (sx..sx + 72).map(move |x| (x, y)))
        .map(|(x, y)| panel.get_pixel(x, y).0)
        .max_by_key(|c| {
            let (mx, mn) = (*c.iter().max().unwrap() as i32, *c.iter().min().unwrap() as i32);
            (mx - mn) * 2 + mx
        })
        .unwrap_or([120, 200, 255]);
    let (plain, lit) = (jpeg(&panel), jpeg(&pressed));
    let hub = Hub::start().expect("hub");
    std::thread::sleep(Duration::from_millis(800));
    let keys = hub.first(ModuleKind::Keys).expect("the keys module");
    keys.send_all(&report::panel_image(&plain, keys.output_len)).expect("send");
    std::thread::sleep(Duration::from_millis(1500));
    for _ in 0..presses {
        if variant == "b"
            && let Some(r) = report::fill_key(key as u8, accent)
        {
            keys.send(&r).expect("key colour");
        }
        keys.send_all(&report::panel_image(&lit, keys.output_len)).expect("send");
        std::thread::sleep(Duration::from_millis(250));
        keys.send_all(&report::panel_image(&plain, keys.output_len)).expect("send");
        std::thread::sleep(Duration::from_millis(1250));
    }
    println!("{presses} presses of key {key}, variant {variant}, accent {accent:?}");
    std::thread::sleep(Duration::from_millis(300));
    drop(hub);
}
