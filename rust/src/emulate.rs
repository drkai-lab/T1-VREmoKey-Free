//! Stand-in T1 chords and a stand-in VRChat.
//!
//! A T1 button, in this free edition, is only the Shift+F1 through Shift+F4
//! chord that button would type. Those chords are queued under the platform
//! hook: Linux `poll` turns them into the same evdev `KeyEvent`s as
//! `/dev/input`, and Windows `poll` keeps the low-level hook's Shift / F1..F4
//! filter. Release builds omit this module.

use std::collections::VecDeque;
use std::fs;
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tempfile::TempDir;

use crate::focus::probe_focus;
use crate::listener::{AvatarOscListener, ExpressionListener};
use crate::osc::{encode_osc_string, parse_osc_packet, OscTyped};
use crate::paths::AppPaths;
use crate::profile::{self, import_profile_pack, Expression, Profile};
use crate::settings::{self, resolve_config};

const FOCUS_LIVE: u8 = 0;
const FOCUS_ON: u8 = 1;
const FOCUS_OFF: u8 = 2;
const OSC_WAIT: Duration = Duration::from_secs(3);
const SILENT_WAIT: Duration = Duration::from_millis(700);

static FOCUS_MODE: AtomicU8 = AtomicU8::new(FOCUS_LIVE);
static SUPPRESS_REAL: AtomicBool = AtomicBool::new(false);
static REAL_PROBES: AtomicU64 = AtomicU64::new(0);
static HOOK_POLLS: AtomicU64 = AtomicU64::new(0);
static WALK_LOCK: Mutex<()> = Mutex::new(());

#[cfg(windows)]
static HOOK_STOP: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "linux")]
enum Raw {
    Evdev { code: u16, value: i32 },
}

#[cfg(windows)]
enum Raw {
    Vk { vk: u16, is_up: bool },
}

#[cfg(any(target_os = "linux", windows))]
static QUEUE: Mutex<VecDeque<Raw>> = Mutex::new(VecDeque::new());

struct ResetGuard;

impl Drop for ResetGuard {
    fn drop(&mut self) {
        reset_stand_in();
    }
}

fn reset_stand_in() {
    set_focus_live();
    set_suppress_real_keys(false);
    REAL_PROBES.store(0, Ordering::SeqCst);
    HOOK_POLLS.store(0, Ordering::SeqCst);
    #[cfg(any(target_os = "linux", windows))]
    clear_keys();
    #[cfg(windows)]
    clear_hook_stop();
}

pub(crate) fn set_focus_live() {
    FOCUS_MODE.store(FOCUS_LIVE, Ordering::SeqCst);
}

pub(crate) fn set_focus_fixed(focused: bool) {
    FOCUS_MODE.store(if focused { FOCUS_ON } else { FOCUS_OFF }, Ordering::SeqCst);
}

/// `None` keeps the real foreground check. `Some(answer)` replaces it.
pub(crate) fn forced_focus_answer() -> Option<Option<bool>> {
    match FOCUS_MODE.load(Ordering::SeqCst) {
        FOCUS_ON => Some(Some(true)),
        FOCUS_OFF => Some(Some(false)),
        _ => None,
    }
}

pub(crate) fn note_real_focus_probe() {
    REAL_PROBES.fetch_add(1, Ordering::SeqCst);
}

pub(crate) fn real_focus_probes() -> u64 {
    REAL_PROBES.load(Ordering::SeqCst)
}

pub(crate) fn set_suppress_real_keys(suppress: bool) {
    SUPPRESS_REAL.store(suppress, Ordering::SeqCst);
}

pub(crate) fn suppress_real_keys() -> bool {
    SUPPRESS_REAL.load(Ordering::SeqCst)
}

pub(crate) fn note_hook_poll() {
    HOOK_POLLS.fetch_add(1, Ordering::SeqCst);
}

pub(crate) fn hook_polls() -> u64 {
    HOOK_POLLS.load(Ordering::SeqCst)
}

#[cfg(windows)]
pub(crate) fn request_hook_stop() {
    HOOK_STOP.store(true, Ordering::SeqCst);
}

#[cfg(windows)]
pub(crate) fn clear_hook_stop() {
    HOOK_STOP.store(false, Ordering::SeqCst);
}

