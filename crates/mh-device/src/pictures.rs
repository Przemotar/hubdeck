//! macOS: pictures written several reports at a time, as `overlapped.rs` does on Windows. hidapi's
//! write waits for each 320-byte report before the next; on the owner's deck a q98 key panel took
//! 121-133 ms on the Mac (2026-10-30, ~8 pictures a second, where the PC with eight in flight
//! showed the same scene smoothly). `pictures_macos.m` opens the module again, shared, and keeps
//! [`DEPTH`] writes in flight with IOKit's asynchronous set-report call.
//! `HUBDECK_PIPELINE=0` turns it off (hidapi again), for comparing.

use std::ffi::{CStr, c_void};

use crate::{Error, Result};

/// Writes in flight at once (eight was the fastest on Windows; `examples/pipeline.rs` on the Mac).
pub const DEPTH: i32 = 8;
/// As hidapi on Windows: a picture whose writes take longer than this has failed (a module pulled off).
const TIMEOUT_S: f64 = 1.0;

unsafe extern "C" {
    fn mh_pictures_open(registry_id: u64, depth: i32, code: *mut i32) -> *mut c_void;
    fn mh_pictures_write(handle: *mut c_void, data: *const u8, len: usize, count: usize, timeout_s: f64) -> i32;
    fn mh_pictures_close(handle: *mut c_void);
}

pub struct Pipelined {
    handle: *mut c_void,
    /// The reports of one picture back to back, each padded to the report length.
    buf: Vec<u8>,
}

// The handle is only used under the module's lock.
unsafe impl Send for Pipelined {}

impl Pipelined {
    /// Opens the module behind a hidapi path (`DevSrvsID:<registry entry id>`) again for writes;
    /// None when macOS refuses it (hidapi then).
    pub fn open(path: &CStr) -> Option<Self> {
        Self::open_with(path, DEPTH).map_err(|e| log::info!("pictures one report at a time: {e}")).ok()
    }

    /// As [`Pipelined::open`] with this many writes in flight; the error says why it was refused.
    pub fn open_with(path: &CStr, depth: i32) -> std::result::Result<Self, String> {
        if std::env::var("HUBDECK_PIPELINE").is_ok_and(|v| v == "0") {
            return Err("HUBDECK_PIPELINE=0".into());
        }
        let id: u64 = path.to_str().ok().and_then(|p| p.strip_prefix("DevSrvsID:")).and_then(|n| n.parse().ok()).ok_or_else(|| format!("not a macOS HID path: {path:?}"))?;
        let mut code = 0;
        // SAFETY: a plain C function; it returns an owned handle or NULL with the code set.
        let handle = unsafe { mh_pictures_open(id, depth, &mut code) };
        if handle.is_null() {
            return Err(format!("IOKit refused to open it: {code:#010x}"));
        }
        Ok(Self { handle, buf: Vec::new() })
    }

    /// Writes every report (each padded to `len`), several at a time, and returns when all have
    /// ended.
    pub fn write_all<'a>(&mut self, reports: impl Iterator<Item = &'a [u8]>, len: usize) -> Result<()> {
        self.buf.clear();
        let mut count = 0;
        for bytes in reports {
            if bytes.len() > len {
                return Err(Error::Write(format!("a report of {} B, longer than the report length {len}", bytes.len())));
            }
            self.buf.extend_from_slice(bytes);
            self.buf.resize((count + 1) * len, 0);
            count += 1;
        }
        // SAFETY: `buf` holds `count` reports of `len` bytes; the call copies each before it returns.
        let code = unsafe { mh_pictures_write(self.handle, self.buf.as_ptr(), len, count, TIMEOUT_S) };
        if code == 0 { Ok(()) } else { Err(Error::Write(format!("write failed: IOKit {:#010x}", code as u32))) }
    }
}

impl Drop for Pipelined {
    fn drop(&mut self) {
        // SAFETY: opened in `open_with`; closed once.
        unsafe { mh_pictures_close(self.handle) };
    }
}
