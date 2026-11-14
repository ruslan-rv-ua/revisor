//! The settings window (TASK-9): the application's own settings (ADR-0007) that have a
//! control of their own — the interface language (TASK-16), its theme (TASK-34), what the
//! focus does when a snapshot ends, what it does when a comparison ends (TASK-11), how sizes
//! are written (TASK-13), how the details of a change write attributes (TASK-14), what the
//! nodes of the difference tree say (TASK-33), whether deleting snapshots asks first,
//! whether showing a change no longer on disk in File Explorer asks first (TASK-14),
//! whether the status bar names the path copied (TASK-41), and whether the list of typical
//! folders offers other users' folders (TASK-43), whether a snapshot that rights closed
//! in part offers a restart as administrator (TASK-48), and which columns the difference
//! list shows in which order (TASK-49). The
//! profile opened last and the view of the difference — its types, order, list or tree — are
//! kept without a control. The settings lie on tabs (TASK-44): «Загальні», «Знімки»,
//! «Різниця» and «Експорт», which holds whether the exported file opens (TASK-39) and what
//! separates the values of CSV (TASK-40).

use std::cell::RefCell;
use std::rc::Rc;

use revisor_core::settings::{
    AttributeFormat, CsvDelimiter, DifferenceColumn, DifferenceColumns, FocusAfterComparison,
    FocusAfterSnapshot, Language, SizeFormat, Theme, TreeNodeText,
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
    pub show_copied_path: bool,
    pub other_users_folders: bool,
    pub open_after_export: bool,
    pub csv_delimiter: CsvDelimiter,
    pub sound_after_snapshot: bool,
    pub sound_after_snapshot_and_compare: bool,
    pub sound_only_after_long: bool,
    pub speak_status: bool,
    pub offer_restart: bool,
    pub difference_columns: DifferenceColumns,
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
    SoundAfterSnapshot,
    SoundAfterSnapshotAndCompare,
    SoundOnlyAfterLong,
    SpeakStatus,
    FocusAfterSnapshot,
    ConfirmDeletion,
    OtherUsersFolders,
    OfferRestart,
    FocusAfterComparison,
    SizeFormat,
    AttributeFormat,
    TreeNodes,
    DifferenceColumns,
    MoveUp,
    MoveDown,
    Defaults,
    WarnMissingEntry,
    ShowCopiedPath,
    CsvDelimiter,
    OpenAfterExport,
}

impl Control {
    /// In tab order: tab by tab, as each tab shows them.
    const ALL: [Control; 22] = [
        Self::Language,
        Self::Theme,
        Self::SoundAfterSnapshot,
        Self::SoundAfterSnapshotAndCompare,
        Self::SoundOnlyAfterLong,
        Self::SpeakStatus,
        Self::FocusAfterSnapshot,
        Self::ConfirmDeletion,
        Self::OtherUsersFolders,
        Self::OfferRestart,
        Self::FocusAfterComparison,
        Self::SizeFormat,
        Self::AttributeFormat,
        Self::TreeNodes,
        Self::DifferenceColumns,
        Self::MoveUp,
        Self::MoveDown,
        Self::Defaults,
        Self::WarnMissingEntry,
        Self::ShowCopiedPath,
        Self::CsvDelimiter,
        Self::OpenAfterExport,
    ];

    /// The tab the control is on.
    fn page(self) -> Page {
        match self {
            Self::Language
            | Self::Theme
            | Self::SoundAfterSnapshot
            | Self::SoundAfterSnapshotAndCompare
            | Self::SoundOnlyAfterLong
            | Self::SpeakStatus => Page::General,
            Self::FocusAfterSnapshot
            | Self::ConfirmDeletion
            | Self::OtherUsersFolders
            | Self::OfferRestart => Page::Snapshots,
            Self::FocusAfterComparison
            | Self::SizeFormat
            | Self::AttributeFormat
            | Self::TreeNodes
            | Self::DifferenceColumns
            | Self::MoveUp
            | Self::MoveDown
            | Self::Defaults
            | Self::WarnMissingEntry
            | Self::ShowCopiedPath => Page::Difference,
            Self::CsvDelimiter | Self::OpenAfterExport => Page::Export,
        }
    }

