//! The main window: the menu bar and context menus built from the command table, the
//! status bar that speaks without moving the focus, the key handler, the two parts of
//! the window, the snapshot being taken, with its progress and its outcome (TASK-8), the
//! snapshot list of the open profile with its comments, deletion and the compaction
//! that follows it (TASK-9), the comparison of the current snapshot with another, made
//! in the background, whose difference the difference list shows (TASK-11) with the field
//! «Порівняння» above it and its filter under it (TASK-13), and «Знімок + порівняння»,
//! which chains a snapshot and its comparison (TASK-12). The profile box lists the
//! profiles (TASK-7).

use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::io;
use std::path::{Path, PathBuf};
use std::ptr;
use std::rc::Rc;
use std::thread;
use std::time::Duration;

use revisor_core::compare::{self, CompareError, Difference};
use revisor_core::job::{Lost, Outcome};
use revisor_core::profiles::{self, ProfileError, ProfileFile, Unreadable};
use revisor_core::settings::{
    DifferenceSort, FocusAfterComparison, FocusAfterSnapshot, Language, Settings,
};
use revisor_core::snapshot::{Phase, Report, SnapshotError};
use revisor_core::store::{
    Compacted, FolderState, ProfileSettings, SnapshotInfo, Store, StoreError, Unavailable,
};
use wxdragon::prelude::*;

use crate::commands::{COMMANDS, CommandId, CommandKind, MenuId, Pane, command};
use crate::compare_window;
use crate::difference::{self, Column, List, View};
use crate::failure;
use crate::help;
use crate::i18n::Strings;
use crate::jobs::{
    COMPARISON_SHOWN_AFTER, Ended, Event, Jobs, Kind, Shown, Superseded, ThenCompare,
};
use crate::keyboard::{Control, Focus, Route, route};
use crate::keys::Shortcut;
use crate::menus::{
    button_label, context_label, context_menu, field_label, menu_label, submenu_label,
    top_menu_label, window_title,
};
use crate::message_box::{ask_text, choose, confirm, tell};
use crate::profile_window::{self, Purpose};
use crate::settings_window::{self, Values};
use crate::snapshots::{
    Arrival, FocusPlace, arrival, arrival_after_comparison, row_after_deletion,
};
use crate::state::{AppState, ChangeType, SortColumn};
use crate::status::Announcer;
use crate::win32;

/// The commands that show or hide the types of the difference list's rows, in the order
/// of [`ChangeType::ALL`] (TASK-13 #22).
const TYPE_COMMANDS: [CommandId; 4] = [
    CommandId::ShowAdded,
    CommandId::ShowDeleted,
    CommandId::ShowModified,
    CommandId::ShowInaccessible,
];

/// The wx id of the first command; the others follow in table order.
const FIRST_COMMAND_ID: i32 = 1000;

/// How often the progress of a snapshot is shown, in milliseconds.
const PROGRESS_INTERVAL: i32 = 100;

/// How long a comment waits for the snapshot that is writing the profile's database
/// (TASK-9 #29): the save's commit is a matter of seconds.
const COMMENT_TIMEOUT: Duration = Duration::from_secs(60);

/// How long a revisor started while another compacts a profile waits between its looks
/// at the profile (TASK-9 #30).
const BUSY_RETRY: Duration = Duration::from_secs(1);

/// How long a comparison runs before its progress gets a bar with the percent done
/// (TASK-11 #8); the progress row itself shows it after [`crate::jobs::COMPARISON_SHOWN_AFTER`].
const COMPARISON_BAR_AFTER: Duration = Duration::from_secs(5);

/// How long the window waits between its looks at a comparison that still reads the
/// database it is about to write or delete (TASK-11 #17).
const HOLD_RETRY: Duration = Duration::from_millis(50);

/// The file of the application's settings in the data folder (ADR-0007).
pub const SETTINGS_FILE: &str = "settings.db";

/// The widths of the snapshot list's columns, which the «Порівняти з…» window takes too:
/// the name is fixed, the comment takes the rest.
pub const DATE_TIME_WIDTH: i32 = 190;
pub const COMMENT_WIDTH: i32 = 420;

/// The widths of the difference list's columns, in the order of [`Column::ALL`]: the
/// folder, the longest, comes last (TASK-13 #12).
const COLUMN_WIDTHS: [i32; 6] = [220, 140, 200, 90, 160, 420];

/// The height of the field «Порівняння»: the line of the pair, the summary and a warning
/// show at once; more warnings scroll (TASK-13 #1).
const COMPARISON_HEIGHT: i32 = 64;

/// How long the typing in the field «Фільтр:» pauses, in milliseconds, before the status
/// bar says how many rows the filter shows (TASK-13 #26): a message after every letter
/// would cut off the screen reader's echo of the typing.
const SHOWING_PAUSE: i32 = 1000;

/// The access keys of the labels of the main window. They do not take the letters of the
/// menus, which would no longer open while the focus is in the panel.
const PROFILE_ACCESS: u8 = b'R';
const PROGRESS_ACCESS: u8 = b'P';
/// The field «Порівняння», the field «Фільтр:» and the check boxes of the types under the
/// difference list, in the order of [`ChangeType::ALL`] (TASK-13).
const COMPARISON_ACCESS: u8 = b'M';
const FILTER_ACCESS: u8 = b'L';
const TYPE_ACCESS: [u8; 4] = *b"ATEB";

/// The access key of the comment field in the «Коментар» window.
const COMMENT_ACCESS: u8 = b'C';

/// «Скасувати (N)» next to the progress presses the command «Скасувати знімок» or, for
/// a comparison, «Скасувати порівняння», and «Скасувати стискання (O)» the command
/// «Скасувати стискання»: the button takes the letter of the command of the job it
/// shows.
fn cancel_snapshot_access() -> u8 {
    command(CommandId::CancelSnapshot).access
}

fn cancel_comparison_access() -> u8 {
    command(CommandId::CancelComparison).access
}

fn cancel_compaction_access() -> u8 {
    command(CommandId::CancelCompaction).access
}

/// The command that cancels a job of `kind`: what the progress row's button presses.
fn cancel_command(kind: Kind) -> CommandId {
    match kind {
        Kind::Snapshot => CommandId::CancelSnapshot,
        Kind::Comparison => CommandId::CancelComparison,
        Kind::Compaction => CommandId::CancelCompaction,
    }
}

/// The positions of the labels and buttons in [`MainWindow::labels`], which
/// `Route::FocusLabel` names them by; the check boxes of the types follow
/// `LABEL_TYPES`, one after another.
const LABEL_PROFILE: usize = 0;
const LABEL_PROGRESS: usize = 1;
const LABEL_CANCEL: usize = 2;
const LABEL_COMPARISON: usize = 3;
const LABEL_FILTER: usize = 4;
const LABEL_TYPES: usize = 5;

/// The access keys of the action buttons of questions: «Видалити», «Перезапустити» and
/// «Закрити».
const DELETE_ACCESS: u8 = b'D';
const RESTART_ACCESS: u8 = b'R';
const CLOSE_ACCESS: u8 = b'C';
/// The buttons of the question after the interface language changed (TASK-16).
const RESTART_NOW_ACCESS: u8 = b'R';
const LATER_ACCESS: u8 = b'L';

fn wx_id(id: CommandId) -> i32 {
    let index = COMMANDS
        .iter()
        .position(|c| c.id == id)
        .expect("every command id is in the table");
    FIRST_COMMAND_ID + index as i32
}

fn command_of(wx_id: i32) -> Option<CommandId> {
    let index = usize::try_from(wx_id - FIRST_COMMAND_ID).ok()?;
    COMMANDS.get(index).map(|c| c.id)
}

pub struct MainWindow {
    frame: Frame,
    s: &'static dyn Strings,
    state: RefCell<AppState>,
    menu_bar: MenuBar,
    status: StatusBar,
    /// Joins the messages of one event into one (TASK-21).
    announcer: Announcer,
    panel: Panel,
    profile: Choice,
    /// The progress of the job the progress row shows — a snapshot being taken, a
    /// comparison that has run for a second, a compaction — shown only while one is.
    progress_label: StaticText,
    progress_text: TextCtrl,
    /// The percent of a comparison done, shown once it has run for a while (TASK-11 #8).
    gauge: Gauge,
    /// Cancels the job the progress row shows.
    cancel_button: Button,
    snapshots: ListCtrl,
    /// The field «Порівняння» above the difference list: the pair of snapshots, the
    /// summary and the warnings, or, before the first comparison, how to compare
    /// (TASK-13 #1, #11).
    comparison_field: TextCtrl,
    /// The panel the difference lists take turns in: the one shown fills it, the other is
    /// disabled and has no size (TASK-13).
    difference_holder: Panel,
    /// The difference lists: two virtual lists over `difference_list`, which they ask for
    /// the text of the rows they show (TASK-11 #5), one of them shown at a time. A new
    /// difference goes into the other list, which then takes the shown one's place: the
    /// focus comes to it as to a new list, which screen readers name again even when the
    /// focus was in the old one (#21); nothing says so when the rows under the focus are
    /// merely replaced. Neither list is ever hidden: UI Automation goes through every row
    /// of a list that is shown after it was hidden, which keeps the window busy for
    /// seconds with a large difference (TASK-13 #29), so the other list is disabled — no
    /// stop for Tab — and has no size instead.
    differences: [ListCtrl; 2],
    /// Which of `differences` is shown.
    shown_difference: Cell<usize>,
    /// The model of the difference list (TASK-13).
    difference_list: Rc<RefCell<List>>,
    /// The field «Фільтр:» and the check boxes of the types under the difference list
    /// (TASK-13 #22, #24), which work once there is a difference.
    filter_field: TextCtrl,
    type_boxes: [CheckBox; 4],
    /// Says how many rows the filter shows once the typing in the field «Фільтр:» pauses
    /// (TASK-13 #26).
    showing_timer: Timer<Frame>,
    /// Polls the jobs.
    timer: Timer<Frame>,
    /// The background jobs: a snapshot being taken, a comparison being made and a
    /// database being compacted (ADR-0011).
    jobs: RefCell<Jobs>,
    /// The kind of the job the progress row showed when it was last refreshed, so a
    /// change of the job shown — a comparison that has run for a second — is noticed.
    shown_kind: Cell<Option<Kind>>,
    /// Whether the progress row has said, in the status bar, that the comparison shown
    /// waits for the database (`Some(true)`) or compares (`Some(false)`) (TASK-11 #7,
    /// #16); `None` while the row shows no comparison.
    said_waiting: Cell<Option<bool>>,
    /// The control that had the keyboard focus when the comparison was started — for
    /// «Знімок + порівняння», when the command was (TASK-12 #5) — for the setting
    /// «Фокус після порівняння» (TASK-11 #12).
    comparison_focus: Cell<*mut c_void>,
    /// The folder of profiles, `data\profiles` next to the exe.
    profiles_dir: PathBuf,
    /// The application's settings (ADR-0007), or the reason their file could not be used,
    /// in the interface's words: then the defaults stand, and changes are not kept.
    settings: Result<Settings, String>,
    /// The profiles in the order the profile box shows them.
    profiles: RefCell<Vec<ProfileFile>>,
    /// The open profile: its position in `profiles`.
    open_index: Cell<Option<usize>>,
    /// The open profile's snapshots, in the order of the snapshot list: oldest first.
    rows: RefCell<Vec<SnapshotInfo>>,
}

