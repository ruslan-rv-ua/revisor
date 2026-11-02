//! Taking a snapshot of a profile (TASK-8): each folder of the profile is examined and
//! scanned with the profile's exclusions, and what was collected is saved in the
//! profile's database in one transaction, so a snapshot that was cancelled or failed
//! leaves nothing behind.

use std::fmt;
use std::fs;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation;

use crate::exclude::{self, Exclusions, Expanded, SystemEnvironment};
use crate::folders::{self, LocateError};
use crate::scan::{self, Progress, ScanError, ScanOptions, ScanStats};
use crate::store::{
    self, FolderSnapshot, FolderState, Snapshot, SnapshotExclusion, SnapshotFolder, Store,
    StoreError, Unavailable,
};

// Windows error codes as `io::Error::raw_os_error` gives them.
const ERROR_FILE_NOT_FOUND: i32 = Foundation::ERROR_FILE_NOT_FOUND as i32;
const ERROR_PATH_NOT_FOUND: i32 = Foundation::ERROR_PATH_NOT_FOUND as i32;
const ERROR_ACCESS_DENIED: i32 = Foundation::ERROR_ACCESS_DENIED as i32;

/// What a finished snapshot reports.
#[derive(Clone, Debug)]
pub struct Report {
    /// The id of the saved snapshot.
    pub id: i64,
    pub entries: u64,
    /// Entries marked inaccessible.
    pub inaccessible: u64,
    /// Entries and folders that access rights kept closed; an administrator may read them.
    pub denied: u64,
    /// Exe and dll files whose version was not read.
    pub versions_unread: u64,
    /// The profile's folders with what the snapshot holds for each.
    pub folders: Vec<SnapshotFolder>,
    /// Scanning and saving together.
    pub elapsed: Duration,
}

impl Report {
    /// The folders that were not scanned: missing from the disk or unavailable.
    pub fn not_found(&self) -> impl Iterator<Item = &SnapshotFolder> {
        self.folders
            .iter()
            .filter(|f| !matches!(f.state, FolderState::Scanned { .. }))
    }
}

#[derive(Debug)]
pub enum SnapshotError {
    Cancelled,
    /// The profile's database could not be read or written.
    Store(StoreError),
}

impl fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SnapshotError::Cancelled => f.write_str("snapshot cancelled"),
            SnapshotError::Store(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for SnapshotError {}

impl From<StoreError> for SnapshotError {
    fn from(e: StoreError) -> Self {
        match e {
            StoreError::Cancelled => SnapshotError::Cancelled,
            e => SnapshotError::Store(e),
        }
    }
}

/// Takes a snapshot of the profile whose database is at `db` and saves it there. The
/// scans report to `progress`, and setting `cancel` stops the snapshot with
/// [`SnapshotError::Cancelled`].
pub fn take(
    db: &Path,
    comment: Option<&str>,
    progress: &Progress,
    cancel: &AtomicBool,
) -> Result<Report, SnapshotError> {
    let started = Instant::now();
    let mut store = Store::open(db)?;
    let scope = store.settings()?.scope;
    let taken_at = store::now_millis();
    // The exclusions are expanded once: the scan applies them, and the snapshot records
    // them, exactly as expanded here.
    let expanded: Vec<Expanded> = scope
        .exclusions
        .iter()
        .map(|pattern| exclude::expand(pattern, &SystemEnvironment))
        .collect();
    let options = ScanOptions {
        exclusions: Exclusions::from_expanded(&expanded),
        ..ScanOptions::default()
    };
    let exclusions = scope
        .exclusions
        .iter()
        .zip(&expanded)
        .map(|(pattern, expanded)| SnapshotExclusion {
            pattern: pattern.clone(),
            applied: expanded.applied().map(String::from),
        })
        .collect();

    let mut totals = Totals::default();
    let mut folders = Vec::with_capacity(scope.folders.len());
    for path in &scope.folders {
        if cancel.load(Ordering::Relaxed) {
            return Err(SnapshotError::Cancelled);
        }
        folders.push(scan_folder(path, &options, cancel, progress, &mut totals)?);
    }
    let snapshot = Snapshot {
        taken_at,
        comment: comment.map(String::from),
        folders,
        exclusions,
    };
    progress.set_saving(true);
    let id = store.save(&snapshot, cancel)?;
    Ok(Report {
        id,
        entries: totals.entries,
        inaccessible: totals.inaccessible,
        denied: totals.denied,
        versions_unread: totals.versions_unread,
        folders: snapshot.folders.iter().map(SnapshotFolder::from).collect(),
        elapsed: started.elapsed(),
    })
}

#[derive(Default)]
struct Totals {
    entries: u64,
    inaccessible: u64,
    denied: u64,
    versions_unread: u64,
}

impl Totals {
    fn add(&mut self, stats: &ScanStats, entries: usize) {
        self.entries += entries as u64;
        self.inaccessible += stats.inaccessible;
        self.denied += stats.denied;
        self.versions_unread += stats.versions_unread;
    }
}

/// Scans one folder of the profile, or finds out why it cannot be scanned.
fn scan_folder(
    path: &Path,
    options: &ScanOptions,
    cancel: &AtomicBool,
    progress: &Progress,
    totals: &mut Totals,
) -> Result<FolderSnapshot, SnapshotError> {
    let without_tree = |state| FolderSnapshot {
        path: path.to_path_buf(),
        state,
        entries: Vec::new(),
    };
    // A folder that even the attributes of cannot be read is closed by rights too.
    let mut not_opened = |e: &io::Error| {
        if e.raw_os_error() == Some(ERROR_ACCESS_DENIED) {
            totals.denied += 1;
        }
        without_tree(absent(path, e))
    };
    match folders::locate(path) {
        Ok(_) => {}
        Err(LocateError::NotNtfs(fs)) => {
            return Ok(without_tree(FolderState::Unavailable(
                Unavailable::NotNtfs(fs),
            )));
        }
        Err(LocateError::Network) => {
            return Ok(without_tree(FolderState::Unavailable(Unavailable::Network)));
        }
        Err(LocateError::Io(e)) => return Ok(not_opened(&e)),
    }
    match scan::scan(path, options, cancel, progress) {
        Ok(scan) => {
            totals.add(&scan.stats, scan.entries.len());
            Ok(FolderSnapshot {
                path: path.to_path_buf(),
                state: FolderState::Scanned {
                    volume_serial: scan.volume_serial,
                },
                entries: scan.entries,
            })
        }
        Err(ScanError::Cancelled) => Err(SnapshotError::Cancelled),
        Err(ScanError::Root(e)) if e.kind() == io::ErrorKind::NotADirectory => Ok(without_tree(
            FolderState::Unavailable(Unavailable::NotAFolder),
        )),
        Err(ScanError::Root(e)) => Ok(not_opened(&e)),
    }
}

/// The state of a folder that could not be opened: missing when it is not there but the
/// nearest folder above it is (so its volume is), unavailable with the error otherwise.
fn absent(path: &Path, error: &io::Error) -> FolderState {
    let code = error.raw_os_error().unwrap_or(0);
    let not_found =
        |code: Option<i32>| matches!(code, Some(ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND));
    if not_found(Some(code)) {
        for above in path.ancestors().skip(1) {
            match fs::metadata(above) {
                Ok(_) => return FolderState::Missing,
                Err(e) if not_found(e.raw_os_error()) => continue,
                Err(_) => break,
            }
        }
    }
    FolderState::Unavailable(Unavailable::Error(code))
}

#[cfg(test)]
mod tests;
