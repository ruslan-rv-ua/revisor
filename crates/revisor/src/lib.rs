//! revisor, the application: the windows on wxDragon over `revisor-core` (ADR-0005).
//! `main.rs` only calls [`run`]; the library is there so that the development tools in
//! `examples/` can reach the interface strings (TASK-17).
mod access;
mod commands;
mod compare_window;
mod details;
mod difference;
mod difference_tree;
mod explorer;
mod failure;
mod help;
mod i18n;
mod jobs;
mod keyboard;
mod keys;
mod main_window;
mod menus;
mod message_box;
mod profile_window;
mod settings_window;
mod snapshot_list;
mod snapshots;
mod state;
mod status;
mod text_window;
mod theme;
mod typical_folders_window;
mod win32;

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use i18n::{En, Lang, Strings, Uk};
use main_window::MainWindow;
use revisor_core::elevation;
use revisor_core::settings::{Language, Settings};

pub use i18n::catalog::catalog;

/// Starts revisor: reads the language and the theme, opens the main window and runs the
/// GUI.
pub fn run() {
    let data_dir = data_dir();
    // Both are read before any window is made; the main window opens the settings itself.
    let settings = Settings::open(&data_dir.join(main_window::SETTINGS_FILE)).ok();
    let strings: &'static dyn Strings = match interface_language(settings.as_ref()) {
        Lang::Uk => &Uk,
        Lang::En => &En,
    };
    // A settings file that cannot be read gives the theme of Windows (TASK-34).
    let theme = settings
        .as_ref()
        .and_then(|settings| settings.theme().ok())
        .unwrap_or_default();
    drop(settings);
    // An administrator reads with the backup privilege (TASK-8); a standard user has no
    // such privilege, and nothing changes for them.
    let _ = elevation::enable_backup_privilege();
    let elevated = elevation::is_elevated();
    let open = profile_argument();
    wxdragon::main(move |_| {
        // The theme first, so that no window ever shows in another one.
        theme::apply(theme);
        // The frame's event handlers hold the window, so it lives as long as the frame.
        MainWindow::open(strings, elevated, data_dir, open);
    })
    .expect("failed to start the GUI");
}

/// The language the interface speaks for this run (TASK-16): the setting «Language», read
/// before any window is made; «Same as Windows», and a settings file that cannot be read,
/// take the interface language of Windows. The main window reports the unusable file.
fn interface_language(settings: Option<&Settings>) -> Lang {
    let chosen = settings
        .and_then(|settings| settings.language().ok())
        .unwrap_or_default();
    language_of(chosen, win32::language::ui_language)
}

/// The language a value of the setting stands for; `windows` is read only when it is
/// needed, as it is a system call.
pub(crate) fn language_of(chosen: Language, windows: impl FnOnce() -> Lang) -> Lang {
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
