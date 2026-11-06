//! English interface strings, the base language.

use revisor_core::compare::{Side, Warning};
use revisor_core::exclude::PatternError;
use revisor_core::scan::EntryKind;
use revisor_core::settings::{
    AttributeFormat, FocusAfterComparison, FocusAfterSnapshot, Language, SizeFormat,
};

use super::plural::{Category, category};
use super::{Failure, Lang, Strings, in_sentence};
use crate::commands::{CommandId, MenuId, Pane, Reason, SubmenuId};
use crate::details::{Attribute, Inaccessibility, Property, Unreadable};
use crate::difference::{
    Column, Counted, Field, InaccessibleIn, PairText, RowChange, SizeChange, SnapshotText,
    SummaryText,
};

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

    /// `noun` with a count: "1 change", "20 changes".
    fn counted(&self, count: &Counted, one: &str, other: &str) -> String {
        match category(Lang::En, count.n.into()) {
            Category::One => format!("{} {one}", count.text),
            _ => format!("{} {other}", count.text),
        }
    }

    /// "the earlier", "the later".
    fn side(&self, side: Side) -> &'static str {
        match side {
            Side::Earlier => "earlier",
            Side::Later => "later",
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
            CommandId::CancelComment => "Cancel comment",
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
            Reason::CommentWaiting => "a comment is still waiting for the profile database",
            Reason::CommentNotWaiting => "no comment is waiting for the profile database",
            Reason::CompactionRunning => {
                "a database is being compacted, and only one goes at a time"
            }
            Reason::CompactionOfThisProfile => "the database of this profile is being compacted",
            Reason::NoSelection => "no snapshot is selected",
            Reason::NoPreviousSnapshot => "there is no previous snapshot",
            Reason::NoOtherSnapshot => "the profile has no other snapshot",
            Reason::AlreadyAdmin => "revisor is already running as administrator",
            Reason::NoChangeShown => "the difference list shows no changes",
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
            "revisor {version}\nLicense: MIT\nhttps://github.com/ruslan-rv-ua/revisor\n\n\
             Third-party components and their licenses:\n"
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
        format!(
            "The folder “{folder}” cannot be added because revisor cannot examine it: {reason}."
        )
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
                "access is denied — check the permissions of this file or folder".into()
            }
            Failure::InUse => "the file is open in another program, maybe another copy of \
                revisor — close it and try again"
                .into(),
            Failure::DiskFull => "the disk is full — free some space and try again".into(),
            Failure::NewerVersion => {
                "a newer version of revisor made this profile — update revisor".into()
            }
            Failure::NotAProfile => {
                "this is not a revisor profile file — move it out of the folder of profiles".into()
            }
            Failure::NotSettings => {
                "this is not a revisor settings file — move it out of the data folder".into()
            }
            Failure::Cancelled => "the action was canceled".into(),
            Failure::SnapshotDeleted => "the snapshot has been deleted meanwhile".into(),
            Failure::Other {
                text,
                code: Some(code),
            } => format!("{} (code {code})", in_sentence(text)),
            Failure::Other { text, code: None } => in_sentence(text),
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
        "Snapshot canceled."
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

    fn comment_cancelled(&self) -> &'static str {
        "Comment canceled."
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
            "Compaction of the database of the profile “{profile}” canceled. The snapshots \
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
        "Comparison canceled."
    }

    fn comparison_failed(&self, reason: &str) -> String {
        format!("The snapshots were not compared: {reason}.")
    }

    fn comparison_stopped(&self) -> &'static str {
        "the comparison stopped unexpectedly"
    }

    fn comment_stopped(&self) -> &'static str {
        "writing the comment stopped unexpectedly"
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

    fn snapshot_saved_comparison_cancelled(&self) -> &'static str {
        "Snapshot saved, comparison canceled."
    }

    fn nothing_to_compare(&self) -> &'static str {
        "The snapshot was not compared: the profile has no other snapshot."
    }

    fn column(&self, column: Column) -> &'static str {
        match column {
            Column::Name => "Name",
            Column::Change => "Change",
            Column::WhatChanged => "What changed",
            Column::Size => "Size",
            Column::DateModified => "Date modified",
            Column::Folder => "Folder",
        }
    }

    fn change(&self, change: RowChange, kind: EntryKind) -> &'static str {
        match (kind, change) {
            (EntryKind::File, RowChange::Added) => "Added",
            (EntryKind::File, RowChange::Deleted) => "Deleted",
            (EntryKind::File, RowChange::Modified) => "Modified",
            (EntryKind::File, RowChange::Replaced) => "Replaced",
            (EntryKind::File, RowChange::Inaccessible) => "Inaccessible",
            (EntryKind::Dir, RowChange::Added) => "Folder added",
            (EntryKind::Dir, RowChange::Deleted) => "Folder deleted",
            (EntryKind::Dir, RowChange::Modified) => "Folder modified",
            (EntryKind::Dir, RowChange::Replaced) => "Folder replaced",
            (EntryKind::Dir, RowChange::Inaccessible) => "Inaccessible folder",
            (EntryKind::Link, RowChange::Added) => "Link added",
            (EntryKind::Link, RowChange::Deleted) => "Link deleted",
            (EntryKind::Link, RowChange::Modified) => "Link modified",
            (EntryKind::Link, RowChange::Replaced) => "Link replaced",
            (EntryKind::Link, RowChange::Inaccessible) => "Inaccessible link",
        }
    }

    fn field(&self, field: Field) -> &'static str {
        match field {
            Field::FileVersion => "file version",
            Field::ProductVersion => "product version",
            Field::LinkTarget => "link target",
            Field::Size => "size",
            Field::DateModified => "date modified",
            Field::DateCreated => "date created",
            Field::Attributes => "attributes",
            Field::NameCase => "case of the name",
        }
    }

    fn inaccessible_in(&self, which: InaccessibleIn) -> &'static str {
        match which {
            InaccessibleIn::Earlier => "in the earlier snapshot",
            InaccessibleIn::Later => "in the later snapshot",
            InaccessibleIn::Both => "in both snapshots",
        }
    }

    fn bytes(&self, bytes: u64, text: &str) -> String {
        match category(Lang::En, bytes.into()) {
            Category::One => format!("{text} byte"),
            _ => format!("{text} bytes"),
        }
    }

    fn size_unit(&self, power: u32) -> &'static str {
        match power {
            1 => "KB",
            2 => "MB",
            3 => "GB",
            4 => "TB",
            5 => "PB",
            _ => "EB",
        }
    }

    fn pair(&self, pair: &PairText) -> String {
        let snapshot = |snapshot: &SnapshotText, last: bool| {
            let mut text = snapshot.name.clone();
            if snapshot.deleted {
                text.push_str(" (snapshot deleted)");
            }
            if let Some(comment) = &snapshot.comment {
                text.push_str(&format!(", \u{201c}{comment}\u{201d}"));
                if !last {
                    text.push(',');
                }
            }
            text
        };
        let mut profile = pair.profile.clone();
        if pair.profile_deleted {
            profile.push_str(" (profile deleted)");
        }
        format!(
            "{profile}: from {} to {}",
            snapshot(&pair.earlier, false),
            snapshot(&pair.later, true)
        )
    }

    fn summary(&self, summary: &SummaryText) -> String {
        let t = summary;
        let mut text = if t.changes.n == 0 {
            "No changes.".to_string()
        } else {
            let mut parts = Vec::new();
            if t.added.n > 0 {
                parts.push(format!("{} added", t.added.text));
            }
            if t.deleted.n > 0 {
                parts.push(format!("{} deleted", t.deleted.text));
            }
            if t.modified.n > 0 {
                let mut modified = format!("{} modified", t.modified.text);
                if t.replaced.n > 0 {
                    modified.push_str(&format!(", {} of them replaced", t.replaced.text));
                }
                parts.push(modified);
            }
            format!(
                "{}: {}.",
                self.counted(&t.changes, "change", "changes"),
                parts.join(", ")
            )
        };
        if t.inaccessible.n > 0 {
            text.push_str(&format!(" {} inaccessible.", t.inaccessible.text));
        }
        if t.changes.n > 0 {
            text.push_str(&match &t.size {
                SizeChange::Increased(size) => format!(" Size increased by {size}."),
                SizeChange::Decreased(size) => format!(" Size decreased by {size}."),
                SizeChange::Unchanged => " Size did not change.".to_string(),
            });
        }
        if t.warnings.n > 0 {
            text.push_str(&format!(
                " {}.",
                self.counted(&t.warnings, "warning", "warnings")
            ));
        }
        text
    }

    fn showing(&self, shown_text: &str, total_text: &str) -> String {
        format!("Showing {shown_text} of {total_text}.")
    }

    fn warning(&self, warning: &Warning, reason: &str) -> String {
        match warning {
            Warning::FolderOnlyIn { path, side } => format!(
                "The folder \u{201c}{}\u{201d} is only in the {} snapshot, so it was not compared.",
                path.display(),
                self.side(*side)
            ),
            Warning::FolderNarrowed {
                wider,
                side,
                compared,
            } => format!(
                "In the {} snapshot the folder \u{201c}{}\u{201d} is wider, so only its part \u{201c}{}\u{201d}, which both snapshots have, was compared.",
                self.side(*side),
                wider.display(),
                compared.display()
            ),
            Warning::FolderMissing {
                path,
                side: Side::Earlier,
            } => format!(
                "The folder \u{201c}{}\u{201d} was missing in the earlier snapshot, so its contents show as added.",
                path.display()
            ),
            Warning::FolderMissing {
                path,
                side: Side::Later,
            } => format!(
                "The folder \u{201c}{}\u{201d} is missing in the later snapshot, so its contents show as deleted.",
                path.display()
            ),
            Warning::FolderUnavailable { path, side, .. } => format!(
                "The folder \u{201c}{}\u{201d} could not be read in the {} snapshot ({reason}), so its contents were not compared.",
                path.display(),
                self.side(*side)
            ),
            Warning::DifferentVolumes { path, .. } => format!(
                "The folder \u{201c}{}\u{201d} lies on different volumes in the two snapshots, so it may have many changes.",
                path.display()
            ),
            Warning::ExclusionOnlyIn { pattern, side } => format!(
                "The exclusion \u{201c}{pattern}\u{201d} is only in the {} snapshot, and it was applied to both.",
                self.side(*side)
            ),
            Warning::ExclusionAppliedDifferently {
                pattern,
                earlier,
                later,
            } => {
                let applied = |applied: &Option<String>| match applied {
                    Some(applied) => format!("meant \u{201c}{applied}\u{201d}"),
                    None => "did not apply".to_string(),
                };
                format!(
                    "The exclusion \u{201c}{pattern}\u{201d} {} in the earlier snapshot and {} in the later one; both were applied.",
                    applied(earlier),
                    applied(later)
                )
            }
        }
    }

    fn no_difference_yet(&self) -> &'static str {
        "No difference yet"
    }

    fn no_match(&self) -> &'static str {
        "No changes match the filter"
    }

    fn nothing_compared_yet(&self, quick: &str, choose: &str) -> String {
        format!(
            "Nothing compared yet. Press {quick} to compare the current snapshot with the \
             previous one, or {choose} to choose the snapshot to compare it with."
        )
    }

    fn comparison_field(&self) -> &'static str {
        "Comparison"
    }

    fn filter_field(&self) -> &'static str {
        "Filter"
    }

    fn size_format(&self) -> &'static str {
        "Size"
    }

    fn size_option(&self, value: SizeFormat) -> &'static str {
        match value {
            SizeFormat::Explorer => "Like File Explorer",
            SizeFormat::Decimal => "Decimal",
            SizeFormat::Bytes => "In bytes",
        }
    }

    fn property(&self, property: Property) -> &'static str {
        match property {
            Property::Path => "Path",
            Property::Name => "Name",
            Property::Change => "Change",
            Property::Kind => "Kind",
            Property::Size => "Size",
            Property::DateCreated => "Date created",
            Property::DateModified => "Date modified",
            Property::Attributes => "Attributes",
            Property::FileVersion => "File version",
            Property::ProductVersion => "Product version",
            Property::Target => "Target",
            Property::Inaccessible => "Inaccessible",
        }
    }

    fn before_after(&self, before: &str, after: &str) -> String {
        format!("before {before}, after {after}")
    }

    fn unchanged(&self, value: &str) -> String {
        format!("{value} (unchanged)")
    }

    fn not_counted(&self, text: &str) -> String {
        format!("{text} (ignored)")
    }

    fn change_type(&self, change: RowChange) -> &'static str {
        match change {
            RowChange::Added => "added",
            RowChange::Deleted => "deleted",
            RowChange::Modified => "modified",
            RowChange::Replaced => "replaced",
            RowChange::Inaccessible => "inaccessible",
        }
    }

    fn what_changed(&self, fields: &str) -> String {
        format!("what changed: {fields}")
    }

    fn kind(&self, kind: EntryKind) -> &'static str {
        match kind {
            EntryKind::File => "file",
            EntryKind::Dir => "folder",
            EntryKind::Link => "link",
        }
    }

    fn attribute(&self, attribute: Attribute) -> &'static str {
        match attribute {
            Attribute::ReadOnly => "read-only",
            Attribute::Hidden => "hidden",
            Attribute::System => "system",
            Attribute::Archive => "archive",
            Attribute::Temporary => "temporary",
            Attribute::SparseFile => "sparse",
            Attribute::ReparsePoint => "reparse point",
            Attribute::Compressed => "compressed",
            Attribute::Offline => "data not on the device",
            Attribute::NotContentIndexed => "content not indexed",
            Attribute::Encrypted => "encrypted",
            Attribute::IntegrityStream => "with integrity checking",
            Attribute::NoScrubData => "no background integrity scan",
            Attribute::RecallOnOpen => "downloaded when opened",
            Attribute::Pinned => "always on this device",
            Attribute::Unpinned => "unpinned",
            Attribute::RecallOnDataAccess => "online-only",
            Attribute::StrictlySequential => "strictly sequential",
        }
    }

    fn no_attributes(&self) -> &'static str {
        "no attributes"
    }

    fn version_not_read(&self) -> &'static str {
        "version not read"
    }

    fn no_version(&self) -> &'static str {
        "none"
    }

    fn target_unknown(&self) -> &'static str {
        "unknown"
    }

    fn inaccessible_details(&self, inaccessible: &Inaccessibility) -> String {
        let why = match inaccessible.why {
            Unreadable::Folder => "the contents of the folder could not be read",
            Unreadable::LinkTarget => "the target of the link could not be read",
            Unreadable::Unknown => {
                "whether it was there is unknown: a folder on the way to it could not be read \
                 or is a link"
            }
        };
        let mut text = format!("{} — {why}", self.inaccessible_in(inaccessible.which));
        if let Some(side) = inaccessible.absent_in {
            text.push_str(&format!(
                "; the {} snapshot does not have it",
                self.side(side)
            ));
        }
        text.push('.');
        text
    }

    fn attribute_format(&self) -> &'static str {
        "Attributes"
    }

    fn attribute_option(&self, value: AttributeFormat) -> &'static str {
        match value {
            AttributeFormat::Words => "As words",
            AttributeFormat::Letters => "As letters, like File Explorer",
            AttributeFormat::LettersWithCommas => "As letters with commas",
        }
    }

    fn warn_missing_entry(&self) -> &'static str {
        "Warn when the entry is no longer on disk"
    }

    fn entry_gone_question(&self) -> &'static str {
        "This entry is no longer on disk. Open the folder where it was?"
    }

    fn open(&self) -> &'static str {
        "Open"
    }

    fn no_folder_on_disk(&self, path: &str) -> String {
        format!("no folder of the path “{path}” is on disk")
    }

    fn explorer_failed(&self, reason: &str) -> String {
        format!("File Explorer did not open: {reason}.")
    }

    fn key_space(&self) -> &'static str {
        "Spacebar"
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
