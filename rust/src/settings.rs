//! `settings.json` is a JSON object. Unknown keys are kept.
//!
//! A missing file loads as `{}`. Defaults are applied when a value is read,
//! and they are written only after the window changes that value.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

use serde_json::{Map, Value};

use crate::focus::{as_bool, resolve_target, Target};
use crate::osc::{osc_from_settings, OscSender};
use crate::paths::AppPaths;
use crate::profile::{active_profile, expression_slot};
use crate::{AVATAR_LISTEN_PORT, DEFAULT_AVATAR_ID, FKEY_NAMES, OSC_PORT};

static SETTINGS_IO: Mutex<()> = Mutex::new(());

#[derive(Clone, Debug)]
pub struct Resolved {
    pub enabled: bool,
    pub target: Target,
    pub hotkeys: std::collections::BTreeMap<String, String>,
}

pub fn default_settings() -> Value {
    let mut root = Map::new();
    let mut osc = Map::new();
    osc.insert("host".into(), Value::String("127.0.0.1".into()));
    osc.insert("port".into(), Value::from(OSC_PORT));
    root.insert("osc".into(), Value::Object(osc));
    let mut avatar = Map::new();
    avatar.insert("port".into(), Value::from(AVATAR_LISTEN_PORT));
    root.insert("avatar_osc".into(), Value::Object(avatar));
    root.insert("enabled".into(), Value::Bool(true));
    root.insert("target".into(), target_value(&Target::default()));
    root.insert(
        "active_avatar".into(),
        Value::String(DEFAULT_AVATAR_ID.into()),
    );
    let mut assign = Map::new();
    for (number, name) in FKEY_NAMES.iter().enumerate() {
        assign.insert((*name).to_string(), Value::from(number as i64 + 1));
    }
    root.insert("fkey_assign".into(), Value::Object(assign));
    Value::Object(root)
}

fn target_value(target: &Target) -> Value {
    let mut obj = Map::new();
    obj.insert("enabled".into(), Value::Bool(target.enabled));
    obj.insert(
        "windows_process".into(),
        Value::Array(
            target
                .windows_process
                .iter()
                .cloned()
                .map(Value::String)
                .collect(),
        ),
    );
    obj.insert(
        "linux_match".into(),
        Value::Array(
            target
                .linux_match
                .iter()
                .cloned()
                .map(Value::String)
                .collect(),
        ),
    );
    obj.insert(
        "when_unknown".into(),
        Value::String(target.when_unknown.clone()),
    );
    Value::Object(obj)
}

pub fn load_settings(paths: &AppPaths) -> Value {
    let _guard = lock();
    load_unlocked(paths)
}

pub fn save_settings(paths: &AppPaths, settings: &Value) -> Result<(), String> {
    let _guard = lock();
    save_unlocked(paths, settings)
}

pub(crate) fn lock() -> std::sync::MutexGuard<'static, ()> {
    SETTINGS_IO.lock().unwrap_or_else(|err| err.into_inner())
}

pub(crate) fn load_unlocked(paths: &AppPaths) -> Value {
    match fs::read_to_string(&paths.config_file) {
        Ok(text) => match serde_json::from_str::<Value>(&text) {
            Ok(Value::Object(map)) => Value::Object(map),
            Ok(_) | Err(_) => default_settings(),
        },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Value::Object(Map::new()),
        Err(_) => default_settings(),
    }
}

pub(crate) fn save_unlocked(paths: &AppPaths, settings: &Value) -> Result<(), String> {
    if let Some(parent) = paths.config_file.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let text = to_pretty(settings)?;
    let tmp = tmp_path(&paths.config_file);
    {
        let mut file = fs::File::create(&tmp).map_err(|err| err.to_string())?;
        file.write_all(text.as_bytes())
            .map_err(|err| err.to_string())?;
    }
    fs::rename(&tmp, &paths.config_file).map_err(|err| err.to_string())?;
    Ok(())
}

pub fn to_pretty(value: &impl serde::Serialize) -> Result<String, String> {
    let mut text = serde_json::to_string_pretty(value).map_err(|err| err.to_string())?;
    text.push('\n');
    Ok(text)
}

pub fn settings_mtime(paths: &AppPaths) -> Option<std::time::SystemTime> {
    fs::metadata(&paths.config_file)
        .and_then(|meta| meta.modified())
        .ok()
}

pub fn normalize_assign(raw: Option<&Value>) -> std::collections::BTreeMap<String, i64> {
    let obj = raw.and_then(Value::as_object);
    let mut out = std::collections::BTreeMap::new();
    for (offset, name) in FKEY_NAMES.iter().enumerate() {
        let number = offset as i64 + 1;
        let value = obj.and_then(|map| map.get(*name));
        let index = match value {
            None => number,
            Some(item) => as_index(item).unwrap_or(0),
        };
        out.insert(
            (*name).to_string(),
            if (1..=crate::SLOT_COUNT).contains(&index) {
                index
            } else {
                0
            },
        );
    }
    out
}

pub fn hotkeys_for(settings: &Value, root: &Path) -> std::collections::BTreeMap<String, String> {
    let profile = active_profile(settings, root);
    let assign = normalize_assign(settings.get("fkey_assign"));
    let mut hotkeys = std::collections::BTreeMap::new();
    for (name, index) in assign {
        if index == 0 {
            continue;
        }
        if let Some(slot) = expression_slot(index, &profile, root) {
            if !slot.osc.is_empty() {
                hotkeys.insert(name, slot.osc);
            }
        }
    }
    hotkeys
}

pub fn resolve_config(settings: &Value, root: &Path) -> Resolved {
    Resolved {
        enabled: as_bool(settings.get("enabled"), true),
        target: resolve_target(settings.get("target").unwrap_or(&Value::Null)),
        hotkeys: hotkeys_for(settings, root),
    }
}

pub fn avatar_listen_port(settings: &Value) -> u16 {
    let block = settings.get("avatar_osc").and_then(Value::as_object);
    let Some(raw) = block.and_then(|obj| obj.get("port")) else {
        return AVATAR_LISTEN_PORT;
    };
    match as_index(raw) {
        Some(port) if (1..=65535).contains(&port) => port as u16,
        _ => AVATAR_LISTEN_PORT,
    }
}

pub fn sender_from(settings: &Value) -> OscSender {
    osc_from_settings(settings)
}

pub fn lang_of(settings: &Value) -> String {
    settings
        .get("lang")
        .and_then(Value::as_str)
        .unwrap_or("en")
        .to_string()
}

pub fn zoom_of(settings: &Value) -> f32 {
    let Some(number) = settings.get("ui_zoom").and_then(Value::as_f64) else {
        return 1.0;
    };
    let zoom = number as f32;
    if (0.75..=3.0).contains(&zoom) {
        zoom
    } else {
        1.0
    }
}

fn as_index(value: &Value) -> Option<i64> {
    if let Some(number) = value.as_i64() {
        return Some(number);
    }
    if let Some(number) = value.as_u64() {
        return i64::try_from(number).ok();
    }
    if let Some(number) = value.as_f64() {
        if number.fract() == 0.0 {
            return Some(number as i64);
        }
    }
    if let Some(text) = value.as_str() {
        return text.trim().parse().ok();
    }
    if let Some(flag) = value.as_bool() {
        return Some(i64::from(flag));
    }
    None
}

fn tmp_path(path: &Path) -> std::path::PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".tmp");
    std::path::PathBuf::from(name)
}