impl MainWindow {
    /// Opens the window on the profile in the file `open`, when there is one, else on the
    /// profile opened last time, else on the first profile; `data_dir` holds the profiles
    /// and the settings.
    pub fn open(
        s: &'static dyn Strings,
        state: AppState,
        data_dir: PathBuf,
        open: Option<PathBuf>,
    ) -> Rc<Self> {
        let frame = Frame::builder()
            .with_title(&window_title(state.profile.as_deref()))
            .with_size(Size::new(900, 640))
            .build();
        set_frame_icon(&frame);

        frame.set_menu_bar(build_menu_bar(s));
        let menu_bar = frame.get_menu_bar().expect("the menu bar was just set");
        let file_menu = MenuId::ALL
            .iter()
            .position(|&m| m == MenuId::File)
            .expect("File is a menu") as i32;
        win32::mark_with_shield(
            frame.get_handle(),
            file_menu,
            wx_id(CommandId::RestartAsAdmin),
        );
        let status = StatusBar::builder(&frame).build();
        frame.set_existing_status_bar(Some(&status));
        win32::make_live_region(status.get_handle());

        let settings_path = data_dir.join(SETTINGS_FILE);
        let settings =
            Settings::open(&settings_path).map_err(|e| s.failure(&failure::of_settings(&e)));
        // The view of the difference list and the size format, as kept (TASK-13 #17, #28).
        let view = settings
            .as_ref()
            .ok()
            .map(|settings| View {
                types: settings.difference_types().unwrap_or([true; 4]),
                sort: settings.difference_sort().unwrap_or_default(),
            })
            .unwrap_or_default();
        let size_format = settings
            .as_ref()
            .ok()
            .and_then(|settings| settings.size_format().ok())
            .unwrap_or_default();
        let difference_list = Rc::new(RefCell::new(List::new(
            s,
            Box::new(win32::RegionalFormat),
            view,
            size_format,
        )));

        let panel = Panel::builder(&frame).build();
        // Made in tab order. Each label comes right before its control, so screen readers
        // name the control after it.
        let profile_label = StaticText::builder(&panel)
            .with_label(&field_label(s, s.profile(), PROFILE_ACCESS))
            .build();
        let profile = Choice::builder(&panel).build();
        let progress_label = StaticText::builder(&panel)
            .with_label(&field_label(s, s.progress(), PROGRESS_ACCESS))
            .build();
        let progress_text = TextCtrl::builder(&panel)
            .with_style(TextCtrlStyle::ReadOnly)
            .build();
        let gauge = Gauge::builder(&panel).with_range(100).build();
        let cancel_button = Button::builder(&panel)
            .with_label(&button_label(
                s,
                s.cancel(),
                cancel_snapshot_access(),
                false,
            ))
            .build();
        let snapshots = ListCtrl::builder(&panel)
            .with_style(ListCtrlStyle::Report)
            .build();
        snapshots.insert_column(
            0,
            s.column_date_time(),
            ListColumnFormat::Left,
            DATE_TIME_WIDTH,
        );
        snapshots.insert_column(1, s.column_comment(), ListColumnFormat::Left, COMMENT_WIDTH);
        win32::full_row_select_no_sort(snapshots.get_handle());
        win32::set_accessible_name(snapshots.get_handle(), s.pane(Pane::Snapshots));
        let comparison_label = StaticText::builder(&panel)
            .with_label(&field_label(s, s.comparison_field(), COMPARISON_ACCESS))
            .build();
        // Read with the arrows a line at a time, so the lines do not wrap (TASK-13 #1).
        let comparison_field = TextCtrl::builder(&panel)
            .with_style(
                TextCtrlStyle::MultiLine | TextCtrlStyle::ReadOnly | TextCtrlStyle::DontWrap,
            )
            .build();
        comparison_field.set_min_size(Size::new(-1, COMPARISON_HEIGHT));
        // Virtual lists: a difference may have a million rows (TASK-11 #6), and a list
        // asks for the text of the ones it shows. Two of them in a panel of their own, the
        // second disabled and without a size at first.
        let difference_holder = Panel::builder(&panel).build();
        let differences = [(); 2].map(|()| {
            let list = ListCtrl::builder(&difference_holder)
                .with_style(
                    ListCtrlStyle::Report | ListCtrlStyle::Virtual | ListCtrlStyle::SingleSel,
                )
                .build();
            for (i, column) in Column::ALL.into_iter().enumerate() {
                let format = match column {
                    Column::Size => ListColumnFormat::Right,
                    _ => ListColumnFormat::Left,
                };
                list.insert_column(i as i64, s.column(column), format, COLUMN_WIDTHS[i]);
            }
            // The headers sort the list (TASK-13 #21).
            win32::full_row_select(list.get_handle());
            let model = Rc::clone(&difference_list);
            list.set_virtual_text_callback(move |row, column| {
                let row = usize::try_from(row).unwrap_or(usize::MAX);
                let column = usize::try_from(column)
                    .ok()
                    .and_then(|i| Column::ALL.get(i).copied());
                column.map_or_else(String::new, |column| model.borrow().cell(row, column))
            });
            list
        });
        differences[1].enable(false);
        // The holder answers for the difference lists' text while they are empty.
        win32::subclass_list_parent(difference_holder.get_handle());
        let filter_label = StaticText::builder(&panel)
            .with_label(&field_label(s, s.filter_field(), FILTER_ACCESS))
            .build();
        let filter_field = TextCtrl::builder(&panel).build();
        let type_boxes = ChangeType::ALL.map(|kind| {
            let name = s.command(TYPE_COMMANDS[kind as usize]);
            CheckBox::builder(&panel)
                .with_label(&button_label(s, name, TYPE_ACCESS[kind as usize], false))
                .build()
        });
        // The panel answers for the snapshot list's text while it is empty.
        win32::subclass_list_parent(panel.get_handle());

        let profile_row = BoxSizer::builder(Orientation::Horizontal).build();
        profile_row.add(
            &profile_label,
            0,
            SizerFlag::AlignCenterVertical | SizerFlag::All,
            5,
        );
        profile_row.add(&profile, 1, SizerFlag::All, 5);
        let progress_row = BoxSizer::builder(Orientation::Horizontal).build();
        progress_row.add(
            &progress_label,
            0,
            SizerFlag::AlignCenterVertical | SizerFlag::All,
            5,
        );
        progress_row.add(&progress_text, 1, SizerFlag::All, 5);
        progress_row.add(
            &gauge,
            1,
            SizerFlag::AlignCenterVertical | SizerFlag::All,
            5,
        );
        progress_row.add(&cancel_button, 0, SizerFlag::All, 5);
        let filter_row = BoxSizer::builder(Orientation::Horizontal).build();
        filter_row.add(
            &filter_label,
            0,
            SizerFlag::AlignCenterVertical | SizerFlag::All,
            5,
        );
        filter_row.add(&filter_field, 1, SizerFlag::All, 5);
        for check_box in &type_boxes {
            filter_row.add(
                check_box,
                0,
                SizerFlag::AlignCenterVertical | SizerFlag::All,
                5,
            );
        }
        let sizer = BoxSizer::builder(Orientation::Vertical).build();
        sizer.add_sizer(&profile_row, 0, SizerFlag::Expand, 0);
        sizer.add_sizer(&progress_row, 0, SizerFlag::Expand, 0);
        sizer.add(&snapshots, 1, SizerFlag::Expand | SizerFlag::All, 5);
        sizer.add(
            &comparison_label,
            0,
            SizerFlag::Left | SizerFlag::Right | SizerFlag::Top,
            5,
        );
        sizer.add(&comparison_field, 0, SizerFlag::Expand | SizerFlag::All, 5);
        sizer.add(&difference_holder, 2, SizerFlag::Expand | SizerFlag::All, 5);
        sizer.add_sizer(&filter_row, 0, SizerFlag::Expand, 0);
        panel.set_sizer(sizer, true);
        let timer = Timer::new(&frame);
        let showing_timer = Timer::new(&frame);

        let window = Rc::new(Self {
            frame,
            s,
            state: RefCell::new(state),
            menu_bar,
            status,
            announcer: Announcer::default(),
            panel,
            profile,
            progress_label,
            progress_text,
            gauge,
            cancel_button,
            snapshots,
            comparison_field,
            difference_holder,
            differences,
            shown_difference: Cell::new(0),
            difference_list,
            filter_field,
            type_boxes,
            showing_timer,
            timer,
            jobs: RefCell::new(Jobs::default()),
            shown_kind: Cell::new(None),
            said_waiting: Cell::new(None),
            comparison_focus: Cell::new(ptr::null_mut()),
            profiles_dir: data_dir.join("profiles"),
            settings,
            profiles: RefCell::new(Vec::new()),
            open_index: Cell::new(None),
            rows: RefCell::new(Vec::new()),
        });
        window.refresh_progress_row();
        window.bind_events();
        win32::subclass_main_window(window.frame.get_handle());
        window.load_profiles(open.as_deref());
        if let Err(reason) = &window.settings {
            window.say(&s.settings_not_opened(&file_name(&settings_path), reason));
        }
        window.frame.centre();
        window.frame.show(true);
        window
    }

    fn bind_events(self: &Rc<Self>) {
        let w = Rc::clone(self);
        self.frame.on_char_hook(move |data| {
            if let WindowEventData::Keyboard(key) = data {
                let shortcut = Shortcut::from_event(&key);
                let routed = route(shortcut, w.focus(), &w.state.borrow(), &w.labels());
                if routed == Route::Pass {
                    key.event.skip(true);
                } else {
                    key.event.skip(false);
                    w.apply(routed);
                }
            }
        });

        // A newly shown frame ends up with the focus on itself once Windows has activated
        // it; the snapshot list should have it. The first idle event comes after that.
        // Later activations are left to wx, which restores whatever had the focus last.
        let w = Rc::clone(self);
        let focus_pending = Cell::new(true);
        self.frame.on_idle(move |data| {
            if focus_pending.replace(false) {
                w.snapshots.set_focus();
            }
            if let WindowEventData::Idle(event) = data {
                event.event.skip(true);
            }
        });

        let w = Rc::clone(self);
        self.frame.on_menu(move |event| {
            if let Some(id) = command_of(event.get_id()) {
                w.run(id);
            }
        });
        let w = Rc::clone(self);
        self.frame.on_menu_opened(move |event| {
            w.refresh_menus();
            event.skip(true);
        });
        // Not skipped, so wxFrame does not put the item's help text in the status bar,
        // where it would overwrite the last message.
        self.frame.on_menu_highlighted(|event| event.skip(false));

        // Closing during a snapshot cancels it, so it asks first (TASK-8 #26); a close
        // that cannot be refused, as at a shutdown, stops the snapshot without asking. A
        // comparison is cancelled without a question and not waited for (TASK-11 #18).
        // A compaction goes on behind the hidden window, and the process ends with it
        // (TASK-9 #30); a close that cannot be refused interrupts it instead.
        let w = Rc::clone(self);
        self.frame.on_close(move |data| {
            if let WindowEventData::General(event) = data {
                let s = w.s;
                let can_veto = event.can_veto();
                let may_close = if can_veto {
                    w.stop_snapshot_if_agreed(
                        s.command(CommandId::Exit),
                        s.close_during_snapshot(),
                        (s.close(), CLOSE_ACCESS),
                    )
                } else {
                    w.stop_snapshot();
                    true
                };
                if may_close {
                    w.jobs.borrow().cancel(Kind::Comparison);
                    w.finish_compaction_hidden(!can_veto);
                    event.skip(true);
                } else {
                    event.veto();
                }
            }
        });

        let w = Rc::clone(self);
        self.timer.on_tick(move |_| w.poll_jobs());

        let w = Rc::clone(self);
        self.cancel_button.on_click(move |_| {
            if let Some(kind) = w.shown_kind() {
                w.run(cancel_command(kind));
            }
        });

        let w = Rc::clone(self);
        self.profile.on_selection_changed(move |event| {
            let selected = event.get_selection().and_then(|i| usize::try_from(i).ok());
            w.show_profile(selected);
        });

        // Deletion acts on the selected snapshots, and a comparison on the current one, so
        // the commands know how many there are and which is current.
        let w = Rc::clone(self);
        self.snapshots.on_item_selected(move |_| w.sync_selection());
        let w = Rc::clone(self);
        self.snapshots
            .on_item_deselected(move |_| w.sync_selection());
        let w = Rc::clone(self);
        self.snapshots.on_item_focused(move |_| w.sync_selection());

        // The list shown fills the panel of the difference lists.
        let w = Rc::clone(self);
        self.difference_holder.on_size(move |data| {
            w.place_difference_lists();
            if let WindowEventData::Size(event) = data {
                event.event.skip(true);
            }
        });

        // The difference list (TASK-13): its headers sort it, the focus picks the one row
        // selected, and its filter's field and check boxes change what it shows.
        for (i, list) in self.differences.iter().enumerate() {
            let w = Rc::clone(self);
            list.on_column_click(move |event| {
                if let Some(column) = event.get_column() {
                    w.sort_by_column(column);
                }
            });
            let w = Rc::clone(self);
            list.on_item_focused(move |event| w.difference_focused(i, event.get_item_index()));
        }
        let w = Rc::clone(self);
        self.filter_field.on_text_changed(move |_| w.filter_typed());
        for kind in ChangeType::ALL {
            let w = Rc::clone(self);
            self.type_boxes[kind as usize].on_toggled(move |_| w.type_toggled(kind));
        }
        let w = Rc::clone(self);
        self.showing_timer.on_tick(move |_| {
            let text = w.difference_list.borrow().showing();
            w.say(&text);
        });

        // A right click: the list under the mouse has the focus by then.
        let w = Rc::clone(self);
        self.panel.on_context_menu(move |event| {
            let focus = w.focus();
            match (focus.control, focus.part) {
                (Control::MainList, Some(pane)) => {
                    let at = event
                        .get_context_position()
                        .filter(|p| p.x != -1 || p.y != -1);
                    w.show_context_menu(pane, at.map(|p| (p.x, p.y)));
                }
                _ => event.skip(true),
            }
        });
    }

