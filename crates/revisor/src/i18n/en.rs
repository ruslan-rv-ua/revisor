//! English interface strings, the base language.

use revisor_core::exclude::PatternError;
use revisor_core::settings::{FocusAfterComparison, FocusAfterSnapshot, Language};

use super::plural::{Category, category};
use super::{Failure, Lang, Strings};
use crate::commands::{CommandId, MenuId, Pane, Reason, SubmenuId};
use crate::difference::Kind;

pub struct En;

impl En {
    /// "1 entry", "12,345 entries".
    fn entries(&self, entries: u64, entries_text: &str) -> String {
        match category(Lang::En, entries.into()) {
            Category::One => format!("{entries_text} entry"),
            _ => format!("{entries_text} entries"),
        }
    }

    /// "1 snapshot", "3 snapshots".
    fn snapshots(&self, count: usize) -> String {
        match category(Lang::En, (count as u64).into()) {
            Category::One => format!("{count} snapshot"),
            _ => format!("{count} snapshots"),
        }
    }
}

impl Strings for En {
    fn lang(&self) -> Lang {
        Lang::En
    }

    fn command(&self, id: CommandId) -> &'static str {
        match id {
            CommandId::NewProfile => "New profile",
            CommandId::EditProfile => "Edit profile",
            CommandId::DeleteProfile => "Delete profile",
            CommandId::Settings => "Settings",
            CommandId::RestartAsAdmin => "Restart as administrator",
            CommandId::Exit => "Exit",
            CommandId::TakeSnapshot => "Take snapshot",
            CommandId::SnapshotAndCompare => "Snapshot and compare",
            CommandId::Comment => "Comment",
            CommandId::DeleteSnapshots => "Delete snapshots",
            CommandId::CancelSnapshot => "Cancel snapshot",
            CommandId::CancelCompaction => "Cancel compaction",
            CommandId::CompareWithPrevious => "Compare with previous",
            CommandId::CompareWith => "Compare with",
            CommandId::CancelComparison => "Cancel comparison",
            CommandId::Details => "Details",
            CommandId::ShowInExplorer => "Show in File Explorer",
            CommandId::FilterByText => "Filter by text",
            CommandId::ShowAdded => "Added",
            CommandId::ShowDeleted => "Deleted",
            CommandId::ShowModified => "Modified",
            CommandId::ShowInaccessible => "Inaccessible",
            CommandId::SortByPath => "Path",
            CommandId::SortByName => "Name",
            CommandId::SortByChange => "Change",
            CommandId::SortByWhatChanged => "What changed",
            CommandId::SortBySize => "Size",
            CommandId::SortByDateModified => "Date modified",
            CommandId::SortByFolder => "Folder",
            CommandId::SortAscending => "Ascending",
            CommandId::SortDescending => "Descending",
            CommandId::KeyboardShortcuts => "Keyboard shortcuts",
            CommandId::About => "About revisor",
        }
    }

    fn menu(&self, id: MenuId) -> &'static str {
        match id {
            MenuId::File => "File",
            MenuId::Snapshot => "Snapshot",
            MenuId::Compare => "Compare",
            MenuId::Difference => "Difference",
            MenuId::Help => "Help",
        }
    }

    fn submenu(&self, id: SubmenuId) -> &'static str {
        match id {
            SubmenuId::SortBy => "Sort by",
        }
    }

    fn reason(&self, reason: Reason) -> &'static str {
        match reason {
            Reason::NoProfiles => "there are no profiles yet",
            Reason::NoSnapshots => "there are no snapshots yet",
            Reason::NoDifference => "there is no difference yet",
            Reason::SnapshotRunning => "a snapshot is being taken",
            Reason::SnapshotOfThisProfile => "a snapshot of this profile is being taken",
            Reason::SnapshotNotRunning => "no snapshot is being taken",
            Reason::ComparisonNotRunning => "no comparison is running",
            Reason::CompactionNotRunning => "no compaction is running",
            Reason::CompactionRunning => {
                "a database is being compacted, and only one goes at a time"
            }
            Reason::CompactionOfThisProfile => "the database of this profile is being compacted",
            Reason::NoSelection => "no snapshot is selected",
            Reason::NoPreviousSnapshot => "there is no previous snapshot",
            Reason::NoOtherSnapshot => "the profile has no other snapshot",
            Reason::AlreadyAdmin => "revisor is already running as administrator",
            Reason::NotYet => "this version of revisor does not have this command yet",
        }
    }

    fn unavailable(&self, command: &str, reason: &str) -> String {
        format!("{command}: {reason}.")
    }

    fn key_only_in(&self, key: &str, pane: Pane) -> String {
        let pane = match pane {
            Pane::Snapshots => "the snapshot list",
            Pane::Difference => "the difference list",
        };
        format!("The {key} key works only in {pane}.")
    }

    fn only_in(&self, pane: Pane) -> String {
        match pane {
            Pane::Snapshots => "(only in the snapshot list)".into(),
            Pane::Difference => "(only in the difference list)".into(),
        }
    }

    fn in_list(&self, key: &str, pane: Pane) -> String {
        match pane {
            Pane::Snapshots => format!("{key} in the snapshot list"),
            Pane::Difference => format!("{key} in the difference list"),
        }
    }

    fn navigation(&self) -> &'static str {
        "Navigation"
    }

    fn switch_panes(&self) -> &'static str {
        "Move between the snapshot list and the difference list"
    }

    fn context_menu(&self) -> &'static str {
        "Context menu"
    }

    fn pane(&self, pane: Pane) -> &'static str {
        match pane {
            Pane::Snapshots => "Snapshots",
            Pane::Difference => "Difference",
        }
    }

    fn profile(&self) -> &'static str {
        "Profile"
    }

    fn no_actions(&self) -> &'static str {
        "No actions are available here now."
    }

    fn close(&self) -> &'static str {
        "Close"
    }

    fn about(&self, version: &str) -> String {
        format!(
            "revisor {version}\nLicence: MIT\nhttps://github.com/ruslan-rv-ua/revisor\n\n\
             Third-party components and their licences:\n"
        )
    }

    fn profile_name(&self) -> &'static str {
        "Name"
    }

    fn profile_folders(&self) -> &'static str {
        "Folders"
    }

    fn add_folder(&self) -> &'static str {
        "Add folder"
    }

    fn remove_folder(&self) -> &'static str {
        "Remove folder"
    }

    fn profile_exclusions(&self) -> &'static str {
        "Exclusions"
    }

    fn create(&self) -> &'static str {
        "Create"
    }

    fn ok(&self) -> &'static str {
        "OK"
    }

    fn cancel(&self) -> &'static str {
        "Cancel"
    }

    fn choose_folder(&self) -> &'static str {
        "Choose a folder for the profile"
    }

    fn name_missing(&self) -> &'static str {
        "Enter a name for the profile."
    }

    fn name_taken(&self, name: &str) -> String {
        format!("There is a profile called “{name}” already. Choose another name.")
    }

    fn no_folders(&self) -> &'static str {
        "Add at least one folder to the profile."
    }

    fn bad_pattern(&self, line: usize, pattern: &str, error: PatternError) -> String {
        let why = match error {
            PatternError::NotFullPath => "a pattern with \\ is a full path, so it starts with \
                a drive, such as C:\\, or with an environment variable, such as %TEMP%"
                .to_string(),
            PatternError::Slash => "paths are written with \\, not /".to_string(),
            PatternError::Colon => {
                "a colon comes only after the drive letter at the start of a path, as in C:\\"
                    .to_string()
            }
            PatternError::Control(c) => {
                format!("it has a control character (U+{:04X})", u32::from(c))
            }
            PatternError::BadChar(c) => format!("no file or folder name has the character {c}"),
        };
        format!("Line {line} of the exclusions, “{pattern}”: {why}.")
    }

    fn folder_covered(&self, folder: &str, by: &str) -> String {
        format!(
            "The folder “{folder}” was not added: the profile's folder “{by}” covers its \
             files already."
        )
    }

    fn folder_not_ntfs(&self, folder: &str, file_system: &str) -> String {
        format!(
            "The folder “{folder}” cannot be added: revisor supports NTFS volumes only, and \
             this folder is on a {file_system} volume."
        )
    }

    fn folder_on_network(&self, folder: &str) -> String {
        format!(
            "The folder “{folder}” cannot be added: revisor supports NTFS volumes of this \
             computer only, and this is a network folder."
        )
    }

    fn folder_not_examined(&self, folder: &str, reason: &str) -> String {
        format!("The folder “{folder}” cannot be added: revisor cannot examine it: {reason}.")
    }

    fn replace_folders(&self, folder: &str, inside: &[String]) -> String {
        let inside: Vec<String> = inside.iter().map(|f| format!("“{f}”")).collect();
        format!(
            "The folder “{folder}” holds folders of the profile: {}. Replace them with it?",
            inside.join(", ")
        )
    }

    fn replace(&self) -> &'static str {
        "Replace"
    }

    fn delete_profile_question(&self, name: &str, snapshots: usize) -> String {
        if snapshots == 0 {
            return format!(
                "Delete the profile “{name}”? It has no snapshots. This cannot be undone."
            );
        }
        format!(
            "Delete the profile “{name}” and its {}? This cannot be undone.",
            self.snapshots(snapshots)
        )
    }

    fn delete(&self) -> &'static str {
        "Delete"
    }

    fn profile_created(&self, name: &str) -> String {
        format!("Profile “{name}” created.")
    }

    fn profile_changed(&self, name: &str) -> String {
        format!("Profile “{name}” changed.")
    }

    fn profile_deleted(&self, name: &str) -> String {
        format!("Profile “{name}” deleted.")
    }

    fn profile_not_saved(&self, reason: &str) -> String {
        format!("The profile was not saved: {reason}.")
    }

    fn profile_not_deleted(&self, name: &str, reason: &str) -> String {
        format!("The profile “{name}” was not deleted: {reason}.")
    }

    fn profile_not_opened(&self, file: &str, reason: &str) -> String {
        format!("The profile file “{file}” cannot be opened: {reason}.")
    }

    fn profiles_not_listed(&self, folder: &str, reason: &str) -> String {
        format!("The folder of profiles “{folder}” cannot be read: {reason}.")
    }

    fn failure(&self, failure: &Failure) -> String {
        match failure {
            Failure::AccessDenied => {
                "access is denied: check the permissions of this file or folder".into()
            }
            Failure::InUse => "the file is open in another program, maybe another copy of \
                revisor: close it and try again"
                .into(),
            Failure::DiskFull => "the disk is full: free some space and try again".into(),
            Failure::NewerVersion => {
                "a newer version of revisor made this profile: update revisor".into()
            }
            Failure::NotAProfile => {
                "this is not a revisor profile file: move it out of the folder of profiles".into()
            }
            Failure::NotSettings => {
                "this is not a revisor settings file: move it out of the data folder".into()
            }
            Failure::Cancelled => "the action was cancelled".into(),
            Failure::SnapshotDeleted => "the snapshot has been deleted meanwhile".into(),
            Failure::Other {
                text,
                code: Some(code),
            } => format!("{text} (code {code})"),
            Failure::Other { text, code: None } => text.clone(),
        }
    }

    fn progress(&self) -> &'static str {
        "Progress"
    }

    fn snapshot_progress(&self, entries: u64, entries_text: &str, folder: &str) -> String {
        let entries = self.entries(entries, entries_text);
        if folder.is_empty() {
            entries
        } else {
            format!("{entries}, reading {folder}")
        }
    }

    fn snapshot_saving(&self, entries: u64, entries_text: &str) -> String {
        format!(
            "Saving the snapshot: {}…",
            self.entries(entries, entries_text)
        )
    }

    fn snapshot_started(&self) -> &'static str {
        "Snapshot started."
    }

    fn snapshot_saved(&self, entries: u64, entries_text: &str) -> String {
        format!("Snapshot saved: {}.", self.entries(entries, entries_text))
    }

    fn entries_inaccessible(&self, entries_text: &str) -> String {
        format!("Inaccessible entries: {entries_text}.")
    }

    fn folders_not_found(&self, folders: &[(String, String)]) -> String {
        let named: Vec<String> = folders
            .iter()
            .map(|(path, why)| format!("“{path}” ({why})"))
            .collect();
        format!("Folders not found: {}.", named.join("; "))
    }

    fn folder_missing(&self) -> &'static str {
        "the folder is missing"
    }

    fn folder_volume_not_ntfs(&self, file_system: &str) -> String {
        format!("the folder is on a {file_system} volume, not NTFS")
    }

    fn folder_volume_network(&self) -> &'static str {
        "the folder is on a network share"
    }

    fn folder_is_a_file(&self) -> &'static str {
        "a file stands at this path, not a folder"
    }

    fn folder_unreachable(&self, reason: &str) -> String {
        format!("the folder cannot be read: {reason}")
    }

    fn snapshot_cancelled(&self) -> &'static str {
        "Snapshot cancelled."
    }

    fn snapshot_failed(&self, reason: &str) -> String {
        format!("The snapshot was not saved: {reason}.")
    }

    fn snapshot_stopped(&self) -> &'static str {
        "the snapshot stopped unexpectedly"
    }

    fn restart_question(&self, denied_text: &str) -> String {
        format!(
            "Entries inaccessible because of access rights: {denied_text}. Restart revisor as \
             administrator? It will open this profile again; take a new snapshot then, and \
             this one stays in the list."
        )
    }

    fn restart(&self) -> &'static str {
        "Restart"
    }

    fn close_during_snapshot(&self) -> &'static str {
        "A snapshot is being taken, and closing revisor cancels it. Close revisor?"
    }

    fn restart_during_snapshot(&self) -> &'static str {
        "A snapshot is being taken, and restarting revisor cancels it. Restart as \
         administrator?"
    }

    fn restart_failed(&self, reason: &str) -> String {
        format!("revisor was not restarted: {reason}.")
    }

    fn column_date_time(&self) -> &'static str {
        "Date and time"
    }

    fn column_comment(&self) -> &'static str {
        "Comment"
    }

    fn no_snapshots_yet(&self, key: &str) -> String {
        format!("No snapshots yet. Press {key} to take the first snapshot.")
    }

    fn no_profiles_yet(&self, key: &str) -> String {
        format!("No profiles yet. Press {key} to create the first profile.")
    }

    fn no_profiles_item(&self) -> &'static str {
        "No profiles yet"
    }

    fn comment_of(&self, snapshot: &str) -> String {
        format!("Comment on the snapshot {snapshot}")
    }

    fn comment_saved(&self) -> &'static str {
        "Comment saved."
    }

    fn comment_removed(&self) -> &'static str {
        "Comment removed."
    }

    fn comment_not_saved(&self, reason: &str) -> String {
        format!("The comment was not saved: {reason}.")
    }

    fn delete_snapshot_question(&self, name: &str, comment: Option<&str>) -> String {
        match comment {
            Some(comment) => {
                format!("Delete the snapshot {name} “{comment}”? This cannot be undone.")
            }
            None => format!("Delete the snapshot {name}? This cannot be undone."),
        }
    }

    fn delete_snapshots_question(&self, count: usize) -> String {
        format!("Delete {}? This cannot be undone.", self.snapshots(count))
    }

    fn snapshots_deleted(&self, count: usize, profile: &str) -> String {
        format!(
            "Deleted {}; the database of the profile “{profile}” is being compacted.",
            self.snapshots(count)
        )
    }

    fn snapshots_not_deleted(&self, reason: &str) -> String {
        format!("The snapshots were not deleted: {reason}.")
    }

    fn compacting(&self, profile: &str) -> String {
        format!("Compacting the database of the profile “{profile}”…")
    }

    fn compaction_done(&self, profile: &str, freed: &str) -> String {
        format!("The database of the profile “{profile}” is compacted: {freed} freed.")
    }

    fn compaction_cancelled(&self, profile: &str) -> String {
        format!(
            "Compaction of the database of the profile “{profile}” cancelled. The snapshots \
             are deleted, and the next snapshots will take their space."
        )
    }

    fn compaction_failed(&self, profile: &str, reason: &str) -> String {
        format!(
            "The database of the profile “{profile}” was not compacted: {reason}. The \
             snapshots are deleted, and the next snapshots will take their space."
        )
    }

    fn compaction_stopped(&self) -> &'static str {
        "the compaction stopped unexpectedly"
    }

    fn focus_after_snapshot(&self) -> &'static str {
        "Focus after a snapshot"
    }

    fn focus_option(&self, value: FocusAfterSnapshot) -> &'static str {
        match value {
            FocusAfterSnapshot::IfInList => "If focus is in the list",
            FocusAfterSnapshot::Always => "Always",
            FocusAfterSnapshot::Never => "Never",
        }
    }

    fn confirm_snapshot_deletion(&self) -> &'static str {
        "Confirm snapshot deletion"
    }

    fn settings_saved(&self) -> &'static str {
        "Settings saved."
    }

    fn settings_not_saved(&self, reason: &str) -> String {
        format!("The settings were not saved: {reason}.")
    }

    fn settings_not_opened(&self, file: &str, reason: &str) -> String {
        format!(
            "The settings file “{file}” cannot be opened: {reason}. The default settings are \
             used, and changes to them are not kept."
        )
    }

    fn language(&self) -> &'static str {
        "Language"
    }

    fn language_option(&self, value: Language) -> &'static str {
        match value {
            Language::SameAsWindows => "Same as Windows",
            Language::English => "English",
            Language::Ukrainian => "Українська",
        }
    }

    fn language_changes_after_restart(&self) -> &'static str {
        "The interface language changes once revisor is restarted. Restart it now?"
    }

    fn restart_now(&self) -> &'static str {
        "Restart now"
    }

    fn later(&self) -> &'static str {
        "Later"
    }

    fn comparing(&self) -> &'static str {
        "Comparing snapshots…"
    }

    fn comparing_percent(&self, percent: u8) -> String {
        format!("Comparing snapshots: {percent}%…")
    }

    fn waiting_for_database(&self) -> &'static str {
        "Waiting for the profile database…"
    }

    fn comparison_finished(&self) -> &'static str {
        "Comparison finished."
    }

    fn comparison_cancelled(&self) -> &'static str {
        "Comparison cancelled."
    }

    fn comparison_failed(&self, reason: &str) -> String {
        format!("The snapshots were not compared: {reason}.")
    }

    fn comparison_stopped(&self) -> &'static str {
        "the comparison stopped unexpectedly"
    }

    fn compare_with_label(&self, snapshot: &str) -> String {
        format!("Compare the snapshot {snapshot} with")
    }

    fn compare(&self) -> &'static str {
        "Compare"
    }

    fn focus_after_comparison(&self) -> &'static str {
        "Focus after a comparison"
    }

    fn comparison_focus_option(&self, value: FocusAfterComparison) -> &'static str {
        match value {
            FocusAfterComparison::IfNotMoved => "If focus has not moved",
            FocusAfterComparison::Always => "Always",
            FocusAfterComparison::Never => "Never",
        }
    }

    fn column_change(&self) -> &'static str {
        "Change"
    }

    fn column_path(&self) -> &'static str {
        "Path"
    }

    fn change_kind(&self, kind: Kind) -> &'static str {
        match kind {
            Kind::Added => "Added",
            Kind::Deleted => "Deleted",
            Kind::Modified => "Modified",
            Kind::Inaccessible => "Inaccessible",
        }
    }

    fn demo_done(&self, command: &str) -> String {
        format!("Done: {command}.")
    }

    fn key_space(&self) -> &'static str {
        "Space"
    }
    fn key_up(&self) -> &'static str {
        "Up Arrow"
    }
    fn key_down(&self) -> &'static str {
        "Down Arrow"
    }
    fn key_left(&self) -> &'static str {
        "Left Arrow"
    }
    fn key_right(&self) -> &'static str {
        "Right Arrow"
    }
    fn key_context_menu(&self) -> &'static str {
        "Menu key"
    }
}
