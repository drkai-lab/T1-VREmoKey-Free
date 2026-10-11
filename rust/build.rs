fn main() {
    println!("cargo:rerun-if-changed=assets/app.ico");
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" {
        let mut res = winres::WindowsResource::new();
        if let Ok(target) = std::env::var("TARGET") {
            if target.contains("windows-gnu") {
                let (windres, ar) = gnu_tools(&target);
                res.set_windres_path(&windres);
                res.set_ar_path(&ar);
            }
        }
        res.set_icon("assets/app.ico");
        res.set("FileVersion", "2.1.0.0");
        res.set("ProductVersion", "2.1.0.0");
        res.set("ProductName", "T1-VREmoKey-Free");
        res.set("FileDescription", "VRChat expression hotkeys");
        res.set("CompanyName", "drkai-lab");
        res.set("LegalCopyright", "MIT");
        res.set("InternalName", "vrc-expression-hotkeys");
        res.set("OriginalFilename", "vrc-expression-hotkeys.exe");
        res.compile().expect("embed the Windows application icon");
    }
}

fn gnu_tools(target: &str) -> (String, String) {
    let prefixed = (format!("{target}-windres"), format!("{target}-ar"));
    if tool_exists(&prefixed.0) && tool_exists(&prefixed.1) {
        return prefixed;
    }
    (
        "x86_64-w64-mingw32-windres".into(),
        "x86_64-w64-mingw32-ar".into(),
    )
}

fn tool_exists(name: &str) -> bool {
    std::process::Command::new(name)
        .arg("--version")
        .output()
        .is_ok()
}
