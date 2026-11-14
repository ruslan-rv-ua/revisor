//! Where the focus goes when a difference arrives (TASK-11 #12, #13, #21; TASK-12 #5, #7):
//! what the window does with a comparison that has just ended, under the setting «Фокус
//! після порівняння». The counterpart of `snapshots` for the difference list; in the
//! application, not with the difference in revisor-ui, because it is about the focus of the
//! window (TASK-47).

use revisor_core::settings::FocusAfterComparison;

use crate::commands::Pane;
use crate::keyboard::Focus;

/// What the window does with a difference that has just arrived.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arrival {
    /// The difference takes the place of the one shown; the focus stays where it is, and
    /// the end is said without it.
    Replace,
    /// The difference takes the place of the one shown, and the keyboard focus moves to
    /// its first row: into a new list, which is announced as on coming to it from
    /// elsewhere, also when the focus was in the old one (#21).
    Focus,
    /// The profile of the snapshot of «Знімок + порівняння» is opened in place of another
    /// one, and the difference arrives as with `Focus` (TASK-12 #7).
    OpenProfileAndFocus,
}

/// What happens when a comparison ends under the setting «Фокус після порівняння», with
/// the focus at `focus` (TASK-11 #12, #13, #21).
/// The focus still where it was when the comparison started, or on its progress row,
/// which goes away with it, counts as not moved.
pub fn arrival(setting: FocusAfterComparison, focus: Focus) -> Arrival {
    match setting {
        _ if focus.in_list(Pane::Difference) => Arrival::Focus,
        FocusAfterComparison::Always => Arrival::Focus,
        FocusAfterComparison::Never => Arrival::Replace,
        FocusAfterComparison::IfNotMoved if focus.unmoved || focus.progress => Arrival::Focus,
        FocusAfterComparison::IfNotMoved => Arrival::Replace,
    }
}

/// What happens when the comparison of «Знімок + порівняння» ends (TASK-12 #5, #7): as
/// after any comparison while the snapshot's profile is open, `of_open_profile`, with the
/// focus counted from the command's start. Another profile opened meanwhile has moved the
/// focus, so it goes to the difference only with «Завжди», which opens the snapshot's
/// profile again, or when it is in the difference list already (TASK-11 #21).
pub fn arrival_after_snapshot(
    setting: FocusAfterComparison,
    focus: Focus,
    of_open_profile: bool,
) -> Arrival {
    match setting {
        _ if of_open_profile => arrival(setting, focus),
        FocusAfterComparison::Always => Arrival::OpenProfileAndFocus,
        _ if focus.in_list(Pane::Difference) => Arrival::Focus,
        _ => Arrival::Replace,
    }
}

#[cfg(test)]
mod tests {
    use super::{Arrival, arrival, arrival_after_snapshot};
    use crate::commands::Pane;
    use crate::keyboard::{Control, Focus};
    use revisor_core::settings::FocusAfterComparison::{self, *};

    const DIFFERENCE_LIST: Focus = Focus::new(Some(Pane::Difference), Control::MainList);
    const UNMOVED: Focus = Focus {
        unmoved: true,
        ..Focus::new(Some(Pane::Snapshots), Control::MainList)
    };
    const PROGRESS: Focus = Focus {
        progress: true,
        ..Focus::new(None, Control::Other)
    };
    const ELSEWHERE: Focus = Focus::new(Some(Pane::Snapshots), Control::MainList);
    const SETTINGS: [FocusAfterComparison; 3] = [Never, IfNotMoved, Always];

    #[test]
    fn a_difference_takes_the_focus_as_the_setting_says() {
        for setting in SETTINGS {
            // In the difference list the focus comes to the new list in any case (#21).
            assert_eq!(arrival(setting, DIFFERENCE_LIST), Arrival::Focus);
        }
        for focus in [UNMOVED, PROGRESS, ELSEWHERE] {
            assert_eq!(arrival(Always, focus), Arrival::Focus);
            assert_eq!(arrival(Never, focus), Arrival::Replace);
        }
        assert_eq!(arrival(IfNotMoved, UNMOVED), Arrival::Focus);
        assert_eq!(arrival(IfNotMoved, PROGRESS), Arrival::Focus);
        assert_eq!(arrival(IfNotMoved, ELSEWHERE), Arrival::Replace);
    }

    #[test]
    fn the_difference_of_snapshot_and_compare_opens_its_profile_only_with_always() {
        // Of the open profile: as after any comparison.
        for setting in SETTINGS {
            for focus in [DIFFERENCE_LIST, UNMOVED, PROGRESS, ELSEWHERE] {
                assert_eq!(
                    arrival_after_snapshot(setting, focus, true),
                    arrival(setting, focus)
                );
            }
        }
        // Another profile is open: «Завжди» opens the snapshot's profile again; else the
        // focus goes to the difference only from the difference list.
        for focus in [DIFFERENCE_LIST, UNMOVED, PROGRESS, ELSEWHERE] {
            assert_eq!(
                arrival_after_snapshot(Always, focus, false),
                Arrival::OpenProfileAndFocus
            );
        }
        for setting in [Never, IfNotMoved] {
            assert_eq!(
                arrival_after_snapshot(setting, DIFFERENCE_LIST, false),
                Arrival::Focus
            );
            for focus in [UNMOVED, PROGRESS, ELSEWHERE] {
                assert_eq!(
                    arrival_after_snapshot(setting, focus, false),
                    Arrival::Replace
                );
            }
        }
    }
}
