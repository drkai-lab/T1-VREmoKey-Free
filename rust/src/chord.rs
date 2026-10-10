//! Shift+F chord. One TRUE on press, one FALSE on release, no repeat.
//!
//! F5 through F12 are ignored. A press that the focus gate rejects stays
//! blocked until that key comes up, so a later repeat cannot sneak through.

use std::collections::{HashMap, HashSet};

pub const KEY_F1: u16 = 59;
pub const KERNEL_EV_KEY: u16 = 0x01;

const EVDEV_F: [(i64, u16); 12] = [
    (1, 59),
    (2, 60),
    (3, 61),
    (4, 62),
    (5, 63),
    (6, 64),
    (7, 65),
    (8, 66),
    (9, 67),
    (10, 68),
    (11, 87),
    (12, 88),
];

pub fn evdev_fkey(code: u16) -> Option<&'static str> {
    fkey_from(code, true)
}

pub fn vk_fkey(code: u16) -> Option<&'static str> {
    fkey_from(code, false)
}

pub fn is_ignored_evdev(code: u16) -> bool {
    EVDEV_F
        .iter()
        .any(|(number, ev)| *number > 4 && *ev == code)
}

pub fn is_ignored_vk(code: u16) -> bool {
    (5..=12).any(|number| code == 0x6F + number as u16)
}

fn is_shift_evdev(code: u16) -> bool {
    matches!(code, 42 | 54)
}

fn is_shift_vk(code: u16) -> bool {
    matches!(code, 0x10 | 0xA0 | 0xA1)
}

fn fkey_from(code: u16, evdev: bool) -> Option<&'static str> {
    if evdev {
        EVDEV_F
            .iter()
            .find_map(|(number, ev)| (*ev == code && *number <= 4).then_some(name_for(*number)))
    } else {
        (1..=4).find_map(|number| (code == 0x6F + number as u16).then_some(name_for(number)))
    }
}

fn name_for(number: i64) -> &'static str {
    match number {
        1 => "F1",
        2 => "F2",
        3 => "F3",
        4 => "F4",
        _ => "",
    }
}

#[derive(Debug, Default)]
pub struct ChordTracker {
    shift: HashMap<String, HashSet<u16>>,
    latched: HashMap<String, HashSet<String>>,
    blocked: HashMap<String, HashSet<String>>,
}

impl ChordTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn shift_held(&self) -> bool {
        self.shift.values().any(|held| !held.is_empty())
    }

    pub fn would_press(&self, device: &str, code: u16, value: i32, evdev: bool) -> bool {
        if is_ignored_evdev(code) || is_ignored_vk(code) || value != 1 {
            return false;
        }
        let Some(name) = fkey_from(code, evdev) else {
            return false;
        };
        if !self.shift_held() {
            return false;
        }
        if self
            .blocked
            .get(device)
            .is_some_and(|names| names.contains(name))
        {
            return false;
        }
        !self
            .latched
            .get(device)
            .is_some_and(|names| names.contains(name))
    }

    pub fn note_evdev(
        &mut self,
        device: &str,
        code: u16,
        value: i32,
        accept_press: bool,
    ) -> Vec<(String, bool)> {
        if is_ignored_evdev(code) {
            return Vec::new();
        }
        self.note(device, code, value, true, accept_press)
    }

    pub fn note_vk(
        &mut self,
        device: &str,
        vk: u16,
        is_up: bool,
        accept_press: bool,
    ) -> Vec<(String, bool)> {
        if is_ignored_vk(vk) {
            return Vec::new();
        }
        let value = if is_up { 0 } else { 1 };
        self.note(device, vk, value, false, accept_press)
    }

    pub fn drop_device(&mut self, device: &str) -> Vec<(String, bool)> {
        self.shift.remove(device);
        self.blocked.remove(device);
        let mut released = self.take(device);
        if !self.shift_held() {
            released.extend(self.take_all());
        }
        released
    }

    fn note(
        &mut self,
        device: &str,
        code: u16,
        value: i32,
        evdev: bool,
        accept_press: bool,
    ) -> Vec<(String, bool)> {
        let shift = if evdev {
            is_shift_evdev(code)
        } else {
            is_shift_vk(code)
        };
        if shift {
            return self.set_shift(device, code, value != 0);
        }
        let Some(name) = fkey_from(code, evdev) else {
            return Vec::new();
        };
        if value == 2 {
            return Vec::new();
        }
        if value == 1 {
            if self
                .blocked
                .get(device)
                .is_some_and(|names| names.contains(name))
            {
                return Vec::new();
            }
            if !self.shift_held() {
                return Vec::new();
            }
            if self
                .latched
                .get(device)
                .is_some_and(|names| names.contains(name))
            {
                return Vec::new();
            }
            if !accept_press {
                self.blocked
                    .entry(device.to_string())
                    .or_default()
                    .insert(name.to_string());
                return Vec::new();
            }
            self.latched
                .entry(device.to_string())
                .or_default()
                .insert(name.to_string());
            return vec![(name.to_string(), true)];
        }
        if value == 0 {
            if let Some(blocked) = self.blocked.get_mut(device) {
                if blocked.contains(name) {
                    blocked.remove(name);
                    if blocked.is_empty() {
                        self.blocked.remove(device);
                    }
                    return Vec::new();
                }
            }
            let Some(latched) = self.latched.get_mut(device) else {
                return Vec::new();
            };
            if !latched.contains(name) {
                return Vec::new();
            }
            latched.remove(name);
            if latched.is_empty() {
                self.latched.remove(device);
            }
            return vec![(name.to_string(), false)];
        }
        Vec::new()
    }

    fn set_shift(&mut self, device: &str, code: u16, down: bool) -> Vec<(String, bool)> {
        let held = self.shift.entry(device.to_string()).or_default();
        if down {
            held.insert(code);
            return Vec::new();
        }
        held.remove(&code);
        if held.is_empty() {
            self.shift.remove(device);
        }
        if !self.shift_held() {
            return self.take_all();
        }
        Vec::new()
    }

    fn take(&mut self, device: &str) -> Vec<(String, bool)> {
        let Some(names) = self.latched.remove(device) else {
            return Vec::new();
        };
        let mut ordered: Vec<String> = names.into_iter().collect();
        ordered.sort_by_key(|name| fkey_ord(name));
        ordered.into_iter().map(|name| (name, false)).collect()
    }

    fn take_all(&mut self) -> Vec<(String, bool)> {
        let devices: Vec<String> = self.latched.keys().cloned().collect();
        let mut events = Vec::new();
        for device in devices {
            events.extend(self.take(&device));
        }
        events
    }
}

fn fkey_ord(name: &str) -> u32 {
    name.trim_start_matches('F').parse().unwrap_or(0)
}

/// True when the evdev key bitmap includes F1 or either Shift key.
pub fn looks_like_keyboard(buf: &[u8]) -> bool {
    bits_include(buf, KEY_F1) || bits_include(buf, 42) || bits_include(buf, 54)
}

fn bits_include(buf: &[u8], code: u16) -> bool {
    let index = (code / 8) as usize;
    if index >= buf.len() {
        return false;
    }
    buf[index] & (1 << (code % 8)) != 0
}
