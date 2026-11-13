//! Comparison of two snapshots of a profile (TASK-10): which folders they share, what
//! each snapshot holds for them, and the exclusions they were taken with; the trees of
//! the shared folders are then compared by the rules of [`crate::diff`].
//!
//! The difference goes from the earlier snapshot to the later one by the order they were
//! taken in — the order of their ids — whichever was asked for first. Only what both
//! snapshots cover is compared: a folder of one snapshot only is left out, a folder that
//! lies inside a folder of the other snapshot is compared within it, and the exclusions
//! of both snapshots apply to both. Whatever is left out, and whatever explains a mass
//! of changes, is reported as a [`Warning`].
//!
//! The folders are compared together, so an entry renamed or moved is recognised between
//! any folders of the profile that lie on one volume (TASK-42): it is a change of the
//! folder it is in now, and the entry it was is in the earlier tree of the folder it came
//! from ([`Difference::earlier_tree`]).
//!
//! A comparison has two stages: [`load`] reads what the database holds for the two
//! snapshots — their folders, exclusions and the trees of the folders both cover — and
//! [`Loaded::compare`] compares the trees, with no more need of the database.
//! [`CompareWork`] runs both as a background job (ADR-0011, TASK-11), waiting for the
//! database while another connection holds it, and [`compare`] runs both in one go.

use std::ffi::OsString;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::time::Duration;

use crate::diff::{self, Diff, Filter, Inaccessible, Rules, Subtree};
use crate::exclude::{Branch, Exclusions, RecordedEnvironment};
use crate::job::{Cancel, Outcome, Work};
use crate::path::{self, Folded, eq_ignore_case, eq_ignore_case_os};
use crate::scan::{Entry, EntryKind};
use crate::store::{
    self, FolderState, SnapshotExclusion, SnapshotFolder, SnapshotInfo, Store, StoreError,
    Unavailable,
};

/// One of the two snapshots compared.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Earlier,
    Later,
}

impl Side {
    fn index(self) -> usize {
        match self {
            Side::Earlier => 0,
            Side::Later => 1,
        }
    }

    fn other(self) -> Side {
        match self {
            Side::Earlier => Side::Later,
            Side::Later => Side::Earlier,
        }
    }
}

/// What the comparison left out, or what explains a mass of changes (TASK-10 #16, #17,
/// #20, #21). The warnings about folders come in the order of the later snapshot's
/// folders, then those about the earlier snapshot's folders the later one does not have,
/// then those about exclusions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Warning {
    /// The folder is in this snapshot only, so it is not compared.
    FolderOnlyIn { path: PathBuf, side: Side },
    /// The folder `wider` of this snapshot holds the folder `compared` of the other
    /// snapshot: only `compared` is compared, and the rest of `wider` is not.
    FolderNarrowed {
        wider: PathBuf,
        side: Side,
        compared: PathBuf,
    },
    /// The folder was recorded as missing in this snapshot, so what the other snapshot
    /// holds for it shows as added or deleted.
    FolderMissing { path: PathBuf, side: Side },
    /// The folder could not be read in this snapshot, so it is not compared.
    FolderUnavailable {
        path: PathBuf,
        side: Side,
        reason: Unavailable,
    },
    /// Another volume stood at the folder's path in each snapshot: the folder is compared
    /// as usual, but file IDs are not, so nothing in it is replaced, renamed or moved,
    /// and a mass of changes may come from that.
    DifferentVolumes {
        path: PathBuf,
        earlier: u64,
        later: u64,
    },
    /// The exclusion is in this snapshot only; it applies to both.
    ExclusionOnlyIn { pattern: String, side: Side },
    /// The exclusion applied differently in the two snapshots, as when TEMP expands
    /// elsewhere under another account; both ways apply to both. `None`: it did not
    /// apply, its variable being not set.
    ExclusionAppliedDifferently {
        pattern: String,
        earlier: Option<String>,
        later: Option<String>,
    },
}

/// The difference between the trees of one folder covered by both snapshots.
#[derive(Debug)]
pub struct FolderDifference {
    /// The folder compared: a folder of the profile, or the narrower of two nested ones.
    pub path: PathBuf,
    /// The tree of the earlier snapshot that holds the folder; empty when the folder was
    /// missing then.
    pub old: Vec<Entry>,
    /// The tree of the later snapshot that holds the folder.
    pub new: Vec<Entry>,
    /// Changes and inaccessible entries, by indices into `old` and `new`. The earlier
    /// entry of one that came from another folder of the difference, or lies in a folder
    /// that did, is in that folder's `old` ([`Difference::earlier_tree`]).
    pub diff: Diff,
}