    /// The access keys of the main window's labels and buttons, for `keyboard::route`, at
    /// the positions `LABEL_*`: those of the progress row only while it is shown, with
    /// the Cancel button's letter being that of the command of the job shown; a place
    /// left empty holds 0, which is no letter.
    fn labels(&self) -> Vec<u8> {
        let (progress, cancel) = match self.shown_kind() {
            Some(kind) => (PROGRESS_ACCESS, command(cancel_command(kind)).access),
            None => (0, 0),
        };
        let mut labels = vec![
            PROFILE_ACCESS,
            progress,
            cancel,
            COMPARISON_ACCESS,
            FILTER_ACCESS,
        ];
        labels.extend(TYPE_ACCESS);
        labels
    }

    /// The kind of the job the progress row shows, if it shows one.
    fn shown_kind(&self) -> Option<Kind> {
        self.jobs.borrow().shown().map(|shown| shown.kind())
    }

    /// Where the keyboard focus is, in the terms of `keyboard::route`.
    fn focus(&self) -> Focus {
        let focused = win32::focused_window();
        if focused == self.snapshots.get_handle() {
            Focus {
                part: Some(Pane::Snapshots),
                control: Control::MainList,
            }
        } else if self.is_difference(focused) {
            Focus {
                part: Some(Pane::Difference),
                control: Control::MainList,
            }
        } else if focused == self.profile.get_handle() {
            Focus {
                part: Some(Pane::Snapshots),
                control: Control::Other,
            }
        } else if focused == self.filter_field.get_handle() {
            Focus {
                part: Some(Pane::Difference),
                control: Control::FilterField,
            }
        } else if focused == self.comparison_field.get_handle() {
            Focus {
                part: Some(Pane::Difference),
                control: Control::TextEntry,
            }
        } else if self.type_boxes.iter().any(|b| b.get_handle() == focused) {
            Focus {
                part: Some(Pane::Difference),
                control: Control::Other,
            }
        } else if !focused.is_null() && win32::is_text_field(focused) {
            Focus {
                part: None,
                control: Control::TextEntry,
            }
        } else {
            Focus {
                part: None,
                control: Control::Other,
            }
        }
    }

    /// Where the keyboard focus is, as far as a snapshot's arrival cares.
    fn focus_place(&self) -> FocusPlace {
        let focused = win32::focused_window();
        if focused == self.snapshots.get_handle() {
            FocusPlace::SnapshotList
        } else if focused == self.progress_text.get_handle()
            || focused == self.cancel_button.get_handle()
        {
            FocusPlace::SnapshotProgress
        } else {
            FocusPlace::Elsewhere
        }
    }

    /// Where the keyboard focus is, as far as a difference's arrival cares (TASK-11 #12,
    /// #21): in the difference list, still where it was when the comparison started (or
    /// «Знімок + порівняння», TASK-12 #5), on the progress row, or elsewhere.
    fn comparison_focus_place(&self) -> difference::FocusPlace {
        let focused = win32::focused_window();
        if self.is_difference(focused) {
            difference::FocusPlace::DifferenceList
        } else if focused == self.comparison_focus.get() {
            difference::FocusPlace::Unmoved
        } else if focused == self.progress_text.get_handle()
            || focused == self.cancel_button.get_handle()
        {
            difference::FocusPlace::Progress
        } else {
            difference::FocusPlace::Elsewhere
        }
    }

    fn list(&self, pane: Pane) -> &ListCtrl {
        match pane {
            Pane::Snapshots => &self.snapshots,
            Pane::Difference => self.difference(),
        }
    }

    /// The difference list shown.
    fn difference(&self) -> &ListCtrl {
        &self.differences[self.shown_difference.get()]
    }

    /// Whether `hwnd` is a difference list, shown or not.
    fn is_difference(&self, hwnd: *mut c_void) -> bool {
        self.differences
            .iter()
            .any(|list| list.get_handle() == hwnd)
    }

    fn apply(&self, routed: Route) {
        let s = self.s;
        match routed {
            Route::Run(id) => self.run(id),
            Route::Refuse(id, reason) => self.say(&s.unavailable(s.command(id), s.reason(reason))),
            Route::OnlyIn(key, pane) => self.say(&s.key_only_in(&key.name(s), pane)),
            Route::FocusPane(pane) => self.list(pane).set_focus(),
            Route::FocusLabel(LABEL_PROFILE) => self.profile.set_focus(),
            Route::FocusLabel(LABEL_PROGRESS) => self.progress_text.set_focus(),
            Route::FocusLabel(LABEL_CANCEL) => {
                if let Some(kind) = self.shown_kind() {
                    self.run(cancel_command(kind));
                }
            }
            Route::FocusLabel(LABEL_COMPARISON) => self.comparison_field.set_focus(),
            // The filter's controls work once there is a difference; before that their
            // letters say why not, as the commands do.
            Route::FocusLabel(LABEL_FILTER) => self.run(CommandId::FilterByText),
            Route::FocusLabel(label) if (LABEL_TYPES..LABEL_TYPES + 4).contains(&label) => {
                let i = label - LABEL_TYPES;
                self.run(TYPE_COMMANDS[i]);
                if self.type_boxes[i].is_enabled() {
                    self.type_boxes[i].set_focus();
                }
            }
            Route::FocusLabel(_) => {}
            Route::ClearFilter => self.speaking(|| self.clear_filter()),
            Route::ContextMenu(pane) => self.show_context_menu(pane, None),
            Route::SelectAll(pane) => win32::select_all(self.list(pane).get_handle()),
            Route::Pass => {}
        }
    }

    /// Says a message in the status bar; screen readers read it without the focus
    /// moving. During a command or a tick of the jobs' timer the message waits for the
    /// event's end and is shown with the others of that event, as one (TASK-21).
    pub fn say(&self, text: &str) {
        if let Some(text) = self.announcer.say(text) {
            self.show(&text);
        }
    }

    fn show(&self, text: &str) {
        self.status.set_status_text(text, 0);
        win32::announce_status(self.status.get_handle());
    }

    /// Runs an event whose messages are said together, once it is over: the messages of
    /// one moment would otherwise replace each other before a screen reader read them.
    fn speaking<T>(&self, event: impl FnOnce() -> T) -> T {
        let (answer, text) = self.announcer.gather(event);
        if let Some(text) = text {
            self.show(&text);
        }
        answer
    }

    /// Runs a command, then looks at the jobs: one that ended meanwhile, under the
    /// command's dialog or while the command waited for its write, is reported together
    /// with the command's own outcome (TASK-21).
    fn run(&self, id: CommandId) {
        self.speaking(|| {
            if self.run_command(id) {
                self.poll_jobs();
            }
        });
    }

    /// Runs the command; returns whether the window goes on, which it does not after
    /// «Вихід».
    fn run_command(&self, id: CommandId) -> bool {
        let s = self.s;
        let available = (command(id).available)(&self.state.borrow());
        if let Err(reason) = available {
            self.say(&s.unavailable(s.command(id), s.reason(reason)));
            return true;
        }
        match id {
            CommandId::Exit => {
                self.frame.close(false);
                return false;
            }
            CommandId::NewProfile => self.new_profile(),
            CommandId::EditProfile => self.edit_profile(),
            CommandId::DeleteProfile => self.delete_profile(),
            CommandId::Settings => self.settings(),
            CommandId::RestartAsAdmin => self.restart_as_admin(),
            CommandId::TakeSnapshot => self.take_snapshot(),
            CommandId::SnapshotAndCompare => self.snapshot_and_compare(),
            CommandId::Comment => self.comment(),
            CommandId::DeleteSnapshots => self.delete_snapshots(),
            CommandId::CancelSnapshot => self.cancel_snapshot(),
            CommandId::CancelCompaction => self.cancel_compaction(),
            CommandId::CompareWithPrevious => self.compare_with_previous(),
            CommandId::CompareWith => self.compare_with(),
            CommandId::CancelComparison => self.cancel_comparison(),
            CommandId::FilterByText => self.filter_by_text(),
            CommandId::ShowAdded => self.toggle_type(ChangeType::Added),
            CommandId::ShowDeleted => self.toggle_type(ChangeType::Deleted),
            CommandId::ShowModified => self.toggle_type(ChangeType::Modified),
            CommandId::ShowInaccessible => self.toggle_type(ChangeType::Inaccessible),
            CommandId::SortByPath => self.sort_by(SortColumn::Path),
            CommandId::SortByName => self.sort_by(SortColumn::Name),
            CommandId::SortByChange => self.sort_by(SortColumn::Change),
            CommandId::SortByWhatChanged => self.sort_by(SortColumn::WhatChanged),
            CommandId::SortBySize => self.sort_by(SortColumn::Size),
            CommandId::SortByDateModified => self.sort_by(SortColumn::DateModified),
            CommandId::SortByFolder => self.sort_by(SortColumn::Folder),
            CommandId::SortAscending => self.sort_direction(false),
            CommandId::SortDescending => self.sort_direction(true),
            CommandId::KeyboardShortcuts => help::show_shortcuts(&self.frame, s),
            CommandId::About => help::show_about(&self.frame, s),
            // Only the demo state gets here: TASK-14's commands are not done yet.
            CommandId::Details | CommandId::ShowInExplorer => {
                self.say(&s.demo_done(s.command(id)));
            }
        }
        self.refresh_menus();
        true
    }

    // --- Profiles (TASK-7, TASK-9) ----------------------------------------------------

    /// Reads the profiles from the disk into the profile box, by the user's alphabet, and
    /// opens the one in the file `open`, else the one opened last time, else the first.
    /// Says which files of the folder of profiles could not be opened.
    fn load_profiles(&self, open: Option<&Path>) {
        let s = self.s;
        let (mut found, unreadable) = self.list_profiles();
        found.sort_by(|a, b| win32::compare_names(&a.name, &b.name));
        let last = self
            .settings
            .as_ref()
            .ok()
            .and_then(|settings| settings.last_profile().ok().flatten());
        let selected = open
            .and_then(|path| found.iter().position(|p| p.path == path))
            .or_else(|| {
                let last = last?;
                found
                    .iter()
                    .position(|p| file_name(&p.path).to_lowercase() == last.to_lowercase())
            })
            .or((!found.is_empty()).then_some(0));
        self.profile.clear();
        if found.is_empty() {
            // The box says there is nothing to choose from (TASK-9 #17).
            self.profile.append(s.no_profiles_item());
            self.profile.set_selection(0);
        }
        for profile in &found {
            self.profile.append(&profile.name);
        }
        if let Some(i) = selected {
            self.profile.set_selection(i as u32);
        }
        *self.profiles.borrow_mut() = found;
        self.show_profile(selected);
        // The difference names its profile as it is now: renamed, or deleted (TASK-13 #5).
        self.refresh_pair();
        self.describe_difference();
        if !unreadable.is_empty() {
            let messages: Vec<String> = unreadable
                .iter()
                .map(|u| {
                    let reason = s.failure(&failure::of_store(&u.error));
                    s.profile_not_opened(&file_name(&u.path), &reason)
                })
                .collect();
            self.say(&messages.join(" "));
        }
    }

