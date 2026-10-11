//! Avatar profiles: `avatars/<id>/profile.json` and `thumbs/N.png`.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::Value;
use zip::ZipArchive;

use crate::i18n::import_notice;
use crate::paths::{log_line, AppPaths};
use crate::settings::{self, load_unlocked, save_unlocked};
use crate::{DEFAULT_AVATAR_ID, SLOT_COUNT};

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Expression {
    pub index: i64,
    pub name: String,
    pub parameter: String,
    pub osc: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub thumb: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub blueprint_id: String,
    pub expressions: Vec<Expression>,
}

#[derive(Clone, Debug)]
pub struct Slot {
    pub index: i64,
    pub label: String,
    pub thumb: String,
    pub osc: String,
}

/// What the fullscreen expression card draws: the display name, and the
/// thumbnail when that PNG is on disk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StageCard {
    pub index: i64,
    pub name: String,
    pub thumb: String,
}

#[derive(Clone, Debug)]
pub struct ImportResult {
    pub profile: Profile,
    pub skipped: Vec<i64>,
    pub notice: String,
}

pub fn safe_avatar_id(value: &str) -> String {
    let mut cleaned: String = value
        .trim()
        .chars()
        .filter(|ch| ch.is_alphanumeric() || matches!(ch, '.' | '_' | '-'))
        .collect();
    if cleaned.chars().count() > 128 {
        cleaned = cleaned.chars().take(128).collect();
    }
    if cleaned.is_empty() || cleaned == "." || cleaned == ".." {
        String::new()
    } else {
        cleaned
    }
}

pub fn profile_root(root: &Path) -> PathBuf {
    root.join("avatars")
}

pub fn default_profile() -> Profile {
    let expressions = (1..=SLOT_COUNT)
        .map(|number| {
            let parameter = format!("Expression{number}");
            Expression {
                index: number,
                name: parameter.clone(),
                parameter: parameter.clone(),
                osc: format!("/avatar/parameters/{parameter}"),
                kind: "bool".into(),
                thumb: String::new(),
            }
        })
        .collect();
    Profile {
        id: DEFAULT_AVATAR_ID.into(),
        name: "Default".into(),
        blueprint_id: String::new(),
        expressions,
    }
}

pub fn normalize_profile(raw: &Value) -> Option<Profile> {
    let obj = raw.as_object()?;
    let id_source = obj
        .get("id")
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .or_else(|| obj.get("blueprint_id").and_then(Value::as_str))
        .unwrap_or("");
    let avatar_id = safe_avatar_id(id_source);
    if avatar_id.is_empty() {
        return None;
    }
    let mut expressions = Vec::new();
    for item in obj
        .get("expressions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(expr) = item.as_object() else {
            continue;
        };
        let Some(index) = expr.get("index").and_then(json_i64) else {
            continue;
        };
        if !(1..=SLOT_COUNT).contains(&index) {
            continue;
        }
        let name = non_empty(expr.get("name"), &format!("Expression{index}"));
        let parameter = non_empty(expr.get("parameter"), &name);
        let osc = match expr.get("osc").and_then(Value::as_str) {
            Some(text) if !text.trim().is_empty() => text.trim().to_string(),
            _ => format!("/avatar/parameters/{parameter}"),
        };
        let thumb = expr
            .get("thumb")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        expressions.push(Expression {
            index,
            name,
            parameter,
            osc,
            kind: "bool".into(),
            thumb,
        });
    }
    expressions.sort_by_key(|item| item.index);
    let name = non_empty(obj.get("name"), &avatar_id);
    Some(Profile {
        id: avatar_id,
        name,
        blueprint_id: obj
            .get("blueprint_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string(),
        expressions,
    })
}

pub fn load_profile(avatar_id: &str, root: &Path) -> Option<Profile> {
    let safe = safe_avatar_id(avatar_id);
    if safe.is_empty() {
        return if avatar_id.is_empty() || avatar_id == DEFAULT_AVATAR_ID {
            Some(default_profile())
        } else {
            None
        };
    }
    let path = profile_root(root).join(&safe).join("profile.json");
    let data = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(_) => {
            return if safe == DEFAULT_AVATAR_ID {
                Some(default_profile())
            } else {
                None
            };
        }
    };
    let parsed = serde_json::from_str::<Value>(&data).ok();
    match parsed.as_ref().and_then(normalize_profile) {
        Some(profile) => Some(profile),
        None if safe == DEFAULT_AVATAR_ID => Some(default_profile()),
        None => None,
    }
}

