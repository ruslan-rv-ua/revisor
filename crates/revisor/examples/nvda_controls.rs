//! Manual accessibility check for native wxDragon controls with NVDA (TASK-2).
//!
//! One window with a menu bar, a virtual list of one million rows, a lazily filled tree
//! and a cancellable background job. Run with `cargo run --example nvda_controls`.
//! This code is a spike and is not part of the application.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::thread;
use std::time::Duration;

use wxdragon::prelude::*;

const ROW_COUNT: i64 = 1_000_000;
const TREE_DEPTH: usize = 5;
const FOLDERS_PER_NODE: usize = 3;
const FILES_PER_NODE: usize = 2;
const JOB_STEPS: u32 = 200;
const JOB_STEP_DELAY: Duration = Duration::from_millis(100);
const POLL_INTERVAL_MS: i32 = 100;

const ID_GO_LIST: Id = ID_HIGHEST + 1;
const ID_GO_TREE: Id = ID_HIGHEST + 2;
const ID_GO_ROW: Id = ID_HIGHEST + 3;
const ID_START: Id = ID_HIGHEST + 4;
const ID_CANCEL: Id = ID_HIGHEST + 5;

const CHANGES: [&str; 3] = ["Added", "Modified", "Removed"];

/// Text of one cell of the synthetic diff list, generated from the row number.
fn cell_text(row: i64, col: i32) -> String {
    match col {
        0 => format!(r"C:\data\folder {:04}\file {:07}.txt", row / 1000, row + 1),
        1 => CHANGES[row as usize % CHANGES.len()].to_string(),
        2 => format!("{} bytes", (row * 7919) % 10_000_000),
        3 => format!(
            "2026-10-{:02} {:02}:{:02}",
            row % 28 + 1,
            row % 24,
            row % 60
        ),
        _ => String::new(),
    }
}

/// State shared between the background job thread and the UI.
#[derive(Default)]
struct Job {
    progress: AtomicU32,
    cancel: AtomicBool,
    finished: AtomicBool,
}

