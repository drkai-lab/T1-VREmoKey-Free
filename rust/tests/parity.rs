//! Behavior carried over from the Python free edition.

use std::fs;
use std::io::Write;
use std::net::UdpSocket;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};
use vrc_expression_hotkeys::chord::{
    evdev_fkey, is_ignored_evdev, is_ignored_vk, looks_like_keyboard, vk_fkey, ChordTracker,
    KERNEL_EV_KEY, KEY_F1,
};
use vrc_expression_hotkeys::focus::{
    linux_active_fields, observe_evdev, observe_vk, parse_hypr_active, process_matches,
    resolve_target, window_matches, FocusGate,
};
use vrc_expression_hotkeys::i18n::import_notice;
use vrc_expression_hotkeys::keys::{KeyEvent, KeySource};
use vrc_expression_hotkeys::listener::{AvatarOscListener, ExpressionListener};
use vrc_expression_hotkeys::osc::{encode_osc_string, parse_osc_packet, OscSender, OscTyped};
use vrc_expression_hotkeys::paths::AppPaths;
use vrc_expression_hotkeys::profile::{
    default_profile, expression_slot, import_profile_pack, load_profile, note_avatar_change,
    profile_thumb_path, save_profile, Profile,
};
use vrc_expression_hotkeys::settings::{
    default_settings, hotkeys_for, load_settings, save_settings, to_pretty,
};
use vrc_expression_hotkeys::{FKEY_NAMES, SLOT_COUNT};

const GOLDEN_SETTINGS: &str = r#"{
  "osc": {
    "host": "127.0.0.1",
    "port": 9000
  },
  "avatar_osc": {
    "port": 9001
  },
  "enabled": true,
  "target": {
    "enabled": true,
    "windows_process": [
      "VRChat.exe"
    ],
    "linux_match": [
      "steam_app_438100",
      "VRChat"
    ],
    "when_unknown": "deny"
  },
  "active_avatar": "default",
  "fkey_assign": {
    "F1": 1,
    "F2": 2,
    "F3": 3,
    "F4": 4
  }
}
"#;

fn scratch() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("veh-parity-{nanos}-{}", std::process::id()));
    fs::create_dir_all(&path).unwrap();
    path
}

fn png(path: &Path) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(
        path,
        [
            0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00,
            0x00, 0x90, 0x77, 0x53, 0xde, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41, 0x54, 0x78,
            0x9c, 0x63, 0xf8, 0xcf, 0xc0, 0x00, 0x00, 0x00, 0x03, 0x00, 0x01, 0x00, 0x05, 0xfe,
            0xd4, 0xef, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
        ],
    )
    .unwrap();
}

#[test]
fn chord_codes_and_f5_through_f12_are_ignored() {
    assert_eq!(SLOT_COUNT, 4);
    assert_eq!(FKEY_NAMES, ["F1", "F2", "F3", "F4"]);
    assert_eq!(evdev_fkey(59), Some("F1"));
    assert_eq!(evdev_fkey(62), Some("F4"));
    assert_eq!(evdev_fkey(63), None);
    assert_eq!(evdev_fkey(88), None);
    assert!(is_ignored_evdev(63));
    assert!(is_ignored_evdev(88));
    assert_eq!(vk_fkey(0x70), Some("F1"));
    assert_eq!(vk_fkey(0x73), Some("F4"));
    assert_eq!(vk_fkey(0x74), None);
    assert!(is_ignored_vk(0x7B));

    let mut tracker = ChordTracker::new();
    tracker.note_evdev("kbd", 42, 1, true);
    assert!(tracker.note_evdev("kbd", 63, 1, true).is_empty());
    assert!(tracker.note_evdev("kbd", 88, 1, true).is_empty());
    assert!(tracker.note_evdev("kbd", 63, 0, true).is_empty());
    tracker.note_vk("win", 0xA0, false, true);
    assert!(tracker.note_vk("win", 0x74, false, true).is_empty());
    assert!(tracker.note_vk("win", 0x7B, false, true).is_empty());
    assert_eq!(
        tracker.note_vk("win", 0x73, false, true),
        vec![("F4".into(), true)]
    );
}

