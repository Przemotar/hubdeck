//! Windows: pictures written several reports at a time. hidapi writes one 320-byte report and waits
//! for it before the next; on the owner's deck that held a q98 key panel to ~1.2 MB/s (a snowy
//! panel of 80 KB: 76 ms, 10.9 pictures a second). With writes in flight on an overlapped handle of
//! its own the same pictures go faster and cost less CPU (2026-10-26, `examples/pipeline.rs`, 40 KB
//! pictures: one by one 33.6-109.6 ms and 4.2-6.2 ms of CPU a picture while the PC was busy, eight
//! in flight 21.7-28.2 ms and 2.2-2.5 ms; two in flight 21.9-42.1 ms, 3.3-3.6 ms). The reports
//! still go in order: the HID class driver queues writes on a handle and the device takes them as
//! they come; the camera saw every picture whole (two in flight).
//! `HUBDECK_PIPELINE=0` turns it off (hidapi again), for comparing.

use std::ffi::CStr;

use windows_sys::Win32::Foundation::{CloseHandle, ERROR_IO_PENDING, GENERIC_WRITE, GetLastError, HANDLE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0};
use windows_sys::Win32::Storage::FileSystem::{CreateFileW, FILE_FLAG_OVERLAPPED, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, WriteFile};
use windows_sys::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};
use windows_sys::Win32::System::Threading::{CreateEventW, WaitForSingleObject};

use crate::{Error, Result};

/// Writes in flight at once (eight was the fastest and the cheapest of 1, 2, 4, 8).
const DEPTH: usize = 8;
/// As hidapi: a write that takes longer than this has failed (a module pulled off).
const TIMEOUT_MS: u32 = 1000;

struct Slot {
    /// Boxed: the system holds its address until the write ends.
    ov: Box<OVERLAPPED>,
    event: HANDLE,
    /// The report being written (kept alive until the write ends).
    buf: Vec<u8>,
    busy: bool,
}

pub struct Pipelined {
    handle: HANDLE,
    slots: Vec<Slot>,
}

// The handle and events are only used under the module's lock.
unsafe impl Send for Pipelined {}

impl Pipelined {
    /// Opens the device again for overlapped writes; None when Windows refuses it (hidapi then).
    pub fn open(path: &CStr) -> Option<Self> {
        if std::env::var("HUBDECK_PIPELINE").is_ok_and(|v| v == "0") {
            return None;
        }
        let wide: Vec<u16> = path.to_string_lossy().encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: a valid NUL-terminated path; the handle is closed in Drop.
        let handle = unsafe { CreateFileW(wide.as_ptr(), GENERIC_WRITE, FILE_SHARE_READ | FILE_SHARE_WRITE, std::ptr::null(), OPEN_EXISTING, FILE_FLAG_OVERLAPPED, std::ptr::null_mut()) };
        if handle == INVALID_HANDLE_VALUE {
            return None;
        }
        let mut slots = Vec::new();
        for _ in 0..DEPTH {
            // SAFETY: a manual-reset event, closed in Drop.
            let event = unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) };
            if event.is_null() {
                let me = Self { handle, slots };
                drop(me);
                return None;
            }
            // SAFETY: OVERLAPPED is plain data; zeroed is its documented start.
            slots.push(Slot { ov: Box::new(unsafe { std::mem::zeroed() }), event, buf: Vec::new(), busy: false });
        }
        Some(Self { handle, slots })
    }

    /// Waits for a slot's write to end.
    fn finish(handle: HANDLE, slot: &mut Slot) -> Result<()> {
        if !slot.busy {
            return Ok(());
        }
        slot.busy = false;
        // SAFETY: the event belongs to this slot's pending write.
        let waited = unsafe { WaitForSingleObject(slot.event, TIMEOUT_MS) };
        let mut done = 0u32;
        if waited != WAIT_OBJECT_0 {
            // SAFETY: cancel this one write, then wait for the cancel so the buffer is free.
            unsafe {
                CancelIoEx(handle, &*slot.ov);
                GetOverlappedResult(handle, &*slot.ov, &mut done, 1);
            }
            return Err(Error::Write(format!("a write did not end in {TIMEOUT_MS} ms")));
        }
        // SAFETY: the write has ended; this reads its result.
        if unsafe { GetOverlappedResult(handle, &*slot.ov, &mut done, 0) } == 0 {
            return Err(Error::Write(format!("write failed: Windows error {}", unsafe { GetLastError() })));
        }
        Ok(())
    }

    /// Writes every report (each padded to `len`), up to [`DEPTH`] at a time, and returns when all
    /// have ended.
    pub fn write_all<'a>(&mut self, reports: impl Iterator<Item = &'a [u8]>, len: usize) -> Result<()> {
        let handle = self.handle;
        let mut result = Ok(());
        for (n, bytes) in reports.enumerate() {
            let slot = &mut self.slots[n % DEPTH];
            if let Err(e) = Self::finish(handle, slot) {
                result = Err(e);
                break;
            }
            slot.buf.clear();
            slot.buf.extend_from_slice(bytes);
            slot.buf.resize(len.max(bytes.len()), 0);
            // SAFETY: zeroed again for a new write, with this slot's event.
            *slot.ov = unsafe { std::mem::zeroed() };
            slot.ov.hEvent = slot.event;
            // SAFETY: the buffer and the OVERLAPPED stay alive and untouched until `finish`.
            let ok = unsafe { WriteFile(handle, slot.buf.as_ptr(), slot.buf.len() as u32, std::ptr::null_mut(), &mut *slot.ov) };
            if ok == 0 {
                let e = unsafe { GetLastError() };
                if e != ERROR_IO_PENDING {
                    result = Err(Error::Write(format!("write failed: Windows error {e}")));
                    break;
                }
            }
            slot.busy = true;
        }
        // every write ends before the reports' memory may change (and before the next picture)
        for slot in &mut self.slots {
            let ended = Self::finish(handle, slot);
            if result.is_ok() {
                result = ended;
            }
        }
        result
    }
}

impl Drop for Pipelined {
    fn drop(&mut self) {
        for slot in &mut self.slots {
            let _ = Self::finish(self.handle, slot);
            // SAFETY: created in `open`, no write pending.
            unsafe { CloseHandle(slot.event) };
        }
        // SAFETY: opened in `open`.
        unsafe { CloseHandle(self.handle) };
    }
}