    /// The profiles in the folder of profiles and the files there that could not be
    /// opened. A profile whose database this revisor is compacting is known already; one
    /// another revisor is compacting is waited for before the window shows (TASK-9 #30),
    /// and reported as in use once it is shown.
    fn list_profiles(&self) -> (Vec<ProfileFile>, Vec<Unreadable>) {
        let s = self.s;
        loop {
            let (mut found, unreadable) = match profiles::list(&self.profiles_dir) {
                Ok(listed) => listed,
                Err(e) => {
                    let folder = self.profiles_dir.to_string_lossy();
                    self.say(&s.profiles_not_listed(&folder, &s.failure(&failure::of_io(&e))));
                    return (Vec::new(), Vec::new());
                }
            };
            let mut others = Vec::new();
            let mut busy_elsewhere = Vec::new();
            for u in unreadable {
                if !u.error.is_busy() {
                    others.push(u);
                    continue;
                }
                let jobs = self.jobs.borrow();
                match jobs.closed_profile(&u.path) {
                    Some((profile, _)) => found.push(profile.clone()),
                    None => busy_elsewhere.push(u),
                }
            }
            if busy_elsewhere.is_empty() || self.frame.is_shown() {
                others.extend(busy_elsewhere);
                return (found, others);
            }
            thread::sleep(BUSY_RETRY);
        }
    }

    /// Opens the profile at position `index` of the profile box, or none: the title, the
    /// state, the snapshot list with the newest snapshot current, and the memory of the
    /// profile for the next start.
    fn show_profile(&self, index: Option<usize>) {
        let s = self.s;
        let open = index.and_then(|i| self.profiles.borrow().get(i).cloned());
        self.open_index.set(index.filter(|_| open.is_some()));
        self.frame
            .set_title(&window_title(open.as_ref().map(|p| p.name.as_str())));
        if let (Ok(settings), Some(open)) = (&self.settings, &open) {
            let _ = settings.set_last_profile(Some(&file_name(&open.path)));
        }
        let rows = match &open {
            None => Vec::new(),
            Some(open) => {
                let closed = self
                    .jobs
                    .borrow()
                    .closed_profile(&open.path)
                    .map(|(_, rows)| rows.to_vec());
                match closed {
                    // Its database is closed meanwhile; the compaction knows its snapshots.
                    Some(rows) => rows,
                    None => match profiles::snapshots_of(&open.path) {
                        Ok(rows) => rows,
                        Err(e) => {
                            let reason = s.failure(&failure::of_store(&e));
                            self.say(&s.profile_not_opened(&file_name(&open.path), &reason));
                            Vec::new()
                        }
                    },
                }
            }
        };
        self.snapshots.delete_all_items();
        for (i, row) in rows.iter().enumerate() {
            self.insert_row(i, row);
        }
        if let Some(newest) = rows.len().checked_sub(1) {
            win32::select_only(self.snapshots.get_handle(), newest);
        }
        {
            let mut state = self.state.borrow_mut();
            state.profile = open.as_ref().map(|p| p.name.clone());
            state.snapshots = rows.len();
        }
        self.sync_job_flags();
        *self.rows.borrow_mut() = rows;
        self.sync_selection();
        self.update_empty_text();
        self.refresh_menus();
    }

    /// The open profile, if there is one.
    fn open_profile(&self) -> Option<ProfileFile> {
        let index = self.open_index.get()?;
        self.profiles.borrow().get(index).cloned()
    }

    /// Whether `profile` is the open profile.
    fn is_open(&self, profile: &ProfileFile) -> bool {
        self.open_profile()
            .is_some_and(|open| open.path == profile.path)
    }

    /// Opens the profile whose database is at `path`, with its newest snapshot current,
    /// if the profile box has it; returns whether it is open now.
    fn open_profile_at(&self, path: &Path) -> bool {
        let index = self.profiles.borrow().iter().position(|p| p.path == path);
        let Some(index) = index else {
            return false;
        };
        self.profile.set_selection(index as u32);
        self.show_profile(Some(index));
        true
    }

    /// Reads something from the database of the open profile; when it cannot be read, says
    /// why in a message box titled after the command `id`.
    fn read_open<T>(
        &self,
        id: CommandId,
        read: impl FnOnce(&Path) -> Result<T, StoreError>,
    ) -> Option<(ProfileFile, T)> {
        let s = self.s;
        let open = self.open_profile()?;
        match read(&open.path) {
            Ok(value) => Some((open, value)),
            Err(e) => {
                let reason = s.failure(&failure::of_store(&e));
                let text = s.profile_not_opened(&file_name(&open.path), &reason);
                tell(&self.frame, s, s.command(id), &text);
                None
            }
        }
    }

    fn new_profile(&self) {
        let s = self.s;
        let known = self.profiles.borrow().clone();
        let dir = self.profiles_dir.clone();
        let created = profile_window::show(
            &self.frame,
            s,
            Purpose::New,
            &ProfileSettings::new_profile(),
            move |name| profiles::name_taken(&known, name, None),
            move |settings| {
                profiles::create(&dir, settings).map_err(|e| save_failure(s, settings, &e))
            },
        );
        if let Some(file) = created {
            self.load_profiles(Some(&file.path));
            self.say(&s.profile_created(&file.name));
        }
    }

    fn edit_profile(&self) {
        let s = self.s;
        let Some((open, initial)) = self.read_open(CommandId::EditProfile, profiles::settings_of)
        else {
            return;
        };
        let known = self.profiles.borrow().clone();
        let path = open.path.clone();
        let old_path = open.path.clone();
        let updated = profile_window::show(
            &self.frame,
            s,
            Purpose::Edit,
            &initial,
            move |name| profiles::name_taken(&known, name, Some(&open.path)),
            move |settings| {
                profiles::update(&path, settings).map_err(|e| save_failure(s, settings, &e))
            },
        );
        if let Some(file) = updated {
            self.profile_renamed(&old_path, &file);
            self.load_profiles(Some(&file.path));
            self.say(&s.profile_changed(&file.name));
        }
    }

    /// The profile whose database was at `old` is now `profile`, its file renamed with it
    /// (TASK-7): the difference of it, shown or still being made, names it so (TASK-13 #5).
    fn profile_renamed(&self, old: &Path, profile: &ProfileFile) {
        self.jobs.borrow_mut().profile_renamed(old, profile);
        let mut list = self.difference_list.borrow_mut();
        if list.profile().is_some_and(|p| p.path == old) {
            list.set_profile(Some(profile.clone()));
        }
    }

    /// Deletes the open profile with all its snapshots, but only once the user confirms.
    fn delete_profile(&self) {
        let s = self.s;
        let title = s.command(CommandId::DeleteProfile);
        let Some((open, snapshots)) =
            self.read_open(CommandId::DeleteProfile, profiles::snapshot_count)
        else {
            return;
        };
        let question = s.delete_profile_question(&open.name, snapshots);
        let delete = (s.delete(), DELETE_ACCESS);
        if !confirm(&self.frame, s, title, &question, delete, true) {
            return;
        }
        self.wait_for_comparison_to_load(&open.path);
        match profiles::delete(&open.path) {
            Ok(()) => {
                self.load_profiles(None);
                self.say(&s.profile_deleted(&open.name));
            }
            Err(e) => {
                let reason = s.failure(&failure::of_io(&e));
                tell(
                    &self.frame,
                    s,
                    title,
                    &s.profile_not_deleted(&open.name, &reason),
                );
            }
        }
    }

    // --- The settings (TASK-9) --------------------------------------------------------

    fn focus_after_snapshot(&self) -> FocusAfterSnapshot {
        self.settings
            .as_ref()
            .ok()
            .and_then(|settings| settings.focus_after_snapshot().ok())
            .unwrap_or_default()
    }

    fn focus_after_comparison(&self) -> FocusAfterComparison {
        self.settings
            .as_ref()
            .ok()
            .and_then(|settings| settings.focus_after_comparison().ok())
            .unwrap_or_default()
    }

    fn confirm_deletion(&self) -> bool {
        self.settings
            .as_ref()
            .ok()
            .and_then(|settings| settings.confirm_snapshot_deletion().ok())
            .unwrap_or(true)
    }

    fn language(&self) -> Language {
        self.settings
            .as_ref()
            .ok()
            .and_then(|settings| settings.language().ok())
            .unwrap_or_default()
    }

    /// Shows the settings window and keeps what the user confirmed. When the language
    /// setting makes the next start speak another language, offers to restart now
    /// (TASK-16 #3).
    fn settings(&self) {
        let s = self.s;
        let title = s.command(CommandId::Settings);
        let initial = Values {
            language: self.language(),
            focus_after_snapshot: self.focus_after_snapshot(),
            focus_after_comparison: self.focus_after_comparison(),
            size_format: self.difference_list.borrow().size_format(),
            confirm_snapshot_deletion: self.confirm_deletion(),
        };
        let Some(values) = settings_window::show(&self.frame, s, initial) else {
            return;
        };
        let saved = match &self.settings {
            Ok(settings) => settings
                .set_language(values.language)
                .and_then(|()| settings.set_focus_after_snapshot(values.focus_after_snapshot))
                .and_then(|()| settings.set_focus_after_comparison(values.focus_after_comparison))
                .and_then(|()| settings.set_size_format(values.size_format))
                .and_then(|()| {
                    settings.set_confirm_snapshot_deletion(values.confirm_snapshot_deletion)
                })
                .map_err(|e| s.failure(&failure::of_settings(&e))),
            Err(message) => Err(message.clone()),
        };
        match saved {
            Ok(()) => self.say(s.settings_saved()),
            Err(reason) => {
                tell(&self.frame, s, title, &s.settings_not_saved(&reason));
                return;
            }
        }
        // Sizes in the list and the summary as the setting «Розмір» says (TASK-13 #17).
        let current = {
            let mut list = self.difference_list.borrow_mut();
            list.set_size_format(values.size_format);
            list.current()
        };
        self.refresh_difference(current);
        // «Same as Windows» is read now, not at the start: the language of Windows may
        // have changed meanwhile, and the next start would take the new one.
        let next = crate::language_of(values.language, win32::ui_language);
        if next == s.lang() {
            return;
        }
        let restart_now = choose(
            &self.frame,
            s,
            title,
            s.language_changes_after_restart(),
            (s.restart_now(), RESTART_NOW_ACCESS),
            Some((s.later(), LATER_ACCESS)),
            false,
        );
        if !restart_now {
            return;
        }
        // During a snapshot, the same question as closing revisor asks (TASK-16 #4).
        let agreed = self.stop_snapshot_if_agreed(
            s.command(CommandId::Exit),
            s.close_during_snapshot(),
            (s.close(), CLOSE_ACCESS),
        );
        if agreed {
            self.restart_with_same_rights(self.open_profile().as_ref());
        }
    }

    // --- The snapshot list (TASK-9) ---------------------------------------------------

    /// Puts a snapshot in the list at `index`: its name, then its comment.
    fn insert_row(&self, index: usize, row: &SnapshotInfo) {
        self.snapshots
            .insert_item(index as i64, &win32::format_date_time(row.taken_at), None);
        if let Some(comment) = &row.comment {
            self.snapshots
                .set_item_text_by_column(index as i64, 1, comment);
        }
    }

    /// The current snapshot: the row with the focus rectangle, or the newest one when no
    /// row has it.
    fn current_row(&self) -> Option<usize> {
        let count = self.rows.borrow().len();
        win32::focused_row(self.snapshots.get_handle())
            .filter(|&row| row < count)
            .or_else(|| count.checked_sub(1))
    }

    /// Keeps the state's count of selected snapshots and its current snapshot as the list
    /// has them.
    fn sync_selection(&self) {
        let selected = win32::selected_count(self.snapshots.get_handle());
        let current = self.current_row();
        let mut state = self.state.borrow_mut();
        state.selected_snapshots = selected;
        state.current_snapshot = current;
    }

    /// What the empty snapshot list says, in its area and in its name for screen readers:
    /// that there are no profiles, or no snapshots, and which key makes the first one.
    fn update_empty_text(&self) {
        let s = self.s;
        let key = |id| command(id).shortcut.map(|k| k.name(s)).unwrap_or_default();
        let hint = if self.profiles.borrow().is_empty() {
            Some(s.no_profiles_yet(&key(CommandId::NewProfile)))
        } else if self.rows.borrow().is_empty() {
            Some(s.no_snapshots_yet(&key(CommandId::TakeSnapshot)))
        } else {
            None
        };
        let list = self.snapshots.get_handle();
        let name = match &hint {
            Some(hint) => format!("{}. {hint}", s.pane(Pane::Snapshots)),
            None => s.pane(Pane::Snapshots).to_string(),
        };
        win32::set_accessible_name(list, &name);
        win32::set_empty_text(list, hint.as_deref().unwrap_or_default());
    }

