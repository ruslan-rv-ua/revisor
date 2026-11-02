//! English interface strings, the base language.

use revisor_core::exclude::PatternError;

use super::plural::{Category, category};
use super::{Failure, Lang, Strings};
use crate::commands::{CommandId, MenuId, Pane, Reason, SubmenuId};

pub struct En;

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
            Reason::SnapshotNotRunning => "no snapshot is being taken",
            Reason::ComparisonNotRunning => "no comparison is running",
            Reason::CompactionNotRunning => "no compaction is running",
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
        let snapshots = match (snapshots, category(Lang::En, (snapshots as u64).into())) {
            (0, _) => {
                return format!(
                    "Delete the profile “{name}”? It has no snapshots. This cannot be undone."
                );
            }
            (n, Category::One) => format!("{n} snapshot"),
            (n, _) => format!("{n} snapshots"),
        };
        format!("Delete the profile “{name}” and its {snapshots}? This cannot be undone.")
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
            Failure::Other {
                text,
                code: Some(code),
            } => format!("{text} (code {code})"),
            Failure::Other { text, code: None } => text.clone(),
        }
    }

    fn demo_snapshot(&self, n: usize) -> String {
        format!("Demo snapshot {n}")
    }

    fn demo_change(&self, n: usize) -> String {
        format!("Demo change {n}")
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
