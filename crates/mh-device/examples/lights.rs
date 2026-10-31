//! The keys' documented commands HubDeck has not used yet, watched on the deck (2026-10-26,
//! PROTOCOL.md > What the hardware offers): a key filled with one colour (`03 37`), the whole
//! screen filled (`03 36`), how long each takes, whether a picture after them shows again, and
//! the backlight (`03 34`) changed fast enough for a flash (a lightning: 100 -> 20 -> 100), with
//! the time a change takes. Never brightness 0 (PROTOCOL.md: unsafe on macOS).
//!
//!     cargo run --release -p mh-device --example lights
//!
//! Quit HubDeck first. The keys go back to their own animation at the end.

use std::io::Cursor;
use std::time::{Duration, Instant};

use image::codecs::jpeg::JpegEncoder;
use image::{Rgb, RgbImage};
use mh_device::Hub;
use mh_protocol::{ModuleKind, report};

fn panel(c: [u8; 3]) -> Vec<u8> {
    let img = RgbImage::from_fn(480, 272, |x, y| if (x / 24 + y / 24) % 2 == 0 { Rgb(c) } else { Rgb([20, 20, 28]) });
    let mut out = Cursor::new(Vec::new());
    JpegEncoder::new_with_quality(&mut out, 98).encode_image(&img).expect("jpeg");
    out.into_inner()
}

fn timed(what: &str, f: impl FnOnce()) {
    let t = Instant::now();
    f();
    println!("{:>8.1} ms  {what}", t.elapsed().as_secs_f64() * 1000.0);
}

fn main() {
    let hub = Hub::start().expect("hub");
    std::thread::sleep(Duration::from_millis(800));
    let keys = hub.first(ModuleKind::Keys).expect("the keys module");
    let pause = |s: f32| std::thread::sleep(Duration::from_secs_f32(s));
    let send = |r: Option<mh_protocol::Report>| keys.send(&r.expect("a report")).expect("send");

    println!("[{:?}] 1. a blue checkered picture", Instant::now());
    timed("panel picture", || keys.send_all(&report::panel_image(&panel([60, 120, 230]), keys.output_len)).expect("send"));
    pause(2.0);
    println!("2. key 8 (middle of the middle row) red, by the key colour command");
    timed("fill key 7 red", || send(report::fill_key(7, [230, 40, 40])));
    pause(2.5);
    println!("3. the whole screen green, by the screen colour command");
    timed("fill screen green", || send(report::fill_screen(ModuleKind::Keys, [40, 200, 90])));
    pause(2.5);
    println!("4. the picture again (does it come back after a fill?)");
    timed("panel picture", || keys.send_all(&report::panel_image(&panel([230, 160, 40]), keys.output_len)).expect("send"));
    pause(2.0);
    println!("5. five quick flashes of the backlight (100 -> 20 -> 100), then ten a second for 2 s");
    for _ in 0..5 {
        timed("backlight 20", || send(report::backlight(ModuleKind::Keys, 20)));
        pause(0.15);
        timed("backlight 100", || send(report::backlight(ModuleKind::Keys, 100)));
        pause(0.6);
    }
    let t = Instant::now();
    let mut n = 0;
    while t.elapsed() < Duration::from_secs(2) {
        send(report::backlight(ModuleKind::Keys, if n % 2 == 0 { 30 } else { 100 }));
        n += 1;
        pause(0.05);
    }
    send(report::backlight(ModuleKind::Keys, 100));
    println!("{n} backlight changes in 2 s, back at 100");
    pause(1.0);
    drop(hub); // the keys back to their own animation
}
