//! The Win32 layer.
//!
//! Modules gated `any(windows, meh_typecheck)` use only `windows-sys` (and
//! `notify`), so they type-check on Linux with
//! `RUSTFLAGS="--cfg meh_typecheck" cargo check --lib`. `tray` and `app` use
//! `tray-icon`, a Windows-only dependency, and are `cfg(windows)` only.

pub mod clipboard;
pub mod hook;
pub mod inject;
pub mod single_instance;

/// One translated keyboard or mouse event from the hooks. It never leaves
/// memory and is never logged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyInput {
    /// A typed character. Enter arrives as `'\n'`, Tab as `'\t'`.
    Char(char),
    /// Backspace: drop one buffered character.
    Backspace,
    /// The caret may have moved: clear the buffer.
    Reset,
}

/// Messages to the worker thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Key(KeyInput),
    /// Reload the match files (watcher or tray menu).
    Reload,
    /// Tray Enable/Disable.
    SetEnabled(bool),
    Quit,
}

/// UTF-16, NUL-terminated, for `PCWSTR` arguments.
pub(crate) fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
