//! The snapshot list of the open profile (TASK-27): its rows, the current and the selected
//! snapshots, the arrival of a new snapshot, the deletion of the selected ones with the
//! focus rectangle out of the way, and what the list says while it is empty. It reaches
//! its control only through [`Control`]: [`Native`] over a `ListCtrl` and `win32` in the
//! windows, a mock in the tests. The «Порівняти з…» window fills its list with [`Native`]
//! too, so a snapshot's row looks the same in both.
//!
//! The rows shown are given to [`SnapshotList::show`]: from the database, or those a
//! compaction keeps while the database is closed (ADR-0011); the list does not know which.

use std::cell::{Cell, RefCell};
use std::ffi::c_void;

use revisor_core::store::SnapshotInfo;
use wxdragon::prelude::*;

use crate::commands::{CommandId, Pane, command};
use crate::i18n::Strings;
use crate::snapshots::Arrival;
use crate::win32;

/// The widths of the snapshot list's columns, in both windows: the name is fixed, the
/// comment takes the rest.
const DATE_TIME_WIDTH: i32 = 190;
const COMMENT_WIDTH: i32 = 420;

/// What the list needs of its control. The rows are positions in the list, top to bottom.
pub trait Control {
    /// Takes every row out.
    fn clear(&self);
    /// Puts the row of `snapshot` at `index`.
    fn insert(&self, index: usize, snapshot: &SnapshotInfo);
    /// Writes `comment` in the row `index`.
    fn set_comment(&self, index: usize, comment: &str);
    fn delete(&self, index: usize);
    /// Makes `row` the one selected row, with the focus rectangle; the keyboard focus
    /// stays where it is.
    fn select_only(&self, row: usize);
    fn select_all(&self);
    /// The row with the focus rectangle.
    fn focused_row(&self) -> Option<usize>;
    /// The selected rows, top to bottom.
    fn selected_rows(&self) -> Vec<usize>;
    /// How many rows are selected.
    fn selected_count(&self) -> usize;
    /// Takes the focus rectangle off every row; the selection stays.
    fn unfocus_rows(&self);
    /// Puts the focus rectangle on `row`; the selection stays.
    fn focus_row(&self, row: usize);
    /// Whether the list has the keyboard focus.
    fn has_focus(&self) -> bool;
    /// Moves the keyboard focus to the list.
    fn set_focus(&self);
    /// Gives the list the name screen readers announce and the text its area shows while
    /// it has no rows.
    fn set_hint(&self, name: &str, empty_text: &str);
}

/// The control of a snapshot list in a window: a report `ListCtrl` with the columns
/// «Дата й час» and «Коментар», which selects whole rows and does not sort.
pub struct Native {
    list: ListCtrl,
}

impl Native {
    /// Makes the list in `parent`; `single` lets one row at most be selected.
    pub fn new(parent: &dyn WxWidget, s: &dyn Strings, single: bool) -> Self {
        let style = if single {
            ListCtrlStyle::Report | ListCtrlStyle::SingleSel
        } else {
            ListCtrlStyle::Report
        };
        let list = ListCtrl::builder(parent).with_style(style).build();
        list.insert_column(
            0,
            s.column_date_time(),
            ListColumnFormat::Left,
            DATE_TIME_WIDTH,
        );
        list.insert_column(1, s.column_comment(), ListColumnFormat::Left, COMMENT_WIDTH);
        win32::full_row_select_no_sort(list.get_handle());
        Self { list }
    }

    /// The control, for the window's sizers and events.
    pub fn list(&self) -> &ListCtrl {
        &self.list
    }

    fn hwnd(&self) -> *mut c_void {
        self.list.get_handle()
    }

    /// Where a context menu opened from the keyboard goes: by the row with the focus
    /// rectangle.
    pub fn context_menu_point(&self) -> (i32, i32) {
        win32::context_menu_point(self.hwnd())
    }

    /// Calls `changed` whenever a row is selected, deselected or gets the focus
    /// rectangle: what the window's state knows of the list may have changed.
    pub fn on_change(&self, changed: impl Fn() + Clone + 'static) {
        let f = changed.clone();
        self.list.on_item_selected(move |_| f());
        let f = changed.clone();
        self.list.on_item_deselected(move |_| f());
        self.list.on_item_focused(move |_| changed());
    }
}

impl Control for Native {
    fn clear(&self) {
        self.list.delete_all_items();
    }

    /// A snapshot's row: its name — the moment it was taken — then its comment.
    fn insert(&self, index: usize, snapshot: &SnapshotInfo) {
        self.list.insert_item(
            index as i64,
            &win32::format_date_time(snapshot.taken_at),
            None,
        );
        if let Some(comment) = &snapshot.comment {
            self.set_comment(index, comment);
        }
    }

