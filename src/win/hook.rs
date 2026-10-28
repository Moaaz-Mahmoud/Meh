//! Low-level keyboard and mouse hooks (`WH_KEYBOARD_LL`, `WH_MOUSE_LL`).
//!
//! The procs run on the thread that installed them, inside its message
//! loop. Windows silently removes a hook that is too slow, so a proc only
//! translates the event and sends it over a channel. It never blocks, never
//! touches files and never calls `SendInput` (the worker does).
//!
//! Buffer rules (D-08, D-15): Backspace drops one char; Enter, Tab, arrows,
//! Home, End, PgUp, PgDn, Esc, IME keys, mouse clicks and Ctrl, Alt or Win
//! combos reset. Enter and Tab never fire a trigger (D-15). Ctrl+Alt held
//! together is AltGr and is translated normally. A dead key resets.

use super::inject::INJECT_TAG;
use super::{Event, KeyInput};
use crate::core::input::{Translation, Typed, Utf16Joiner, interpret_to_unicode, is_shortcut};
use std::cell::Cell;
use std::io;
use std::ptr::null_mut;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyState, GetKeyboardLayout, ToUnicodeEx, VK_BACK, VK_CAPITAL, VK_CONTROL,
    VK_DOWN, VK_END, VK_ESCAPE, VK_HOME, VK_LCONTROL, VK_LEFT, VK_LMENU, VK_LSHIFT, VK_LWIN,
    VK_MENU, VK_NEXT, VK_NUMLOCK, VK_PACKET, VK_PRIOR, VK_PROCESSKEY, VK_RCONTROL, VK_RETURN,
    VK_RIGHT, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SCROLL, VK_SHIFT, VK_TAB, VK_UP,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetForegroundWindow, GetWindowThreadProcessId, HC_ACTION, HHOOK,
    KBDLLHOOKSTRUCT, SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL, WH_MOUSE_LL,
    WM_KEYDOWN, WM_LBUTTONDOWN, WM_MBUTTONDOWN, WM_RBUTTONDOWN, WM_SYSKEYDOWN, WM_XBUTTONDOWN,
};

/// `ToUnicodeEx` flag: do not change the keyboard state (Windows 10 1607+),
/// so dead keys typed by the user still compose in the target app.
const TOUNICODE_NO_STATE_CHANGE: u32 = 0x4;

static TX: OnceLock<Sender<Event>> = OnceLock::new();
static ENABLED: AtomicBool = AtomicBool::new(true);

thread_local! {
    /// A high surrogate from a `VK_PACKET` event, waiting for its low half
    /// (emoji panel). Only the hook thread touches it.
    static PACKET: Cell<Utf16Joiner> = const { Cell::new(Utf16Joiner::new()) };
}

/// Turns translation on or off (tray Enable/Disable). When off, the procs
/// pass every event straight on.
pub fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
}

/// Both installed hooks. Dropping this removes them. Must be created and
/// dropped on the thread that runs the message loop.
pub struct Hooks {
    keyboard: HHOOK,
    mouse: HHOOK,
}

impl Drop for Hooks {
    fn drop(&mut self) {
        // SAFETY: both handles came from SetWindowsHookExW on this thread.
        unsafe {
            UnhookWindowsHookEx(self.keyboard);
            UnhookWindowsHookEx(self.mouse);
        }
    }
}

/// Installs both hooks on the calling thread, which must run a message
/// loop. Events go to `tx`. Can be called once per process.
pub fn install(tx: Sender<Event>) -> io::Result<Hooks> {
    if TX.set(tx).is_err() {
        return Err(io::Error::other("hooks already installed"));
    }
    // SAFETY: a null name returns this exe's module handle; the procs are
    // `extern "system"` functions with the HOOKPROC signature.
    unsafe {
        let module = GetModuleHandleW(std::ptr::null());
        let keyboard = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), module, 0);
        if keyboard.is_null() {
            return Err(io::Error::last_os_error());
        }
        let mouse = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), module, 0);
        if mouse.is_null() {
            let err = io::Error::last_os_error();
            UnhookWindowsHookEx(keyboard);
            return Err(err);
        }
        Ok(Hooks { keyboard, mouse })
    }
}

fn emit(input: KeyInput) {
    if let Some(tx) = TX.get() {
        // Unbounded channel: never blocks. A closed worker is ignored.
        let _ = tx.send(Event::Key(input));
    }
}

fn typed(out: &mut impl FnMut(KeyInput), t: Typed) {
    out(match t {
        Typed::Char(c) => KeyInput::Char(c),
        Typed::Reset => KeyInput::Reset,
    })
}

/// A non-packet event arrived: a high surrogate still waiting for its low
/// half will never be completed, so drop it and reset.
fn interrupt_packet(out: &mut impl FnMut(KeyInput)) {
    let mut j = PACKET.get();
    if j.interrupt() {
        PACKET.set(j);
        out(KeyInput::Reset);
    }
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && ENABLED.load(Ordering::Relaxed) {
        // SAFETY: for HC_ACTION, lparam points to a KBDLLHOOKSTRUCT.
        let kb = unsafe { &*(lparam as *const KBDLLHOOKSTRUCT) };
        let msg = wparam as u32;
        // Our own injected events are tagged; they are passed on untouched.
        if kb.dwExtraInfo != INJECT_TAG && (msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN) {
            translate(kb, &mut emit);
        }
    }
    // SAFETY: always pass the event on; we never swallow input.
    unsafe { CallNextHookEx(null_mut(), code, wparam, lparam) }
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && ENABLED.load(Ordering::Relaxed) {
        match wparam as u32 {
            WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN => {
                // Drops a waiting surrogate half; this Reset covers it.
                interrupt_packet(&mut |_| {});
                emit(KeyInput::Reset)
            }
            _ => {}
        }
    }
    // SAFETY: pass the event on unchanged.
    unsafe { CallNextHookEx(null_mut(), code, wparam, lparam) }
}

