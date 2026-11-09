//! What the main window knows about the work in progress; commands decide from it whether
//! they can run now.

use revisor_core::settings::DifferenceSort;
pub use revisor_core::settings::{ChangeType, DifferenceView, SortColumn};

use crate::difference::ListState;
use crate::jobs::Flags;
use crate::snapshot_list::Summary;

/// What the state is built from (ADR-0013): the sources of truth of the window.
#[derive(Clone, Debug, Default)]
pub struct Sources {
    /// The name of the open profile; none while there are no profiles.
    pub profile: Option<String>,
    /// What the snapshot list says of itself (TASK-27).
    pub snapshot_list: Summary,
    /// The flags of the background jobs for the open profile (ADR-0011).
    pub jobs: Flags,
    /// What the difference list says of itself (TASK-13).
    pub difference: ListState,
    /// Whether revisor runs as administrator (TASK-8).
    pub elevated: bool,
}

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
    /// How many rows the difference list shows: the details and File Explorer act on the
    /// one with the focus (TASK-14).
    pub difference_rows: usize,
    // The flags of the background jobs, as `Jobs` gives them (ADR-0011).
    pub snapshot_running: bool,
    /// Whether the snapshot being taken is of the open profile, which then cannot be
    /// changed or deleted (TASK-8).
    pub snapshotting_open_profile: bool,
    pub comparison_running: bool,
    pub compaction_running: bool,
    /// Whether the database being compacted is the open profile's, which is then closed
    /// even to reading (TASK-9).
    pub compacting_open_profile: bool,
    /// Whether a comment waits for the database of its profile (TASK-24).
    pub comment_waiting: bool,
    /// Whether the comment waiting is of the open profile, which then cannot be changed
    /// nor its snapshots deleted.
    pub commenting_open_profile: bool,
    /// Whether revisor runs as administrator (TASK-8).
    pub elevated: bool,
    /// Which types of rows the difference list shows, indexed by `ChangeType`, and in
    /// which order (TASK-13): what the check marks of the menu «Різниця» say.
    pub shown_types: [bool; 4],
    pub sort: DifferenceSort,
    /// Whether the difference shows as a list or as a tree (TASK-33).
    pub layout: DifferenceView,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            shown_types: [true; 4],
            ..Self::default()
        }
    }

    /// The state as its sources say it now, built afresh at the end of every window
    /// event (ADR-0013): no one changes it by hand.
    pub fn of(sources: Sources) -> Self {
        let Sources {
            profile,
            snapshot_list,
            jobs,
            difference,
            elevated,
        } = sources;
        let Flags {
            snapshot_running,
            snapshotting_open_profile,
            comparison_running,
            compaction_running,
            compacting_open_profile,
            comment_waiting,
            commenting_open_profile,
        } = jobs;
        Self {
            profile,
            snapshots: snapshot_list.snapshots,
            selected_snapshots: snapshot_list.selected,
            current_snapshot: snapshot_list.current,
            has_difference: difference.has_difference,
            difference_rows: difference.rows,
            snapshot_running,
            snapshotting_open_profile,
            comparison_running,
            compaction_running,
            compacting_open_profile,
            comment_waiting,
            commenting_open_profile,
            elevated,
            shown_types: difference.view.types,
            sort: difference.view.sort,
            layout: difference.view.layout,
        }
    }

    pub fn shows(&self, change: ChangeType) -> bool {
        self.shown_types[change as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::difference::View;
    use revisor_core::settings::DifferenceSort;

    #[test]
    fn the_state_is_what_its_sources_say() {
        let sources = Sources {
            profile: Some("Документи".into()),
            snapshot_list: Summary {
                snapshots: 5,
                selected: 2,
                current: Some(3),
            },
            jobs: Flags {
                snapshot_running: true,
                snapshotting_open_profile: true,
                comparison_running: true,
                compaction_running: true,
                compacting_open_profile: false,
                comment_waiting: true,
                commenting_open_profile: false,
            },
            difference: ListState {
                has_difference: true,
                rows: 42,
                view: View {
                    types: [true, false, true, false],
                    sort: DifferenceSort {
                        column: SortColumn::Size,
                        descending: true,
                    },
                    layout: DifferenceView::Tree,
                },
            },
            elevated: true,
        };
        let state = AppState::of(sources.clone());
        assert_eq!(state.profile.as_deref(), Some("Документи"));
        assert_eq!(
            (
                state.snapshots,
                state.selected_snapshots,
                state.current_snapshot
            ),
            (5, 2, Some(3))
        );
        assert!(state.snapshot_running && state.snapshotting_open_profile);
        assert!(state.comparison_running && state.compaction_running);
        assert!(!state.compacting_open_profile);
        assert!(state.comment_waiting && !state.commenting_open_profile);
        assert!(state.has_difference);
        assert_eq!(state.difference_rows, 42);
        assert_eq!(state.shown_types, [true, false, true, false]);
        assert!(state.shows(ChangeType::Added) && !state.shows(ChangeType::Deleted));
        assert_eq!(state.sort.column, SortColumn::Size);
        assert!(state.sort.descending);
        assert_eq!(state.layout, DifferenceView::Tree);
        assert!(state.elevated);
    }

    #[test]
    fn nothing_open_and_nothing_running_is_the_state_of_a_new_window() {
        let state = AppState::of(Sources::default());
        let new = AppState::new();
        assert_eq!(state.profile, new.profile);
        assert_eq!(state.snapshots, 0);
        assert_eq!(state.current_snapshot, None);
        assert!(!state.has_difference && !state.snapshot_running && !state.elevated);
        // No difference yet: every type shown, by path, as a list, as a new window has it.
        assert_eq!(state.shown_types, new.shown_types);
        assert_eq!(state.sort, new.sort);
        assert_eq!(state.layout, new.layout);
    }
}
