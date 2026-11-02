//! Interface strings (ADR-0009): one module per language behind the `Strings` trait.
//!
//! Every string is a method, and strings chosen by an id are an exhaustive `match`, so a
//! language that lacks a string, or takes other parameters for it, does not compile.

mod en;
pub mod plural;
mod uk;

pub use en::En;
pub use uk::Uk;

use revisor_core::exclude::PatternError;

use crate::commands::{CommandId, MenuId, Pane, Reason, SubmenuId};

/// An interface language.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    Uk,
}

/// Why an action on a file failed, in the terms revisor explains in its own words (brief,
/// section 5); anything else keeps the text and code of Windows or SQLite.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    AccessDenied,
    /// Another program, maybe another copy of revisor, has the file open.
    InUse,
    DiskFull,
    /// A newer version of revisor made the profile.
    NewerVersion,
    NotAProfile,
    Other {
        text: String,
        code: Option<i64>,
    },
}

/// All interface strings of one language.
pub trait Strings {
    fn lang(&self) -> Lang;

    /// A command's name as the menu shows it, without access key or ellipsis.
    fn command(&self, id: CommandId) -> &'static str;
    fn menu(&self, id: MenuId) -> &'static str;
    fn submenu(&self, id: SubmenuId) -> &'static str;
    /// Why a command cannot run now, as the second half of `unavailable`.
    fn reason(&self, reason: Reason) -> &'static str;
    /// The status bar message for a command that cannot run now.
    fn unavailable(&self, command: &str, reason: &str) -> String;
    /// The status bar message for a key used outside the only pane it works in.
    fn key_only_in(&self, key: &str, pane: Pane) -> String;

    // The «Keyboard shortcuts» window.
    /// After a shortcut limited to one pane: "(only in the snapshot list)".
    fn only_in(&self, pane: Pane) -> String;
    /// A key that works in one list: "Enter in the snapshot list".
    fn in_list(&self, key: &str, pane: Pane) -> String;
    fn navigation(&self) -> &'static str;
    fn switch_panes(&self) -> &'static str;
    fn context_menu(&self) -> &'static str;

    // The main window.
    /// A pane's main list, as screen readers name it.
    fn pane(&self, pane: Pane) -> &'static str;
    /// The label of the profile box, without access key or colon.
    fn profile(&self) -> &'static str;
    /// Said when a list's context menu would have no items.
    fn no_actions(&self) -> &'static str;
    /// Closes the «Help» windows; Esc does the same.
    fn close(&self) -> &'static str;
    /// The «About revisor» text above the third-party licences.
    fn about(&self, version: &str) -> String;

    // The profile window (TASK-7); labels come without access key or colon.
    fn profile_name(&self) -> &'static str;
    fn profile_folders(&self) -> &'static str;
    fn add_folder(&self) -> &'static str;
    fn remove_folder(&self) -> &'static str;
    fn profile_exclusions(&self) -> &'static str;
    /// The button that makes a new profile.
    fn create(&self) -> &'static str;
    fn ok(&self) -> &'static str;
    fn cancel(&self) -> &'static str;
    /// The prompt of the folder picker.
    fn choose_folder(&self) -> &'static str;
    fn name_missing(&self) -> &'static str;
    fn name_taken(&self, name: &str) -> String;
    fn no_folders(&self) -> &'static str;
    fn bad_pattern(&self, line: usize, pattern: &str, error: PatternError) -> String;
    /// A folder whose files a folder of the profile covers already.
    fn folder_covered(&self, folder: &str, by: &str) -> String;
    fn folder_not_ntfs(&self, folder: &str, file_system: &str) -> String;
    fn folder_on_network(&self, folder: &str) -> String;
    fn folder_not_examined(&self, folder: &str, reason: &str) -> String;
    /// The question whether a new folder replaces the profile's folders inside it.
    fn replace_folders(&self, folder: &str, inside: &[String]) -> String;
    fn replace(&self) -> &'static str;

    // Profiles in the main window (TASK-7).
    fn delete_profile_question(&self, name: &str, snapshots: usize) -> String;
    fn delete(&self) -> &'static str;
    fn profile_created(&self, name: &str) -> String;
    fn profile_changed(&self, name: &str) -> String;
    fn profile_deleted(&self, name: &str) -> String;
    fn profile_not_saved(&self, reason: &str) -> String;
    fn profile_not_deleted(&self, name: &str, reason: &str) -> String;
    /// A file in the folder of profiles that revisor cannot open as a profile.
    fn profile_not_opened(&self, file: &str, reason: &str) -> String;
    fn profiles_not_listed(&self, folder: &str, reason: &str) -> String;
    /// Why an action on a file failed, as the second half of a message.
    fn failure(&self, failure: &Failure) -> String;

    // The debug-only demo state (REVISOR_DEMO=1).
    fn demo_snapshot(&self, n: usize) -> String;
    fn demo_change(&self, n: usize) -> String;
    fn demo_done(&self, command: &str) -> String;

    // Key names that are not printed on most keyboards.
    fn key_space(&self) -> &'static str;
    fn key_up(&self) -> &'static str;
    fn key_down(&self) -> &'static str;
    fn key_left(&self) -> &'static str;
    fn key_right(&self) -> &'static str;
    fn key_context_menu(&self) -> &'static str;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_delete_question_counts_the_snapshots_in_each_language() {
        let uk = |n| Uk.delete_profile_question("Дім", n);
        assert_eq!(
            uk(0),
            "Видалити профіль «Дім»? Знімків у ньому немає. Цю дію не можна скасувати."
        );
        for (n, snapshots) in [
            (1, "1 знімок"),
            (3, "3 знімки"),
            (5, "5 знімків"),
            (11, "11 знімків"),
            (21, "21 знімок"),
        ] {
            assert_eq!(
                uk(n),
                format!("Видалити профіль «Дім» і {snapshots} у ньому? Цю дію не можна скасувати.")
            );
        }
        let en = |n| En.delete_profile_question("Home", n);
        assert_eq!(
            en(0),
            "Delete the profile “Home”? It has no snapshots. This cannot be undone."
        );
        assert_eq!(
            en(1),
            "Delete the profile “Home” and its 1 snapshot? This cannot be undone."
        );
        assert_eq!(
            en(2),
            "Delete the profile “Home” and its 2 snapshots? This cannot be undone."
        );
    }
}
