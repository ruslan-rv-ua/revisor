//! Comparison of trees of file system entries by the rules of TASK-10, with the entries
//! renamed or moved between them (TASK-42). The comparison of two snapshots, which pairs
//! their folders, loads the trees and reports what was left out, is [`crate::compare`].
//!
//! Entries are matched by path. Among the children of two matched folders, those with the
//! very same name are matched first, then the rest without regard to case, as NTFS
//! compares names: a folder made case-sensitive may hold both `a.txt` and `A.txt`. A
//! change is found from metadata alone:
//! - an entry only in the newer tree is added, one only in the older is deleted, with
//!   every entry of its subtree;
//! - an entry whose kind changed (file, folder, link) is deleted and added again;
//! - an entry is modified when its size, creation time, last write time, attributes,
//!   version, link target or NTFS file ID differ. Another file ID at the same path means
//!   the entry was deleted and made again: it is replaced. The archive, normal and cloud
//!   state attributes are ignored, and so is the last write time of folders, which
//!   changes with their contents;
//! - an entry whose name differs in case only, and which is not replaced, is renamed.
//!
//! An entry that would be deleted, or is replaced, and one that would be added are one
//! entry, renamed or moved, when they have the same file ID on the same volume, the same
//! kind and the same creation time (`moves`). It is one change, at its newer path. A
//! folder renamed or moved is compared with what it held before, by the paths below it,
//! so only what changed inside it shows, at the newer paths; and "the same folder" is a
//! folder matched by path or moved, so a file renamed inside a moved folder is renamed,
//! not moved. A folder that took the place of one that moved away is replaced, and holds
//! nothing of what the other held: its contents are added, unless they moved there.
//! Trees compared together ([`diff_many`]) may hold the two entries of a pair in two of
//! the comparisons: an entry moved from one folder of a profile to another.
//!
//! Entries that match an exclusion are left out with their subtrees; the compared roots
//! never are. Inside a folder that moved, what is excluded at either of its places is
//! left out. Entries inaccessible in at least one tree are listed apart and are never
//! added or deleted. Children of a folder inaccessible on one side that are missing on
//! the other side are skipped: it is unknown whether they were there.

mod moves;

use std::cmp::Ordering;
use std::ffi::OsStr;
use std::fmt;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

use crate::exclude::{Branch, Exclusions};
use crate::job::Cancel;
use crate::path::{parts, upcase};
use crate::scan::{Entry, EntryKind, VersionInfo};
use moves::{Moves, Site};

const FILE_ATTRIBUTE_ARCHIVE: u32 = 0x20;
const FILE_ATTRIBUTE_NORMAL: u32 = 0x80;
const FILE_ATTRIBUTE_OFFLINE: u32 = 0x1000;
const FILE_ATTRIBUTE_RECALL_ON_OPEN: u32 = 0x4_0000;
const FILE_ATTRIBUTE_PINNED: u32 = 0x8_0000;
const FILE_ATTRIBUTE_UNPINNED: u32 = 0x10_0000;
const FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS: u32 = 0x40_0000;
/// Attributes whose difference alone is not a change (TASK-10 #10, #11): the archive
/// attribute, which backup programs reset in bulk; the normal attribute, which NTFS
/// reports in place of no attributes at all; and the cloud state attributes — pinned,
/// unpinned, recall on open, recall on data access and offline — which say where the
/// data of a file is kept, not what the file is.
const IGNORED_ATTRIBUTES: u32 = FILE_ATTRIBUTE_ARCHIVE
    | FILE_ATTRIBUTE_NORMAL
    | FILE_ATTRIBUTE_OFFLINE
    | FILE_ATTRIBUTE_RECALL_ON_OPEN
    | FILE_ATTRIBUTE_PINNED
    | FILE_ATTRIBUTE_UNPINNED
    | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS;
/// How many entries are compared between checks for cancellation.
const CANCEL_CHECK: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Deleted,
    Modified,
    /// Renamed or moved (TASK-42): the entry `old` is now the entry `new`, at another
    /// path. [`Fields::NAME`] and [`Fields::FOLDER`] say which part of the path differs,
    /// and the other fields what else does, as for a modified entry.
    Moved,
}

/// Fields of an entry that differ between the snapshots, as bit flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Fields(pub u16);

impl Fields {
    /// The name differs: in any way for a renamed or moved entry, in case only for a
    /// replaced one.
    pub const NAME: Fields = Fields(1);
    pub const SIZE: Fields = Fields(1 << 1);
    pub const CREATED: Fields = Fields(1 << 2);
    pub const MODIFIED: Fields = Fields(1 << 3);
    pub const ATTRIBUTES: Fields = Fields(1 << 4);
    pub const VERSION: Fields = Fields(1 << 5);
    pub const LINK_TARGET: Fields = Fields(1 << 6);
    /// Another NTFS file ID at the same path: the entry was deleted and made again, so it
    /// is replaced, even when nothing else differs.
    pub const FILE_ID: Fields = Fields(1 << 7);
    /// A moved entry is in another folder than it was. A folder matched by path, or one
    /// that moved itself, is the same folder.
    pub const FOLDER: Fields = Fields(1 << 8);

