//! Typing the expansion with `SendInput`.
//!
//! Every event we send carries [`INJECT_TAG`] in `dwExtraInfo`, so the
//! keyboard hook can drop exactly our own events and nothing else.

use crate::core::expand::Key;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY,
    KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, MAPVK_VK_TO_VSC, MapVirtualKeyW, SendInput, VK_BACK,
    VK_CONTROL, VK_LEFT, VK_LWIN, VK_MENU, VK_RETURN, VK_RWIN, VK_SHIFT,
};

/// `dwExtraInfo` marker on every event meh injects ("MEH\0").
pub const INJECT_TAG: usize = 0x4D45_4800;

/// How long to wait for the user to let go of modifier keys.
pub const MODIFIER_WAIT: Duration = Duration::from_millis(500);

static BUSY: AtomicBool = AtomicBool::new(false);

/// True from the moment the worker sees a trigger until its replacement has
/// been sent (rendering, the modifier wait and `SendInput`). Keys the user
/// types meanwhile reach the app before our Backspaces, so the hook turns
/// them into resets instead of buffering them (verifier defect 4). Keys
/// queued before it was set are dropped by the worker (verifier N4).
pub fn is_busy() -> bool {
    BUSY.load(Ordering::Acquire)
}

/// Clears [`is_busy`] when dropped.
pub struct BusyGuard(());

/// Sets [`is_busy`] until the returned guard is dropped.
pub fn mark_busy() -> BusyGuard {
    BUSY.store(true, Ordering::Release);
    BusyGuard(())
}

impl Drop for BusyGuard {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::Release);
    }
}

/// Whether Shift, Ctrl, Alt or Win is physically held.
fn modifier_held() -> bool {
    [VK_SHIFT, VK_CONTROL, VK_MENU, VK_LWIN, VK_RWIN]
        .iter()
        // SAFETY: plain query, no pointers.
        .any(|&vk| unsafe { GetAsyncKeyState(vk as i32) } < 0)
}

/// Waits up to `timeout` for all modifiers to be released (a held Shift
/// would turn our Left presses into a selection). Returns whether they were.
pub fn wait_for_modifiers_released(timeout: Duration) -> bool {
    let end = Instant::now() + timeout;
    while modifier_held() {
        if Instant::now() >= end {
            return false;
        }
        thread::sleep(Duration::from_millis(10));
    }
    true
}

fn key_event(vk: u16, scan: u16, flags: u32) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: INJECT_TAG,
            },
        },
    }
}

/// Down and up events for a virtual key.
fn push_vk(out: &mut Vec<INPUT>, vk: u16, extended: bool) {
    // SAFETY: plain query, no pointers.
    let scan = unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC) } as u16;
    let ext = if extended { KEYEVENTF_EXTENDEDKEY } else { 0 };
    out.push(key_event(vk, scan, ext));
    out.push(key_event(vk, scan, ext | KEYEVENTF_KEYUP));
}

/// Down and up events for one UTF-16 code unit (`wVk` must be 0).
fn push_unit(out: &mut Vec<INPUT>, unit: u16) {
    out.push(key_event(0, unit, KEYEVENTF_UNICODE));
    out.push(key_event(0, unit, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP));
}

fn build(keys: &[Key]) -> Vec<INPUT> {
    let mut out = Vec::with_capacity(keys.len() * 2);
    for key in keys {
        match *key {
            Key::Back => push_vk(&mut out, VK_BACK, false),
            Key::Unit(u) => push_unit(&mut out, u),
            Key::Enter => push_vk(&mut out, VK_RETURN, false),
            Key::Left => push_vk(&mut out, VK_LEFT, true),
        }
    }
    out
}

/// Sends `keys` in one `SendInput` batch, so the user's own typing cannot
/// interleave. Returns false if Windows rejected some events (for example
/// UIPI blocks input to an elevated window, D-13).
pub fn send(keys: &[Key]) -> bool {
    let inputs = build(keys);
    if inputs.is_empty() {
        return true;
    }
    // SAFETY: `inputs` is a valid slice of INPUT for the duration of the call
    // and cbsize is the size of one INPUT.
    let sent = unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        )
    };
    sent as usize == inputs.len()
}