/// The difference between two snapshots, from the earlier to the later.
#[derive(Debug)]
pub struct Difference {
    pub earlier: SnapshotInfo,
    pub later: SnapshotInfo,
    /// The folders covered by both snapshots, in the order of the later snapshot's.
    pub folders: Vec<FolderDifference>,
    pub warnings: Vec<Warning>,
}

impl Difference {
    /// The earlier tree that holds the earlier entry of a change or an inaccessible entry
    /// of the folder `folder`: that folder's own, or, with `old_in`
    /// ([`diff::Change::old_in`]), that of the folder the entry came from.
    pub fn earlier_tree(&self, folder: usize, old_in: Option<u32>) -> &[Entry] {
        &self.folders[old_in.map_or(folder, |from| from as usize)].old
    }
}

#[derive(Debug)]
pub enum CompareError {
    Cancelled,
    /// No snapshot has this id: it was deleted.
    NoSuchSnapshot(i64),
    /// The profile's database could not be read.
    Store(StoreError),
}

impl fmt::Display for CompareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompareError::Cancelled => f.write_str("comparison cancelled"),
            CompareError::NoSuchSnapshot(id) => write!(f, "there is no snapshot {id} any more"),
            CompareError::Store(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for CompareError {}

impl From<StoreError> for CompareError {
    fn from(e: StoreError) -> Self {
        match e {
            StoreError::Cancelled => CompareError::Cancelled,
            e => CompareError::Store(e),
        }
    }
}

impl From<diff::Cancelled> for CompareError {
    fn from(_: diff::Cancelled) -> Self {
        CompareError::Cancelled
    }
}

/// What a comparison is doing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    /// The trees are being loaded from the profile's database.
    #[default]
    Loading,
    /// The database is held by another connection — a snapshot being saved, another
    /// revisor compacting it — and the comparison waits for it (TASK-11 #16).
    Waiting,
    /// The trees are loaded, the database is not needed any more, and they are being
    /// compared.
    Comparing,
}

/// What a comparison reports to whoever waits for it (TASK-11 #8): its phase, and how
/// far it is, as entries done out of the entries to do — the entries of both snapshots,
/// loaded and then visited by the comparison. The count never goes back and reaches the
/// total only once the comparison is over: entries the comparison leaves out, excluded
/// or under a folder inaccessible on one side only, are never visited. Shared with
/// another thread, which reads it now and then.
#[derive(Debug, Default)]
pub struct Progress {
    phase: AtomicU8,
    done: AtomicU64,
    total: AtomicU64,
}

impl Progress {
    pub fn phase(&self) -> Phase {
        match self.phase.load(Ordering::Relaxed) {
            1 => Phase::Waiting,
            2 => Phase::Comparing,
            _ => Phase::Loading,
        }
    }

    /// The entries loaded and visited so far.
    pub fn done(&self) -> u64 {
        self.done.load(Ordering::Relaxed)
    }

    /// The entries to load and visit: twice the entries of both snapshots.
    pub fn total(&self) -> u64 {
        self.total.load(Ordering::Relaxed)
    }

    /// How far the comparison is, in whole percent; 100 only once it is over, and 0 for
    /// snapshots with nothing to compare.
    pub fn percent(&self) -> u8 {
        (self.done() * 100)
            .checked_div(self.total())
            .map_or(0, |percent| percent.min(100) as u8)
    }

    fn set_phase(&self, phase: Phase) {
        self.phase.store(phase as u8, Ordering::Relaxed);
    }

    /// Ends the count: the comparison is over, whatever it did not visit.
    fn finish(&self) {
        self.done.store(self.total(), Ordering::Relaxed);
    }
}

/// Runs `read`, an access to the database, waiting while the database is busy; the
/// progress says so meanwhile (TASK-11 #16).
fn access<T>(
    cancel: &Cancel,
    progress: &Progress,
    read: impl FnMut() -> Result<T, StoreError>,
) -> Result<T, CompareError> {
    let waiting = |waiting: bool| {
        progress.set_phase(if waiting {
            Phase::Waiting
        } else {
            Phase::Loading
        });
    };
    Ok(store::retry_while_busy(cancel, waiting, read)?)
}

