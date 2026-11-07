//! The settings window (TASK-9): the application's own settings (ADR-0007) that have a
//! control of their own — the interface language (TASK-16), what the focus does when a
//! snapshot ends, what it does when a comparison ends (TASK-11), how sizes are written
//! (TASK-13), how the details of a change write attributes (TASK-14), whether deleting
//! snapshots asks first, and whether showing a change no longer on disk in File Explorer
//! asks first (TASK-14). The profile opened last and the view of the difference list are
//! kept without a control.

use std::rc::Rc;

use revisor_core::settings::{
    AttributeFormat, FocusAfterComparison, FocusAfterSnapshot, Language, SizeFormat,
};
use wxdragon::prelude::*;

use crate::access::{self, Act, Name, Target, field_label, plain};
use crate::commands::CommandId;
use crate::i18n::Strings;

/// The settings the window edits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Values {
    pub language: Language,
    pub focus_after_snapshot: FocusAfterSnapshot,
    pub focus_after_comparison: FocusAfterComparison,
    pub size_format: SizeFormat,
    pub attribute_format: AttributeFormat,
    pub confirm_snapshot_deletion: bool,
    pub warn_missing_entry: bool,
}

/// The labelled controls of the window. OK and Cancel are not among them: they have no
/// access keys, as Enter and Esc press them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Control {
    Language,
    FocusAfterSnapshot,
    FocusAfterComparison,
    SizeFormat,
    AttributeFormat,
    ConfirmDeletion,
    WarnMissingEntry,
}

impl Control {
    const ALL: [Control; 7] = [
        Self::Language,
        Self::FocusAfterSnapshot,
        Self::FocusAfterComparison,
        Self::SizeFormat,
        Self::AttributeFormat,
        Self::ConfirmDeletion,
        Self::WarnMissingEntry,
    ];

    /// The name of the control's label or check box, whose English marks its access key.
    fn name(self) -> Name {
        match self {
            Self::Language => |s| s.language(),
            Self::FocusAfterSnapshot => |s| s.focus_after_snapshot(),
            Self::FocusAfterComparison => |s| s.focus_after_comparison(),
            Self::SizeFormat => |s| s.size_format(),
            Self::AttributeFormat => |s| s.attribute_format(),
            Self::ConfirmDeletion => |s| s.confirm_snapshot_deletion(),
            Self::WarnMissingEntry => |s| s.warn_missing_entry(),
        }
    }

    /// Whether the control is a check box, which its access key toggles, as an access key
    /// on a check box does in Windows; the others are lists that take the focus.
    fn is_check_box(self) -> bool {
        matches!(self, Self::ConfirmDeletion | Self::WarnMissingEntry)
    }

    /// The text of the control's label or check box: "&Focus after a snapshot:",
    /// "Підтверджувати видалення знімків (&C)".
    fn label(self, s: &dyn Strings) -> String {
        if self.is_check_box() {
            access::label(s, self.name())
        } else {
            field_label(s, self.name())
        }
    }
}

/// The access keys of the window: a check box is pressed, a list takes the focus.
pub(crate) fn targets() -> Vec<Target<Control>> {
    Control::ALL
        .into_iter()
        .map(|c| {
            let act = if c.is_check_box() {
                Act::Press
            } else {
                Act::Focus
            };
            Target::of(c.name(), act, c)
        })
        .collect()
}

struct Window {
    language: Choice,
    focus: Choice,
    comparison_focus: Choice,
    size_format: Choice,
    attribute_format: Choice,
    confirm: CheckBox,
    warn_missing: CheckBox,
}