fn down(vk: u16) -> bool {
    // SAFETY: plain query, no pointers.
    unsafe { GetAsyncKeyState(vk as i32) < 0 }
}

/// Turns one key-down into zero or more [`KeyInput`]s.
fn translate(kb: &KBDLLHOOKSTRUCT, out: &mut impl FnMut(KeyInput)) {
    let vk = kb.vkCode as u16;
    match vk {
        // Modifiers and locks on their own do nothing.
        VK_SHIFT | VK_LSHIFT | VK_RSHIFT | VK_CONTROL | VK_LCONTROL | VK_RCONTROL | VK_MENU
        | VK_LMENU | VK_RMENU | VK_CAPITAL | VK_NUMLOCK | VK_SCROLL => return,
        VK_LWIN | VK_RWIN => return out(KeyInput::Reset),
        _ => {}
    }
    if vk != VK_PACKET {
        interrupt_packet(out);
    }

    let ctrl = down(VK_CONTROL);
    let alt = down(VK_MENU);
    let win = down(VK_LWIN) || down(VK_RWIN);
    // Ctrl+Alt together is AltGr: keep translating. Ctrl or Alt alone, or
    // any Win combo, is a shortcut that may move the caret.
    if is_shortcut(ctrl, alt, win) {
        return out(KeyInput::Reset);
    }

    match vk {
        VK_BACK => out(KeyInput::Backspace),
        // D-15: the app acts on Enter and Tab before we could, so they only
        // reset and never fire a trigger.
        VK_RETURN | VK_TAB => out(KeyInput::Reset),
        VK_LEFT | VK_RIGHT | VK_UP | VK_DOWN | VK_HOME | VK_END | VK_PRIOR | VK_NEXT
        | VK_ESCAPE | VK_PROCESSKEY => out(KeyInput::Reset),
        // Unicode input injected by another tool (emoji panel, on-screen
        // keyboard, ...): the scan code holds one UTF-16 unit, so an emoji
        // comes as two events and is joined here.
        VK_PACKET => {
            let mut j = PACKET.get();
            j.push((kb.scanCode & 0xFFFF) as u16, &mut |t| typed(out, t));
            PACKET.set(j);
        }
        // Here Ctrl and Alt are either both held (AltGr) or both up.
        _ => to_unicode(kb, ctrl && alt, out),
    }
}

/// Translates a character key with the foreground window's layout.
/// `ctrl_and_alt`: Ctrl+Alt (AltGr) is held.
fn to_unicode(kb: &KBDLLHOOKSTRUCT, ctrl_and_alt: bool, out: &mut impl FnMut(KeyInput)) {
    // Built from the async state: GetKeyboardState on this thread would be
    // stale, because this thread never receives keyboard input.
    let mut state = [0u8; 256];
    for vk in [
        VK_SHIFT,
        VK_LSHIFT,
        VK_RSHIFT,
        VK_CONTROL,
        VK_LCONTROL,
        VK_RCONTROL,
        VK_MENU,
        VK_LMENU,
        VK_RMENU,
    ] {
        if down(vk) {
            state[vk as usize] = 0x80;
        }
    }
    // CapsLock: the low-order bit of GetKeyState is the toggle state.
    // Caveat (needs a Windows test, verifier defect 10): GetKeyState is
    // per-thread input state, and this thread never receives keyboard
    // messages, so the toggle could in theory be stale, as GetKeyboardState
    // is above. In practice the toggle is commonly reported correctly from
    // LL hooks. If a test with CapsLock on shows the wrong case, track the
    // toggle from VK_CAPITAL key-downs here instead.
    // SAFETY: plain query, no pointers.
    let caps_on = (unsafe { GetKeyState(VK_CAPITAL as i32) } & 0x0001) != 0;
    if caps_on {
        state[VK_CAPITAL as usize] = 0x01;
    }

    let mut buf = [0u16; 8];
    // SAFETY: `state` is 256 bytes and `buf` is valid for `buf.len()` units.
    let n = unsafe {
        let thread = GetWindowThreadProcessId(GetForegroundWindow(), null_mut());
        let layout = GetKeyboardLayout(thread);
        ToUnicodeEx(
            kb.vkCode,
            kb.scanCode,
            state.as_ptr(),
            buf.as_mut_ptr(),
            buf.len() as i32,
            TOUNICODE_NO_STATE_CHANGE,
            layout,
        )
    };
    // Dead keys and Ctrl+Alt shortcuts that type nothing reset (see
    // `interpret_to_unicode`).
    match interpret_to_unicode(n, ctrl_and_alt, buf.len()) {
        Translation::Reset => out(KeyInput::Reset),
        Translation::Nothing => {}
        Translation::Units(n) => {
            // A full surrogate pair arrives in one call; a dangling half
            // resets.
            let mut j = Utf16Joiner::new();
            for &u in &buf[..n] {
                j.push(u, &mut |t| typed(out, t));
            }
            if j.interrupt() {
                out(KeyInput::Reset);
            }
        }
    }
}