#[cfg(windows)]
pub(crate) fn hook_should_stop() -> bool {
    HOOK_STOP.load(Ordering::SeqCst)
}

#[cfg(any(target_os = "linux", windows))]
fn clear_keys() {
    QUEUE.lock().unwrap_or_else(|err| err.into_inner()).clear();
}

#[cfg(target_os = "linux")]
fn enqueue_evdev(code: u16, value: i32) {
    QUEUE
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .push_back(Raw::Evdev { code, value });
}

#[cfg(windows)]
fn enqueue_vk(vk: u16, is_up: bool) {
    QUEUE
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .push_back(Raw::Vk { vk, is_up });
}

#[cfg(target_os = "linux")]
pub(crate) fn drain_evdev() -> Vec<(u16, i32)> {
    let mut queue = QUEUE.lock().unwrap_or_else(|err| err.into_inner());
    queue
        .drain(..)
        .map(|raw| {
            let Raw::Evdev { code, value } = raw;
            (code, value)
        })
        .collect()
}

#[cfg(windows)]
pub(crate) fn drain_vk() -> Vec<(u16, bool)> {
    let mut queue = QUEUE.lock().unwrap_or_else(|err| err.into_inner());
    queue
        .drain(..)
        .map(|raw| {
            let Raw::Vk { vk, is_up } = raw;
            (vk, is_up)
        })
        .collect()
}

#[cfg(any(target_os = "linux", windows))]
fn enqueue_shift(down: bool) {
    #[cfg(target_os = "linux")]
    enqueue_evdev(42, if down { 1 } else { 0 });
    #[cfg(windows)]
    enqueue_vk(0xA0, !down);
}

