//! Foreground check. Keys are never swallowed; a rejected press is simply
//! not sent until that physical key is released.

use std::path::Path;
#[cfg(target_os = "linux")]
use std::process::{Command, Stdio};
#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::chord::ChordTracker;
use crate::paths::log_line;
use crate::paths::AppPaths;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub enabled: bool,
    pub windows_process: Vec<String>,
    pub linux_match: Vec<String>,
    pub when_unknown: String,
}

impl Default for Target {
    fn default() -> Self {
        Self {
            enabled: true,
            windows_process: vec!["VRChat.exe".to_string()],
            linux_match: vec!["steam_app_438100".to_string(), "VRChat".to_string()],
            when_unknown: "deny".to_string(),
        }
    }
}

pub fn resolve_target(raw: &Value) -> Target {
    let obj = raw.as_object();
    let mut target = Target {
        enabled: as_bool(obj.and_then(|item| item.get("enabled")), true),
        ..Target::default()
    };
    if let Some(obj) = obj {
        target.windows_process = string_list(obj.get("windows_process"), &target.windows_process);
        target.linux_match = string_list(obj.get("linux_match"), &target.linux_match);
        let unknown = obj
            .get("when_unknown")
            .and_then(Value::as_str)
            .unwrap_or("deny")
            .trim()
            .to_ascii_lowercase();
        target.when_unknown = if unknown == "allow" || unknown == "deny" {
            unknown
        } else {
            "deny".to_string()
        };
    }
    target
}

pub struct FocusGate<F> {
    pub target: Target,
    detect: F,
    pub last_state: Option<bool>,
    pub warning_count: u32,
    pub last_warning: Option<String>,
    paths: Option<AppPaths>,
}

impl<F> FocusGate<F>
where
    F: FnMut() -> Option<bool>,
{
    pub fn new(target: Target, detect: F) -> Self {
        Self {
            target,
            detect,
            last_state: None,
            warning_count: 0,
            last_warning: None,
            paths: None,
        }
    }

    pub fn with_paths(mut self, paths: AppPaths) -> Self {
        self.paths = Some(paths);
        self
    }

    pub fn allows_press(&mut self) -> bool {
        if !self.target.enabled {
            self.last_state = Some(true);
            return true;
        }
        let state = (self.detect)();
        self.last_state = state;
        match state {
            None => {
                self.warn();
                self.target.when_unknown == "allow"
            }
            Some(focused) => focused,
        }
    }

    fn warn(&mut self) {
        if self.warning_count > 0 {
            return;
        }
        self.warning_count = 1;
        let action = if self.target.when_unknown == "allow" {
            "Presses are sent anyway."
        } else {
            "Presses are ignored until the focused app can be read."
        };
        let message = format!("WARNING: cannot tell which window is focused. {action}");
        if let Some(paths) = &self.paths {
            log_line(paths, &message);
        }
        eprintln!("{message}");
        self.last_warning = Some(message);
    }
}

pub fn observe_evdev<F>(
    tracker: &mut ChordTracker,
    gate: &mut FocusGate<F>,
    device: &str,
    code: u16,
    value: i32,
) -> Vec<(String, bool)>
where
    F: FnMut() -> Option<bool>,
{
    let would = tracker.would_press(device, code, value, true);
    let accept = if would { gate.allows_press() } else { true };
    tracker.note_evdev(device, code, value, accept)
}

pub fn observe_vk<F>(
    tracker: &mut ChordTracker,
    gate: &mut FocusGate<F>,
    device: &str,
    vk: u16,
    is_up: bool,
) -> Vec<(String, bool)>
where
    F: FnMut() -> Option<bool>,
{
    let value = if is_up { 0 } else { 1 };
    let would = tracker.would_press(device, vk, value, false);
    let accept = if would { gate.allows_press() } else { true };
    tracker.note_vk(device, vk, is_up, accept)
}

pub fn process_matches(image: &str, names: &[String]) -> bool {
    if image.is_empty() || names.is_empty() {
        return false;
    }
    let base = basename(image).to_lowercase();
    names
        .iter()
        .any(|name| basename(name).to_lowercase() == base)
}