    fn set_comment(&self, index: usize, comment: &str) {
        self.list.set_item_text_by_column(index as i64, 1, comment);
    }

    fn delete(&self, index: usize) {
        self.list.delete_item(index as i64);
    }

    fn select_only(&self, row: usize) {
        win32::select_only(self.hwnd(), row);
    }

    fn select_all(&self) {
        win32::select_all(self.hwnd());
    }

    fn focused_row(&self) -> Option<usize> {
        win32::focused_row(self.hwnd())
    }

    fn selected_rows(&self) -> Vec<usize> {
        win32::selected_rows(self.hwnd())
    }

    fn selected_count(&self) -> usize {
        win32::selected_count(self.hwnd())
    }

    fn unfocus_rows(&self) {
        win32::unfocus_rows(self.hwnd());
    }

    fn focus_row(&self, row: usize) {
        win32::focus_row(self.hwnd(), row);
    }

    fn has_focus(&self) -> bool {
        win32::focused_window() == self.hwnd()
    }

    fn set_focus(&self) {
        self.list.set_focus();
    }

    fn set_hint(&self, name: &str, empty_text: &str) {
        win32::set_accessible_name(self.hwnd(), name);
        win32::set_empty_text(self.hwnd(), empty_text);
    }
}

/// What the window's state knows of the list (`AppState::of`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    /// How many snapshots the list shows.
    pub snapshots: usize,
    /// How many of them are selected; deletion acts on them.
    pub selected: usize,
    /// The current snapshot: the row with the focus rectangle, else the newest one; none
    /// while there are no snapshots.
    pub current: Option<usize>,
}

/// The snapshot list of the open profile, oldest first, over its control `C`.
///
/// The control's events come while its calls run — a row selected sends its event
/// before `select_only` returns — and the window then asks for the [`summary`]; so no
/// borrow of the rows is held across a call to the control.
///
/// [`summary`]: SnapshotList::summary
pub struct SnapshotList<C> {
    control: C,
    s: &'static dyn Strings,
    rows: RefCell<Vec<SnapshotInfo>>,
    /// Whether there are profiles at all: the empty list says which is missing.
    any_profiles: Cell<bool>,
}

impl<C: Control> SnapshotList<C> {
    /// An empty list, of no profile; its name is set at once.
    pub fn new(control: C, s: &'static dyn Strings) -> Self {
        let list = Self {
            control,
            s,
            rows: RefCell::new(Vec::new()),
            any_profiles: Cell::new(false),
        };
        list.update_hint();
        list
    }

    pub fn control(&self) -> &C {
        &self.control
    }

    /// Shows `rows`, the open profile's snapshots oldest first, with the newest current;
    /// `any_profiles` says whether there are profiles, for what an empty list says.
    pub fn show(&self, rows: Vec<SnapshotInfo>, any_profiles: bool) {
        self.any_profiles.set(any_profiles);
        let count = rows.len();
        *self.rows.borrow_mut() = rows.clone();
        self.control.clear();
        for (i, row) in rows.iter().enumerate() {
            self.control.insert(i, row);
        }
        if let Some(newest) = count.checked_sub(1) {
            self.control.select_only(newest);
        }
        self.update_hint();
    }

    /// The rows, oldest first.
    pub fn rows(&self) -> Vec<SnapshotInfo> {
        self.rows.borrow().clone()
    }

    /// The row at `index`.
    pub fn row(&self, index: usize) -> Option<SnapshotInfo> {
        self.rows.borrow().get(index).cloned()
    }

    /// The newest snapshot.
    pub fn newest(&self) -> Option<SnapshotInfo> {
        self.rows.borrow().last().cloned()
    }

    /// The position of the current snapshot: the row with the focus rectangle, or the
    /// newest one when no row has it.
    pub fn current(&self) -> Option<usize> {
        let count = self.rows.borrow().len();
        self.control
            .focused_row()
            .filter(|&row| row < count)
            .or_else(|| count.checked_sub(1))
    }

    pub fn summary(&self) -> Summary {
        Summary {
            snapshots: self.rows.borrow().len(),
            selected: self.control.selected_count(),
            current: self.current(),
        }
    }

    pub fn has_focus(&self) -> bool {
        self.control.has_focus()
    }

    pub fn set_focus(&self) {
        self.control.set_focus();
    }

    pub fn select_all(&self) {
        self.control.select_all();
    }

