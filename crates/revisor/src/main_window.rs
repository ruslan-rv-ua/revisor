//! The main window: the menu bar and context menus built from the command table, the
//! status bar that speaks without moving the focus, the key handler, the two parts of
//! the window, the snapshot being taken, with its progress and its outcome (TASK-8), the
//! snapshot list of the open profile with its comments, deletion and the compaction
//! that follows it (TASK-9), the comparison of the current snapshot with another, made
//! in the background, whose difference the difference list shows (TASK-11) with the field
//! «Порівняння» above it and its filter under it (TASK-13), the details of its change in
//! focus and the change shown in File Explorer (TASK-14), and «Знімок + порівняння», which
//! chains a snapshot and its comparison (TASK-12). The profile box lists the profiles
//! (TASK-7). The difference shows as a list or, in the same place, as a tree of its
//! folders (TASK-33).

use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::ptr;
use std::rc::Rc;
use std::thread;
use std::time::Duration;

use revisor_core::compare::{self, CompareError, Difference};
use revisor_core::job::{Cancel, Lost, Outcome};
use revisor_core::profiles::{self, ProfileError, ProfileFile, Unreadable};
use revisor_core::settings::{
    DifferenceSort, DifferenceView, FocusAfterComparison, FocusAfterSnapshot, Language, Settings,
};
use revisor_core::snapshot::{Phase, Report, SnapshotError};
use revisor_core::store::{
    Compacted, FolderState, ProfileSettings, SnapshotInfo, Store, StoreError, Unavailable,
};
use wxdragon::prelude::*;

use crate::access::{Dispatcher, Name, Target, compose, field_label, plain};
use crate::commands::{COMMANDS, CommandId, CommandKind, MenuId, Pane, Reason, command};
use crate::compare_window;
use crate::difference::{self, Column, List, Location, View};
use crate::difference_tree::{self, TreeView};
use crate::explorer::{self, Reveal};
use crate::failure::reason;
use crate::help;
use crate::i18n::{En, Strings};
use crate::jobs::{
    COMPARISON_SHOWN_AFTER, Comment, Ended, Event, Jobs, Kind, Shown, Superseded, ThenCompare,
};
use crate::keyboard::{Control, Focus, Label, Route, route};
use crate::keys::Shortcut;
use crate::menus::{
    context_label, context_menu, menu_label, submenu_label, top_menu_label, window_title,
};
use crate::message_box::{ask_text, choose, confirm, tell};
use crate::profile_window::{self, Purpose};
use crate::settings_window::{self, Values};
use crate::snapshot_list::{Native, SnapshotList};
use crate::snapshots::{Arrival, arrival, arrival_after_comparison};
use crate::state::{AppState, ChangeType, SortColumn, Sources};
use crate::status::Announcer;
use crate::text_window;
use crate::win32;
use crate::win32::format::{Locale, RegionalFormat};
use crate::win32::frame::MainFrame;
use crate::win32::lists::EmptyTexts;

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

/// How long a comparison runs before its progress gets a bar with the percent done
/// (TASK-11 #8); the progress row itself shows it after [`crate::jobs::COMPARISON_SHOWN_AFTER`].
const COMPARISON_BAR_AFTER: Duration = Duration::from_secs(5);

/// How long the window waits between its looks at a comparison that still reads the
/// database it is about to write or delete (TASK-11 #17).
const HOLD_RETRY: Duration = Duration::from_millis(50);

/// The file of the application's settings in the data folder (ADR-0007).
pub const SETTINGS_FILE: &str = "settings.db";

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

/// The command that cancels a job of `kind`: what the progress row's button presses.
fn cancel_command(kind: Kind) -> CommandId {
    match kind {
        Kind::Snapshot => CommandId::CancelSnapshot,
        Kind::Comparison => CommandId::CancelComparison,
        Kind::Compaction => CommandId::CancelCompaction,
        Kind::Comment => CommandId::CancelComment,
    }
}

/// The English of the progress row's button while it shows a job of `kind`: «Скасувати
/// (N)» for a snapshot, a comparison and a comment waiting for the database (TASK-24), so
/// that the button keeps its label from the snapshot of «Знімок + порівняння» to its
/// comparison (TASK-12 #9) — no letter of «Cancel comment» is free in the main window,
/// where M is the field «Порівняння» —, and the command's own name for a compaction.
fn cancel_english(kind: Kind) -> &'static str {
    match kind {
        Kind::Compaction => En.command(CommandId::CancelCompaction),
        Kind::Snapshot | Kind::Comparison | Kind::Comment => En.cancel_job(),
    }
}

/// The label of the progress row's button while it shows a job of `kind`.
fn cancel_label(s: &dyn Strings, kind: Kind) -> String {
    let words = match kind {
        Kind::Compaction => s.command(CommandId::CancelCompaction),
        Kind::Snapshot | Kind::Comparison | Kind::Comment => s.cancel_job(),
    };
    compose(s, cancel_english(kind), words)
}

/// The labelled controls of the main window that Alt with a letter reaches while the
/// progress row shows a job of `shown`, if any: the row's field and button only then.
/// Their letters are no letters of the menu bar, which would no longer open while the
/// focus is in the panel.
pub(crate) fn targets(shown: Option<Kind>) -> Vec<Target<Label>> {
    let target = |english: &str, label: Label| Target::new(english, label.act(), label);
    let mut targets = vec![target(En.profile(), Label::Profile)];
    if let Some(kind) = shown {
        targets.push(target(En.progress(), Label::Progress));
        targets.push(target(cancel_english(kind), Label::Cancel));
    }
    targets.push(target(En.comparison_field(), Label::Comparison));
    targets.push(target(En.filter_field(), Label::Filter));
    for kind in ChangeType::ALL {
        targets.push(target(En.type_box(kind), Label::Type(kind)));
    }
    targets
}

/// Where a window event came from, which decides where its failures go (ADR-0013).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    /// An explicit command: its failure opens a window titled after it. The end of a
    /// snapshot and of a comment that waited for the database is the end of their
    /// command, which the user waits for.
    Command(CommandId),
    /// The end of a comparison or of a compaction: its failure goes to the status bar.
    Background,
    /// Opening the window, showing a profile, moving in the lists, the filter, the jobs'
    /// timer: a failure goes to the status bar.
    Navigation,
}

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
    /// The frame's subclass, through which its context menus open (TASK-32).
    main_frame: MainFrame,
    s: &'static dyn Strings,
    /// What the commands decide from; built afresh at the end of every event (ADR-0013).
    state: RefCell<AppState>,
    /// Whether revisor runs as administrator (TASK-8).
    elevated: bool,
    /// The source of the event being handled, or `None` between events (ADR-0013).
    source: Cell<Option<Source>>,
    /// Where the keyboard focus is, read once per event, the first time it is asked for,
    /// with the window it is on (ADR-0013).
    event_focus: Cell<Option<(Focus, *mut c_void)>>,
    /// Set once the window is closing: the event then neither polls the jobs nor starts
    /// their timer again.
    closing: Cell<bool>,
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
    /// The snapshot list of the open profile, with its rows (TASK-27).
    snapshots: SnapshotList<Native>,
    /// The field «Порівняння» above the difference list: the pair of snapshots, the
    /// summary and the warnings, or, before the first comparison, how to compare
    /// (TASK-13 #1, #11).
    comparison_field: TextCtrl,
    /// The panel the difference lists take turns in: the one shown fills it, the other is
    /// disabled and has no size (TASK-13).
    difference_holder: Panel,
    /// The texts the holder shows for the difference lists while they are empty.
    difference_empty_texts: EmptyTexts,
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
    /// Which of `differences` is shown, while the difference shows as a list.
    shown_difference: Cell<usize>,
    /// The difference trees (TASK-33): two tree controls over `difference_list`, which
    /// take turns as the lists do — a new difference goes into the other one, so screen
    /// readers name the tree again (#8). One of the four controls of the holder is shown
    /// at a time; the others are disabled and have no size, and none is ever hidden.
    trees: [TreeView<difference_tree::Native>; 2],
    /// Which of `trees` is shown, while the difference shows as a tree.
    shown_tree: Cell<usize>,
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
    /// The press of a button or check box by its access key, until Alt is up (TASK-30).
    access_keys: Dispatcher<Label>,
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
}

