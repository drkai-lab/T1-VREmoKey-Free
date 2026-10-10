//! Free VRChat expression hotkeys.
//!
//! Shift+F1 through Shift+F4 send an OSC bool while the key is held.
//! This crate does not open a device or send a device report.

pub mod chord;
pub mod cli;
#[cfg(any(test, feature = "emulate"))]
pub mod emulate;
pub mod focus;
pub mod gui;
pub mod i18n;
pub mod keys;
pub mod listener;
pub mod osc;
pub mod paths;
pub mod profile;
pub mod settings;
pub mod tray;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const APP_NAME: &str = "vrc-expression-hotkeys";
pub const DISPLAY_NAME: &str = "T1-VREmoKey-Free";
pub const SLOT_COUNT: i64 = 4;
pub const DEFAULT_AVATAR_ID: &str = "default";
pub const OSC_PORT: u16 = 9000;
pub const AVATAR_LISTEN_PORT: u16 = 9001;

pub const FKEY_NAMES: [&str; 4] = ["F1", "F2", "F3", "F4"];

pub fn fkey_name(number: i64) -> Option<&'static str> {
    FKEY_NAMES.get((number as usize).wrapping_sub(1)).copied()
}
