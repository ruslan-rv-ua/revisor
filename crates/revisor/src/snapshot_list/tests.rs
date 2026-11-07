use std::cell::{Cell, RefCell};

use super::*;
use crate::i18n::En;

/// A list control in memory: its rows, the selection, the focus rectangle and the
/// keyboard focus, as a list-view keeps them.
#[derive(Default)]
struct Mock {
    rows: RefCell<Vec<(i64, String)>>,
    selected: RefCell<Vec<bool>>,
    focused: Cell<Option<usize>>,
    has_focus: Cell<bool>,
    name: RefCell<String>,
    empty_text: RefCell<String>,
}

impl Control for Mock {
    fn clear(&self) {
        self.rows.borrow_mut().clear();
        self.selected.borrow_mut().clear();
        self.focused.set(None);
    }

    fn insert(&self, index: usize, snapshot: &SnapshotInfo) {
        let comment = snapshot.comment.clone().unwrap_or_default();
        self.rows.borrow_mut().insert(index, (snapshot.id, comment));
        self.selected.borrow_mut().insert(index, false);
    }

    fn set_comment(&self, index: usize, comment: &str) {
        self.rows.borrow_mut()[index].1 = comment.to_string();
    }

    /// As a list-view does, the focus rectangle of a deleted row goes to a neighbour.
    fn delete(&self, index: usize) {
        self.rows.borrow_mut().remove(index);
        self.selected.borrow_mut().remove(index);
        let len = self.rows.borrow().len();
        self.focused.set(match self.focused.get() {
            Some(f) if f > index => Some(f - 1),
            Some(f) if f == index => (len > 0).then(|| index.min(len - 1)),
            other => other,
        });
    }

    fn select_only(&self, row: usize) {
        for (i, selected) in self.selected.borrow_mut().iter_mut().enumerate() {
            *selected = i == row;
        }
        self.focused.set(Some(row));
    }

    fn select_all(&self) {
        self.selected.borrow_mut().fill(true);
    }

    fn focused_row(&self) -> Option<usize> {
        self.focused.get()
    }

    fn selected_rows(&self) -> Vec<usize> {
        let selected = self.selected.borrow();
        selected
            .iter()
            .enumerate()
            .filter_map(|(i, &s)| s.then_some(i))
            .collect()
    }

    fn selected_count(&self) -> usize {
        self.selected.borrow().iter().filter(|&&s| s).count()
    }

    fn unfocus_rows(&self) {
        self.focused.set(None);
    }

    fn focus_row(&self, row: usize) {
        self.focused.set(Some(row));
    }

    fn has_focus(&self) -> bool {
        self.has_focus.get()
    }

    fn set_focus(&self) {
        self.has_focus.set(true);
    }

    fn set_hint(&self, name: &str, empty_text: &str) {
        *self.name.borrow_mut() = name.to_string();
        *self.empty_text.borrow_mut() = empty_text.to_string();
    }
}

fn snapshot(id: i64) -> SnapshotInfo {
    SnapshotInfo {
        id,
        taken_at: 1_700_000_000_000 + id * 1000,
        comment: None,
    }
}

/// A list of a profile with the snapshots `ids`, oldest first.
fn list_of(ids: &[i64]) -> SnapshotList<Mock> {
    let list = SnapshotList::new(Mock::default(), &En);
    list.show(ids.iter().map(|&id| snapshot(id)).collect(), true);
    list
}

/// The ids of the rows the control shows.
fn shown(list: &SnapshotList<Mock>) -> Vec<i64> {
    list.control()
        .rows
        .borrow()
        .iter()
        .map(|(id, _)| *id)
        .collect()
}

/// The ids of the rows the model holds.
fn held(list: &SnapshotList<Mock>) -> Vec<i64> {
    list.rows().iter().map(|row| row.id).collect()
}

#[test]
fn a_profile_shows_its_snapshots_with_the_newest_current() {
    let list = list_of(&[1, 2, 3]);
    assert_eq!(shown(&list), [1, 2, 3]);
    assert_eq!(
        list.summary(),
        Summary {
            snapshots: 3,
            selected: 1,
            current: Some(2),
        }
    );
    // The focus rectangle makes a row current; without it, the newest is.
    list.control().focus_row(0);
    assert_eq!(list.current(), Some(0));
    list.control().unfocus_rows();
    assert_eq!(list.current(), Some(2));
    // Another profile replaces the rows.
    list.show(vec![snapshot(7)], true);
    assert_eq!(shown(&list), [7]);
    assert_eq!(list.summary().current, Some(0));
    list.show(Vec::new(), true);
    assert_eq!(list.summary(), Summary::default());
}