    pub fn contains(self, other: Fields) -> bool {
        self.0 & other.0 == other.0
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl std::ops::BitOr for Fields {
    type Output = Fields;

    fn bitor(self, other: Fields) -> Fields {
        Fields(self.0 | other.0)
    }
}

impl std::ops::BitOrAssign for Fields {
    fn bitor_assign(&mut self, other: Fields) {
        self.0 |= other.0;
    }
}

/// One changed entry. `old` and `new` are indices into the compared trees; an added
/// entry has only `new`, a deleted one only `old`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Change {
    pub kind: ChangeKind,
    pub old: Option<u32>,
    pub new: Option<u32>,
    /// What differs; empty for an added or deleted entry.
    pub fields: Fields,
    /// Among comparisons made together ([`diff_many`]), the one whose older tree holds
    /// `old`, when that is another one than the change's own: the entry, or a folder it
    /// is in, moved between their trees.
    pub old_in: Option<u32>,
}

impl Change {
    /// The path of the changed entry: in the newer tree for an added, renamed or moved
    /// entry, in the older one otherwise. `old` is the older tree that holds the entry
    /// ([`Change::old_in`]).
    pub fn path(&self, old: &[Entry], new: &[Entry]) -> PathBuf {
        match (self.kind, self.old, self.new) {
            (ChangeKind::Moved, _, Some(b)) | (_, None, Some(b)) => path(new, b),
            (_, Some(a), _) => path(old, a),
            (_, None, None) => unreachable!("a change is in at least one tree"),
        }
    }
}

/// An entry inaccessible in at least one tree, by its indices in the trees it is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Inaccessible {
    pub old: Option<u32>,
    pub new: Option<u32>,
    /// How many changes come before the entry in tree order: the entry stands right
    /// before the change at this index, so the changes and the inaccessible entries
    /// merge into one list in tree order (TASK-13 #20). An entry that is also modified,
    /// renamed or moved comes before its change.
    pub changes_before: u32,
    /// The comparison whose older tree holds `old`, as [`Change::old_in`].
    pub old_in: Option<u32>,
}

/// Differences between two trees, in tree order: each folder comes before its contents,
/// and the children of a folder are sorted by name. The two lists are each in tree order;
/// [`Inaccessible::changes_before`] says where the inaccessible entries stand among the
/// changes. A renamed or moved entry stands where it is in the newer tree, and what was
/// deleted inside a moved folder where that folder is now.
#[derive(Clone, Debug, Default)]
pub struct Diff {
    pub changes: Vec<Change>,
    pub inaccessible: Vec<Inaccessible>,
}

#[derive(Debug)]
pub struct Cancelled;

impl fmt::Display for Cancelled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("comparison cancelled")
    }
}

impl std::error::Error for Cancelled {}

/// A tree and the entry in it the comparison starts from: the whole tree from its root,
/// or one folder of it, when the profile folder of one snapshot lies inside a folder of
/// the other.
#[derive(Clone, Copy, Debug)]
pub struct Subtree<'a> {
    pub entries: &'a [Entry],
    /// Index of the entry matched with the other side's; the names of the two are never
    /// compared. Meaningless when `entries` is empty: an empty tree holds nothing.
    pub root: u32,
}

impl<'a> Subtree<'a> {
    /// A whole tree from its root, laid out like [`crate::scan::Scan::entries`].
    pub fn whole(entries: &'a [Entry]) -> Self {
        Subtree { entries, root: 0 }
    }
}

/// Exclusions applied to a comparison: an entry that matches one is left out with its
/// subtree, and the compared roots never are.
#[derive(Clone, Debug)]
pub struct Filter<'a> {
    /// The exclusions of both snapshots as they applied.
    pub exclusions: &'a Exclusions,
    /// Where the compared roots stand against them, as [`Exclusions::branch`] gives it.
    pub branch: Branch,
}

/// What a comparison applies beyond the trees themselves.
#[derive(Clone, Debug)]
pub struct Rules<'a> {
    /// `None` leaves nothing out.
    pub filter: Option<Filter<'a>>,
    /// The volumes the older and the newer tree lie on, by their serial numbers. File IDs
    /// are unique within a volume, so they are compared, and renamed and moved entries
    /// looked for, only between trees of one volume.
    pub volumes: [u64; 2],
}

impl Rules<'_> {
    /// No exclusions, and file IDs compared: two trees of the same volume.
    pub fn plain() -> Self {
        Rules {
            filter: None,
            volumes: [0; 2],
        }
    }
}

/// One comparison among those made together by [`diff_many`]: the two subtrees, whose
/// roots are always matched with each other, and what the comparison applies to them.
#[derive(Clone, Debug)]
pub struct Pair<'a> {
    pub old: Subtree<'a>,
    pub new: Subtree<'a>,
    pub rules: Rules<'a>,
}

/// Compares the subtree `old` with the subtree `new`: their roots are always matched
/// with each other. Cancelling `cancel` stops with [`Cancelled`].
pub fn diff(
    old: Subtree<'_>,
    new: Subtree<'_>,
    rules: &Rules<'_>,
    cancel: &Cancel,
) -> Result<Diff, Cancelled> {
    diff_counting(old, new, rules, cancel, &AtomicU64::new(0))
}

/// [`diff`] that counts the entries visited in `visited`, as [`diff_many`] does.
pub fn diff_counting(
    old: Subtree<'_>,
    new: Subtree<'_>,
    rules: &Rules<'_>,
    cancel: &Cancel,
    visited: &AtomicU64,
) -> Result<Diff, Cancelled> {
    let pair = Pair {
        old,
        new,
        rules: rules.clone(),
    };
    let mut diffs = diff_many(&[pair], cancel, visited)?;
    Ok(diffs.pop().expect("one difference for one comparison"))
}

