//! What the snapshot list decides on its own, apart from the window (TASK-9): where the
//! focus and the current snapshot go when a snapshot taken in the background ends, and
//! after rows are deleted.

use revisor_core::settings::FocusAfterSnapshot;

/// Where the keyboard focus was when the snapshot ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusPlace {
    /// In the snapshot list.
    SnapshotList,
    /// On the snapshot's progress field or its Cancel button, which go away with it.
    SnapshotProgress,
    Elsewhere,
}

/// What the window does with a snapshot that has just been saved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arrival {
    /// The snapshot is of another profile: it shows when that profile is opened.
    Nothing,
    /// The row goes to the end of the list; the current snapshot and the focus stay.
    Append,
    /// The row goes to the end and becomes current; the keyboard focus stays where it is.
    Current,
    /// The row goes to the end and becomes current, and the keyboard focus moves to it.
    Focus,
    /// The snapshot's profile is opened, with the new snapshot current, and the keyboard
    /// focus moves to it.
    OpenProfileAndFocus,
}

/// What happens when a snapshot ends under the setting «Фокус після знімка», with the
/// focus at `focus`; `of_open_profile` says whether the snapshot is of the open profile.
pub fn arrival(setting: FocusAfterSnapshot, focus: FocusPlace, of_open_profile: bool) -> Arrival {
    let waiting = matches!(
        focus,
        FocusPlace::SnapshotList | FocusPlace::SnapshotProgress
    );
    match (setting, of_open_profile) {
        (FocusAfterSnapshot::Never, true) => Arrival::Append,
        (FocusAfterSnapshot::IfInList, true) if waiting => Arrival::Focus,
        (FocusAfterSnapshot::IfInList, true) => Arrival::Current,
        // On the progress of a snapshot of another profile the user waits for it too, so
        // the focus follows it there (#19).
        (FocusAfterSnapshot::IfInList, false) if focus == FocusPlace::SnapshotProgress => {
            Arrival::OpenProfileAndFocus
        }
        (FocusAfterSnapshot::Always, true) => Arrival::Focus,
        (FocusAfterSnapshot::Always, false) => Arrival::OpenProfileAndFocus,
        (FocusAfterSnapshot::Never | FocusAfterSnapshot::IfInList, false) => Arrival::Nothing,
    }
}

/// The row that takes the focus once the rows `deleted` (in ascending order) are gone
/// from a list of `len` rows: the one that stood right after the last deleted row, or
/// the new last row; `None` when no row is left.
pub fn row_after_deletion(deleted: &[usize], len: usize) -> Option<usize> {
    let remaining = len.checked_sub(deleted.len())?;
    if remaining == 0 {
        return None;
    }
    let last_deleted = *deleted.last()?;
    // The rows before the last deleted one that stay: that many stand before the row
    // that followed it.
    let after = last_deleted + 1 - deleted.len();
    Some(after.min(remaining - 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_snapshot_gets_the_focus_when_the_user_waited_for_it() {
        use FocusAfterSnapshot::*;
        use FocusPlace::*;
        assert_eq!(arrival(IfInList, SnapshotList, true), Arrival::Focus);
        assert_eq!(arrival(IfInList, SnapshotProgress, true), Arrival::Focus);
        assert_eq!(arrival(IfInList, Elsewhere, true), Arrival::Current);
        // Of another profile: the focus follows only from the snapshot's own progress;
        // elsewhere nothing moves, and the profile stays.
        assert_eq!(
            arrival(IfInList, SnapshotProgress, false),
            Arrival::OpenProfileAndFocus
        );
        assert_eq!(arrival(IfInList, SnapshotList, false), Arrival::Nothing);
        assert_eq!(arrival(IfInList, Elsewhere, false), Arrival::Nothing);
        for focus in [SnapshotList, SnapshotProgress, Elsewhere] {
            assert_eq!(arrival(Never, focus, false), Arrival::Nothing);
            assert_eq!(arrival(Never, focus, true), Arrival::Append);
            assert_eq!(arrival(Always, focus, true), Arrival::Focus);
            assert_eq!(arrival(Always, focus, false), Arrival::OpenProfileAndFocus);
        }
    }

    #[test]
    fn after_a_deletion_the_focus_goes_to_the_row_that_followed() {
        // Five rows; the row after the last deleted one, at its new position.
        assert_eq!(row_after_deletion(&[1, 3], 5), Some(2));
        assert_eq!(row_after_deletion(&[0], 5), Some(0));
        assert_eq!(row_after_deletion(&[0, 1, 2], 5), Some(0));
        assert_eq!(row_after_deletion(&[2], 5), Some(2));
        // The last row deleted: the new last row.
        assert_eq!(row_after_deletion(&[4], 5), Some(3));
        assert_eq!(row_after_deletion(&[2, 4], 5), Some(2));
        assert_eq!(row_after_deletion(&[3, 4], 5), Some(2));
        // Nothing left.
        assert_eq!(row_after_deletion(&[0, 1, 2, 3, 4], 5), None);
        assert_eq!(row_after_deletion(&[0], 1), None);
        assert_eq!(row_after_deletion(&[], 0), None);
        assert_eq!(row_after_deletion(&[], 3), None);
    }
}
