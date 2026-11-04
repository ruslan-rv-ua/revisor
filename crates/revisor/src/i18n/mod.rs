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
use revisor_core::settings::FocusAfterSnapshot;

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
    /// The file in the place of the settings database is something else.
    NotSettings,
    /// The user cancelled the action, say at the administrator prompt.
    Cancelled,
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

    // The snapshot in progress and its outcome (TASK-8). Counts come with their text in
    // the user's regional format; the number itself picks the plural form.
    /// The label of the progress field, without access key or colon.
    fn progress(&self) -> &'static str;
    /// The progress field while folders are read; without a folder, the count alone.
    fn snapshot_progress(&self, entries: u64, entries_text: &str, folder: &str) -> String;
    /// The progress field while the snapshot is written to the database.
    fn snapshot_saving(&self, entries: u64, entries_text: &str) -> String;
    fn snapshot_started(&self) -> &'static str;
    fn snapshot_saved(&self, entries: u64, entries_text: &str) -> String;
    fn entries_inaccessible(&self, entries_text: &str) -> String;
    /// The folders of the profile the snapshot did not find: each path with the reason.
    fn folders_not_found(&self, folders: &[(String, String)]) -> String;
    fn folder_missing(&self) -> &'static str;
    fn folder_volume_not_ntfs(&self, file_system: &str) -> String;
    fn folder_volume_network(&self) -> &'static str;
    fn folder_is_a_file(&self) -> &'static str;
    fn folder_unreachable(&self, reason: &str) -> String;
    fn snapshot_cancelled(&self) -> &'static str;
    fn snapshot_failed(&self, reason: &str) -> String;
    /// The reason when the snapshot's thread ended without a result.
    fn snapshot_stopped(&self) -> &'static str;
    /// The offer to restart as administrator after a snapshot with entries closed by rights.
    fn restart_question(&self, denied_text: &str) -> String;
    fn restart(&self) -> &'static str;
    /// The questions asked while a snapshot is being taken.
    fn close_during_snapshot(&self) -> &'static str;
    fn restart_during_snapshot(&self) -> &'static str;
    fn restart_failed(&self, reason: &str) -> String;

    // The snapshot list (TASK-9).
    fn column_date_time(&self) -> &'static str;
    fn column_comment(&self) -> &'static str;
    /// Shown in the empty list; `key` is the shortcut that takes a snapshot.
    fn no_snapshots_yet(&self, key: &str) -> String;
    /// Shown in the empty list while there are no profiles; `key` makes a profile.
    fn no_profiles_yet(&self, key: &str) -> String;
    /// The only item of the profile box while there are no profiles.
    fn no_profiles_item(&self) -> &'static str;
    /// The label of the comment field, without access key or colon.
    fn comment_of(&self, snapshot: &str) -> String;
    fn comment_saved(&self) -> &'static str;
    fn comment_removed(&self) -> &'static str;
    fn comment_not_saved(&self, reason: &str) -> String;
    /// The question before one snapshot, named with its comment, is deleted.
    fn delete_snapshot_question(&self, name: &str, comment: Option<&str>) -> String;
    /// The question before several snapshots are deleted.
    fn delete_snapshots_question(&self, count: usize) -> String;
    /// The status bar message once snapshots are deleted and the compaction started.
    fn snapshots_deleted(&self, count: usize, profile: &str) -> String;
    fn snapshots_not_deleted(&self, reason: &str) -> String;
    /// The progress field while a profile's database is compacted.
    fn compacting(&self, profile: &str) -> String;
    /// `freed` is the space returned, in the user's format.
    fn compaction_done(&self, profile: &str, freed: &str) -> String;
    fn compaction_cancelled(&self, profile: &str) -> String;
    fn compaction_failed(&self, profile: &str, reason: &str) -> String;
    /// The reason when the compaction's thread ended without a result.
    fn compaction_stopped(&self) -> &'static str;

    // The settings window (TASK-9); labels come without access key or colon.
    fn focus_after_snapshot(&self) -> &'static str;
    fn focus_option(&self, value: FocusAfterSnapshot) -> &'static str;
    fn confirm_snapshot_deletion(&self) -> &'static str;
    fn settings_saved(&self) -> &'static str;
    fn settings_not_saved(&self, reason: &str) -> String;
    /// The settings file could not be used, so the defaults are.
    fn settings_not_opened(&self, file: &str, reason: &str) -> String;

    // The debug-only demo state (REVISOR_DEMO=1).
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

    #[test]
    fn snapshot_counts_take_their_plural_forms() {
        assert_eq!(
            Uk.snapshot_progress(1, "1", r"C:\x"),
            r"1 запис, читається C:\x"
        );
        assert_eq!(Uk.snapshot_progress(3, "3", ""), "3 записи");
        assert_eq!(Uk.snapshot_saved(5, "5"), "Знімок збережено: 5 записів.");
        assert_eq!(Uk.snapshot_saved(21, "21"), "Знімок збережено: 21 запис.");
        assert_eq!(
            Uk.snapshot_saving(12345, "12 345"),
            "Збереження знімка: 12 345 записів…"
        );
        assert_eq!(
            En.snapshot_progress(1, "1", r"C:\x"),
            r"1 entry, reading C:\x"
        );
        assert_eq!(En.snapshot_progress(0, "0", ""), "0 entries");
        assert_eq!(
            En.snapshot_saved(12345, "12,345"),
            "Snapshot saved: 12,345 entries."
        );
    }

    #[test]
    fn the_delete_snapshot_questions_name_the_snapshot_or_count_them() {
        assert_eq!(
            Uk.delete_snapshot_question("31.10.2026 14:05:12", Some("перед встановленням X")),
            "Видалити знімок 31.10.2026 14:05:12 «перед встановленням X»? Цю дію не можна \
             скасувати."
        );
        assert_eq!(
            Uk.delete_snapshot_question("31.10.2026 14:05:12", None),
            "Видалити знімок 31.10.2026 14:05:12? Цю дію не можна скасувати."
        );
        for (n, snapshots) in [(2, "2 знімки"), (5, "5 знімків"), (21, "21 знімок")]
        {
            assert_eq!(
                Uk.delete_snapshots_question(n),
                format!("Видалити {snapshots}? Цю дію не можна скасувати.")
            );
        }
        assert_eq!(
            Uk.snapshots_deleted(3, "Дім"),
            "Видалено 3 знімки; триває стискання бази профілю «Дім»."
        );
        assert_eq!(
            En.delete_snapshot_question("10/31/2026 2:05:12 PM", Some("before X")),
            "Delete the snapshot 10/31/2026 2:05:12 PM “before X”? This cannot be undone."
        );
        assert_eq!(
            En.delete_snapshots_question(2),
            "Delete 2 snapshots? This cannot be undone."
        );
        assert_eq!(
            En.snapshots_deleted(1, "Home"),
            "Deleted 1 snapshot; the database of the profile “Home” is being compacted."
        );
    }

    #[test]
    fn the_empty_list_hints_name_the_keys() {
        // In a sentence the word «клавіша» comes before the key's name (document
        // «Клавіатура й меню»).
        assert_eq!(
            Uk.no_snapshots_yet("F5"),
            "Знімків ще немає. Натисніть клавішу F5, щоб зробити перший знімок."
        );
        assert_eq!(
            Uk.no_profiles_yet("Ctrl+N"),
            "Профілів ще немає. Натисніть клавіші Ctrl+N, щоб створити перший профіль."
        );
        assert_eq!(
            En.no_snapshots_yet("F5"),
            "No snapshots yet. Press F5 to take the first snapshot."
        );
        assert_eq!(
            En.no_profiles_yet("Ctrl+N"),
            "No profiles yet. Press Ctrl+N to create the first profile."
        );
    }

    #[test]
    fn folders_not_found_are_named_with_their_reasons() {
        let folders = [
            (r"D:\Робота".to_string(), Uk.folder_missing().to_string()),
            (r"E:\Архів".to_string(), Uk.folder_volume_not_ntfs("FAT32")),
        ];
        assert_eq!(
            Uk.folders_not_found(&folders),
            r"Тек не знайдено: «D:\Робота» (теки немає); «E:\Архів» (тека на томі FAT32, не NTFS)."
        );
        let folders = [(r"D:\Work".to_string(), En.folder_missing().to_string())];
        assert_eq!(
            En.folders_not_found(&folders),
            r"Folders not found: “D:\Work” (the folder is missing)."
        );
    }
}
