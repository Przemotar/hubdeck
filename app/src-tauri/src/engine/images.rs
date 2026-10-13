//! The user's own pictures for keys, the dial and the full panel. Imported files are copied into
//! `<app data>/images/` under a name made from their content, so a profile keeps working when the
//! original moves. The renderer embeds them as data URIs; animated GIFs are split into frames once.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use base64::Engine as _;
use image::AnimationDecoder;

const EXTENSIONS: [&str; 6] = ["png", "jpg", "jpeg", "gif", "webp", "svg"];
const MAX_BYTES: u64 = 15 * 1024 * 1024;
/// Animation frames are scaled to fit this square; enough for the dial, small for the keys.
const FRAME_PX: u32 = 240;
const MAX_FRAMES: usize = 300;
/// GIF delays this short are played at 100 ms, as browsers do (so the window's preview and the
/// deck keep the same pace).
const BROWSER_MIN_DELAY_MS: u64 = 10;

static DIR: OnceLock<PathBuf> = OnceLock::new();
/// Cover art of what is playing now, as a data URI; the image reference `cover` draws it.
static COVER: Mutex<Option<String>> = Mutex::new(None);

/// Name under which the current cover art is drawn.
pub const COVER_REF: &str = "cover";

pub fn set_cover(cover: Option<(&str, &[u8])>) {
    *COVER.lock().unwrap_or_else(|e| e.into_inner()) = cover.map(|(mime, bytes)| data_uri(mime, bytes));
}
static CACHE: Mutex<Option<HashMap<String, Arc<Picture>>>> = Mutex::new(None);

struct Picture {
    /// The whole file (first frame of a GIF) as a data URI.
    still: String,
    /// Animated GIFs: each frame as a PNG data URI with its delay.
    frames: Vec<(String, Duration)>,
    /// A logo on a see-through background (an app's icon): drawn like an icon, not edge to edge.
    icon_like: bool,
}

/// A PNG or WebP whose corners are see-through: an icon or a logo rather than a picture.
fn looks_like_icon(name: &str, bytes: &[u8]) -> bool {
    if !(name.ends_with(".png") || name.ends_with(".webp")) {
        return false;
    }
    let Ok(img) = image::load_from_memory(bytes) else { return false };
    let img = img.to_rgba8();
    let (w, h) = img.dimensions();
    w > 0 && h > 0 && [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)].iter().all(|&(x, y)| img.get_pixel(x, y)[3] < 16)
}

/// Is this stored picture an icon or a logo (drawn centred with the label under it)?
pub fn is_icon(reference: &str) -> bool {
    load(reference.split('#').next().unwrap_or(reference)).is_some_and(|p| p.icon_like)
}

pub fn init(data_dir: &Path) {
    let _ = DIR.set(data_dir.join("images"));
}

fn dir() -> Option<&'static PathBuf> {
    DIR.get()
}

/// Store picture bytes (e.g. an app icon) under a name made from their content.
pub fn import_bytes(bytes: &[u8], ext: &str) -> Result<String, String> {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut h);
    let name = format!("{:016x}.{ext}", h.finish());
    let dir = dir().ok_or("images folder not set")?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let dest = dir.join(&name);
    if !dest.exists()
        && let Err(e) = std::fs::write(&dest, bytes)
        // the same picture written at the same moment by another thread: it is there all the same
        && !dest.exists()
    {
        return Err(e.to_string());
    }
    Ok(name)
}

/// Copy a picture into the images folder; returns the name to store in a look.
pub fn import(src: &Path) -> Result<String, String> {
    let ext = src.extension().and_then(|e| e.to_str()).map(str::to_lowercase).unwrap_or_default();
    if !EXTENSIONS.contains(&ext.as_str()) {
        return Err(format!("{ext:?} is not a supported picture (PNG, JPG, GIF, WebP or SVG)"));
    }
    let size = std::fs::metadata(src).map_err(|e| e.to_string())?.len();
    if size > MAX_BYTES {
        return Err(format!("the picture is {} MB; the limit is 15 MB", size / 1_048_576));
    }
    let bytes = std::fs::read(src).map_err(|e| e.to_string())?;
    if ext != "svg" {
        image::load_from_memory(&bytes).map_err(|e| format!("cannot read this picture: {e}"))?;
    }
    let mut h = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut h);
    let name = format!("{:016x}.{ext}", h.finish());
    let dir = dir().ok_or("images folder not set")?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let dest = dir.join(&name);
    if !dest.exists() {
        std::fs::write(&dest, &bytes).map_err(|e| e.to_string())?;
    }
    Ok(name)
}

/// Only names we created: no paths, no surprises.
fn valid(name: &str) -> bool {
    name.len() <= 40 && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '.') && EXTENSIONS.iter().any(|e| name.ends_with(&format!(".{e}")))
}

/// Pictures that came from a launcher's cache on this PC (game covers): shared profiles must not
/// carry them. Listed in `<images>/local-only.txt`, one name per line.
pub fn mark_local_only(name: &str) {
    let Some(file) = dir().map(|d| d.join("local-only.txt")) else { return };
    let mut names = std::fs::read_to_string(&file).unwrap_or_default();
    if !names.lines().any(|l| l == name) {
        names.push_str(name);
        names.push('\n');
        let _ = std::fs::write(&file, names);
    }
}

/// Did this picture come from a launcher's cache on this PC (so it stays on this PC)?
pub fn is_local_only(name: &str) -> bool {
    let name = name.split('#').next().unwrap_or(name);
    dir().and_then(|d| std::fs::read_to_string(d.join("local-only.txt")).ok()).is_some_and(|t| t.lines().any(|l| l == name))
}