#[cfg(any(target_os = "linux", windows))]
fn enqueue_function(slot: u8, down: bool) {
    #[cfg(target_os = "linux")]
    {
        let code = match slot {
            1 => 59,
            2 => 60,
            3 => 61,
            4 => 62,
            5 => 63,
            _ => return,
        };
        enqueue_evdev(code, if down { 1 } else { 0 });
    }
    #[cfg(windows)]
    {
        if !(1..=12).contains(&slot) {
            return;
        }
        enqueue_vk(0x6F + u16::from(slot), !down);
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
pub fn t1_button(_slot: u8, _down: bool) -> Result<(), String> {
    Err("the stand-in needs the Linux evdev hook or the Windows low-level hook".into())
}

/// Press or release one stand-in button. Slot 1 is F1, slot 4 is F4.
///
/// Down sends Shift then the function key. Up sends the function key then Shift.
#[cfg(any(target_os = "linux", windows))]
pub fn t1_button(slot: u8, down: bool) -> Result<(), String> {
    if !(1..=4).contains(&slot) {
        return Err(format!("button {slot} is outside F1..F4"));
    }
    if down {
        enqueue_shift(true);
        enqueue_function(slot, true);
    } else {
        enqueue_function(slot, false);
        enqueue_shift(false);
    }
    Ok(())
}

#[cfg(any(target_os = "linux", windows))]
fn inject_f5(down: bool) {
    if down {
        enqueue_shift(true);
        enqueue_function(5, true);
    } else {
        enqueue_function(5, false);
        enqueue_shift(false);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Hit {
    address: String,
    value: bool,
}

struct FakeVrchat {
    port: u16,
    log: Arc<Mutex<Vec<Hit>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    send: UdpSocket,
}

impl FakeVrchat {
    fn open() -> Result<Self, String> {
        let socket = UdpSocket::bind("127.0.0.1:0").map_err(|err| err.to_string())?;
        socket
            .set_read_timeout(Some(Duration::from_millis(100)))
            .map_err(|err| err.to_string())?;
        let port = socket.local_addr().map_err(|err| err.to_string())?.port();
        let send = UdpSocket::bind("127.0.0.1:0").map_err(|err| err.to_string())?;
        let log = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let log_thread = Arc::clone(&log);
        let stop_thread = Arc::clone(&stop);
        let thread = thread::Builder::new()
            .name("fake-vrchat".into())
            .spawn(move || recv_osc(socket, log_thread, stop_thread))
            .map_err(|err| err.to_string())?;
        Ok(Self {
            port,
            log,
            stop,
            thread: Some(thread),
            send,
        })
    }

    fn snapshot(&self) -> Vec<Hit> {
        self.log
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .clone()
    }

    fn clear(&self) {
        self.log
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .clear();
    }

    fn send_avatar(&self, port: u16, avatar_id: &str) -> Result<(), String> {
        let packet = encode_osc_string("/avatar/change", avatar_id);
        self.send
            .send_to(&packet, ("127.0.0.1", port))
            .map(|_| ())
            .map_err(|err| err.to_string())
    }
}

impl Drop for FakeVrchat {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Bool packets from this app. An address whose length is already a multiple
/// of 4 is sent without a trailing NUL, matching the long-standing encoder.
fn parse_app_bool(data: &[u8]) -> Option<(String, bool)> {
    if let Some((address, OscTyped::Bool(value))) = parse_osc_packet(data) {
        return Some((address, value));
    }
    let tag = data
        .windows(2)
        .position(|part| part == b",T" || part == b",F")?;
    let value = data[tag + 1] == b'T';
    let raw = &data[..tag];
    let end = raw.iter().position(|byte| *byte == 0).unwrap_or(raw.len());
    if end == 0 || !raw.starts_with(b"/") {
        return None;
    }
    let address = std::str::from_utf8(&raw[..end]).ok()?.to_string();
    Some((address, value))
}

fn recv_osc(socket: UdpSocket, log: Arc<Mutex<Vec<Hit>>>, stop: Arc<AtomicBool>) {
    let mut buf = [0u8; 2048];
    while !stop.load(Ordering::SeqCst) {
        match socket.recv_from(&mut buf) {
            Ok((size, _)) => {
                let Some((address, value)) = parse_app_bool(&buf[..size]) else {
                    continue;
                };
                log.lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .push(Hit { address, value });
            }
            Err(err)
                if err.kind() == ErrorKind::WouldBlock || err.kind() == ErrorKind::TimedOut => {}
            Err(_) => break,
        }
    }
}

struct InjectServer {
    port: u16,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl InjectServer {
    fn bind() -> Result<Self, String> {
        let listener = TcpListener::bind("127.0.0.1:0").map_err(|err| err.to_string())?;
        listener
            .set_nonblocking(true)
            .map_err(|err| err.to_string())?;
        let port = listener.local_addr().map_err(|err| err.to_string())?.port();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = Arc::clone(&stop);
        let thread = thread::Builder::new()
            .name("emulate-inject".into())
            .spawn(move || serve_inject(listener, stop_thread))
            .map_err(|err| err.to_string())?;
        Ok(Self {
            port,
            stop,
            thread: Some(thread),
        })
    }
}

impl Drop for InjectServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn serve_inject(listener: TcpListener, stop: Arc<AtomicBool>) {
    while !stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => handle_client(stream),
            Err(err)
                if err.kind() == ErrorKind::WouldBlock || err.kind() == ErrorKind::Interrupted =>
            {
                thread::sleep(Duration::from_millis(20));
            }
            Err(_) => break,
        }
    }
}

fn handle_client(stream: TcpStream) {
    let _ = stream.set_nodelay(true);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut write = match stream.try_clone() {
        Ok(stream) => stream,
        Err(_) => return,
    };
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    if reader
        .read_line(&mut line)
        .ok()
        .filter(|size| *size > 0)
        .is_none()
    {
        return;
    }
    let response = match apply_command(line.trim()) {
        Ok(()) => "ok\n".to_string(),
        Err(err) => format!("err {err}\n"),
    };
    let _ = write.write_all(response.as_bytes());
}

fn apply_command(line: &str) -> Result<(), String> {
    let mut parts = line.split_whitespace();
    let verb = parts.next().unwrap_or("");
    let arg = parts.next().unwrap_or("");
    if parts.next().is_some() {
        return Err(format!("extra words in {line}"));
    }
    match verb {
        "press" => t1_button(parse_slot(arg)?, true),
        "release" => t1_button(parse_slot(arg)?, false),
        "focus" => match arg {
            "on" => {
                set_focus_fixed(true);
                Ok(())
            }
            "off" => {
                set_focus_fixed(false);
                Ok(())
            }
            "live" => {
                set_focus_live();
                Ok(())
            }
            _ => Err(format!("focus {arg}")),
        },
        _ => Err(format!("unknown command {verb}")),
    }
}

fn parse_slot(text: &str) -> Result<u8, String> {
    let slot: u8 = text.parse().map_err(|_| format!("button {text}"))?;
    if (1..=4).contains(&slot) {
        Ok(slot)
    } else {
        Err(format!("button {slot} is outside F1..F4"))
    }
}

fn remote_command(port: u16, line: &str) -> Result<(), String> {
    let mut last = String::from("inject port is closed");
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        match TcpStream::connect(("127.0.0.1", port)) {
            Ok(mut stream) => {
                let _ = stream.set_nodelay(true);
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
                let text = format!("{line}\n");
                stream
                    .write_all(text.as_bytes())
                    .map_err(|err| err.to_string())?;
                let mut reply = String::new();
                BufReader::new(stream)
                    .read_line(&mut reply)
                    .map_err(|err| err.to_string())?;
                if reply.starts_with("ok") {
                    return Ok(());
                }
                return Err(format!("inject reply {reply}"));
            }
            Err(err) => {
                last = err.to_string();
                thread::sleep(Duration::from_millis(20));
            }
        }
    }
    Err(last)
}

struct Report {
    lines: Vec<String>,
}

impl Report {
    fn new() -> Self {
        Self { lines: Vec::new() }
    }

    fn push(&mut self, line: impl Into<String>) {
        self.lines.push(line.into());
    }

    fn text(&self) -> String {
        let mut text = self.lines.join("\n");
        text.push('\n');
        text
    }

    fn fail(&self, detail: impl AsRef<str>) -> String {
        format!("{}{}", self.text(), detail.as_ref())
    }
}

struct Session {
    dir: TempDir,
    paths: AppPaths,
    vrchat: FakeVrchat,
    listener: ExpressionListener,
    avatar: AvatarOscListener,
    inject: InjectServer,
    avatar_port: u16,
}

impl Drop for Session {
    fn drop(&mut self) {
        self.listener.stop();
        self.avatar.stop();
        let _keep_config = &self.dir;
    }
}

enum Drive {
    Local,
    Remote(u16),
}

/// Configure, confirm, use, and switch avatars. A private temporary directory
/// is used, so the real settings folder is left alone.
#[cfg(any(target_os = "linux", windows))]
pub fn run_walkthrough() -> Result<String, String> {
    let _lock = WALK_LOCK.lock().unwrap_or_else(|err| err.into_inner());
    reset_stand_in();
    let _guard = ResetGuard;
    walkthrough_body()
}

#[cfg(not(any(target_os = "linux", windows)))]
pub fn run_walkthrough() -> Result<String, String> {
    Err("the stand-in walkthrough needs the Linux evdev hook or the Windows low-level hook".into())
}

#[cfg(any(target_os = "linux", windows))]
fn walkthrough_body() -> Result<String, String> {
    let mut report = Report::new();
    let dir = tempfile::tempdir().map_err(|err| err.to_string())?;
    let paths = AppPaths::in_dir(dir.path());
    let vrchat = FakeVrchat::open()?;
    let avatar_port = free_udp_port()?;
    import_two(&paths)?;
    write_settings(&paths, vrchat.port, avatar_port)?;
    confirm_saved(
        &paths,
        "avtr_smile",
        "/avatar/parameters/Smile",
        "/avatar/parameters/Grin",
    )?;
    report.push(format!(
        "configure: imported avtr_smile avtr_wave osc={} avatar={avatar_port}",
        vrchat.port
    ));
    report.push(
        "confirm: avatar=avtr_smile F1=/avatar/parameters/Smile F2=/avatar/parameters/Grin"
            .to_string(),
    );

    set_suppress_real_keys(true);
    let listener = ExpressionListener::new(paths.clone());
    listener.start();
    let paths_for_avatar = paths.clone();
    let avatar = AvatarOscListener::new(avatar_port, "127.0.0.1", move |avatar_id| {
        let _ = profile::note_avatar_change(&paths_for_avatar, &avatar_id);
    });
    if !avatar.start() {
        listener.stop();
        return Err(report.fail(format!("avatar port {avatar_port}: {}", avatar.error())));
    }
    let inject = match InjectServer::bind() {
        Ok(server) => server,
        Err(err) => {
            listener.stop();
            avatar.stop();
            return Err(report.fail(err));
        }
    };
    let session = Session {
        dir,
        paths,
        vrchat,
        listener,
        avatar,
        inject,
        avatar_port,
    };

    if !wait_until(Duration::from_secs(3), || {
        session.listener.is_running() && hook_polls() > 0
    }) {
        return Err(report.fail("hook poll did not run"));
    }
    #[cfg(target_os = "linux")]
    report.push(format!("hook: evdev poll polls={}", hook_polls()));
    #[cfg(windows)]
    {
        let _ = wait_until(Duration::from_secs(2), || {
            crate::keys::low_level_hook_ready().is_some()
        });
        let status = match crate::keys::low_level_hook_ready() {
            Some(true) => "installed",
            Some(false) => "unavailable",
            None => "unknown",
        };
        report.push(format!("hook: low-level {status} polls={}", hook_polls()));
    }

    set_focus_fixed(true);
    expect_chord(&session, Drive::Local, 1, "/avatar/parameters/Smile")
        .map_err(|err| report.fail(format!("use: {err}")))?;
    report.push("use: Shift+F1 /avatar/parameters/Smile press=true release=false".to_string());

    set_focus_fixed(false);
    expect_silent(&session, 1).map_err(|err| report.fail(format!("unfocused: {err}")))?;
    report.push("unfocused: suppressed".to_string());

    set_focus_fixed(true);
    session.vrchat.clear();
    let before_f5 = shift_lines(&session.paths);
    inject_f5(true);
    wait_quiet_osc(&session.vrchat).map_err(|err| report.fail(format!("ignored-f5: {err}")))?;
    inject_f5(false);
    wait_quiet_osc(&session.vrchat).map_err(|err| report.fail(format!("ignored-f5: {err}")))?;
    if shift_lines(&session.paths) != before_f5 {
        return Err(report.fail("ignored-f5: a Shift line was logged"));
    }
    report.push("ignored-f5: suppressed".to_string());

    let target = resolve_config(
        &settings::load_settings(&session.paths),
        &session.paths.config_dir,
    )
    .target;
    let before_probes = real_focus_probes();
    set_focus_live();
    let probed = probe_focus(&target);
    if real_focus_probes() <= before_probes {
        return Err(report.fail("real-focus: the foreground check did not run"));
    }
    let probe_label = match probed {
        Some(true) => "true",
        Some(false) => "false",
        None => "none",
    };
    if matches!(probed, Some(true)) {
        expect_chord(&session, Drive::Local, 1, "/avatar/parameters/Smile")
            .map_err(|err| report.fail(format!("real-focus: {err}")))?;
        report.push(format!("real-focus: {probe_label} sent=true"));
    } else {
        expect_silent(&session, 1).map_err(|err| report.fail(format!("real-focus: {err}")))?;
        report.push(format!("real-focus: {probe_label} sent=false"));
    }

    session
        .vrchat
        .send_avatar(session.avatar_port, "avtr_wave")
        .map_err(|err| report.fail(format!("switch: {err}")))?;
    if !wait_until(Duration::from_secs(3), || {
        settings::load_settings(&session.paths)
            .get("active_avatar")
            .and_then(Value::as_str)
            == Some("avtr_wave")
    }) {
        return Err(report.fail("switch: active avatar stayed on avtr_smile"));
    }
    let log = fs::read_to_string(&session.paths.log_file).unwrap_or_default();
    if !log.contains("avatar -> avtr_wave") {
        return Err(report.fail("switch: avatar change was not logged"));
    }
    thread::sleep(Duration::from_millis(500));
    confirm_saved(
        &session.paths,
        "avtr_wave",
        "/avatar/parameters/Wave",
        "/avatar/parameters/Bye",
    )
    .map_err(|err| report.fail(format!("switch: {err}")))?;
    report.push(
        "switch: avatar=avtr_wave F1=/avatar/parameters/Wave F2=/avatar/parameters/Bye".to_string(),
    );

    set_focus_fixed(true);
    expect_chord(&session, Drive::Local, 1, "/avatar/parameters/Wave")
        .map_err(|err| report.fail(format!("use-after-switch: {err}")))?;
    report.push(
        "use-after-switch: Shift+F1 /avatar/parameters/Wave press=true release=false".to_string(),
    );

    let port = session.inject.port;
    remote_command(port, "focus on").map_err(|err| report.fail(format!("external: {err}")))?;
    expect_chord(&session, Drive::Remote(port), 2, "/avatar/parameters/Bye")
        .map_err(|err| report.fail(format!("external: {err}")))?;
    report.push(format!(
        "external: 127.0.0.1:{port} Shift+F2 /avatar/parameters/Bye press=true release=false"
    ));
    report.push("walkthrough: ok".to_string());
    Ok(report.text())
}

#[cfg(any(target_os = "linux", windows))]
fn import_two(paths: &AppPaths) -> Result<(), String> {
    let smile = avatar_profile(
        "avtr_smile",
        "Smile Avatar",
        &[("Smile", "Smile"), ("Grin", "Grin")],
    );
    let wave = avatar_profile(
        "avtr_wave",
        "Wave Avatar",
        &[("Wave", "Wave"), ("Bye", "Bye")],
    );
    import_one(paths, &smile)?;
    import_one(paths, &wave)?;
    Ok(())
}

#[cfg(any(target_os = "linux", windows))]
fn import_one(paths: &AppPaths, profile: &Profile) -> Result<(), String> {
    let pack = paths.config_dir.join(format!("_pack_{}", profile.id));
    write_pack(&pack, profile)?;
    let imported = import_profile_pack(&pack, paths)?;
    if imported.profile.id != profile.id {
        return Err(format!(
            "imported {} as {}",
            profile.id, imported.profile.id
        ));
    }
    Ok(())
}

#[cfg(any(target_os = "linux", windows))]
fn write_pack(dir: &Path, profile: &Profile) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|err| err.to_string())?;
    let text = serde_json::to_string_pretty(profile).map_err(|err| err.to_string())?;
    fs::write(dir.join("profile.json"), text).map_err(|err| err.to_string())
}

#[cfg(any(target_os = "linux", windows))]
fn avatar_profile(id: &str, name: &str, rows: &[(&str, &str)]) -> Profile {
    let expressions = rows
        .iter()
        .enumerate()
        .map(|(offset, (label, parameter))| {
            let index = offset as i64 + 1;
            Expression {
                index,
                name: (*label).to_string(),
                parameter: (*parameter).to_string(),
                osc: format!("/avatar/parameters/{parameter}"),
                kind: "bool".into(),
                thumb: String::new(),
            }
        })
        .collect();
    Profile {
        id: id.to_string(),
        name: name.to_string(),
        blueprint_id: id.to_string(),
        expressions,
    }
}

#[cfg(any(target_os = "linux", windows))]
fn write_settings(paths: &AppPaths, osc_port: u16, avatar_port: u16) -> Result<(), String> {
    let mut settings = settings::load_settings(paths);
    if profile::activate_avatar(&mut settings, "avtr_smile", &paths.config_dir).is_none() {
        return Err("could not activate avtr_smile".into());
    }
    let Some(obj) = settings.as_object_mut() else {
        return Err("settings are not an object".into());
    };
    obj.insert("enabled".into(), Value::Bool(true));
    obj.insert(
        "osc".into(),
        json!({
            "host": "127.0.0.1",
            "port": i64::from(osc_port),
        }),
    );
    obj.insert(
        "avatar_osc".into(),
        json!({ "port": i64::from(avatar_port) }),
    );
    obj.insert(
        "fkey_assign".into(),
        json!({"F1": 1, "F2": 2, "F3": 0, "F4": 0}),
    );
    obj.insert(
        "target".into(),
        json!({
            "enabled": true,
            "windows_process": ["VRChat.exe"],
            "linux_match": ["steam_app_438100", "VRChat"],
            "when_unknown": "deny"
        }),
    );
    settings::save_settings(paths, &settings)
}

#[cfg(any(target_os = "linux", windows))]
fn confirm_saved(paths: &AppPaths, avatar: &str, f1: &str, f2: &str) -> Result<(), String> {
    let settings = settings::load_settings(paths);
    let active = settings
        .get("active_avatar")
        .and_then(Value::as_str)
        .unwrap_or("");
    if active != avatar {
        return Err(format!("saved avatar is {active}, want {avatar}"));
    }
    let resolved = resolve_config(&settings, &paths.config_dir);
    let got_f1 = resolved.hotkeys.get("F1").map(String::as_str).unwrap_or("");
    let got_f2 = resolved.hotkeys.get("F2").map(String::as_str).unwrap_or("");
    if got_f1 != f1 || got_f2 != f2 {
        return Err(format!("saved hotkeys F1={got_f1} F2={got_f2}"));
    }
    if !resolved.enabled || !resolved.target.enabled {
        return Err("listener or focus gate is disabled".into());
    }
    Ok(())
}

#[cfg(any(target_os = "linux", windows))]
fn expect_chord(session: &Session, drive: Drive, slot: u8, address: &str) -> Result<(), String> {
    session.vrchat.clear();
    let before = shift_lines(&session.paths);
    drive_button(&drive, slot, true)?;
    wait_exact(&session.vrchat, &[(address, true)]).map_err(|err| explain(session, err))?;
    drive_button(&drive, slot, false)?;
    wait_exact(&session.vrchat, &[(address, true), (address, false)])
        .map_err(|err| explain(session, err))?;
    let want_last = format!("Shift+F{slot} UP");
    wait_last(&session.listener, &want_last)?;
    wait_shift_added(&session.paths, before.len(), slot, address)?;
    Ok(())
}

#[cfg(any(target_os = "linux", windows))]
fn expect_silent(session: &Session, slot: u8) -> Result<(), String> {
    session.vrchat.clear();
    let before = shift_lines(&session.paths);
    t1_button(slot, true)?;
    wait_quiet_osc(&session.vrchat)?;
    t1_button(slot, false)?;
    wait_quiet_osc(&session.vrchat)?;
    if shift_lines(&session.paths) != before {
        return Err(format!(
            "Shift+F{slot} was logged while the gate was closed"
        ));
    }
    Ok(())
}

#[cfg(any(target_os = "linux", windows))]
fn drive_button(drive: &Drive, slot: u8, down: bool) -> Result<(), String> {
    match drive {
        Drive::Local => t1_button(slot, down),
        Drive::Remote(port) => {
            let verb = if down { "press" } else { "release" };
            remote_command(*port, &format!("{verb} {slot}"))
        }
    }
}

#[cfg(any(target_os = "linux", windows))]
fn wait_exact(vr: &FakeVrchat, expected: &[(&str, bool)]) -> Result<(), String> {
    let deadline = Instant::now() + OSC_WAIT;
    loop {
        let got = vr.snapshot();
        let matches = got.len() == expected.len()
            && got
                .iter()
                .zip(expected)
                .all(|(hit, (address, value))| hit.address == *address && hit.value == *value);
        if matches {
            return Ok(());
        }
        if got.len() > expected.len() || Instant::now() >= deadline {
            return Err(format!(
                "osc want {} got {}",
                dump_expected(expected),
                dump_hits(&got)
            ));
        }
        thread::sleep(Duration::from_millis(20));
    }
}

#[cfg(any(target_os = "linux", windows))]
fn wait_quiet_osc(vr: &FakeVrchat) -> Result<(), String> {
    let start = Instant::now();
    while start.elapsed() < SILENT_WAIT {
        let got = vr.snapshot();
        if !got.is_empty() {
            return Err(format!("unexpected osc {}", dump_hits(&got)));
        }
        thread::sleep(Duration::from_millis(20));
    }
    Ok(())
}

#[cfg(any(target_os = "linux", windows))]
fn wait_last(listener: &ExpressionListener, expected: &str) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(1);
    while Instant::now() < deadline {
        if listener.last() == expected {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(10));
    }
    Err(format!(
        "listener last is {:?}, want {expected}",
        listener.last()
    ))
}

