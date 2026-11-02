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
mod state;
mod win32;

use std::path::{Path, PathBuf};

use i18n::{En, Lang, Strings, Uk};
use main_window::MainWindow;
use state::AppState;

fn main() {
    let strings: &'static dyn Strings = match win32::ui_language() {
        Lang::Uk => &Uk,
        Lang::En => &En,
    };
    // REVISOR_DEMO=1 in a debug build stands in for the state TASK-8…14 will bring, so
    // the keyboard and the status bar can be checked with NVDA.
    let demo = cfg!(debug_assertions) && std::env::var_os("REVISOR_DEMO").is_some_and(|v| v == "1");
    let state = if demo {
        AppState::demo()
    } else {
        AppState::new()
    };
    wxdragon::main(move |_| {
        // The frame's event handlers hold the window, so it lives as long as the frame.
        MainWindow::open(strings, state, profiles_dir());
    })
    .expect("failed to start the GUI");
}

/// The folder of profiles: revisor is portable, so all its data is in `data` next to the
/// exe.
fn profiles_dir() -> PathBuf {
    let exe = std::env::current_exe().unwrap_or_default();
    exe.parent()
        .unwrap_or(Path::new(""))
        .join("data")
        .join("profiles")
}