    /// Adds a saved snapshot of the open profile to the end of the list, as its `arrival`
    /// says: as a row, as the current snapshot, or as the current snapshot with the
    /// keyboard focus.
    fn append_row(&self, row: SnapshotInfo, arrival: Arrival) {
        let index = {
            let mut rows = self.rows.borrow_mut();
            rows.push(row);
            rows.len() - 1
        };
        self.insert_row(index, &self.rows.borrow()[index]);
        self.state.borrow_mut().snapshots = index + 1;
        if matches!(arrival, Arrival::Current | Arrival::Focus) {
            win32::select_only(self.snapshots.get_handle(), index);
        }
        if arrival == Arrival::Focus {
            self.snapshots.set_focus();
        }
        self.sync_selection();
        self.update_empty_text();
    }

    /// Runs `dialog` with the jobs' timer at rest, so a snapshot that ends meanwhile does
    /// not change the list under the dialog; its end is reported once the dialog closes.
    fn with_jobs_paused<T>(&self, dialog: impl FnOnce() -> T) -> T {
        self.timer.stop();
        let answer = dialog();
        if self.jobs.borrow().any() {
            self.timer.start(PROGRESS_INTERVAL, false);
        }
        answer
    }

    /// Asks for the current snapshot's comment and keeps it (TASK-9 #7). While a snapshot
    /// is being saved, the write waits for it (#29).
    fn comment(&self) {
        let s = self.s;
        let title = s.command(CommandId::Comment);
        let Some(open) = self.open_profile() else {
            return;
        };
        let Some(row) = self.current_row() else {
            return;
        };
        let info = self.rows.borrow()[row].clone();
        let name = win32::format_date_time(info.taken_at);
        let asked = self.with_jobs_paused(|| {
            ask_text(
                &self.frame,
                s,
                title,
                &s.comment_of(&name),
                COMMENT_ACCESS,
                info.comment.as_deref().unwrap_or_default(),
            )
        });
        let Some(text) = asked else {
            return;
        };
        let comment = (!text.is_empty()).then_some(text);
        let written = Store::open_with_timeout(&open.path, COMMENT_TIMEOUT)
            .and_then(|mut store| store.set_comment(info.id, comment.as_deref()));
        match written {
            Ok(()) => {
                self.snapshots.set_item_text_by_column(
                    row as i64,
                    1,
                    comment.as_deref().unwrap_or_default(),
                );
                let removed = comment.is_none();
                self.rows.borrow_mut()[row].comment = comment;
                self.refresh_pair();
                self.describe_difference();
                self.say(if removed {
                    s.comment_removed()
                } else {
                    s.comment_saved()
                });
            }
            Err(e) => {
                let reason = s.failure(&failure::of_store(&e));
                tell(&self.frame, s, title, &s.comment_not_saved(&reason));
            }
        }
    }

    /// Deletes the selected snapshots, after the confirmation the settings ask for, and
    /// compacts the profile's database in the background (TASK-9 #24–#27).
    fn delete_snapshots(&self) {
        let s = self.s;
        let title = s.command(CommandId::DeleteSnapshots);
        let Some(open) = self.open_profile() else {
            return;
        };
        let list = self.snapshots.get_handle();
        let selected = win32::selected_rows(list);
        let chosen: Vec<SnapshotInfo> = {
            let rows = self.rows.borrow();
            selected
                .iter()
                .filter_map(|&i| rows.get(i).cloned())
                .collect()
        };
        if chosen.is_empty() {
            return;
        }
        // No row keeps the focus rectangle from here on: when the question closes, the
        // focus comes back to the list, and a screen reader would announce the focused
        // row just as it is deleted. A refusal puts the rectangle back.
        let focused = win32::focused_row(list);
        win32::unfocus_rows(list);
        if self.confirm_deletion() {
            let question = match chosen.as_slice() {
                [one] => s.delete_snapshot_question(
                    &win32::format_date_time(one.taken_at),
                    one.comment.as_deref(),
                ),
                many => s.delete_snapshots_question(many.len()),
            };
            let delete = (s.delete(), DELETE_ACCESS);
            let agreed =
                self.with_jobs_paused(|| confirm(&self.frame, s, title, &question, delete, true));
            if !agreed {
                if let Some(row) = focused {
                    win32::focus_row(list, row);
                }
                return;
            }
        }
        let ids: Vec<i64> = chosen.iter().map(|row| row.id).collect();
        self.wait_for_comparison_to_load(&open.path);
        let deleted = Store::open(&open.path).and_then(|mut store| {
            store.delete_many(&ids)?;
            Ok(store)
        });
        let store = match deleted {
            Ok(store) => store,
            Err(e) => {
                let reason = s.failure(&failure::of_store(&e));
                tell(&self.frame, s, title, &s.snapshots_not_deleted(&reason));
                if let Some(row) = focused {
                    win32::focus_row(list, row);
                }
                return;
            }
        };
        // The rows go at once, from the bottom up so the positions hold; then the focus
        // goes to the row that followed them (#26).
        let count = self.rows.borrow().len();
        for &i in selected.iter().rev() {
            self.snapshots.delete_item(i as i64);
            self.rows.borrow_mut().remove(i);
        }
        if let Some(next) = row_after_deletion(&selected, count) {
            win32::select_only(list, next);
        }
        let rows = self.rows.borrow().clone();
        self.state.borrow_mut().snapshots = rows.len();
        self.sync_selection();
        self.update_empty_text();
        self.refresh_pair();
        self.describe_difference();
        self.jobs
            .borrow_mut()
            .start_compaction(open.clone(), rows, store);
        self.sync_jobs();
        self.say(&s.snapshots_deleted(chosen.len(), &open.name));
    }

    /// Asks the compaction to stop; its end comes through the timer, like any other.
    fn cancel_compaction(&self) {
        self.jobs.borrow().cancel(Kind::Compaction);
    }

    /// Reports the end of the compaction.
    fn compaction_ended(&self, ended: Ended<Outcome<Compacted, StoreError>>) {
        let s = self.s;
        self.sync_jobs();
        let name = &ended.profile.name;
        match ended.outcome {
            Ok(Outcome::Done(compacted)) => {
                let freed = win32::format_size(compacted.before.saturating_sub(compacted.after));
                self.say(&s.compaction_done(name, &freed));
            }
            Ok(Outcome::Cancelled) => self.say(&s.compaction_cancelled(name)),
            Ok(Outcome::Failed(e)) => {
                let reason = s.failure(&failure::of_store(&e));
                self.say(&s.compaction_failed(name, &reason));
            }
            Err(Lost) => self.say(&s.compaction_failed(name, s.compaction_stopped())),
        }
    }

    // --- The comparison (TASK-11) -------------------------------------------------------

    /// Compares the current snapshot with the one before it in the list (#1).
    fn compare_with_previous(&self) {
        let Some(open) = self.open_profile() else {
            return;
        };
        let Some(row) = self.current_row().filter(|&row| row > 0) else {
            return;
        };
        let (current, previous) = {
            let rows = self.rows.borrow();
            (rows[row].id, rows[row - 1].id)
        };
        self.start_comparison(open, current, previous);
    }

    /// Asks which other snapshot of the profile to compare the current one with, and
    /// compares (#2, #3). The comparison then starts from the snapshot list, wherever
    /// the command came from (#12).
    fn compare_with(&self) {
        let s = self.s;
        let Some(open) = self.open_profile() else {
            return;
        };
        let Some(row) = self.current_row() else {
            return;
        };
        let rows = self.rows.borrow().clone();
        let chosen = self.with_jobs_paused(|| compare_window::show(&self.frame, s, &rows, row));
        let Some(other) = chosen else {
            return;
        };
        if win32::focused_window() != self.snapshots.get_handle() {
            self.snapshots.set_focus();
        }
        self.start_comparison(open, rows[row].id, other.id);
    }

    /// Starts comparing the snapshots `a` and `b` of `profile` in the background, in
    /// place of a comparison that still runs (#15), remembering where the keyboard focus
    /// is for the difference's arrival (#12). The progress row shows the comparison once
    /// it has run for a second (#7). The report of a «Знімок + порівняння» whose
    /// comparison this one takes the place of is said now (TASK-12).
    fn start_comparison(&self, profile: ProfileFile, a: i64, b: i64) {
        self.comparison_focus.set(win32::focused_window());
        let superseded = self.jobs.borrow_mut().start_comparison(profile, a, b);
        self.sync_jobs();
        if let Some(superseded) = superseded {
            self.report_superseded(superseded);
        }
    }

    /// Asks the comparison to stop; its end comes through the timer, like any other.
    fn cancel_comparison(&self) {
        self.jobs.borrow().cancel(Kind::Comparison);
    }

    /// Waits until no comparison of the profile whose database is at `path` reads that
    /// database any more: its trees load in seconds, and deleting the profile's
    /// snapshots, or the profile, goes on then (#17).
    fn wait_for_comparison_to_load(&self, path: &Path) {
        while self.jobs.borrow().comparison_holds_database(path) {
            thread::sleep(HOLD_RETRY);
        }
    }

    /// Ends the comparison in the window: a difference takes the place of the one shown,
    /// as the setting «Фокус після порівняння» says; a cancelled or failed comparison
    /// leaves the difference as it was and says so (#10). Where the focus is decides
    /// where it goes, so it is read before the progress row goes and takes the focus
    /// with it. The comparison of «Знімок + порівняння», `after` its snapshot, ends the
    /// command (TASK-12).
    fn comparison_ended(
        &self,
        ended: Ended<Outcome<Difference, CompareError>>,
        after: Option<Report>,
    ) {
        if let Some(report) = after {
            self.snapshot_and_compare_ended(ended, report);
            return;
        }
        let s = self.s;
        let focus = self.comparison_focus_place();
        self.sync_jobs();
        let Ended { profile, outcome } = ended;
        match outcome {
            Ok(Outcome::Done(difference)) => {
                let arrival = difference::arrival(self.focus_after_comparison(), focus);
                let to_difference = arrival == difference::Arrival::Focus;
                self.show_difference(difference, profile, to_difference);
                if to_difference {
                    // The focus says it: the status bar only stops showing the
                    // comparison's progress, without a word from the screen reader.
                    self.status.set_status_text(s.comparison_finished(), 0);
                } else {
                    self.say(s.comparison_finished());
                }
            }
            Ok(Outcome::Cancelled) => self.say(s.comparison_cancelled()),
            Ok(Outcome::Failed(e)) => {
                let reason = s.failure(&failure::of_compare(&e));
                self.say(&s.comparison_failed(&reason));
            }
            Err(Lost) => self.say(&s.comparison_failed(s.comparison_stopped())),
        }
    }

    /// Puts `difference`, of `profile`, in the difference list that is not shown, which
    /// then takes the place of the shown one, with the keyboard focus moving to its first
    /// row when `focus` says so (#12–#14, #21). The view stays, the text filter goes
    /// (TASK-13 #28), and the new list is named by the summary of its difference before
    /// it shows, so a screen reader names it so when the focus comes to it (TASK-13 #3).
    fn show_difference(&self, difference: Difference, profile: ProfileFile, focus: bool) {
        let current = self
            .difference_list
            .borrow_mut()
            .replace(difference, profile);
        self.filter_field.change_value("");
        self.showing_timer.stop();
        self.refresh_pair();
        let count = self.difference_list.borrow().len();
        let old = self.difference();
        let next = 1 - self.shown_difference.get();
        let new = &self.differences[next];
        new.set_item_count(count as i64);
        if let Some(row) = current {
            win32::select_only(new.get_handle(), row);
        }
        new.enable(true);
        self.shown_difference.set(next);
        self.place_difference_lists();
        self.describe_difference();
        if focus {
            new.set_focus();
        }
        // Disabled once the focus has left it, so that the focus does not go elsewhere.
        old.enable(false);
        old.set_item_count(0);
    }

    /// The list shown fills the panel of the difference lists; the other has no size.
    fn place_difference_lists(&self) {
        let size = self.difference_holder.get_client_size();
        let shown = self.shown_difference.get();
        for (i, list) in self.differences.iter().enumerate() {
            if i == shown {
                list.set_size_with_pos(0, 0, size.width, size.height);
            } else {
                list.set_size_with_pos(0, 0, 0, 0);
            }
        }
    }

    // --- The difference list (TASK-13) --------------------------------------------------

