//! What the main window knows about the work in progress; commands decide from it whether
//! they can run now.

/// A difference list column to sort by (TASK-13).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortColumn {
    /// The tree order, which is the default.
    #[default]
    Path,
    Name,
    Change,
    WhatChanged,
    Size,
    DateModified,
    Folder,
}

/// A change type the difference filter can show (TASK-13), in the order of its check boxes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeType {
    Added,
    Deleted,
    Modified,
    Inaccessible,
}

#[derive(Clone, Debug, Default)]
pub struct AppState {
    /// The open profile; none while there are no profiles.
    pub profile: Option<String>,
    /// How many snapshots the open profile has (TASK-9).
    pub snapshots: usize,
    /// How many of them are selected in the snapshot list; deletion acts on them.
    pub selected_snapshots: usize,
    /// Whether a comparison has produced a difference to show.
    pub has_difference: bool,
    pub snapshot_running: bool,
    /// Whether the snapshot being taken is of the open profile, which then cannot be
    /// changed or deleted (TASK-8).
    pub snapshotting_open_profile: bool,
    pub comparison_running: bool,
    pub compaction_running: bool,
    /// Whether the database being compacted is the open profile's, which is then closed
    /// even to reading (TASK-9).
    pub compacting_open_profile: bool,
    /// Whether revisor runs as administrator (TASK-8).
    pub elevated: bool,
    /// Which change types the filter shows, indexed by `ChangeType`.
    pub shown_types: [bool; 4],
    pub sort: SortColumn,
    pub descending: bool,
    /// Debug-only stand-in state (REVISOR_DEMO=1) for checking the keyboard with NVDA
    /// before TASK-11…14 exist: commands without a state of their own count as available.
    /// The profiles and the snapshots are the real ones.
    pub demo: bool,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            shown_types: [true; 4],
            ..Self::default()
        }
    }

    /// A profile with snapshots, one of them selected, and a difference, so most commands
    /// can run; for tests, and for the demo state, whose window then puts the real
    /// profile and snapshots in.
    pub fn demo() -> Self {
        Self {
            profile: Some("Demo".into()),
            snapshots: 3,
            selected_snapshots: 1,
            has_difference: true,
            demo: true,
            ..Self::new()
        }
    }

    pub fn shows(&self, change: ChangeType) -> bool {
        self.shown_types[change as usize]
    }

    pub fn toggle_shown(&mut self, change: ChangeType) {
        self.shown_types[change as usize] ^= true;
    }
}