pub fn list_profiles(root: &Path) -> Vec<Profile> {
    let mut found = std::collections::BTreeMap::new();
    let base = profile_root(root);
    if let Ok(entries) = fs::read_dir(&base) {
        let mut names: Vec<String> = entries
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        for name in names {
            if let Some(profile) = load_profile(&name, root) {
                found.insert(profile.id.clone(), profile);
            }
        }
    }
    found
        .entry(DEFAULT_AVATAR_ID.to_string())
        .or_insert_with(default_profile);
    let default_profile = found
        .remove(DEFAULT_AVATAR_ID)
        .unwrap_or_else(default_profile);
    let mut ordered = vec![default_profile];
    ordered.extend(found.into_values());
    ordered
}

pub fn profile_thumb_path(profile: &Profile, expr: &Expression, root: &Path) -> String {
    let thumb = expr.thumb.as_str();
    if thumb.is_empty() {
        return String::new();
    }
    let path = Path::new(thumb);
    if path.is_absolute() {
        return path.to_string_lossy().into_owned();
    }
    let normalized = thumb.replace('\\', "/");
    let parts: Vec<&str> = normalized
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect();
    if parts.is_empty() || parts.contains(&"..") {
        return String::new();
    }
    let mut out = profile_root(root).join(&profile.id);
    for part in parts {
        out.push(part);
    }
    out.to_string_lossy().into_owned()
}

pub fn save_profile(profile: &Profile, root: &Path) -> Result<Profile, String> {
    let normalized =
        normalize_profile(&serde_json::to_value(profile).map_err(|err| err.to_string())?)
            .ok_or_else(|| "invalid profile".to_string())?;
    let dest = profile_root(root).join(&normalized.id);
    fs::create_dir_all(&dest).map_err(|err| err.to_string())?;
    let path = dest.join("profile.json");
    let text = settings::to_pretty(&normalized)?;
    let tmp = {
        let mut name = path.as_os_str().to_os_string();
        name.push(".tmp");
        PathBuf::from(name)
    };
    {
        let mut file = fs::File::create(&tmp).map_err(|err| err.to_string())?;
        file.write_all(text.as_bytes())
            .map_err(|err| err.to_string())?;
    }
    fs::rename(&tmp, &path).map_err(|err| err.to_string())?;
    Ok(normalized)
}

pub fn stage_card(index: i64, profile: &Profile, root: &Path) -> Option<StageCard> {
    if !(1..=SLOT_COUNT).contains(&index) {
        return None;
    }
    let Some(expr) = profile.expressions.iter().find(|item| item.index == index) else {
        return Some(StageCard {
            index,
            name: format!("Expression{index}"),
            thumb: String::new(),
        });
    };
    let path = profile_thumb_path(profile, expr, root);
    let thumb = if !path.is_empty() && Path::new(&path).is_file() {
        path
    } else {
        String::new()
    };
    let trimmed = expr.name.trim();
    let name = if trimmed.is_empty() {
        format!("Expression{index}")
    } else {
        trimmed.to_string()
    };
    Some(StageCard { index, name, thumb })
}

pub fn expression_slot(index: i64, profile: &Profile, root: &Path) -> Option<Slot> {
    if !(1..=SLOT_COUNT).contains(&index) {
        return None;
    }
    let label = format!("Expression{index}");
    let expr = profile.expressions.iter().find(|item| item.index == index);
    let Some(expr) = expr else {
        return Some(Slot {
            index,
            label,
            thumb: String::new(),
            osc: String::new(),
        });
    };
    let path = profile_thumb_path(profile, expr, root);
    let thumb = if !path.is_empty() && Path::new(&path).is_file() {
        path
    } else {
        String::new()
    };
    let label = if thumb.is_empty() {
        label
    } else {
        let name = expr.name.trim();
        if name.is_empty() {
            format!("Expression{index}")
        } else {
            name.to_string()
        }
    };
    Some(Slot {
        index,
        label,
        thumb,
        osc: expr.osc.clone(),
    })
}

pub fn active_profile(settings: &Value, root: &Path) -> Profile {
    let avatar_id = settings
        .get("active_avatar")
        .and_then(Value::as_str)
        .unwrap_or("");
    if !avatar_id.is_empty() {
        if let Some(profile) = load_profile(avatar_id, root) {
            let file = profile_root(root).join(&profile.id).join("profile.json");
            if avatar_id == DEFAULT_AVATAR_ID || file.is_file() {
                return profile;
            }
        }
    }
    default_profile()
}