    /// The name of the control's label or check box, whose English marks its access key.
    fn name(self) -> Name {
        match self {
            Self::Language => |s| s.language(),
            Self::Theme => |s| s.theme(),
            Self::SoundAfterSnapshot => |s| s.sound_after_snapshot(),
            Self::SoundAfterSnapshotAndCompare => |s| s.sound_after_snapshot_and_compare(),
            Self::SoundOnlyAfterLong => |s| s.sound_only_after_long(),
            Self::SpeakStatus => |s| s.speak_status(),
            Self::FocusAfterSnapshot => |s| s.focus_after_snapshot(),
            Self::FocusAfterComparison => |s| s.focus_after_comparison(),
            Self::SizeFormat => |s| s.size_format(),
            Self::AttributeFormat => |s| s.attribute_format(),
            Self::TreeNodes => |s| s.tree_nodes(),
            Self::DifferenceColumns => |s| s.difference_columns(),
            Self::MoveUp => |s| s.move_up(),
            Self::MoveDown => |s| s.move_down(),
            Self::Defaults => |s| s.defaults(),
            Self::ConfirmDeletion => |s| s.confirm_snapshot_deletion(),
            Self::WarnMissingEntry => |s| s.warn_missing_entry(),
            Self::ShowCopiedPath => |s| s.show_copied_path(),
            Self::OtherUsersFolders => |s| s.other_users_folders(),
            Self::OfferRestart => |s| s.offer_restart(),
            Self::CsvDelimiter => |s| s.csv_delimiter(),
            Self::OpenAfterExport => |s| s.open_after_export(),
        }
    }

    /// Whether the control is a check box, which its access key toggles, as an access key
    /// on a check box does in Windows; the others are lists that take the focus.
    fn is_check_box(self) -> bool {
        matches!(
            self,
            Self::SoundAfterSnapshot
                | Self::SoundAfterSnapshotAndCompare
                | Self::SoundOnlyAfterLong
                | Self::SpeakStatus
                | Self::ConfirmDeletion
                | Self::WarnMissingEntry
                | Self::ShowCopiedPath
                | Self::OtherUsersFolders
                | Self::OfferRestart
                | Self::OpenAfterExport
        )
    }

    /// Whether the control is a button, which its access key presses (TASK-49).
    fn is_button(self) -> bool {
        matches!(self, Self::MoveUp | Self::MoveDown | Self::Defaults)
    }

    /// The text of the control's label, check box or button: "&Focus after a snapshot:",
    /// "Підтверджувати видалення знімків (&C)", "Вгору (&U)".
    fn label(self, s: &dyn Strings) -> String {
        if self.is_check_box() || self.is_button() {
            access::label(s, self.name())
        } else {
            field_label(s, self.name())
        }
    }

