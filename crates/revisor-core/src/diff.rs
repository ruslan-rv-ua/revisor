//! Comparison of two trees of file system entries by the rules of TASK-10. The
//! comparison of two snapshots, which pairs their folders, loads the trees and reports
//! what was left out, is [`crate::compare`].
//!
//! Entries are matched by path. Among the children of two matched folders, those with the
//! very same name are matched first, then the rest without regard to case, as NTFS
//! compares names: a folder made case-sensitive may hold both `a.txt` and `A.txt`. A
//! change is found from metadata alone:
//! - an entry only in the newer tree is added, one only in the older is deleted, with
//!   every entry of its subtree;
//! - an entry whose kind changed (file, folder, link) is deleted and added again;
//! - an entry is modified when its name case, size, creation time, last write time,
//!   attributes, version, link target or NTFS file ID differ. Another file ID at the same
//!   path means the entry was deleted and made again: it is replaced. The archive, normal
//!   and cloud state attributes are ignored, and so is the last write time of folders,
//!   which changes with their contents.
//!
//! Entries that match an exclusion are left out with their subtrees; the compared roots
//! never are. Entries inaccessible in at least one tree are listed apart and are never
//! added or deleted. Children of a folder inaccessible on one side that are missing on
//! the other side are skipped: it is unknown whether they were there.

use std::cmp::Ordering;
use std::ffi::OsStr;
use std::fmt;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

use crate::exclude::{Branch, Exclusions};
use crate::job::Cancel;
use crate::names::upcase;
use crate::scan::{Entry, EntryKind, VersionInfo};

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
}

/// Fields of an entry that differ between the snapshots, as bit flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Fields(pub u8);

impl Fields {
    /// The name differs in case only.
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
    /// What differs; empty unless the entry is modified.
    pub fields: Fields,
}