impl MainWindow {
    /// Opens the window on the profile in the file `open`, when there is one, else on the
    /// profile opened last time, else on the first profile; `data_dir` holds the profiles
    /// and the settings.
    pub fn open(
        s: &'static dyn Strings,
        elevated: bool,
        data_dir: PathBuf,
        open: Option<PathBuf>,
    ) -> Rc<Self> {
        let frame = Frame::builder()
            .with_title(&window_title(None))
            .with_size(Size::new(900, 640))
            .build();
        set_frame_icon(&frame);

        frame.set_menu_bar(build_menu_bar(s));
        let menu_bar = frame.get_menu_bar().expect("the menu bar was just set");
        let file_menu = MenuId::ALL
            .iter()
            .position(|&m| m == MenuId::File)
            .expect("File is a menu") as i32;
        win32::shell::mark_with_shield(
            frame.get_handle(),
            file_menu,
            wx_id(CommandId::RestartAsAdmin),
        );
        let status = StatusBar::builder(&frame).build();
        frame.set_existing_status_bar(Some(&status));
        win32::accessibility::make_live_region(status.get_handle());

        let settings_path = data_dir.join(SETTINGS_FILE);
        let settings = Settings::open(&settings_path).map_err(|e| reason(s, &e));
        // The view of the difference list and the size format, as kept (TASK-13 #17, #28),
        // and whether it shows as a tree (TASK-33 #3).
        let view = settings
            .as_ref()
            .ok()
            .map(|settings| View {
                types: settings.difference_types().unwrap_or([true; 4]),
                sort: settings.difference_sort().unwrap_or_default(),
                layout: settings.difference_view().unwrap_or_default(),
            })
            .unwrap_or_default();
        let size_format = settings
            .as_ref()
            .ok()
            .and_then(|settings| settings.size_format().ok())
            .unwrap_or_default();
        let mut list = List::new(s, Box::new(RegionalFormat(Locale::USER)), view, size_format);
        // How the details write attributes, as kept (TASK-14 #7).
        list.set_attribute_format(
            settings
                .as_ref()
                .ok()
                .and_then(|settings| settings.attribute_format().ok())
                .unwrap_or_default(),
        );
        // What the tree's nodes say, as kept (TASK-33 #6).
        list.set_node_text(
            settings
                .as_ref()
                .ok()
                .and_then(|settings| settings.tree_node_text().ok())
                .unwrap_or_default(),
        );
        let difference_list = Rc::new(RefCell::new(list));

        let panel = Panel::builder(&frame).build();
        // Made in tab order. Each label comes right before its control, so screen readers
        // name the control after it.
        let profile_label = StaticText::builder(&panel)
            .with_label(&field_label(s, |s| s.profile()))
            .build();
        let profile = Choice::builder(&panel).build();
        let progress_label = StaticText::builder(&panel)
            .with_label(&field_label(s, |s| s.progress()))
            .build();
        let progress_text = TextCtrl::builder(&panel)
            .with_style(TextCtrlStyle::ReadOnly)
            .build();
        let gauge = Gauge::builder(&panel).with_range(100).build();
        let cancel_button = Button::builder(&panel)
            .with_label(&cancel_label(s, Kind::Snapshot))
            .build();
        // The panel answers for the snapshot list's text while it is empty.
        let snapshot_empty_texts = EmptyTexts::subclass(panel.get_handle());
        let snapshots = SnapshotList::new(Native::new(&panel, &snapshot_empty_texts, s, false), s);
        let comparison_label = StaticText::builder(&panel)
            .with_label(&field_label(s, |s| s.comparison_field()))
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
            win32::lists::full_row_select(list.get_handle());
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
        // The trees after the lists, in the same holder; one of the four is enabled, as the
        // view kept says (TASK-33 #3).
        let trees =
            [(); 2].map(|()| TreeView::new(difference_tree::Native::new(&difference_holder)));
        let as_tree = view.layout == DifferenceView::Tree;
        differences[0].enable(!as_tree);
        differences[1].enable(false);
        trees[0].control().tree().enable(as_tree);
        trees[1].control().tree().enable(false);
        // The holder answers for the difference lists' text while they are empty.
        let difference_empty_texts = EmptyTexts::subclass(difference_holder.get_handle());
        let filter_label = StaticText::builder(&panel)
            .with_label(&field_label(s, |s| s.filter_field()))
            .build();
        let filter_field = TextCtrl::builder(&panel).build();
        let type_boxes = ChangeType::ALL.map(|kind| {
            CheckBox::builder(&panel)
                .with_label(&compose(s, En.type_box(kind), s.type_box(kind)))
                .build()
        });

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
        sizer.add(
            snapshots.control().list(),
            1,
            SizerFlag::Expand | SizerFlag::All,
            5,
        );
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

        let main_frame = win32::frame::subclass(frame.get_handle());

        let window = Rc::new(Self {
            frame,
            main_frame,
            s,
            state: RefCell::new(AppState::new()),
            elevated,
            source: Cell::new(None),
            event_focus: Cell::new(None),
            closing: Cell::new(false),
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
            difference_empty_texts,
            differences,
            shown_difference: Cell::new(0),
            trees,
            shown_tree: Cell::new(0),
            difference_list,
            filter_field,
            type_boxes,
            showing_timer,
            timer,
            jobs: RefCell::new(Jobs::default()),
            shown_kind: Cell::new(None),
            access_keys: Dispatcher::default(),
            said_waiting: Cell::new(None),
            comparison_focus: Cell::new(ptr::null_mut()),
            profiles_dir: data_dir.join("profiles"),
            settings,
            profiles: RefCell::new(Vec::new()),
            open_index: Cell::new(None),
        });
        window.bind_events();
        // Opening the window is an event too (ADR-0013).
        window.event(Source::Navigation, || {
            window.refresh_progress_row();
            window.load_profiles(open.as_deref());
            if let Err(reason) = &window.settings {
                window.fail(&s.settings_not_opened(&file_name(&settings_path), reason));
            }
        });
        window.frame.centre();
        window.frame.show(true);
        window
    }

