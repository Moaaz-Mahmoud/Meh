//! Small Win32 UI helpers used by the tray and the app loop: the message
//! loop, the balloon notification, message boxes and opening a folder.
//! Only `windows-sys`, so this type-checks on Linux.

use super::wide;
use crate::core::reload::Notice;
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};
use std::thread;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::System::Com::{
    COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoUninitialize,
};
use windows_sys::Win32::UI::Shell::{
    NIF_GUID, NIF_INFO, NIF_TIP, NIIF_ERROR, NIIF_WARNING, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
    Shell_NotifyIconW, ShellExecuteW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, IDYES, KillTimer, MB_ICONERROR, MB_ICONINFORMATION,
    MB_ICONWARNING, MB_OK, MB_SETFOREGROUND, MB_YESNO, MSG, MessageBoxW, PostQuitMessage,
    SW_SHOWNORMAL, SetTimer, TranslateMessage, WM_TIMER,
};
use windows_sys::core::GUID;

/// Runs the thread's message loop until [`quit`] is called, calling
/// `on_tick` every `tick_ms` from a thread timer. Hooks and the tray
/// window are serviced by this loop.
pub fn message_loop(tick_ms: u32, mut on_tick: impl FnMut()) {
    // SAFETY: standard Win32 message loop on the calling thread; `msg` is a
    // valid MSG for every call.
    unsafe {
        let timer = SetTimer(null_mut(), 0, tick_ms, None);
        let mut msg: MSG = std::mem::zeroed();
        // GetMessageW returns 0 on WM_QUIT and -1 on error; stop on both.
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            if msg.message == WM_TIMER && msg.hwnd.is_null() && msg.wParam == timer {
                on_tick();
                continue;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        if timer != 0 {
            KillTimer(null_mut(), timer);
        }
    }
}

/// Ends [`message_loop`] on the calling thread.
pub fn quit() {
    // SAFETY: no pointers.
    unsafe { PostQuitMessage(0) };
}

/// Copies `s` into a fixed UTF-16 buffer, truncated, NUL-terminated.
fn copy_wide(dst: &mut [u16], s: &str) {
    let max = dst.len().saturating_sub(1);
    let mut n = 0;
    for u in s.encode_utf16().take(max) {
        dst[n] = u;
        n += 1;
    }
    // Do not end on half a surrogate pair.
    if n > 0 && (0xD800..0xDC00).contains(&dst[n - 1]) {
        n -= 1;
    }
    dst[n] = 0;
}

fn notify_data(hwnd: HWND, guid: u128, flags: u32) -> NOTIFYICONDATAW {
    // SAFETY: NOTIFYICONDATAW is plain data; all-zero is a valid value.
    let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    // With NIF_GUID the shell finds the icon by GUID; uID is ignored.
    nid.uFlags = flags | NIF_GUID;
    nid.guidItem = GUID::from_u128(guid);
    nid
}

/// Shows a balloon (a toast on Windows 10/11) on the tray icon that was
/// created with `guid` by the window `hwnd`. Returns whether the shell
/// accepted it.
pub fn show_balloon(hwnd: HWND, guid: u128, notice: &Notice) -> bool {
    let mut nid = notify_data(hwnd, guid, NIF_INFO);
    copy_wide(&mut nid.szInfoTitle, &notice.title);
    copy_wide(&mut nid.szInfo, &notice.body);
    nid.dwInfoFlags = if notice.is_error {
        NIIF_ERROR
    } else {
        NIIF_WARNING
    };
    // SAFETY: `nid` is a fully initialised NOTIFYICONDATAW.
    unsafe { Shell_NotifyIconW(NIM_MODIFY, &nid) != 0 }
}

/// Removes a tray icon left behind by a meh that was killed, so the new
/// icon with the same GUID can be added. Failing is the normal case.
pub fn delete_stale_icon(guid: u128) {
    let nid = notify_data(null_mut(), guid, 0);
    // SAFETY: `nid` is a fully initialised NOTIFYICONDATAW.
    unsafe { Shell_NotifyIconW(NIM_DELETE, &nid) };
}

/// Sets the tray tooltip, addressing the icon by GUID.
pub fn set_tip(hwnd: HWND, guid: u128, tip: &str) -> bool {
    let mut nid = notify_data(hwnd, guid, NIF_TIP);
    copy_wide(&mut nid.szTip, tip);
    // SAFETY: `nid` is a fully initialised NOTIFYICONDATAW.
    unsafe { Shell_NotifyIconW(NIM_MODIFY, &nid) != 0 }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoxKind {
    Info,
    Warning,
    Error,
}

/// A modal message box. Safe to call on the hook thread: `MessageBoxW` runs
/// its own modal message loop, and low-level hook callbacks are delivered
/// to the installing thread through its message retrieval, which that loop
/// keeps doing, so the hooks stay serviced while the box is open. Only our
/// 100 ms tick (tray menu polling, worker updates) pauses until it closes.
pub fn message_box(title: &str, text: &str, kind: BoxKind) {
    let icon = match kind {
        BoxKind::Info => MB_ICONINFORMATION,
        BoxKind::Warning => MB_ICONWARNING,
        BoxKind::Error => MB_ICONERROR,
    };
    let (title, text) = (wide(title), wide(text));
    // SAFETY: both strings are NUL-terminated and outlive the call.
    unsafe {
        MessageBoxW(
            null_mut(),
            text.as_ptr(),
            title.as_ptr(),
            MB_OK | icon | MB_SETFOREGROUND,
        );
    }
}

/// A modal Yes/No warning box. Returns true for Yes. Closing the box
/// counts as No.
pub fn ask_yes_no(title: &str, text: &str) -> bool {
    let (title, text) = (wide(title), wide(text));
    // SAFETY: both strings are NUL-terminated and outlive the call.
    let r = unsafe {
        MessageBoxW(
            null_mut(),
            text.as_ptr(),
            title.as_ptr(),
            MB_YESNO | MB_ICONWARNING | MB_SETFOREGROUND,
        )
    };
    r == IDYES
}

/// Opens `dir` in Explorer on a short-lived thread, and shows a warning box
/// from that thread if it fails. `ShellExecuteW` can block for seconds
/// (COM, a cold Explorer), longer than Windows allows a low-level hook to
/// take, so it must never run on the hook thread (verifier defect 2).
pub fn open_folder_in_background(dir: Option<PathBuf>, title: &'static str, failure: String) {
    let spawned = thread::Builder::new()
        .name("meh-open-folder".into())
        .spawn(move || {
            if !dir.as_deref().is_some_and(open_folder) {
                message_box(title, &failure, BoxKind::Warning);
            }
        });
    // If no thread could be created, skip it rather than block the hooks.
    drop(spawned);
}

/// Opens `dir` in Explorer. Returns whether the shell accepted it. Blocks;
/// call it off the hook thread (see [`open_folder_in_background`]).
fn open_folder(dir: &Path) -> bool {
    let (verb, path) = (wide("open"), wide(&dir.to_string_lossy()));
    // ShellExecuteW may use COM, so this thread gets an apartment first,
    // as its documentation asks.
    // SAFETY: reserved pointer is null; flags are valid COINIT values.
    let com = unsafe {
        CoInitializeEx(
            null(),
            (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32,
        )
    };
    // SAFETY: strings are NUL-terminated and outlive the call.
    let r = unsafe {
        ShellExecuteW(
            null_mut(),
            verb.as_ptr(),
            path.as_ptr(),
            null(),
            null(),
            SW_SHOWNORMAL,
        )
    };
    if com >= 0 {
        // SAFETY: balances the successful CoInitializeEx above.
        unsafe { CoUninitialize() };
    }
    // Values above 32 mean success.
    r as usize > 32
}
