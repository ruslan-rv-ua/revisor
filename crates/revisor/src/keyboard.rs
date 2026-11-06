//! What a key pressed in the main window does (ADR-0008). The window's char hook sees
//! every key first and asks `route`; only `Route::Pass` lets the key go on to the focused
//! control and to Windows.

use crate::commands::{COMMANDS, CommandId, Pane, Reason};
use crate::keys::{Key, Shortcut};
use crate::state::AppState;

/// The kind of control that has the focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    /// The main list of a pane: the snapshot list or the difference list.
    MainList,
    /// A text field, which needs Delete, Home, End and the like for itself.
    TextEntry,
    /// The field «Фільтр:» under the difference list: a text field that Esc clears
    /// (TASK-13 #24).
    FilterField,
    Other,
}

impl Control {
    /// Whether the control is a text field, which keeps the keys it edits with.
    fn is_text(self) -> bool {
        matches!(self, Control::TextEntry | Control::FilterField)
    }
}

/// Where the focus is: the control, and the part of the window it belongs to, if any.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Focus {
    pub part: Option<Pane>,
    pub control: Control,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    Run(CommandId),
    /// The command cannot run now; the status bar says why.
    Refuse(CommandId, Reason),
    /// The key works only in this pane's list; the status bar says so.
    OnlyIn(Shortcut, Pane),
    /// F6: move the focus to this pane's main list.
    FocusPane(Pane),
    /// Shift+F10 or the context menu key in a list: open that list's context menu.
    ContextMenu(Pane),
    /// Ctrl+A in the snapshot list: select all its rows, which the list control does not
    /// do by itself (TASK-9 #9). The difference list selects one row only (TASK-13 #19).
    SelectAll(Pane),
    /// Esc in the field «Фільтр:»: clear it, so the filter lets every row through
    /// (TASK-13 #24).
    ClearFilter,
    /// Alt with a label's access key: focus the control the label names.
    FocusLabel(usize),
    /// Not revisor's key: the focused control and Windows get it.
    Pass,
}

/// Routes `key` pressed with the focus at `focus`; `labels` are the access keys of the
/// main window's labels, as uppercase Latin letters.
pub fn route(key: Shortcut, focus: Focus, state: &AppState, labels: &[u8]) -> Route {
    let in_list = |pane| focus.part == Some(pane) && focus.control == Control::MainList;
    let decide =
        |id: CommandId, available: fn(&AppState) -> Result<(), Reason>| match available(state) {
            Ok(()) => Route::Run(id),
            Err(reason) => Route::Refuse(id, reason),
        };

    if key.key == Key::F(6) && !key.ctrl && !key.alt {
        // With two parts, forward and backward both lead to the other one.
        return match focus.part {
            Some(Pane::Snapshots) => Route::FocusPane(Pane::Difference),
            Some(Pane::Difference) | None => Route::FocusPane(Pane::Snapshots),
        };
    }
    let context_menu_key =
        key == Shortcut::new(Key::F(10)).shift() || key == Shortcut::new(Key::ContextMenu);
    if context_menu_key
        && focus.control == Control::MainList
        && let Some(pane) = focus.part
    {
        return Route::ContextMenu(pane);
    }
    if key == Shortcut::new(Key::Letter(b'A')).ctrl()
        && focus.control == Control::MainList
        && focus.part == Some(Pane::Snapshots)
    {
        return Route::SelectAll(Pane::Snapshots);
    }
    if key == Shortcut::new(Key::Esc) && focus.control == Control::FilterField {
        return Route::ClearFilter;
    }
    if let Some(letter) = key.access_letter()
        && let Some(label) = labels.iter().position(|&l| l == letter)
    {
        return Route::FocusLabel(label);
    }
    for c in COMMANDS {
        if c.list_key
            .is_some_and(|(pane, k)| k == key && in_list(pane))
        {
            return decide(c.id, c.available);
        }
        if c.shortcut != Some(key) {
            continue;
        }
        return match c.scope {
            None => decide(c.id, c.available),
            Some(pane) if in_list(pane) => decide(c.id, c.available),
            Some(_) if focus.control.is_text() => Route::Pass,
            Some(pane) => Route::OnlyIn(key, pane),
        };
    }
    Route::Pass
}