pub fn activate_avatar(settings: &mut Value, avatar_id: &str, root: &Path) -> Option<Profile> {
    let profile = load_profile(avatar_id, root)?;
    if let Some(obj) = settings.as_object_mut() {
        obj.insert("active_avatar".into(), Value::String(profile.id.clone()));
    }
    Some(profile)
}

pub fn skipped_expression_indexes(raw: &Value) -> Vec<i64> {
    let mut skipped = Vec::new();
    let Some(items) = raw.get("expressions").and_then(Value::as_array) else {
        return skipped;
    };
    for item in items {
        let Some(obj) = item.as_object() else {
            continue;
        };
        let Some(index) = obj.get("index").and_then(json_i64) else {
            continue;
        };
        if index > SLOT_COUNT {
            skipped.push(index);
        }
    }
    skipped
}

pub fn import_profile_pack(path: &Path, paths: &AppPaths) -> Result<ImportResult, String> {
    let path = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if path.is_dir() {
        return import_profile_dir(&path, paths);
    }
    if path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
    {
        let parent = path.parent().unwrap_or(Path::new("."));
        return import_profile_dir(parent, paths);
    }
    if !is_zip(&path) {
        return Err("profile pack must be a .zip, a folder, or profile.json".into());
    }
    let tmp = tempfile::Builder::new()
        .prefix("veh-profile-")
        .tempdir()
        .map_err(|err| err.to_string())?;
    extract_zip(&path, tmp.path())?;
    let found = find_profile_dir(tmp.path()).ok_or_else(|| "profile.json not found".to_string())?;
    import_profile_dir(&found, paths)
}

pub fn find_profile_id(token: &str, root: &Path) -> Option<String> {
    let text = token.trim();
    if text.is_empty() {
        return None;
    }
    let safe = safe_avatar_id(text);
    if !safe.is_empty() {
        let file = profile_root(root).join(&safe).join("profile.json");
        if file.is_file() {
            return Some(safe);
        }
    }
    if safe == DEFAULT_AVATAR_ID {
        return Some(DEFAULT_AVATAR_ID.to_string());
    }
    list_profiles(root).into_iter().find_map(|profile| {
        if text == profile.id || text == profile.blueprint_id || text == profile.name {
            Some(profile.id)
        } else {
            None
        }
    })
}

pub fn note_avatar_change(paths: &AppPaths, avatar_id: &str) -> Option<Profile> {
    let _guard = settings::lock();
    let mut settings = load_unlocked(paths);
    let Some(found) = find_profile_id(avatar_id, &paths.config_dir) else {
        log_line(paths, &format!("avatar {avatar_id} has no profile"));
        return None;
    };
    let profile = activate_avatar(&mut settings, &found, &paths.config_dir)?;
    save_unlocked(paths, &settings).ok()?;
    log_line(paths, &format!("avatar -> {found}"));
    Some(profile)
}

fn import_profile_dir(src: &Path, paths: &AppPaths) -> Result<ImportResult, String> {
    let root = &paths.config_dir;
    let raw_text = fs::read_to_string(src.join("profile.json")).map_err(|err| err.to_string())?;
    let raw: Value = serde_json::from_str(&raw_text).map_err(|err| err.to_string())?;
    let skipped = skipped_expression_indexes(&raw);
    let mut profile = normalize_profile(&raw).ok_or_else(|| "invalid profile.json".to_string())?;
    copy_thumbs(src, &mut profile, root)?;
    let saved = save_profile(&profile, root)?;
    let notice = import_notice(&skipped, "en");
    if !skipped.is_empty() {
        log_line(paths, &notice);
    }
    Ok(ImportResult {
        profile: saved,
        skipped,
        notice,
    })
}

fn copy_thumbs(src: &Path, profile: &mut Profile, root: &Path) -> Result<(), String> {
    let dest = profile_root(root).join(&profile.id);
    for expr in &mut profile.expressions {
        let thumb = expr.thumb.replace('\\', "/");
        let parts: Vec<&str> = thumb
            .split('/')
            .filter(|part| !part.is_empty() && *part != ".")
            .collect();
        if thumb.is_empty() || Path::new(&expr.thumb).is_absolute() || parts.contains(&"..") {
            expr.thumb.clear();
            continue;
        }
        let mut source = src.to_path_buf();
        for part in &parts {
            source.push(part);
        }
        if !source.is_file() {
            expr.thumb.clear();
            continue;
        }
        let target = dest.join("thumbs").join(format!("{}.png", expr.index));
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        fs::copy(&source, &target).map_err(|err| err.to_string())?;
        expr.thumb = format!("thumbs/{}.png", expr.index);
    }
    Ok(())
}

