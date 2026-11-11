//! Message boxes: a message to read and close, a question to confirm, and a line of text
//! to enter.
//!
//! They are revisor's own windows rather than the ones of Windows or wxWidgets, so the
//! access key of an action button works by the key's position in any layout, as everywhere
//! in revisor (document «Клавіші доступу»). NVDA reads the text as the window's
//! description: by itself when it reports object descriptions, and with NVDA+B in any
//! case.

use wxdragon::prelude::*;

use crate::access::{self, Act, Name, Target, compose};
use crate::i18n::{En, Strings};

/// How wide the text of a message box gets before it wraps, in pixels.
const TEXT_WIDTH: i32 = 440;

/// Asks for one line of text under the label that `label` gives in each language (without
/// colon), starting from `initial`, which is selected so typing replaces it; Enter
/// confirms and Esc cancels. Returns the text, trimmed, or `None` when cancelled (TASK-9:
/// the comment of a snapshot).
pub fn ask_text(
    parent: &dyn WxWidget,
    s: &dyn Strings,
    title: &str,
    label: &dyn Fn(&dyn Strings) -> String,
    initial: &str,
) -> Option<String> {
    let dialog = Dialog::builder(parent, title).build();
    // The label comes right before the field, so screen readers name the field after it.
    let english = label(&En);
    let field_name = StaticText::builder(&dialog)
        .with_label(&format!("{}:", compose(s, &english, &label(s))))
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
    access::bind(
        &dialog,
        vec![Target::new(english, Act::Focus, ())],
        move |()| {
            field.set_focus();
        },
    );

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

/// Asks a question with the buttons `act` and Cancel; Esc cancels. Returns whether the
/// user chose `act`. With `cancel_default`, Enter cancels too, as an action that cannot be
/// undone should.
pub fn confirm(
    parent: &dyn WxWidget,
    s: &dyn Strings,
    title: &str,
    text: &str,
    act: Name,
    cancel_default: bool,
) -> bool {
    choose(parent, s, title, text, act, None, cancel_default)
}

/// Asks a question with two buttons: `act` and `decline` (a name of its own, or Cancel,
/// which has no access key), which Esc presses too. Returns whether the user chose `act`.
/// With `decline_default`, Enter declines too (TASK-16: «Restart now» or «Later»).
pub fn choose(
    parent: &dyn WxWidget,
    s: &dyn Strings,
    title: &str,
    text: &str,
    act: Name,
    decline: Option<Name>,
    decline_default: bool,
) -> bool {
    let dialog = Dialog::builder(parent, title).build();
    let act_button = Button::builder(&dialog)
        .with_id(ID_OK)
        .with_label(&access::label(s, act))
        .build();
    let decline_label = match decline {
        Some(name) => access::label(s, name),
        None => s.cancel().to_string(),
    };
    let cancel = Button::builder(&dialog)
        .with_id(ID_CANCEL)
        .with_label(&decline_label)
        .build();
    dialog.set_escape_id(ID_CANCEL);
    // The box closes once Alt is up, as every button pressed by its access key acts.
    let mut targets = vec![Target::of(act, Act::Press, ID_OK)];
    targets.extend(decline.map(|name| Target::of(name, Act::Press, ID_CANCEL)));
    access::bind(&dialog, targets, move |id| dialog.end_modal(id));
    let default = if decline_default {
        &cancel
    } else {
        &act_button
    };
    show(&dialog, text, &[&act_button, &cancel], default) == ID_OK
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
