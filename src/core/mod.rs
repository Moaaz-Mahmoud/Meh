//! Pure, cross-platform logic: config parsing, matching, expansion, reload
//! state and the in-memory problems list.

/// The display name in the tray tooltip, notifications and message boxes
/// (D-04). The exe, folder and crate name stay lowercase `meh`.
pub const APP_NAME: &str = "Meh";

pub mod config;
pub mod expand;
pub mod ident;
pub mod input;
pub mod matcher;
pub mod problems;
pub mod reload;
