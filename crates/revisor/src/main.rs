// Hide the console window in release builds; keep it in debug for logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod failure;
mod help;
mod i18n;
mod keyboard;
mod keys;
mod main_window;
mod menus;
mod message_box;
mod profile_window;
mod settings_window;
mod snapshots;
mod state;
mod status;
mod win32;

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use i18n::{En, Lang, Strings, Uk};
use main_window::MainWindow;
use revisor_core::elevation;
use state::AppState;

fn main() {
    let strings: &'static dyn Strings = match win32::ui_language() {
        Lang::Uk => &Uk,
        Lang::En => &En,
    };
    // An administrator reads with the backup privilege (TASK-8); a standard user has no
    // such privilege, and nothing changes for them.
    let _ = elevation::enable_backup_privilege();
    // REVISOR_DEMO=1 in a debug build stands in for the state TASK-11…14 will bring, so
    // the keyboard and the status bar can be checked with NVDA.
    let demo = cfg!(debug_assertions) && std::env::var_os("REVISOR_DEMO").is_some_and(|v| v == "1");
    let mut state = if demo {
        AppState::demo()
    } else {
        AppState::new()
    };
    state.elevated = elevation::is_elevated();
    let open = profile_argument();
    wxdragon::main(move |_| {
        // The frame's event handlers hold the window, so it lives as long as the frame.
        MainWindow::open(strings, state, data_dir(), open);
    })
    .expect("failed to start the GUI");
}

/// The data folder: revisor is portable, so all its data — the profiles and the settings
/// — is in `data` next to the exe.
fn data_dir() -> PathBuf {
    let exe = std::env::current_exe().unwrap_or_default();
    exe.parent().unwrap_or(Path::new("")).join("data")
}

/// The profile file named on the command line as `--profile <path>`: a revisor restarted
/// as administrator opens the profile it was restarted from (TASK-8).
fn profile_argument() -> Option<PathBuf> {
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if arg == OsStr::new("--profile") {
            return args.next().map(PathBuf::from);
        }
    }
    None
}
