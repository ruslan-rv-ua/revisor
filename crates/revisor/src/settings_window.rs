//! The settings window (TASK-9): the application's own settings (ADR-0007) that have a
//! control of their own — the interface language (TASK-16), its theme (TASK-34), what the
//! focus does when a snapshot ends, what it does when a comparison ends (TASK-11), how sizes
//! are written (TASK-13), how the details of a change write attributes (TASK-14), what the
//! nodes of the difference tree say (TASK-33), whether deleting snapshots asks first, and
//! whether showing a change no longer on disk in File Explorer asks first (TASK-14), and
//! whether the list of typical folders offers other users' folders (TASK-43). The
//! profile opened last and the view of the difference — its types, order, list or tree — are
//! kept without a control. The settings lie on tabs (TASK-44): «Загальні», «Знімки»,
//! «Різниця» and «Експорт», which holds whether the exported file opens (TASK-39) and what
//! separates the values of CSV (TASK-40).

use std::rc::Rc;

use revisor_core::settings::{
    AttributeFormat, CsvDelimiter, FocusAfterComparison, FocusAfterSnapshot, Language, SizeFormat,
    Theme, TreeNodeText,
};
use wxdragon::prelude::*;

use crate::access::{self, Act, Name, Target, field_label, plain};
use crate::commands::CommandId;
use crate::i18n::Strings;
use crate::win32;

/// The settings the window edits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Values {
    pub language: Language,
    pub theme: Theme,
    pub focus_after_snapshot: FocusAfterSnapshot,
    pub focus_after_comparison: FocusAfterComparison,
    pub size_format: SizeFormat,
    pub attribute_format: AttributeFormat,
    pub tree_node_text: TreeNodeText,
    pub confirm_snapshot_deletion: bool,
    pub warn_missing_entry: bool,
    pub other_users_folders: bool,
    pub open_after_export: bool,
    pub csv_delimiter: CsvDelimiter,
}

/// The tabs of the window (TASK-44), in their order. Settings added later go into the tab
/// they belong to; «Експорт» came with its first setting (TASK-39).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Page {
    General,
    Snapshots,
    Difference,
    Export,
}

impl Page {
    pub(crate) const ALL: [Page; 4] = [
        Self::General,
        Self::Snapshots,
        Self::Difference,
        Self::Export,
    ];

    /// The name of the tab. Tabs have no access keys: Ctrl+Tab and Ctrl+PgDn switch them.
    fn name(self, s: &dyn Strings) -> &'static str {
        match self {
            Self::General => s.settings_general(),
            Self::Snapshots => s.settings_snapshots(),
            Self::Difference => s.settings_difference(),
            Self::Export => s.settings_export(),
        }
    }
}

/// The labelled controls of the window. OK and Cancel are not among them: they have no
/// access keys, as Enter and Esc press them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Control {
    Language,
    Theme,
    FocusAfterSnapshot,
    ConfirmDeletion,
    OtherUsersFolders,
    FocusAfterComparison,
    SizeFormat,
    AttributeFormat,
    TreeNodes,
    WarnMissingEntry,
    CsvDelimiter,
    OpenAfterExport,
}

impl Control {
    /// In tab order: tab by tab, as each tab shows them.
    const ALL: [Control; 12] = [
        Self::Language,
        Self::Theme,
        Self::FocusAfterSnapshot,
        Self::ConfirmDeletion,
        Self::OtherUsersFolders,
        Self::FocusAfterComparison,
        Self::SizeFormat,
        Self::AttributeFormat,
        Self::TreeNodes,
        Self::WarnMissingEntry,
        Self::CsvDelimiter,
        Self::OpenAfterExport,
    ];

    /// The tab the control is on.
    fn page(self) -> Page {
        match self {
            Self::Language | Self::Theme => Page::General,
            Self::FocusAfterSnapshot | Self::ConfirmDeletion | Self::OtherUsersFolders => {
                Page::Snapshots
            }
            Self::FocusAfterComparison
            | Self::SizeFormat
            | Self::AttributeFormat
            | Self::TreeNodes
            | Self::WarnMissingEntry => Page::Difference,
            Self::CsvDelimiter | Self::OpenAfterExport => Page::Export,
        }
    }

