#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

fn main() {
    std::process::exit(vrc_expression_hotkeys::cli::entry());
}
