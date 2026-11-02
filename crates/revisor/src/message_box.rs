//! Message boxes: a message to read and close, a question to confirm, and a line of text
//! to enter.
//!
//! They are revisor's own windows rather than the ones of Windows or wxWidgets, so the
//! access key of an action button works by the key's position in any layout, as everywhere
//! in revisor (document «Клавіатура й меню»). NVDA reads the text as the window's
//! description: by itself when it reports object descriptions, and with NVDA+B in any
//! case.

use std::cell::Cell;
use std::rc::Rc;

use wxdragon::prelude::*;

use crate::i18n::Strings;
use crate::keys::Shortcut;
use crate::menus::{button_label, field_label};
use crate::win32;

/// How wide the text of a message box gets before it wraps, in pixels.
const TEXT_WIDTH: i32 = 440;

/// Asks for one line of text under `label` (without access key or colon), whose access
/// key is `access`, starting from `initial`, which is selected so typing replaces it;
/// Enter confirms and Esc cancels. Returns the text, trimmed, or `None` when cancelled
/// (TASK-9: the comment of a snapshot).
pub fn ask_text(
    parent: &dyn WxWidget,
    s: &dyn Strings,
    title: &str,
    label: &str,
    access: u8,
    initial: &str,
) -> Option<String> {
    let dialog = Dialog::builder(parent, title).build();
    // The label comes right before the field, so screen readers name the field after it.
    let field_name = StaticText::builder(&dialog)
        .with_label(&field_label(s, label, access))
        .build();
    let field = TextCtrl::builder(&dialog).with_value(initial).build();
    field.set_min_size(Size::new(TEXT_WIDTH, -1));
    let ok = Button::builder(&dialog)
        .with_id(ID_OK)
        .with_label(s.ok())
        .build();
    let cancel = Button::builder(&dialog)
        .with_id(ID_CANCEL)
        .with_label(s.cancel())
        .build();
    dialog.set_escape_id(ID_CANCEL);
    // Alt with the label's letter, by the key's position, puts the focus in the field.
    let target = field;
    dialog.on_char_hook(move |data| {
        if let WindowEventData::Keyboard(key) = data {
            let hit = Shortcut::from_event(&key).access_letter() == Some(access);
            key.event.skip(!hit);
            if hit {
                target.set_focus();
            }
        }
    });

    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    sizer.add(
        &field_name,
        0,
        SizerFlag::Left | SizerFlag::Right | SizerFlag::Top,
        12,
    );
    sizer.add(&field, 0, SizerFlag::Expand | SizerFlag::All, 12);
    let row = BoxSizer::builder(Orientation::Horizontal).build();
    row.add(&ok, 0, SizerFlag::AlignLeft, 8);
    row.add(&cancel, 0, SizerFlag::Left, 8);
    sizer.add_sizer(
        &row,
        0,
        SizerFlag::AlignRight | SizerFlag::Left | SizerFlag::Right | SizerFlag::Bottom,
        12,
    );
    dialog.set_sizer_and_fit(sizer, true);
    dialog.centre();
    ok.set_default();
    field.set_focus();
    field.select_all();
    let confirmed = dialog.show_modal() == ID_OK;
    let text = field.get_value().trim().to_string();
    // A plain wxDragon Dialog is not destroyed when dropped.
    dialog.destroy();
    confirmed.then_some(text)
}

/// Shows a message with an OK button; Enter and Esc close it.
pub fn tell(parent: &dyn WxWidget, s: &dyn Strings, title: &str, text: &str) {
    let dialog = Dialog::builder(parent, title).build();
    let ok = Button::builder(&dialog)
        .with_id(ID_OK)
        .with_label(s.ok())
        .build();
    dialog.set_escape_id(ID_OK);
    show(&dialog, text, &[&ok], &ok);
}

/// Asks a question with the buttons `act`, whose access key is `access`, and Cancel; Esc
/// cancels. Returns whether the user chose `act`. With `cancel_default`, Enter cancels
/// too, as an action that cannot be undone should.
pub fn confirm(
    parent: &dyn WxWidget,
    s: &dyn Strings,
    title: &str,
    text: &str,
    (act, access): (&str, u8),
    cancel_default: bool,
) -> bool {
    let dialog = Dialog::builder(parent, title).build();
    let act = Button::builder(&dialog)
        .with_id(ID_OK)
        .with_label(&button_label(s, act, access, false))
        .build();
    let cancel = Button::builder(&dialog)
        .with_id(ID_CANCEL)
        .with_label(s.cancel())
        .build();
    dialog.set_escape_id(ID_CANCEL);
    // The box closes once Alt is up: closing it on the key press would hand the release of
    // Alt to the window below, which takes it for Alt alone and goes into its window menu,
    // where the next keys are lost.
    let acted = Rc::new(Cell::new(false));
    let pressed = Rc::clone(&acted);
    dialog.on_char_hook(move |data| {
        if let WindowEventData::Keyboard(key) = data {
            let hit = Shortcut::from_event(&key).access_letter() == Some(access);
            key.event.skip(!hit);
            if hit {
                pressed.set(true);
            }
        }
    });
    dialog.on_idle(move |data| {
        if acted.get() && !win32::alt_down() {
            acted.set(false);
            dialog.end_modal(ID_OK);
        }
        if let WindowEventData::Idle(event) = data {
            event.event.skip(true);
        }
    });
    let default = if cancel_default { &cancel } else { &act };
    show(&dialog, text, &[&act, &cancel], default) == ID_OK
}

/// Lays out the text above the buttons, shows the dialog with the focus on its default
/// button, and returns the id of the button that closed it.
fn show(dialog: &Dialog, text: &str, buttons: &[&Button], default: &Button) -> i32 {
    // "&" would underline the next letter as an access key.
    let label = StaticText::builder(dialog)
        .with_label(&text.replace('&', "&&"))
        .build();
    label.wrap(TEXT_WIDTH);
    let row = BoxSizer::builder(Orientation::Horizontal).build();
    for (i, button) in buttons.iter().enumerate() {
        let flag = if i == 0 {
            SizerFlag::AlignLeft
        } else {
            SizerFlag::Left
        };
        row.add(*button, 0, flag, 8);
    }
    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    sizer.add(&label, 0, SizerFlag::All, 12);
    sizer.add_sizer(
        &row,
        0,
        SizerFlag::AlignRight | SizerFlag::Left | SizerFlag::Right | SizerFlag::Bottom,
        12,
    );
    dialog.set_sizer_and_fit(sizer, true);
    dialog.centre();
    default.set_default();
    default.set_focus();
    let closed_by = dialog.show_modal();
    // A plain wxDragon Dialog is not destroyed when dropped.
    dialog.destroy();
    closed_by
}
