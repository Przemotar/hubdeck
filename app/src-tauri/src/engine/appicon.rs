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
    use windows::Win32::UI::Shell::SHDefExtractIconW;
    use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, HICON, ICONINFO};
    use windows::core::HSTRING;

    const SIZE: i32 = 128;

    pub fn png(path: &str) -> Result<Vec<u8>, String> {
        // the shell's own documented icon reader, at the size asked for (the owner's rule of
        // 2026-10-16: no private or undocumented APIs; it was PrivateExtractIconsW)
        let mut icon = HICON::default();
        // SAFETY: the path is a NUL-terminated HSTRING alive for the call; one large icon is asked for.
        let read = unsafe { SHDefExtractIconW(&HSTRING::from(path), 0, 0, Some(&mut icon), None, SIZE as u32) };
        if read.is_err() || icon.is_invalid() {
            return Err("this file has no icon".into());
        }
        let result = rgba(icon);
        // SAFETY: the icon came from SHDefExtractIconW and is ours to free.
        unsafe {
            let _ = DestroyIcon(icon);
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
    use objc2::AllocAnyThread;
    use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSWorkspace};
    use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize, NSString};

    pub fn png(path: &str) -> Result<Vec<u8>, String> {
        // an app linked into /Applications (Safari on macOS 26) would get Finder's alias arrow
        let real = std::fs::canonicalize(path).map_err(|e| format!("{path}: {e}"))?;
        let path = real.to_string_lossy();
        // Only the 128 px picture is rendered. Asking for the TIFF of the icon rendered every size
        // up to 1024 px (2048 at 2x): reading the icons of 102 apps for the window's lists took
        // 4.9 GB and 90 s (2026-10-15; HubDeck jumped to 1.8 GB when the window opened). The
        // autorelease pool still matters: AppKit's objects are autoreleased and a Rust thread
        // has no pool of its own (2026-10-14: 5 GB at start).
        objc2::rc::autoreleasepool(|_| -> Result<Vec<u8>, String> {
            // SAFETY: AppKit calls with objects we own; NSWorkspace, NSImage's CGImage and image
            // encoding work off the main thread.
            unsafe {
                let image = NSWorkspace::sharedWorkspace().iconForFile(&NSString::from_str(&path));
                let mut rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(128.0, 128.0));
                let cg = image.CGImageForProposedRect_context_hints(&mut rect, None, None).ok_or("no icon")?;
                let rep = NSBitmapImageRep::initWithCGImage(NSBitmapImageRep::alloc(), &cg);
                let png = rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new()).ok_or("no icon")?;
                let png = png.to_vec();
                // without a context the 1x picture comes (128 px); scale it if a system gives more
                let img = image::load_from_memory(&png).map_err(|e| e.to_string())?;
                if img.width() == 128 && img.height() == 128 {
                    return Ok(png);
                }
                let mut out = std::io::Cursor::new(Vec::new());
                img.resize(128, 128, image::imageops::FilterType::Lanczos3).write_to(&mut out, image::ImageFormat::Png).map_err(|e| e.to_string())?;
                Ok(out.into_inner())
            }
        })
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
        // sharp on a key: the 128 px size asked for, not a 32 px icon scaled up
        assert!(img.width() >= 128, "{} px", img.width());
    }
}
