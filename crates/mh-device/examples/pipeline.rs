//! Can the key panel take a full-quality picture faster when its reports are written several at a
//! time? (2026-10-26: with snow on the owner's deck a q98 panel of ~80 KB took 76 ms, 10.9 pictures
//! a second; hidapi writes one 320-byte report and waits for it before the next.) Sends the same
//! busy pictures (a snowy panel, q98) for a few seconds each way: hidapi one by one as HubDeck does,
//! then Windows overlapped writes with 2, 4 and 8 in flight on a handle of its own; prints the
//! time a picture takes and pictures a second.
//!
//!     cargo run --release -p mh-device --example pipeline -- [seconds per test]
//!
//! Quit HubDeck first (it would mix its pictures in); the keys go back to their own animation at
//! the end. Windows only.

#[cfg(windows)]
fn main() {
    win::main();
}

#[cfg(not(windows))]
fn main() {
    eprintln!("Windows only");
}

#[cfg(windows)]
mod win {
    use std::io::Cursor;
    use std::time::{Duration, Instant};

    use image::codecs::jpeg::JpegEncoder;
    use image::{Rgb, RgbImage};
    use mh_device::Hub;
    use mh_protocol::{ModuleKind, Report, report};
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::Foundation::{CloseHandle, ERROR_IO_PENDING, GENERIC_WRITE, GetLastError, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{CreateFileW, FILE_FLAG_OVERLAPPED, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, WriteFile};
    use windows_sys::Win32::System::IO::{GetOverlappedResult, OVERLAPPED};
    use windows_sys::Win32::System::Threading::{CreateEventW, GetCurrentProcess, GetProcessTimes};

    /// This process's CPU time so far (user + kernel), ms.
    fn cpu_ms() -> f64 {
        let z = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
        let (mut c, mut e, mut k, mut u) = (z, z, z, z);
        unsafe { GetProcessTimes(GetCurrentProcess(), &mut c, &mut e, &mut k, &mut u) };
        let t = |f: FILETIME| ((f.dwHighDateTime as u64) << 32 | f.dwLowDateTime as u64) as f64 / 10_000.0;
        t(k) + t(u)
    }

    /// A busy picture like snow on the owner's deck: dark keys, light marks, soft flakes everywhere.
    fn frame(i: u32) -> RgbImage {
        let mut img = RgbImage::from_fn(480, 272, |x, y| {
            let key = (x % 100) < 79 && (y % 99) < 75;
            if key { Rgb([27, 26, 32]) } else { Rgb([0, 0, 0]) }
        });
        let mut seed = 0x9e37_79b9u32 ^ i.wrapping_mul(2_654_435_761);
        let mut rnd = || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed
        };
        for _ in 0..140 {
            let (cx, cy, r) = ((rnd() % 480) as i32, (rnd() % 272) as i32, 1 + (rnd() % 4) as i32);
            for y in cy - r..=cy + r {
                for x in cx - r..=cx + r {
                    if (0..480).contains(&x) && (0..272).contains(&y) && (x - cx).pow(2) + (y - cy).pow(2) <= r * r {
                        img.put_pixel(x as u32, y as u32, Rgb([235, 242, 255]));
                    }
                }
            }
        }
        img
    }