    /// The focus of the difference list `list` has moved to the row at `index`: the model
    /// keeps it, and it is the one row selected, as the list selects only the change with
    /// the focus (#19).
    fn difference_focused(&self, list: usize, index: i32) {
        let Ok(index) = usize::try_from(index) else {
            return;
        };
        if list != self.shown_difference.get() {
            return;
        }
        self.difference_list.borrow_mut().focus(index);
        let hwnd = self.differences[list].get_handle();
        if !win32::is_selected(hwnd, index) {
            win32::select_only(hwnd, index);
        }
    }

    /// Brings the difference list in line with the model once the view or the difference
    /// changed: its rows, the row with the focus, at `current` (#21, #27), and what is
    /// said about the difference.
    fn refresh_difference(&self, current: Option<usize>) {
        let list = self.difference();
        let count = self.difference_list.borrow().len();
        list.set_item_count(count as i64);
        if let Some(row) = current {
            win32::select_only(list.get_handle(), row);
        }
        if count > 0 {
            list.refresh_items(0, count as i64 - 1);
        }
        self.describe_difference();
    }

    /// What is said about the difference shown: the name of the list and the text of its
    /// empty area (#3, #11, #25), the field «Порівняння» (#1, #11), the check boxes and
    /// the menu's marks, which follow the view (#22), the sort arrow of the headers (#21),
    /// and the filter's controls, which work once there is a difference (#30).
    fn describe_difference(&self) {
        let s = self.s;
        let (name, text, view, has_difference) = {
            let list = self.difference_list.borrow();
            (
                list.list_name(),
                list.comparison_text(),
                list.view(),
                list.has_difference(),
            )
        };
        let text = text.unwrap_or_else(|| {
            let key = |id| command(id).shortcut.map(|k| k.name(s)).unwrap_or_default();
            s.nothing_compared_yet(
                &key(CommandId::CompareWithPrevious),
                &key(CommandId::CompareWith),
            )
        });
        let list = self.difference().get_handle();
        win32::set_accessible_name(list, &name);
        win32::set_empty_text(list, &name);
        // The caret stays where it is while the text does.
        if self.comparison_field.get_value() != text {
            self.comparison_field.change_value(&text);
        }
        for kind in ChangeType::ALL {
            let check_box = &self.type_boxes[kind as usize];
            check_box.set_value(view.types[kind as usize]);
            check_box.enable(has_difference);
        }
        self.filter_field.enable(has_difference);
        let arrow = Column::ALL
            .iter()
            .position(|column| column.sort() == view.sort.column)
            .map(|i| (i, view.sort.descending));
        win32::set_sort_arrow(list, arrow);
        {
            let mut state = self.state.borrow_mut();
            state.has_difference = has_difference;
            state.shown_types = view.types;
            state.sort = view.sort;
        }
        self.refresh_menus();
    }

    /// Brings the line of the pair in line with the profile and the snapshots of the
    /// difference as they are now (#5): the profile renamed or deleted, a snapshot deleted,
    /// a comment changed. The profile is known by its file.
    fn refresh_pair(&self) {
        let Some(profile) = self.difference_list.borrow().profile().cloned() else {
            return;
        };
        let now = self
            .profiles
            .borrow()
            .iter()
            .find(|p| p.path == profile.path)
            .cloned();
        let snapshots = now.as_ref().and_then(|now| self.snapshots_now(now));
        let mut list = self.difference_list.borrow_mut();
        list.set_profile(now);
        if let Some(snapshots) = snapshots {
            list.set_snapshots(&snapshots);
        }
    }

    /// The snapshots `profile` has now: those of the snapshot list while it is open, those
    /// a compaction keeps while it has closed the database, else as the database says, if
    /// it can be read at once.
    fn snapshots_now(&self, profile: &ProfileFile) -> Option<Vec<SnapshotInfo>> {
        if self.is_open(profile) {
            return Some(self.rows.borrow().clone());
        }
        if let Some((_, rows)) = self.jobs.borrow().closed_profile(&profile.path) {
            return Some(rows.to_vec());
        }
        Store::open_with_timeout(&profile.path, Duration::ZERO)
            .and_then(|store| store.list())
            .ok()
    }

    /// Shows the rows of `types` (#22), from the check boxes, the menu and its keys; the
    /// view is kept for the next differences and starts (#28), and the status bar says
    /// how many rows the list shows (#26).
    fn set_types(&self, types: [bool; 4]) {
        let current = self.difference_list.borrow_mut().set_types(types);
        self.refresh_difference(current);
        if let Ok(settings) = &self.settings {
            let _ = settings.set_difference_types(types);
        }
        let text = self.difference_list.borrow().showing();
        self.say(&text);
    }

    /// The menu item of a type, or its key: shows the type, or hides it.
    fn toggle_type(&self, kind: ChangeType) {
        let mut types = self.difference_list.borrow().view().types;
        types[kind as usize] ^= true;
        self.set_types(types);
    }

    /// The check box of a type was clicked or pressed.
    fn type_toggled(&self, kind: ChangeType) {
        let shown = self.type_boxes[kind as usize].get_value();
        let mut types = self.difference_list.borrow().view().types;
        if types[kind as usize] != shown {
            types[kind as usize] = shown;
            self.speaking(|| self.set_types(types));
        }
    }

    /// Puts the list in the order `sort` (#21), kept for the next differences and starts
    /// (#28); the focus stays on its change.
    fn set_sort(&self, sort: DifferenceSort) {
        let current = self.difference_list.borrow_mut().set_sort(sort);
        self.refresh_difference(current);
        if let Ok(settings) = &self.settings {
            let _ = settings.set_difference_sort(sort);
        }
    }

    /// A column of the menu «Сортувати за»: sorts by it, the direction as it is, as in
    /// File Explorer.
    fn sort_by(&self, column: SortColumn) {
        let sort = self.difference_list.borrow().view().sort;
        self.set_sort(DifferenceSort { column, ..sort });
    }

    /// «За зростанням» or «За спаданням».
    fn sort_direction(&self, descending: bool) {
        let sort = self.difference_list.borrow().view().sort;
        self.set_sort(DifferenceSort { descending, ..sort });
    }

    /// A click on the header of the column `column`: sorts by it ascending, or, when the
    /// list is sorted by it already, the other way (#21). Without a difference there is
    /// nothing to sort, as the commands say.
    fn sort_by_column(&self, column: i32) {
        let Some(column) = usize::try_from(column)
            .ok()
            .and_then(|i| Column::ALL.get(i))
        else {
            return;
        };
        let (sort, has_difference) = {
            let list = self.difference_list.borrow();
            (list.view().sort, list.has_difference())
        };
        if !has_difference {
            return;
        }
        let column = column.sort();
        let descending = sort.column == column && !sort.descending;
        self.speaking(|| self.set_sort(DifferenceSort { column, descending }));
    }

    /// The text of the field «Фільтр:» changed as the user typed: the list follows at
    /// once, the focus staying in the field, and how many rows it shows is said once the
    /// typing pauses (#24, #26).
    fn filter_typed(&self) {
        let text = self.filter_field.get_value();
        if text == self.difference_list.borrow().text() {
            return;
        }
        let current = self.difference_list.borrow_mut().set_text(&text);
        self.refresh_difference(current);
        self.showing_timer.start(SHOWING_PAUSE, true);
    }

    /// Esc in the field «Фільтр:»: the field empties, so the list shows every row of the
    /// types shown, and that is said (#24, #26).
    fn clear_filter(&self) {
        if self.filter_field.get_value().is_empty() {
            return;
        }
        self.filter_field.change_value("");
        self.showing_timer.stop();
        let current = self.difference_list.borrow_mut().set_text("");
        self.refresh_difference(current);
        let text = self.difference_list.borrow().showing();
        self.say(&text);
    }

    /// «Фільтрувати за текстом»: the focus goes to the field «Фільтр:» with its text
    /// selected, to be typed over, as in a search box (#23).
    fn filter_by_text(&self) {
        self.filter_field.set_focus();
        self.filter_field.select_all();
    }

    /// The progress field for the comparison the row shows, with the bar once the
    /// comparison has run for [`COMPARISON_BAR_AFTER`] (#8). A phase the row has not
    /// said yet — comparing, or waiting for the database — is said in the status bar,
    /// which screen readers read without the focus moving (#7, #16), once the comparison
    /// has run for a second: that of «Знімок + порівняння» is in the row at once, in
    /// place of its snapshot, and a short one says no more than its end (TASK-12).
    fn comparison_progress_text(&self, progress: &compare::Progress, elapsed: Duration) -> String {
        let s = self.s;
        let waiting = progress.phase() == compare::Phase::Waiting;
        let phase_text = if waiting {
            s.waiting_for_database()
        } else {
            s.comparing()
        };
        if elapsed >= COMPARISON_SHOWN_AFTER && self.said_waiting.get() != Some(waiting) {
            self.said_waiting.set(Some(waiting));
            self.say(phase_text);
        }
        let bar = elapsed >= COMPARISON_BAR_AFTER;
        if bar != self.gauge.is_shown() {
            self.gauge.show(bar);
            self.panel.layout();
        }
        if bar && !waiting {
            let percent = progress.percent();
            self.gauge.set_value(i32::from(percent));
            s.comparing_percent(percent)
        } else {
            phase_text.to_string()
        }
    }

    /// On closing: a compaction that is running goes on behind the hidden window, and the
    /// process ends only when it has (TASK-9 #30); with `interrupt`, as at a shutdown,
    /// the compaction is interrupted instead, which leaves the database as it was.
    fn finish_compaction_hidden(&self, interrupt: bool) {
        if !self.jobs.borrow().runs(Kind::Compaction) {
            return;
        }
        self.timer.stop();
        self.frame.show(false);
        self.jobs.borrow_mut().finish_compaction(interrupt);
    }

    // --- «Знімок + порівняння» (TASK-12) -----------------------------------------------

    /// Starts «Знімок + порівняння» on the open profile: a snapshot, which is compared,
    /// once saved, with the snapshot that is the newest of the profile now, wherever the
    /// focus is (#1). The command is a comparison from now on, so one that still runs
    /// gives way to it (TASK-11 #15), and where the focus is now is where «Фокус після
    /// порівняння» counts from (#5).
    fn snapshot_and_compare(&self) {
        let s = self.s;
        let Some(open) = self.open_profile() else {
            return;
        };
        let with = self.rows.borrow().last().map(|row| row.id);
        self.comparison_focus.set(win32::focused_window());
        let superseded = self
            .jobs
            .borrow_mut()
            .start_snapshot_and_compare(open, with);
        self.sync_jobs();
        if let Some(superseded) = superseded {
            self.report_superseded(superseded);
        }
        self.say(s.snapshot_started());
    }

    /// The snapshot of «Знімок + порівняння» is saved: it goes to the end of its
    /// profile's list (#6), and its comparison with `with` starts at once, before the
    /// progress row changes, so the row and the focus on it stay (#5, #8, #9); the
    /// snapshot's report waits for the difference. Without a snapshot to compare it with,
    /// the command is over: the snapshot is reported, and that said (#4). `focus` is
    /// where the keyboard focus was while the snapshot ran.
    fn snapshot_to_compare(
        &self,
        profile: ProfileFile,
        report: Report,
        with: Option<i64>,
        focus: FocusPlace,
    ) {
        let s = self.s;
        let of_open_profile = self.is_open(&profile);
        let Some(with) = with else {
            if of_open_profile {
                self.append_row(
                    snapshot_row(&report),
                    arrival_after_comparison(focus, false),
                );
            }
            self.sync_jobs();
            let text = format!(
                "{} {}",
                self.snapshot_report(&report),
                s.nothing_to_compare()
            );
            self.say(&text);
            self.offer_restart(&profile, &report, CommandId::SnapshotAndCompare);
            return;
        };
        if of_open_profile {
            self.append_row(snapshot_row(&report), Arrival::Append);
        }
        self.jobs
            .borrow_mut()
            .start_comparison_after_snapshot(profile, with, report);
        self.sync_jobs();
    }

