//! Key sources. Nothing here grabs the device or swallows a key.

use std::time::Duration;

use crate::focus::Target;

#[derive(Clone, Debug)]
pub enum KeyEvent {
    Evdev {
        device: String,
        code: u16,
        value: i32,
    },
    Vk {
        vk: u16,
        is_up: bool,
    },
    Drop {
        device: String,
    },
}

/// Why a key source cannot deliver keys. `Ok` means at least one keyboard is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyProblem {
    Ok,
    PermissionDenied,
    NoKeyboard,
    Unavailable,
}

pub trait KeySource: Send {
    fn poll(&mut self, timeout: Duration) -> Vec<KeyEvent>;
    fn close(&mut self);
    fn problem(&self) -> KeyProblem {
        KeyProblem::Ok
    }
}

pub fn open_key_source() -> Option<Box<dyn KeySource>> {
    #[cfg(windows)]
    {
        Some(Box::new(windows::WinKeyHook::new()))
    }
    #[cfg(target_os = "linux")]
    {
        Some(Box::new(linux::LinuxKeySource::new()))
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        None
    }
}

#[cfg(windows)]
pub fn windows_focused(target: &Target) -> Option<bool> {
    windows::foreground_matches(target)
}

#[cfg(not(windows))]
pub fn windows_focused(target: &Target) -> Option<bool> {
    let _ = target;
    None
}

#[cfg(target_os = "linux")]
#[path = "keys_linux.rs"]
mod linux;

#[cfg(windows)]
#[path = "keys_windows.rs"]
mod windows;

#[cfg(all(windows, any(test, feature = "emulate")))]
pub fn low_level_hook_ready() -> Option<bool> {
    windows::hook_status()
}