/// Compares each of `pairs`, and looks for entries renamed or moved within them and
/// between them: a difference for each comparison, in their order. An entry moved from
/// the older tree of one comparison to the newer tree of another is a change of the
/// latter ([`Change::old_in`]). Cancelling `cancel` stops with [`Cancelled`].
///
/// `visited` counts the entries visited, for the progress of a comparison (TASK-11 #8):
/// an entry matched in both trees counts for both, one whose kind changed counts once as
/// deleted and once as added. The count goes up every [`CANCEL_CHECK`] entries and at
/// the end, and is never taken back. Entries the comparison leaves out — excluded, or
/// under a folder inaccessible on one side only — are never visited, so the count may
/// end short of the trees' size. Where renamed or moved entries are found, what holds
/// them is walked again, which is not counted.
pub fn diff_many(
    pairs: &[Pair<'_>],
    cancel: &Cancel,
    visited: &AtomicU64,
) -> Result<Vec<Diff>, Cancelled> {
    let forest = Forest {
        old: pairs.iter().map(|p| Tree::new(p.old.entries)).collect(),
        new: pairs.iter().map(|p| Tree::new(p.new.entries)).collect(),
    };
    let mut moves = Moves::default();
    let mut walked = Vec::with_capacity(pairs.len());
    for at in 0..pairs.len() as u32 {
        walked.push(walk(&forest, pairs, &moves, at, cancel, visited)?);
    }
    loop {
        // The blocks of each comparison that hold an entry of a pair just found.
        let again = moves::find(&forest, pairs, &walked, &mut moves, cancel)?;
        if again.iter().all(Vec::is_empty) {
            return Ok(walked.into_iter().map(|walked| walked.diff).collect());
        }
        for (at, again) in again.iter().enumerate().filter(|(_, a)| !a.is_empty()) {
            let was = std::mem::take(&mut walked[at]);
            walked[at] = walk_again(&forest, pairs, &moves, at as u32, was, again, cancel)?;
        }
    }
}

/// Path of entry `index`: the root path joined with the names down to the entry.
pub fn path(entries: &[Entry], index: u32) -> PathBuf {
    let mut names = Vec::new();
    let mut i = index as usize;
    while let Some(parent) = entries[i].parent {
        names.push(&*entries[i].name);
        i = parent as usize;
    }
    let mut path = PathBuf::from(&*entries[i].name);
    path.extend(names.iter().rev());
    path
}

/// The name of entry `index` alone: a root, which is named by its whole path, by the
/// last part of it, or by the whole path for the root of a drive.
pub fn name(entries: &[Entry], index: u32) -> &OsStr {
    let entry = &entries[index as usize];
    match entry.parent {
        Some(_) => &entry.name,
        None => Path::new(&*entry.name).file_name().unwrap_or(&entry.name),
    }
}

/// The path of the folder entry `index` is in; empty for the root of a drive.
pub fn folder(entries: &[Entry], index: u32) -> PathBuf {
    let entry = &entries[index as usize];
    match entry.parent {
        Some(parent) => path(entries, parent),
        None => Path::new(&*entry.name)
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default(),
    }
}

/// Children of every entry, found from the parent links, so the order of the entries
/// does not matter.
struct Tree<'a> {
    entries: &'a [Entry],
    /// Children of entry `i` are `children[start[i]..start[i + 1]]`.
    start: Vec<u32>,
    children: Vec<u32>,
}

impl<'a> Tree<'a> {
    fn new(entries: &'a [Entry]) -> Self {
        let mut start = vec![0u32; entries.len() + 1];
        for e in entries {
            if let Some(p) = e.parent {
                start[p as usize + 1] += 1;
            }
        }
        for i in 1..start.len() {
            start[i] += start[i - 1];
        }
        let mut next = start.clone();
        let mut children = vec![0u32; entries.len().saturating_sub(1)];
        for (i, e) in entries.iter().enumerate() {
            if let Some(p) = e.parent {
                children[next[p as usize] as usize] = i as u32;
                next[p as usize] += 1;
            }
        }
        Tree {
            entries,
            start,
            children,
        }
    }

    fn children(&self, index: u32) -> &[u32] {
        let i = index as usize;
        &self.children[self.start[i] as usize..self.start[i + 1] as usize]
    }
}

/// The trees of the comparisons made together, by comparison: an entry of one may have
/// moved to another.
struct Forest<'a> {
    old: Vec<Tree<'a>>,
    new: Vec<Tree<'a>>,
}

/// A child of a folder in [`Keys`].
#[derive(Clone, Copy)]
struct Span {
    /// Index of the child in its tree.
    child: u32,
    /// Where its key starts in `units`; its name follows the key.
    start: usize,
    /// The length of the key, and of the name.
    len: usize,
}

/// Names of the children of one folder, ready to be sorted for matching. Kept between
/// folders so their buffers are reused.
#[derive(Default)]
struct Keys {
    spans: Vec<Span>,
    /// The upper-case key of each child followed by its name, in UTF-16.
    units: Vec<u16>,
}

impl Keys {
    fn fill(&mut self, tree: &Tree<'_>, parent: u32) {
        self.spans.clear();
        self.units.clear();
        for &child in tree.children(parent) {
            let name = &tree.entries[child as usize].name;
            let start = self.units.len();
            self.units.extend(name.encode_wide());
            let len = self.units.len() - start;
            self.units.extend_from_within(start..start + len);
            for u in &mut self.units[start..start + len] {
                *u = upcase(*u);
            }
            self.spans.push(Span { child, start, len });
        }
        let units = &self.units;
        self.spans
            .sort_unstable_by(|&a, &b| order_keys(units, a, b));
    }

    fn key(&self, span: Span) -> &[u16] {
        &self.units[span.start..span.start + span.len]
    }

    /// Where the run of children whose key is that of the child at `i` ends.
    fn run_end(&self, i: usize) -> usize {
        let key = self.key(self.spans[i]);
        self.spans[i + 1..]
            .iter()
            .position(|&s| self.key(s) != key)
            .map_or(self.spans.len(), |n| i + 1 + n)
    }
}

