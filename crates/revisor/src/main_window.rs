//! The main window: the menu bar and context menus built from the command table, the
//! status bar that speaks without moving the focus, the key handler, the two parts of
//! the window, and the snapshot being taken, with its progress and its outcome (TASK-8).
//! The profile box lists the profiles (TASK-7); both lists are placeholders until TASK-9
//! and TASK-11 fill them.

use std::cell::{Cell, RefCell};
use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread::{self, JoinHandle};

use revisor_core::profiles::{self, ProfileError, ProfileFile};
use revisor_core::scan::Progress;
use revisor_core::snapshot::{self, Report, SnapshotError};
use revisor_core::store::{FolderState, ProfileSettings, StoreError, Unavailable};
use wxdragon::prelude::*;

use crate::commands::{COMMANDS, CommandId, CommandKind, MenuId, Pane, command};
use crate::failure;
use crate::help;
use crate::i18n::Strings;
use crate::keyboard::{Control, Focus, Route, route};
use crate::keys::Shortcut;
use crate::menus::{
    button_label, context_label, context_menu, field_label, menu_label, submenu_label,
    top_menu_label, window_title,
};
use crate::message_box::{confirm, tell};
use crate::profile_window::{self, Purpose};
use crate::state::{AppState, ChangeType, SortColumn};
use crate::win32;

/// The wx id of the first command; the others follow in table order.
const FIRST_COMMAND_ID: i32 = 1000;

/// How often the progress of a snapshot is shown, in milliseconds.
const PROGRESS_INTERVAL: i32 = 100;

/// The access keys of the labels and buttons of the main window. They do not take the
/// letters of the menus, which would no longer open while the focus is in the panel.
const PROFILE_ACCESS: u8 = b'R';
const PROGRESS_ACCESS: u8 = b'P';
/// «Скасувати (N)» next to the progress: the letter of the menu command, too.
const CANCEL_SNAPSHOT_ACCESS: u8 = b'N';

/// The access keys of the action buttons of questions: «Видалити», «Перезапустити» and
/// «Закрити».
const DELETE_ACCESS: u8 = b'D';
const RESTART_ACCESS: u8 = b'R';
const CLOSE_ACCESS: u8 = b'C';

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

/// A snapshot being taken on its own thread. The window polls its progress on a timer and
/// takes its result from the channel.
struct SnapshotJob {
    /// The profile being snapshotted, which cannot change meanwhile.
    profile: ProfileFile,
    progress: Arc<Progress>,
    cancel: Arc<AtomicBool>,
    done: Receiver<Result<Report, SnapshotError>>,
    thread: Option<JoinHandle<()>>,
}

impl SnapshotJob {
    fn start(profile: ProfileFile) -> Self {
        let progress = Arc::new(Progress::default());
        let cancel = Arc::new(AtomicBool::new(false));
        let (sender, done) = mpsc::channel();
        let thread = {
            let path = profile.path.clone();
            let (progress, cancel) = (Arc::clone(&progress), Arc::clone(&cancel));
            thread::spawn(move || {
                let _ = sender.send(snapshot::take(&path, None, &progress, &cancel));
            })
        };
        Self {
            profile,
            progress,
            cancel,
            done,
            thread: Some(thread),
        }
    }