#[test]
fn press_release_repeat_and_shift_across_keyboards() {
    let mut tracker = ChordTracker::new();
    assert!(tracker.note_evdev("kbd", 42, 1, true).is_empty());
    assert_eq!(
        tracker.note_evdev("kbd", KEY_F1, 1, true),
        vec![("F1".into(), true)]
    );
    assert_eq!(
        tracker.note_evdev("kbd", KEY_F1, 0, true),
        vec![("F1".into(), false)]
    );

    tracker.note_evdev("kbd", 42, 1, true);
    tracker.note_evdev("kbd", KEY_F1, 1, true);
    assert!(tracker.note_evdev("kbd", KEY_F1, 2, true).is_empty());

    let mut bare = ChordTracker::new();
    assert!(bare.note_evdev("kbd", KEY_F1, 1, true).is_empty());
    assert!(bare.note_evdev("kbd", KEY_F1, 0, true).is_empty());

    let mut boards = ChordTracker::new();
    boards.note_evdev("board-a", 42, 1, true);
    assert_eq!(
        boards.note_evdev("board-b", KEY_F1, 1, true),
        vec![("F1".into(), true)]
    );
    assert_eq!(
        boards.note_evdev("board-a", 42, 0, true),
        vec![("F1".into(), false)]
    );
    assert!(boards.note_evdev("board-b", KEY_F1, 0, true).is_empty());
}

#[test]
fn windows_vk_and_unplug() {
    let mut tracker = ChordTracker::new();
    tracker.note_vk("win", 0xA0, false, true);
    assert_eq!(
        tracker.note_vk("win", 0x70, false, true),
        vec![("F1".into(), true)]
    );
    assert!(tracker.note_vk("win", 0x70, false, true).is_empty());
    assert_eq!(
        tracker.note_vk("win", 0x70, true, true),
        vec![("F1".into(), false)]
    );

    let mut unplug = ChordTracker::new();
    unplug.note_evdev("kbd", 42, 1, true);
    unplug.note_evdev("kbd", KEY_F1, 1, true);
    assert_eq!(unplug.drop_device("kbd"), vec![("F1".into(), false)]);
}

#[test]
fn keyboard_bitmap_and_input_event_width() {
    assert!(!looks_like_keyboard(&[0; 96]));
    let mut bits = vec![0u8; 96];
    bits[(KEY_F1 / 8) as usize] |= 1 << (KEY_F1 % 8);
    assert!(looks_like_keyboard(&bits));
    assert_eq!(KERNEL_EV_KEY, 1);
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        assert_eq!(std::mem::size_of::<libc::input_event>(), 24);
    }
}

