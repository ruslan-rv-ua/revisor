//! English interface strings, the base language.

use super::{Lang, Strings};
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
            Reason::NoDifference => "there is no difference yet, compare snapshots first",
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
