//! One meh per user session, via a named mutex. The installer's
//! `AppMutex=` uses the same name to ask the user to quit meh first.

use super::wide;
use windows_sys::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE};
use windows_sys::Win32::System::Threading::CreateMutexW;

/// Mutex name. Must match `AppMutex` in `installer/meh.iss`.
pub const MUTEX_NAME: &str = "meh-single-instance";

/// Holds the mutex for the life of the process.
pub struct Instance(HANDLE);

impl Drop for Instance {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: the handle came from CreateMutexW and is closed once.
            unsafe { CloseHandle(self.0) };
        }
    }
}

/// Takes the single-instance mutex. `None` means meh is already running.
/// If the mutex cannot be created at all, meh runs anyway.
pub fn acquire() -> Option<Instance> {
    let name = wide(MUTEX_NAME);
    // SAFETY: `name` is NUL-terminated and outlives the call. GetLastError
    // is read right after CreateMutexW, before any other API call.
    let (handle, err) = unsafe {
        let h = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
        (h, GetLastError())
    };
    let instance = Instance(handle);
    if !handle.is_null() && err == ERROR_ALREADY_EXISTS {
        return None;
    }
    Some(instance)
}
