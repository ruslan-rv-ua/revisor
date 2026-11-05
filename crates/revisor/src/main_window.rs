//! The main window: the menu bar and context menus built from the command table, the
//! status bar that speaks without moving the focus, the key handler, the two parts of
//! the window, the snapshot being taken, with its progress and its outcome (TASK-8), the
//! snapshot list of the open profile with its comments, deletion and the compaction
//! that follows it (TASK-9), and the comparison of the current snapshot with another,
//! made in the background, whose difference the difference list shows (TASK-11). The
//! profile box lists the profiles (TASK-7).

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
use revisor_core::settings::{FocusAfterComparison, FocusAfterSnapshot, Language, Settings};
use revisor_core::snapshot::{Phase, Report, SnapshotError};
use revisor_core::store::{
    Compacted, FolderState, ProfileSettings, SnapshotInfo, Store, StoreError, Unavailable,
};
use wxdragon::prelude::*;

use crate::commands::{COMMANDS, CommandId, CommandKind, MenuId, Pane, command};
use crate::compare_window;
use crate::difference::{self, Rows};
use crate::failure;
use crate::help;
use crate::i18n::Strings;
use crate::jobs::{Ended, Event, Jobs, Kind, Shown};
use crate::keyboard::{Control, Focus, Route, route};
use crate::keys::Shortcut;
use crate::menus::{
    button_label, context_label, context_menu, field_label, menu_label, submenu_label,
    top_menu_label, window_title,
};
use crate::message_box::{ask_text, choose, confirm, tell};
use crate::profile_window::{self, Purpose};
use crate::settings_window::{self, Values};
use crate::snapshots::{Arrival, FocusPlace, arrival, row_after_deletion};
use crate::state::{AppState, ChangeType, SortColumn};
use crate::status::Announcer;
use crate::win32;

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

/// The widths of the difference list's columns: the kind of the change is short, the
/// path takes the rest (TASK-11).
const CHANGE_WIDTH: i32 = 130;
const PATH_WIDTH: i32 = 700;

