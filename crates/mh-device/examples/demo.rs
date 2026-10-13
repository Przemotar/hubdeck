//! Interactive hardware check, same as the Python prototype's layout test.
//!
//!     cargo run -p mh-device --example demo -- [seconds]
//!
//! keys: colored tiles, a pressed key turns yellow; dial screen: a ring showing a value
//! 0..=100 changed by the dial (1), knobs (5) and rollers (1), dial press resets it;
//! knob and fader LEDs follow the value; base edge goes cyan. Pull a module off and put it
//! back: it reconnects and redraws by itself.

use std::io::Cursor;
use std::time::{Duration, Instant};

use image::{ImageFormat, Rgb, RgbImage};
use mh_device::{Event, Hub, Module};
use mh_protocol::ModuleKind;
use mh_protocol::module::KEY_PX;
use mh_protocol::report::{self, LedMode, LedTarget};

fn jpeg(img: &RgbImage) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, ImageFormat::Jpeg).expect("encode jpeg");
    out.into_inner()
}

fn value_color(v: u8) -> [u8; 3] {
    [((100 - v as u32) * 255 / 100) as u8, (v as u32 * 255 / 100) as u8, 40]
}

fn key_tile(index: u8, pressed: bool) -> RgbImage {
    let base = if pressed { [250, 200, 0] } else { [20, (30 + index as u32 * 11) as u8, (90 - index as u32 * 3) as u8] };
    // no font: (row + 1) x (column + 1) white dots tell the keys apart
    let (rows, cols) = (index as u32 / 5 + 1, index as u32 % 5 + 1);
    RgbImage::from_fn(KEY_PX, KEY_PX, |x, y| {
        let border = x < 3 || y < 3 || x >= KEY_PX - 3 || y >= KEY_PX - 3;
        let (gx, gy) = (x.wrapping_sub(8), y.wrapping_sub(8));
        let dot = gx < 60 && gy < 60 && gx / 12 < cols && gy / 12 < rows && gx % 12 < 8 && gy % 12 < 8;
        if border || dot { Rgb([255, 255, 255]) } else { Rgb(base) }
    })
}

fn dial_ring(value: u8) -> RgbImage {
    let c = value_color(value);
    RgbImage::from_fn(240, 240, |x, y| {
        let (dx, dy) = (x as f32 - 120.0, y as f32 - 120.0);
        let r = (dx * dx + dy * dy).sqrt();
        let angle = (dx.atan2(-dy).to_degrees() + 360.0) % 360.0;
        if (95.0..112.0).contains(&r) {
            if angle <= value as f32 * 3.6 { Rgb(c) } else { Rgb([50, 50, 70]) }
        } else if r < 80.0 * value as f32 / 100.0 {
            Rgb([c[0] / 3, c[1] / 3, c[2] / 3])
        } else {
            Rgb([10, 10, 20])
        }
    })
}

fn draw(m: &Module, value: u8) -> mh_device::Result<()> {
    let led = value_color(value);
    match m.kind() {
        ModuleKind::Keys => {
            for i in 0..15u8 {
                m.send_all(&report::key_image(i, &jpeg(&key_tile(i, false)), m.output_len))?;
            }
        }
        ModuleKind::Dial => m.send_all(&report::dial_image(&jpeg(&dial_ring(value)), m.output_len))?,
        ModuleKind::Knobs | ModuleKind::Faders => {
            m.send(&report::led_mode(m.kind(), LedTarget::All, LedMode::Static, false).unwrap())?;
            m.send(&report::led_color(m.kind(), LedTarget::All, led, false).unwrap())?;
        }
        ModuleKind::Base => {
            m.send(&report::led_mode(m.kind(), LedTarget::All, LedMode::Static, false).unwrap())?;
            m.send(&report::led_color(m.kind(), LedTarget::All, [0, 200, 255], false).unwrap())?;
        }
        ModuleKind::Rollers => {}
    }
    Ok(())
}

fn main() -> mh_device::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let seconds: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(60);
    let hub = Hub::start()?;
    let mut value: u8 = 50;
    for m in hub.modules() {
        draw(&m, value)?;
    }
    println!("running {seconds}s with {:?}", hub.modules().iter().map(|m| m.kind().name()).collect::<Vec<_>>());
    let end = Instant::now() + Duration::from_secs(seconds);
    while Instant::now() < end {
        let Ok(ev) = hub.events().recv_timeout(Duration::from_millis(200)) else { continue };
        println!("{ev:?}");
        let before = value;
        match &ev {
            Event::Connected(id) => {
                if let Some(m) = hub.module(id) {
                    draw(&m, value)?;
                }
            }
            Event::KeyDown { module, index } | Event::KeyUp { module, index } => {
                if let Some(m) = hub.module(module) {
                    let tile = key_tile(*index, matches!(ev, Event::KeyDown { .. }));
                    m.send_all(&report::key_image(*index, &jpeg(&tile), m.output_len))?;
                }
            }
            Event::Dial { delta, .. } => value = (value as i16 + *delta as i16).clamp(0, 100) as u8,
            Event::Knob { delta, .. } => value = (value as i16 + 5 * *delta as i16).clamp(0, 100) as u8,
            Event::Roller { delta, .. } => value = (value as i16 + *delta as i16).clamp(0, 100) as u8,
            Event::Fader { value: v, .. } => value = *v,
            Event::DialDown { .. } => value = 50,
            _ => {}
        }
        if value != before {
            for kind in [ModuleKind::Dial, ModuleKind::Knobs, ModuleKind::Faders] {
                if let Some(m) = hub.first(kind) {
                    draw(&m, value)?;
                }
            }
        }
    }
    hub.close();
    println!("done; keys and dial back in hardware mode");
    Ok(())
}