#[cfg(test)]
mod tests {
    use super::*;

    const SNAPSHOT_LIST: Focus = Focus {
        part: Some(Pane::Snapshots),
        control: Control::MainList,
    };
    const DIFFERENCE_LIST: Focus = Focus {
        part: Some(Pane::Difference),
        control: Control::MainList,
    };
    const FILTER_FIELD: Focus = Focus {
        part: Some(Pane::Difference),
        control: Control::FilterField,
    };
    const COMPARISON_FIELD: Focus = Focus {
        part: Some(Pane::Difference),
        control: Control::TextEntry,
    };
    const PROFILE_BOX: Focus = Focus {
        part: Some(Pane::Snapshots),
        control: Control::Other,
    };

    fn key(key: Key) -> Shortcut {
        Shortcut::new(key)
    }

    /// A profile with three snapshots, the newest current and selected, and a difference
    /// of two rows shown: every command but the cancels can run.
    fn ready() -> AppState {
        AppState {
            profile: Some("Дім".into()),
            snapshots: 3,
            selected_snapshots: 1,
            current_snapshot: Some(2),
            has_difference: true,
            difference_rows: 2,
            ..AppState::new()
        }
    }

    #[test]
    fn a_shortcut_runs_its_command_or_says_why_not() {
        let f5 = key(Key::F(5));
        assert_eq!(
            route(f5, SNAPSHOT_LIST, &ready(), &[]),
            Route::Run(CommandId::TakeSnapshot)
        );
        assert_eq!(
            route(f5, SNAPSHOT_LIST, &AppState::new(), &[]),
            Route::Refuse(CommandId::TakeSnapshot, Reason::NoProfiles)
        );
        assert_eq!(
            route(f5.shift(), DIFFERENCE_LIST, &ready(), &[]),
            Route::Refuse(CommandId::CancelSnapshot, Reason::SnapshotNotRunning)
        );
        // Shortcuts work from the filter field too: it has no use for them.
        assert_eq!(
            route(key(Key::Digit(1)).ctrl(), FILTER_FIELD, &ready(), &[]),
            Route::Run(CommandId::ShowAdded)
        );
        assert_eq!(
            route(key(Key::F(1)), FILTER_FIELD, &AppState::new(), &[]),
            Route::Run(CommandId::KeyboardShortcuts)
        );
    }

    #[test]
    fn keys_that_are_not_revisors_pass() {
        let state = ready();
        assert_eq!(
            route(key(Key::Letter(b'Q')).ctrl(), SNAPSHOT_LIST, &state, &[]),
            Route::Pass
        );
        assert_eq!(
            route(key(Key::Letter(b'Q')), SNAPSHOT_LIST, &state, &[]),
            Route::Pass
        );
        assert_eq!(
            route(key(Key::Down), SNAPSHOT_LIST, &state, &[]),
            Route::Pass
        );
        assert_eq!(
            route(key(Key::Esc), COMPARISON_FIELD, &state, &[]),
            Route::Pass
        );
    }

    #[test]
    fn a_key_limited_to_one_list_says_where_it_works() {
        let delete = key(Key::Delete);
        let state = ready();
        assert_eq!(
            route(delete, SNAPSHOT_LIST, &state, &[]),
            Route::Run(CommandId::DeleteSnapshots)
        );
        let no_snapshots = AppState {
            snapshots: 0,
            ..ready()
        };
        assert_eq!(
            route(delete, SNAPSHOT_LIST, &no_snapshots, &[]),
            Route::Refuse(CommandId::DeleteSnapshots, Reason::NoSnapshots)
        );
        assert_eq!(
            route(delete, DIFFERENCE_LIST, &state, &[]),
            Route::OnlyIn(delete, Pane::Snapshots)
        );
        assert_eq!(
            route(delete, PROFILE_BOX, &state, &[]),
            Route::OnlyIn(delete, Pane::Snapshots)
        );
        // The filter field deletes characters with it.
        assert_eq!(route(delete, FILTER_FIELD, &state, &[]), Route::Pass);
    }