fn main() {
    wxdragon::main(|_| {
        let frame = Frame::builder()
            .with_title("revisor: NVDA controls check")
            .with_size(Size::new(1000, 700))
            .build();

        frame.set_menu_bar(build_menu_bar());

        let panel = Panel::builder(&frame).build();
        let sizer = BoxSizer::builder(Orientation::Vertical).build();

        // A static text right before a control becomes its accessible name.
        let list_label = StaticText::builder(&panel)
            .with_label(&format!("&Changes ({ROW_COUNT} rows):"))
            .build();
        let list = ListCtrl::builder(&panel)
            .with_style(ListCtrlStyle::Report | ListCtrlStyle::Virtual | ListCtrlStyle::SingleSel)
            .build();
        for (col, (heading, width)) in [
            ("Path", 420),
            ("Change", 100),
            ("Size", 140),
            ("Modified", 160),
        ]
        .into_iter()
        .enumerate()
        {
            list.insert_column(col as i64, heading, ListColumnFormat::Left, width);
        }
        list.set_virtual_text_callback(cell_text);
        list.set_item_count(ROW_COUNT);

        let tree_label = StaticText::builder(&panel).with_label("&Folders:").build();
        let tree = TreeCtrl::builder(&panel)
            .with_style(TreeCtrlStyle::Default | TreeCtrlStyle::Single | TreeCtrlStyle::HideRoot)
            .build();
        let root = tree
            .add_root("root", None, None)
            .expect("failed to add tree root");
        append_tree_children(&tree, &root, "");
        tree.on_item_expanding(move |event| {
            let Some(item) = event.get_item() else { return };
            if tree.get_children_count(&item, false) > 0 {
                return;
            }
            if let Some(path) = tree
                .get_custom_data_direct(&item)
                .and_then(|data| data.downcast_ref::<String>().cloned())
            {
                append_tree_children(&tree, &item, &path);
            }
        });

        let start = Button::builder(&panel).with_label("&Start job").build();
        let progress_label = StaticText::builder(&panel)
            .with_label("Job &progress:")
            .build();
        let gauge = Gauge::builder(&panel).with_range(JOB_STEPS as i32).build();
        // A read-only text box is a tab stop, so the job state can be reached with Tab.
        let status_label = StaticText::builder(&panel)
            .with_label("Job s&tatus:")
            .build();
        let status = TextCtrl::builder(&panel)
            .with_value("Idle")
            .with_style(TextCtrlStyle::ReadOnly)
            .build();
        let cancel = Button::builder(&panel).with_label("Ca&ncel job").build();
        cancel.enable(false);

        sizer.add(&list_label, 0, SizerFlag::All, 5);
        sizer.add(
            &list,
            3,
            SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right,
            5,
        );
        sizer.add(&tree_label, 0, SizerFlag::All, 5);
        sizer.add(
            &tree,
            2,
            SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right,
            5,
        );
        let job_row = BoxSizer::builder(Orientation::Horizontal).build();
        job_row.add(&start, 0, SizerFlag::All, 5);
        job_row.add(
            &progress_label,
            0,
            SizerFlag::All | SizerFlag::AlignCenterVertical,
            5,
        );
        job_row.add(
            &gauge,
            1,
            SizerFlag::All | SizerFlag::AlignCenterVertical,
            5,
        );
        job_row.add(
            &status_label,
            0,
            SizerFlag::All | SizerFlag::AlignCenterVertical,
            5,
        );
        job_row.add(
            &status,
            1,
            SizerFlag::All | SizerFlag::AlignCenterVertical,
            5,
        );
        job_row.add(&cancel, 0, SizerFlag::All, 5);
        sizer.add_sizer(&job_row, 0, SizerFlag::Expand, 0);
        panel.set_sizer(sizer, true);

        let job_ui = JobUi {
            start,
            cancel,
            gauge,
            status,
            timer: Rc::new(Timer::new(&frame)),
            job: Rc::new(Cell::new(None)),
        };
        let job_ui = Rc::new(job_ui);
        {
            // The tick handler keeps `JobUi` and so the timer alive for the whole run.
            let ui = job_ui.clone();
            job_ui.timer.on_tick(move |_| ui.poll());
        }
        {
            let job_ui = job_ui.clone();
            start.on_click(move |_| job_ui.start());
        }
        {
            let job_ui = job_ui.clone();
            cancel.on_click(move |_| job_ui.cancel());
        }

        frame.on_menu(move |event| match event.get_id() {
            ID_EXIT => {
                frame.close(true);
            }
            ID_GO_LIST => list.set_focus(),
            ID_GO_TREE => tree.set_focus(),
            ID_GO_ROW => go_to_row(&frame, &list),
            ID_START => job_ui.start(),
            ID_CANCEL => job_ui.cancel(),
            ID_ABOUT => {
                MessageDialog::builder(
                    &frame,
                    "Checks native wxDragon controls with NVDA (revisor TASK-2).",
                    "About",
                )
                .build()
                .show_modal();
            }
            _ => event.skip(true),
        });

        frame.centre();
        frame.show(true);
        list.set_focus();
    })
    .expect("failed to start the GUI");
}

fn build_menu_bar() -> MenuBar {
    let file = Menu::builder()
        .append_item(ID_EXIT, "E&xit\tCtrl+Q", "Close the window")
        .build();
    let view = Menu::builder()
        .append_item(ID_GO_LIST, "Go to &list\tCtrl+1", "Move focus to the list")
        .append_item(ID_GO_TREE, "Go to &tree\tCtrl+2", "Move focus to the tree")
        .append_item(
            ID_GO_ROW,
            "Go to &row...\tCtrl+G",
            "Jump to a row by its number",
        )
        .build();
    let work = Menu::builder()
        .append_item(ID_START, "&Start job\tF5", "Start the background job")
        .append_item(
            ID_CANCEL,
            "&Cancel job\tShift+F5",
            "Cancel the background job",
        )
        .build();
    let help = Menu::builder()
        .append_item(ID_ABOUT, "&About\tF1", "About this check")
        .build();
    MenuBar::builder()
        .append(file, "&File")
        .append(view, "&View")
        .append(work, "&Work")
        .append(help, "&Help")
        .build()
}