#[cfg(any(target_os = "linux", windows))]
fn wait_shift_added(
    paths: &AppPaths,
    before: usize,
    slot: u8,
    address: &str,
) -> Result<(), String> {
    let down = format!("Shift+F{slot} DOWN -> {address}");
    let up = format!("Shift+F{slot} UP -> {address}");
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        let lines = shift_lines(paths);
        if lines.len() >= before + 2 {
            let added = &lines[before..];
            let text = added.join("\n");
            if text.contains(&down) && text.contains(&up) && added.len() == 2 {
                return Ok(());
            }
            if Instant::now() >= deadline || added.len() > 2 {
                return Err(format!("log added {added:?}"));
            }
        }
        if Instant::now() >= deadline {
            return Err(format!("log missing {down} / {up}"));
        }
        thread::sleep(Duration::from_millis(20));
    }
}

#[cfg(any(target_os = "linux", windows))]
fn shift_lines(paths: &AppPaths) -> Vec<String> {
    fs::read_to_string(&paths.log_file)
        .unwrap_or_default()
        .lines()
        .filter(|line| line.contains("Shift+"))
        .map(str::to_string)
        .collect()
}

#[cfg(any(target_os = "linux", windows))]
fn explain(session: &Session, err: String) -> String {
    let log = fs::read_to_string(&session.paths.log_file).unwrap_or_default();
    let settings = fs::read_to_string(&session.paths.config_file).unwrap_or_default();
    format!(
        "{err}\nrunning={} last={:?} polls={} suppress={}\nsettings:\n{settings}\nlog:\n{log}",
        session.listener.is_running(),
        session.listener.last(),
        hook_polls(),
        suppress_real_keys(),
    )
}