/// Comparing two snapshots as a background job (ADR-0011, TASK-11): [`load`] and
/// [`Loaded::compare`] on the job's thread. The profile's database is opened without
/// SQLite's own wait for another connection's write, which a cancel could not end: the
/// work waits in its own way, access by access, as long as the database is busy, and
/// ends as cancelled when asked to (#16). The database is closed before the trees are
/// compared, so the profile is free by then (#17).
#[derive(Clone, Debug)]
pub struct CompareWork {
    db: PathBuf,
    a: i64,
    b: i64,
}

impl CompareWork {
    /// A comparison of the snapshots `a` and `b` of the profile whose database is at
    /// `db`.
    pub fn new(db: &Path, a: i64, b: i64) -> Self {
        Self {
            db: db.to_path_buf(),
            a,
            b,
        }
    }

    fn compare(&self, progress: &Progress, cancel: &Cancel) -> Result<Difference, CompareError> {
        let loaded = {
            let store = access(cancel, progress, || {
                Store::open_with_timeout(&self.db, Duration::ZERO)
            })?;
            load(&store, self.a, self.b, cancel, progress)?
        };
        loaded.compare(cancel, progress)
    }
}

impl Work for CompareWork {
    type Progress = Progress;
    type Output = Outcome<Difference, CompareError>;

    fn run(self, progress: &Progress, cancel: &Cancel) -> Self::Output {
        let result = self.compare(progress, cancel);
        Outcome::of(result, |e| matches!(e, CompareError::Cancelled))
    }
}

/// Compares the snapshots `a` and `b` of the profile in `store`, the earlier with the
/// later, loading their trees from the database. Cancelling `cancel` stops with
/// [`CompareError::Cancelled`], while the trees are being loaded too.
pub fn compare(store: &Store, a: i64, b: i64, cancel: &Cancel) -> Result<Difference, CompareError> {
    let progress = Progress::default();
    load(store, a, b, cancel, &progress)?.compare(cancel, &progress)
}

/// What [`load`] read for a comparison: the trees of the folders both snapshots cover,
/// with what comparing them needs, and the warnings gathered so far. The database is not
/// needed any more: its snapshots may be deleted, or the profile itself, meanwhile
/// (TASK-11 #17).
#[derive(Debug)]
pub struct Loaded {
    earlier: SnapshotInfo,
    later: SnapshotInfo,
    pairs: Vec<LoadedPair>,
    warnings: Vec<Warning>,
    /// The exclusions of both snapshots together, as they applied.
    union: Exclusions,
}

/// The trees of one folder covered by both snapshots, with what comparing them needs.
#[derive(Debug)]
struct LoadedPair {
    /// The folder compared: a folder of the profile, or the narrower of two nested ones.
    path: PathBuf,
    old: Vec<Entry>,
    new: Vec<Entry>,
    /// Where the compared folder stands in each tree.
    located: [Located; 2],
    /// Where the compared roots stand against the exclusions.
    branch: Branch,
    /// The volumes the folder lay on in the earlier and in the later snapshot: file IDs
    /// are compared only within one volume.
    volumes: [u64; 2],
}