/// Appends the synthetic children of the folder at `path` ("" is the root, "2.1" is a folder).
fn append_tree_children(tree: &TreeCtrl, parent: &TreeItemId, path: &str) {
    let depth = if path.is_empty() {
        0
    } else {
        path.split('.').count()
    };
    let child_path = |i: usize| {
        if path.is_empty() {
            i.to_string()
        } else {
            format!("{path}.{i}")
        }
    };
    if depth < TREE_DEPTH {
        for i in 1..=FOLDERS_PER_NODE {
            let folder_path = child_path(i);
            let label = format!("folder {folder_path}");
            if let Some(item) = tree.append_item_with_data(parent, &label, folder_path, None, None)
            {
                // Children are added only when the folder is expanded.
                tree.set_item_has_children(&item, true);
            }
        }
    }
    if depth > 0 {
        for i in 1..=FILES_PER_NODE {
            tree.append_item(parent, &format!("file {}.txt", child_path(i)), None, None);
        }
    }
}

fn go_to_row(frame: &Frame, list: &ListCtrl) {
    let dialog = TextEntryDialog::builder(
        frame,
        &format!("Row number (1 to {ROW_COUNT}):"),
        "Go to row",
    )
    .build();
    if dialog.show_modal() != ID_OK {
        return;
    }
    let Some(row) = dialog
        .get_value()
        .and_then(|value| value.trim().replace([',', ' '], "").parse::<i64>().ok())
        .filter(|row| (1..=ROW_COUNT).contains(row))
    else {
        MessageDialog::builder(
            frame,
            &format!("Enter a number from 1 to {ROW_COUNT}."),
            "Go to row",
        )
        .build()
        .show_modal();
        return;
    };
    let index = row - 1;
    let state = ListItemState::Selected | ListItemState::Focused;
    list.set_item_state(index, state, state);
    list.ensure_visible(index);
    list.set_focus();
}

/// Controls and state of the background job.
struct JobUi {
    start: Button,
    cancel: Button,
    gauge: Gauge,
    status: TextCtrl,
    timer: Rc<Timer<Frame>>,
    job: Rc<Cell<Option<Arc<Job>>>>,
}

impl JobUi {
    fn current(&self) -> Option<Arc<Job>> {
        let job = self.job.take();
        self.job.set(job.clone());
        job
    }

    fn start(&self) {
        if self.current().is_some() {
            return;
        }
        let job = Arc::new(Job::default());
        let worker = job.clone();
        thread::spawn(move || {
            for step in 1..=JOB_STEPS {
                if worker.cancel.load(Ordering::Relaxed) {
                    break;
                }
                thread::sleep(JOB_STEP_DELAY);
                worker.progress.store(step, Ordering::Relaxed);
            }
            worker.finished.store(true, Ordering::Release);
        });
        self.job.set(Some(job));
        self.gauge.set_value(0);
        self.status.set_value("Running: 0%");
        swap_enabled(&self.start, &self.cancel);
        self.timer.start(POLL_INTERVAL_MS, false);
    }

    fn cancel(&self) {
        if let Some(job) = self.current() {
            job.cancel.store(true, Ordering::Relaxed);
            self.status.set_value("Cancelling...");
        }
    }

    fn poll(&self) {
        let Some(job) = self.current() else {
            self.timer.stop();
            return;
        };
        // Read `finished` before `progress`: the Acquire load then guarantees the final step.
        let finished = job.finished.load(Ordering::Acquire);
        let step = job.progress.load(Ordering::Relaxed);
        let percent = step * 100 / JOB_STEPS;
        self.gauge.set_value(step as i32);
        if !finished {
            if !job.cancel.load(Ordering::Relaxed) {
                self.status.set_value(&format!("Running: {percent}%"));
            }
            return;
        }
        self.timer.stop();
        self.job.set(None);
        let summary = if step < JOB_STEPS {
            format!("Cancelled at {percent}%")
        } else {
            "Finished: 100%".to_string()
        };
        self.status.set_value(&summary);
        swap_enabled(&self.cancel, &self.start);
    }
}

/// Disables `from` and enables `to`, moving the keyboard focus along if `from` had it;
/// a disabled focused button would leave the focus nowhere.
fn swap_enabled(from: &Button, to: &Button) {
    let move_focus = from.has_focus();
    to.enable(true);
    from.enable(false);
    if move_focus {
        to.set_focus();
    }
}
