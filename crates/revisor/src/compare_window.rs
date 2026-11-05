//! The «Порівняти з…» window (TASK-11 #3): the other snapshots of the profile, from which
//! the one to compare the current snapshot with is chosen. The list has the columns of
//! the snapshot list and starts on the previous snapshot, or on the next one when there is
//! none; Enter or «Порівняти» compares, Esc or «Скасувати» closes the window.

use revisor_core::store::SnapshotInfo;
use wxdragon::prelude::*;

use crate::commands::{CommandId, command};
use crate::i18n::Strings;
use crate::keys::Shortcut;
use crate::main_window::{COMMENT_WIDTH, DATE_TIME_WIDTH};
use crate::menus::field_label;
use crate::win32;

/// The access key of the list's label: that of the command «Порівняти з…», whose words
/// the label takes up.
fn access() -> u8 {
    command(CommandId::CompareWith).access
}

/// The row the list starts on among the `count` other snapshots, when the current
/// snapshot stands at `current` in the full list: the previous snapshot, or the next one
/// when the current is the oldest; `None` without other snapshots.
pub fn initial_row(current: usize, count: usize) -> Option<usize> {
    (count > 0).then(|| current.saturating_sub(1).min(count - 1))
}

/// Shows the window for the snapshot at `current` of `snapshots`, the profile's snapshots
/// in list order, and returns the snapshot chosen to compare it with, or `None` when the
/// user cancels, or there is no other snapshot.
pub fn show(
    parent: &Frame,
    s: &'static dyn Strings,
    snapshots: &[SnapshotInfo],
    current: usize,
) -> Option<SnapshotInfo> {
    let others: Vec<&SnapshotInfo> = snapshots
        .iter()
        .enumerate()
        .filter(|&(i, _)| i != current)
        .map(|(_, row)| row)
        .collect();
    let start = initial_row(current, others.len())?;
    let dialog = Dialog::builder(parent, s.command(CommandId::CompareWith))
        .with_style(DialogStyle::DefaultDialogStyle | DialogStyle::ResizeBorder)
        .with_size(680, 420)
        .build();
    // The label comes right before the list, so screen readers name the list after it.
    let name = win32::format_date_time(snapshots[current].taken_at);
    let label = StaticText::builder(&dialog)
        .with_label(&field_label(s, &s.compare_with_label(&name), access()))
        .build();
    let list = ListCtrl::builder(&dialog)
        .with_style(ListCtrlStyle::Report | ListCtrlStyle::SingleSel)
        .build();
    list.insert_column(
        0,
        s.column_date_time(),
        ListColumnFormat::Left,
        DATE_TIME_WIDTH,
    );
    list.insert_column(1, s.column_comment(), ListColumnFormat::Left, COMMENT_WIDTH);
    win32::full_row_select_no_sort(list.get_handle());
    for (i, row) in others.iter().enumerate() {
        list.insert_item(i as i64, &win32::format_date_time(row.taken_at), None);
        if let Some(comment) = &row.comment {
            list.set_item_text_by_column(i as i64, 1, comment);
        }
    }
    win32::select_only(list.get_handle(), start);
    let compare = Button::builder(&dialog)
        .with_id(ID_OK)
        .with_label(s.compare())
        .build();
    let cancel = Button::builder(&dialog)
        .with_id(ID_CANCEL)
        .with_label(s.cancel())
        .build();
    compare.set_default();
    dialog.set_escape_id(ID_CANCEL);
    // Enter in the list activates its row, which the list keeps for itself instead of
    // pressing the default button; it compares too, as a double click does.
    list.on_item_activated(move |_| dialog.end_modal(ID_OK));
    // Alt with the label's letter, by the key's position, puts the focus in the list.
    dialog.on_char_hook(move |data| {
        if let WindowEventData::Keyboard(key) = data {
            let hit = Shortcut::from_event(&key).access_letter() == Some(access());
            key.event.skip(!hit);
            if hit {
                list.set_focus();
            }
        }
    });

    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    let side = SizerFlag::Left | SizerFlag::Right;
    sizer.add(&label, 0, side | SizerFlag::Top, 12);
    sizer.add(&list, 1, SizerFlag::Expand | SizerFlag::All, 12);
    let row = BoxSizer::builder(Orientation::Horizontal).build();
    row.add(&compare, 0, SizerFlag::AlignLeft, 8);
    row.add(&cancel, 0, SizerFlag::Left, 8);
    sizer.add_sizer(
        &row,
        0,
        SizerFlag::AlignRight | side | SizerFlag::Bottom,
        12,
    );
    dialog.set_sizer(sizer, true);
    dialog.centre();
    list.set_focus();
    let confirmed = dialog.show_modal() == ID_OK;
    let chosen = win32::focused_row(list.get_handle())
        .or_else(|| usize::try_from(list.get_first_selected_item()).ok())
        .unwrap_or(start);
    // A plain wxDragon Dialog is not destroyed when dropped; its handlers go with it.
    dialog.destroy();
    confirmed
        .then(|| others.get(chosen).copied().cloned())
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::En;

    #[test]
    fn the_list_starts_on_the_previous_snapshot_or_else_the_next() {
        // Five snapshots, the current one taken out of the list.
        assert_eq!(initial_row(3, 4), Some(2));
        assert_eq!(initial_row(4, 4), Some(3));
        assert_eq!(initial_row(1, 4), Some(0));
        // The oldest is current: the next one is first in the list.
        assert_eq!(initial_row(0, 4), Some(0));
        assert_eq!(initial_row(0, 1), Some(0));
        assert_eq!(initial_row(1, 1), Some(0));
        assert_eq!(initial_row(0, 0), None);
    }

    #[test]
    fn the_label_takes_the_letter_of_the_command() {
        assert_eq!(access(), b'W');
        assert!(
            En.compare_with_label("x")
                .to_ascii_uppercase()
                .contains(char::from(access()))
        );
    }
}