    #[test]
    fn enter_runs_the_lists_action_and_stays_the_controls_elsewhere() {
        let enter = key(Key::Enter);
        let state = ready();
        assert_eq!(
            route(enter, SNAPSHOT_LIST, &state, &[]),
            Route::Run(CommandId::CompareWithPrevious)
        );
        // Enter in the difference list is «Подробиці» (TASK-14 #1).
        assert_eq!(
            route(enter, DIFFERENCE_LIST, &state, &[]),
            Route::Run(CommandId::Details)
        );
        let no_rows = AppState {
            difference_rows: 0,
            ..ready()
        };
        assert_eq!(
            route(enter, DIFFERENCE_LIST, &no_rows, &[]),
            Route::Refuse(CommandId::Details, Reason::NoChangeShown)
        );
        assert_eq!(
            route(enter, SNAPSHOT_LIST, &AppState::new(), &[]),
            Route::Refuse(CommandId::CompareWithPrevious, Reason::NoProfiles)
        );
        assert_eq!(route(enter, FILTER_FIELD, &state, &[]), Route::Pass);
        assert_eq!(route(enter, PROFILE_BOX, &state, &[]), Route::Pass);
    }

    #[test]
    fn alt_enter_and_ctrl_enter_act_on_the_change_from_anywhere() {
        // «Подробиці» and «Показати в Провіднику» have shortcuts of the whole window, so
        // they work from the field «Фільтр:» too (TASK-14 #14).
        let state = ready();
        for focus in [DIFFERENCE_LIST, FILTER_FIELD, SNAPSHOT_LIST] {
            assert_eq!(
                route(key(Key::Enter).alt(), focus, &state, &[]),
                Route::Run(CommandId::Details)
            );
            assert_eq!(
                route(key(Key::Enter).ctrl(), focus, &state, &[]),
                Route::Run(CommandId::ShowInExplorer)
            );
        }
        assert_eq!(
            route(key(Key::Enter).ctrl(), SNAPSHOT_LIST, &AppState::new(), &[]),
            Route::Refuse(CommandId::ShowInExplorer, Reason::NoDifference)
        );
    }

    #[test]
    fn f6_moves_between_the_two_lists() {
        let state = AppState::new();
        for f6 in [key(Key::F(6)), key(Key::F(6)).shift()] {
            assert_eq!(
                route(f6, SNAPSHOT_LIST, &state, &[]),
                Route::FocusPane(Pane::Difference)
            );
            assert_eq!(
                route(f6, PROFILE_BOX, &state, &[]),
                Route::FocusPane(Pane::Difference)
            );
            assert_eq!(
                route(f6, FILTER_FIELD, &state, &[]),
                Route::FocusPane(Pane::Snapshots)
            );
            let nowhere = Focus {
                part: None,
                control: Control::Other,
            };
            assert_eq!(
                route(f6, nowhere, &state, &[]),
                Route::FocusPane(Pane::Snapshots)
            );
        }
    }

    #[test]
    fn the_context_menu_keys_open_the_lists_menu() {
        let state = AppState::new();
        for menu_key in [key(Key::F(10)).shift(), key(Key::ContextMenu)] {
            assert_eq!(
                route(menu_key, SNAPSHOT_LIST, &state, &[]),
                Route::ContextMenu(Pane::Snapshots)
            );
            assert_eq!(
                route(menu_key, DIFFERENCE_LIST, &state, &[]),
                Route::ContextMenu(Pane::Difference)
            );
            // Text fields and other controls have their own context menus.
            assert_eq!(route(menu_key, FILTER_FIELD, &state, &[]), Route::Pass);
            assert_eq!(route(menu_key, PROFILE_BOX, &state, &[]), Route::Pass);
        }
    }

