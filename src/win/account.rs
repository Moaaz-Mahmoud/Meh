//! Which account meh runs as, versus the signed-in user (D-18). Only
//! `windows-sys`, so this type-checks on Linux.
//!
//! On managed PCs, setup may run as a separate admin account and start meh
//! as that account. meh then reads and writes that account's `%APPDATA%`,
//! so the signed-in user's snippets are never used. [`check`] warns once at
//! startup.

use crate::core::APP_NAME;
use crate::core::config;
use crate::core::ident::{same_account, wrong_account_message};
use crate::win::ui;
use std::ptr::{null, null_mut};
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::Security::{
    GetTokenInformation, LookupAccountSidW, SID_NAME_USE, TOKEN_QUERY, TOKEN_USER, TokenUser,
};
use windows_sys::Win32::System::RemoteDesktop::{
    WTS_CURRENT_SERVER_HANDLE, WTS_CURRENT_SESSION, WTS_INFO_CLASS, WTSDomainName, WTSFreeMemory,
    WTSQuerySessionInformationW, WTSUserName,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows_sys::core::PWSTR;

/// `(domain, user)`. Either may be empty when Windows does not say.
type Owned = (String, String);

/// Shows the wrong-account warning if meh runs as another account than the
/// signed-in user. Returns false if the user chose to quit. Unknown
/// accounts never warn.
pub fn check() -> bool {
    let (Some(process), Some(session)) = (process_account(), session_account()) else {
        return true;
    };
    let p = (process.0.as_str(), process.1.as_str());
    let s = (session.0.as_str(), session.1.as_str());
    if same_account(p, s) {
        return true;
    }
    let dir = config::config_dir().map(|d| d.to_string_lossy().into_owned());
    let text = wrong_account_message(p, s, dir.as_deref());
    !ui::ask_yes_no(APP_NAME, &text)
}

/// Reads a NUL-terminated UTF-16 string.
///
/// # Safety
/// `p` must be null or point to a NUL-terminated UTF-16 string.
unsafe fn from_pwstr(p: PWSTR) -> String {
    if p.is_null() {
        return String::new();
    }
    let mut len = 0;
    // SAFETY: the caller guarantees a NUL terminator.
    unsafe {
        while *p.add(len) != 0 {
            len += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(p, len))
    }
}

/// From a UTF-16 buffer, up to its first NUL.
fn from_buf(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

/// Closes a handle on drop.
struct Handle(HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: the handle is owned and closed once.
            unsafe { CloseHandle(self.0) };
        }
    }
}

/// The account of this process's token, as `DOMAIN`, `user`.
fn process_account() -> Option<Owned> {
    let mut token: HANDLE = null_mut();
    // SAFETY: GetCurrentProcess is a pseudo-handle; `token` is written once
    // on success.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return None;
    }
    let token = Handle(token);

    let mut len = 0u32;
    // SAFETY: a size query with a null buffer; only `len` is written.
    unsafe { GetTokenInformation(token.0, TokenUser, null_mut(), 0, &mut len) };
    if len == 0 {
        return None;
    }
    // u64 elements keep the buffer aligned for TOKEN_USER and its SID.
    let mut buf = vec![0u64; (len as usize).div_ceil(8)];
    // SAFETY: `buf` holds at least `len` bytes.
    let ok =
        unsafe { GetTokenInformation(token.0, TokenUser, buf.as_mut_ptr().cast(), len, &mut len) };
    if ok == 0 {
        return None;
    }
    // SAFETY: on success the buffer starts with a TOKEN_USER whose SID
    // points into the same buffer, which outlives the lookup below.
    let sid = unsafe { (*buf.as_ptr().cast::<TOKEN_USER>()).User.Sid };

    let mut name = vec![0u16; 256];
    let mut domain = vec![0u16; 256];
    for _ in 0..2 {
        let (mut n, mut d) = (name.len() as u32, domain.len() as u32);
        let mut kind: SID_NAME_USE = 0;
        // SAFETY: the buffers hold `n` and `d` UTF-16 units; `sid` is valid.
        let ok = unsafe {
            LookupAccountSidW(
                null(),
                sid,
                name.as_mut_ptr(),
                &mut n,
                domain.as_mut_ptr(),
                &mut d,
                &mut kind,
            )
        };
        if ok != 0 {
            return Some((from_buf(&domain), from_buf(&name)));
        }
        // Too small: the sizes now hold what is needed (with the NUL).
        if n as usize <= name.len() && d as usize <= domain.len() {
            return None;
        }
        name.resize(n as usize, 0);
        domain.resize(d as usize, 0);
    }
    None
}

/// One string from the current session's information.
fn session_string(class: WTS_INFO_CLASS) -> Option<String> {
    let mut buf: PWSTR = null_mut();
    let mut bytes = 0u32;
    // SAFETY: on success `buf` points to a NUL-terminated string that
    // WTSFreeMemory releases.
    unsafe {
        if WTSQuerySessionInformationW(
            WTS_CURRENT_SERVER_HANDLE,
            WTS_CURRENT_SESSION,
            class,
            &mut buf,
            &mut bytes,
        ) == 0
        {
            return None;
        }
        let s = from_pwstr(buf);
        WTSFreeMemory(buf.cast());
        Some(s)
    }
}

/// The user signed in to this process's session, as `DOMAIN`, `user`.
fn session_account() -> Option<Owned> {
    let user = session_string(WTSUserName)?;
    let domain = session_string(WTSDomainName).unwrap_or_default();
    Some((domain, user))
}