#[cfg(any(target_os = "linux", windows))]
fn dump_hits(hits: &[Hit]) -> String {
    if hits.is_empty() {
        return "(none)".into();
    }
    hits.iter()
        .map(|hit| format!("{}={}", hit.address, hit.value))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(any(target_os = "linux", windows))]
fn dump_expected(expected: &[(&str, bool)]) -> String {
    if expected.is_empty() {
        return "(none)".into();
    }
    expected
        .iter()
        .map(|(address, value)| format!("{address}={value}"))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(any(target_os = "linux", windows))]
fn free_udp_port() -> Result<u16, String> {
    let socket = UdpSocket::bind("127.0.0.1:0").map_err(|err| err.to_string())?;
    socket
        .local_addr()
        .map(|addr| addr.port())
        .map_err(|err| err.to_string())
}

#[cfg(any(target_os = "linux", windows))]
fn wait_until(timeout: Duration, mut pred: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if pred() {
            return true;
        }
        thread::sleep(Duration::from_millis(20));
    }
    pred()
}

#[cfg(test)]
mod tests {
    use super::{parse_app_bool, run_walkthrough};
    use crate::osc::OscSender;

    #[test]
    fn aligned_expression_address_still_parses() {
        let down = OscSender::packet("/avatar/parameters/Smile", true);
        let (address, value) = parse_app_bool(&down).expect("down");
        assert_eq!(address, "/avatar/parameters/Smile");
        assert!(value);
        let up = OscSender::packet("/avatar/parameters/Smile", false);
        let (address, value) = parse_app_bool(&up).expect("up");
        assert_eq!(address, "/avatar/parameters/Smile");
        assert!(!value);
        let ordinary = OscSender::packet("/avatar/parameters/Expression1", true);
        let (address, value) = parse_app_bool(&ordinary).expect("ordinary");
        assert_eq!(address, "/avatar/parameters/Expression1");
        assert!(value);
    }

    #[test]
    fn walkthrough_configure_confirm_use_and_switch_avatar() {
        let report = run_walkthrough().unwrap_or_else(|err| panic!("{err}"));
        println!("{report}");
        for needle in [
            "confirm:",
            "/avatar/parameters/Smile",
            "/avatar/parameters/Wave",
            "/avatar/parameters/Bye",
            "unfocused: suppressed",
            "ignored-f5: suppressed",
            "real-focus:",
            "external:",
            "walkthrough: ok",
        ] {
            assert!(report.contains(needle), "missing {needle}\n{report}");
        }
        let chords = report.matches("press=true release=false").count();
        assert!(chords >= 3, "{report}");
    }
}