/// Loads from `store` what comparing the snapshots `a` and `b` needs: the snapshots,
/// their folders and exclusions, and the trees of the folders both cover, so that
/// [`Loaded::compare`] goes on without the database. Cancelling `cancel` stops with
/// [`CompareError::Cancelled`]. `progress` learns how many entries there are to load and
/// to compare, counts the loaded ones, and says when an access waits for a busy database
/// (TASK-11 #16).
pub fn load(
    store: &Store,
    a: i64,
    b: i64,
    cancel: &Cancel,
    progress: &Progress,
) -> Result<Loaded, CompareError> {
    progress.set_phase(Phase::Loading);
    let info = |id: i64| -> Result<SnapshotInfo, CompareError> {
        access(cancel, progress, || store.snapshot(id))?.ok_or(CompareError::NoSuchSnapshot(id))
    };
    // Ids are given out in the order the snapshots were taken, which the clock may not
    // follow.
    let (earlier, later) = (info(a.min(b))?, info(a.max(b))?);
    let ids = [earlier.id, later.id];
    let folders = [
        access(cancel, progress, || store.snapshot_folders(ids[0]))?,
        access(cancel, progress, || store.snapshot_folders(ids[1]))?,
    ];
    let exclusions = [
        access(cancel, progress, || store.snapshot_exclusions(ids[0]))?,
        access(cancel, progress, || store.snapshot_exclusions(ids[1]))?,
    ];
    let entries = [
        access(cancel, progress, || store.snapshot_entries(ids[0]))?,
        access(cancel, progress, || store.snapshot_entries(ids[1]))?,
    ];
    // Every entry is loaded once and visited once.
    progress
        .total
        .store(2 * (entries[0] + entries[1]), Ordering::Relaxed);
    // The exclusions of each snapshot as they applied, and both together: what either
    // snapshot left out, the comparison leaves out of both.
    let own = exclusions
        .each_ref()
        .map(|e| Exclusions::new(&applied(e), &RecordedEnvironment));
    let both: Vec<&str> = exclusions.iter().flat_map(|e| applied(e)).collect();
    let union = Exclusions::new(&both, &RecordedEnvironment);

    let mut warnings = Vec::new();
    let mut pairs = Vec::new();
    for pairing in pair_folders(&folders[0], &folders[1]) {
        if cancel.is_cancelled() {
            return Err(CompareError::Cancelled);
        }
        match pairing {
            Pairing::OnlyIn { position, side } => warnings.push(Warning::FolderOnlyIn {
                path: folders[side.index()][position].path.clone(),
                side,
            }),
            Pairing::Both(pair) => {
                let paired = [&folders[0][pair.earlier], &folders[1][pair.later]];
                let loading = FolderLoading {
                    store,
                    ids,
                    pair,
                    folders: paired,
                    relative: match pair.wider {
                        Some(side) => relative_parts(
                            &paired[side.index()].path,
                            &paired[side.other().index()].path,
                        ),
                        None => Vec::new(),
                    },
                    union: &union,
                    own: own.each_ref(),
                    cancel,
                    progress,
                };
                if let Some(loaded) = loading.load(&mut warnings)? {
                    pairs.push(loaded);
                }
            }
        }
    }
    // A snapshot deleted meanwhile loads as empty trees, which would look like a folder
    // emptied: the comparison is only as good as both snapshots still being there.
    for id in ids {
        info(id)?;
    }
    warnings.extend(exclusion_warnings(&exclusions[0], &exclusions[1]));
    Ok(Loaded {
        earlier,
        later,
        pairs,
        warnings,
        union,
    })
}

impl Loaded {
    /// Compares the loaded trees, from the earlier snapshot to the later. Cancelling
    /// `cancel` stops with [`CompareError::Cancelled`]; `progress` counts the entries
    /// visited and is complete once the difference is.
    pub fn compare(self, cancel: &Cancel, progress: &Progress) -> Result<Difference, CompareError> {
        progress.set_phase(Phase::Comparing);
        // Every folder takes its place among the comparisons, so that a change names the
        // folder its earlier entry came from by the folder's place in the difference. A
        // folder that one snapshot knows nothing of is compared as two empty trees.
        let known = |pair: &LoadedPair| {
            !matches!(pair.located, [Located::Unknown, _] | [_, Located::Unknown])
        };
        let comparisons: Vec<diff::Pair<'_>> = self
            .pairs
            .iter()
            .map(|pair| {
                let [old, new] = match known(pair) {
                    true => [
                        pair.located[0].subtree(&pair.old),
                        pair.located[1].subtree(&pair.new),
                    ],
                    false => [Subtree::whole(&[]); 2],
                };
                let filter = Some(Filter {
                    exclusions: &self.union,
                    branch: pair.branch.clone(),
                });
                let rules = Rules {
                    filter,
                    volumes: pair.volumes,
                };
                diff::Pair { old, new, rules }
            })
            .collect();
        let mut diffs = diff::diff_many(&comparisons, cancel, &progress.done)?;
        drop(comparisons);
        for (pair, diff) in self.pairs.iter().zip(&mut diffs) {
            // Whether the folder was there is unknown: it is inaccessible, not changed.
            let (old, new) = match pair.located {
                [Located::Unknown, Located::Found(b)] => (None, Some(b)),
                [Located::Found(a), Located::Unknown] => (Some(a), None),
                _ => continue,
            };
            diff.inaccessible.push(Inaccessible {
                old,
                new,
                changes_before: 0,
                old_in: None,
            });
        }
        let folders = self
            .pairs
            .into_iter()
            .zip(diffs)
            .map(|(pair, diff)| FolderDifference {
                path: pair.path,
                old: pair.old,
                new: pair.new,
                diff,
            })
            .collect();
        progress.finish();
        Ok(Difference {
            earlier: self.earlier,
            later: self.later,
            folders,
            warnings: self.warnings,
        })
    }
}