/// Shows the window with the `initial` values and returns the new ones once the user
/// confirms, or `None` when they cancel.
pub fn show(parent: &Frame, s: &'static dyn Strings, initial: Values) -> Option<Values> {
    let dialog = Dialog::builder(parent, &plain(s.command(CommandId::Settings))).build();
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
    let comparison_focus_label = StaticText::builder(&dialog)
        .with_label(&Control::FocusAfterComparison.label(s))
        .build();
    let comparison_focus = Choice::builder(&dialog).build();
    for value in FocusAfterComparison::ALL {
        comparison_focus.append(s.comparison_focus_option(value));
    }
    let chosen = FocusAfterComparison::ALL
        .iter()
        .position(|&v| v == initial.focus_after_comparison)
        .unwrap_or(0);
    comparison_focus.set_selection(chosen as u32);
    let size_label = StaticText::builder(&dialog)
        .with_label(&Control::SizeFormat.label(s))
        .build();
    let size_format = Choice::builder(&dialog).build();
    for value in SizeFormat::ALL {
        size_format.append(s.size_option(value));
    }
    let chosen = SizeFormat::ALL
        .iter()
        .position(|&v| v == initial.size_format)
        .unwrap_or(0);
    size_format.set_selection(chosen as u32);
    let attribute_label = StaticText::builder(&dialog)
        .with_label(&Control::AttributeFormat.label(s))
        .build();
    let attribute_format = Choice::builder(&dialog).build();
    for value in AttributeFormat::ALL {
        attribute_format.append(s.attribute_option(value));
    }
    let chosen = AttributeFormat::ALL
        .iter()
        .position(|&v| v == initial.attribute_format)
        .unwrap_or(0);
    attribute_format.set_selection(chosen as u32);
    let confirm = CheckBox::builder(&dialog)
        .with_label(&Control::ConfirmDeletion.label(s))
        .build();
    confirm.set_value(initial.confirm_snapshot_deletion);
    let warn_missing = CheckBox::builder(&dialog)
        .with_label(&Control::WarnMissingEntry.label(s))
        .build();
    warn_missing.set_value(initial.warn_missing_entry);
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
    sizer.add(&comparison_focus_label, 0, side, 12);
    sizer.add(&comparison_focus, 0, SizerFlag::Expand | SizerFlag::All, 12);
    sizer.add(&size_label, 0, side, 12);
    sizer.add(&size_format, 0, SizerFlag::Expand | SizerFlag::All, 12);
    sizer.add(&attribute_label, 0, side, 12);
    sizer.add(&attribute_format, 0, SizerFlag::Expand | SizerFlag::All, 12);
    sizer.add(&confirm, 0, side | SizerFlag::Bottom, 12);
    sizer.add(&warn_missing, 0, side | SizerFlag::Bottom, 12);
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
        comparison_focus,
        size_format,
        attribute_format,
        confirm,
        warn_missing,
    });
    // Access keys by the key's position, so they work in the Ukrainian layout too
    // (document «Клавіатура й меню»).
    let w = Rc::clone(&window);
    access::bind(&dialog, targets(), move |control| w.access_key(control));

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
    /// Acts on the access key of `control`. The check box's key toggles it.
    fn access_key(&self, control: Control) {
        match control {
            Control::Language => self.language.set_focus(),
            Control::FocusAfterSnapshot => self.focus.set_focus(),
            Control::FocusAfterComparison => self.comparison_focus.set_focus(),
            Control::SizeFormat => self.size_format.set_focus(),
            Control::AttributeFormat => self.attribute_format.set_focus(),
            Control::ConfirmDeletion => {
                self.confirm.set_value(!self.confirm.get_value());
                self.confirm.set_focus();
            }
            Control::WarnMissingEntry => {
                self.warn_missing.set_value(!self.warn_missing.get_value());
                self.warn_missing.set_focus();
            }
        }
    }

    fn values(&self) -> Values {
        let language = self.language.get_selection().unwrap_or(0) as usize;
        let chosen = self.focus.get_selection().unwrap_or(0) as usize;
        let comparison = self.comparison_focus.get_selection().unwrap_or(0) as usize;
        let size = self.size_format.get_selection().unwrap_or(0) as usize;
        let attributes = self.attribute_format.get_selection().unwrap_or(0) as usize;
        Values {
            language: Language::ALL.get(language).copied().unwrap_or_default(),
            focus_after_snapshot: FocusAfterSnapshot::ALL
                .get(chosen)
                .copied()
                .unwrap_or_default(),
            focus_after_comparison: FocusAfterComparison::ALL
                .get(comparison)
                .copied()
                .unwrap_or_default(),
            size_format: SizeFormat::ALL.get(size).copied().unwrap_or_default(),
            attribute_format: AttributeFormat::ALL
                .get(attributes)
                .copied()
                .unwrap_or_default(),
            confirm_snapshot_deletion: self.confirm.get_value(),
            warn_missing_entry: self.warn_missing.get_value(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{En, Uk};

    #[test]
    fn the_labels_mark_their_letters_in_both_languages() {
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
            Control::FocusAfterComparison.label(&En),
            "Focus after a co&mparison:"
        );
        assert_eq!(
            Control::FocusAfterComparison.label(&Uk),
            "Фокус після порівняння (&M):"
        );
        assert_eq!(Control::SizeFormat.label(&En), "&Size:");
        assert_eq!(Control::SizeFormat.label(&Uk), "Розмір (&S):");
        assert_eq!(
            Control::ConfirmDeletion.label(&En),
            "&Confirm snapshot deletion"
        );
        assert_eq!(
            Control::ConfirmDeletion.label(&Uk),
            "Підтверджувати видалення знімків (&C)"
        );
        // TASK-14 #7, #13.
        assert_eq!(Control::AttributeFormat.label(&En), "&Attributes:");
        assert_eq!(Control::AttributeFormat.label(&Uk), "Атрибути (&A):");
        assert_eq!(
            Control::WarnMissingEntry.label(&En),
            "&Warn when the entry is no longer on disk"
        );
        assert_eq!(
            Control::WarnMissingEntry.label(&Uk),
            "Попереджати, коли запису вже немає на диску (&W)"
        );
    }

    #[test]
    fn the_attribute_options_are_named_in_both_languages_in_setting_order() {
        // TASK-14 #7; the first is the default.
        let uk: Vec<&str> = AttributeFormat::ALL
            .iter()
            .map(|&v| Uk.attribute_option(v))
            .collect();
        assert_eq!(
            uk,
            [
                "Словами",
                "Літерами, як у Провіднику",
                "Літерами через кому"
            ]
        );
        let en: Vec<&str> = AttributeFormat::ALL
            .iter()
            .map(|&v| En.attribute_option(v))
            .collect();
        assert_eq!(
            en,
            [
                "As words",
                "As letters, like File Explorer",
                "As letters with commas"
            ]
        );
        assert_eq!(AttributeFormat::ALL[0], AttributeFormat::default());
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
    fn the_size_options_are_named_in_both_languages_in_setting_order() {
        // TASK-13 #17; the first is the default.
        let uk: Vec<&str> = SizeFormat::ALL.iter().map(|&v| Uk.size_option(v)).collect();
        assert_eq!(uk, ["Як у Провіднику", "По 1000", "У байтах"]);
        let en: Vec<&str> = SizeFormat::ALL.iter().map(|&v| En.size_option(v)).collect();
        assert_eq!(en, ["Like File Explorer", "Decimal", "In bytes"]);
        assert_eq!(SizeFormat::ALL[0], SizeFormat::default());
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