#[test]
fn a_new_snapshot_arrives_as_its_arrival_says() {
    // Nothing: of another profile, the list stays as it is.
    let list = list_of(&[1, 2]);
    list.control().focus_row(0);
    list.arrive(snapshot(3), Arrival::Nothing);
    assert_eq!((shown(&list), held(&list)), (vec![1, 2], vec![1, 2]));
    assert_eq!(list.current(), Some(0));
    assert!(!list.has_focus());

    // Append: a row at the end; the current snapshot and the focus stay.
    let list = list_of(&[1, 2]);
    list.control().select_only(0);
    list.arrive(snapshot(3), Arrival::Append);
    assert_eq!((shown(&list), held(&list)), (vec![1, 2, 3], vec![1, 2, 3]));
    assert_eq!(list.summary().current, Some(0));
    assert!(!list.has_focus());

    // Current: the new row is current; the keyboard focus stays elsewhere.
    let list = list_of(&[1, 2]);
    list.control().select_only(0);
    list.arrive(snapshot(3), Arrival::Current);
    assert_eq!(shown(&list), [1, 2, 3]);
    assert_eq!(
        list.summary(),
        Summary {
            snapshots: 3,
            selected: 1,
            current: Some(2),
        }
    );
    assert!(!list.has_focus());

    // Focus: current, and the keyboard focus comes to the list.
    let list = list_of(&[1, 2]);
    list.control().select_only(0);
    list.arrive(snapshot(3), Arrival::Focus);
    assert_eq!(shown(&list), [1, 2, 3]);
    assert_eq!(list.summary().current, Some(2));
    assert!(list.has_focus());

    // OpenProfileAndFocus: the window has opened the snapshot's profile, whose rows hold
    // it already; it is current, with the focus, and is not added twice.
    let list = list_of(&[1, 2, 3]);
    list.control().select_only(0);
    list.arrive(snapshot(3), Arrival::OpenProfileAndFocus);
    assert_eq!((shown(&list), held(&list)), (vec![1, 2, 3], vec![1, 2, 3]));
    assert_eq!(list.summary().current, Some(2));
    assert!(list.has_focus());
}

#[test]
fn the_first_snapshot_ends_the_hint_of_the_empty_list() {
    let list = list_of(&[]);
    list.arrive(snapshot(1), Arrival::Append);
    assert_eq!(list.control().empty_text.borrow().as_str(), "");
    assert_eq!(list.summary().snapshots, 1);
}

#[test]
fn a_snapshot_is_placed_once_its_comparison_is_over() {
    let list = list_of(&[1, 2, 3]);
    list.control().select_only(0);
    // Append leaves the list as it is.
    assert!(list.place(3, Arrival::Append));
    assert_eq!(list.current(), Some(0));
    assert!(list.place(2, Arrival::Current));
    assert_eq!(list.current(), Some(1));
    assert!(!list.has_focus());
    assert!(list.place(3, Arrival::Focus));
    assert_eq!(list.current(), Some(2));
    assert!(list.has_focus());
    // Deleted meanwhile: nothing changes.
    assert!(!list.place(9, Arrival::Focus));
    assert_eq!(list.current(), Some(2));
}

#[test]
fn a_comment_is_written_in_its_row() {
    let list = list_of(&[1, 2]);
    assert!(list.set_comment(1, Some("before update".into())));
    assert_eq!(list.control().rows.borrow()[0].1, "before update");
    assert_eq!(
        list.row(0).unwrap().comment.as_deref(),
        Some("before update")
    );
    assert!(list.set_comment(1, None));
    assert_eq!(list.control().rows.borrow()[0].1, "");
    assert_eq!(list.row(0).unwrap().comment, None);
    assert!(!list.set_comment(9, Some("x".into())));
}

#[test]
fn no_row_has_the_focus_rectangle_while_the_deletion_is_asked() {
    let list = list_of(&[1, 2, 3, 4, 5]);
    list.control().select_only(1);
    let asked = Cell::new(false);
    let deleted = list.delete(|chosen| {
        asked.set(true);
        assert_eq!(chosen.iter().map(|r| r.id).collect::<Vec<_>>(), [2]);
        assert_eq!(list.control().focused_row(), None);
        true
    });
    assert!(asked.get());
    assert_eq!(deleted.map(|d| d.len()), Some(1));
}

