//! The icon of a program, as a PNG for a key: Windows reads it from the `.exe` at 128 px, so it
//! stays sharp on the key. Other systems: not yet.

/// PNG bytes of the program's icon (read once per program and run: reading it is slow, and the
/// tour rebuilds its deck at every click).
pub fn png(path: &str) -> Result<Vec<u8>, String> {
    static READ: std::sync::Mutex<Option<std::collections::HashMap<String, Vec<u8>>>> = std::sync::Mutex::new(None);
    let path = path.trim().trim_matches('"');
    if let Some(bytes) = READ.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|m| m.get(path)).cloned() {
        return Ok(bytes);
    }
    let bytes = platform::png(path)?;
    READ.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(Default::default).insert(path.to_string(), bytes.clone());
    Ok(bytes)
}

#[cfg(windows)]
mod platform {
    use std::io::Cursor;

    use windows::Win32::Graphics::Gdi::{BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDIBits, GetObjectW, HGDIOBJ};
    use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, HICON, ICONINFO, PrivateExtractIconsW};

    const SIZE: i32 = 128;

    pub fn png(path: &str) -> Result<Vec<u8>, String> {
        let wide: Vec<u16> = path.encode_utf16().collect();
        if wide.len() >= 260 {
            return Err("the path is too long".into());
        }
        let mut name = [0u16; 260];
        name[..wide.len()].copy_from_slice(&wide);
        let mut icons = [HICON::default()];
        // SAFETY: the name is NUL-terminated (zeroed tail); one icon slot is passed.
        let got = unsafe { PrivateExtractIconsW(&name, 0, SIZE, SIZE, Some(&mut icons), None, 0) };
        if got == 0 || got == u32::MAX || icons[0].is_invalid() {
            return Err("this file has no icon".into());
        }
        let result = rgba(icons[0]);
        // SAFETY: the icon came from PrivateExtractIconsW and is ours to free.
        unsafe {
            let _ = DestroyIcon(icons[0]);
        }
        let img = result?;
        let mut out = Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).map_err(|e| e.to_string())?;
        Ok(out.into_inner())
    }

    fn rgba(icon: HICON) -> Result<image::RgbaImage, String> {
        // SAFETY: standard GDI calls; every object created here is deleted before returning.
        unsafe {
            let mut info = ICONINFO::default();
            GetIconInfo(icon, &mut info).map_err(|e| e.to_string())?;
            let mut bm = BITMAP::default();
            GetObjectW(HGDIOBJ(info.hbmColor.0), std::mem::size_of::<BITMAP>() as i32, Some(&mut bm as *mut _ as *mut _));
            let (w, h) = (bm.bmWidth.max(1), bm.bmHeight.max(1));
            let dc = CreateCompatibleDC(None);
            let mut bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w,
                    biHeight: -h,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bgra = vec![0u8; (w * h * 4) as usize];
            let lines = GetDIBits(dc, info.hbmColor, 0, h as u32, Some(bgra.as_mut_ptr() as *mut _), &mut bmi, DIB_RGB_COLORS);
            let _ = DeleteDC(dc);
            let _ = DeleteObject(HGDIOBJ(info.hbmColor.0));
            let _ = DeleteObject(HGDIOBJ(info.hbmMask.0));
            if lines == 0 {
                return Err("could not read the icon".into());
            }
            // old icons have no alpha channel: treat them as opaque
            let has_alpha = bgra.chunks(4).any(|p| p[3] != 0);
            let pixels: Vec<u8> = bgra.chunks(4).flat_map(|p| [p[2], p[1], p[0], if has_alpha { p[3] } else { 255 }]).collect();
            image::RgbaImage::from_raw(w as u32, h as u32, pixels).ok_or_else(|| "bad icon size".into())
        }
    }
}

/// macOS: the icon Finder shows for an app bundle (or any file), 128x128 PNG.
#[cfg(target_os = "macos")]
mod platform {
    use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSWorkspace};
    use objc2_foundation::{NSDictionary, NSSize, NSString};

    pub fn png(path: &str) -> Result<Vec<u8>, String> {
        // an app linked into /Applications (Safari on macOS 26) would get Finder's alias arrow
        let real = std::fs::canonicalize(path).map_err(|e| format!("{path}: {e}"))?;
        let path = real.to_string_lossy();
        // SAFETY: AppKit calls with objects we own; NSWorkspace and image encoding work off the
        // main thread.
        unsafe {
            let image = NSWorkspace::sharedWorkspace().iconForFile(&NSString::from_str(&path));
            image.setSize(NSSize::new(128.0, 128.0));
            let tiff = image.TIFFRepresentation().ok_or("no icon")?;
            let rep = NSBitmapImageRep::imageRepWithData(&tiff).ok_or("no icon")?;
            let png = rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new()).ok_or("no icon")?;
            // the encoding carries the largest size (1024 px); keys need 128
            let big = image::load_from_memory(&png.to_vec()).map_err(|e| e.to_string())?;
            let mut out = std::io::Cursor::new(Vec::new());
            big.resize(128, 128, image::imageops::FilterType::Lanczos3).write_to(&mut out, image::ImageFormat::Png).map_err(|e| e.to_string())?;
            Ok(out.into_inner())
        }
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod platform {
    pub fn png(_path: &str) -> Result<Vec<u8>, String> {
        Err("app icons are not available on this system yet".into())
    }
}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn notepad_has_an_icon() {
        let png = super::png(r"C:\Windows\System32\notepad.exe").expect("notepad icon");
        let img = image::load_from_memory(&png).unwrap();
        assert!(img.width() >= 32);
    }
}
