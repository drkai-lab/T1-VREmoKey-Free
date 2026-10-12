//! Chord listener and the `/avatar/change` socket.
//!
//! A busy avatar port is reported and does not stop the process.

use std::io::ErrorKind;
use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::chord::ChordTracker;
use crate::focus::{observe_evdev, observe_vk, probe_focus, FocusGate};
use crate::keys::{open_key_source, KeyEvent, KeyProblem, KeySource};
use crate::osc::{parse_osc_packet, OscSender, OscTyped};
use crate::paths::{log_line, AppPaths};
use crate::profile::note_avatar_change;
use crate::settings::{self, avatar_listen_port, resolve_config, sender_from, Resolved};

type SourceFactory = Arc<dyn Fn() -> Option<Box<dyn KeySource>> + Send + Sync>;
type Detect = Box<dyn FnMut() -> Option<bool>>;

struct Shared {
    last: Mutex<String>,
    focused: Mutex<Option<bool>>,
    held: Mutex<Option<String>>,
    problem: Mutex<KeyProblem>,
}

pub struct ExpressionListener {
    paths: AppPaths,
    shared: Arc<Shared>,
    stop: Arc<AtomicBool>,
    thread: Mutex<Option<JoinHandle<()>>>,
    open_source: SourceFactory,
}

impl ExpressionListener {
    pub fn new(paths: AppPaths) -> Self {
        Self::with_source(paths, open_key_source)
    }

    pub fn with_source<F>(paths: AppPaths, open: F) -> Self
    where
        F: Fn() -> Option<Box<dyn KeySource>> + Send + Sync + 'static,
    {
        Self {
            paths,
            shared: Arc::new(Shared {
                last: Mutex::new(String::new()),
                focused: Mutex::new(None),
                held: Mutex::new(None),
                problem: Mutex::new(KeyProblem::Ok),
            }),
            stop: Arc::new(AtomicBool::new(false)),
            thread: Mutex::new(None),
            open_source: Arc::new(open),
        }
    }

    pub fn is_running(&self) -> bool {
        self.thread
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .as_ref()
            .is_some_and(|thread| !thread.is_finished())
    }

    pub fn last(&self) -> String {
        self.shared
            .last
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .clone()
    }

    /// The F-key whose expression is currently on. Release clears it.
    pub fn held(&self) -> Option<String> {
        self.shared
            .held
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .clone()
    }

    pub fn key_problem(&self) -> KeyProblem {
        *self
            .shared
            .problem
            .lock()
            .unwrap_or_else(|err| err.into_inner())
    }

    pub fn start(&self) {
        let mut slot = self.thread.lock().unwrap_or_else(|err| err.into_inner());
        if slot.as_ref().is_some_and(|thread| !thread.is_finished()) {
            return;
        }
        self.stop.store(false, Ordering::SeqCst);
        let paths = self.paths.clone();
        let shared = Arc::clone(&self.shared);
        let stop = Arc::clone(&self.stop);
        let open_source = Arc::clone(&self.open_source);
        *slot = thread::Builder::new()
            .name("expressions".into())
            .spawn(move || listen_loop(paths, shared, stop, open_source))
            .ok();
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
        let handle = self
            .thread
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .take();
        if let Some(handle) = handle {
            let _ = handle.join();
        }
        *self
            .shared
            .held
            .lock()
            .unwrap_or_else(|err| err.into_inner()) = None;
    }
}

fn listen_loop(
    paths: AppPaths,
    shared: Arc<Shared>,
    stop: Arc<AtomicBool>,
    open_source: SourceFactory,
) {
    let mut source: Option<Box<dyn KeySource>> = None;
    let mut tracker = ChordTracker::new();
    let mut mtime = None;
    let mut cfg: Option<Resolved> = None;
    let mut osc = OscSender::default();
    let mut gate: Option<FocusGate<Detect>> = None;
    while !stop.load(Ordering::SeqCst) {
        let current = settings::settings_mtime(&paths);
        if cfg.is_none() || current != mtime {
            mtime = current;
            let settings = settings::load_settings(&paths);
            let resolved = resolve_config(&settings, &paths.config_dir);
            osc = sender_from(&settings);
            let target = resolved.target.clone();
            let watched = target.clone();
            let detect: Detect = Box::new(move || probe_focus(&watched));
            gate = Some(FocusGate::new(target, detect).with_paths(paths.clone()));
            cfg = Some(resolved);
        }
        let enabled = cfg.as_ref().is_some_and(|item| item.enabled);
        if !enabled {
            close_source(&mut source);
            set_problem(&shared, KeyProblem::Ok);
            if stop_sleep(&stop, Duration::from_millis(400)) {
                break;
            }
            continue;
        }
        if source.is_none() {
            source = open_source();
        }
        if source.is_none() {
            set_problem(&shared, KeyProblem::Unavailable);
            if stop_sleep(&stop, Duration::from_secs(1)) {
                break;
            }
            continue;
        }
        let batch = source
            .as_mut()
            .map(|item| item.poll(Duration::from_millis(250)))
            .unwrap_or_default();
        if let Some(item) = source.as_ref() {
            set_problem(&shared, item.problem());
        }
        if let (Some(cfg), Some(gate)) = (cfg.as_ref(), gate.as_mut()) {
            emit(&paths, &shared, cfg, &osc, &mut tracker, gate, &batch);
        }
    }
    close_source(&mut source);
}