#[test]
fn a_refused_or_failed_deletion_puts_the_focus_rectangle_back() {
    // `decide` is false both when the user refuses and when the database fails.
    let list = list_of(&[1, 2, 3, 4, 5]);
    list.control().select_only(1);
    list.control().selected.borrow_mut()[3] = true;
    list.control().focus_row(3);
    assert!(list.delete(|_| false).is_none());
    assert_eq!(list.control().focused_row(), Some(3));
    assert_eq!(list.control().selected_rows(), [1, 3]);
    assert_eq!(
        (shown(&list), held(&list)),
        (vec![1, 2, 3, 4, 5], vec![1, 2, 3, 4, 5])
    );
    assert_eq!(
        list.summary(),
        Summary {
            snapshots: 5,
            selected: 2,
            current: Some(3),
        }
    );

    // No row had the rectangle: none gets it back.
    list.control().unfocus_rows();
    assert!(list.delete(|_| false).is_none());
    assert_eq!(list.control().focused_row(), None);
}

#[test]
fn nothing_selected_asks_nothing() {
    let list = list_of(&[1, 2]);
    list.control().selected.borrow_mut().fill(false);
    let deleted = list.delete(|_| panic!("nothing to ask about"));
    assert!(deleted.is_none());
}

#[test]
fn after_a_deletion_the_focus_is_on_the_row_that_followed() {
    // One row in the middle.
    let list = list_of(&[1, 2, 3, 4, 5]);
    list.control().select_only(2);
    let deleted = list.delete(|_| true).unwrap();
    assert_eq!(deleted.iter().map(|r| r.id).collect::<Vec<_>>(), [3]);
    assert_eq!(
        (shown(&list), held(&list)),
        (vec![1, 2, 4, 5], vec![1, 2, 4, 5])
    );
    assert_eq!(
        list.current().and_then(|i| list.row(i)).map(|r| r.id),
        Some(4)
    );
    assert_eq!(list.control().selected_rows(), [2]);

    // Several rows apart: the one after the last deleted.
    let list = list_of(&[1, 2, 3, 4, 5]);
    list.control().select_only(1);
    list.control().selected.borrow_mut()[3] = true;
    list.delete(|_| true).unwrap();
    assert_eq!(shown(&list), [1, 3, 5]);
    assert_eq!(
        list.current().and_then(|i| list.row(i)).map(|r| r.id),
        Some(5)
    );
    assert_eq!(
        list.summary(),
        Summary {
            snapshots: 3,
            selected: 1,
            current: Some(2),
        }
    );

    // The last row: the new last row.
    let list = list_of(&[1, 2, 3]);
    list.delete(|_| true).unwrap();
    assert_eq!(shown(&list), [1, 2]);
    assert_eq!(list.current(), Some(1));

    // Every row: none is left, and the list says how to take one.
    let list = list_of(&[1, 2]);
    list.select_all();
    list.delete(|_| true).unwrap();
    assert!(shown(&list).is_empty() && held(&list).is_empty());
    assert_eq!(list.summary(), Summary::default());
    assert!(!list.control().empty_text.borrow().is_empty());
}

#[test]
fn the_empty_list_says_what_is_missing() {
    let s: &dyn Strings = &En;
    let key = |id| command(id).shortcut.unwrap().name(s);
    let pane = s.pane(Pane::Snapshots);

    // No profiles: a new one first.
    let list = SnapshotList::new(Mock::default(), &En);
    let hint = s.no_profiles_yet(&key(CommandId::NewProfile));
    assert_eq!(*list.control().empty_text.borrow(), hint);
    assert_eq!(*list.control().name.borrow(), format!("{pane}. {hint}"));
    list.show(Vec::new(), false);
    assert_eq!(*list.control().empty_text.borrow(), hint);

    // A profile without snapshots: the first snapshot.
    list.show(Vec::new(), true);
    let hint = s.no_snapshots_yet(&key(CommandId::TakeSnapshot));
    assert_eq!(*list.control().empty_text.borrow(), hint);
    assert_eq!(*list.control().name.borrow(), format!("{pane}. {hint}"));

    // Snapshots: the list is named after its pane alone.
    list.show(vec![snapshot(1)], true);
    assert_eq!(list.control().empty_text.borrow().as_str(), "");
    assert_eq!(*list.control().name.borrow(), pane);
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
