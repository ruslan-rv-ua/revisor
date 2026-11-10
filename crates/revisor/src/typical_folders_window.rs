//! The «Додати типові теки» window (TASK-36): the typical folders a profile can take in,
//! each with a check box, all checked when the window opens. A native list with check
//! boxes, which NVDA reads with the state of each (checked 2026-11-09). Space checks or
//! unchecks a folder; Enter or «Додати» adds the checked ones, Esc or «Скасувати» closes
//! the window.

use revisor_core::typical::Offered;
use wxdragon::prelude::*;

use crate::access::{self, Act, Name, Target, field_label, plain};
use crate::i18n::Strings;
use crate::win32;

/// The text of a folder's row: its name, then its path on this computer.
pub fn row_text(s: &dyn Strings, folder: &Offered) -> String {
    format!(
        "{} ({})",
        s.typical_folder(folder.folder),
        folder.path.display()
    )
}

/// Shows the window with the `offered` folders and returns the checked ones, or nothing
/// when the user cancels.
pub fn show(parent: &Dialog, s: &'static dyn Strings, offered: &[Offered]) -> Vec<Offered> {
    let dialog = Dialog::builder(parent, &plain(s.add_typical_folders()))
        .with_style(DialogStyle::DefaultDialogStyle | DialogStyle::ResizeBorder)
        .with_size(600, 360)
        .build();
    // The label comes right before the list, so screen readers name the list after it.
    let name: Name = |s| s.profile_folders();
    let label = StaticText::builder(&dialog)
        .with_label(&field_label(s, name))
        .build();
    let list = ListCtrl::builder(&dialog)
        .with_style(ListCtrlStyle::Report | ListCtrlStyle::NoHeader | ListCtrlStyle::SingleSel)
        .build();
    let hwnd = list.get_handle();
    win32::lists::enable_check_boxes(hwnd);
    list.insert_column(0, "", ListColumnFormat::Left, 540);
    for (i, folder) in offered.iter().enumerate() {
        list.insert_item(i as i64, &row_text(s, folder), None);
        win32::lists::set_checked(hwnd, i, true);
    }
    // At least as wide as the longest row, so no path is cut off.
    list.set_column_width(0, -1);
    let longest = list.get_column_width(0);
    win32::lists::select_only(hwnd, 0);
    let add = Button::builder(&dialog)
        .with_id(ID_OK)
        .with_label(s.add())
        .build();
    let cancel = Button::builder(&dialog)
        .with_id(ID_CANCEL)
        .with_label(s.cancel())
        .build();
    add.set_default();
    dialog.set_escape_id(ID_CANCEL);
    // Enter in the list activates its row, which the list keeps for itself instead of
    // pressing the default button; it adds the folders too. wx activates the row on Space
    // as well, which here only checks or unchecks it.
    list.on_item_activated(move |_| {
        if !win32::focus::space_down() {
            dialog.end_modal(ID_OK);
        }
    });
    access::bind(&dialog, vec![Target::of(name, Act::Focus, ())], move |()| {
        list.set_focus();
    });

    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    let side = SizerFlag::Left | SizerFlag::Right;
    sizer.add(&label, 0, side | SizerFlag::Top, 12);
    sizer.add(&list, 1, SizerFlag::Expand | SizerFlag::All, 12);
    let row = BoxSizer::builder(Orientation::Horizontal).build();
    row.add(&add, 0, SizerFlag::AlignLeft, 8);
    row.add(&cancel, 0, SizerFlag::Left, 8);
    sizer.add_sizer(
        &row,
        0,
        SizerFlag::AlignRight | side | SizerFlag::Bottom,
        12,
    );
    dialog.set_sizer(sizer, true);
    win32::lists::fill_width(hwnd, longest);
    dialog.centre();
    list.set_focus();
    let confirmed = dialog.show_modal() == ID_OK;
    let checked = offered
        .iter()
        .enumerate()
        .filter(|&(i, _)| win32::lists::is_checked(hwnd, i))
        .map(|(_, folder)| folder.clone())
        .collect();
    // A plain wxDragon Dialog is not destroyed when dropped; its handlers go with it.
    dialog.destroy();
    if confirmed { checked } else { Vec::new() }
}