/// Is this picture in the images folder (a name a look stores; `name#3` frames count as the file)?
pub fn exists(reference: &str) -> bool {
    let name = reference.split('#').next().unwrap_or(reference);
    valid(name) && dir().is_some_and(|d| d.join(name).is_file())
}

fn mime(name: &str) -> &'static str {
    match name.rsplit('.').next() {
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        _ => "image/jpeg",
    }
}

fn data_uri(mime: &str, bytes: &[u8]) -> String {
    format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))
}

fn load(name: &str) -> Option<Arc<Picture>> {
    if !valid(name) {
        return None;
    }
    if let Some(p) = CACHE.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|c| c.get(name)) {
        return Some(p.clone());
    }
    let bytes = std::fs::read(dir()?.join(name)).ok()?;
    let frames = if name.ends_with(".gif") { gif_frames(&bytes) } else { Vec::new() };
    let icon_like = looks_like_icon(name, &bytes);
    let picture = Arc::new(Picture { still: data_uri(mime(name), &bytes), frames, icon_like });
    CACHE.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).insert(name.to_string(), picture.clone());
    Some(picture)
}

fn gif_frames(bytes: &[u8]) -> Vec<(String, Duration)> {
    let Ok(decoder) = image::codecs::gif::GifDecoder::new(Cursor::new(bytes)) else { return Vec::new() };
    let mut out = Vec::new();
    for frame in decoder.into_frames().take(MAX_FRAMES) {
        let Ok(frame) = frame else { break };
        let (num, den) = frame.delay().numer_denom_ms();
        let ms = num.checked_div(den).unwrap_or(0) as u64;
        // the real delay: the engine skips frames it cannot draw in time instead of slowing down
        let delay = Duration::from_millis(if ms <= BROWSER_MIN_DELAY_MS { 100 } else { ms });
        let img = image::DynamicImage::ImageRgba8(frame.into_buffer());
        let img = if img.width() > FRAME_PX || img.height() > FRAME_PX { img.thumbnail(FRAME_PX, FRAME_PX) } else { img };
        let mut png = Cursor::new(Vec::new());
        if img.write_to(&mut png, image::ImageFormat::Png).is_ok() {
            out.push((data_uri("image/png", png.get_ref()), delay));
        }
    }
    if out.len() < 2 { Vec::new() } else { out }
}

/// The picture as a data URI for an SVG `<image>`. `name#3` is frame 3 of an animated GIF.
/// A picture a plugin sent: only base64 data URIs of image types, so nothing can break out of the
/// SVG attribute it is placed in.
pub fn safe_data_uri(s: &str) -> bool {
    let Some(rest) = s.strip_prefix("data:image/") else { return false };
    let Some((kind, data)) = rest.split_once(";base64,") else { return false };
    matches!(kind, "png" | "jpeg" | "gif" | "webp" | "svg+xml") && s.len() < 4_000_000 && data.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'='))
}

pub fn uri(reference: &str) -> Option<String> {
    if reference.starts_with("data:") {
        return safe_data_uri(reference).then(|| reference.to_string());
    }
    if reference == COVER_REF {
        return COVER.lock().unwrap_or_else(|e| e.into_inner()).clone();
    }
    let (name, frame) = match reference.split_once('#') {
        Some((n, f)) => (n, f.parse::<usize>().ok()),
        None => (reference, None),
    };
    let p = load(name)?;
    match frame {
        Some(i) if !p.frames.is_empty() => Some(p.frames[i % p.frames.len()].0.clone()),
        _ => Some(p.still.clone()),
    }
}

/// Frame delays of an animated GIF; None for a still picture.
pub fn animation(name: &str) -> Option<Vec<Duration>> {
    let p = load(name)?;
    (!p.frames.is_empty()).then(|| p.frames.iter().map(|(_, d)| *d).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_pictures_are_checked() {
        assert!(safe_data_uri("data:image/png;base64,iVBORw0KGgo="));
        assert!(!safe_data_uri("data:image/png;base64,abc\"/><script>"));
        assert!(!safe_data_uri("data:text/html;base64,PGI+"));
        assert!(!safe_data_uri("https://example.com/a.png"));
    }

    #[test]
    fn names_are_checked() {
        assert!(valid("00ff00ff00ff00ff.png"));
        assert!(!valid("../secret.png"));
        assert!(!valid("C:\\x.png"));
        assert!(!valid("a.exe"));
    }

    #[test]
    fn import_and_animate() {
        let dir = std::env::temp_dir().join(format!("hubdeck-images-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let _ = DIR.set(dir.join("images"));
        // a two-frame GIF built in memory
        let gif_path = dir.join("two.gif");
        {
            let mut file = std::fs::File::create(&gif_path).unwrap();
            let mut enc = image::codecs::gif::GifEncoder::new(&mut file);
            for c in [[255u8, 0, 0, 255], [0, 0, 255, 255]] {
                let buf = image::RgbaImage::from_pixel(8, 8, image::Rgba(c));
                enc.encode_frame(image::Frame::from_parts(buf, 0, 0, image::Delay::from_numer_denom_ms(20, 1))).unwrap();
            }
        }
        let name = import(&gif_path).unwrap();
        assert!(name.ends_with(".gif"));
        assert_eq!(import(&gif_path).unwrap(), name, "same content, same name");
        let delays = animation(&name).unwrap();
        assert_eq!(delays, vec![Duration::from_millis(20); 2], "the GIF's own pace, as in the window");
        assert!(uri(&format!("{name}#1")).unwrap().starts_with("data:image/png;base64,"));
        assert!(uri(&name).unwrap().starts_with("data:image/gif;base64,"));
        assert!(import(&dir.join("missing.txt")).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