    /// The name of the control's label or check box, whose English marks its access key.
    fn name(self) -> Name {
        match self {
            Self::Language => |s| s.language(),
            Self::Theme => |s| s.theme(),
            Self::FocusAfterSnapshot => |s| s.focus_after_snapshot(),
            Self::FocusAfterComparison => |s| s.focus_after_comparison(),
            Self::SizeFormat => |s| s.size_format(),
            Self::AttributeFormat => |s| s.attribute_format(),
            Self::TreeNodes => |s| s.tree_nodes(),
            Self::ConfirmDeletion => |s| s.confirm_snapshot_deletion(),
            Self::WarnMissingEntry => |s| s.warn_missing_entry(),
            Self::OtherUsersFolders => |s| s.other_users_folders(),
            Self::CsvDelimiter => |s| s.csv_delimiter(),
            Self::OpenAfterExport => |s| s.open_after_export(),
        }
    }

    /// Whether the control is a check box, which its access key toggles, as an access key
    /// on a check box does in Windows; the others are lists that take the focus.
    fn is_check_box(self) -> bool {
        matches!(
            self,
            Self::ConfirmDeletion
                | Self::WarnMissingEntry
                | Self::OtherUsersFolders
                | Self::OpenAfterExport
        )
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

    /// The access key: a check box is pressed, a list takes the focus.
    fn target(self) -> Target<Control> {
        let act = if self.is_check_box() {
            Act::Press
        } else {
            Act::Focus
        };
        Target::of(self.name(), act, self)
    }
}

/// The access keys of the tab `page`, the only ones that act while it is shown
/// (TASK-44 #3).
pub(crate) fn targets_of(page: Page) -> Vec<Target<Control>> {
    Control::ALL
        .into_iter()
        .filter(|c| c.page() == page)
        .map(Control::target)
        .collect()
}

struct Window {
    notebook: Notebook,
    language: Choice,
    theme: Choice,
    focus: Choice,
    comparison_focus: Choice,
    size_format: Choice,
    attribute_format: Choice,
    tree_nodes: Choice,
    confirm: CheckBox,
    other_users: CheckBox,
    warn_missing: CheckBox,
    csv_delimiter: Choice,
    open_after_export: CheckBox,
}

/// A tab under construction: its panel and the column of its controls.
struct Tab {
    panel: Panel,
    sizer: BoxSizer,
}

impl Tab {
    fn new(notebook: &Notebook) -> Self {
        Self {
            panel: Panel::builder(notebook).build(),
            sizer: BoxSizer::builder(Orientation::Vertical).build(),
        }
    }

    /// Adds the list of `control` with `chosen` of its `options` selected. Its label comes
    /// right before it, so screen readers name the list after it.
    fn choice(&self, s: &dyn Strings, control: Control, options: &[&str], chosen: usize) -> Choice {
        let label = StaticText::builder(&self.panel)
            .with_label(&control.label(s))
            .build();
        let choice = Choice::builder(&self.panel).build();
        for option in options {
            choice.append(option);
        }
        choice.set_selection(chosen as u32);
        let side = SizerFlag::Left | SizerFlag::Right;
        self.sizer.add(&label, 0, side | SizerFlag::Top, 12);
        self.sizer.add_spacer(4);
        self.sizer.add(&choice, 0, SizerFlag::Expand | side, 12);
        choice
    }

    /// Adds the check box of `control`, ticked as `value` says.
    fn check_box(&self, s: &dyn Strings, control: Control, value: bool) -> CheckBox {
        let check_box = CheckBox::builder(&self.panel)
            .with_label(&control.label(s))
            .build();
        check_box.set_value(value);
        let side = SizerFlag::Left | SizerFlag::Right;
        self.sizer.add(&check_box, 0, side | SizerFlag::Top, 12);
        check_box
    }

