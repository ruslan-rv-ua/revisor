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
    pub snapshots: usize,
    /// Whether a comparison has produced a difference to show.
    pub has_difference: bool,
    pub snapshot_running: bool,
    pub comparison_running: bool,
    pub compaction_running: bool,
    /// Which change types the filter shows, indexed by `ChangeType`.
    pub shown_types: [bool; 4],
    pub sort: SortColumn,
    pub descending: bool,
    /// Debug-only stand-in state (REVISOR_DEMO=1) for checking the keyboard with NVDA
    /// before TASK-7…14 exist: commands without a state of their own count as available.
    pub demo: bool,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            shown_types: [true; 4],
            ..Self::default()
        }
    }

    /// A profile with snapshots and a difference, so most commands can run.
    pub fn demo() -> Self {
        Self {
            profile: Some("Demo".into()),
            snapshots: 3,
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
