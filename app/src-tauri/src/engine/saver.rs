//! The screensaver (roadmap phase 12 G; the assistant's advice the owner passed on, 2026-10-12):
//! after some idle minutes a calm effect across all 15 keys, drawn here from the clock (no
//! pictures), gone at the first touch, which only wakes the deck. Mood when you are away, nothing
//! moving while you work. The window draws the same effects itself (src/saver.ts): keep the two alike.

/// The effects (`Settings > Screens > Screensaver`).
pub const EFFECTS: [&str; 3] = ["rain", "stars", "waves"];
/// A frame every this often while it runs (the whole panel is one image): ~15 a second (owner,
/// 2026-10-14: five a second looked choppy).
pub const FRAME_MS: u64 = 66;

/// Seconds since the screensaver's clock started (the effects move with it).
pub fn clock() -> f32 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_secs_f32()
}

/// A steady pseudo-random number in 0..1 for `i` (the same every frame).
fn rnd(i: u32, salt: u32) -> f32 {
    let mut x = i.wrapping_mul(2_654_435_761) ^ salt.wrapping_mul(40_503);
    x ^= x >> 15;
    x = x.wrapping_mul(2_246_822_519);
    x ^= x >> 13;
    (x % 10_000) as f32 / 10_000.0
}

/// One frame of `effect` at `t` seconds, in `color` (#rrggbb), as the content of the 480x272
/// panel; None for an unknown effect.
pub fn body(effect: &str, t: f32, color: &str) -> Option<String> {
    let (w, h) = (super::render::PANEL_W as f32, super::render::PANEL_H as f32);
    let mut out = String::new();
    match effect {
        // falling columns of glyphs, the head bright, the tail fading
        "rain" => {
            const GLYPHS: [char; 16] = ['0', '1', '7', 'Z', 'A', 'X', '$', '#', '%', '@', '*', '+', '=', '[', ']', '?'];
            let cols = 24;
            let step = 16.0;
            for c in 0..cols {
                let speed = 40.0 + rnd(c, 1) * 70.0;
                let tail = 6 + (rnd(c, 2) * 8.0) as u32;
                let head = (t * speed + rnd(c, 3) * (h + 200.0)) % (h + tail as f32 * step + 40.0);
                let x = (c as f32 + 0.5) * w / cols as f32;
                for k in 0..tail {
                    let y = head - k as f32 * step;
                    if !(0.0..h + step).contains(&y) {
                        continue;
                    }
                    let row = (y / step) as u32;
                    let g = GLYPHS[((rnd(c * 97 + row, 4) * 16.0) as usize + (t * 3.0) as usize * (k == 0) as usize) % 16];
                    let alpha = if k == 0 { 1.0 } else { 0.85 * (1.0 - k as f32 / tail as f32) };
                    let fill = if k == 0 { "#e8ffe8" } else { color };
                    out += &format!(r#"<text x="{x:.1}" y="{y:.1}" font-size="14" text-anchor="middle" fill="{fill}" fill-opacity="{alpha:.2}" font-family="IBM Plex Sans">{g}</text>"#);
                }
            }
        }
        // a still night sky: stars twinkling at their own pace, now and then a shooting star
        "stars" => {
            for i in 0..70u32 {
                let (x, y) = (rnd(i, 5) * w, rnd(i, 6) * h);
                let r = 0.6 + rnd(i, 7) * 1.6;
                let a = 0.25 + 0.75 * (0.5 + 0.5 * (t * (0.6 + rnd(i, 8) * 1.8) + rnd(i, 9) * std::f32::consts::TAU).sin());
                out += &format!(r##"<circle cx="{x:.1}" cy="{y:.1}" r="{r:.2}" fill="#ffffff" fill-opacity="{a:.2}"/>"##);
            }
            // every 9 s a shooting star crosses for a second
            let phase = t % 9.0;
            if phase < 1.0 {
                let n = (t / 9.0) as u32;
                let (x0, y0) = (rnd(n, 10) * w * 0.6, rnd(n, 11) * h * 0.4);
                let (x, y) = (x0 + phase * 220.0, y0 + phase * 90.0);
                out += &format!(
                    r#"<line x1="{:.1}" y1="{:.1}" x2="{x:.1}" y2="{y:.1}" stroke="{color}" stroke-width="2" stroke-linecap="round" stroke-opacity="{:.2}"/>"#,
                    x - 40.0,
                    y - 16.0,
                    1.0 - phase
                );
            }
        }
        // slow waves in the profile's color
        "waves" => {
            for layer in 0..3 {
                let amp = 18.0 + layer as f32 * 8.0;
                let base = h * (0.45 + layer as f32 * 0.17);
                let speed = 0.5 + layer as f32 * 0.25;
                let mut d = format!("M0 {h}");
                let mut x = 0.0;
                while x <= w {
                    let y = base + amp * ((x / w * std::f32::consts::TAU * (1.2 + layer as f32 * 0.4)) + t * speed).sin();
                    d += &format!(" L{x:.0} {y:.1}");
                    x += 12.0;
                }
                d += &format!(" L{w} {h} Z");
                out += &format!(r#"<path d="{d}" fill="{color}" fill-opacity="{:.2}"/>"#, 0.18 + layer as f32 * 0.12);
            }
        }
        _ => return None,
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rnd_matches_the_window() {
        // the window draws the screensaver itself (src/saver.ts, pinned there too)
        let got: Vec<f32> = [(0, 1), (3, 1), (97, 4), (69, 9), (1234, 11)].iter().map(|&(i, s)| rnd(i, s)).collect();
        assert_eq!(got, [0.9995, 0.7379, 0.492, 0.1734, 0.0383]);
    }

    #[test]
    fn every_effect_draws_and_moves() {
        for e in EFFECTS {
            let a = body(e, 1.0, "#7c6cf0").unwrap();
            let b = body(e, 1.4, "#7c6cf0").unwrap();
            assert!(!a.is_empty(), "{e} draws");
            assert_ne!(a, b, "{e} moves");
        }
        assert!(body("nope", 0.0, "#fff").is_none());
    }

    /// `cargo test -p hubdeck saver_frames -- --ignored`: one frame of each effect as a PNG in
    /// the temp folder, to look at.
    #[test]
    #[ignore]
    fn saver_frames() {
        let r = super::super::render::Renderer::default();
        for e in EFFECTS {
            let svg = super::super::render::panel_body_svg(&body(e, 3.3, "#7c6cf0").unwrap(), 0);
            let png = r.png(&svg).expect("renders");
            std::fs::write(std::env::temp_dir().join(format!("saver-{e}.png")), png).unwrap();
        }
    }
}
