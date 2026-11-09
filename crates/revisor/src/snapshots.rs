//! What the snapshot list decides on its own, apart from the window (TASK-9): where the
//! focus and the current snapshot go when a snapshot taken in the background ends — on
//! its own or as part of «Знімок + порівняння» (TASK-12). The list itself is
//! [`crate::snapshot_list`] (TASK-27).

use revisor_core::settings::FocusAfterSnapshot;

use crate::commands::Pane;
use crate::keyboard::Focus;

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
/// In the snapshot list or on the progress row, the user waits for the snapshot.
pub fn arrival(setting: FocusAfterSnapshot, focus: Focus, of_open_profile: bool) -> Arrival {
    let waiting = focus.in_list(Pane::Snapshots) || focus.progress;
    match (setting, of_open_profile) {
        (FocusAfterSnapshot::Never, true) => Arrival::Append,
        (FocusAfterSnapshot::IfInList, true) if waiting => Arrival::Focus,
        (FocusAfterSnapshot::IfInList, true) => Arrival::Current,
        // On the progress of a snapshot of another profile the user waits for it too, so
        // the focus follows it there (#19).
        (FocusAfterSnapshot::IfInList, false) if focus.progress => Arrival::OpenProfileAndFocus,
        (FocusAfterSnapshot::Always, true) => Arrival::Focus,
        (FocusAfterSnapshot::Always, false) => Arrival::OpenProfileAndFocus,
        (FocusAfterSnapshot::Never | FocusAfterSnapshot::IfInList, false) => Arrival::Nothing,
    }
}

/// What happens to the saved snapshot of «Знімок + порівняння» once the command is over
/// (TASK-12 #6): it is at the end of its profile's list already, and becomes current
/// unless the keyboard focus stays in the list, so that a focus the user put there does
/// not move. `to_difference` says whether the focus goes to the difference; from the
/// progress row, which goes, it comes to the new snapshot. «Фокус після знімка» does not
/// count (#5).
pub fn arrival_after_comparison(focus: Focus, to_difference: bool) -> Arrival {
    if to_difference {
        Arrival::Current
    } else if focus.in_list(Pane::Snapshots) {
        Arrival::Append
    } else if focus.progress {
        Arrival::Focus
    } else {
        Arrival::Current
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyboard::Control;

    // The three places of the focus a snapshot's arrival tells apart.
    const SNAPSHOT_LIST: Focus = Focus::new(Some(Pane::Snapshots), Control::MainList);
    const SNAPSHOT_PROGRESS: Focus = Focus {
        progress: true,
        ..Focus::new(None, Control::TextEntry)
    };
    const ELSEWHERE: Focus = Focus::new(None, Control::Other);

    #[test]
    fn a_new_snapshot_gets_the_focus_when_the_user_waited_for_it() {
        use FocusAfterSnapshot::*;
        assert_eq!(arrival(IfInList, SNAPSHOT_LIST, true), Arrival::Focus);
        assert_eq!(arrival(IfInList, SNAPSHOT_PROGRESS, true), Arrival::Focus);
        assert_eq!(arrival(IfInList, ELSEWHERE, true), Arrival::Current);
        // Of another profile: the focus follows only from the snapshot's own progress;
        // elsewhere nothing moves, and the profile stays.
        assert_eq!(
            arrival(IfInList, SNAPSHOT_PROGRESS, false),
            Arrival::OpenProfileAndFocus
        );
        assert_eq!(arrival(IfInList, SNAPSHOT_LIST, false), Arrival::Nothing);
        assert_eq!(arrival(IfInList, ELSEWHERE, false), Arrival::Nothing);
        for focus in [SNAPSHOT_LIST, SNAPSHOT_PROGRESS, ELSEWHERE] {
            assert_eq!(arrival(Never, focus, false), Arrival::Nothing);
            assert_eq!(arrival(Never, focus, true), Arrival::Append);
            assert_eq!(arrival(Always, focus, true), Arrival::Focus);
            assert_eq!(arrival(Always, focus, false), Arrival::OpenProfileAndFocus);
        }
    }

    #[test]
    fn the_snapshot_of_snapshot_and_compare_becomes_current_unless_the_focus_stays_in_the_list() {
        // The focus goes to the difference: the list keeps the new snapshot current.
        for focus in [SNAPSHOT_LIST, SNAPSHOT_PROGRESS, ELSEWHERE] {
            assert_eq!(arrival_after_comparison(focus, true), Arrival::Current);
        }
        // It stays: in the list it does not move, from the progress it comes to the new
        // snapshot, and elsewhere it stays there.
        assert_eq!(
            arrival_after_comparison(SNAPSHOT_LIST, false),
            Arrival::Append
        );
        assert_eq!(
            arrival_after_comparison(SNAPSHOT_PROGRESS, false),
            Arrival::Focus
        );
        assert_eq!(arrival_after_comparison(ELSEWHERE, false), Arrival::Current);
    }
}
