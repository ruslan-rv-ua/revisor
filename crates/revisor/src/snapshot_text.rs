//! What a saved snapshot says (TASK-8), in the window's status bar and on the command line
//! (TASK-47): how many entries it holds, how many of them are inaccessible, and which
//! folders it did not find.

use revisor_core::snapshot::Report;
use revisor_core::store::{FolderState, Unavailable};

use crate::difference;
use crate::i18n::Strings;
use crate::win32::format::{self, Locale};

/// What the snapshot of `report` says.
pub fn report(s: &dyn Strings, report: &Report) -> String {
    let entries = format::format_count(Locale::USER, report.entries);
    report_after(s, s.snapshot_saved(report.entries, &entries), report)
}

/// `first`, then what the snapshot of `report` says after how many entries it holds: how
/// many of them are inaccessible, and which folders it did not find.
pub fn report_after(s: &dyn Strings, first: String, report: &Report) -> String {
    let mut text = first;
    if report.inaccessible > 0 {
        text.push(' ');
        text.push_str(
            &s.entries_inaccessible(&format::format_count(Locale::USER, report.inaccessible)),
        );
    }
    let not_found: Vec<(String, String)> = report
        .not_found()
        .map(|f| {
            (
                f.path.to_string_lossy().into_owned(),
                folder_state_reason(s, &f.state),
            )
        })
        .collect();
    if !not_found.is_empty() {
        text.push(' ');
        text.push_str(&s.folders_not_found(&not_found));
    }
    text
}

/// Why the snapshot did not scan a folder, in the interface's words: the reason of an
/// unavailable folder is the comparison's (TASK-25), and a Windows error says the folder
/// could not be reached.
pub fn folder_state_reason(s: &dyn Strings, state: &FolderState) -> String {
    match state {
        FolderState::Scanned { .. } => String::new(),
        FolderState::Missing => s.folder_missing().into(),
        FolderState::Unavailable(reason @ Unavailable::Error(_)) => {
            s.folder_unreachable(&difference::unavailable_reason(s, reason))
        }
        FolderState::Unavailable(reason) => difference::unavailable_reason(s, reason),
    }
}