/// The access keys of the labels of the main window. They do not take the letters of the
/// menus, which would no longer open while the focus is in the panel.
const PROFILE_ACCESS: u8 = b'R';
const PROGRESS_ACCESS: u8 = b'P';

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
/// `Route::FocusLabel` names them by.
const LABEL_PROFILE: usize = 0;
const LABEL_PROGRESS: usize = 1;
const LABEL_CANCEL: usize = 2;

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
    /// The difference lists: two virtual lists over `difference_rows`, which they ask for
    /// the text of the rows they show (TASK-11 #5), one of them shown at a time. A new
    /// difference goes into the other list, which then takes the shown one's place: the
    /// focus comes to it as to a new list, which screen readers name again even when the
    /// focus was in the old one (#21); nothing says so when the rows under the focus are
    /// merely replaced.
    differences: [ListCtrl; 2],
    /// Which of `differences` is shown.
    shown_difference: Cell<usize>,
    difference_rows: Rc<RefCell<Rows>>,
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
    /// The control that had the keyboard focus when the comparison was started, for the
    /// setting «Фокус після порівняння» (TASK-11 #12).
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
        // Virtual lists: a difference may have a million rows (TASK-11 #6), and a list
        // asks for the text of the ones it shows. Two of them, the second hidden at first.
        let difference_rows = Rc::new(RefCell::new(Rows::default()));
        let differences = [(); 2].map(|()| {
            let list = ListCtrl::builder(&panel)
                .with_style(
                    ListCtrlStyle::Report | ListCtrlStyle::Virtual | ListCtrlStyle::SingleSel,
                )
                .build();
            list.insert_column(0, s.column_change(), ListColumnFormat::Left, CHANGE_WIDTH);
            list.insert_column(1, s.column_path(), ListColumnFormat::Left, PATH_WIDTH);
            win32::full_row_select_no_sort(list.get_handle());
            win32::set_accessible_name(list.get_handle(), s.pane(Pane::Difference));
            let rows_shown = Rc::clone(&difference_rows);
            list.set_virtual_text_callback(move |row, column| {
                let rows = rows_shown.borrow();
                let row = usize::try_from(row).unwrap_or(usize::MAX);
                match column {
                    0 => rows
                        .kind(row)
                        .map(|kind| s.change_kind(kind).to_string())
                        .unwrap_or_default(),
                    _ => rows
                        .path(row)
                        .map(|path| path.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                }
            });
            list
        });
        differences[1].show(false);
        // The panel answers for the lists' text while they are empty.
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
        let sizer = BoxSizer::builder(Orientation::Vertical).build();
        sizer.add_sizer(&profile_row, 0, SizerFlag::Expand, 0);
        sizer.add_sizer(&progress_row, 0, SizerFlag::Expand, 0);
        sizer.add(&snapshots, 1, SizerFlag::Expand | SizerFlag::All, 5);
        for list in &differences {
            sizer.add(list, 2, SizerFlag::Expand | SizerFlag::All, 5);
        }
        panel.set_sizer(sizer, true);
        let timer = Timer::new(&frame);

        let settings_path = data_dir.join(SETTINGS_FILE);
        let settings =
            Settings::open(&settings_path).map_err(|e| s.failure(&failure::of_settings(&e)));
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
            differences,
            shown_difference: Cell::new(0),
            difference_rows,
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

    /// The access keys of the main window's labels and buttons, for `keyboard::route`:
    /// those of the progress row only while it is shown, with the Cancel button's letter
    /// being that of the command of the job shown.
    fn labels(&self) -> Vec<u8> {
        match self.shown_kind() {
            Some(kind) => vec![
                PROFILE_ACCESS,
                PROGRESS_ACCESS,
                command(cancel_command(kind)).access,
            ],
            None => vec![PROFILE_ACCESS],
        }
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
    /// #21): in the difference list, still where it was when the comparison started, on
    /// the comparison's progress, or elsewhere.
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
            Route::FocusLabel(_) => {}
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
            CommandId::Comment => self.comment(),
            CommandId::DeleteSnapshots => self.delete_snapshots(),
            CommandId::CancelSnapshot => self.cancel_snapshot(),
            CommandId::CancelCompaction => self.cancel_compaction(),
            CommandId::CompareWithPrevious => self.compare_with_previous(),
            CommandId::CompareWith => self.compare_with(),
            CommandId::CancelComparison => self.cancel_comparison(),
            CommandId::KeyboardShortcuts => help::show_shortcuts(&self.frame, s),
            CommandId::About => help::show_about(&self.frame, s),
            // Only the demo state gets here: the other commands' tasks are not done yet.
            _ => {
                self.demo_run(id);
                self.say(&s.demo_done(s.command(id)));
            }
        }
        self.refresh_menus();
        true
    }

    /// What the demo state does for a command: the filter and sort items keep their marks.
    fn demo_run(&self, id: CommandId) {
        let mut state = self.state.borrow_mut();
        match id {
            CommandId::ShowAdded => state.toggle_shown(ChangeType::Added),
            CommandId::ShowDeleted => state.toggle_shown(ChangeType::Deleted),
            CommandId::ShowModified => state.toggle_shown(ChangeType::Modified),
            CommandId::ShowInaccessible => state.toggle_shown(ChangeType::Inaccessible),
            CommandId::SortByPath => state.sort = SortColumn::Path,
            CommandId::SortByName => state.sort = SortColumn::Name,
            CommandId::SortByChange => state.sort = SortColumn::Change,
            CommandId::SortByWhatChanged => state.sort = SortColumn::WhatChanged,
            CommandId::SortBySize => state.sort = SortColumn::Size,
            CommandId::SortByDateModified => state.sort = SortColumn::DateModified,
            CommandId::SortByFolder => state.sort = SortColumn::Folder,
            CommandId::SortAscending => state.descending = false,
            CommandId::SortDescending => state.descending = true,
            _ => {}
        }
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
            self.load_profiles(Some(&file.path));
            self.say(&s.profile_changed(&file.name));
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
    /// it has run for a second (#7).
    fn start_comparison(&self, profile: ProfileFile, a: i64, b: i64) {
        self.comparison_focus.set(win32::focused_window());
        self.jobs.borrow_mut().start_comparison(profile, a, b);
        self.sync_jobs();
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
    /// with it.
    fn comparison_ended(&self, ended: Ended<Outcome<Difference, CompareError>>) {
        let s = self.s;
        let focus = self.comparison_focus_place();
        self.sync_jobs();
        match ended.outcome {
            Ok(Outcome::Done(difference)) => self.show_difference(difference, focus),
            Ok(Outcome::Cancelled) => self.say(s.comparison_cancelled()),
            Ok(Outcome::Failed(e)) => {
                let reason = s.failure(&failure::of_compare(&e));
                self.say(&s.comparison_failed(&reason));
            }
            Err(Lost) => self.say(&s.comparison_failed(s.comparison_stopped())),
        }
    }

    /// Puts `difference` in the difference list that is not shown, which then takes the
    /// place of the shown one, with the focus moving to its first row, or the end said
    /// without it, as its arrival says (#12–#14, #21).
    fn show_difference(&self, difference: Difference, focus: difference::FocusPlace) {
        let s = self.s;
        let count = {
            let mut rows = self.difference_rows.borrow_mut();
            rows.replace(difference);
            rows.len()
        };
        let old = self.difference();
        let next = 1 - self.shown_difference.get();
        let new = &self.differences[next];
        new.set_item_count(count as i64);
        if !self.difference_rows.borrow().is_empty() {
            win32::select_only(new.get_handle(), 0);
        }
        new.show(true);
        self.shown_difference.set(next);
        self.state.borrow_mut().has_difference = true;
        match difference::arrival(self.focus_after_comparison(), focus) {
            difference::Arrival::Replace => self.say(s.comparison_finished()),
            // The focus says it: the status bar only stops showing the comparison's
            // progress, without a word from the screen reader.
            difference::Arrival::Focus => {
                self.status.set_status_text(s.comparison_finished(), 0);
                new.set_focus();
            }
        }
        old.show(false);
        old.set_item_count(0);
        self.panel.layout();
        self.refresh_menus();
    }

    /// The progress field for the comparison the row shows, with the bar once the
    /// comparison has run for [`COMPARISON_BAR_AFTER`] (#8). A phase the row has not
    /// said yet — comparing, or waiting for the database — is said in the status bar,
    /// which screen readers read without the focus moving (#7, #16).
    fn comparison_progress_text(&self, progress: &compare::Progress, elapsed: Duration) -> String {
        let s = self.s;
        let waiting = progress.phase() == compare::Phase::Waiting;
        let phase_text = if waiting {
            s.waiting_for_database()
        } else {
            s.comparing()
        };
        if self.said_waiting.get() != Some(waiting) {
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
                    Event::Snapshot(ended) => self.snapshot_ended(ended, false),
                    Event::Comparison(ended) => self.comparison_ended(ended),
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
    /// read before the progress row goes and takes the focus with it.
    fn snapshot_ended(&self, ended: Ended<Outcome<Report, SnapshotError>>, stopped: bool) {
        let s = self.s;
        let focus = self.focus_place();
        self.sync_jobs();
        match ended.outcome {
            Ok(Outcome::Done(report)) => {
                self.report_snapshot(&ended.profile, &report, !stopped, focus);
            }
            Ok(Outcome::Failed(SnapshotError::Store(e))) if !stopped => {
                self.snapshot_failed(&s.failure(&failure::of_store(&e)));
            }
            Err(Lost) if !stopped => self.snapshot_failed(s.snapshot_stopped()),
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
                self.cancel_button.set_label(button);
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
        let s = self.s;
        let of_open_profile = self
            .open_profile()
            .is_some_and(|open| open.path == profile.path);
        let row = SnapshotInfo {
            id: report.id,
            taken_at: report.taken_at,
            comment: None,
        };
        match arrival(self.focus_after_snapshot(), focus, of_open_profile) {
            Arrival::Nothing => {}
            how @ (Arrival::Append | Arrival::Current | Arrival::Focus) => {
                self.append_row(row, how);
            }
            Arrival::OpenProfileAndFocus => {
                let index = self
                    .profiles
                    .borrow()
                    .iter()
                    .position(|p| p.path == profile.path);
                if let Some(index) = index {
                    self.profile.set_selection(index as u32);
                    self.show_profile(Some(index));
                    self.snapshots.set_focus();
                }
            }
        }
        self.refresh_menus();
        let mut text = s.snapshot_saved(report.entries, &win32::format_count(report.entries));
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
        self.say(&text);
        if offer_restart && report.denied > 0 && !self.state.borrow().elevated {
            let question = s.restart_question(&win32::format_count(report.denied));
            let restart = (s.restart(), RESTART_ACCESS);
            let title = s.command(CommandId::TakeSnapshot);
            if confirm(&self.frame, s, title, &question, restart, true) {
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

    fn snapshot_failed(&self, reason: &str) {
        let s = self.s;
        let text = s.snapshot_failed(reason);
        self.say(&text);
        tell(&self.frame, s, s.command(CommandId::TakeSnapshot), &text);
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
            self.snapshot_ended(ended, true);
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
        for labels in [
            [PROFILE_ACCESS, PROGRESS_ACCESS, cancel_snapshot_access()],
            [PROFILE_ACCESS, PROGRESS_ACCESS, cancel_comparison_access()],
            [PROFILE_ACCESS, PROGRESS_ACCESS, cancel_compaction_access()],
        ] {
            for (i, &label) in labels.iter().enumerate() {
                assert!(MenuId::ALL.iter().all(|m| m.access() != label));
                assert!(!labels[..i].contains(&label));
            }
            assert_eq!(labels[LABEL_PROFILE], PROFILE_ACCESS);
            assert_eq!(labels[LABEL_PROGRESS], PROGRESS_ACCESS);
        }
        assert_ne!(cancel_snapshot_access(), cancel_compaction_access());
        // The letters are in the English names, as everywhere.
        let has = |name: &str, letter: u8| name.to_ascii_uppercase().contains(char::from(letter));
        assert!(has(En.profile(), PROFILE_ACCESS));
        assert!(has(En.progress(), PROGRESS_ACCESS));
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
}