#[test]
fn focus_gate_blocks_until_release_and_warns_once() {
    let mut tracker = ChordTracker::new();
    let focused = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&focused);
    let mut gate = FocusGate::new(resolve_target(&json!({})), move || {
        Some(flag.load(Ordering::SeqCst))
    });
    tracker.note_evdev("kbd", 42, 1, true);
    assert!(observe_evdev(&mut tracker, &mut gate, "kbd", KEY_F1, 1).is_empty());
    focused.store(true, Ordering::SeqCst);
    assert!(observe_evdev(&mut tracker, &mut gate, "kbd", KEY_F1, 1).is_empty());
    assert!(observe_evdev(&mut tracker, &mut gate, "kbd", KEY_F1, 0).is_empty());

    let mut tracker = ChordTracker::new();
    let focused = Arc::new(AtomicBool::new(true));
    let flag = Arc::clone(&focused);
    let mut gate = FocusGate::new(resolve_target(&json!({})), move || {
        Some(flag.load(Ordering::SeqCst))
    });
    tracker.note_evdev("kbd", 42, 1, true);
    assert_eq!(
        observe_evdev(&mut tracker, &mut gate, "kbd", KEY_F1, 1),
        vec![("F1".into(), true)]
    );
    focused.store(false, Ordering::SeqCst);
    assert_eq!(
        observe_evdev(&mut tracker, &mut gate, "kbd", KEY_F1, 0),
        vec![("F1".into(), false)]
    );

    let mut tracker = ChordTracker::new();
    let mut gate = FocusGate::new(resolve_target(&json!({"enabled": false})), || Some(false));
    tracker.note_evdev("kbd", 42, 1, true);
    assert_eq!(
        observe_evdev(&mut tracker, &mut gate, "kbd", KEY_F1, 1),
        vec![("F1".into(), true)]
    );

    let mut tracker = ChordTracker::new();
    let mut gate = FocusGate::new(resolve_target(&json!({"when_unknown": "deny"})), || None);
    tracker.note_evdev("kbd", 42, 1, true);
    assert!(observe_evdev(&mut tracker, &mut gate, "kbd", KEY_F1, 1).is_empty());
    assert!(observe_evdev(&mut tracker, &mut gate, "kbd", KEY_F1, 1).is_empty());
    assert_eq!(gate.warning_count, 1);
    assert!(gate
        .last_warning
        .as_deref()
        .unwrap_or("")
        .contains("WARNING: cannot tell"));

    let mut tracker = ChordTracker::new();
    let mut gate = FocusGate::new(resolve_target(&json!({"when_unknown": "allow"})), || None);
    tracker.note_evdev("kbd", 42, 1, true);
    assert_eq!(
        observe_evdev(&mut tracker, &mut gate, "kbd", KEY_F1, 1),
        vec![("F1".into(), true)]
    );
}

#[test]
fn windows_repeat_outside_the_target_does_not_arm() {
    let mut tracker = ChordTracker::new();
    let focused = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&focused);
    let mut gate = FocusGate::new(resolve_target(&json!({})), move || {
        Some(flag.load(Ordering::SeqCst))
    });
    tracker.note_vk("win", 0xA0, false, true);
    assert!(observe_vk(&mut tracker, &mut gate, "win", 0x70, false).is_empty());
    focused.store(true, Ordering::SeqCst);
    assert!(observe_vk(&mut tracker, &mut gate, "win", 0x70, false).is_empty());
    assert!(observe_vk(&mut tracker, &mut gate, "win", 0x70, true).is_empty());
}

#[test]
fn process_window_and_detector_order() {
    assert!(process_matches(
        r"C:\Games\VRChat\VRChat.exe",
        &["vrchat.exe".into()]
    ));
    assert!(!process_matches("notepad.exe", &["VRChat.exe".into()]));
    let fields = parse_hypr_active(r#"{"class":"steam_app_438100","title":"VRChat"}"#).unwrap();
    let patterns = vec!["steam_app_438100".into(), "VRChat".into()];
    assert!(window_matches(&fields, &patterns));
    assert!(!window_matches(&fields, &["firefox".into()]));

    let calls = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&calls);
    let fields = linux_active_fields(
        move |argv| {
            seen.lock().unwrap().push(argv[0].clone());
            Some(r#"{"class":"steam_app_438100","title":"VRChat"}"#.into())
        },
        |name| (name == "hyprctl").then(|| "/usr/bin/hyprctl".into()),
    )
    .unwrap();
    assert_eq!(calls.lock().unwrap().as_slice(), ["hyprctl"]);
    assert!(window_matches(&fields, &["steam_app_438100".into()]));

    let calls = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&calls);
    let fields = linux_active_fields(
        move |argv| {
            seen.lock().unwrap().push(argv[0].clone());
            if argv[0] == "xdotool" {
                return None;
            }
            if argv.len() >= 2 && argv[0] == "xprop" && argv[1] == "-root" {
                return Some("_NET_ACTIVE_WINDOW(WINDOW): window id # 0x1234".into());
            }
            Some("WM_CLASS(STRING) = \"steam_app_438100\"\n".into())
        },
        |name| matches!(name, "xdotool" | "xprop").then(|| name.to_string()),
    )
    .unwrap();
    assert_eq!(
        calls.lock().unwrap().as_slice(),
        ["xdotool", "xprop", "xprop"]
    );
    assert!(window_matches(&fields, &["steam_app_438100".into()]));

    let target = resolve_target(&Value::Null);
    assert!(target.enabled);
    assert_eq!(target.windows_process, ["VRChat.exe"]);
    assert_eq!(target.linux_match, ["steam_app_438100", "VRChat"]);
    assert_eq!(target.when_unknown, "deny");
}

