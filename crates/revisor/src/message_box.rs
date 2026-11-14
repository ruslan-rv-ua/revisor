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
use crate::win32;

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
    show(&dialog, text, None, &[&ok], &ok);
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
    ask(parent, s, title, text, act, decline, decline_default, None).0
}

/// As [`confirm`], with the check box `check` above the buttons, unticked at first; its
/// access key toggles it. Returns whether the user chose `act` and whether the check box
/// was ticked, however the box was closed (TASK-48: «Більше не пропонувати»).
pub fn confirm_with_check_box(
    parent: &dyn WxWidget,
    s: &dyn Strings,
    title: &str,
    text: &str,
    act: Name,
    check: Name,
    cancel_default: bool,
) -> (bool, bool) {
    ask(
        parent,
        s,
        title,
        text,
        act,
        None,
        cancel_default,
        Some(check),
    )
}

/// The id the access key of the check box of [`ask`] dispatches; no button has it.
const CHECK_BOX: i32 = -1;

/// The question of [`choose`], with the check box `check` above the buttons when there is
/// one: returns the choice and whether the check box was ticked.
#[allow(clippy::too_many_arguments)]
fn ask(
    parent: &dyn WxWidget,
    s: &dyn Strings,
    title: &str,
    text: &str,
    act: Name,
    decline: Option<Name>,
    decline_default: bool,
    check: Option<Name>,
) -> (bool, bool) {
    let dialog = Dialog::builder(parent, title).build();
    // Made before the buttons, so the tab order goes from it to them.
    let check_box = check.map(|name| {
        CheckBox::builder(&dialog)
            .with_label(&access::label(s, name))
            .build()
    });
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
    targets.extend(check.map(|name| Target::of(name, Act::Press, CHECK_BOX)));
    access::bind(&dialog, targets, move |id| match check_box {
        // Pressed as a click does, so screen readers say its new state.
        Some(check_box) if id == CHECK_BOX => win32::focus::click(check_box.get_handle()),
        _ => dialog.end_modal(id),
    });
    let default = if decline_default {
        &cancel
    } else {
        &act_button
    };
    let (closed_by, ticked) = show(
        &dialog,
        text,
        check_box.as_ref(),
        &[&act_button, &cancel],
        default,
    );
    (closed_by == ID_OK, ticked)
}

/// Lays out the text, the check box `check` if there is one, and the buttons below them,
/// shows the dialog with the focus on its default button, and returns the id of the button
/// that closed it and whether the check box was ticked.
fn show(
    dialog: &Dialog,
    text: &str,
    check: Option<&CheckBox>,
    buttons: &[&Button],
    default: &Button,
) -> (i32, bool) {
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
    if let Some(check) = check {
        sizer.add(
            check,
            0,
            SizerFlag::Left | SizerFlag::Right | SizerFlag::Bottom,
            12,
        );
    }
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
    let ticked = check.is_some_and(CheckBox::get_value);
    // A plain wxDragon Dialog is not destroyed when dropped.
    dialog.destroy();
    (closed_by, ticked)
}