    /// Ends «Знімок + порівняння» once its comparison has: the difference takes the place
    /// of the one shown as «Фокус після порівняння» says, counted from the command's
    /// start, with the progress of the snapshot as that of the comparison (#5) — with
    /// «Завжди» while another profile is open, the snapshot's profile is opened too (#7);
    /// the snapshot becomes current in its list unless the keyboard focus stays there
    /// (#6); and the snapshot's report, held so far, is said with the comparison's end,
    /// then the restart offered by the rules of TASK-8 (#8). A cancelled comparison
    /// leaves the snapshot, and says so (#9). Where the focus is decides where it goes,
    /// so it is read before the progress row goes and takes the focus with it.
    fn snapshot_and_compare_ended(
        &self,
        ended: Ended<Outcome<Difference, CompareError>>,
        report: Report,
    ) {
        let s = self.s;
        let focus = self.comparison_focus_place();
        let list_focus = self.focus_place();
        let profile = ended.profile;
        let (to_difference, text) = match ended.outcome {
            Ok(Outcome::Done(difference)) => {
                let arrival = difference::arrival_after_snapshot(
                    self.focus_after_comparison(),
                    focus,
                    self.is_open(&profile),
                );
                if arrival == difference::Arrival::OpenProfileAndFocus {
                    self.open_profile_at(&profile.path);
                }
                let to_difference = arrival != difference::Arrival::Replace;
                self.show_difference(difference, profile.clone(), to_difference);
                let mut text = self.snapshot_report(&report);
                // The focus on the difference says the comparison is over.
                if !to_difference {
                    text.push(' ');
                    text.push_str(s.comparison_finished());
                }
                (to_difference, text)
            }
            Ok(Outcome::Cancelled) => {
                let cancelled = s.snapshot_saved_comparison_cancelled().to_string();
                (false, self.snapshot_report_after(cancelled, &report))
            }
            Ok(Outcome::Failed(e)) => {
                let failed = s.comparison_failed(&s.failure(&failure::of_compare(&e)));
                (false, format!("{} {failed}", self.snapshot_report(&report)))
            }
            Err(Lost) => {
                let failed = s.comparison_failed(s.comparison_stopped());
                (false, format!("{} {failed}", self.snapshot_report(&report)))
            }
        };
        self.place_snapshot(
            &profile,
            report.id,
            arrival_after_comparison(list_focus, to_difference),
        );
        self.sync_jobs();
        self.say(&text);
        self.offer_restart(&profile, &report, CommandId::SnapshotAndCompare);
    }

    /// Makes the snapshot `id` of `profile` current in the snapshot list as `arrival`
    /// says — `Current`, or `Focus` with the keyboard focus moving to it; `Append` leaves
    /// the list as it is. Nothing changes while another profile is open, or when the
    /// snapshot has been deleted meanwhile.
    fn place_snapshot(&self, profile: &ProfileFile, id: i64, arrival: Arrival) {
        if !self.is_open(profile) || !matches!(arrival, Arrival::Current | Arrival::Focus) {
            return;
        }
        let Some(index) = self.rows.borrow().iter().position(|row| row.id == id) else {
            return;
        };
        win32::select_only(self.snapshots.get_handle(), index);
        if arrival == Arrival::Focus {
            self.snapshots.set_focus();
        }
        self.sync_selection();
    }

    /// Says the report of the snapshot of «Знімок + порівняння» whose comparison another
    /// comparison has taken the place of (TASK-11 #15): the difference it waited for will
    /// not come, so it is said now, as after a regular snapshot, with the restart offer of
    /// TASK-8 (#8). Its row is in the list already.
    fn report_superseded(&self, superseded: Superseded) {
        let Superseded { profile, report } = superseded;
        self.say(&self.snapshot_report(&report));
        self.offer_restart(&profile, &report, CommandId::SnapshotAndCompare);
    }

    // --- The snapshot being taken (TASK-8) ---------------------------------------------

    /// Starts a snapshot of the open profile on its own thread and shows its progress.
    fn take_snapshot(&self) {
        let s = self.s;
        let Some(open) = self.open_profile() else {
            return;
        };
        self.jobs.borrow_mut().start_snapshot(open);
        self.sync_jobs();
        self.say(s.snapshot_started());
    }

    /// Asks the snapshot to stop; its end comes through the timer, like any other.
    fn cancel_snapshot(&self) {
        self.jobs.borrow().cancel(Kind::Snapshot);
    }

    /// Reports the jobs that have ended, together when several have, and shows the
    /// progress of the job the row shows. The ends come first: a job over by now says
    /// nothing more of its progress.
    fn poll_jobs(&self) {
        self.speaking(|| {
            let ended = self.jobs.borrow_mut().poll();
            for event in ended {
                match event {
                    Event::Snapshot(ended, then) => self.snapshot_ended(ended, then, false),
                    Event::Comparison(ended, after) => self.comparison_ended(ended, after),
                    Event::Compaction(ended) => self.compaction_ended(ended),
                }
            }
            self.show_job_progress();
        });
    }

    /// Shows what the job the progress row shows has done so far. When the row comes to
    /// show another job — a comparison that has run for a second (TASK-11 #7) — it is
    /// set up for that job first.
    fn show_job_progress(&self) {
        if self.shown_kind() != self.shown_kind.get() {
            self.refresh_progress_row();
        }
        let s = self.s;
        let text = match self.jobs.borrow().shown() {
            Some(Shown::Snapshot(progress)) => {
                let entries = progress.entries();
                let entries_text = win32::format_count(entries);
                match progress.phase() {
                    Phase::Scanning => {
                        let folder = progress.folder();
                        s.snapshot_progress(entries, &entries_text, &folder.to_string_lossy())
                    }
                    Phase::Saving | Phase::Saved => s.snapshot_saving(entries, &entries_text),
                }
            }
            Some(Shown::Comparison { progress, elapsed }) => {
                self.comparison_progress_text(progress, elapsed)
            }
            _ => return,
        };
        self.progress_text.change_value(&text);
    }

    /// Ends the snapshot in the window, whichever way it ended: on the timer, or stopped
    /// on closing and restarting, `stopped`, which reports it without questions. Where
    /// the focus was while the snapshot ran decides where it goes (TASK-9 #19), so it is
    /// read before the progress row goes and takes the focus with it. The snapshot of
    /// «Знімок + порівняння», `then`, goes on to its comparison once saved; cancelled,
    /// failed or stopped, it ends the command as it ends a regular snapshot (TASK-12 #9,
    /// #10, #12).
    fn snapshot_ended(
        &self,
        ended: Ended<Outcome<Report, SnapshotError>>,
        then: Option<ThenCompare>,
        stopped: bool,
    ) {
        let s = self.s;
        let focus = self.focus_place();
        let outcome = match (ended.outcome, then) {
            (Ok(Outcome::Done(report)), Some(then)) if !stopped => {
                self.snapshot_to_compare(ended.profile, report, then.with, focus);
                return;
            }
            (outcome, _) => outcome,
        };
        // The message boxes are titled after the command that took the snapshot.
        let command = match then {
            Some(_) => CommandId::SnapshotAndCompare,
            None => CommandId::TakeSnapshot,
        };
        self.sync_jobs();
        match outcome {
            Ok(Outcome::Done(report)) => {
                self.report_snapshot(&ended.profile, &report, !stopped, focus);
            }
            Ok(Outcome::Failed(SnapshotError::Store(e))) if !stopped => {
                self.snapshot_failed(command, &s.failure(&failure::of_store(&e)));
            }
            Err(Lost) if !stopped => self.snapshot_failed(command, s.snapshot_stopped()),
            // A snapshot stopped on closing or restarting is lost however it ended.
            _ => self.say(s.snapshot_cancelled()),
        }
    }

    /// Brings the window in line with the jobs once one has started or ended: the flags
    /// of the state, the progress row and the menus.
    fn sync_jobs(&self) {
        self.sync_job_flags();
        self.refresh_progress_row();
        self.refresh_menus();
    }

    /// The one place the jobs' flags of the state are set: as the jobs give them for the
    /// open profile (ADR-0011).
    fn sync_job_flags(&self) {
        let open = self.open_profile();
        let flags = self
            .jobs
            .borrow()
            .flags(open.as_ref().map(|p| p.path.as_path()));
        self.state.borrow_mut().set_jobs(flags);
    }

    /// Shows the progress row for the job it shows, with the Cancel button named after
    /// that job's command, or hides it when there is none; the focus leaves a hidden row
    /// for the snapshot list. The timer runs while any job does: a comparison is polled
    /// before its row shows (TASK-11 #7).
    fn refresh_progress_row(&self) {
        let s = self.s;
        // The button is «Скасувати (N)» for a snapshot and a comparison and, so that its
        // letter is in its English name as everywhere, the command's own name for a
        // compaction.
        let shown = match self.jobs.borrow().shown() {
            Some(Shown::Snapshot(_)) => Some((
                Kind::Snapshot,
                s.snapshot_progress(0, &win32::format_count(0), ""),
                button_label(s, s.cancel(), cancel_snapshot_access(), false),
            )),
            Some(Shown::Comparison { progress, elapsed }) => Some((
                Kind::Comparison,
                self.comparison_progress_text(progress, elapsed),
                button_label(s, s.cancel(), cancel_comparison_access(), false),
            )),
            Some(Shown::Compaction(profile)) => Some((
                Kind::Compaction,
                s.compacting(&profile.name),
                button_label(
                    s,
                    s.command(CommandId::CancelCompaction),
                    cancel_compaction_access(),
                    false,
                ),
            )),
            None => None,
        };
        let kind = shown.as_ref().map(|(kind, ..)| *kind);
        if kind != Some(Kind::Comparison) {
            self.said_waiting.set(None);
            self.gauge.show(false);
        }
        match &shown {
            None => {
                let focused = win32::focused_window();
                if focused == self.progress_text.get_handle()
                    || focused == self.cancel_button.get_handle()
                {
                    self.snapshots.set_focus();
                }
            }
            Some((kind, text, button)) => {
                // A snapshot's text is replaced by its progress on the next tick.
                if *kind != Kind::Snapshot || self.progress_text.get_value().is_empty() {
                    self.progress_text.change_value(text);
                }
                // The same label is not set again: the comparison of «Знімок +
                // порівняння» takes the row over from its snapshot under the same
                // «Скасувати (N)» (TASK-12 #9).
                if self.cancel_button.get_label() != *button {
                    self.cancel_button.set_label(button);
                }
            }
        }
        if self.jobs.borrow().any() {
            if !self.timer.is_running() {
                self.timer.start(PROGRESS_INTERVAL, false);
            }
        } else {
            self.timer.stop();
        }
        self.shown_kind.set(kind);
        let visible = shown.is_some();
        self.progress_label.show(visible);
        self.progress_text.show(visible);
        self.cancel_button.show(visible);
        if !visible {
            self.progress_text.change_value("");
        }
        self.panel.layout();
    }

    /// Puts the saved snapshot in its list as the setting «Фокус після знімка» says
    /// (TASK-9 #5, #19–#21), says what it holds, names the folders it did not find, and,
    /// with `offer_restart`, offers a restart as administrator when rights closed some
    /// entries (TASK-8 #7, #16, #18).
    fn report_snapshot(
        &self,
        profile: &ProfileFile,
        report: &Report,
        offer_restart: bool,
        focus: FocusPlace,
    ) {
        match arrival(self.focus_after_snapshot(), focus, self.is_open(profile)) {
            Arrival::Nothing => {}
            how @ (Arrival::Append | Arrival::Current | Arrival::Focus) => {
                self.append_row(snapshot_row(report), how);
            }
            Arrival::OpenProfileAndFocus => {
                if self.open_profile_at(&profile.path) {
                    self.snapshots.set_focus();
                }
            }
        }
        self.refresh_menus();
        self.say(&self.snapshot_report(report));
        if offer_restart {
            self.offer_restart(profile, report, CommandId::TakeSnapshot);
        }
    }

    /// What a saved snapshot says (TASK-8): how many entries it holds, how many of them
    /// are inaccessible, and which folders it did not find.
    fn snapshot_report(&self, report: &Report) -> String {
        let entries = win32::format_count(report.entries);
        self.snapshot_report_after(self.s.snapshot_saved(report.entries, &entries), report)
    }

    /// `first`, then what a saved snapshot says after how many entries it holds: how many
    /// of them are inaccessible, and which folders it did not find.
    fn snapshot_report_after(&self, first: String, report: &Report) -> String {
        let s = self.s;
        let mut text = first;
        if report.inaccessible > 0 {
            text.push(' ');
            text.push_str(&s.entries_inaccessible(&win32::format_count(report.inaccessible)));
        }
        let not_found: Vec<(String, String)> = report
            .not_found()
            .map(|f| {
                (
                    f.path.to_string_lossy().into_owned(),
                    self.folder_state_reason(&f.state),
                )
            })
            .collect();
        if !not_found.is_empty() {
            text.push(' ');
            text.push_str(&s.folders_not_found(&not_found));
        }
        text
    }

