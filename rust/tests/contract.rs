//! The listener still lets keys through, and the Rust sources stay off the hardware protocol.

use std::fs;
use std::path::Path;

#[test]
fn hook_and_focus_sources_are_the_passthrough_ones() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let combined = read_tree(&root);
    for token in [
        "CallNextHookEx",
        "WH_KEYBOARD_LL",
        "GetForegroundWindow",
        "QueryFullProcessImageNameW",
        "hyprctl",
        "xdotool",
        "xprop",
    ] {
        assert!(combined.contains(token), "missing {token}");
    }
    for token in [
        "RegisterHotKey(",
        "GetAsyncKeyState(",
        "EVIOCGRAB",
        ".grab(",
    ] {
        assert!(!combined.contains(token), "found {token}");
    }
    let product: String = [84u8, 49].into_iter().map(char::from).collect();
    let hid: String = [72u8, 73, 68].into_iter().map(char::from).collect();
    let usbfs: String = [117u8, 115, 98, 102, 115]
        .into_iter()
        .map(char::from)
        .collect();
    assert!(!combined.contains(&hid));
    assert!(!combined.contains(&usbfs));
    assert!(combined.contains(&product));
}

#[test]
fn emulate_injection_stays_behind_the_feature() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let toml = fs::read_to_string(root.join("Cargo.toml")).unwrap();
    let features = toml
        .split("[features]")
        .nth(1)
        .expect("features table")
        .split("\n[")
        .next()
        .unwrap();
    assert!(features.contains("emulate = []"));
    assert!(!features.contains("default"));
    for relative in [
        "src/lib.rs",
        "src/cli.rs",
        "src/focus.rs",
        "src/keys_linux.rs",
        "src/keys_windows.rs",
    ] {
        let text = fs::read_to_string(root.join(relative)).unwrap();
        assert!(
            text.contains("feature = \"emulate\""),
            "{relative} does not gate the stand-in"
        );
    }
}

fn read_tree(root: &Path) -> String {
    let mut out = String::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
                out.push_str(&fs::read_to_string(&path).unwrap());
                out.push('\n');
            }
        }
    }
    out
}