#[test]
fn osc_packets_match_python_bytes() {
    let down = OscSender::packet("/avatar/parameters/Expression1", true);
    let up = OscSender::packet("/avatar/parameters/Expression1", false);
    assert_eq!(
        hex(&down),
        "2f6176617461722f706172616d65746572732f45787072657373696f6e3100002c54000001"
    );
    assert_eq!(
        hex(&up),
        "2f6176617461722f706172616d65746572732f45787072657373696f6e3100002c46000000"
    );
    let (address, value) = parse_osc_packet(&down).unwrap();
    assert_eq!(address, "/avatar/parameters/Expression1");
    assert_eq!(value, OscTyped::Bool(true));
    let packet = encode_osc_string("/avatar/change", "avtr_smile");
    assert_eq!(
        hex(&packet),
        "2f6176617461722f6368616e676500002c730000617674725f736d696c650000"
    );
}

#[test]
fn default_settings_json_and_four_hotkeys() {
    assert_eq!(to_pretty(&default_settings()).unwrap(), GOLDEN_SETTINGS);
    let root = scratch();
    let hotkeys = hotkeys_for(&default_settings(), &root);
    assert_eq!(
        hotkeys.get("F1").map(String::as_str),
        Some("/avatar/parameters/Expression1")
    );
    assert_eq!(
        hotkeys.get("F4").map(String::as_str),
        Some("/avatar/parameters/Expression4")
    );
    assert!(!hotkeys.contains_key("F5"));
    assert!(!hotkeys.contains_key("F12"));
    assert_eq!(hotkeys.len(), 4);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn settings_keep_unknown_keys_and_replace_corrupt_files() {
    let root = scratch();
    let paths = AppPaths::in_dir(&root);
    assert!(load_settings(&paths).as_object().unwrap().is_empty());
    let mut settings = default_settings();
    settings
        .as_object_mut()
        .unwrap()
        .insert("custom_note".into(), json!("keep-me"));
    save_settings(&paths, &settings).unwrap();
    let loaded = load_settings(&paths);
    assert_eq!(
        loaded.get("custom_note").and_then(Value::as_str),
        Some("keep-me")
    );
    fs::write(&paths.config_file, "{").unwrap();
    assert_eq!(load_settings(&paths).get("enabled"), Some(&json!(true)));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn profile_label_needs_the_png_and_backslash_thumbs() {
    let root = scratch();
    let mut profile = default_profile();
    profile.id = "smile".into();
    profile.name = "Smile".into();
    profile.expressions[2].name = "Grin".into();
    profile.expressions[2].osc = "/avatar/parameters/Grin".into();
    profile.expressions[2].thumb = "thumbs/3.png".into();
    save_profile(&profile, &root).unwrap();
    let mut settings = default_settings();
    settings["active_avatar"] = json!("smile");
    settings["fkey_assign"]["F3"] = json!(3);
    let slot = expression_slot(3, &load_profile("smile", &root).unwrap(), &root).unwrap();
    assert_eq!(slot.label, "Expression3");
    assert!(slot.thumb.is_empty());
    assert_eq!(slot.osc, "/avatar/parameters/Grin");
    png(&root.join("avatars/smile/thumbs/3.png"));
    let slot = expression_slot(3, &load_profile("smile", &root).unwrap(), &root).unwrap();
    assert_eq!(slot.label, "Grin");
    assert_eq!(
        Path::new(&slot.thumb),
        root.join("avatars/smile/thumbs/3.png")
    );

    let mut profile = default_profile();
    profile.id = "smile".into();
    profile.expressions[0].thumb = r"thumbs\1.png".into();
    let path = profile_thumb_path(&profile, &profile.expressions[0], &root);
    assert_eq!(Path::new(&path), root.join("avatars/smile/thumbs/1.png"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn import_zip_skips_indexes_above_four_and_rejects_parent_paths() {
    let root = scratch();
    let paths = AppPaths::in_dir(&root);
    let src = root.join("pack");
    fs::create_dir_all(&src).unwrap();
    let mut profile = default_profile();
    profile.id = "smile".into();
    profile.blueprint_id = "avtr_smile".into();
    profile.expressions[0].thumb = "thumbs/1.png".into();
    write_profile_json(&src.join("profile.json"), &profile);
    png(&src.join("thumbs/1.png"));
    let pack = root.join("smile.zip");
    zip_files(
        &pack,
        &[
            (
                "smile/profile.json",
                &fs::read(src.join("profile.json")).unwrap(),
            ),
            (
                "smile/thumbs/1.png",
                &fs::read(src.join("thumbs/1.png")).unwrap(),
            ),
        ],
    );
    let loaded = import_profile_pack(&pack, &paths).unwrap();
    assert_eq!(loaded.profile.id, "smile");
    assert!(root.join("avatars/smile/thumbs/1.png").is_file());

    let wide = root.join("wide");
    fs::create_dir_all(&wide).unwrap();
    let mut raw = serde_json::to_value(default_profile()).unwrap();
    raw["id"] = json!("wide");
    raw["expressions"].as_array_mut().unwrap().push(json!({
        "index": 12,
        "name": "Extra",
        "parameter": "Extra",
        "osc": "/avatar/parameters/Extra",
        "type": "bool",
        "thumb": ""
    }));
    fs::write(
        wide.join("profile.json"),
        serde_json::to_string(&raw).unwrap(),
    )
    .unwrap();
    let loaded = import_profile_pack(&wide, &paths).unwrap();
    let indexes: Vec<i64> = loaded
        .profile
        .expressions
        .iter()
        .map(|item| item.index)
        .collect();
    assert_eq!(indexes, vec![1, 2, 3, 4]);
    assert_eq!(loaded.skipped, vec![12]);
    assert!(loaded.notice.contains("Ignored indexes: 12"));
    assert!(import_notice(&[12], "ja").contains('1'));
    let stored: Value =
        serde_json::from_str(&fs::read_to_string(root.join("avatars/wide/profile.json")).unwrap())
            .unwrap();
    assert!(stored.get("import_notice").is_none());
    assert!(expression_slot(12, &loaded.profile, &root).is_none());

    let bad = root.join("bad.zip");
    zip_files(&bad, &[("../outside.json", b"{}".as_slice())]);
    let err = import_profile_pack(&bad, &paths).unwrap_err();
    assert!(err.contains("unsafe path in profile pack"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn avatar_change_switches_the_saved_profile() {
    let root = scratch();
    let paths = AppPaths::in_dir(&root);
    let mut profile = default_profile();
    profile.id = "smile".into();
    profile.blueprint_id = "avtr_smile".into();
    save_profile(&profile, &root).unwrap();
    save_settings(&paths, &default_settings()).unwrap();
    let switched = note_avatar_change(&paths, "avtr_smile").unwrap();
    assert_eq!(switched.id, "smile");
    assert_eq!(
        load_settings(&paths)
            .get("active_avatar")
            .and_then(Value::as_str),
        Some("smile")
    );
    assert!(note_avatar_change(&paths, "missing").is_none());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn busy_avatar_port_is_reported_and_change_messages_arrive() {
    let held = UdpSocket::bind("127.0.0.1:0").unwrap();
    let port = held.local_addr().unwrap().port();
    let listener = AvatarOscListener::new(port, "127.0.0.1", |_| {});
    assert!(!listener.start());
    assert!(!listener.ok());
    assert!(!listener.error().is_empty());
    drop(held);

    let probe = UdpSocket::bind("127.0.0.1:0").unwrap();
    let port = probe.local_addr().unwrap().port();
    drop(probe);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let record = Arc::clone(&seen);
    let listener = AvatarOscListener::new(port, "127.0.0.1", move |avatar| {
        record.lock().unwrap().push(avatar);
    });
    assert!(listener.start());
    let packet = encode_osc_string("/avatar/change", "avtr_smile");
    UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .send_to(&packet, ("127.0.0.1", port))
        .unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while std::time::Instant::now() < deadline && seen.lock().unwrap().is_empty() {
        thread::sleep(Duration::from_millis(20));
    }
    listener.stop();
    assert_eq!(seen.lock().unwrap().as_slice(), ["avtr_smile"]);
}

struct FakeKeys {
    sent: bool,
    closed: Arc<AtomicBool>,
}

impl KeySource for FakeKeys {
    fn poll(&mut self, _timeout: Duration) -> Vec<KeyEvent> {
        if !self.sent {
            self.sent = true;
            return vec![
                KeyEvent::Evdev {
                    device: "kbd".into(),
                    code: 42,
                    value: 1,
                },
                KeyEvent::Evdev {
                    device: "kbd".into(),
                    code: 59,
                    value: 1,
                },
                KeyEvent::Evdev {
                    device: "kbd".into(),
                    code: 59,
                    value: 0,
                },
            ];
        }
        thread::sleep(Duration::from_millis(20));
        Vec::new()
    }

    fn close(&mut self) {
        self.closed.store(true, Ordering::SeqCst);
    }
}

#[test]
fn listener_sends_press_and_release() {
    let root = scratch();
    let paths = AppPaths::in_dir(&root);
    let server = UdpSocket::bind("127.0.0.1:0").unwrap();
    server
        .set_read_timeout(Some(Duration::from_millis(200)))
        .unwrap();
    let port = server.local_addr().unwrap().port();
    let mut settings = default_settings();
    settings["osc"] = json!({"host": "127.0.0.1", "port": port});
    settings["target"] = json!({"enabled": false});
    save_settings(&paths, &settings).unwrap();
    let closed = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&closed);
    let listener = ExpressionListener::with_source(paths, move || {
        Some(Box::new(FakeKeys {
            sent: false,
            closed: Arc::clone(&flag),
        }))
    });
    listener.start();
    let mut blob = Vec::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    let mut buf = [0u8; 4096];
    while std::time::Instant::now() < deadline && !blob.windows(2).any(|part| part == b",F") {
        if let Ok((size, _)) = server.recv_from(&mut buf) {
            blob.extend_from_slice(&buf[..size]);
        }
    }
    listener.stop();
    assert!(closed.load(Ordering::SeqCst));
    let text = String::from_utf8_lossy(&blob);
    assert!(text.contains("/avatar/parameters/Expression1"), "{text}");
    assert!(blob.windows(2).any(|part| part == b",T"));
    assert!(blob.windows(2).any(|part| part == b",F"));
    let _ = fs::remove_dir_all(root);
}

fn write_profile_json(path: &Path, profile: &Profile) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, serde_json::to_string(profile).unwrap()).unwrap();
}

fn zip_files(path: &Path, files: &[(&str, &[u8])]) {
    let file = fs::File::create(path).unwrap();
    let mut writer = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();
    for (name, bytes) in files {
        writer.start_file(*name, options).unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap();
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
