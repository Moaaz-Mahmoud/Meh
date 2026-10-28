//! Reading clipboard text for `{{clipboard}}` variables.

use std::thread;
use std::time::Duration;
use windows_sys::Win32::System::DataExchange::{
    CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
};
use windows_sys::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};

/// `CF_UNICODETEXT` (defined here to avoid the `Win32_System_Ole` feature).
const CF_UNICODETEXT: u32 = 13;

/// Another app may hold the clipboard briefly; retry this many times.
const OPEN_ATTEMPTS: u32 = 10;
const RETRY_DELAY: Duration = Duration::from_millis(10);

/// Closes the clipboard when dropped.
struct Open;

impl Drop for Open {
    fn drop(&mut self) {
        // SAFETY: only constructed after OpenClipboard succeeded.
        unsafe { CloseClipboard() };
    }
}

fn open() -> Option<Open> {
    for _ in 0..OPEN_ATTEMPTS {
        // SAFETY: a null owner is allowed for reading.
        if unsafe { OpenClipboard(std::ptr::null_mut()) } != 0 {
            return Some(Open);
        }
        thread::sleep(RETRY_DELAY);
    }
    None
}

/// The clipboard's text, or `None` if it holds no text or is busy.
pub fn read_text() -> Option<String> {
    // SAFETY: plain query.
    if unsafe { IsClipboardFormatAvailable(CF_UNICODETEXT) } == 0 {
        return None;
    }
    let _open = open()?;
    // SAFETY: the clipboard is open for the whole block. The handle is a
    // global memory block of `GlobalSize` bytes holding UTF-16 text; it is
    // locked while read and unlocked before the clipboard closes.
    unsafe {
        let handle = GetClipboardData(CF_UNICODETEXT);
        if handle.is_null() {
            return None;
        }
        let ptr = GlobalLock(handle) as *const u16;
        if ptr.is_null() {
            return None;
        }
        let units = std::slice::from_raw_parts(ptr, GlobalSize(handle) / 2);
        let len = units.iter().position(|&u| u == 0).unwrap_or(units.len());
        let text = String::from_utf16_lossy(&units[..len]);
        GlobalUnlock(handle);
        Some(text)
    }
}