    #[test]
    fn ctrl_a_selects_all_snapshots_and_stays_the_controls_elsewhere() {
        let ctrl_a = key(Key::Letter(b'A')).ctrl();
        let state = AppState::new();
        assert_eq!(
            route(ctrl_a, SNAPSHOT_LIST, &state, &[]),
            Route::SelectAll(Pane::Snapshots)
        );
        // The difference list selects one change only (TASK-13 #19).
        assert_eq!(route(ctrl_a, DIFFERENCE_LIST, &state, &[]), Route::Pass);
        // Text fields select their text with it; the profile box has no use for it.
        assert_eq!(route(ctrl_a, FILTER_FIELD, &state, &[]), Route::Pass);
        assert_eq!(route(ctrl_a, PROFILE_BOX, &state, &[]), Route::Pass);
    }

    #[test]
    fn the_comparison_field_keeps_the_keys_that_read_select_and_copy_its_text() {
        // The field is read with the arrows and copied (TASK-13 #1).
        let state = ready();
        for shortcut in [
            key(Key::Down),
            key(Key::Up),
            key(Key::Home).ctrl(),
            key(Key::End).shift(),
            key(Key::Down).shift(),
            key(Key::Letter(b'A')).ctrl(),
            key(Key::Letter(b'C')).ctrl(),
        ] {
            assert_eq!(
                route(shortcut, COMPARISON_FIELD, &state, &[]),
                Route::Pass,
                "{shortcut:?}"
            );
        }
    }

    #[test]
    fn the_filter_field_keeps_its_keys_and_esc_clears_it() {
        let state = ready();
        let esc = key(Key::Esc);
        assert_eq!(route(esc, FILTER_FIELD, &state, &[]), Route::ClearFilter);
        // Esc cancels no background job (document «Клавіатура й меню»).
        for focus in [COMPARISON_FIELD, DIFFERENCE_LIST, SNAPSHOT_LIST] {
            assert_eq!(route(esc, focus, &state, &[]), Route::Pass);
        }
        // Ctrl+1…Ctrl+4 and Ctrl+F work from the field too (TASK-13 #30).
        let types = [
            CommandId::ShowAdded,
            CommandId::ShowDeleted,
            CommandId::ShowModified,
            CommandId::ShowInaccessible,
        ];
        for (digit, id) in (1..=4).zip(types) {
            assert_eq!(
                route(key(Key::Digit(digit)).ctrl(), FILTER_FIELD, &state, &[]),
                Route::Run(id)
            );
        }
        let ctrl_f = key(Key::Letter(b'F')).ctrl();
        assert_eq!(
            route(ctrl_f, DIFFERENCE_LIST, &state, &[]),
            Route::Run(CommandId::FilterByText)
        );
        // Without a difference there is nothing to filter, and that is said.
        assert_eq!(
            route(ctrl_f, SNAPSHOT_LIST, &AppState::new(), &[]),
            Route::Refuse(CommandId::FilterByText, Reason::NoDifference)
        );
        // Delete, Home and the like edit the text; F6 leads to the snapshot list.
        for edit in [key(Key::Delete), key(Key::Home), key(Key::End).shift()] {
            assert_eq!(route(edit, FILTER_FIELD, &state, &[]), Route::Pass);
        }
        assert_eq!(
            route(key(Key::F(6)), FILTER_FIELD, &state, &[]),
            Route::FocusPane(Pane::Snapshots)
        );
    }

    #[test]
    fn alt_with_a_labels_letter_focuses_its_control() {
        let state = AppState::new();
        let labels = *b"RL";
        let alt = |letter| key(Key::Letter(letter)).alt();
        assert_eq!(
            route(alt(b'L'), SNAPSHOT_LIST, &state, &labels),
            Route::FocusLabel(1)
        );
        // Menu letters go on to Windows, which opens the menu (or asks WM_MENUCHAR).
        assert_eq!(
            route(alt(b'F'), SNAPSHOT_LIST, &state, &labels),
            Route::Pass
        );
        assert_eq!(
            route(alt(b'R').ctrl(), SNAPSHOT_LIST, &state, &labels),
            Route::Pass
        );
    }
}
