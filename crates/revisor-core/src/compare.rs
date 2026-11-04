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

use std::ffi::OsString;
use std::fmt;
use std::os::windows::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::diff::{self, Diff, Filter, Inaccessible, Rules, Subtree};
use crate::exclude::{Exclusions, RecordedEnvironment};
use crate::folders;
use crate::names::{eq_ignore_case, eq_ignore_case_os};
use crate::scan::{Entry, EntryKind};
use crate::store::{
    FolderState, SnapshotExclusion, SnapshotFolder, SnapshotInfo, Store, StoreError, Unavailable,
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
    /// as usual, but file IDs are not, and a mass of changes may come from that.
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
    /// Changes and inaccessible entries, by indices into `old` and `new`.
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

/// Compares the snapshots `a` and `b` of the profile in `store`, the earlier with the
/// later, loading their trees from the database. Setting `cancel` stops with
/// [`CompareError::Cancelled`], while the trees are being loaded too.
pub fn compare(
    store: &Store,
    a: i64,
    b: i64,
    cancel: &AtomicBool,
) -> Result<Difference, CompareError> {
    let info = |id: i64| -> Result<SnapshotInfo, CompareError> {
        store.snapshot(id)?.ok_or(CompareError::NoSuchSnapshot(id))
    };
    // Ids are given out in the order the snapshots were taken, which the clock may not
    // follow.
    let (earlier, later) = (info(a.min(b))?, info(a.max(b))?);
    let ids = [earlier.id, later.id];
    let folders = [
        store.snapshot_folders(ids[0])?,
        store.snapshot_folders(ids[1])?,
    ];
    let exclusions = [
        store.snapshot_exclusions(ids[0])?,
        store.snapshot_exclusions(ids[1])?,
    ];
    // The exclusions of each snapshot as they applied, and both together: what either
    // snapshot left out, the comparison leaves out of both.
    let own = exclusions
        .each_ref()
        .map(|e| Exclusions::new(&applied(e), &RecordedEnvironment));
    let both: Vec<&str> = exclusions.iter().flat_map(|e| applied(e)).collect();
    let union = Exclusions::new(&both, &RecordedEnvironment);

    let mut warnings = Vec::new();
    let mut compared = Vec::new();
    for pairing in pair_folders(&folders[0], &folders[1]) {
        if cancel.load(Ordering::Relaxed) {
            return Err(CompareError::Cancelled);
        }
        match pairing {
            Pairing::OnlyIn { position, side } => warnings.push(Warning::FolderOnlyIn {
                path: folders[side.index()][position].path.clone(),
                side,
            }),
            Pairing::Both(pair) => {
                let paired = [&folders[0][pair.earlier], &folders[1][pair.later]];
                let comparison = FolderComparison {
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
                };
                if let Some(difference) = comparison.run(&mut warnings)? {
                    compared.push(difference);
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
    Ok(Difference {
        earlier,
        later,
        folders: compared,
        warnings,
    })
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
    let earlier_parts: Vec<Vec<Vec<u16>>> =
        earlier.iter().map(|f| folders::parts(&f.path)).collect();
    // The folders of one snapshot do not nest, so a folder that holds a folder of the
    // other snapshot is the same as none of them and inside none of them; it may hold
    // several, though, so a paired folder stays available.
    let mut taken = vec![false; earlier.len()];
    let mut pairings = Vec::new();
    for (l, folder) in later.iter().enumerate() {
        let parts = folders::parts(&folder.path);
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
fn is_inside(nested: &[Vec<u16>], wider: &[Vec<u16>]) -> bool {
    nested.len() > wider.len() && nested.starts_with(wider)
}

/// The comparison of one pair of folders.
struct FolderComparison<'a> {
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
    cancel: &'a AtomicBool,
}

impl FolderComparison<'_> {
    /// Compares the pair, adding its warnings; `None` when nothing could be compared.
    fn run(&self, warnings: &mut Vec<Warning>) -> Result<Option<FolderDifference>, CompareError> {
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
        let file_ids = match self
            .folders
            .map(|f| f.state.scanned().map(|(serial, _)| serial))
        {
            [Some(earlier), Some(later)] if earlier != later => {
                warnings.push(Warning::DifferentVolumes {
                    path: compared.path.clone(),
                    earlier,
                    later,
                });
                false
            }
            _ => true,
        };
        let old = self
            .store
            .load_folder(self.ids[0], self.pair.earlier, self.cancel)?;
        let new = self
            .store
            .load_folder(self.ids[1], self.pair.later, self.cancel)?;
        // The compared roots are never excluded, as the root of a scan is not: the
        // exclusions apply below them, from where the folder really was.
        let rules = Rules {
            filter: Some(Filter {
                exclusions: self.union,
                branch: self.union.branch(&self.real_path()),
            }),
            file_ids,
        };
        let diff = match [
            self.locate(Side::Earlier, &old),
            self.locate(Side::Later, &new),
        ] {
            // Whether the folder was there is unknown: it is inaccessible, not changed.
            [Located::Unknown, Located::Found(b)] => Diff {
                changes: Vec::new(),
                inaccessible: vec![Inaccessible {
                    old: None,
                    new: Some(b),
                }],
            },
            [Located::Found(a), Located::Unknown] => Diff {
                changes: Vec::new(),
                inaccessible: vec![Inaccessible {
                    old: Some(a),
                    new: None,
                }],
            },
            [Located::Unknown, _] | [_, Located::Unknown] => Diff::default(),
            [o, n] => diff::diff(o.subtree(&old), n.subtree(&new), &rules, self.cancel)?,
        };
        Ok(Some(FolderDifference {
            path: compared.path.clone(),
            old,
            new,
            diff,
        }))
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

/// The parts of the path of `nested` below the path of `wider`, upper-cased, as the parts
/// of a path compare.
fn relative_parts(wider: &Path, nested: &Path) -> Vec<OsString> {
    let depth = folders::parts(wider).len();
    folders::parts(nested)[depth..]
        .iter()
        .map(|part| OsString::from_wide(part))
        .collect()
}

/// Where a folder stands in a tree.
#[derive(Clone, Copy)]
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
