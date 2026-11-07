//! The «Порівняти з…» window (TASK-11 #3): the other snapshots of the profile, from which
//! the one to compare the current snapshot with is chosen. The list has the columns of
//! the snapshot list and starts on the previous snapshot, or on the next one when there is
//! none; Enter or «Порівняти» compares, Esc or «Скасувати» closes the window.

use revisor_core::store::SnapshotInfo;
use wxdragon::prelude::*;

use crate::access::{self, Act, Target, compose, plain};
use crate::commands::CommandId;
use crate::i18n::{En, Strings};
use crate::snapshot_list::{Control, Native};
use crate::win32;

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
    let dialog = Dialog::builder(parent, &plain(s.command(CommandId::CompareWith)))
        .with_style(DialogStyle::DefaultDialogStyle | DialogStyle::ResizeBorder)
        .with_size(680, 420)
        .build();
    // The label comes right before the list, so screen readers name the list after it.
    // It takes up the words of the command «Порівняти з…», and its letter.
    let name = win32::format_date_time(snapshots[current].taken_at);
    let english = En.compare_with_label(&name);
    let label = StaticText::builder(&dialog)
        .with_label(&format!(
            "{}:",
            compose(s, &english, &s.compare_with_label(&name))
        ))
        .build();
    // The rows of the snapshot list, as its control shows them (TASK-27 #7).
    let rows = Native::new(&dialog, s, true);
    for (i, row) in others.iter().enumerate() {
        rows.insert(i, row);
    }
    rows.select_only(start);
    let list = *rows.list();
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
    access::bind(
        &dialog,
        vec![Target::new(english, Act::Focus, ())],
        move |()| {
            list.set_focus();
        },
    );

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
    let chosen = rows
        .focused_row()
        .or_else(|| rows.selected_rows().first().copied())
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
        let command = access::letter(En.command(CommandId::CompareWith));
        assert_eq!(command, Some(b'W'));
        assert_eq!(access::letter(&En.compare_with_label("x")), command);
    }
}