pub fn window_matches(fields: &[String], patterns: &[String]) -> bool {
    if fields.is_empty() || patterns.is_empty() {
        return false;
    }
    let haystack = fields
        .iter()
        .filter(|field| !field.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join("\n")
        .to_lowercase();
    patterns
        .iter()
        .any(|pattern| !pattern.is_empty() && haystack.contains(&pattern.to_lowercase()))
}

pub fn parse_hypr_active(raw: &str) -> Option<Vec<String>> {
    let data: Value = serde_json::from_str(raw).ok()?;
    let obj = data.as_object()?;
    let mut fields = Vec::new();
    for key in ["class", "initialClass", "title", "initialTitle"] {
        if let Some(value) = obj.get(key).and_then(Value::as_str) {
            if !value.is_empty() {
                fields.push(value.to_string());
            }
        }
    }
    Some(fields)
}

pub fn parse_xprop_window(raw: &str) -> Option<Vec<String>> {
    let mut fields = Vec::new();
    for line in raw.lines() {
        let Some((_, rest)) = line.split_once('=') else {
            continue;
        };
        for part in rest.split(',') {
            let text = part.trim().trim_matches('"');
            if !text.is_empty() && text != "0x0" {
                fields.push(text.to_string());
            }
        }
    }
    if fields.is_empty() {
        None
    } else {
        Some(fields)
    }
}

pub fn linux_active_fields(
    mut run: impl FnMut(&[String]) -> Option<String>,
    mut which: impl FnMut(&str) -> Option<String>,
) -> Option<Vec<String>> {
    if which("hyprctl").is_some() {
        if let Some(raw) = run(&args(&["hyprctl", "activewindow", "-j"])) {
            if let Some(fields) = parse_hypr_active(&raw) {
                return Some(fields);
            }
        }
    }
    if which("xdotool").is_some() {
        if let Some(raw) = run(&args(&[
            "xdotool",
            "getactivewindow",
            "getwindowname",
            "getwindowclassname",
        ])) {
            let lines: Vec<String> = raw
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_string)
                .collect();
            if !lines.is_empty() {
                return Some(lines);
            }
        }
    }
    if which("xprop").is_some() {
        if let Some(root) = run(&args(&["xprop", "-root", "_NET_ACTIVE_WINDOW"])) {
            let window_id = root
                .replace(',', " ")
                .split_whitespace()
                .find(|token| token.starts_with("0x") && *token != "0x0")
                .map(str::to_string);
            if let Some(window_id) = window_id {
                if let Some(detail) = run(&args(&[
                    "xprop",
                    "-id",
                    &window_id,
                    "WM_CLASS",
                    "_NET_WM_NAME",
                ])) {
                    return parse_xprop_window(&detail);
                }
            }
        }
    }
    None
}

pub fn probe_focus(target: &Target) -> Option<bool> {
    #[cfg(any(test, feature = "emulate"))]
    {
        if let Some(answer) = crate::emulate::forced_focus_answer() {
            return answer;
        }
        crate::emulate::note_real_focus_probe();
    }
    #[cfg(windows)]
    {
        crate::keys::windows_focused(target)
    }
    #[cfg(target_os = "linux")]
    {
        linux_focused(target)
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = target;
        None
    }
}

#[cfg(target_os = "linux")]
pub fn linux_focused(target: &Target) -> Option<bool> {
    let fields = linux_active_fields(capture_command, which_in_path)?;
    Some(window_matches(&fields, &target.linux_match))
}

#[cfg(target_os = "linux")]
fn capture_command(argv: &[String]) -> Option<String> {
    if argv.is_empty() {
        return None;
    }
    let mut child = Command::new(&argv[0])
        .args(&argv[1..])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let started = Instant::now();
    let timeout = Duration::from_millis(400);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return None;
                }
                let mut out = String::new();
                use std::io::Read;
                child.stdout.as_mut()?.read_to_string(&mut out).ok()?;
                return Some(out);
            }
            Ok(None) => {
                if started.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(_) => return None,
        }
    }
}

#[cfg(target_os = "linux")]
fn which_in_path(name: &str) -> Option<String> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().into_owned());
        }
    }
    None
}

fn args(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| (*part).to_string()).collect()
}

fn basename(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    Path::new(&normalized)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path)
        .to_string()
}

pub fn as_bool(value: Option<&Value>, default: bool) -> bool {
    match value {
        None | Some(Value::Null) => default,
        Some(Value::Bool(flag)) => *flag,
        Some(Value::String(text)) => !matches!(
            text.trim().to_ascii_lowercase().as_str(),
            "0" | "false" | "no" | "off"
        ),
        Some(Value::Number(number)) => number.as_i64().unwrap_or(1) != 0,
        Some(_) => true,
    }
}

fn string_list(value: Option<&Value>, default: &[String]) -> Vec<String> {
    match value {
        None | Some(Value::Null) => default.to_vec(),
        Some(Value::String(text)) => {
            let text = text.trim();
            if text.is_empty() {
                default.to_vec()
            } else {
                vec![text.to_string()]
            }
        }
        Some(Value::Array(items)) => {
            let list: Vec<String> = items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(str::to_string)
                .collect();
            if list.is_empty() {
                Vec::new()
            } else {
                list
            }
        }
        Some(_) => default.to_vec(),
    }
}
