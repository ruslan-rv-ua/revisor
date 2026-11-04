//! The settings window (TASK-9): the application's own settings (ADR-0007) that have a
//! control of their own — the interface language (TASK-16), what the focus does when a
//! snapshot ends, and whether deleting snapshots asks first. The profile opened last is
//! kept without a control.

use std::rc::Rc;

use revisor_core::settings::{FocusAfterSnapshot, Language};
use wxdragon::prelude::*;

use crate::commands::CommandId;
use crate::i18n::Strings;
use crate::keys::Shortcut;
use crate::menus::{button_label, field_label};

/// The settings the window edits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Values {
    pub language: Language,
    pub focus_after_snapshot: FocusAfterSnapshot,
    pub confirm_snapshot_deletion: bool,
}

/// The labelled controls of the window. OK and Cancel are not among them: they have no
/// access keys, as Enter and Esc press them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Control {
    Language,
    FocusAfterSnapshot,
    ConfirmDeletion,
}

impl Control {
    const ALL: [Control; 3] = [
        Self::Language,
        Self::FocusAfterSnapshot,
        Self::ConfirmDeletion,
    ];

    /// The access key, as the document «Клавіатура й меню» lists it.
    const fn access(self) -> u8 {
        match self {
            Self::Language => b'L',
            Self::FocusAfterSnapshot => b'F',
            Self::ConfirmDeletion => b'C',
        }
    }

    fn name(self, s: &dyn Strings) -> &'static str {
        match self {
            Self::Language => s.language(),
            Self::FocusAfterSnapshot => s.focus_after_snapshot(),
            Self::ConfirmDeletion => s.confirm_snapshot_deletion(),
        }
    }

    /// The text of the control's label or check box: "&Focus after a snapshot:",
    /// "Підтверджувати видалення знімків (&C)".
    fn label(self, s: &dyn Strings) -> String {
        let (name, access) = (self.name(s), self.access());
        match self {
            Self::Language | Self::FocusAfterSnapshot => field_label(s, name, access),
            Self::ConfirmDeletion => button_label(s, name, access, false),
        }
    }
}

struct Window {
    language: Choice,
    focus: Choice,
    confirm: CheckBox,
}

/// Shows the window with the `initial` values and returns the new ones once the user
/// confirms, or `None` when they cancel.
pub fn show(parent: &Frame, s: &'static dyn Strings, initial: Values) -> Option<Values> {
    let dialog = Dialog::builder(parent, s.command(CommandId::Settings)).build();
    // Made in tab order. The label comes right before its control, so screen readers name
    // the control after it.
    let language_label = StaticText::builder(&dialog)
        .with_label(&Control::Language.label(s))
        .build();
    let language = Choice::builder(&dialog).build();
    for value in Language::ALL {
        language.append(s.language_option(value));
    }
    let chosen = Language::ALL
        .iter()
        .position(|&v| v == initial.language)
        .unwrap_or(0);
    language.set_selection(chosen as u32);
    let focus_label = StaticText::builder(&dialog)
        .with_label(&Control::FocusAfterSnapshot.label(s))
        .build();
    let focus = Choice::builder(&dialog).build();
    for value in FocusAfterSnapshot::ALL {
        focus.append(s.focus_option(value));
    }
    let chosen = FocusAfterSnapshot::ALL
        .iter()
        .position(|&v| v == initial.focus_after_snapshot)
        .unwrap_or(0);
    focus.set_selection(chosen as u32);
    let confirm = CheckBox::builder(&dialog)
        .with_label(&Control::ConfirmDeletion.label(s))
        .build();
    confirm.set_value(initial.confirm_snapshot_deletion);
    let ok = Button::builder(&dialog)
        .with_id(ID_OK)
        .with_label(s.ok())
        .build();
    let cancel = Button::builder(&dialog)
        .with_id(ID_CANCEL)
        .with_label(s.cancel())
        .build();
    ok.set_default();
    dialog.set_escape_id(ID_CANCEL);

    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    let side = SizerFlag::Left | SizerFlag::Right;
    sizer.add(&language_label, 0, side | SizerFlag::Top, 12);
    sizer.add(&language, 0, SizerFlag::Expand | SizerFlag::All, 12);
    sizer.add(&focus_label, 0, side, 12);
    sizer.add(&focus, 0, SizerFlag::Expand | SizerFlag::All, 12);
    sizer.add(&confirm, 0, side | SizerFlag::Bottom, 12);
    let commit_row = BoxSizer::builder(Orientation::Horizontal).build();
    commit_row.add(&ok, 0, SizerFlag::AlignLeft, 8);
    commit_row.add(&cancel, 0, SizerFlag::Left, 8);
    sizer.add_sizer(
        &commit_row,
        0,
        SizerFlag::AlignRight | side | SizerFlag::Bottom,
        12,
    );
    dialog.set_sizer_and_fit(sizer, true);

    let window = Rc::new(Window {
        language,
        focus,
        confirm,
    });
    // Access keys by the key's position, so they work in the Ukrainian layout too
    // (document «Клавіатура й меню»).
    let w = Rc::clone(&window);
    dialog.on_char_hook(move |data| {
        if let WindowEventData::Keyboard(key) = data {
            let handled = Shortcut::from_event(&key)
                .access_letter()
                .is_some_and(|letter| w.access_key(letter));
            key.event.skip(!handled);
        }
    });

    dialog.centre();
    window.language.set_focus();
    let confirmed = dialog.show_modal() == ID_OK;
    let values = window.values();
    // A plain wxDragon Dialog is not destroyed when dropped; its handlers, which hold the
    // window, go with it.
    dialog.destroy();
    confirmed.then_some(values)
}