    /// Puts the tab into `notebook`, with room below its last control.
    fn finish(self, notebook: &Notebook, s: &dyn Strings, page: Page) {
        self.sizer.add_spacer(12);
        self.panel.set_sizer(self.sizer, true);
        // NVDA says the tab when Ctrl+Tab brings the focus onto its page (TASK-44 #7).
        win32::accessibility::make_property_page(self.panel.get_handle(), page.name(s));
        notebook.add_page(&self.panel, page.name(s), page == Page::General, None);
    }
}

/// Presses the check box as a click does: it toggles and takes the focus, and screen
/// readers say its new state also when it had the focus already (TASK-39).
fn press(check_box: &CheckBox) {
    win32::focus::click(check_box.get_handle());
}

/// The position of `value` among `all`, or the first.
fn position<V: PartialEq>(all: &[V], value: &V) -> usize {
    all.iter().position(|v| v == value).unwrap_or(0)
}

/// Shows the window with the `initial` values and returns the new ones once the user
/// confirms, or `None` when they cancel. It opens on «Загальні» with the focus on its first
/// list; OK keeps the settings of every tab, Cancel none (TASK-44 #1, #4).
pub fn show(parent: &Frame, s: &'static dyn Strings, initial: Values) -> Option<Values> {
    let dialog = Dialog::builder(parent, &plain(s.command(CommandId::Settings))).build();
    // Made in tab order: the tabs, then OK and Cancel below them.
    let notebook = Notebook::builder(&dialog).build();

    let general = Tab::new(&notebook);
    let options: Vec<&str> = Language::ALL
        .iter()
        .map(|&v| s.language_option(v))
        .collect();
    let language = general.choice(
        s,
        Control::Language,
        &options,
        position(&Language::ALL, &initial.language),
    );
    let options: Vec<&str> = Theme::ALL.iter().map(|&v| s.theme_option(v)).collect();
    let theme = general.choice(
        s,
        Control::Theme,
        &options,
        position(&Theme::ALL, &initial.theme),
    );
    general.finish(&notebook, s, Page::General);

    let snapshots = Tab::new(&notebook);
    let options: Vec<&str> = FocusAfterSnapshot::ALL
        .iter()
        .map(|&v| s.focus_option(v))
        .collect();
    let focus = snapshots.choice(
        s,
        Control::FocusAfterSnapshot,
        &options,
        position(&FocusAfterSnapshot::ALL, &initial.focus_after_snapshot),
    );
    let confirm = snapshots.check_box(
        s,
        Control::ConfirmDeletion,
        initial.confirm_snapshot_deletion,
    );
    let other_users =
        snapshots.check_box(s, Control::OtherUsersFolders, initial.other_users_folders);
    snapshots.finish(&notebook, s, Page::Snapshots);

    let difference = Tab::new(&notebook);
    let options: Vec<&str> = FocusAfterComparison::ALL
        .iter()
        .map(|&v| s.comparison_focus_option(v))
        .collect();
    let comparison_focus = difference.choice(
        s,
        Control::FocusAfterComparison,
        &options,
        position(&FocusAfterComparison::ALL, &initial.focus_after_comparison),
    );
    let options: Vec<&str> = SizeFormat::ALL.iter().map(|&v| s.size_option(v)).collect();
    let size_format = difference.choice(
        s,
        Control::SizeFormat,
        &options,
        position(&SizeFormat::ALL, &initial.size_format),
    );
    let options: Vec<&str> = AttributeFormat::ALL
        .iter()
        .map(|&v| s.attribute_option(v))
        .collect();
    let attribute_format = difference.choice(
        s,
        Control::AttributeFormat,
        &options,
        position(&AttributeFormat::ALL, &initial.attribute_format),
    );
    let options: Vec<&str> = TreeNodeText::ALL
        .iter()
        .map(|&v| s.tree_node_option(v))
        .collect();
    let tree_nodes = difference.choice(
        s,
        Control::TreeNodes,
        &options,
        position(&TreeNodeText::ALL, &initial.tree_node_text),
    );
    let warn_missing =
        difference.check_box(s, Control::WarnMissingEntry, initial.warn_missing_entry);
    difference.finish(&notebook, s, Page::Difference);

    let export = Tab::new(&notebook);
    let options: Vec<&str> = CsvDelimiter::ALL
        .iter()
        .map(|&v| s.csv_delimiter_option(v))
        .collect();
    let csv_delimiter = export.choice(
        s,
        Control::CsvDelimiter,
        &options,
        position(&CsvDelimiter::ALL, &initial.csv_delimiter),
    );
    let open_after_export =
        export.check_box(s, Control::OpenAfterExport, initial.open_after_export);
    export.finish(&notebook, s, Page::Export);

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

    // The notebook takes the size of its largest tab (TASK-44 #6).
    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    sizer.add(&notebook, 1, SizerFlag::Expand | SizerFlag::All, 12);
    let commit_row = BoxSizer::builder(Orientation::Horizontal).build();
    commit_row.add(&ok, 0, SizerFlag::AlignLeft, 8);
    commit_row.add(&cancel, 0, SizerFlag::Left, 8);
    sizer.add_sizer(
        &commit_row,
        0,
        SizerFlag::AlignRight | SizerFlag::Left | SizerFlag::Right | SizerFlag::Bottom,
        12,
    );
    dialog.set_sizer_and_fit(sizer, true);

    let window = Rc::new(Window {
        notebook,
        language,
        theme,
        focus,
        comparison_focus,
        size_format,
        attribute_format,
        tree_nodes,
        confirm,
        other_users,
        warn_missing,
        csv_delimiter,
        open_after_export,
    });
    // Access keys by the key's position, so they work in the Ukrainian layout too
    // (document «Клавіші доступу»), and only those of the tab shown.
    let (shown, w) = (Rc::clone(&window), Rc::clone(&window));
    let targets = Page::ALL.into_iter().flat_map(targets_of).collect();
    access::bind_where(
        &dialog,
        targets,
        move |control| shown.page() == Some(control.page()),
        move |control| w.access_key(control),
    );

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
    /// The tab shown.
    fn page(&self) -> Option<Page> {
        let shown = usize::try_from(self.notebook.selection()).ok()?;
        Page::ALL.get(shown).copied()
    }

    /// Acts on the access key of `control`. The check box's key toggles it.
    fn access_key(&self, control: Control) {
        match control {
            Control::Language => self.language.set_focus(),
            Control::Theme => self.theme.set_focus(),
            Control::FocusAfterSnapshot => self.focus.set_focus(),
            Control::FocusAfterComparison => self.comparison_focus.set_focus(),
            Control::SizeFormat => self.size_format.set_focus(),
            Control::AttributeFormat => self.attribute_format.set_focus(),
            Control::TreeNodes => self.tree_nodes.set_focus(),
            Control::CsvDelimiter => self.csv_delimiter.set_focus(),
            Control::ConfirmDeletion => press(&self.confirm),
            Control::WarnMissingEntry => press(&self.warn_missing),
            Control::OtherUsersFolders => press(&self.other_users),
            Control::OpenAfterExport => press(&self.open_after_export),
        }
    }

    fn values(&self) -> Values {
        let language = self.language.get_selection().unwrap_or(0) as usize;
        let theme = self.theme.get_selection().unwrap_or(0) as usize;
        let chosen = self.focus.get_selection().unwrap_or(0) as usize;
        let comparison = self.comparison_focus.get_selection().unwrap_or(0) as usize;
        let size = self.size_format.get_selection().unwrap_or(0) as usize;
        let attributes = self.attribute_format.get_selection().unwrap_or(0) as usize;
        let tree_nodes = self.tree_nodes.get_selection().unwrap_or(0) as usize;
        let delimiter = self.csv_delimiter.get_selection().unwrap_or(0) as usize;
        Values {
            language: Language::ALL.get(language).copied().unwrap_or_default(),
            theme: Theme::ALL.get(theme).copied().unwrap_or_default(),
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
            tree_node_text: TreeNodeText::ALL
                .get(tree_nodes)
                .copied()
                .unwrap_or_default(),
            confirm_snapshot_deletion: self.confirm.get_value(),
            warn_missing_entry: self.warn_missing.get_value(),
            other_users_folders: self.other_users.get_value(),
            open_after_export: self.open_after_export.get_value(),
            csv_delimiter: CsvDelimiter::ALL
                .get(delimiter)
                .copied()
                .unwrap_or_default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{En, Uk};

    #[test]
    fn each_tab_holds_its_settings_in_tab_order() {
        // TASK-44 #1.
        let on = |page| -> Vec<Control> { targets_of(page).into_iter().map(|t| t.id).collect() };
        assert_eq!(on(Page::General), [Control::Language, Control::Theme]);
        assert_eq!(
            on(Page::Snapshots),
            [
                Control::FocusAfterSnapshot,
                Control::ConfirmDeletion,
                Control::OtherUsersFolders,
            ]
        );
        assert_eq!(
            on(Page::Difference),
            [
                Control::FocusAfterComparison,
                Control::SizeFormat,
                Control::AttributeFormat,
                Control::TreeNodes,
                Control::WarnMissingEntry,
            ]
        );
        // TASK-39 #9.
        assert_eq!(
            on(Page::Export),
            [Control::CsvDelimiter, Control::OpenAfterExport]
        );
        assert_eq!(Page::ALL[0], Page::General);
    }

    #[test]
    fn the_tabs_are_named_in_both_languages() {
        let uk: Vec<&str> = Page::ALL.iter().map(|&p| p.name(&Uk)).collect();
        assert_eq!(uk, ["Загальні", "Знімки", "Різниця", "Експорт"]);
        let en: Vec<&str> = Page::ALL.iter().map(|&p| p.name(&En)).collect();
        assert_eq!(en, ["General", "Snapshots", "Difference", "Export"]);
    }

    #[test]
    fn the_labels_mark_their_letters_in_both_languages() {
        assert_eq!(Control::Language.label(&En), "&Language:");
        assert_eq!(Control::Language.label(&Uk), "Мова (&L):");
        // TASK-34 #1: T is the tree nodes'.
        assert_eq!(Control::Theme.label(&En), "T&heme:");
        assert_eq!(Control::Theme.label(&Uk), "Тема (&H):");
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
        // TASK-33 #6.
        assert_eq!(Control::TreeNodes.label(&En), "&Tree nodes:");
        assert_eq!(Control::TreeNodes.label(&Uk), "Вузли дерева (&T):");
        assert_eq!(
            Control::WarnMissingEntry.label(&En),
            "&Warn when the entry is no longer on disk"
        );
        assert_eq!(
            Control::WarnMissingEntry.label(&Uk),
            "Попереджати, коли запису вже немає на диску (&W)"
        );
        // TASK-43 #5.
        assert_eq!(
            Control::OtherUsersFolders.label(&En),
            "Show &other users' folders in the typical folders list"
        );
        assert_eq!(
            Control::OtherUsersFolders.label(&Uk),
            "Показувати теки інших користувачів у переліку типових тек (&O)"
        );
        // TASK-39 #9: O again, on another tab.
        assert_eq!(
            Control::OpenAfterExport.label(&En),
            "&Open the file after export"
        );
        assert_eq!(
            Control::OpenAfterExport.label(&Uk),
            "Відкривати файл після експорту (&O)"
        );
        // TASK-40 #5.
        assert_eq!(Control::CsvDelimiter.label(&En), "CSV &delimiter:");
        assert_eq!(Control::CsvDelimiter.label(&Uk), "Роздільник у CSV (&D):");
    }

    #[test]
    fn the_csv_delimiter_options_are_named_in_both_languages_in_setting_order() {
        // TASK-40 #5; the first, as Windows has it, is the default.
        let uk: Vec<&str> = CsvDelimiter::ALL
            .iter()
            .map(|&v| Uk.csv_delimiter_option(v))
            .collect();
        assert_eq!(uk, ["Як у Windows", "Кома", "Крапка з комою", "Табуляція"]);
        let en: Vec<&str> = CsvDelimiter::ALL
            .iter()
            .map(|&v| En.csv_delimiter_option(v))
            .collect();
        assert_eq!(en, ["Same as Windows", "Comma", "Semicolon", "Tab"]);
        assert_eq!(CsvDelimiter::ALL[0], CsvDelimiter::default());
    }

    #[test]
    fn no_letter_repeats_on_one_tab() {
        for page in Page::ALL {
            let mut letters: Vec<u8> = targets_of(page).iter().filter_map(|t| t.letter()).collect();
            let all = letters.len();
            letters.sort_unstable();
            letters.dedup();
            assert_eq!(letters.len(), all, "{page:?}");
        }
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
    fn the_tree_node_options_are_named_in_both_languages_in_setting_order() {
        // TASK-33 #6; the first, the long text, is the default.
        let uk: Vec<&str> = TreeNodeText::ALL
            .iter()
            .map(|&v| Uk.tree_node_option(v))
            .collect();
        assert_eq!(uk, ["Ім'я, зміна й що змінилося", "Лише ім'я й зміна"]);
        let en: Vec<&str> = TreeNodeText::ALL
            .iter()
            .map(|&v| En.tree_node_option(v))
            .collect();
        assert_eq!(
            en,
            ["Name, change, and what changed", "Name and change only"]
        );
        assert_eq!(TreeNodeText::ALL[0], TreeNodeText::default());
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
    fn the_theme_options_are_named_in_both_languages_in_setting_order() {
        // TASK-34 #1; the first, as Windows is, is the default.
        let uk: Vec<&str> = Theme::ALL.iter().map(|&v| Uk.theme_option(v)).collect();
        assert_eq!(uk, ["Як у Windows", "Світла", "Темна"]);
        let en: Vec<&str> = Theme::ALL.iter().map(|&v| En.theme_option(v)).collect();
        assert_eq!(en, ["Same as Windows", "Light", "Dark"]);
        assert_eq!(Theme::ALL[0], Theme::default());
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
