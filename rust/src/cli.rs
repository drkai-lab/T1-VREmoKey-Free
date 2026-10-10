use clap::Parser;

use crate::listener;
use crate::paths::AppPaths;
use crate::{APP_NAME, VERSION};

#[derive(Parser)]
#[command(
    name = APP_NAME,
    version = VERSION,
    about = "Free VRChat expression hotkeys (Shift+F1..F4)"
)]
struct Cli {
    /// run the chord listener without the window
    #[arg(long)]
    listen: bool,
}

pub fn entry() -> i32 {
    let cli = Cli::parse();
    let paths = AppPaths::system();
    if cli.listen {
        listener::run_listen(&paths)
    } else {
        crate::gui::run(&paths)
    }
}