    /// Offers a restart as administrator after the snapshot of `report` when rights
    /// closed some of its entries and revisor runs as a standard user (TASK-8 #7, #16,
    /// #18); the question is titled after `command`, which took the snapshot.
    fn offer_restart(&self, profile: &ProfileFile, report: &Report, command: CommandId) {
        let s = self.s;
        if report.denied > 0 && !self.state.borrow().elevated {
            let question = s.restart_question(&win32::format_count(report.denied));
            let restart = (s.restart(), RESTART_ACCESS);
            if confirm(&self.frame, s, s.command(command), &question, restart, true) {
                // On the snapshotted profile, whichever is open by now.
                self.restart_now(Some(profile));
            }
        }
    }

    /// Why the snapshot did not scan a folder, in the interface's words.
    fn folder_state_reason(&self, state: &FolderState) -> String {
        let s = self.s;
        match state {
            FolderState::Scanned { .. } => String::new(),
            FolderState::Missing => s.folder_missing().into(),
            FolderState::Unavailable(Unavailable::NotNtfs(fs)) => s.folder_volume_not_ntfs(fs),
            FolderState::Unavailable(Unavailable::Network) => s.folder_volume_network().into(),
            FolderState::Unavailable(Unavailable::NotAFolder) => s.folder_is_a_file().into(),
            FolderState::Unavailable(Unavailable::Error(code)) => {
                let error = io::Error::from_raw_os_error(*code);
                s.folder_unreachable(&s.failure(&failure::of_io(&error)))
            }
        }
    }

    /// Says why the snapshot was not saved, in the status bar and in a message box titled
    /// after `command`, which took it (TASK-8; TASK-12 #10).
    fn snapshot_failed(&self, command: CommandId, reason: &str) {
        let s = self.s;
        let text = s.snapshot_failed(reason);
        self.say(&text);
        tell(&self.frame, s, s.command(command), &text);
    }

    /// Stops the snapshot being taken, if there is one, once the user agrees to
    /// `question`; returns whether there is nothing in the way any more. The timer rests
    /// while the question is up, so the snapshot's end does not answer it.
    fn stop_snapshot_if_agreed(&self, title: &str, question: &str, action: (&str, u8)) -> bool {
        if !self.jobs.borrow().runs(Kind::Snapshot) {
            return true;
        }
        let agreed =
            self.with_jobs_paused(|| confirm(&self.frame, self.s, title, question, action, true));
        if !agreed {
            return false;
        }
        self.stop_snapshot();
        true
    }

    /// Stops the snapshot being taken, if there is one, without asking. A snapshot that
    /// ended meanwhile, while a question was up, is reported as it ended.
    fn stop_snapshot(&self) {
        let stopped = self.jobs.borrow_mut().stop_snapshot();
        if let Some(ended) = stopped {
            self.timer.stop();
            self.snapshot_ended(ended, None, true);
        }
    }

    /// Restarts revisor as administrator on the open profile; during a snapshot, once the
    /// user agrees to lose it (TASK-8 #30).
    fn restart_as_admin(&self) {
        let s = self.s;
        let agreed = self.stop_snapshot_if_agreed(
            s.command(CommandId::RestartAsAdmin),
            s.restart_during_snapshot(),
            (s.restart(), RESTART_ACCESS),
        );
        if agreed {
            self.restart_now(self.open_profile().as_ref());
        }
    }

    /// Starts revisor again as administrator, on `profile`, and closes this one.
    fn restart_now(&self, profile: Option<&ProfileFile>) {
        let s = self.s;
        let args = profile
            .map(|p| format!("--profile \"{}\"", p.path.display()))
            .unwrap_or_default();
        let started = std::env::current_exe().and_then(|exe| win32::run_as_admin(&exe, &args));
        match started {
            Ok(()) => self.frame.close(true),
            Err(e) => self.say(&s.restart_failed(&s.failure(&failure::of_io(&e)))),
        }
    }

    /// Starts revisor again with the rights this one has, on `profile`, and closes this
    /// one (TASK-16 #4): a child process inherits the token of its parent, so a revisor
    /// run as administrator starts another administrator, without the UAC prompt, and
    /// a standard one another standard one.
    fn restart_with_same_rights(&self, profile: Option<&ProfileFile>) {
        let s = self.s;
        let started = std::env::current_exe().and_then(|exe| {
            let mut command = std::process::Command::new(exe);
            if let Some(profile) = profile {
                command.arg("--profile").arg(&profile.path);
            }
            command.spawn().map(drop)
        });
        match started {
            Ok(()) => self.frame.close(true),
            Err(e) => self.say(&s.restart_failed(&s.failure(&failure::of_io(&e)))),
        }
    }

    /// Greys out the commands that cannot run now and sets the check and radio marks.
    fn refresh_menus(&self) {
        let state = self.state.borrow();
        for c in COMMANDS {
            let id = wx_id(c.id);
            self.menu_bar.enable_item(id, (c.available)(&state).is_ok());
            if let Some(checked) = c.checked {
                let checked = checked(&state);
                // Checking a radio item clears the others of its group.
                if checked || c.kind == CommandKind::Check {
                    self.menu_bar.check_item(id, checked);
                }
            }
        }
    }

    /// Opens a list's context menu at `at` (screen coordinates), or by its focused row.
    fn show_context_menu(&self, pane: Pane, at: Option<(i32, i32)>) {
        let menu_items = context_menu(pane, &self.state.borrow());
        if menu_items.groups.is_empty() {
            self.say(self.s.no_actions());
            return;
        }
        let mut menu = Menu::builder().build();
        for (i, group) in menu_items.groups.iter().enumerate() {
            if i > 0 {
                menu.append_separator();
            }
            for &id in group {
                menu.append(
                    wx_id(id),
                    &context_label(self.s, command(id)),
                    "",
                    ItemKind::Normal,
                );
            }
        }
        let (x, y) = at.unwrap_or_else(|| win32::context_menu_point(self.list(pane).get_handle()));
        win32::bold_first_item_of_next_popup(menu_items.default.is_some());
        self.frame.popup_menu(&mut menu, Some(Point::new(x, y)));
        win32::bold_first_item_of_next_popup(false);
    }
}

fn build_menu_bar(s: &dyn Strings) -> MenuBar {
    let item_kind = |kind| match kind {
        CommandKind::Normal => ItemKind::Normal,
        CommandKind::Check => ItemKind::Check,
        CommandKind::Radio => ItemKind::Radio,
    };
    let mut bar = MenuBar::builder();
    for menu_id in MenuId::ALL {
        let menu = Menu::builder().build();
        let mut last_group = None;
        let mut submenus_done = Vec::new();
        for c in COMMANDS.iter().filter(|c| c.place.menu == menu_id) {
            if let Some((sub, _)) = c.place.submenu {
                if submenus_done.contains(&sub) {
                    continue;
                }
                submenus_done.push(sub);
            }
            if last_group.is_some_and(|g| g != c.place.group) {
                menu.append_separator();
            }
            last_group = Some(c.place.group);
            match c.place.submenu {
                None => {
                    menu.append(wx_id(c.id), &menu_label(s, c), "", item_kind(c.kind));
                }
                Some((sub, _)) => {
                    let submenu = Menu::builder().build();
                    let mut last_sub_group = None;
                    for item in COMMANDS
                        .iter()
                        .filter(|i| i.place.submenu.is_some_and(|(id, _)| id == sub))
                    {
                        let group = item.place.submenu.map(|(_, g)| g);
                        if last_sub_group.is_some_and(|g| g != group) {
                            submenu.append_separator();
                        }
                        last_sub_group = Some(group);
                        submenu.append(
                            wx_id(item.id),
                            &menu_label(s, item),
                            "",
                            item_kind(item.kind),
                        );
                    }
                    menu.append_submenu(submenu, &submenu_label(s, sub), "");
                }
            }
        }
        bar = bar.append(menu, &top_menu_label(s, menu_id));
    }
    bar.build()
}

/// The row of the snapshot list for the snapshot of `report`, just saved: it has no
/// comment yet.
fn snapshot_row(report: &Report) -> SnapshotInfo {
    SnapshotInfo {
        id: report.id,
        taken_at: report.taken_at,
        comment: None,
    }
}

/// What the profile window says when a profile cannot be saved.
fn save_failure(s: &dyn Strings, settings: &ProfileSettings, e: &ProfileError) -> String {
    let failure = match e {
        // Another copy of revisor may have taken the name meanwhile.
        ProfileError::NameTaken => return s.name_taken(&settings.name),
        ProfileError::Store(e) => failure::of_store(e),
        ProfileError::Io(e) => failure::of_io(e),
    };
    s.profile_not_saved(&s.failure(&failure))
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .into_owned()
}

/// Gives the window the same icon Explorer shows for the exe. wxMSW does not adopt the
/// exe's icon resource for a frame, so the frame takes it from the SVG that app.ico is
/// generated from. A failure leaves the generic window icon, which is not worth failing over.
fn set_frame_icon(frame: &Frame) {
    const ICON: &[u8] = include_bytes!("../resources/icon.svg");
    if let Some(bitmap) = BitmapBundle::from_svg_data(ICON, Size::new(32, 32))
        .and_then(|bundle| bundle.get_bitmap_for(frame))
    {
        frame.set_icon(&bitmap);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::En;

    #[test]
    fn the_labels_of_the_main_window_take_no_menu_letter_and_differ() {
        for cancel in [
            cancel_snapshot_access(),
            cancel_comparison_access(),
            cancel_compaction_access(),
        ] {
            let mut labels = vec![
                PROFILE_ACCESS,
                PROGRESS_ACCESS,
                cancel,
                COMPARISON_ACCESS,
                FILTER_ACCESS,
            ];
            labels.extend(TYPE_ACCESS);
            for (i, &label) in labels.iter().enumerate() {
                assert!(MenuId::ALL.iter().all(|m| m.access() != label));
                assert!(!labels[..i].contains(&label), "{}", char::from(label));
            }
            assert_eq!(labels[LABEL_PROFILE], PROFILE_ACCESS);
            assert_eq!(labels[LABEL_PROGRESS], PROGRESS_ACCESS);
            assert_eq!(labels[LABEL_COMPARISON], COMPARISON_ACCESS);
            assert_eq!(labels[LABEL_FILTER], FILTER_ACCESS);
            assert_eq!(labels[LABEL_TYPES..], TYPE_ACCESS);
        }
        assert_ne!(cancel_snapshot_access(), cancel_compaction_access());
        // The letters are in the English names, as everywhere.
        let has = |name: &str, letter: u8| name.to_ascii_uppercase().contains(char::from(letter));
        assert!(has(En.profile(), PROFILE_ACCESS));
        assert!(has(En.progress(), PROGRESS_ACCESS));
        assert!(has(En.comparison_field(), COMPARISON_ACCESS));
        assert!(has(En.filter_field(), FILTER_ACCESS));
        for kind in ChangeType::ALL {
            let i = kind as usize;
            assert!(
                has(En.command(TYPE_COMMANDS[i]), TYPE_ACCESS[i]),
                "{kind:?}"
            );
        }
        assert!(has(En.cancel(), cancel_snapshot_access()));
        assert!(has(En.cancel(), cancel_comparison_access()));
        assert!(has(
            En.command(CommandId::CancelCompaction),
            cancel_compaction_access()
        ));
        assert!(has(En.restart(), RESTART_ACCESS));
        assert!(has(En.restart_now(), RESTART_NOW_ACCESS));
        assert!(has(En.later(), LATER_ACCESS));
        assert_ne!(RESTART_NOW_ACCESS, LATER_ACCESS);
        assert!(has(En.close(), CLOSE_ACCESS));
        assert!(has(En.delete(), DELETE_ACCESS));
        assert!(has(&En.comment_of("x"), COMMENT_ACCESS));
    }

    #[test]
    fn the_cancel_button_keeps_its_label_from_a_snapshot_to_its_comparison() {
        // «Знімок + порівняння» goes from its snapshot to its comparison under one Cancel
        // button, whose label does not change under the focus (TASK-12 #9).
        assert_eq!(cancel_snapshot_access(), cancel_comparison_access());
        for s in [&En as &dyn Strings, &crate::i18n::Uk] {
            assert_eq!(
                button_label(s, s.cancel(), cancel_snapshot_access(), false),
                button_label(s, s.cancel(), cancel_comparison_access(), false)
            );
        }
    }
}