impl Change {
    /// The path of the changed entry: in the newer tree for an added entry, in the older
    /// one otherwise.
    pub fn path(&self, old: &[Entry], new: &[Entry]) -> PathBuf {
        match (self.old, self.new) {
            (Some(a), _) => path(old, a),
            (None, Some(b)) => path(new, b),
            (None, None) => unreachable!("a change is in at least one tree"),
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
    /// merge into one list in tree order (TASK-13 #20). An entry that is also modified
    /// comes before its change.
    pub changes_before: u32,
}

/// Differences between two trees, in tree order: each folder comes before its contents,
/// and the children of a folder are sorted by name. The two lists are each in tree order;
/// [`Inaccessible::changes_before`] says where the inaccessible entries stand among the
/// changes.
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
    /// Whether file IDs are compared: only when both trees lie on the same volume, where
    /// the IDs are unique.
    pub file_ids: bool,
}

impl Rules<'_> {
    /// No exclusions, and file IDs compared: two trees of the same volume.
    pub fn plain() -> Self {
        Rules {
            filter: None,
            file_ids: true,
        }
    }
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

/// [`diff`] that counts the entries visited in `visited`, for the progress of a
/// comparison (TASK-11 #8): an entry matched in both trees counts for both, one whose
/// kind changed counts once as deleted and once as added. The count goes up every
/// [`CANCEL_CHECK`] entries and at the end, and is never taken back. Entries the
/// comparison leaves out — excluded, or under a folder inaccessible on one side only —
/// are never visited, so the count may end short of the trees' size.
pub fn diff_counting(
    old: Subtree<'_>,
    new: Subtree<'_>,
    rules: &Rules<'_>,
    cancel: &Cancel,
    visited: &AtomicU64,
) -> Result<Diff, Cancelled> {
    let roots = match (old.entries.is_empty(), new.entries.is_empty()) {
        (false, false) => vec![Item::Both(old.root, new.root)],
        (false, true) => vec![Item::Old(old.root)],
        (true, false) => vec![Item::New(new.root)],
        (true, true) => Vec::new(),
    };
    let mut walk = Walk {
        old: Tree::new(old.entries),
        new: Tree::new(new.entries),
        rules,
        diff: Diff::default(),
        frames: vec![Frame {
            items: roots,
            branch: rules
                .filter
                .as_ref()
                .map(|f| f.branch.clone())
                .unwrap_or_default(),
            of_roots: true,
        }],
        old_keys: Keys::default(),
        new_keys: Keys::default(),
    };
    let mut seen = 0usize;
    let mut counted = 0u64;
    while let Some(frame) = walk.frames.last_mut() {
        let Some(item) = frame.items.pop() else {
            walk.frames.pop();
            continue;
        };
        if seen.is_multiple_of(CANCEL_CHECK) {
            if cancel.is_cancelled() {
                return Err(Cancelled);
            }
            visited.fetch_add(counted, Relaxed);
            counted = 0;
        }
        seen += 1;
        // A pair whose kinds differ is visited again as a deleted and an added entry.
        counted += match item {
            Item::Both(a, b) => {
                let same_kind =
                    walk.old.entries[a as usize].kind == walk.new.entries[b as usize].kind;
                if same_kind { 2 } else { 0 }
            }
            Item::Old(_) | Item::New(_) => 1,
        };
        walk.visit(item);
    }
    visited.fetch_add(counted, Relaxed);
    Ok(walk.diff)
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

/// The items of one folder still to visit.
struct Frame {
    /// Reversed, so `pop` gives the next item in tree order.
    items: Vec<Item>,
    /// Where the folder whose children these are stands against the exclusions; in the
    /// frame of the roots, where the roots themselves stand.
    branch: Branch,
    of_roots: bool,
}

struct Walk<'a, 'r> {
    old: Tree<'a>,
    new: Tree<'a>,
    rules: &'r Rules<'r>,
    diff: Diff,
    /// A stack of frames, one per folder being walked.
    frames: Vec<Frame>,
    old_keys: Keys,
    new_keys: Keys,
}

impl Walk<'_, '_> {
    fn visit(&mut self, item: Item) {
        match item {
            Item::Both(a, b) => self.visit_both(a, b),
            Item::Old(a) => {
                let entry = &self.old.entries[a as usize];
                if entry.inaccessible {
                    self.inaccessible(Some(a), None);
                } else {
                    self.change(ChangeKind::Deleted, Some(a), None, Fields::default());
                    if entry.kind == EntryKind::Dir {
                        let branch = self.branch_of(&entry.name);
                        self.push_children(Some(a), None, false, branch);
                    }
                }
            }
            Item::New(b) => {
                let entry = &self.new.entries[b as usize];
                if entry.inaccessible {
                    self.inaccessible(None, Some(b));
                } else {
                    self.change(ChangeKind::Added, None, Some(b), Fields::default());
                    if entry.kind == EntryKind::Dir {
                        let branch = self.branch_of(&entry.name);
                        self.push_children(None, Some(b), false, branch);
                    }
                }
            }
        }
    }

    fn visit_both(&mut self, a: u32, b: u32) {
        let (old, new) = (&self.old.entries[a as usize], &self.new.entries[b as usize]);
        let frame = self.frames.last().expect("an item comes from a frame");
        if old.kind != new.kind {
            // Visited in this order: deleted first, then added.
            self.frames.push(Frame {
                items: vec![Item::New(b), Item::Old(a)],
                branch: frame.branch.clone(),
                of_roots: frame.of_roots,
            });
            return;
        }
        let unknown = old.inaccessible || new.inaccessible;
        let fields = changed_fields(old, new, frame.of_roots, unknown, self.rules.file_ids);
        if unknown {
            self.inaccessible(Some(a), Some(b));
        }
        if !fields.is_empty() {
            self.change(ChangeKind::Modified, Some(a), Some(b), fields);
        }
        if old.kind == EntryKind::Dir {
            let branch = self.branch_of(&old.name);
            self.push_children(Some(a), Some(b), unknown, branch);
        }
    }

    fn change(&mut self, kind: ChangeKind, old: Option<u32>, new: Option<u32>, fields: Fields) {
        self.diff.changes.push(Change {
            kind,
            old,
            new,
            fields,
        });
    }

    fn inaccessible(&mut self, old: Option<u32>, new: Option<u32>) {
        let changes_before = self.diff.changes.len() as u32;
        self.diff.inaccessible.push(Inaccessible {
            old,
            new,
            changes_before,
        });
    }

    /// Where a folder named `name`, an item of the frame on top, stands against the
    /// exclusions.
    fn branch_of(&self, name: &OsStr) -> Branch {
        let frame = self.frames.last().expect("an item comes from a frame");
        match &self.rules.filter {
            Some(filter) if !frame.of_roots => filter
                .exclusions
                .child(&frame.branch, name)
                .expect("an excluded child is never queued"),
            _ => frame.branch.clone(),
        }
    }

    /// Matches the children of a folder by name and queues them, except the excluded ones.
    /// With `skip_unmatched`, children found on one side only are dropped.
    fn push_children(
        &mut self,
        old: Option<u32>,
        new: Option<u32>,
        skip_unmatched: bool,
        branch: Branch,
    ) {
        match old {
            Some(a) => self.old_keys.fill(&self.old, a),
            None => self.old_keys.spans.clear(),
        }
        match new {
            Some(b) => self.new_keys.fill(&self.new, b),
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
                            &self.old,
                            &ok.spans[i..i2],
                            &self.new,
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
        if let Some(filter) = &self.rules.filter {
            let name = |item: &Item| match *item {
                Item::Both(a, _) | Item::Old(a) => &self.old.entries[a as usize].name,
                Item::New(b) => &self.new.entries[b as usize].name,
            };
            items.retain(|item| filter.exclusions.child(&branch, name(item)).is_some());
        }
        items.reverse();
        self.frames.push(Frame {
            items,
            branch,
            of_roots: false,
        });
    }
}

/// Fields that differ between two entries of the same kind. The names of the compared
/// roots are not compared: a root is named by its whole path. `unknown` is set when
/// either entry is inaccessible, so its version or link target may be missing.
fn changed_fields(
    old: &Entry,
    new: &Entry,
    is_root: bool,
    unknown: bool,
    file_ids: bool,
) -> Fields {
    let mut fields = Fields::default();
    let is_dir = old.kind == EntryKind::Dir;
    if !is_root && old.name != new.name {
        fields |= Fields::NAME;
    }
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