/// Orders children by key, and children whose keys are equal — possible only in folders
/// that are case-sensitive — by exact name.
fn order_keys(units: &[u16], a: Span, b: Span) -> Ordering {
    let key = |s: Span| &units[s.start..s.start + s.len];
    let name = |s: Span| &units[s.start + s.len..s.start + 2 * s.len];
    key(a).cmp(key(b)).then_with(|| name(a).cmp(name(b)))
}

/// Matches children of two folders whose names are the same regardless of case, in the
/// order [`Keys`] sorted them: those with the very same name first, then the rest in
/// order, so `a.txt` and `A.txt` of a case-sensitive folder are not confused.
fn match_run(old: &Tree<'_>, olds: &[Span], new: &Tree<'_>, news: &[Span], items: &mut Vec<Item>) {
    let mut taken = vec![false; news.len()];
    let mut unmatched = Vec::new();
    for a in olds {
        let name = &old.entries[a.child as usize].name;
        let same = news
            .iter()
            .enumerate()
            .find(|&(k, b)| !taken[k] && new.entries[b.child as usize].name == *name);
        match same {
            Some((k, b)) => {
                taken[k] = true;
                items.push(Item::Both(a.child, b.child));
            }
            None => unmatched.push(a.child),
        }
    }
    let mut rest = news
        .iter()
        .zip(&taken)
        .filter(|&(_, &taken)| !taken)
        .map(|(b, _)| b.child);
    for a in unmatched {
        items.push(match rest.next() {
            Some(b) => Item::Both(a, b),
            None => Item::Old(a),
        });
    }
    items.extend(rest.map(Item::New));
}

/// An entry waiting to be compared.
#[derive(Clone, Copy)]
enum Item {
    /// The same path in both trees.
    Both(u32, u32),
    /// Only in the older tree.
    Old(u32),
    /// Only in the newer tree.
    New(u32),
}

/// Where a folder stands against the exclusions.
#[derive(Clone, Default)]
struct Place {
    /// By its path in the newer tree, or in the older one where the newer has none.
    branch: Branch,
    /// By the path it had in the older tree, for a folder that moved and for those inside
    /// it: what is excluded at either place is left out.
    old_branch: Option<Branch>,
}

/// The folder whose children a frame holds.
struct Folder {
    place: Place,
    /// The comparison whose older tree the older children are in.
    old_tree: u32,
    /// The older folder that the newer one continues, in `old_tree`: the folder matched
    /// with it by path, or the one that moved to it. A newer entry that an older child of
    /// this folder became has stayed in its folder.
    old_dir: Option<u32>,
    /// The block that ends with the last of the children.
    closes: Option<u32>,
}

/// The items of one folder still to visit.
struct Frame {
    /// Reversed, so `pop` gives the next item in tree order.
    items: Vec<Item>,
    /// Where the folder whose children these are stands against the exclusions; in the
    /// frame of the roots, where the roots themselves stand.
    place: Place,
    of_roots: bool,
    /// As in [`Folder`].
    old_tree: u32,
    old_dir: Option<u32>,
    /// The children of a folder that is in one tree only: all of them are in that tree,
    /// below an entry deleted or added.
    one_sided: bool,
    /// The frame among the contexts of the blocks, once a block of an item of it is noted.
    context: Option<u32>,
    closes: Option<u32>,
}

/// A part of a difference that is walked again when an entry in it turns out to have
/// moved, while the rest of the difference stays as it is: what one item of a frame gave,
/// with all that is below it. Such an item is an entry in one tree only, met among
/// entries of both — deleted or added with all it holds, any of which may have moved — or
/// a folder replaced at its path, which holds nothing of the older one once that one is
/// found elsewhere. The blocks stand in the order of the walk, one inside another where a
/// folder inside a block has both sides again.
#[derive(Clone, Copy)]
struct Block {
    item: Item,
    /// The frame the item was met in, among [`Walked::contexts`].
    context: u32,
    /// Where what the item gave starts and ends among the changes, and among the
    /// inaccessible entries.
    changes: (u32, u32),
    inaccessible: (u32, u32),
}

/// What a frame was when an item of it was met, to walk the item again.
struct Context {
    place: Place,
    of_roots: bool,
    old_tree: u32,
    old_dir: Option<u32>,
}

/// What the walk of a comparison gives: its difference, and the blocks of it.
#[derive(Default)]
struct Walked {
    diff: Diff,
    blocks: Vec<Block>,
    /// The frames of the blocks; one walked again keeps its frame, so they only grow.
    contexts: Vec<Context>,
}

/// The walk of one comparison: every frame is in its newer tree, and in an older tree of
/// any of the comparisons made together.
struct Walk<'w, 'a> {
    forest: &'w Forest<'a>,
    pairs: &'w [Pair<'a>],
    /// The entries renamed or moved, as found so far.
    moves: &'w Moves,
    /// The comparison walked.
    at: u32,
    out: Walked,
    /// A stack of frames, one per folder being walked.
    frames: Vec<Frame>,
    old_keys: Keys,
    new_keys: Keys,
    cancel: &'w Cancel,
    /// Counts the entries visited, on the first walk.
    visited: Option<&'w AtomicU64>,
    /// How many items the walk has visited, and how many entries it has yet to count.
    seen: usize,
    counted: u64,
}

