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
use revisor_core::settings::{Language, Settings};
use state::AppState;

fn main() {
    let data_dir = data_dir();
    let strings: &'static dyn Strings = match interface_language(&data_dir) {
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
        MainWindow::open(strings, state, data_dir, open);
    })
    .expect("failed to start the GUI");
}

/// The language the interface speaks for this run (TASK-16): the setting «Language», read
/// before any window is made; «Same as Windows», and a settings file that cannot be read,
/// take the interface language of Windows. The main window reports the unusable file.
fn interface_language(data_dir: &Path) -> Lang {
    let chosen = Settings::open(&data_dir.join(main_window::SETTINGS_FILE))
        .and_then(|settings| settings.language())
        .unwrap_or_default();
    language_of(chosen, win32::ui_language)
}

/// The language a value of the setting stands for; `windows` is read only when it is
/// needed, as it is a system call.
pub fn language_of(chosen: Language, windows: impl FnOnce() -> Lang) -> Lang {
    match chosen {
        Language::SameAsWindows => windows(),
        Language::English => Lang::En,
        Language::Ukrainian => Lang::Uk,
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_setting_picks_the_language_and_only_same_as_windows_asks_windows() {
        assert_eq!(language_of(Language::SameAsWindows, || Lang::Uk), Lang::Uk);
        assert_eq!(language_of(Language::SameAsWindows, || Lang::En), Lang::En);
        assert_eq!(language_of(Language::English, || unreachable!()), Lang::En);
        assert_eq!(
            language_of(Language::Ukrainian, || unreachable!()),
            Lang::Uk
        );
    }
}
