//! What the main window knows about the work in progress; commands decide from it whether
//! they can run now.

use revisor_core::settings::DifferenceSort;
pub use revisor_core::settings::{ChangeType, SortColumn};

use crate::jobs::Flags;

#[derive(Clone, Debug, Default)]
pub struct AppState {
    /// The open profile; none while there are no profiles.
    pub profile: Option<String>,
    /// How many snapshots the open profile has (TASK-9).
    pub snapshots: usize,
    /// How many of them are selected in the snapshot list; deletion acts on them.
    pub selected_snapshots: usize,
    /// The current snapshot: the row of the snapshot list with the focus, else the newest
    /// one (TASK-9); none while there are no snapshots. Quick compare needs a snapshot
    /// before it (TASK-11).
    pub current_snapshot: Option<usize>,
    /// Whether the difference list shows a difference: of any profile, which need not be
    /// the open one (TASK-13 #6).
    pub has_difference: bool,
    // The flags of the background jobs: the window sets them all at once, with `set_jobs`.
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
    /// Which types of rows the difference list shows, indexed by `ChangeType`, and in
    /// which order (TASK-13): what the check marks of the menu «Різниця» say.
    pub shown_types: [bool; 4],
    pub sort: DifferenceSort,
    /// Debug-only stand-in state (REVISOR_DEMO=1) for checking the keyboard with NVDA
    /// before TASK-14 exists: its commands count as available.
    pub demo: bool,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            shown_types: [true; 4],
            ..Self::default()
        }
    }

    /// The demo state: TASK-14's commands run, on the real profiles, snapshots and
    /// difference.
    pub fn demo() -> Self {
        Self {
            demo: true,
            ..Self::new()
        }
    }

    /// Takes the flags of the background jobs, all five at once, as the jobs give them
    /// for the open profile (ADR-0011).
    pub fn set_jobs(&mut self, flags: Flags) {
        let Flags {
            snapshot_running,
            snapshotting_open_profile,
            comparison_running,
            compaction_running,
            compacting_open_profile,
        } = flags;
        self.snapshot_running = snapshot_running;
        self.snapshotting_open_profile = snapshotting_open_profile;
        self.comparison_running = comparison_running;
        self.compaction_running = compaction_running;
        self.compacting_open_profile = compacting_open_profile;
    }

    pub fn shows(&self, change: ChangeType) -> bool {
        self.shown_types[change as usize]
    }
}