/// Walks the comparison `at` of `pairs` for the first time, when no entry is known to
/// have moved, counting the entries visited in `visited`.
fn walk(
    forest: &Forest<'_>,
    pairs: &[Pair<'_>],
    moves: &Moves,
    at: u32,
    cancel: &Cancel,
    visited: &AtomicU64,
) -> Result<Walked, Cancelled> {
    let pair = &pairs[at as usize];
    let roots = match (pair.old.entries.is_empty(), pair.new.entries.is_empty()) {
        (false, false) => vec![Item::Both(pair.old.root, pair.new.root)],
        (false, true) => vec![Item::Old(pair.old.root)],
        (true, false) => vec![Item::New(pair.new.root)],
        (true, true) => Vec::new(),
    };
    let branch = pair
        .rules
        .filter
        .as_ref()
        .map(|f| f.branch.clone())
        .unwrap_or_default();
    let mut walk = Walk::new(forest, pairs, moves, at, cancel, Some(visited));
    walk.run(Frame {
        items: roots,
        place: Place {
            branch,
            old_branch: None,
        },
        of_roots: true,
        old_tree: at,
        old_dir: None,
        one_sided: false,
        context: None,
        closes: None,
    })?;
    Ok(walk.out)
}

/// Walks again the blocks of `was`, the walk of the comparison `at` so far, that `again`
/// marks, with the renamed and moved entries found since; the rest of the difference
/// stays as it was.
fn walk_again(
    forest: &Forest<'_>,
    pairs: &[Pair<'_>],
    moves: &Moves,
    at: u32,
    was: Walked,
    again: &[bool],
    cancel: &Cancel,
) -> Result<Walked, Cancelled> {
    let Walked {
        diff: old,
        blocks,
        contexts,
    } = was;
    let mut walk = Walk::new(forest, pairs, moves, at, cancel, None);
    walk.out.contexts = contexts;
    walk.out.diff.changes.reserve(old.changes.len());
    walk.out.diff.inaccessible.reserve(old.inaccessible.len());
    // How far the old difference is taken over: so many changes and inaccessible entries.
    let mut taken = (0u32, 0u32);
    // Takes over the old difference up to `to`. The inaccessible entries say how many
    // changes stand before them, which is as many more as the blocks walked again added.
    let take = |walk: &mut Walk<'_, '_>, taken: &mut (u32, u32), to: (u32, u32)| {
        let (changes, inaccessible) = (
            taken.0 as usize..to.0 as usize,
            taken.1 as usize..to.1 as usize,
        );
        let added = walk.out.diff.changes.len() as i64 - i64::from(taken.0);
        let entries = old.inaccessible[inaccessible]
            .iter()
            .map(|entry| Inaccessible {
                changes_before: (i64::from(entry.changes_before) + added) as u32,
                ..*entry
            });
        walk.out.diff.inaccessible.extend(entries);
        walk.out
            .diff
            .changes
            .extend_from_slice(&old.changes[changes]);
        *taken = to;
    };
    let ends = |block: &Block| (block.changes.1, block.inaccessible.1);
    // The blocks kept that the walk is inside of, by their places in the new difference,
    // with where each ends in the old one.
    let mut inside: Vec<(usize, (u32, u32))> = Vec::new();
    let close = |walk: &mut Walk<'_, '_>, kept: usize| {
        let (changes, inaccessible) = walk.end();
        walk.out.blocks[kept].changes.1 = changes;
        walk.out.blocks[kept].inaccessible.1 = inaccessible;
    };
    for (block, &again) in blocks.iter().zip(again) {
        let starts = (block.changes.0, block.inaccessible.0);
        // A block inside one walked again was walked with it.
        if starts.0 < taken.0 || starts.1 < taken.1 {
            continue;
        }
        // The blocks kept that end before this one starts.
        while let Some(&(kept, end)) = inside.last()
            && end.0 <= starts.0
            && end.1 <= starts.1
        {
            take(&mut walk, &mut taken, end);
            close(&mut walk, kept);
            inside.pop();
        }
        take(&mut walk, &mut taken, starts);
        if again {
            let context = &walk.out.contexts[block.context as usize];
            let frame = Frame {
                items: vec![block.item],
                place: context.place.clone(),
                of_roots: context.of_roots,
                old_tree: context.old_tree,
                old_dir: context.old_dir,
                one_sided: false,
                context: Some(block.context),
                closes: None,
            };
            walk.run(frame)?;
            taken = ends(block);
        } else {
            let (changes, inaccessible) = walk.end();
            inside.push((walk.out.blocks.len(), ends(block)));
            walk.out.blocks.push(Block {
                changes: (changes, changes),
                inaccessible: (inaccessible, inaccessible),
                ..*block
            });
        }
    }
    while let Some((kept, end)) = inside.pop() {
        take(&mut walk, &mut taken, end);
        close(&mut walk, kept);
    }
    let end = (old.changes.len() as u32, old.inaccessible.len() as u32);
    take(&mut walk, &mut taken, end);
    Ok(walk.out)
}

impl<'w, 'a> Walk<'w, 'a> {
    fn new(
        forest: &'w Forest<'a>,
        pairs: &'w [Pair<'a>],
        moves: &'w Moves,
        at: u32,
        cancel: &'w Cancel,
        visited: Option<&'w AtomicU64>,
    ) -> Self {
        Walk {
            forest,
            pairs,
            moves,
            at,
            out: Walked::default(),
            frames: Vec::new(),
            old_keys: Keys::default(),
            new_keys: Keys::default(),
            cancel,
            visited,
            seen: 0,
            counted: 0,
        }
    }

    /// Visits the items of `frame` and all that is below them, in tree order.
    fn run(&mut self, frame: Frame) -> Result<(), Cancelled> {
        let base = self.frames.len();
        self.frames.push(frame);
        while self.frames.len() > base {
            let frame = self
                .frames
                .last_mut()
                .expect("there are frames above the base");
            let Some(item) = frame.items.pop() else {
                let closes = frame.closes;
                self.frames.pop();
                if let Some(block) = closes {
                    self.close(block);
                }
                continue;
            };
            let old_tree = frame.old_tree;
            if self.seen.is_multiple_of(CANCEL_CHECK) {
                if self.cancel.is_cancelled() {
                    return Err(Cancelled);
                }
                self.count();
            }
            self.seen += 1;
            // A pair whose kinds differ is visited again as a deleted and an added entry.
            self.counted += match item {
                Item::Both(a, b) => {
                    let same_kind = self.old_entry((old_tree, a)).kind == self.new_entry(b).kind;
                    if same_kind { 2 } else { 0 }
                }
                Item::Old(_) | Item::New(_) => 1,
            };
            self.visit(item);
        }
        self.count();
        Ok(())
    }

    /// Adds the entries visited since the last count to the count of the comparison.
    fn count(&mut self) {
        if let Some(visited) = self.visited {
            visited.fetch_add(self.counted, Relaxed);
        }
        self.counted = 0;
    }

    fn top(&self) -> &Frame {
        self.frames.last().expect("an item comes from a frame")
    }

    fn old_entry(&self, (tree, index): Site) -> &'a Entry {
        &self.forest.old[tree as usize].entries[index as usize]
    }

    fn new_entry(&self, index: u32) -> &'a Entry {
        &self.forest.new[self.at as usize].entries[index as usize]
    }

    /// How many changes and inaccessible entries the difference has so far.
    fn end(&self) -> (u32, u32) {
        (
            self.out.diff.changes.len() as u32,
            self.out.diff.inaccessible.len() as u32,
        )
    }

    /// Starts a block: what `item`, of the frame on top, gives from here on.
    fn open(&mut self, item: Item) -> u32 {
        let frame = self.frames.last_mut().expect("an item comes from a frame");
        let contexts = &mut self.out.contexts;
        let context = *frame.context.get_or_insert_with(|| {
            contexts.push(Context {
                place: frame.place.clone(),
                of_roots: frame.of_roots,
                old_tree: frame.old_tree,
                old_dir: frame.old_dir,
            });
            (contexts.len() - 1) as u32
        });
        let (changes, inaccessible) = self.end();
        self.out.blocks.push(Block {
            item,
            context,
            changes: (changes, changes),
            inaccessible: (inaccessible, inaccessible),
        });
        (self.out.blocks.len() - 1) as u32
    }

    /// Ends the block `block` here.
    fn close(&mut self, block: u32) {
        let (changes, inaccessible) = self.end();
        let block = &mut self.out.blocks[block as usize];
        block.changes.1 = changes;
        block.inaccessible.1 = inaccessible;
    }

    fn visit(&mut self, item: Item) {
        match item {
            Item::Both(a, b) => self.visit_both(a, b),
            Item::Old(a) => self.visit_old(a),
            Item::New(b) => self.visit_new(b),
        }
    }

    fn visit_old(&mut self, a: u32) {
        let frame = self.top();
        let (old_tree, among_both) = (frame.old_tree, !frame.one_sided);
        // An entry that moved is compared where it is now.
        if self.moves.is_moved((old_tree, a)) {
            return;
        }
        // Deleted with all it holds, any of which may turn out to have moved.
        let block = among_both.then(|| self.open(Item::Old(a)));
        let entry = self.old_entry((old_tree, a));
        if entry.inaccessible {
            self.inaccessible(Some((old_tree, a)), None);
        } else {
            self.change(
                ChangeKind::Deleted,
                Some((old_tree, a)),
                None,
                Fields::default(),
            );
            if entry.kind == EntryKind::Dir {
                let folder = Folder {
                    place: self.place_of(&entry.name),
                    old_tree,
                    old_dir: None,
                    closes: block,
                };
                return self.push_children(Some(a), None, false, folder);
            }
        }
        if let Some(block) = block {
            self.close(block);
        }
    }

    fn visit_new(&mut self, b: u32) {
        if let Some(from) = self.moves.old_of((self.at, b)) {
            return self.visit_moved(from, b);
        }
        let frame = self.top();
        let (old_tree, among_both) = (frame.old_tree, !frame.one_sided);
        // Added with all it holds, any of which may turn out to have moved here.
        let block = among_both.then(|| self.open(Item::New(b)));
        let entry = self.new_entry(b);
        if entry.inaccessible {
            self.inaccessible(None, Some(b));
        } else {
            self.change(ChangeKind::Added, None, Some(b), Fields::default());
            if entry.kind == EntryKind::Dir {
                let folder = Folder {
                    place: self.place_of(&entry.name),
                    old_tree,
                    old_dir: None,
                    closes: block,
                };
                return self.push_children(None, Some(b), false, folder);
            }
        }
        if let Some(block) = block {
            self.close(block);
        }
    }

    /// The newer entry `b` is the older entry `from`, renamed or moved: one change, and
    /// for a folder its contents compared with those it had.
    fn visit_moved(&mut self, from: Site, b: u32) {
        let (old, new) = (self.old_entry(from), self.new_entry(b));
        let unknown = old.inaccessible || new.inaccessible;
        // The two have one file ID, so the entry is not replaced.
        let mut fields = changed_fields(old, new, unknown, false);
        let (old_tree, new_tree) = (
            self.forest.old[from.0 as usize].entries,
            self.forest.new[self.at as usize].entries,
        );
        if name(old_tree, from.1) != name(new_tree, b) {
            fields |= Fields::NAME;
        }
        if !self.in_the_same_folder(from, b) {
            fields |= Fields::FOLDER;
        }
        if unknown {
            self.inaccessible(Some(from), Some(b));
        }
        if fields.contains(Fields::NAME) || fields.contains(Fields::FOLDER) {
            self.change(ChangeKind::Moved, Some(from), Some(b), fields);
        } else if !fields.is_empty() {
            // The same name in the same folder: the entry came back to its path, into a
            // folder that took the place of its own.
            self.change(ChangeKind::Modified, Some(from), Some(b), fields);
        }
        if old.kind == EntryKind::Dir {
            let folder = Folder {
                place: Place {
                    branch: self.place_of(&new.name).branch,
                    old_branch: self.old_branch(from),
                },
                old_tree: from.0,
                old_dir: Some(from.1),
                closes: None,
            };
            self.push_children(Some(from.1), Some(b), unknown, folder);
        }
    }

    fn visit_both(&mut self, a: u32, b: u32) {
        let frame = self.top();
        let (old_tree, of_roots, old_dir) = (frame.old_tree, frame.of_roots, frame.old_dir);
        let (old, new) = (self.old_entry((old_tree, a)), self.new_entry(b));
        // A newer entry that moved here from elsewhere is at a path of its own: the
        // older entry at this path is not the one it continues.
        let moved_here = self
            .moves
            .old_of((self.at, b))
            .is_some_and(|from| from != (old_tree, a));
        if old.kind != new.kind || moved_here {
            // Visited in this order: deleted first, then added.
            let place = frame.place.clone();
            self.frames.push(Frame {
                items: vec![Item::New(b), Item::Old(a)],
                place,
                of_roots,
                old_tree,
                old_dir,
                one_sided: false,
                context: None,
                closes: None,
            });
            return;
        }
        let unknown = old.inaccessible || new.inaccessible;
        let file_ids = self.pairs[old_tree as usize].rules.volumes[0]
            == self.pairs[self.at as usize].rules.volumes[1];
        let mut fields = changed_fields(old, new, unknown, file_ids);
        // The names of the compared roots are not compared: a root is named by its
        // whole path.
        if !of_roots && old.name != new.name {
            fields |= Fields::NAME;
        }
        let replaced = fields.contains(Fields::FILE_ID);
        let is_dir = old.kind == EntryKind::Dir;
        // A folder replaced holds nothing of the older one once that one is found
        // elsewhere.
        let block = (is_dir && replaced).then(|| self.open(Item::Both(a, b)));
        if unknown {
            self.inaccessible(Some((old_tree, a)), Some(b));
        }
        if fields.contains(Fields::NAME) && !replaced {
            // The same entry under a name of another case: renamed.
            self.change(ChangeKind::Moved, Some((old_tree, a)), Some(b), fields);
        } else if !fields.is_empty() {
            self.change(ChangeKind::Modified, Some((old_tree, a)), Some(b), fields);
        }
        if !is_dir {
            return;
        }
        let folder = Folder {
            place: self.place_of(&old.name),
            old_tree,
            old_dir: Some(a),
            closes: block,
        };
        if !(replaced && self.moves.is_moved((old_tree, a))) {
            self.push_children(Some(a), Some(b), unknown, folder);
        } else if !unknown {
            // The older folder moved away with what it held, which is compared where the
            // folder is now: this one holds nothing of it.
            self.push_children(None, Some(b), false, folder);
        } else if let Some(block) = block {
            self.close(block);
        }
    }

    fn change(&mut self, kind: ChangeKind, old: Option<Site>, new: Option<u32>, fields: Fields) {
        let old_in = self.old_in(old);
        self.out.diff.changes.push(Change {
            kind,
            old: old.map(|(_, index)| index),
            new,
            fields,
            old_in,
        });
    }

    fn inaccessible(&mut self, old: Option<Site>, new: Option<u32>) {
        let changes_before = self.out.diff.changes.len() as u32;
        let old_in = self.old_in(old);
        self.out.diff.inaccessible.push(Inaccessible {
            old: old.map(|(_, index)| index),
            new,
            changes_before,
            old_in,
        });
    }

    /// The comparison whose older tree holds `old`, when it is not the one walked.
    fn old_in(&self, old: Option<Site>) -> Option<u32> {
        old.map(|(tree, _)| tree).filter(|&tree| tree != self.at)
    }

    /// Whether the newer entry `b` is in the folder the older entry `from` was in: the
    /// folder matched with that one by path, or the one it moved to. The roots of the
    /// comparisons have no folders among the entries, so theirs are told by their paths.
    fn in_the_same_folder(&self, from: Site, b: u32) -> bool {
        let frame = self.top();
        match self.old_entry(from).parent {
            Some(parent) if !frame.of_roots => {
                frame.old_tree == from.0 && frame.old_dir == Some(parent)
            }
            _ => {
                let was = folder(self.forest.old[from.0 as usize].entries, from.1);
                let is = folder(self.forest.new[self.at as usize].entries, b);
                parts(was.as_os_str()) == parts(is.as_os_str())
            }
        }
    }

    /// Where a folder named `name`, an item of the frame on top, stands against the
    /// exclusions.
    fn place_of(&self, name: &OsStr) -> Place {
        let frame = self.top();
        if frame.of_roots {
            return frame.place.clone();
        }
        let below = |comparison: u32, branch: &Branch| match &self.pairs[comparison as usize]
            .rules
            .filter
        {
            Some(filter) => filter
                .exclusions
                .child(branch, name)
                .expect("an excluded child is never queued"),
            None => branch.clone(),
        };
        Place {
            branch: below(self.at, &frame.place.branch),
            old_branch: frame
                .place
                .old_branch
                .as_ref()
                .map(|branch| below(frame.old_tree, branch)),
        }
    }

    /// Where the older folder `from` stood against the exclusions of its comparison, at
    /// its own path; `None` when that comparison leaves nothing out.
    fn old_branch(&self, (tree, index): Site) -> Option<Branch> {
        let pair = &self.pairs[tree as usize];
        let filter = pair.rules.filter.as_ref()?;
        let entries = pair.old.entries;
        let mut names = Vec::new();
        let mut at = index;
        while at != pair.old.root {
            names.push(&*entries[at as usize].name);
            at = entries[at as usize].parent?;
        }
        names
            .iter()
            .rev()
            .try_fold(filter.branch.clone(), |branch, name| {
                filter.exclusions.child(&branch, name)
            })
    }

    /// Matches the children of a folder by name and queues them, except the excluded ones.
    /// With `skip_unmatched`, children found on one side only are dropped.
    fn push_children(
        &mut self,
        old: Option<u32>,
        new: Option<u32>,
        skip_unmatched: bool,
        folder: Folder,
    ) {
        let (forest, pairs) = (self.forest, self.pairs);
        let old_tree = &forest.old[folder.old_tree as usize];
        let new_tree = &forest.new[self.at as usize];
        match old {
            Some(a) => self.old_keys.fill(old_tree, a),
            None => self.old_keys.spans.clear(),
        }
        match new {
            Some(b) => self.new_keys.fill(new_tree, b),
            None => self.new_keys.spans.clear(),
        }
        let (ok, nk) = (&self.old_keys, &self.new_keys);
        let mut items = Vec::with_capacity(ok.spans.len().max(nk.spans.len()));
        let (mut i, mut j) = (0, 0);
        while i < ok.spans.len() || j < nk.spans.len() {
            let order = match (ok.spans.get(i), nk.spans.get(j)) {
                (Some(&x), Some(&y)) => ok.key(x).cmp(nk.key(y)),
                (Some(_), None) => Ordering::Less,
                _ => Ordering::Greater,
            };
            match order {
                Ordering::Equal => {
                    // Every child with this key on each side: more than one only in a
                    // folder that is case-sensitive.
                    let (i2, j2) = (ok.run_end(i), nk.run_end(j));
                    if i2 - i == 1 && j2 - j == 1 {
                        items.push(Item::Both(ok.spans[i].child, nk.spans[j].child));
                    } else {
                        match_run(
                            old_tree,
                            &ok.spans[i..i2],
                            new_tree,
                            &nk.spans[j..j2],
                            &mut items,
                        );
                    }
                    (i, j) = (i2, j2);
                }
                Ordering::Less => {
                    items.push(Item::Old(ok.spans[i].child));
                    i += 1;
                }
                Ordering::Greater => {
                    items.push(Item::New(nk.spans[j].child));
                    j += 1;
                }
            }
        }
        if skip_unmatched {
            items.retain(|item| matches!(item, Item::Both(..)));
        }
        // Checked after matching, once per path rather than once per side: matching is
        // cheap next to a check against the dozens of exclusions a profile has.
        let name_of = |item: &Item| match *item {
            Item::Both(a, _) | Item::Old(a) => &old_tree.entries[a as usize].name,
            Item::New(b) => &new_tree.entries[b as usize].name,
        };
        if let Some(filter) = &pairs[self.at as usize].rules.filter {
            let branch = &folder.place.branch;
            items.retain(|item| filter.exclusions.child(branch, name_of(item)).is_some());
        }
        if let (Some(branch), Some(filter)) = (
            &folder.place.old_branch,
            &pairs[folder.old_tree as usize].rules.filter,
        ) {
            items.retain(|item| filter.exclusions.child(branch, name_of(item)).is_some());
        }
        items.reverse();
        self.frames.push(Frame {
            items,
            place: folder.place,
            of_roots: false,
            old_tree: folder.old_tree,
            old_dir: folder.old_dir,
            one_sided: old.is_none() || new.is_none(),
            context: None,
            closes: folder.closes,
        });
    }
}