/// The exclusions of a snapshot as they applied, for [`Exclusions::new`] with
/// [`RecordedEnvironment`].
fn applied(exclusions: &[SnapshotExclusion]) -> Vec<&str> {
    exclusions
        .iter()
        .filter_map(|e| e.applied.as_deref())
        .collect()
}

/// A folder of the earlier snapshot and one of the later that cover the same place, by
/// their positions.
#[derive(Clone, Copy)]
struct Pair {
    earlier: usize,
    later: usize,
    /// The side whose folder is the wider one and holds the other's; `None` when the
    /// two are the same folder.
    wider: Option<Side>,
}

/// How a folder of one snapshot relates to the folders of the other.
enum Pairing {
    Both(Pair),
    OnlyIn { position: usize, side: Side },
}

/// Pairs the folders of the two snapshots by path, without regard to case: the same
/// folder, or a folder inside a folder of the other snapshot. In the order of the later
/// snapshot's folders, then the earlier snapshot's folders that are left.
fn pair_folders(earlier: &[SnapshotFolder], later: &[SnapshotFolder]) -> Vec<Pairing> {
    let earlier_parts: Vec<Vec<Folded>> = earlier
        .iter()
        .map(|f| path::parts(f.path.as_os_str()))
        .collect();
    // The folders of one snapshot do not nest, so a folder that holds a folder of the
    // other snapshot is the same as none of them and inside none of them; it may hold
    // several, though, so a paired folder stays available.
    let mut taken = vec![false; earlier.len()];
    let mut pairings = Vec::new();
    for (l, folder) in later.iter().enumerate() {
        let parts = path::parts(folder.path.as_os_str());
        let same = (0..earlier.len()).find(|&e| earlier_parts[e] == parts);
        let holds_later = (0..earlier.len()).find(|&e| is_inside(&parts, &earlier_parts[e]));
        let inside_later: Vec<usize> = (0..earlier.len())
            .filter(|&e| is_inside(&earlier_parts[e], &parts))
            .collect();
        let mut pair = |e: usize, wider: Option<Side>| {
            taken[e] = true;
            pairings.push(Pairing::Both(Pair {
                earlier: e,
                later: l,
                wider,
            }));
        };
        if let Some(e) = same {
            pair(e, None);
        } else if let Some(e) = holds_later {
            pair(e, Some(Side::Earlier));
        } else if !inside_later.is_empty() {
            for e in inside_later {
                pair(e, Some(Side::Later));
            }
        } else {
            pairings.push(Pairing::OnlyIn {
                position: l,
                side: Side::Later,
            });
        }
    }
    for (e, &taken) in taken.iter().enumerate() {
        if !taken {
            pairings.push(Pairing::OnlyIn {
                position: e,
                side: Side::Earlier,
            });
        }
    }
    pairings
}

/// Whether the folder whose path has the parts `nested` lies inside the folder whose
/// path has the parts `wider`.
fn is_inside(nested: &[Folded], wider: &[Folded]) -> bool {
    nested.len() > wider.len() && nested.starts_with(wider)
}