fn emit(
    paths: &AppPaths,
    shared: &Shared,
    cfg: &Resolved,
    osc: &OscSender,
    tracker: &mut ChordTracker,
    gate: &mut FocusGate<impl FnMut() -> Option<bool>>,
    batch: &[KeyEvent],
) {
    for item in batch {
        let events = match item {
            KeyEvent::Drop { device } => tracker.drop_device(device),
            KeyEvent::Evdev {
                device,
                code,
                value,
            } => observe_evdev(tracker, gate, device, *code, *value),
            KeyEvent::Vk { vk, is_up } => observe_vk(tracker, gate, "win", *vk, *is_up),
        };
        for (name, down) in events {
            let Some(address) = cfg.hotkeys.get(&name) else {
                continue;
            };
            osc.send_bool(address, down);
            note_held(shared, &name, down);
            let state = if down { "DOWN" } else { "UP" };
            let line = format!("Shift+{name} {state}");
            *shared.last.lock().unwrap_or_else(|err| err.into_inner()) = line.clone();
            *shared.focused.lock().unwrap_or_else(|err| err.into_inner()) = gate.last_state;
            log_line(paths, &format!("{line} -> {address}"));
        }
    }
}

fn note_held(shared: &Shared, name: &str, down: bool) {
    let mut held = shared.held.lock().unwrap_or_else(|err| err.into_inner());
    if down {
        *held = Some(name.to_string());
    } else if held.as_deref() == Some(name) {
        *held = None;
    }
}

fn set_problem(shared: &Shared, problem: KeyProblem) {
    *shared.problem.lock().unwrap_or_else(|err| err.into_inner()) = problem;
}

fn close_source(source: &mut Option<Box<dyn KeySource>>) {
    if let Some(source) = source.as_mut() {
        source.close();
    }
    source.take();
}

fn stop_sleep(stop: &AtomicBool, duration: Duration) -> bool {
    let step = Duration::from_millis(50);
    let mut left = duration;
    while !left.is_zero() {
        if stop.load(Ordering::SeqCst) {
            return true;
        }
        let slice = left.min(step);
        thread::sleep(slice);
        left = left.saturating_sub(slice);
    }
    stop.load(Ordering::SeqCst)
}

