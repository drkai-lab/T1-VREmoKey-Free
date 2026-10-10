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
    /// run the stand-in walkthrough (no keyboard, no VRChat) and exit
    #[cfg(feature = "emulate")]
    #[arg(long)]
    emulate: bool,
}

pub fn entry() -> i32 {
    let cli = Cli::parse();
    #[cfg(feature = "emulate")]
    if cli.emulate {
        if cli.listen {
            eprintln!("--emulate and --listen cannot be used together");
            return 2;
        }
        return match crate::emulate::run_walkthrough() {
            Ok(report) => {
                print!("{report}");
                0
            }
            Err(err) => {
                eprintln!("{err}");
                1
            }
        };
    }
    let paths = AppPaths::system();
    if cli.listen {
        listener::run_listen(&paths)
    } else {
        crate::gui::run(&paths)
    }
}