/// The loading of one pair of folders: what the two snapshots hold for them, and their
/// trees.
struct FolderLoading<'a> {
    store: &'a Store,
    /// The ids of the earlier and the later snapshot.
    ids: [i64; 2],
    pair: Pair,
    /// The earlier and the later folder.
    folders: [&'a SnapshotFolder; 2],
    /// The parts of the compared folder's path below the wider folder's; none without a
    /// wider folder.
    relative: Vec<OsString>,
    /// The exclusions of both snapshots together, as they applied.
    union: &'a Exclusions,
    /// The exclusions of the earlier and of the later snapshot, as they applied.
    own: [&'a Exclusions; 2],
    cancel: &'a Cancel,
    progress: &'a Progress,
}

impl FolderLoading<'_> {
    /// Loads the pair's trees, adding its warnings; `None` when nothing can be compared.
    fn load(&self, warnings: &mut Vec<Warning>) -> Result<Option<LoadedPair>, CompareError> {
        let compared = self.compared();
        let mut unavailable = false;
        for side in [Side::Earlier, Side::Later] {
            let folder = self.folder(side);
            if let FolderState::Unavailable(reason) = &folder.state {
                warnings.push(Warning::FolderUnavailable {
                    path: folder.path.clone(),
                    side,
                    reason: reason.clone(),
                });
                unavailable = true;
            }
        }
        if unavailable {
            return Ok(None);
        }
        if let Some(side) = self.pair.wider {
            // Left out by the wider snapshot's own exclusions on the way down: that
            // snapshot holds nothing of the folder, so only the other one has it.
            if self.excluded_on_the_way_down(side) {
                warnings.push(Warning::FolderOnlyIn {
                    path: compared.path.clone(),
                    side: side.other(),
                });
                return Ok(None);
            }
            warnings.push(Warning::FolderNarrowed {
                wider: self.folder(side).path.clone(),
                side,
                compared: compared.path.clone(),
            });
        }
        for side in [Side::Earlier, Side::Later] {
            let folder = self.folder(side);
            if folder.state == FolderState::Missing {
                warnings.push(Warning::FolderMissing {
                    path: folder.path.clone(),
                    side,
                });
            }
        }
        let volumes = match self
            .folders
            .map(|f| f.state.scanned().map(|(serial, _)| serial))
        {
            [Some(earlier), Some(later)] => {
                if earlier != later {
                    warnings.push(Warning::DifferentVolumes {
                        path: compared.path.clone(),
                        earlier,
                        later,
                    });
                }
                [earlier, later]
            }
            // A folder that was missing holds nothing, on whichever volume.
            [Some(volume), None] | [None, Some(volume)] => [volume; 2],
            [None, None] => [0; 2],
        };
        let old = self.load_tree(Side::Earlier, self.pair.earlier)?;
        let new = self.load_tree(Side::Later, self.pair.later)?;
        let located = [
            self.locate(Side::Earlier, &old),
            self.locate(Side::Later, &new),
        ];
        // The compared roots are never excluded, as the root of a scan is not: the
        // exclusions apply below them, from where the folder really was.
        let branch = self.union.branch(&self.real_path());
        Ok(Some(LoadedPair {
            path: compared.path.clone(),
            old,
            new,
            located,
            branch,
            volumes,
        }))
    }

    /// Loads the tree of the folder at `position` of the snapshot of `side`, counting
    /// its entries for the progress.
    fn load_tree(&self, side: Side, position: usize) -> Result<Vec<Entry>, CompareError> {
        let id = self.ids[side.index()];
        access(self.cancel, self.progress, || {
            self.store
                .load_folder_counting(id, position, self.cancel, &self.progress.done)
        })
    }

    fn folder(&self, side: Side) -> &SnapshotFolder {
        self.folders[side.index()]
    }

    /// The folder compared: the narrower one, or the later snapshot's.
    fn compared(&self) -> &SnapshotFolder {
        match self.pair.wider {
            Some(Side::Later) => self.folder(Side::Earlier),
            _ => self.folder(Side::Later),
        }
    }

    /// Whether the exclusions of the wider snapshot left the compared folder out on the
    /// way down from the wider folder, so that snapshot holds nothing of it. The scan of
    /// the wider folder applied them there; the scan of the compared folder applied its
    /// own below its root only, never to the root itself, so they do not count here.
    fn excluded_on_the_way_down(&self, wider: Side) -> bool {
        let Some((_, real)) = self.folder(wider).state.scanned() else {
            return false;
        };
        let own = self.own[wider.index()];
        let mut branch = own.branch(real);
        for part in &self.relative {
            match own.child(&branch, part) {
                Some(below) => branch = below,
                None => return true,
            }
        }
        false
    }

    /// Where the compared folder really was: the place path exclusions are matched from,
    /// as the scans matched them. The wider folder's real location with the way down to
    /// the compared folder, else a scanned folder's own, else the path as written.
    fn real_path(&self) -> PathBuf {
        let real =
            |folder: &SnapshotFolder| folder.state.scanned().map(|(_, real)| real.to_path_buf());
        if let Some(side) = self.pair.wider {
            if let Some(mut path) = real(self.folder(side)) {
                path.extend(&self.relative);
                return path;
            }
            let compared = self.compared();
            return real(compared).unwrap_or_else(|| compared.path.clone());
        }
        self.folders
            .iter()
            .rev()
            .find_map(|folder| real(folder))
            .unwrap_or_else(|| self.folders[1].path.clone())
    }

    /// Where the compared folder is in the tree of `side`.
    fn locate(&self, side: Side, entries: &[Entry]) -> Located {
        if entries.is_empty() {
            Located::Absent
        } else if self.pair.wider == Some(side) {
            locate(entries, &self.relative)
        } else {
            Located::Found(0)
        }
    }
}