pub struct AvatarOscListener {
    port: u16,
    host: String,
    on_change: Arc<dyn Fn(String) + Send + Sync>,
    error: Mutex<String>,
    ok: AtomicBool,
    stop: Arc<AtomicBool>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl AvatarOscListener {
    pub fn new<F>(port: u16, host: &str, on_change: F) -> Self
    where
        F: Fn(String) + Send + Sync + 'static,
    {
        Self {
            port,
            host: host.to_string(),
            on_change: Arc::new(on_change),
            error: Mutex::new(String::new()),
            ok: AtomicBool::new(false),
            stop: Arc::new(AtomicBool::new(false)),
            thread: Mutex::new(None),
        }
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn ok(&self) -> bool {
        self.ok.load(Ordering::SeqCst)
    }

    pub fn error(&self) -> String {
        self.error
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .clone()
    }

    pub fn running(&self) -> bool {
        self.thread
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .as_ref()
            .is_some_and(|thread| !thread.is_finished())
    }

    pub fn start(&self) -> bool {
        self.stop.store(false, Ordering::SeqCst);
        let socket = match UdpSocket::bind((self.host.as_str(), self.port)) {
            Ok(socket) => socket,
            Err(err) => {
                *self
                    .error
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner()) = err.to_string();
                self.ok.store(false, Ordering::SeqCst);
                return false;
            }
        };
        let _ = socket.set_read_timeout(Some(Duration::from_millis(300)));
        self.ok.store(true, Ordering::SeqCst);
        *self
            .error
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = String::new();
        let stop = Arc::clone(&self.stop);
        let on_change = Arc::clone(&self.on_change);
        let handle = thread::Builder::new()
            .name("avatar-osc".into())
            .spawn(move || avatar_loop(socket, stop, on_change))
            .ok();
        *self.thread.lock().unwrap_or_else(|err| err.into_inner()) = handle;
        true
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
        let handle = self
            .thread
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .take();
        if let Some(handle) = handle {
            let _ = handle.join();
        }
    }
}

fn avatar_loop(
    socket: UdpSocket,
    stop: Arc<AtomicBool>,
    on_change: Arc<dyn Fn(String) + Send + Sync>,
) {
    let mut buf = [0u8; 4096];
    while !stop.load(Ordering::SeqCst) {
        match socket.recv_from(&mut buf) {
            Ok((size, _)) => {
                let Some((address, value)) = parse_osc_packet(&buf[..size]) else {
                    continue;
                };
                if !address.ends_with("/avatar/change") {
                    continue;
                }
                let OscTyped::String(text) = value else {
                    continue;
                };
                let text = text.trim().to_string();
                if text.is_empty() {
                    continue;
                }
                on_change(text);
            }
            Err(err)
                if err.kind() == ErrorKind::WouldBlock || err.kind() == ErrorKind::TimedOut => {}
            Err(_) => break,
        }
    }
}

pub fn run_listen(paths: &AppPaths) -> i32 {
    log_line(
        paths,
        &format!("listener starting (pid {})", std::process::id()),
    );
    let listener = ExpressionListener::new(paths.clone());
    listener.start();
    let settings = settings::load_settings(paths);
    let port = avatar_listen_port(&settings);
    let paths_for_avatar = paths.clone();
    let paths_for_log = paths.clone();
    let avatar = AvatarOscListener::new(port, "127.0.0.1", move |avatar_id| {
        if let Err(err) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            note_avatar_change(&paths_for_avatar, &avatar_id);
        })) {
            let _ = err;
            log_line(&paths_for_log, "avatar change failed");
        }
    });
    if !avatar.start() {
        log_line(
            paths,
            &format!("avatar OSC port {port} is busy: {}", avatar.error()),
        );
    }
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    let _ = ctrlc::set_handler(move || {
        let _ = tx.send(());
    });
    let _ = rx.recv();
    avatar.stop();
    listener.stop();
    0
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::thread;
    use std::time::{Duration, Instant};

    use super::note_held;
    use crate::keys::{KeyEvent, KeyProblem, KeySource};
    use crate::listener::ExpressionListener;
    use crate::paths::AppPaths;

    #[test]
    fn held_follows_press_and_ignores_another_release() {
        let listener = ExpressionListener::new(AppPaths::in_dir(Path::new(".")));
        note_held(&listener.shared, "F2", true);
        assert_eq!(listener.held().as_deref(), Some("F2"));
        note_held(&listener.shared, "F1", false);
        assert_eq!(listener.held().as_deref(), Some("F2"));
        note_held(&listener.shared, "F2", false);
        assert!(listener.held().is_none());
    }

    #[test]
    fn missing_key_source_reports_why_listening_cannot_start() {
        let dir = tempfile::tempdir().unwrap();
        let listener = ExpressionListener::with_source(AppPaths::in_dir(dir.path()), || None);
        listener.start();
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline && listener.key_problem() != KeyProblem::Unavailable {
            thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(listener.key_problem(), KeyProblem::Unavailable);
        listener.stop();
    }

    struct DeniedKeys;

    impl KeySource for DeniedKeys {
        fn poll(&mut self, timeout: Duration) -> Vec<KeyEvent> {
            thread::sleep(timeout.min(Duration::from_millis(30)));
            Vec::new()
        }

        fn close(&mut self) {}

        fn problem(&self) -> KeyProblem {
            KeyProblem::PermissionDenied
        }
    }

    #[test]
    fn permission_failure_reaches_the_listener() {
        let dir = tempfile::tempdir().unwrap();
        let listener = ExpressionListener::with_source(AppPaths::in_dir(dir.path()), || {
            Some(Box::new(DeniedKeys))
        });
        listener.start();
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline && listener.key_problem() != KeyProblem::PermissionDenied {
            thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(listener.key_problem(), KeyProblem::PermissionDenied);
        listener.stop();
    }
}
