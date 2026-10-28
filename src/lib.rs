//! meh: a minimal espanso-style text expander for Windows.
//!
//! `core` is pure and cross-platform (tested on Linux); the Win32 layer
//! lives in `win`. On Linux, `win` is only type-checked, with
//! `RUSTFLAGS="--cfg meh_typecheck" cargo check --lib`.

pub mod core;
pub mod icon;
#[cfg(any(windows, meh_typecheck))]
pub mod win;