/// The parts of the path of `nested` below the path of `wider`, as written
/// ([`path::split`]).
fn relative_parts(wider: &Path, nested: &Path) -> Vec<OsString> {
    let depth = path::split(wider.as_os_str()).len();
    path::split(nested.as_os_str()).split_off(depth)
}

/// Where a folder stands in a tree.
#[derive(Clone, Copy, Debug)]
enum Located {
    Found(u32),
    /// The tree holds nothing at its path.
    Absent,
    /// A folder on the way to it could not be listed, or it or a folder on the way is a
    /// link that was not followed: what was there is unknown.
    Unknown,
}

impl Located {
    fn subtree(self, entries: &[Entry]) -> Subtree<'_> {
        match self {
            Located::Found(root) => Subtree { entries, root },
            Located::Absent => Subtree::whole(&[]),
            Located::Unknown => unreachable!("an unknown folder is not compared"),
        }
    }
}

/// Finds the folder at `parts` below the root of `entries`.
fn locate(entries: &[Entry], parts: &[OsString]) -> Located {
    let mut at = 0u32;
    for part in parts {
        let here = &entries[at as usize];
        match here.kind {
            EntryKind::Dir => {}
            EntryKind::File => return Located::Absent,
            EntryKind::Link => return Located::Unknown,
        }
        let child = entries
            .iter()
            .position(|e| e.parent == Some(at) && eq_ignore_case_os(&e.name, part));
        match child {
            Some(i) => at = i as u32,
            None if here.inaccessible => return Located::Unknown,
            None => return Located::Absent,
        }
    }
    // A profile folder that is a junction was scanned through its target, but the wider
    // folder's scan recorded the junction itself and never followed it.
    match entries[at as usize].kind {
        EntryKind::Link => Located::Unknown,
        _ => Located::Found(at),
    }
}

/// Warnings about the exclusions of one snapshot that the other does not have, or that
/// applied differently. Patterns and the way they applied compare without regard to case,
/// as they match.
fn exclusion_warnings(earlier: &[SnapshotExclusion], later: &[SnapshotExclusion]) -> Vec<Warning> {
    let same_applied = |a: &Option<String>, b: &Option<String>| match (a, b) {
        (Some(a), Some(b)) => eq_ignore_case(a, b),
        (None, None) => true,
        _ => false,
    };
    let mut taken = vec![false; later.len()];
    let mut warnings = Vec::new();
    for e in earlier {
        let same_pattern: Vec<usize> = (0..later.len())
            .filter(|&k| !taken[k] && eq_ignore_case(&later[k].pattern, &e.pattern))
            .collect();
        // The one that applied the same way, if any; else any, to report how it differs.
        let pick = same_pattern
            .iter()
            .copied()
            .find(|&k| same_applied(&later[k].applied, &e.applied))
            .or(same_pattern.first().copied());
        match pick {
            Some(k) => {
                taken[k] = true;
                if !same_applied(&later[k].applied, &e.applied) {
                    warnings.push(Warning::ExclusionAppliedDifferently {
                        pattern: e.pattern.clone(),
                        earlier: e.applied.clone(),
                        later: later[k].applied.clone(),
                    });
                }
            }
            None => warnings.push(Warning::ExclusionOnlyIn {
                pattern: e.pattern.clone(),
                side: Side::Earlier,
            }),
        }
    }
    for (k, l) in later.iter().enumerate() {
        if !taken[k] {
            warnings.push(Warning::ExclusionOnlyIn {
                pattern: l.pattern.clone(),
                side: Side::Later,
            });
        }
    }
    warnings
}

#[cfg(test)]
mod tests;