    fn jpeg(img: &RgbImage) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        JpegEncoder::new_with_quality(&mut out, 98).encode_image(img).expect("encode jpeg");
        out.into_inner()
    }

    fn report_bytes(r: &Report, len: usize) -> Vec<u8> {
        let Report::Output(b) = r else { panic!("an output report") };
        let mut v = b.clone();
        v.resize(len.max(b.len()), 0);
        v
    }

    /// Writes `reports` with up to `depth` in flight on an overlapped handle.
    fn send_overlapped(h: HANDLE, reports: &[Vec<u8>], depth: usize, events: &[HANDLE]) {
        let mut slots: Vec<(Box<OVERLAPPED>, bool)> = (0..depth)
            .map(|k| {
                let mut o: Box<OVERLAPPED> = Box::new(unsafe { std::mem::zeroed() });
                o.hEvent = events[k];
                (o, false)
            })
            .collect();
        let wait = |o: &mut OVERLAPPED| {
            let mut done = 0u32;
            let ok = unsafe { GetOverlappedResult(h, o, &mut done, 1) };
            assert!(ok != 0, "write failed: {}", unsafe { GetLastError() });
        };
        for (n, r) in reports.iter().enumerate() {
            let slot = &mut slots[n % depth];
            if slot.1 {
                wait(&mut slot.0);
            }
            *slot.0 = unsafe { std::mem::zeroed() };
            slot.0.hEvent = events[n % depth];
            let ok = unsafe { WriteFile(h, r.as_ptr(), r.len() as u32, std::ptr::null_mut(), &mut *slot.0) };
            if ok == 0 {
                let e = unsafe { GetLastError() };
                assert_eq!(e, ERROR_IO_PENDING, "WriteFile: {e}");
            }
            slot.1 = true;
        }
        for slot in &mut slots {
            if slot.1 {
                wait(&mut slot.0);
                slot.1 = false;
            }
        }
    }

    pub fn main() {
        let seconds: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(4);
        let hub = Hub::start().expect("hub");
        std::thread::sleep(Duration::from_millis(800));
        let keys = hub.first(ModuleKind::Keys).expect("the keys module");
        let frames: Vec<Vec<u8>> = (0..30).map(|i| jpeg(&frame(i))).collect();
        let avg = frames.iter().map(Vec::len).sum::<usize>() / frames.len();
        let reports: Vec<Vec<Vec<u8>>> = frames.iter().map(|f| report::panel_image(f, keys.output_len).iter().map(|r| report_bytes(r, keys.output_len)).collect()).collect();
        println!("pictures: q98, ~{} KB, {} reports of {} B each", avg / 1024, reports[0].len(), keys.output_len);

        // hidapi, one report at a time (HubDeck today)
        let cpu0 = cpu_ms();
        let start = Instant::now();
        let mut n = 0usize;
        while start.elapsed() < Duration::from_secs(seconds) {
            keys.send_all(&report::panel_image(&frames[n % frames.len()], keys.output_len)).expect("send");
            n += 1;
        }
        let ms = start.elapsed().as_secs_f64() * 1000.0 / n as f64;
        println!("hidapi one by one:      {ms:5.1} ms a picture, {:5.1} a second, CPU {:.2} ms a picture", 1000.0 / ms, (cpu_ms() - cpu0) / n as f64);

        // overlapped, on a handle of its own
        let api = hidapi::HidApi::new().expect("hidapi");
        let path = api.device_list().find(|d| d.vendor_id() == 0x2516 && d.product_id() == 0x01FE).expect("keys in the list").path().to_owned();
        let wide: Vec<u16> = path.to_string_lossy().encode_utf16().chain(std::iter::once(0)).collect();
        let h = unsafe { CreateFileW(wide.as_ptr(), GENERIC_WRITE, FILE_SHARE_READ | FILE_SHARE_WRITE, std::ptr::null(), OPEN_EXISTING, FILE_FLAG_OVERLAPPED, std::ptr::null_mut()) };
        assert!(h != INVALID_HANDLE_VALUE, "open: {}", unsafe { GetLastError() });
        let events: Vec<HANDLE> = (0..8).map(|_| unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) }).collect();
        for depth in [1, 2, 4, 8] {
            let cpu0 = cpu_ms();
            let start = Instant::now();
            let mut n = 0usize;
            while start.elapsed() < Duration::from_secs(seconds) {
                send_overlapped(h, &reports[n % reports.len()], depth, &events);
                n += 1;
            }
            let ms = start.elapsed().as_secs_f64() * 1000.0 / n as f64;
            println!("overlapped, {depth} in flight: {ms:5.1} ms a picture, {:5.1} a second, CPU {:.2} ms a picture", 1000.0 / ms, (cpu_ms() - cpu0) / n as f64);
        }
        for e in events {
            unsafe { CloseHandle(e) };
        }
        unsafe { CloseHandle(h) };
        drop(hub); // the keys back to their own animation
    }
}