    /// Stops the snapshot and waits for its thread, which is quick: the scan notices
    /// between folders and files, the save every few thousand entries.
    fn stop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.wait();
    }

    fn wait(&mut self) {
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub struct MainWindow {
    frame: Frame,
    s: &'static dyn Strings,
    state: RefCell<AppState>,
    menu_bar: MenuBar,
    status: StatusBar,
    panel: Panel,
    profile: Choice,
    /// The progress of the snapshot being taken, shown only while one is.
    progress_label: StaticText,
    progress_text: TextCtrl,
    cancel_snapshot: Button,
    snapshots: ListCtrl,
    difference: ListCtrl,
    /// Polls the snapshot being taken.
    timer: Timer<Frame>,
    job: RefCell<Option<SnapshotJob>>,
    /// The folder of profiles, `data\profiles` next to the exe.
    profiles_dir: PathBuf,
    /// The profiles in the order the profile box shows them.
    profiles: RefCell<Vec<ProfileFile>>,
    /// The open profile: its position in `profiles`.
    open_index: Cell<Option<usize>>,
}

impl MainWindow {
    /// Opens the window on the profile in the file `open`, when there is one, or else on
    /// the first profile.
    pub fn open(
        s: &'static dyn Strings,
        state: AppState,
        profiles_dir: PathBuf,
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
        let cancel_snapshot = Button::builder(&panel)
            .with_label(&button_label(s, s.cancel(), CANCEL_SNAPSHOT_ACCESS, false))
            .build();
        let snapshots = ListCtrl::builder(&panel)
            .with_style(ListCtrlStyle::Report)
            .build();
        let difference = ListCtrl::builder(&panel)
            .with_style(ListCtrlStyle::Report)
            .build();
        for (list, pane) in [
            (&snapshots, Pane::Snapshots),
            (&difference, Pane::Difference),
        ] {
            list.insert_column(0, s.pane(pane), ListColumnFormat::Left, 600);
            win32::set_accessible_name(list.get_handle(), s.pane(pane));
        }
        if state.demo {
            fill_demo(s, &state, &snapshots, &difference);
        }

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
        progress_row.add(&cancel_snapshot, 0, SizerFlag::All, 5);
        let sizer = BoxSizer::builder(Orientation::Vertical).build();
        sizer.add_sizer(&profile_row, 0, SizerFlag::Expand, 0);
        sizer.add_sizer(&progress_row, 0, SizerFlag::Expand, 0);
        sizer.add(&snapshots, 1, SizerFlag::Expand | SizerFlag::All, 5);
        sizer.add(&difference, 2, SizerFlag::Expand | SizerFlag::All, 5);
        panel.set_sizer(sizer, true);
        let timer = Timer::new(&frame);

        let window = Rc::new(Self {
            frame,
            s,
            state: RefCell::new(state),
            menu_bar,
            status,
            panel,
            profile,
            progress_label,
            progress_text,
            cancel_snapshot,
            snapshots,
            difference,
            timer,
            job: RefCell::new(None),
            profiles_dir,
            profiles: RefCell::new(Vec::new()),
            open_index: Cell::new(None),
        });
        window.show_progress(false);
        window.bind_events();
        win32::subclass_main_window(window.frame.get_handle());
        window.load_profiles(open.as_deref());
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
        // that cannot be refused, as at a shutdown, stops the snapshot without asking.
        let w = Rc::clone(self);
        self.frame.on_close(move |data| {
            if let WindowEventData::General(event) = data {
                let s = w.s;
                let may_close = if event.can_veto() {
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
                    event.skip(true);
                } else {
                    event.veto();
                }
            }
        });

        let w = Rc::clone(self);
        self.timer.on_tick(move |_| w.poll_snapshot());

        let w = Rc::clone(self);
        self.cancel_snapshot.on_click(move |_| w.cancel_snapshot());

        let w = Rc::clone(self);
        self.profile.on_selection_changed(move |event| {
            let selected = event.get_selection().and_then(|i| usize::try_from(i).ok());
            w.show_profile(selected);
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

    /// The access keys of the main window's labels and buttons, for `keyboard::route`:
    /// those of the progress row only while it is shown.
    fn labels(&self) -> Vec<u8> {
        let mut labels = vec![PROFILE_ACCESS];
        if self.job.borrow().is_some() {
            labels.extend([PROGRESS_ACCESS, CANCEL_SNAPSHOT_ACCESS]);
        }
        labels
    }

    /// Where the keyboard focus is, in the terms of `keyboard::route`.
    fn focus(&self) -> Focus {
        let focused = win32::focused_window();
        if focused == self.snapshots.get_handle() {
            Focus {
                part: Some(Pane::Snapshots),
                control: Control::MainList,
            }
        } else if focused == self.difference.get_handle() {
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

    fn list(&self, pane: Pane) -> &ListCtrl {
        match pane {
            Pane::Snapshots => &self.snapshots,
            Pane::Difference => &self.difference,
        }
    }

    fn apply(&self, routed: Route) {
        let s = self.s;
        match routed {
            Route::Run(id) => self.run(id),
            Route::Refuse(id, reason) => self.say(&s.unavailable(s.command(id), s.reason(reason))),
            Route::OnlyIn(key, pane) => self.say(&s.key_only_in(&key.name(s), pane)),
            Route::FocusPane(pane) => self.list(pane).set_focus(),
            // In the order of `labels`.
            Route::FocusLabel(0) => self.profile.set_focus(),
            Route::FocusLabel(1) => self.progress_text.set_focus(),
            Route::FocusLabel(_) => self.cancel_snapshot(),
            Route::ContextMenu(pane) => self.show_context_menu(pane, None),
            Route::Pass => {}
        }
    }

    /// Shows a message in the status bar; screen readers read it without the focus moving.
    pub fn say(&self, text: &str) {
        self.status.set_status_text(text, 0);
        win32::announce_status(self.status.get_handle());
    }

    fn run(&self, id: CommandId) {
        let s = self.s;
        let available = (command(id).available)(&self.state.borrow());
        if let Err(reason) = available {
            self.say(&s.unavailable(s.command(id), s.reason(reason)));
            return;
        }
        match id {
            CommandId::Exit => {
                self.frame.close(false);
                return;
            }
            CommandId::NewProfile => self.new_profile(),
            CommandId::EditProfile => self.edit_profile(),
            CommandId::DeleteProfile => self.delete_profile(),
            CommandId::RestartAsAdmin => self.restart_as_admin(),
            CommandId::TakeSnapshot => self.take_snapshot(),
            CommandId::CancelSnapshot => self.cancel_snapshot(),
            CommandId::KeyboardShortcuts => help::show_shortcuts(&self.frame, s),
            CommandId::About => help::show_about(&self.frame, s),
            // Only the demo state gets here: the other commands' tasks are not done yet.
            _ => {
                self.demo_run(id);
                self.say(&s.demo_done(s.command(id)));
            }
        }
        self.refresh_menus();
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

    /// Reads the profiles from the disk into the profile box, by the user's alphabet, and
    /// opens the one in the file `open`, or else the first. Says which files of the folder
    /// of profiles could not be opened.
    fn load_profiles(&self, open: Option<&Path>) {
        let s = self.s;
        let (mut found, unreadable) = match profiles::list(&self.profiles_dir) {
            Ok(found) => found,
            Err(e) => {
                let folder = self.profiles_dir.to_string_lossy();
                self.say(&s.profiles_not_listed(&folder, &s.failure(&failure::of_io(&e))));
                (Vec::new(), Vec::new())
            }
        };
        found.sort_by(|a, b| win32::compare_names(&a.name, &b.name));
        let selected = open
            .and_then(|path| found.iter().position(|p| p.path == path))
            .or((!found.is_empty()).then_some(0));
        self.profile.clear();
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

    /// Opens the profile at position `index` of the profile box, or none.
    fn show_profile(&self, index: Option<usize>) {
        let open = index.and_then(|i| self.profiles.borrow().get(i).cloned());
        self.open_index.set(index.filter(|_| open.is_some()));
        self.frame
            .set_title(&window_title(open.as_ref().map(|p| p.name.as_str())));
        {
            let mut state = self.state.borrow_mut();
            state.profile = open.as_ref().map(|p| p.name.clone());
            if !state.demo {
                state.snapshots = open
                    .as_ref()
                    .and_then(|p| profiles::snapshot_count(&p.path).ok())
                    .unwrap_or(0);
            }
            state.snapshotting_open_profile = match (&open, self.job.borrow().as_ref()) {
                (Some(open), Some(job)) => open.path == job.profile.path,
                _ => false,
            };
        }
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

    // --- The snapshot being taken (TASK-8) ---------------------------------------------

    /// Starts a snapshot of the open profile on its own thread and shows its progress.
    fn take_snapshot(&self) {
        let s = self.s;
        let Some(open) = self.open_profile() else {
            return;
        };
        *self.job.borrow_mut() = Some(SnapshotJob::start(open));
        {
            let mut state = self.state.borrow_mut();
            state.snapshot_running = true;
            state.snapshotting_open_profile = true;
        }
        self.progress_text
            .change_value(&s.snapshot_progress(0, &win32::format_count(0), ""));
        self.show_progress(true);
        self.timer.start(PROGRESS_INTERVAL, false);
        self.say(s.snapshot_started());
    }

    /// Asks the snapshot to stop; its end comes through the timer, like any other.
    fn cancel_snapshot(&self) {
        if let Some(job) = self.job.borrow().as_ref() {
            job.cancel.store(true, Ordering::Relaxed);
        }
    }

    /// Shows the progress of the snapshot and, once it is over, its outcome.
    fn poll_snapshot(&self) {
        let s = self.s;
        let result = {
            let job = self.job.borrow();
            let Some(job) = job.as_ref() else {
                return;
            };
            let entries = job.progress.entries();
            let entries_text = win32::format_count(entries);
            let text = if job.progress.saving() {
                s.snapshot_saving(entries, &entries_text)
            } else {
                let folder = job.progress.folder();
                s.snapshot_progress(entries, &entries_text, &folder.to_string_lossy())
            };
            self.progress_text.change_value(&text);
            match job.done.try_recv() {
                Ok(result) => Some(result),
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => None,
            }
        };
        self.timer.stop();
        let mut job = self
            .job
            .borrow_mut()
            .take()
            .expect("the job was there a moment ago");
        job.wait();
        self.end_snapshot();
        match result {
            Some(Ok(report)) => self.report_snapshot(&job.profile, &report),
            Some(Err(SnapshotError::Cancelled)) => self.say(s.snapshot_cancelled()),
            Some(Err(SnapshotError::Store(e))) => {
                self.snapshot_failed(&s.failure(&failure::of_store(&e)));
            }
            None => self.snapshot_failed(s.snapshot_stopped()),
        }
    }

    /// Takes the window back to its state without a snapshot: the progress row goes, and
    /// the focus with it moves to the snapshot list.
    fn end_snapshot(&self) {
        {
            let mut state = self.state.borrow_mut();
            state.snapshot_running = false;
            state.snapshotting_open_profile = false;
        }
        let focused = win32::focused_window();
        if focused == self.progress_text.get_handle()
            || focused == self.cancel_snapshot.get_handle()
        {
            self.snapshots.set_focus();
        }
        self.show_progress(false);
        self.refresh_menus();
    }

    fn show_progress(&self, shown: bool) {
        self.progress_label.show(shown);
        self.progress_text.show(shown);
        self.cancel_snapshot.show(shown);
        self.panel.layout();
    }

    /// Says what the saved snapshot holds, names the folders it did not find, and offers
    /// a restart as administrator when rights closed some entries (TASK-8 #7, #16, #18).
    fn report_snapshot(&self, profile: &ProfileFile, report: &Report) {
        let s = self.s;
        if self
            .open_profile()
            .is_some_and(|open| open.path == profile.path)
        {
            self.state.borrow_mut().snapshots += 1;
            self.refresh_menus();
        }
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
        if report.denied > 0 && !self.state.borrow().elevated {
            let question = s.restart_question(&win32::format_count(report.denied));
            let restart = (s.restart(), RESTART_ACCESS);
            let title = s.command(CommandId::TakeSnapshot);
            if confirm(&self.frame, s, title, &question, restart, true) {
                self.restart_now();
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
        if self.job.borrow().is_none() {
            return true;
        }
        self.timer.stop();
        if !confirm(&self.frame, self.s, title, question, action, true) {
            self.timer.start(PROGRESS_INTERVAL, false);
            return false;
        }
        self.stop_snapshot();
        true
    }

    /// Stops the snapshot being taken, if there is one, without asking.
    fn stop_snapshot(&self) {
        let Some(mut job) = self.job.borrow_mut().take() else {
            return;
        };
        self.timer.stop();
        job.stop();
        self.end_snapshot();
        self.say(self.s.snapshot_cancelled());
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
            self.restart_now();
        }
    }

    /// Starts revisor again as administrator, on the open profile, and closes this one.
    fn restart_now(&self) {
        let s = self.s;
        let exe = std::env::current_exe().unwrap_or_default();
        let args = self
            .open_profile()
            .map(|p| format!("--profile \"{}\"", p.path.display()))
            .unwrap_or_default();
        match win32::run_as_admin(&exe, &args) {
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

/// Stand-in rows so the lists' keys and context menus can be checked with NVDA.
fn fill_demo(s: &dyn Strings, state: &AppState, snapshots: &ListCtrl, difference: &ListCtrl) {
    for n in 0..state.snapshots {
        snapshots.insert_item(n as i64, &s.demo_snapshot(n + 1), None);
    }
    for n in 0..3 {
        difference.insert_item(n as i64, &s.demo_change(n + 1), None);
    }
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
        let labels = [PROFILE_ACCESS, PROGRESS_ACCESS, CANCEL_SNAPSHOT_ACCESS];
        for (i, &label) in labels.iter().enumerate() {
            assert!(MenuId::ALL.iter().all(|m| m.access() != label));
            assert!(!labels[..i].contains(&label));
        }
        // The letters are in the English names, as everywhere.
        let has = |name: &str, letter: u8| name.to_ascii_uppercase().contains(char::from(letter));
        assert!(has(En.profile(), PROFILE_ACCESS));
        assert!(has(En.progress(), PROGRESS_ACCESS));
        assert!(has(En.cancel(), CANCEL_SNAPSHOT_ACCESS));
        assert!(has(En.restart(), RESTART_ACCESS));
        assert!(has(En.close(), CLOSE_ACCESS));
        assert!(has(En.delete(), DELETE_ACCESS));
    }
}