    /// The access key: a check box or a button is pressed, a list takes the focus.
    fn target(self) -> Target<Control> {
        let act = if self.is_check_box() || self.is_button() {
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
    columns: Rc<ColumnList>,
    confirm: CheckBox,
    other_users: CheckBox,
    warn_missing: CheckBox,
    show_copied_path: CheckBox,
    csv_delimiter: Choice,
    open_after_export: CheckBox,
    sound_after_snapshot: CheckBox,
    sound_after_snapshot_and_compare: CheckBox,
    sound_only_after_long: CheckBox,
    speak_status: CheckBox,
    offer_restart: CheckBox,
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

    /// Adds the list «Колонки списку різниці» with a check box for each column, in the
    /// order of `initial`, and its buttons «Вгору», «Вниз», «Типові» in a row below it
    /// (TASK-49 #1).
    fn column_list(&self, s: &'static dyn Strings, initial: DifferenceColumns) -> Rc<ColumnList> {
        let label = StaticText::builder(&self.panel)
            .with_label(&Control::DifferenceColumns.label(s))
            .build();
        let list = ListCtrl::builder(&self.panel)
            .with_style(ListCtrlStyle::Report | ListCtrlStyle::NoHeader | ListCtrlStyle::SingleSel)
            .build();
        let hwnd = list.get_handle();
        win32::lists::enable_check_boxes(hwnd);
        list.insert_column(0, "", ListColumnFormat::Left, 240);
        for i in 0..initial.choices().len() {
            list.insert_item(i as i64, "", None);
        }
        let kept = win32::lists::keep_checked(self.panel.get_handle(), hwnd, name_row(&initial));
        let columns = Rc::new(ColumnList {
            s,
            list,
            columns: RefCell::new(initial),
            kept,
        });
        columns.fill();
        win32::lists::select_only(hwnd, 0);
        let button = |control: Control| {
            Button::builder(&self.panel)
                .with_label(&control.label(s))
                .build()
        };
        let (up, down, defaults) = (
            button(Control::MoveUp),
            button(Control::MoveDown),
            button(Control::Defaults),
        );
        let w = Rc::clone(&columns);
        up.on_click(move |_| w.move_focused(true));
        let w = Rc::clone(&columns);
        down.on_click(move |_| w.move_focused(false));
        let w = Rc::clone(&columns);
        defaults.on_click(move |_| w.reset());

        let side = SizerFlag::Left | SizerFlag::Right;
        self.sizer.add(&label, 0, side | SizerFlag::Top, 12);
        self.sizer.add_spacer(4);
        // Six rows without a scroll bar.
        list.set_min_size(Size::new(-1, 130));
        self.sizer.add(&list, 0, SizerFlag::Expand | side, 12);
        let row = BoxSizer::builder(Orientation::Horizontal).build();
        row.add(&up, 0, SizerFlag::Right, 8);
        row.add(&down, 0, SizerFlag::Right, 8);
        row.add(&defaults, 0, SizerFlag::Right, 8);
        self.sizer.add_sizer(&row, 0, side | SizerFlag::Top, 12);
        win32::lists::fill_width(hwnd, 0);
        columns
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

/// The list «Колонки списку різниці» (TASK-49): a row with a check box for each column,
/// in the order the difference list shows them. The order and the widths are kept apart,
/// the check boxes say which are shown.
struct ColumnList {
    s: &'static dyn Strings,
    list: ListCtrl,
    columns: RefCell<DifferenceColumns>,
    /// The check box of «Ім'я», which cannot be unchecked (#3).
    kept: Rc<win32::lists::KeptChecked>,
}

/// The row of «Ім'я» among `columns`.
fn name_row(columns: &DifferenceColumns) -> usize {
    columns
        .choices()
        .iter()
        .position(|c| c.column == DifferenceColumn::Name)
        .unwrap_or(0)
}

/// The text of the row of `column`: its name, and for «Ім'я» that it is always shown:
/// "Ім'я (завжди видно)" (TASK-49 #3).
fn column_row_text(s: &dyn Strings, column: DifferenceColumn) -> String {
    let name = s.column(column);
    if column.can_hide() {
        name.to_string()
    } else {
        s.always_shown(name)
    }
}

impl ColumnList {
    /// Writes the rows as the columns are now: their texts and check boxes.
    fn fill(&self) {
        let rows = 0..self.columns.borrow().choices().len();
        self.fill_rows(rows);
    }

    /// Writes the rows `rows` as the columns are now.
    fn fill_rows(&self, rows: impl Iterator<Item = usize>) {
        let hwnd = self.list.get_handle();
        let columns = self.columns.borrow();
        // The lock moves first, so that the row «Ім'я» leaves can be unchecked.
        self.kept.set_row(name_row(&columns));
        for i in rows {
            let choice = columns.choices()[i];
            self.list
                .set_item_text(i as i64, &column_row_text(self.s, choice.column));
            win32::lists::set_checked(hwnd, i, choice.shown);
        }
    }

    /// Takes the check boxes into the columns.
    fn take_checks(&self) {
        let hwnd = self.list.get_handle();
        let mut columns = self.columns.borrow_mut();
        for i in 0..columns.choices().len() {
            columns.set_shown(i, win32::lists::is_checked(hwnd, i));
        }
    }

    /// The columns as the user chose them.
    fn chosen(&self) -> DifferenceColumns {
        self.take_checks();
        *self.columns.borrow()
    }

    /// «Вгору» or «Вниз»: moves the column of the selected row one place, and the
    /// selection with it (#4). The keyboard focus stays where it is: in the list, NVDA
    /// then reads the row in its new place.
    fn move_focused(&self, up: bool) {
        let hwnd = self.list.get_handle();
        let Some(row) = win32::lists::focused_row(hwnd) else {
            return;
        };
        self.take_checks();
        let moved = {
            let mut columns = self.columns.borrow_mut();
            if up {
                columns.move_up(row)
            } else {
                columns.move_down(row)
            }
        };
        // The row the column goes to is written before the focus comes to it, and the row it
        // leaves after: a screen reader then reads the column once, in its new place, and
        // not the text that replaces it under the focus.
        if let Some(to) = moved {
            self.fill_rows(std::iter::once(to));
            win32::lists::select_only(hwnd, to);
            self.fill_rows(std::iter::once(row));
        }
    }

    /// «Типові»: all six columns, in their default order and widths (#5); the selection
    /// stays on its row.
    fn reset(&self) {
        *self.columns.borrow_mut() = DifferenceColumns::default();
        self.fill();
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
    let sound_after_snapshot =
        general.check_box(s, Control::SoundAfterSnapshot, initial.sound_after_snapshot);
    let sound_after_snapshot_and_compare = general.check_box(
        s,
        Control::SoundAfterSnapshotAndCompare,
        initial.sound_after_snapshot_and_compare,
    );
    let sound_only_after_long = general.check_box(
        s,
        Control::SoundOnlyAfterLong,
        initial.sound_only_after_long,
    );
    let speak_status = general.check_box(s, Control::SpeakStatus, initial.speak_status);
    // The threshold is about the sounds, so it is unavailable while both are off
    // (TASK-46 #2).
    let threshold_follows = {
        let (a, b, threshold) = (
            sound_after_snapshot,
            sound_after_snapshot_and_compare,
            sound_only_after_long,
        );
        move || threshold.enable(a.get_value() || b.get_value())
    };
    // The closure holds only the check boxes, which are handles, so it is copied.
    threshold_follows();
    sound_after_snapshot.on_toggled(move |_| threshold_follows());
    sound_after_snapshot_and_compare.on_toggled(move |_| threshold_follows());
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
    // Last on the tab (TASK-48 #1).
    let offer_restart = snapshots.check_box(s, Control::OfferRestart, initial.offer_restart);
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
    // After «Вузли дерева» (TASK-49 #1).
    let columns = difference.column_list(s, initial.difference_columns);
    let warn_missing =
        difference.check_box(s, Control::WarnMissingEntry, initial.warn_missing_entry);
    let show_copied_path =
        difference.check_box(s, Control::ShowCopiedPath, initial.show_copied_path);
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
        columns,
        confirm,
        other_users,
        warn_missing,
        show_copied_path,
        csv_delimiter,
        open_after_export,
        sound_after_snapshot,
        sound_after_snapshot_and_compare,
        sound_only_after_long,
        speak_status,
        offer_restart,
    });
    // Access keys by the key's position, so they work in the Ukrainian layout too
    // (document «Клавіші доступу»), and only those of the tab shown.
    // Enter in the list of columns activates its row, which the list keeps for itself
    // instead of pressing OK; it presses OK too. wx activates the row on Space as well,
    // which there only checks or unchecks it.
    let ok_dialog = dialog;
    window.columns.list.on_item_activated(move |_| {
        if !win32::focus::space_down() {
            ok_dialog.end_modal(ID_OK);
        }
    });
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
            Control::DifferenceColumns => self.columns.list.set_focus(),
            // The buttons act without the focus, so that it stays in the list (#4).
            Control::MoveUp => self.columns.move_focused(true),
            Control::MoveDown => self.columns.move_focused(false),
            Control::Defaults => self.columns.reset(),
            Control::CsvDelimiter => self.csv_delimiter.set_focus(),
            Control::ConfirmDeletion => press(&self.confirm),
            Control::WarnMissingEntry => press(&self.warn_missing),
            Control::ShowCopiedPath => press(&self.show_copied_path),
            Control::OtherUsersFolders => press(&self.other_users),
            Control::OfferRestart => press(&self.offer_restart),
            Control::OpenAfterExport => press(&self.open_after_export),
            Control::SoundAfterSnapshot => press(&self.sound_after_snapshot),
            Control::SoundAfterSnapshotAndCompare => press(&self.sound_after_snapshot_and_compare),
            Control::SoundOnlyAfterLong => {
                // A check box that is unavailable takes no key, as Windows has it.
                if self.sound_only_after_long.is_enabled() {
                    press(&self.sound_only_after_long);
                }
            }
            Control::SpeakStatus => press(&self.speak_status),
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
            show_copied_path: self.show_copied_path.get_value(),
            other_users_folders: self.other_users.get_value(),
            open_after_export: self.open_after_export.get_value(),
            csv_delimiter: CsvDelimiter::ALL
                .get(delimiter)
                .copied()
                .unwrap_or_default(),
            sound_after_snapshot: self.sound_after_snapshot.get_value(),
            sound_after_snapshot_and_compare: self.sound_after_snapshot_and_compare.get_value(),
            sound_only_after_long: self.sound_only_after_long.get_value(),
            speak_status: self.speak_status.get_value(),
            offer_restart: self.offer_restart.get_value(),
            difference_columns: self.columns.chosen(),
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
        // TASK-46 #1: the sounds and the screen reader after the theme.
        assert_eq!(
            on(Page::General),
            [
                Control::Language,
                Control::Theme,
                Control::SoundAfterSnapshot,
                Control::SoundAfterSnapshotAndCompare,
                Control::SoundOnlyAfterLong,
                Control::SpeakStatus,
            ]
        );
        assert_eq!(
            on(Page::Snapshots),
            [
                Control::FocusAfterSnapshot,
                Control::ConfirmDeletion,
                Control::OtherUsersFolders,
                // TASK-48 #1: last.
                Control::OfferRestart,
            ]
        );
        assert_eq!(
            on(Page::Difference),
            [
                Control::FocusAfterComparison,
                Control::SizeFormat,
                Control::AttributeFormat,
                Control::TreeNodes,
                // TASK-49 #1.
                Control::DifferenceColumns,
                Control::MoveUp,
                Control::MoveDown,
                Control::Defaults,
                Control::WarnMissingEntry,
                // TASK-41 #8.
                Control::ShowCopiedPath,
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
        // TASK-49 #10.
        assert_eq!(
            Control::DifferenceColumns.label(&En),
            "Difference list &columns:"
        );
        assert_eq!(
            Control::DifferenceColumns.label(&Uk),
            "Колонки списку різниці (&C):"
        );
        assert_eq!(Control::MoveUp.label(&En), "Move &up");
        assert_eq!(Control::MoveUp.label(&Uk), "Вгору (&U)");
        assert_eq!(Control::MoveDown.label(&En), "Move dow&n");
        assert_eq!(Control::MoveDown.label(&Uk), "Вниз (&N)");
        assert_eq!(Control::Defaults.label(&En), "&Defaults");
        assert_eq!(Control::Defaults.label(&Uk), "Типові (&D)");
        assert_eq!(
            Control::WarnMissingEntry.label(&En),
            "&Warn when the entry is no longer on disk"
        );
        assert_eq!(
            Control::WarnMissingEntry.label(&Uk),
            "Попереджати, коли запису вже немає на диску (&W)"
        );
        // TASK-41 #8: H, as S is the size's.
        assert_eq!(
            Control::ShowCopiedPath.label(&En),
            "S&how the copied path in the status bar"
        );
        assert_eq!(
            Control::ShowCopiedPath.label(&Uk),
            "Показувати скопійований шлях у рядку стану (&H)"
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
        // TASK-48 #1, #8: R, free on «Знімки».
        assert_eq!(
            Control::OfferRestart.label(&En),
            "Offer to &restart as administrator after a snapshot"
        );
        assert_eq!(
            Control::OfferRestart.label(&Uk),
            "Пропонувати перезапуск від імені адміністратора після знімка (&R)"
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
        // TASK-46 #1: L and H are the language's and the theme's.
        let pairs = [
            (
                Control::SoundAfterSnapshot,
                "Play a &sound after a snapshot",
                "Відтворювати звук після знімка (&S)",
            ),
            (
                Control::SoundAfterSnapshotAndCompare,
                "Play a sound after Snapshot and &compare",
                "Відтворювати звук після «Знімок + порівняння» (&C)",
            ),
            (
                Control::SoundOnlyAfterLong,
                "&Only after operations longer than 10 seconds",
                "Лише після операцій, довших за 10 секунд (&O)",
            ),
            (
                Control::SpeakStatus,
                "Pass status bar messages to the screen &reader",
                "Передавати скрінрідеру повідомлення рядка стану (&R)",
            ),
        ];
        for (control, en, uk) in pairs {
            assert_eq!(control.label(&En), en);
            assert_eq!(control.label(&Uk), uk);
        }
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

    #[test]
    fn the_column_rows_say_that_the_name_is_always_shown() {
        // TASK-49 #3.
        let uk: Vec<String> = DifferenceColumn::ALL
            .iter()
            .map(|&c| column_row_text(&Uk, c))
            .collect();
        assert_eq!(
            uk,
            [
                "Ім'я (завжди видно)",
                "Зміна",
                "Що змінилося",
                "Розмір",
                "Дата зміни",
                "Тека"
            ]
        );
        assert_eq!(
            column_row_text(&En, DifferenceColumn::Name),
            "Name (always shown)"
        );
        let mut columns = DifferenceColumns::default();
        assert_eq!(name_row(&columns), 0);
        columns.move_down(0);
        assert_eq!(name_row(&columns), 1);
    }
}