    /// Binds `handler` to the event `on` binds it to, wrapped in the window event of
    /// `source` (ADR-0013): so no handler can miss the rules of the event.
    fn bind<D: 'static, R>(
        self: &Rc<Self>,
        source: Source,
        on: impl FnOnce(Box<dyn Fn(D)>) -> R,
        handler: impl Fn(&Self, D) + 'static,
    ) {
        let w = Rc::clone(self);
        on(Box::new(move |data| w.event(source, || handler(&w, data))));
    }

    fn bind_events(self: &Rc<Self>) {
        // Not an event of its own (ADR-0008): the hook sees every key first and only routes
        // it; the action the route picks is an event.
        let w = Rc::clone(self);
        self.frame.on_char_hook(move |data| {
            if let WindowEventData::Keyboard(key) = data {
                let shortcut = Shortcut::from_event(&key);
                let focus = w.read_focus().0;
                let labels = targets(w.shown_kind());
                let routed = route(shortcut, focus, &w.state.borrow(), &labels);
                if routed == Route::Pass {
                    key.event.skip(true);
                } else {
                    key.event.skip(false);
                    w.event(Source::Navigation, || w.apply(routed));
                }
            }
        });

        // Not an event: it only gives the snapshot list the first focus. A newly shown
        // frame ends up with the focus on itself once Windows has activated it; the
        // snapshot list should have it. The first idle event comes after that. Later
        // activations are left to wx, which restores whatever had the focus last.
        // A button or check box pressed by its access key acts once Alt is up, in an event
        // of its own (TASK-30).
        let w = Rc::clone(self);
        let focus_pending = Cell::new(true);
        self.frame.on_idle(move |data| {
            if focus_pending.replace(false) {
                w.snapshots.set_focus();
            }
            if let Some(label) = w.access_keys.ready(win32::focus::alt_down()) {
                w.event(Source::Navigation, || w.act_on(label));
            }
            if let WindowEventData::Idle(event) = data {
                event.event.skip(true);
            }
        });

        // A command sets its own source as it runs.
        self.bind(
            Source::Navigation,
            |f| self.frame.on_menu(f),
            |w, event| {
                if let Some(id) = command_of(event.get_id()) {
                    w.run(id);
                }
            },
        );
        self.bind(
            Source::Navigation,
            |f| self.frame.on_menu_opened(f),
            |w, event| {
                w.refresh_menus();
                event.skip(true);
            },
        );
        // Not an event: not skipped, so wxFrame does not put the item's help text in the
        // status bar, where it would overwrite the last message. A sort item highlighted
        // while the difference is a tree says why it does not run, as its key would: it has
        // none (TASK-33 #5, decided 2026-11-07).
        let w = Rc::clone(self);
        self.frame.on_menu_highlighted(move |event| {
            event.skip(false);
            if let Some(id) = event.get_menu_id().and_then(command_of) {
                w.menu_item_highlighted(id);
            }
        });

        // Closing during a snapshot cancels it, so it asks first (TASK-8 #26); a close
        // that cannot be refused, as at a shutdown, stops the snapshot without asking. A
        // comparison is cancelled without a question and not waited for (TASK-11 #18),
        // and so is a comment waiting for the database (TASK-24 #4).
        // A compaction goes on behind the hidden window, and the process ends with it
        // (TASK-9 #30); a close that cannot be refused interrupts it instead.
        self.bind(
            Source::Command(CommandId::Exit),
            |f| self.frame.on_close(f),
            |w, data| {
                if let WindowEventData::General(event) = data {
                    let s = w.s;
                    let can_veto = event.can_veto();
                    let may_close = if can_veto {
                        w.stop_snapshot_if_agreed(
                            &plain(s.command(CommandId::Exit)),
                            s.close_during_snapshot(),
                            |s| s.close(),
                        )
                    } else {
                        w.stop_snapshot();
                        true
                    };
                    if may_close {
                        w.close_jobs(!can_veto);
                        event.skip(true);
                    } else {
                        event.veto();
                    }
                }
            },
        );

        // A tick says nothing of its own: the end of the event polls the jobs.
        self.bind(Source::Navigation, |f| self.timer.on_tick(f), |_, _| {});

        self.bind(
            Source::Navigation,
            |f| self.cancel_button.on_click(f),
            |w, _| {
                if let Some(kind) = w.shown_kind() {
                    w.run(cancel_command(kind));
                }
            },
        );

        self.bind(
            Source::Navigation,
            |f| self.profile.on_selection_changed(f),
            |w, event| {
                let selected = event.get_selection().and_then(|i| usize::try_from(i).ok());
                w.show_profile(selected);
            },
        );

        // Deletion acts on the selected snapshots, and a comparison on the current one, so
        // the commands know how many there are and which is current: the end of the event
        // asks the list.
        let list = self.snapshots.control();
        self.bind(
            Source::Navigation,
            |f| {
                let f = Rc::new(f);
                list.on_change(move || f(()));
            },
            |_, ()| {},
        );

        // Not an event: the list shown fills the panel of the difference lists.
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
            self.bind(
                Source::Navigation,
                |f| list.on_column_click(f),
                |w, event| {
                    if let Some(column) = event.get_column() {
                        w.sort_by_column(column);
                    }
                },
            );
            self.bind(
                Source::Navigation,
                |f| list.on_item_focused(f),
                move |w, event| w.difference_focused(i, event.get_item_index()),
            );
        }
        // The difference trees (TASK-33): a node gets its children as it first expands
        // (#2), the focus picks the node selected, and a right click selects the node under
        // the mouse, for the context menu that follows.
        for (i, view) in self.trees.iter().enumerate() {
            let tree = view.control().tree();
            self.bind(
                Source::Navigation,
                |f| tree.on_item_expanding(f),
                move |w, event| w.tree_expanding(i, event.get_item()),
            );
            self.bind(
                Source::Navigation,
                |f| tree.on_selection_changed(f),
                move |w, event| w.tree_selected(i, event.get_item()),
            );
            self.bind(
                Source::Navigation,
                |f| tree.on_item_right_click(f),
                move |w, event| w.tree_right_clicked(i, event.get_item()),
            );
        }
        self.bind(
            Source::Navigation,
            |f| self.filter_field.on_text_changed(f),
            |w, _| w.filter_typed(),
        );
        for kind in ChangeType::ALL {
            self.bind(
                Source::Navigation,
                |f| self.type_boxes[kind as usize].on_toggled(f),
                move |w, _| w.type_toggled(kind),
            );
        }
        self.bind(
            Source::Navigation,
            |f| self.showing_timer.on_tick(f),
            |w, _| {
                let text = w.difference_list.borrow().showing();
                w.say(&text);
            },
        );

        // A right click: the list under the mouse has the focus by then.
        self.bind(
            Source::Navigation,
            |f| self.panel.on_context_menu(f),
            |w, event| {
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
            },
        );
    }

    // --- The window event (ADR-0013) ----------------------------------------------------

    /// Handles a window event by the rules of ADR-0013. The jobs' timer rests all through
    /// it, so a job that ends under a dialog does not change the window under it; the
    /// messages of the event are gathered and said together at its end (TASK-21), after
    /// the jobs that ended meanwhile; then the state is built afresh, the menus follow
    /// it, and the timer starts again while jobs run. An event that comes during another
    /// — a dialog runs its own loop, a row selected by the window sends its event — only
    /// sets its source for its time and leaves the rest to the outer one.
    fn event<T>(&self, source: Source, handle: impl FnOnce() -> T) -> T {
        let outer = self.source.replace(Some(source));
        if outer.is_some() {
            let answer = handle();
            self.source.set(outer);
            return answer;
        }
        self.timer.stop();
        self.event_focus.set(None);
        let (answer, text) = self.announcer.gather(|| {
            let answer = handle();
            if !self.closing.get() {
                self.poll_jobs();
            }
            answer
        });
        self.event_focus.set(None);
        self.source.set(None);
        // A closing window says nothing more, and its timer stays at rest.
        if self.closing.get() {
            return answer;
        }
        self.refresh_state();
        if let Some(text) = text {
            self.show(&text);
        }
        if self.jobs.borrow().any() {
            self.timer.start(PROGRESS_INTERVAL, false);
        }
        answer
    }

    /// Builds the state afresh from its sources of truth and brings the menus in line
    /// with it, at the end of every event (ADR-0013).
    fn refresh_state(&self) {
        let open = self.open_profile();
        let sources = Sources {
            profile: open.as_ref().map(|p| p.name.clone()),
            snapshot_list: self.snapshots.summary(),
            jobs: self
                .jobs
                .borrow()
                .flags(open.as_ref().map(|p| p.path.as_path())),
            difference: self.difference_list.borrow().state(),
            elevated: self.elevated,
        };
        *self.state.borrow_mut() = AppState::of(sources);
        self.refresh_menus();
    }

    /// Says why an action failed where the source of the event says (ADR-0013): in a
    /// window titled after the explicit command, else in the status bar.
    fn fail(&self, text: &str) {
        match self.source.get() {
            Some(Source::Command(id)) => {
                tell(&self.frame, self.s, &plain(self.s.command(id)), text)
            }
            _ => self.say(text),
        }
    }

    /// Where the keyboard focus is, in the terms of `keyboard::Focus`: read once per
    /// event, the first time it is asked for (ADR-0013).
    fn focus(&self) -> Focus {
        self.focused().0
    }

    /// The focus of the event and the window it is on.
    fn focused(&self) -> (Focus, *mut c_void) {
        if let Some(read) = self.event_focus.get() {
            return read;
        }
        let read = self.read_focus();
        if self.source.get().is_some() {
            self.event_focus.set(Some(read));
        }
        read
    }

    /// The kind of the job the progress row shows, if it shows one.
    fn shown_kind(&self) -> Option<Kind> {
        self.jobs.borrow().shown().map(|shown| shown.kind())
    }

    /// Reads where the keyboard focus is now, with the window it is on: on the progress
    /// row or not, where it was when the comparison started or not.
    fn read_focus(&self) -> (Focus, *mut c_void) {
        let focused = win32::focus::focused_window();
        let focus = if self.snapshots.has_focus() {
            Focus::new(Some(Pane::Snapshots), Control::MainList)
        } else if self.is_difference(focused) {
            Focus::new(Some(Pane::Difference), Control::MainList)
        } else if focused == self.profile.get_handle() {
            Focus::new(Some(Pane::Snapshots), Control::Other)
        } else if focused == self.filter_field.get_handle() {
            Focus::new(Some(Pane::Difference), Control::FilterField)
        } else if focused == self.comparison_field.get_handle() {
            Focus::new(Some(Pane::Difference), Control::TextEntry)
        } else if self.type_boxes.iter().any(|b| b.get_handle() == focused) {
            Focus::new(Some(Pane::Difference), Control::Other)
        } else if !focused.is_null() && win32::focus::is_text_field(focused) {
            Focus::new(None, Control::TextEntry)
        } else {
            Focus::new(None, Control::Other)
        };
        let progress = focused == self.progress_text.get_handle()
            || focused == self.cancel_button.get_handle();
        let focus = Focus {
            progress,
            unmoved: focused == self.comparison_focus.get(),
            ..focus
        };
        (focus, focused)
    }

    /// Moves the keyboard focus to the list of `pane`: of the difference, its list or its
    /// tree, whichever is shown.
    fn focus_list(&self, pane: Pane) {
        match pane {
            Pane::Snapshots => self.snapshots.set_focus(),
            Pane::Difference => match self.layout() {
                DifferenceView::List => self.difference().set_focus(),
                DifferenceView::Tree => self.tree().control().tree().set_focus(),
            },
        }
    }

    /// The difference list shown, or that shown last while the difference is a tree.
    fn difference(&self) -> &ListCtrl {
        &self.differences[self.shown_difference.get()]
    }

    /// The difference tree shown, or that shown last while the difference is a list.
    fn tree(&self) -> &TreeView<difference_tree::Native> {
        &self.trees[self.shown_tree.get()]
    }

    /// Whether the difference shows as a list or as a tree (TASK-33).
    fn layout(&self) -> DifferenceView {
        self.difference_list.borrow().view().layout
    }

    /// The window of the control the difference shows in now: its list or its tree.
    fn difference_window(&self) -> *mut c_void {
        match self.layout() {
            DifferenceView::List => self.difference().get_handle(),
            DifferenceView::Tree => self.tree().control().tree().get_handle(),
        }
    }

    /// Whether `hwnd` is a difference list or tree, shown or not.
    fn is_difference(&self, hwnd: *mut c_void) -> bool {
        self.differences
            .iter()
            .any(|list| list.get_handle() == hwnd)
            || self
                .trees
                .iter()
                .any(|tree| tree.control().tree().get_handle() == hwnd)
    }

    fn apply(&self, routed: Route) {
        let s = self.s;
        match routed {
            Route::Run(id) => self.run(id),
            Route::Refuse(id, reason) => self.refuse(id, reason),
            Route::OnlyIn(key, pane) => self.say(&s.key_only_in(&key.name(s), pane)),
            Route::FocusPane(pane) => self.focus_list(pane),
            Route::FocusLabel(label) => {
                if let Some(label) = self.access_keys.dispatch(label.act(), label) {
                    self.act_on(label);
                }
            }
            Route::ClearFilter => self.clear_filter(),
            Route::ContextMenu(pane) => self.show_context_menu(pane, None),
            Route::SelectAll(Pane::Snapshots) => self.snapshots.select_all(),
            Route::SelectAll(Pane::Difference) => {
                win32::lists::select_all(self.difference().get_handle())
            }
            Route::Pass => {}
        }
    }

    /// Acts on the control `label` names, whose access key was pressed: a field or a box
    /// takes the focus; the Cancel button and a check box are pressed (TASK-30).
    fn act_on(&self, label: Label) {
        match label {
            Label::Profile => self.profile.set_focus(),
            Label::Progress => self.progress_text.set_focus(),
            Label::Cancel => {
                if let Some(kind) = self.shown_kind() {
                    self.run(cancel_command(kind));
                }
            }
            Label::Comparison => self.comparison_field.set_focus(),
            // The filter's controls work once there is a difference; before that their
            // letters say why not, as the commands do.
            Label::Filter => self.run(CommandId::FilterByText),
            Label::Type(kind) => {
                let i = kind as usize;
                self.run(TYPE_COMMANDS[i]);
                if self.type_boxes[i].is_enabled() {
                    self.type_boxes[i].set_focus();
                }
            }
        }
    }

    /// Says a message in the status bar; screen readers read it without the focus
    /// moving. During a window event the message waits for the event's end and is shown
    /// with the others of that event, as one (TASK-21, ADR-0013).
    pub fn say(&self, text: &str) {
        if let Some(text) = self.announcer.say(text) {
            self.show(&text);
        }
    }

    fn show(&self, text: &str) {
        self.status.set_status_text(text, 0);
        win32::accessibility::announce_status(self.status.get_handle());
    }

    /// Runs a command, the source of its event (ADR-0013): a job that ended meanwhile,
    /// under the command's dialog or while the command waited for its write, is reported
    /// together with the command's own outcome at the event's end (TASK-21).
    fn run(&self, id: CommandId) {
        self.event(Source::Command(id), || self.run_command(id));
    }

    /// The menu item of `id` is highlighted: an item of «Сортувати за» while the difference
    /// shows as a tree says why it is unavailable, in the status bar, which screen readers
    /// read after the item (TASK-33 #5).
    fn menu_item_highlighted(&self, id: CommandId) {
        let available = (command(id).available)(&self.state.borrow());
        if let Err(reason @ Reason::TreeSortedByName) = available {
            self.refuse(id, reason);
        }
    }

    /// Says why the command cannot run now, wherever it was asked for (ADR-0008): its name
    /// as it reads outside the menu, then the reason.
    fn refuse(&self, id: CommandId, reason: Reason) {
        let s = self.s;
        self.say(&s.unavailable(&command(id).spoken_name(s), s.reason(reason)));
    }

    /// Runs the command, or says why it cannot run now.
    fn run_command(&self, id: CommandId) {
        let s = self.s;
        let available = (command(id).available)(&self.state.borrow());
        if let Err(reason) = available {
            self.refuse(id, reason);
            return;
        }
        match id {
            CommandId::Exit => self.frame.close(false),
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
            CommandId::CancelComment => self.jobs.borrow().cancel(Kind::Comment),
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
            CommandId::ViewAsList => self.set_layout(DifferenceView::List),
            CommandId::ViewAsTree => self.set_layout(DifferenceView::Tree),
            CommandId::Details => self.show_details(),
            CommandId::ShowInExplorer => self.show_in_explorer(),
            CommandId::KeyboardShortcuts => help::show_shortcuts(&self.frame, s),
            CommandId::About => help::show_about(&self.frame, s),
        }
    }

    // --- Profiles (TASK-7, TASK-9) ----------------------------------------------------

    /// Reads the profiles from the disk into the profile box, by the user's alphabet, and
    /// opens the one in the file `open`, else the one opened last time, else the first.
    /// Says which files of the folder of profiles could not be opened. It is navigation,
    /// also when a command loads them (ADR-0013).
    fn load_profiles(&self, open: Option<&Path>) {
        self.event(Source::Navigation, || self.fill_profile_box(open));
    }

    fn fill_profile_box(&self, open: Option<&Path>) {
        let s = self.s;
        let (mut found, unreadable) = self.list_profiles();
        found.sort_by(|a, b| win32::language::compare_names(&a.name, &b.name));
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
                .map(|u| s.profile_not_opened(&file_name(&u.path), &reason(s, &u.error)))
                .collect();
            self.fail(&messages.join(" "));
        }
    }

    /// The profiles in the folder of profiles and the files there that could not be
    /// opened. A profile whose database this revisor is compacting is known already; one
    /// another revisor is compacting is waited for before the window shows (TASK-9 #30),
    /// by the rule of the core for a busy database (ADR-0012), and reported as in use
    /// once it is shown.
    fn list_profiles(&self) -> (Vec<ProfileFile>, Vec<Unreadable>) {
        let s = self.s;
        let listed = if self.frame.is_shown() {
            profiles::list(&self.profiles_dir)
        } else {
            // Nothing cancels the wait at the start: the window is not there yet.
            let jobs = self.jobs.borrow();
            profiles::list_when_free(&self.profiles_dir, &Cancel::new(), |path| {
                jobs.closed_profile(path).is_some()
            })
        };
        let (mut found, unreadable) = match listed {
            Ok(listed) => listed,
            Err(e) => {
                let folder = self.profiles_dir.to_string_lossy();
                self.fail(&s.profiles_not_listed(&folder, &reason(s, &e)));
                return (Vec::new(), Vec::new());
            }
        };
        let mut others = Vec::new();
        for u in unreadable {
            let jobs = self.jobs.borrow();
            match jobs.closed_profile(&u.path).filter(|_| u.error.is_busy()) {
                Some((profile, _)) => found.push(profile.clone()),
                None => others.push(u),
            }
        }
        (found, others)
    }

    /// Opens the profile at position `index` of the profile box, or none: the title, the
    /// snapshot list with the newest snapshot current, and the memory of the profile for
    /// the next start. It is navigation, also when a command shows the profile (ADR-0013).
    fn show_profile(&self, index: Option<usize>) {
        self.event(Source::Navigation, || self.open_at(index));
    }

    fn open_at(&self, index: Option<usize>) {
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
                            let reason = reason(s, &e);
                            self.fail(&s.profile_not_opened(&file_name(&open.path), &reason));
                            Vec::new()
                        }
                    },
                }
            }
        };
        let any_profiles = !self.profiles.borrow().is_empty();
        self.snapshots.show(rows, any_profiles);
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
    /// why, as a failure of the command.
    fn read_open<T>(
        &self,
        read: impl FnOnce(&Path) -> Result<T, StoreError>,
    ) -> Option<(ProfileFile, T)> {
        let s = self.s;
        let open = self.open_profile()?;
        match read(&open.path) {
            Ok(value) => Some((open, value)),
            Err(e) => {
                self.fail(&s.profile_not_opened(&file_name(&open.path), &reason(s, &e)));
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
        let Some((open, initial)) = self.read_open(profiles::settings_of) else {
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
        let title = &plain(s.command(CommandId::DeleteProfile));
        let Some((open, snapshots)) = self.read_open(profiles::snapshot_count) else {
            return;
        };
        let question = s.delete_profile_question(&open.name, snapshots);
        if !confirm(&self.frame, s, title, &question, |s| s.delete(), true) {
            return;
        }
        self.wait_for_comparison_to_load(&open.path);
        match profiles::delete(&open.path) {
            Ok(()) => {
                self.load_profiles(None);
                self.say(&s.profile_deleted(&open.name));
            }
            Err(e) => self.fail(&s.profile_not_deleted(&open.name, &reason(s, &e))),
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

    /// Whether «Показати в Провіднику» asks before it opens the folder of an entry no
    /// longer on disk (TASK-14 #13); off by default.
    fn warn_missing_entry(&self) -> bool {
        self.settings
            .as_ref()
            .ok()
            .and_then(|settings| settings.warn_missing_entry().ok())
            .unwrap_or(false)
    }

    /// Shows the settings window and keeps what the user confirmed. When the language
    /// setting makes the next start speak another language, offers to restart now
    /// (TASK-16 #3).
    fn settings(&self) {
        let s = self.s;
        let title = &plain(s.command(CommandId::Settings));
        let initial = Values {
            language: self.language(),
            focus_after_snapshot: self.focus_after_snapshot(),
            focus_after_comparison: self.focus_after_comparison(),
            size_format: self.difference_list.borrow().size_format(),
            attribute_format: self.difference_list.borrow().attribute_format(),
            tree_node_text: self.difference_list.borrow().node_text(),
            confirm_snapshot_deletion: self.confirm_deletion(),
            warn_missing_entry: self.warn_missing_entry(),
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
                .and_then(|()| settings.set_attribute_format(values.attribute_format))
                .and_then(|()| settings.set_tree_node_text(values.tree_node_text))
                .and_then(|()| {
                    settings.set_confirm_snapshot_deletion(values.confirm_snapshot_deletion)
                })
                .and_then(|()| settings.set_warn_missing_entry(values.warn_missing_entry))
                .map_err(|e| reason(s, &e)),
            Err(message) => Err(message.clone()),
        };
        match saved {
            Ok(()) => self.say(s.settings_saved()),
            Err(reason) => {
                self.fail(&s.settings_not_saved(&reason));
                return;
            }
        }
        // Sizes in the list and the summary as the setting «Розмір» says (TASK-13 #17),
        // attributes in the details as «Атрибути» says (TASK-14 #7), and the tree's nodes
        // as «Вузли дерева» says (TASK-33 #6).
        let current = {
            let mut list = self.difference_list.borrow_mut();
            list.set_size_format(values.size_format);
            list.set_attribute_format(values.attribute_format);
            list.set_node_text(values.tree_node_text);
            list.current()
        };
        self.refresh_difference(current);
        // «Same as Windows» is read now, not at the start: the language of Windows may
        // have changed meanwhile, and the next start would take the new one.
        let next = crate::language_of(values.language, win32::language::ui_language);
        if next == s.lang() {
            return;
        }
        let restart_now = choose(
            &self.frame,
            s,
            title,
            s.language_changes_after_restart(),
            |s| s.restart_now(),
            Some(|s| s.later()),
            false,
        );
        if !restart_now {
            return;
        }
        // During a snapshot, the same question as closing revisor asks (TASK-16 #4).
        let agreed = self.stop_snapshot_if_agreed(
            &plain(s.command(CommandId::Exit)),
            s.close_during_snapshot(),
            |s| s.close(),
        );
        if agreed {
            self.restart_with_same_rights(self.open_profile().as_ref());
        }
    }

    // --- The snapshot list (TASK-9) ---------------------------------------------------

    /// Asks for the current snapshot's comment and keeps it (TASK-9 #7). A database
    /// another connection holds — a snapshot being saved (#29), another revisor
    /// compacting it — is waited for in the background, which the progress row shows and
    /// «Скасувати коментар» cancels (TASK-24).
    fn comment(&self) {
        let s = self.s;
        let title = &plain(s.command(CommandId::Comment));
        let Some(open) = self.open_profile() else {
            return;
        };
        let Some(info) = self
            .snapshots
            .current()
            .and_then(|row| self.snapshots.row(row))
        else {
            return;
        };
        let name = win32::format::format_date_time(Locale::USER, info.taken_at);
        let asked = ask_text(
            &self.frame,
            s,
            title,
            &|s| s.comment_of(&name),
            info.comment.as_deref().unwrap_or_default(),
        );
        let Some(text) = asked else {
            return;
        };
        let comment = Comment {
            id: info.id,
            comment: (!text.is_empty()).then_some(text),
        };
        let written = self
            .jobs
            .borrow_mut()
            .comment(open.clone(), comment.clone());
        match written {
            // It waits for the database; the wait is said once the command is over.
            None => self.refresh_progress_row(),
            Some(Ok(())) => self.comment_saved(&open, comment),
            Some(Err(e)) => self.comment_failed(&reason(s, &e)),
        }
    }

    /// Shows the comment saved in the snapshot list, if its profile is open, and says so.
    fn comment_saved(&self, profile: &ProfileFile, comment: Comment) {
        let s = self.s;
        let removed = comment.comment.is_none();
        if self.is_open(profile) && self.snapshots.set_comment(comment.id, comment.comment) {
            self.refresh_pair();
            self.describe_difference();
        }
        self.say(if removed {
            s.comment_removed()
        } else {
            s.comment_saved()
        });
    }

    fn comment_failed(&self, reason: &str) {
        self.fail(&self.s.comment_not_saved(reason));
    }

    /// Ends a comment that waited for the database (TASK-24): saved, cancelled — the
    /// earlier comment stays — or failed.
    fn comment_ended(&self, ended: Ended<Outcome<(), StoreError>>, comment: Comment) {
        let s = self.s;
        self.refresh_progress_row();
        match ended.outcome {
            Ok(Outcome::Done(())) => self.comment_saved(&ended.profile, comment),
            Ok(Outcome::Cancelled) => self.say(s.comment_cancelled()),
            Ok(Outcome::Failed(e)) => self.comment_failed(&reason(s, &e)),
            Err(Lost) => self.comment_failed(s.comment_stopped()),
        }
    }

    /// Says that a comment waits for the database, once, at the end of the event that
    /// found it waiting: a comment that waited only for the save that ended in the same
    /// event says no more than its result (TASK-24 #1, #2).
    fn say_comment_waiting(&self) {
        let started = self.jobs.borrow_mut().comment_started_waiting();
        if started {
            self.say(self.s.waiting_for_database());
        }
    }

    /// Deletes the selected snapshots, after the confirmation the settings ask for, and
    /// compacts the profile's database in the background (TASK-9 #24–#27).
    fn delete_snapshots(&self) {
        let s = self.s;
        let title = &plain(s.command(CommandId::DeleteSnapshots));
        let Some(open) = self.open_profile() else {
            return;
        };
        // The jobs' timer rests all through the event, so a snapshot that ends while
        // this asks does not change the list under the question (ADR-0013). The list keeps the focus rectangle off its rows while this asks and deletes,
        // and puts it back on a refusal or a failure.
        let mut store = None;
        let deleted = self.snapshots.delete(|chosen| {
            if self.confirm_deletion() {
                let question = match chosen {
                    [one] => s.delete_snapshot_question(
                        &win32::format::format_date_time(Locale::USER, one.taken_at),
                        one.comment.as_deref(),
                    ),
                    many => s.delete_snapshots_question(many.len()),
                };
                if !confirm(&self.frame, s, title, &question, |s| s.delete(), true) {
                    return false;
                }
            }
            let ids: Vec<i64> = chosen.iter().map(|row| row.id).collect();
            self.wait_for_comparison_to_load(&open.path);
            let deleted = Store::open(&open.path).and_then(|mut store| {
                store.delete_many(&ids)?;
                Ok(store)
            });
            match deleted {
                Ok(deleted) => {
                    store = Some(deleted);
                    true
                }
                Err(e) => {
                    self.fail(&s.snapshots_not_deleted(&reason(s, &e)));
                    false
                }
            }
        });
        let (Some(chosen), Some(store)) = (deleted, store) else {
            return;
        };
        let rows = self.snapshots.rows();
        self.refresh_pair();
        self.describe_difference();
        self.jobs
            .borrow_mut()
            .start_compaction(open.clone(), rows, store);
        self.refresh_progress_row();
        self.say(&s.snapshots_deleted(chosen.len(), &open.name));
    }

    /// Asks the compaction to stop; its end comes through the timer, like any other.
    fn cancel_compaction(&self) {
        self.jobs.borrow().cancel(Kind::Compaction);
    }

    /// Reports the end of the compaction.
    fn compaction_ended(&self, ended: Ended<Outcome<Compacted, StoreError>>) {
        let s = self.s;
        self.refresh_progress_row();
        let name = &ended.profile.name;
        match ended.outcome {
            Ok(Outcome::Done(compacted)) => {
                let freed =
                    win32::format::format_size(compacted.before.saturating_sub(compacted.after));
                self.say(&s.compaction_done(name, &freed));
            }
            Ok(Outcome::Cancelled) => self.say(&s.compaction_cancelled(name)),
            Ok(Outcome::Failed(e)) => self.fail(&s.compaction_failed(name, &reason(s, &e))),
            Err(Lost) => self.fail(&s.compaction_failed(name, s.compaction_stopped())),
        }
    }

    // --- The comparison (TASK-11) -------------------------------------------------------

    /// Compares the current snapshot with the one before it in the list (#1).
    fn compare_with_previous(&self) {
        let Some(open) = self.open_profile() else {
            return;
        };
        let Some(row) = self.snapshots.current().filter(|&row| row > 0) else {
            return;
        };
        let rows = self.snapshots.rows();
        let (current, previous) = (rows[row].id, rows[row - 1].id);
        self.start_comparison(open, current, previous, self.focused().1);
    }

    /// Asks which other snapshot of the profile to compare the current one with, and
    /// compares (#2, #3). The comparison then starts from the snapshot list, wherever
    /// the command came from (#12).
    fn compare_with(&self) {
        let s = self.s;
        let Some(open) = self.open_profile() else {
            return;
        };
        let Some(row) = self.snapshots.current() else {
            return;
        };
        let rows = self.snapshots.rows();
        let chosen = compare_window::show(&self.frame, s, &rows, row);
        let Some(other) = chosen else {
            return;
        };
        if !self.snapshots.has_focus() {
            self.snapshots.set_focus();
        }
        let list = self.snapshots.control().list().get_handle();
        self.start_comparison(open, rows[row].id, other.id, list);
    }

    /// Starts comparing the snapshots `a` and `b` of `profile` in the background, in
    /// place of a comparison that still runs (#15), remembering `from`, the window with the
    /// keyboard focus, for the difference's arrival (#12). The progress row shows the
    /// comparison once it has run for a second (#7). The report of a «Знімок + порівняння»
    /// whose comparison this one takes the place of is said now (TASK-12).
    fn start_comparison(&self, profile: ProfileFile, a: i64, b: i64, from: *mut c_void) {
        self.comparison_focus.set(from);
        let superseded = self.jobs.borrow_mut().start_comparison(profile, a, b);
        self.refresh_progress_row();
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
        let focus = self.focus();
        self.refresh_progress_row();
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
            Ok(Outcome::Failed(e)) => self.fail(&s.comparison_failed(&reason(s, &e))),
            Err(Lost) => self.fail(&s.comparison_failed(s.comparison_stopped())),
        }
    }

    /// Puts `difference`, of `profile`, in the difference list that is not shown, which
    /// then takes the place of the shown one, with the keyboard focus moving to its first
    /// row when `focus` says so (#12–#14, #21). The view stays, the text filter goes
    /// (TASK-13 #28), and the new list is named by the summary of its difference before
    /// it shows, so a screen reader names it so when the focus comes to it (TASK-13 #3).
    /// As a tree, the difference goes into the tree not shown the same way, with the way
    /// to its first change expanded and the focus there (TASK-33 #8).
    fn show_difference(&self, difference: Difference, profile: ProfileFile, focus: bool) {
        let current = self
            .difference_list
            .borrow_mut()
            .replace(difference, profile);
        self.filter_field.change_value("");
        self.showing_timer.stop();
        self.refresh_pair();
        if self.layout() == DifferenceView::Tree {
            self.show_difference_tree(focus);
            return;
        }
        // The trees' nodes were those of the difference shown before.
        for tree in &self.trees {
            if !tree.is_empty() {
                tree.clear();
            }
        }
        let count = self.difference_list.borrow().len();
        let old = self.difference();
        let next = 1 - self.shown_difference.get();
        let new = &self.differences[next];
        new.set_item_count(count as i64);
        if let Some(row) = current {
            win32::lists::select_only(new.get_handle(), row);
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

    /// The difference just put in the model goes into the tree not shown, which takes the
    /// place of the shown one (TASK-33 #8), as with the lists.
    fn show_difference_tree(&self, focus: bool) {
        let old = self.tree();
        let next = 1 - self.shown_tree.get();
        let new = &self.trees[next];
        {
            let model = self.difference_list.borrow();
            new.fill(&*model, model.tree_current());
        }
        new.control().tree().enable(true);
        self.shown_tree.set(next);
        self.place_difference_lists();
        self.describe_difference();
        if focus {
            new.control().tree().set_focus();
        }
        old.control().tree().enable(false);
        old.clear();
    }

    /// The control the difference shows in fills the panel of the difference lists; the
    /// other three have no size (TASK-33 #3).
    fn place_difference_lists(&self) {
        let size = self.difference_holder.get_client_size();
        let layout = self.layout();
        let (list, tree) = (self.shown_difference.get(), self.shown_tree.get());
        let place = |window: &dyn WxWidget, shown: bool| {
            if shown {
                window.set_size_with_pos(0, 0, size.width, size.height);
            } else {
                window.set_size_with_pos(0, 0, 0, 0);
            }
        };
        for (i, control) in self.differences.iter().enumerate() {
            place(control, layout == DifferenceView::List && i == list);
        }
        for (i, view) in self.trees.iter().enumerate() {
            place(
                view.control().tree(),
                layout == DifferenceView::Tree && i == tree,
            );
        }
    }

    /// «Списком» or «Деревом», or Ctrl+T (TASK-33 #3, #7): the difference shows the other
    /// way in the same place, kept for the next starts, with the focus on the same entry.
    /// The focus that was on the difference goes to the new control, which screen readers
    /// name with the entry; with the focus elsewhere, the status bar says how the
    /// difference shows now.
    fn set_layout(&self, layout: DifferenceView) {
        if self.layout() == layout {
            return;
        }
        let here = self.focus().in_list(Pane::Difference);
        self.difference_list.borrow_mut().set_layout(layout);
        if let Ok(settings) = &self.settings {
            let _ = settings.set_difference_view(layout);
        }
        match layout {
            DifferenceView::Tree => {
                let view = self.tree();
                let model = self.difference_list.borrow();
                // A tree shown before keeps what was expanded in it.
                if view.is_empty() {
                    view.fill(&*model, model.tree_current());
                } else {
                    view.refresh(&*model, model.tree_current());
                }
                view.control().tree().enable(true);
            }
            DifferenceView::List => {
                let list = self.difference();
                let (count, current) = {
                    let model = self.difference_list.borrow();
                    (model.len(), model.current())
                };
                list.set_item_count(count as i64);
                if let Some(row) = current {
                    win32::lists::select_only(list.get_handle(), row);
                }
                if count > 0 {
                    list.refresh_items(0, count as i64 - 1);
                }
                list.enable(true);
            }
        }
        self.place_difference_lists();
        self.describe_difference();
        if here {
            self.focus_list(Pane::Difference);
        } else {
            self.say(self.s.layout_shown(layout));
        }
        // Disabled once the focus has left it, so that the focus does not go elsewhere.
        match layout {
            DifferenceView::Tree => self.difference().enable(false),
            DifferenceView::List => self.tree().control().tree().enable(false),
        }
    }

    /// The node of `item` of the tree `tree` is about to expand: its children come now,
    /// the first time (TASK-33 #2).
    fn tree_expanding(&self, tree: usize, item: Option<TreeItemId>) {
        let view = &self.trees[tree];
        if view.busy() {
            return;
        }
        let Some(node) = item.and_then(|item| view.control().node_of(&item)) else {
            return;
        };
        let model = self.difference_list.borrow();
        view.expanding(node, &*model);
    }

    /// The focus of the tree `tree` has moved to `item`: the model keeps its node.
    fn tree_selected(&self, tree: usize, item: Option<TreeItemId>) {
        let view = &self.trees[tree];
        if view.busy() || tree != self.shown_tree.get() {
            return;
        }
        if let Some(node) = item.and_then(|item| view.control().node_of(&item)) {
            self.difference_list.borrow_mut().tree_focus(node);
        }
    }

    /// A right click on `item` of the tree `tree`: the node gets the focus, so the
    /// context menu that follows acts on it.
    fn tree_right_clicked(&self, tree: usize, item: Option<TreeItemId>) {
        let control = self.trees[tree].control().tree();
        if let Some(item) = item {
            control.set_focus();
            control.select_item(&item);
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
        if list != self.shown_difference.get() || self.layout() != DifferenceView::List {
            return;
        }
        self.difference_list.borrow_mut().focus(index);
        let hwnd = self.differences[list].get_handle();
        if !win32::lists::is_selected(hwnd, index) {
            win32::lists::select_only(hwnd, index);
        }
    }

    /// Brings the difference list in line with the model once the view or the difference
    /// changed: its rows, the row with the focus, at `current` (#21, #27), and what is
    /// said about the difference. The tree, when it is shown, changes its items in place,
    /// its focus where the model says (TASK-33 #4).
    fn refresh_difference(&self, current: Option<usize>) {
        if self.layout() == DifferenceView::Tree {
            let model = self.difference_list.borrow();
            self.tree().refresh(&*model, model.tree_current());
        } else {
            let list = self.difference();
            let count = self.difference_list.borrow().len();
            list.set_item_count(count as i64);
            if let Some(row) = current {
                win32::lists::select_only(list.get_handle(), row);
            }
            if count > 0 {
                list.refresh_items(0, count as i64 - 1);
            }
        }
        self.describe_difference();
    }

    /// What is said about the difference shown: the name of the list and the text of its
    /// empty area (#3, #11, #25), the field «Порівняння» (#1, #11), the check boxes and
    /// the menu's marks, which follow the view at the event's end (#22), the sort arrow of the headers (#21),
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
        // The tree is named as the list, and has no empty text to show (TASK-33).
        let shown = self.difference_window();
        win32::accessibility::set_accessible_name(shown, &name);
        let list = self.difference().get_handle();
        self.difference_empty_texts.set(list, &name);
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
        win32::lists::set_sort_arrow(list, arrow);
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
            return Some(self.snapshots.rows());
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
            self.set_types(types);
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
        self.set_sort(DifferenceSort { column, descending });
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

    // --- The details and File Explorer (TASK-14) ----------------------------------------

    /// «Подробиці»: the details of the change with the focus in the difference list, in a
    /// window of their own (#1, #2), whose title names the entry. The jobs' timer rests
    /// while it is open, as all through an event, so the difference does not change under
    /// it, and the focus goes back where it was — on the same change, when it was in the
    /// list. In the tree, a folder that is no change itself has its path and what changed
    /// inside it (TASK-33 #9).
    fn show_details(&self) {
        let s = self.s;
        let details = self.difference_list.borrow().current_details();
        let Some(details) = details else {
            return;
        };
        let title = format!("{}: {}", plain(s.command(CommandId::Details)), details.name);
        text_window::show(&self.frame, s, &title, &details.text);
    }

    /// «Показати в Провіднику»: File Explorer opens the folder of the change with the focus
    /// and selects the entry itself (#11); an entry no longer on disk leaves its folder,
    /// or the nearest one above, to open without a selection, after a question when the
    /// setting asks for one (#12, #13). When no folder of the path is on disk, nothing
    /// opens, and the status bar says why, as for a command that cannot run now; when the
    /// shell fails, a window says so, as for the failure of any command (ADR-0013). A
    /// folder of the tree that is no change itself opens, its contents shown (TASK-33 #9).
    fn show_in_explorer(&self) {
        let s = self.s;
        let command = &plain(s.command(CommandId::ShowInExplorer));
        let location = self.difference_list.borrow().current_location();
        let (path, reveal) = match location {
            Some(Location::Entry(path)) => {
                let reveal = explorer::reveal(&path, explorer::on_disk);
                (path, reveal)
            }
            Some(Location::Folder(path)) => {
                let reveal = explorer::open_folder(&path, explorer::on_disk);
                (path, reveal)
            }
            None => return,
        };
        let shown = match reveal {
            Reveal::Select(entry) => win32::shell::select_in_explorer(&entry),
            Reveal::Inside(folder) => win32::shell::open_in_explorer(&folder),
            Reveal::Open(folder) => {
                if self.warn_missing_entry() {
                    let question = s.entry_gone_question();
                    if !confirm(&self.frame, s, command, question, |s| s.open(), false) {
                        return;
                    }
                }
                win32::shell::open_in_explorer(&folder)
            }
            Reveal::Nowhere => {
                let reason = s.no_folder_on_disk(&path.to_string_lossy());
                self.say(&s.unavailable(command, &reason));
                return;
            }
        };
        if let Err(e) = shown {
            self.fail(&s.explorer_failed(&reason(s, &e)));
        }
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

    /// On closing: the jobs' timer stops for good — the one stop outside the window event
    /// (ADR-0013) — a comparison and a comment waiting for the database are cancelled,
    /// and a compaction that is running goes on behind the hidden window, the process
    /// ending only when it has (TASK-9 #30); with `interrupt`, as at a shutdown, the
    /// compaction is interrupted instead, which leaves the database as it was.
    fn close_jobs(&self, interrupt: bool) {
        self.closing.set(true);
        self.timer.stop();
        self.jobs.borrow().cancel(Kind::Comparison);
        self.jobs.borrow().cancel(Kind::Comment);
        if !self.jobs.borrow().runs(Kind::Compaction) {
            return;
        }
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
        let with = self.snapshots.newest().map(|row| row.id);
        self.comparison_focus.set(self.focused().1);
        let superseded = self
            .jobs
            .borrow_mut()
            .start_snapshot_and_compare(open, with);
        self.refresh_progress_row();
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
        focus: Focus,
    ) {
        let s = self.s;
        let of_open_profile = self.is_open(&profile);
        let Some(with) = with else {
            if of_open_profile {
                self.snapshots.arrive(
                    snapshot_row(&report),
                    arrival_after_comparison(focus, false),
                );
            }
            self.refresh_progress_row();
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
            self.snapshots
                .arrive(snapshot_row(&report), Arrival::Append);
        }
        self.jobs
            .borrow_mut()
            .start_comparison_after_snapshot(profile, with, report);
        self.refresh_progress_row();
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
        let focus = self.focus();
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
                let failed = s.comparison_failed(&reason(s, &e));
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
            arrival_after_comparison(focus, to_difference),
        );
        self.refresh_progress_row();
        self.say(&text);
        self.offer_restart(&profile, &report, CommandId::SnapshotAndCompare);
    }

    /// Makes the snapshot `id` of `profile` current in the snapshot list as `arrival`
    /// says — `Current`, or `Focus` with the keyboard focus moving to it; `Append` leaves
    /// the list as it is. Nothing changes while another profile is open, or when the
    /// snapshot has been deleted meanwhile.
    fn place_snapshot(&self, profile: &ProfileFile, id: i64, arrival: Arrival) {
        if self.is_open(profile) {
            self.snapshots.place(id, arrival);
        }
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
        self.refresh_progress_row();
        self.say(s.snapshot_started());
    }

    /// Asks the snapshot to stop; its end comes through the timer, like any other.
    fn cancel_snapshot(&self) {
        self.jobs.borrow().cancel(Kind::Snapshot);
    }

    /// At the end of every window event (ADR-0013): reports the jobs that have ended,
    /// together when several have, and shows the progress of the job the row shows. The
    /// ends come first: a job over by now says nothing more of its progress. The end of a
    /// snapshot and of a comment that waited is the end of its command, which the user
    /// waits for; that of a comparison and of a compaction is background.
    fn poll_jobs(&self) {
        let ended = self.jobs.borrow_mut().poll();
        for event in ended {
            match event {
                Event::Snapshot(ended, then) => {
                    let id = match then {
                        Some(_) => CommandId::SnapshotAndCompare,
                        None => CommandId::TakeSnapshot,
                    };
                    self.event(Source::Command(id), || {
                        self.snapshot_ended(ended, then, false);
                    });
                }
                Event::Comparison(ended, after) => {
                    self.event(Source::Background, || self.comparison_ended(ended, after));
                }
                Event::Compaction(ended) => {
                    self.event(Source::Background, || self.compaction_ended(ended));
                }
                Event::Comment(ended, comment) => {
                    self.event(Source::Command(CommandId::Comment), || {
                        self.comment_ended(ended, comment);
                    });
                }
            }
        }
        self.say_comment_waiting();
        self.show_job_progress();
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
                let entries_text = win32::format::format_count(Locale::USER, entries);
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
        let focus = self.focus();
        let outcome = match (ended.outcome, then) {
            (Ok(Outcome::Done(report)), Some(then)) if !stopped => {
                self.snapshot_to_compare(ended.profile, report, then.with, focus);
                return;
            }
            (outcome, _) => outcome,
        };
        self.refresh_progress_row();
        match outcome {
            Ok(Outcome::Done(report)) => {
                self.report_snapshot(&ended.profile, &report, !stopped, focus);
            }
            Ok(Outcome::Failed(SnapshotError::Store(e))) if !stopped => {
                self.snapshot_failed(&reason(s, &e));
            }
            Err(Lost) if !stopped => self.snapshot_failed(s.snapshot_stopped()),
            // A snapshot stopped on closing or restarting is lost however it ended.
            _ => self.say(s.snapshot_cancelled()),
        }
    }

    /// Shows the progress row for the job it shows, with the Cancel button named after
    /// that job's command, or hides it when there is none; the focus leaves a hidden row
    /// for the snapshot list. The timer that polls the jobs is the window event's
    /// (ADR-0013): it runs while any job does, a comparison before its row shows
    /// (TASK-11 #7).
    fn refresh_progress_row(&self) {
        let s = self.s;
        // The button is «Скасувати (N)» for a snapshot, a comparison and a comment, and
        // the command's own name for a compaction.
        let shown = match self.jobs.borrow().shown() {
            Some(Shown::Snapshot(_)) => Some((
                Kind::Snapshot,
                s.snapshot_progress(0, &win32::format::format_count(Locale::USER, 0), ""),
            )),
            Some(Shown::Comparison { progress, elapsed }) => Some((
                Kind::Comparison,
                self.comparison_progress_text(progress, elapsed),
            )),
            Some(Shown::Comment) => Some((Kind::Comment, s.waiting_for_database().to_string())),
            Some(Shown::Compaction(profile)) => {
                Some((Kind::Compaction, s.compacting(&profile.name)))
            }
            None => None,
        }
        .map(|(kind, text)| (kind, text, cancel_label(s, kind)));
        let kind = shown.as_ref().map(|(kind, ..)| *kind);
        if kind != Some(Kind::Comparison) {
            self.said_waiting.set(None);
            self.gauge.show(false);
        }
        match &shown {
            None => {
                let focused = win32::focus::focused_window();
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
        focus: Focus,
    ) {
        match arrival(self.focus_after_snapshot(), focus, self.is_open(profile)) {
            Arrival::Nothing => {}
            Arrival::OpenProfileAndFocus => {
                if self.open_profile_at(&profile.path) {
                    self.snapshots
                        .arrive(snapshot_row(report), Arrival::OpenProfileAndFocus);
                }
            }
            how => self.snapshots.arrive(snapshot_row(report), how),
        }
        self.say(&self.snapshot_report(report));
        if offer_restart {
            self.offer_restart(profile, report, CommandId::TakeSnapshot);
        }
    }

    /// What a saved snapshot says (TASK-8): how many entries it holds, how many of them
    /// are inaccessible, and which folders it did not find.
    fn snapshot_report(&self, report: &Report) -> String {
        let entries = win32::format::format_count(Locale::USER, report.entries);
        self.snapshot_report_after(self.s.snapshot_saved(report.entries, &entries), report)
    }

    /// `first`, then what a saved snapshot says after how many entries it holds: how many
    /// of them are inaccessible, and which folders it did not find.
    fn snapshot_report_after(&self, first: String, report: &Report) -> String {
        let s = self.s;
        let mut text = first;
        if report.inaccessible > 0 {
            text.push(' ');
            text.push_str(&s.entries_inaccessible(&win32::format::format_count(
                Locale::USER,
                report.inaccessible,
            )));
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
    /// #18); the question is titled after `command`, which took the snapshot. The restart
    /// the user agrees to is the command «Перезапустити як адміністратор» (ADR-0013).
    fn offer_restart(&self, profile: &ProfileFile, report: &Report, command: CommandId) {
        let s = self.s;
        if report.denied > 0 && !self.elevated {
            let question =
                s.restart_question(&win32::format::format_count(Locale::USER, report.denied));
            let title = plain(s.command(command));
            if confirm(&self.frame, s, &title, &question, |s| s.restart(), true) {
                // On the snapshotted profile, whichever is open by now.
                self.event(Source::Command(CommandId::RestartAsAdmin), || {
                    self.restart_now(Some(profile));
                });
            }
        }
    }

    /// Why the snapshot did not scan a folder, in the interface's words: the reason of an
    /// unavailable folder is the comparison's (TASK-25), and a Windows error says the folder
    /// could not be read before it.
    fn folder_state_reason(&self, state: &FolderState) -> String {
        let s = self.s;
        match state {
            FolderState::Scanned { .. } => String::new(),
            FolderState::Missing => s.folder_missing().into(),
            FolderState::Unavailable(reason @ Unavailable::Error(_)) => {
                s.folder_unreachable(&difference::unavailable_reason(s, reason))
            }
            FolderState::Unavailable(reason) => difference::unavailable_reason(s, reason),
        }
    }

    /// Says why the snapshot was not saved, in the status bar and, as a failure of the
    /// command that took it, in a window titled after it (TASK-8; TASK-12 #10).
    fn snapshot_failed(&self, reason: &str) {
        let text = self.s.snapshot_failed(reason);
        self.say(&text);
        self.fail(&text);
    }

    /// Stops the snapshot being taken, if there is one, once the user agrees to
    /// `question`; returns whether there is nothing in the way any more. The timer rests
    /// while the question is up, as all through an event, so the snapshot's end does not
    /// answer it.
    fn stop_snapshot_if_agreed(&self, title: &str, question: &str, action: Name) -> bool {
        if !self.jobs.borrow().runs(Kind::Snapshot) {
            return true;
        }
        if !confirm(&self.frame, self.s, title, question, action, true) {
            return false;
        }
        self.stop_snapshot();
        true
    }

    /// Stops the snapshot being taken, if there is one, without asking, as revisor is
    /// about to close or restart. A snapshot that ended meanwhile, while a question was
    /// up, is reported as it ended. A comment waiting for the database is cancelled
    /// first, so that the database the stopped snapshot leaves free does not take it
    /// (TASK-24 #4).
    fn stop_snapshot(&self) {
        self.jobs.borrow().cancel(Kind::Comment);
        let stopped = self.jobs.borrow_mut().stop_snapshot();
        if let Some(ended) = stopped {
            self.snapshot_ended(ended, None, true);
        }
    }

    /// Restarts revisor as administrator on the open profile; during a snapshot, once the
    /// user agrees to lose it (TASK-8 #30).
    fn restart_as_admin(&self) {
        let s = self.s;
        let agreed = self.stop_snapshot_if_agreed(
            &plain(s.command(CommandId::RestartAsAdmin)),
            s.restart_during_snapshot(),
            |s| s.restart(),
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
        let started =
            std::env::current_exe().and_then(|exe| win32::shell::run_as_admin(&exe, &args));
        match started {
            Ok(()) => self.frame.close(true),
            Err(e) => self.fail(&s.restart_failed(&reason(s, &e))),
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
            Err(e) => self.fail(&s.restart_failed(&reason(s, &e))),
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
        let (x, y) = at.unwrap_or_else(|| match (pane, self.layout()) {
            (Pane::Snapshots, _) => self.snapshots.control().context_menu_point(),
            (Pane::Difference, DifferenceView::List) => {
                win32::lists::context_menu_point(self.difference().get_handle())
            }
            (Pane::Difference, DifferenceView::Tree) => self.tree().control().context_menu_point(),
        });
        self.main_frame.popup(menu_items.default.is_some(), || {
            self.frame.popup_menu(&mut menu, Some(Point::new(x, y)))
        });
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
    let reason = match e {
        // Another copy of revisor may have taken the name meanwhile.
        ProfileError::NameTaken => return s.name_taken(&settings.name),
        ProfileError::Store(e) => reason(s, e),
        ProfileError::Io(e) => reason(s, e),
    };
    s.profile_not_saved(&reason)
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
    fn the_cancel_button_keeps_its_label_from_a_snapshot_to_its_comparison() {
        // «Знімок + порівняння» goes from its snapshot to its comparison under one Cancel
        // button, whose label does not change under the focus (TASK-12 #9).
        for s in [&En as &dyn Strings, &crate::i18n::Uk] {
            assert_eq!(
                cancel_label(s, Kind::Snapshot),
                cancel_label(s, Kind::Comparison)
            );
        }
        assert_eq!(cancel_label(&En, Kind::Comment), "Ca&ncel");
        assert_eq!(cancel_label(&En, Kind::Compaction), "Cancel c&ompaction");
        let uk = &crate::i18n::Uk;
        assert_eq!(cancel_label(uk, Kind::Comment), "Скасувати (&N)");
    }
}
