//! A modal window of text to read: the «Help» windows, «Keyboard shortcuts» (F1) and
//! «About revisor», and the details of a change (TASK-14). A read-only text field, so the
//! text is read with the arrow keys a line at a time and copied, and a Close button; Esc
//! closes the window too, and the focus goes back where it was.

use wxdragon::prelude::*;

use crate::i18n::Strings;

/// Shows `text` under `title` until the user closes the window.
pub fn show(parent: &Frame, s: &dyn Strings, title: &str, text: &str) {
    let dialog = Dialog::builder(parent, title)
        .with_style(DialogStyle::DefaultDialogStyle | DialogStyle::ResizeBorder)
        .with_size(640, 480)
        .build();
    let field = TextCtrl::builder(&dialog)
        .with_style(TextCtrlStyle::MultiLine | TextCtrlStyle::ReadOnly | TextCtrlStyle::DontWrap)
        .build();
    field.set_value(text);
    field.set_insertion_point(0);
    // The dialog's Cancel: Esc presses it, and as in other dialogs it has no access key.
    let close = Button::builder(&dialog)
        .with_id(ID_CANCEL)
        .with_label(s.close_window())
        .build();
    dialog.set_escape_id(ID_CANCEL);

    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    sizer.add(&field, 1, SizerFlag::Expand | SizerFlag::All, 8);
    sizer.add(
        &close,
        0,
        SizerFlag::AlignRight | SizerFlag::Left | SizerFlag::Right | SizerFlag::Bottom,
        8,
    );
    dialog.set_sizer(sizer, true);
    dialog.centre();
    field.set_focus();
    dialog.show_modal();
    // A plain wxDragon Dialog is not destroyed when dropped.
    dialog.destroy();
}