    /// A saved snapshot `row` of the open profile, as its `arrival` says: nothing; a row
    /// at the end; that row current; or current with the keyboard focus. With
    /// `OpenProfileAndFocus` the window has opened the snapshot's profile, whose rows hold
    /// the snapshot already: it becomes current, with the keyboard focus.
    pub fn arrive(&self, row: SnapshotInfo, arrival: Arrival) {
        match arrival {
            Arrival::Nothing => {}
            Arrival::Append | Arrival::Current | Arrival::Focus => {
                let index = {
                    let mut rows = self.rows.borrow_mut();
                    rows.push(row.clone());
                    rows.len() - 1
                };
                self.control.insert(index, &row);
                self.place_at(index, arrival);
                self.update_hint();
            }
            Arrival::OpenProfileAndFocus => {
                // Opened with its newest snapshot current, which this one is, as a rule.
                let current = self.current().and_then(|i| self.row(i)).map(|r| r.id);
                if current == Some(row.id) || !self.place(row.id, Arrival::Focus) {
                    self.control.set_focus();
                }
            }
        }
    }

    /// Makes the snapshot `id` current as `arrival` says — `Current`, or `Focus` with the
    /// keyboard focus moving to it; the others leave the list as it is. Returns whether
    /// the list has the snapshot.
    pub fn place(&self, id: i64, arrival: Arrival) -> bool {
        let index = self.rows.borrow().iter().position(|row| row.id == id);
        match index {
            Some(index) => {
                self.place_at(index, arrival);
                true
            }
            None => false,
        }
    }

    fn place_at(&self, index: usize, arrival: Arrival) {
        if matches!(arrival, Arrival::Current | Arrival::Focus) {
            self.control.select_only(index);
        }
        if arrival == Arrival::Focus {
            self.control.set_focus();
        }
    }

    /// Writes the comment of the snapshot `id`, or takes it off; returns whether the list
    /// has the snapshot.
    pub fn set_comment(&self, id: i64, comment: Option<String>) -> bool {
        let index = self.rows.borrow().iter().position(|row| row.id == id);
        let Some(index) = index else {
            return false;
        };
        self.control
            .set_comment(index, comment.as_deref().unwrap_or_default());
        self.rows.borrow_mut()[index].comment = comment;
        true
    }

    /// Deletes the selected snapshots once `decide`, given them, has asked and deleted them
    /// from the database, and returns them; `decide` returns false when the user refused
    /// or the deletion failed, and nothing is selected returns `None` too.
    ///
    /// While `decide` runs no row has the focus rectangle: when its question closes, the
    /// focus comes back to the list, and a screen reader would announce the focused row
    /// just as it is deleted. A refusal or a failure puts the rectangle back on the same
    /// row; after the deletion it goes to the row that followed the deleted ones (TASK-9
    /// #26).
    pub fn delete(
        &self,
        decide: impl FnOnce(&[SnapshotInfo]) -> bool,
    ) -> Option<Vec<SnapshotInfo>> {
        let selected = self.control.selected_rows();
        let chosen: Vec<SnapshotInfo> = {
            let rows = self.rows.borrow();
            selected
                .iter()
                .filter_map(|&i| rows.get(i).cloned())
                .collect()
        };
        if chosen.is_empty() {
            return None;
        }
        let focused = self.control.focused_row();
        self.control.unfocus_rows();
        if !decide(&chosen) {
            if let Some(row) = focused {
                self.control.focus_row(row);
            }
            return None;
        }
        // From the bottom up, so the positions hold.
        let count = self.rows.borrow().len();
        for &i in selected.iter().rev() {
            self.rows.borrow_mut().remove(i);
            self.control.delete(i);
        }
        if let Some(next) = row_after_deletion(&selected, count) {
            self.control.select_only(next);
        }
        self.update_hint();
        Some(chosen)
    }

    /// What the empty list says, in its area and in its name for screen readers: that
    /// there are no profiles, or no snapshots, and which key makes the first one.
    fn update_hint(&self) {
        let s = self.s;
        let key = |id| command(id).shortcut.map(|k| k.name(s)).unwrap_or_default();
        let hint = if !self.any_profiles.get() {
            Some(s.no_profiles_yet(&key(CommandId::NewProfile)))
        } else if self.rows.borrow().is_empty() {
            Some(s.no_snapshots_yet(&key(CommandId::TakeSnapshot)))
        } else {
            None
        };
        let pane = s.pane(Pane::Snapshots);
        let name = match &hint {
            Some(hint) => format!("{pane}. {hint}"),
            None => pane.to_string(),
        };
        self.control
            .set_hint(&name, hint.as_deref().unwrap_or_default());
    }
}

/// The row that takes the focus once the rows `deleted` (in ascending order) are gone
/// from a list of `len` rows: the one that stood right after the last deleted row, or
/// the new last row; `None` when no row is left.
fn row_after_deletion(deleted: &[usize], len: usize) -> Option<usize> {
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
mod tests;