impl Window {
    /// Acts on the access key `letter`; returns whether it is one of the window's. The
    /// check box's key toggles it, as an access key on a check box does in Windows.
    fn access_key(&self, letter: u8) -> bool {
        let Some(control) = Control::ALL.into_iter().find(|c| c.access() == letter) else {
            return false;
        };
        match control {
            Control::Language => self.language.set_focus(),
            Control::FocusAfterSnapshot => self.focus.set_focus(),
            Control::ConfirmDeletion => {
                self.confirm.set_value(!self.confirm.get_value());
                self.confirm.set_focus();
            }
        }
        true
    }

    fn values(&self) -> Values {
        let language = self.language.get_selection().unwrap_or(0) as usize;
        let chosen = self.focus.get_selection().unwrap_or(0) as usize;
        Values {
            language: Language::ALL.get(language).copied().unwrap_or_default(),
            focus_after_snapshot: FocusAfterSnapshot::ALL
                .get(chosen)
                .copied()
                .unwrap_or_default(),
            confirm_snapshot_deletion: self.confirm.get_value(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{En, Uk};

    #[test]
    fn each_labelled_control_has_its_own_latin_access_key() {
        for (i, control) in Control::ALL.into_iter().enumerate() {
            let letter = char::from(control.access());
            assert!(
                control.name(&En).to_ascii_uppercase().contains(letter),
                "{control:?}"
            );
            assert!(
                Control::ALL[..i]
                    .iter()
                    .all(|c| c.access() != control.access()),
                "{control:?}"
            );
        }
        assert_eq!(Control::Language.label(&En), "&Language:");
        assert_eq!(Control::Language.label(&Uk), "Мова (&L):");
        assert_eq!(
            Control::FocusAfterSnapshot.label(&En),
            "&Focus after a snapshot:"
        );
        assert_eq!(
            Control::FocusAfterSnapshot.label(&Uk),
            "Фокус після знімка (&F):"
        );
        assert_eq!(
            Control::ConfirmDeletion.label(&En),
            "&Confirm snapshot deletion"
        );
        assert_eq!(
            Control::ConfirmDeletion.label(&Uk),
            "Підтверджувати видалення знімків (&C)"
        );
    }

    #[test]
    fn each_language_is_named_in_itself_whatever_the_interface_speaks() {
        // WCAG 2.2, 3.1.2: a reader of either language finds their own by its name.
        let uk: Vec<&str> = Language::ALL
            .iter()
            .map(|&v| Uk.language_option(v))
            .collect();
        assert_eq!(uk, ["Як у Windows", "English", "Українська"]);
        let en: Vec<&str> = Language::ALL
            .iter()
            .map(|&v| En.language_option(v))
            .collect();
        assert_eq!(en, ["Same as Windows", "English", "Українська"]);
        assert_eq!(Language::ALL[0], Language::default());
    }

    #[test]
    fn the_focus_options_are_named_in_both_languages_in_setting_order() {
        let uk: Vec<&str> = FocusAfterSnapshot::ALL
            .iter()
            .map(|&v| Uk.focus_option(v))
            .collect();
        assert_eq!(uk, ["Якщо фокус у переліку", "Завжди", "Ніколи"]);
        let en: Vec<&str> = FocusAfterSnapshot::ALL
            .iter()
            .map(|&v| En.focus_option(v))
            .collect();
        assert_eq!(en, ["If focus is in the list", "Always", "Never"]);
    }
}
