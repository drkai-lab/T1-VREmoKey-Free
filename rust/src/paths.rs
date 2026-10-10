use std::path::{Path, PathBuf};

use crate::APP_NAME;

#[derive(Clone, Debug)]
pub struct AppPaths {
    pub config_dir: PathBuf,
    pub config_file: PathBuf,
    pub state_dir: PathBuf,
    pub log_file: PathBuf,
}

impl AppPaths {
    /// Settings and state directories for this operating system.
    ///
    /// `VEH_CONFIG_DIR` overrides both, so a screenshot or a test can run
    /// without touching the real configuration.
    pub fn system() -> Self {
        if let Some(dir) = std::env::var_os("VEH_CONFIG_DIR") {
            return Self::in_dir(Path::new(&dir));
        }
        system_paths()
    }

    pub fn in_dir(root: &Path) -> Self {
        Self {
            config_dir: root.to_path_buf(),
            config_file: root.join("settings.json"),
            state_dir: root.to_path_buf(),
            log_file: root.join("listener.log"),
        }
    }
}

#[cfg(windows)]
fn system_paths() -> AppPaths {
    let root = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join("AppData").join("Roaming"));
    let config = root.join(APP_NAME);
    let state = config.join("state");
    AppPaths {
        config_file: config.join("settings.json"),
        log_file: state.join("listener.log"),
        config_dir: config,
        state_dir: state,
    }
}

#[cfg(target_os = "macos")]
fn system_paths() -> AppPaths {
    let config = home_dir()
        .join("Library")
        .join("Application Support")
        .join(APP_NAME);
    let state = config.join("state");
    AppPaths {
        config_file: config.join("settings.json"),
        log_file: state.join("listener.log"),
        config_dir: config,
        state_dir: state,
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
fn system_paths() -> AppPaths {
    let home = home_dir();
    let config = home.join(".config").join(APP_NAME);
    let state = home.join(".local").join("state").join(APP_NAME);
    AppPaths {
        config_file: config.join("settings.json"),
        log_file: state.join("listener.log"),
        config_dir: config,
        state_dir: state,
    }
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn log_line(paths: &AppPaths, message: &str) {
    if std::fs::create_dir_all(&paths.state_dir).is_err() {
        return;
    }
    let stamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    let line = format!("{stamp} {message}\n");
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&paths.log_file)
    {
        use std::io::Write;
        let _ = file.write_all(line.as_bytes());
    }
}