/// Fields in which two entries of the same kind differ although the comparison does not
/// count it (TASK-10 #6, #10, #11; TASK-14 #5): attributes that differ in the ignored ones
/// only — archive, normal and the cloud states —, the last write time of a folder, a
/// version only one snapshot read, and the version and link target of an entry
/// inaccessible in either tree, `unknown`, where they may be missing. The name and the
/// file ID are left out: a root is named by its whole path, and another file ID makes the
/// entry replaced.
pub fn ignored_fields(old: &Entry, new: &Entry, unknown: bool) -> Fields {
    let mut differ = Fields::default();
    for (field, differs) in [
        (Fields::SIZE, old.size != new.size),
        (Fields::CREATED, old.created != new.created),
        (Fields::MODIFIED, old.modified != new.modified),
        (Fields::ATTRIBUTES, old.attributes != new.attributes),
        (Fields::VERSION, old.version != new.version),
        (Fields::LINK_TARGET, old.link_target != new.link_target),
    ] {
        if differs {
            differ |= field;
        }
    }
    let counted = changed_fields(old, new, unknown, false);
    Fields(differ.0 & !counted.0)
}

/// Fields that differ between two entries of the same kind, the name apart: whoever
/// compares them knows whether the names count. `unknown` is set when either entry is
/// inaccessible, so its version or link target may be missing.
fn changed_fields(old: &Entry, new: &Entry, unknown: bool, file_ids: bool) -> Fields {
    let mut fields = Fields::default();
    let is_dir = old.kind == EntryKind::Dir;
    if old.size != new.size {
        fields |= Fields::SIZE;
    }
    if old.created != new.created {
        fields |= Fields::CREATED;
    }
    if !is_dir && old.modified != new.modified {
        fields |= Fields::MODIFIED;
    }
    if (old.attributes ^ new.attributes) & !IGNORED_ATTRIBUTES != 0 {
        fields |= Fields::ATTRIBUTES;
    }
    // Versions count only when both snapshots read them (TASK-8 #13, TASK-10 #6).
    let versions_read = old.version != VersionInfo::NotRead && new.version != VersionInfo::NotRead;
    if !unknown && versions_read && old.version != new.version {
        fields |= Fields::VERSION;
    }
    if !unknown && old.link_target != new.link_target {
        fields |= Fields::LINK_TARGET;
    }
    if file_ids && old.file_id != new.file_id {
        fields |= Fields::FILE_ID;
    }
    fields
}

#[cfg(test)]
mod tests;