fn is_zip(path: &Path) -> bool {
    let Ok(file) = fs::File::open(path) else {
        return false;
    };
    ZipArchive::new(file).is_ok()
}

fn extract_zip(path: &Path, dest: &Path) -> Result<(), String> {
    let file = fs::File::open(path).map_err(|err| err.to_string())?;
    let mut archive = ZipArchive::new(file).map_err(|err| err.to_string())?;
    for index in 0..archive.len() {
        let name = archive
            .name_for_index(index)
            .unwrap_or("")
            .replace('\\', "/");
        safe_zip_name(&name)?;
    }
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|err| err.to_string())?;
        let name = entry.name().replace('\\', "/");
        if name.is_empty() {
            continue;
        }
        let relative = safe_zip_name(&name)?;
        let out = dest.join(&relative);
        if name.ends_with('/') || entry.is_dir() {
            fs::create_dir_all(&out).map_err(|err| err.to_string())?;
            continue;
        }
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        let mut file = fs::File::create(&out).map_err(|err| err.to_string())?;
        std::io::copy(&mut entry, &mut file).map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn safe_zip_name(name: &str) -> Result<PathBuf, String> {
    if name.starts_with('/') {
        return Err("unsafe path in profile pack".into());
    }
    let parts: Vec<&str> = name
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect();
    if parts.contains(&"..") {
        return Err("unsafe path in profile pack".into());
    }
    let mut path = PathBuf::new();
    for part in parts {
        path.push(part);
    }
    Ok(path)
}

fn find_profile_dir(root: &Path) -> Option<PathBuf> {
    let mut queue = vec![root.to_path_buf()];
    let mut index = 0;
    while index < queue.len() {
        let dir = queue[index].clone();
        index += 1;
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        let mut children = Vec::new();
        let mut found = false;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file()
                && path.file_name().and_then(|name| name.to_str()) == Some("profile.json")
            {
                found = true;
            } else if path.is_dir() {
                children.push(path);
            }
        }
        if found {
            return Some(dir);
        }
        children.sort();
        queue.extend(children);
    }
    None
}

fn non_empty(value: Option<&Value>, fallback: &str) -> String {
    let text = value.and_then(Value::as_str).unwrap_or("").trim();
    if text.is_empty() {
        fallback.to_string()
    } else {
        text.to_string()
    }
}

/// 1×1 PNG used by the walkthrough and the stage-card test.
#[cfg(any(test, feature = "emulate"))]
pub(crate) const STAGE_PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
    0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x08, 0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00,
    0x00, 0x00, 0x03, 0x00, 0x01, 0x00, 0x05, 0xFE, 0x02, 0xFE, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
];

fn json_i64(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|number| i64::try_from(number).ok()))
        .or_else(|| value.as_str().and_then(|text| text.trim().parse().ok()))
}

#[cfg(test)]
mod tests {
    use super::{profile_root, stage_card, Expression, Profile, STAGE_PNG};
    use std::fs;
    use std::path::Path;

    #[test]
    fn stage_card_shows_the_japanese_name_and_thumbnail() {
        let dir = tempfile::tempdir().expect("temp");
        let profile = Profile {
            id: "avtr_smile".into(),
            name: "笑顔アバター".into(),
            blueprint_id: "avtr_smile".into(),
            expressions: vec![Expression {
                index: 1,
                name: "笑顔".into(),
                parameter: "GrinA".into(),
                osc: "/avatar/parameters/GrinA".into(),
                kind: "bool".into(),
                thumb: "thumbs/1.png".into(),
            }],
        };
        let thumbs = profile_root(dir.path()).join(&profile.id).join("thumbs");
        fs::create_dir_all(&thumbs).expect("thumbs");
        fs::write(thumbs.join("1.png"), STAGE_PNG).expect("png");
        let card = stage_card(1, &profile, dir.path()).expect("card");
        assert_eq!(card.name, "笑顔");
        assert!(Path::new(&card.thumb).is_file());
        assert!(card.thumb.ends_with("1.png"));
        let missing = stage_card(2, &profile, dir.path()).expect("empty slot");
        assert_eq!(missing.name, "Expression2");
        assert!(missing.thumb.is_empty());
    }
}
