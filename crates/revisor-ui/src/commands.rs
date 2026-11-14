//! The identities of the window's commands, menus and panes (ADR-0008), and why a command
//! cannot run now: the strings name them (`Strings::command`, `menu`, `submenu`, `pane`,
//! `reason`), so they live beside the strings (TASK-47). The table itself — the keys, the
//! places in the menus, whether a command can run now — is the `commands` module of the
//! application.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CommandId {
    NewProfile,
    EditProfile,
    DeleteProfile,
    Settings,
    RestartAsAdmin,
    Exit,
    TakeSnapshot,
    SnapshotAndCompare,
    Comment,
    DeleteSnapshots,
    CancelSnapshot,
    CancelCompaction,
    CancelComment,
    CompareWithPrevious,
    CompareWith,
    CancelComparison,
    Details,
    ShowInExplorer,
    CopyPath,
    FilterByText,
    ShowAdded,
    ShowDeleted,
    ShowModified,
    ShowMoved,
    ShowInaccessible,
    SortByPath,
    SortByName,
    SortByChange,
    SortByWhatChanged,
    SortBySize,
    SortByDateModified,
    SortByFolder,
    SortAscending,
    SortDescending,
    ViewAsList,
    ViewAsTree,
    Export,
    CancelExport,
    KeyboardShortcuts,
    About,
}

/// A menu of the menu bar, in menu bar order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MenuId {
    File,
    Snapshot,
    Compare,
    Difference,
    Help,
}

impl MenuId {
    pub const ALL: [MenuId; 5] = [
        Self::File,
        Self::Snapshot,
        Self::Compare,
        Self::Difference,
        Self::Help,
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SubmenuId {
    SortBy,
}

impl SubmenuId {
    pub const ALL: [SubmenuId; 1] = [Self::SortBy];
}

/// One of the two parts of the main window, named after its main control: the snapshot
/// list and the difference list. F6 moves between them, and a key can be limited to one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pane {
    Snapshots,
    Difference,
}

/// Why a command cannot run now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    NoProfiles,
    NoSnapshots,
    NoDifference,
    /// A snapshot is being taken, and only one goes at a time.
    SnapshotRunning,
    /// A snapshot of the open profile is being taken, so the profile cannot change.
    SnapshotOfThisProfile,
    SnapshotNotRunning,
    ComparisonNotRunning,
    CompactionNotRunning,
    /// A comment waits for the database of its profile: only one goes at a time, and the
    /// profile cannot change meanwhile (TASK-24).
    CommentWaiting,
    CommentNotWaiting,
    /// A profile's database is being compacted, and only one compaction goes at a time
    /// (TASK-9).
    CompactionRunning,
    /// The open profile's database is being compacted, which closes it to everything
    /// (TASK-9).
    CompactionOfThisProfile,
    /// No snapshot is selected in the snapshot list, so there is nothing to delete.
    NoSelection,
    /// The current snapshot is the oldest: there is none before it to compare it with
    /// (TASK-11).
    NoPreviousSnapshot,
    /// The profile has one snapshot only: there is no other one to compare it with
    /// (TASK-11).
    NoOtherSnapshot,
    /// revisor already runs as administrator.
    AlreadyAdmin,
    /// The difference list shows no row, so there is no change to act on (TASK-14).
    NoChangeShown,
    /// The difference shows as a tree, whose nodes are always in the order of their names
    /// (TASK-33 #5).
    TreeSortedByName,
    /// The focus of the difference tree is on a node «Ще…», which stands for the nodes of
    /// a page not shown and is no file or folder (TASK-37 #7).
    MoreNode,
    /// The difference is being exported, and one export goes at a time (TASK-39).
    ExportRunning,
    ExportNotRunning,
}
