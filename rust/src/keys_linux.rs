//! Readable keyboards via evdev. The device is never grabbed.

use std::fs;
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use evdev::{Device, EventType, KeyCode};

use crate::keys::{KeyEvent, KeyProblem};
use crate::paths::{log_line, AppPaths};

pub struct LinuxKeySource {
    devices: Vec<OpenDev>,
    next_scan: Instant,
    perm_warned: bool,
    problem: KeyProblem,
}

struct OpenDev {
    path: String,
    device: Device,
}

impl LinuxKeySource {
    pub fn new() -> Self {
        let mut source = Self {
            devices: Vec::new(),
            next_scan: Instant::now(),
            perm_warned: false,
            problem: KeyProblem::Ok,
        };
        source.rescan();
        source
    }

    fn rescan(&mut self) -> Vec<String> {
        let wanted = list_event_nodes();
        let mut denied = Vec::new();
        for path in &wanted {
            if self.devices.iter().any(|dev| dev.path == *path) {
                continue;
            }
            match open_keyboard(path) {
                Opened::Denied => denied.push(path.clone()),
                Opened::Device(device) => self.devices.push(OpenDev {
                    path: path.clone(),
                    device: *device,
                }),
                Opened::Skip => {}
            }
        }
        let mut dropped = Vec::new();
        self.devices.retain(|dev| {
            if wanted.iter().any(|path| path == &dev.path) {
                true
            } else {
                dropped.push(dev.path.clone());
                false
            }
        });
        self.problem = classify_keyboards(self.devices.len(), denied.len());
        if self.problem == KeyProblem::PermissionDenied && !self.perm_warned {
            self.perm_warned = true;
            log_line(
                &AppPaths::system(),
                &format!(
                    "no keyboard is readable ({}). Add your user to the input group and sign in again: sudo usermod -aG input \"$USER\"",
                    denied.join(", ")
                ),
            );
        }
        self.next_scan = Instant::now() + Duration::from_secs(1);
        dropped
    }
}

fn classify_keyboards(open_count: usize, denied_count: usize) -> KeyProblem {
    if open_count > 0 {
        KeyProblem::Ok
    } else if denied_count > 0 {
        KeyProblem::PermissionDenied
    } else {
        KeyProblem::NoKeyboard
    }
}

impl crate::keys::KeySource for LinuxKeySource {
    fn poll(&mut self, timeout: Duration) -> Vec<KeyEvent> {
        #[cfg(any(test, feature = "emulate"))]
        {
            crate::emulate::note_hook_poll();
            if crate::emulate::suppress_real_keys() {
                let events = drain_emulated();
                if events.is_empty() && !timeout.is_zero() {
                    std::thread::sleep(timeout.min(Duration::from_millis(40)));
                }
                return events;
            }
            let injected = drain_emulated();
            if !injected.is_empty() {
                return injected;
            }
        }
        let mut events = Vec::new();
        if Instant::now() >= self.next_scan || self.devices.is_empty() {
            for path in self.rescan() {
                events.push(KeyEvent::Drop { device: path });
            }
        }
        if self.devices.is_empty() {
            if !timeout.is_zero() {
                std::thread::sleep(timeout.min(Duration::from_millis(250)));
            }
            return events;
        }
        let mut fds: Vec<libc::pollfd> = self
            .devices
            .iter()
            .map(|dev| libc::pollfd {
                fd: dev.device.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            })
            .collect();
        let millis = i32::try_from(timeout.as_millis()).unwrap_or(i32::MAX);
        let ready = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, millis) };
        if ready <= 0 {
            return events;
        }
        let mut dead = Vec::new();
        for (index, fd) in fds.iter().enumerate() {
            if fd.revents & (libc::POLLIN | libc::POLLERR | libc::POLLHUP) == 0 {
                continue;
            }
            let path = self.devices[index].path.clone();
            match self.devices[index].device.fetch_events() {
                Ok(batch) => {
                    for ev in batch {
                        if ev.event_type() == EventType::KEY {
                            events.push(KeyEvent::Evdev {
                                device: path.clone(),
                                code: ev.code(),
                                value: ev.value(),
                            });
                        }
                    }
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(_) => dead.push(path),
            }
        }
        for path in dead {
            self.devices.retain(|dev| dev.path != path);
            events.push(KeyEvent::Drop { device: path });
        }
        events
    }

    fn close(&mut self) {
        self.devices.clear();
    }

    fn problem(&self) -> KeyProblem {
        self.problem
    }
}

#[cfg(any(test, feature = "emulate"))]
fn drain_emulated() -> Vec<KeyEvent> {
    crate::emulate::drain_evdev()
        .into_iter()
        .map(|(code, value)| KeyEvent::Evdev {
            device: "emulated-t1".into(),
            code,
            value,
        })
        .collect()
}

enum Opened {
    Device(Box<Device>),
    Denied,
    Skip,
}

fn open_keyboard(path: &str) -> Opened {
    let device = match Device::open(path) {
        Ok(device) => device,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => return Opened::Denied,
        Err(_) => return Opened::Skip,
    };
    if !device_is_keyboard(&device) {
        return Opened::Skip;
    }
    if device.set_nonblocking(true).is_err() {
        return Opened::Skip;
    }
    Opened::Device(Box::new(device))
}

fn device_is_keyboard(device: &Device) -> bool {
    let Some(keys) = device.supported_keys() else {
        return false;
    };
    keys.contains(KeyCode::KEY_F1)
        || keys.contains(KeyCode::KEY_LEFTSHIFT)
        || keys.contains(KeyCode::KEY_RIGHTSHIFT)
}

fn list_event_nodes() -> Vec<String> {
    let Ok(entries) = fs::read_dir("/dev/input") else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            name.starts_with("event").then(|| {
                PathBuf::from("/dev/input")
                    .join(name)
                    .to_string_lossy()
                    .into_owned()
            })
        })
        .collect();
    names.sort();
    names
}

#[cfg(test)]
mod tests {
    use super::classify_keyboards;
    use crate::keys::KeyProblem;

    #[test]
    fn permission_denial_is_not_reported_as_a_missing_keyboard() {
        assert_eq!(classify_keyboards(0, 2), KeyProblem::PermissionDenied);
        assert_eq!(classify_keyboards(0, 0), KeyProblem::NoKeyboard);
        assert_eq!(classify_keyboards(1, 3), KeyProblem::Ok);
    }
}
