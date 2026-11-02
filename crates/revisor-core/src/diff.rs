//! Comparison of two file system snapshots of the same folders.
//!
//! Entries are matched by path, with names compared without regard to case, as NTFS
//! does. A change is found from metadata alone:
//! - an entry only in the newer snapshot is added, one only in the older is removed,
//!   with every entry of its subtree;
//! - an entry whose kind changed (file, folder, link) is removed and added again;
//! - an entry is modified when its name case, size, creation time, last write time,
//!   attributes, version or link target differ. The archive attribute is ignored, and so
//!   is the last write time of folders, which changes with their contents.
//!
//! Entries inaccessible in at least one snapshot are listed apart and are never added or
//! removed. Children of a folder inaccessible on one side that are missing on the other
//! side are skipped: it is unknown whether they were there.

use std::cmp::Ordering;
use std::fmt;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use std::sync::atomic::{self, AtomicBool};

use crate::names::upcase;
use crate::scan::{Entry, EntryKind, VersionInfo};

const FILE_ATTRIBUTE_ARCHIVE: u32 = 0x20;
/// How many entries are compared between checks for cancellation.
const CANCEL_CHECK: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Removed,
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

/// One changed entry. `old` and `new` are indices into the compared snapshots; an added
/// entry has only `new`, a removed one only `old`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Change {
    pub kind: ChangeKind,
    pub old: Option<u32>,
    pub new: Option<u32>,
    /// What differs; empty unless the entry is modified.
    pub fields: Fields,
}

/// An entry inaccessible in at least one snapshot, by its indices in the snapshots it is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Inaccessible {
    pub old: Option<u32>,
    pub new: Option<u32>,
}

/// Differences between two snapshots, in tree order: each folder comes before its
/// contents, and the children of a folder are sorted by name.
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

/// Compares two snapshots laid out like [`crate::scan::Scan::entries`]. The roots are
/// always matched with each other. Setting `cancel` stops with [`Cancelled`].
pub fn diff(old: &[Entry], new: &[Entry], cancel: &AtomicBool) -> Result<Diff, Cancelled> {
    let mut walk = Walk {
        old: Tree::new(old),
        new: Tree::new(new),
        diff: Diff::default(),
        frames: Vec::new(),
        old_keys: Keys::default(),
        new_keys: Keys::default(),
    };
    match (old.is_empty(), new.is_empty()) {
        (false, false) => walk.frames.push(vec![Item::Both(0, 0)]),
        (false, true) => walk.frames.push(vec![Item::Old(0)]),
        (true, false) => walk.frames.push(vec![Item::New(0)]),
        (true, true) => {}
    }
    let mut seen = 0usize;
    while let Some(frame) = walk.frames.last_mut() {
        let Some(item) = frame.pop() else {
            walk.frames.pop();
            continue;
        };
        if seen.is_multiple_of(CANCEL_CHECK) && cancel.load(atomic::Ordering::Relaxed) {
            return Err(Cancelled);
        }
        seen += 1;
        walk.visit(item);
    }
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

/// Names of the children of one folder, ready to be sorted for matching. Kept between
/// folders so their buffers are reused.
#[derive(Default)]
struct Keys {
    /// For each child: its index and where its key and name are in `units`.
    spans: Vec<(u32, usize, usize)>,
    /// The upper-case key of each child followed by its name, in UTF-16.
    units: Vec<u16>,
}

impl Keys {
    fn fill(&mut self, tree: &Tree<'_>, parent: u32) {
        self.spans.clear();
        self.units.clear();
        for &child in tree.children(parent) {
            let start = self.units.len();
            self.units
                .extend(tree.entries[child as usize].name.encode_wide());
            let len = self.units.len() - start;
            self.units.extend_from_within(start..start + len);
            for u in &mut self.units[start..start + len] {
                *u = upcase(*u);
            }
            self.spans.push((child, start, len));
        }
        let units = &self.units;
        self.spans
            .sort_unstable_by(|&(_, a, n), &(_, b, m)| compare(units, a, n, b, m));
    }

    fn key(&self, span: (u32, usize, usize)) -> &[u16] {
        &self.units[span.1..span.1 + span.2]
    }
}

/// Orders children by key, and children whose keys are equal — possible only in folders
/// that are case-sensitive — by exact name.
fn compare(units: &[u16], a: usize, n: usize, b: usize, m: usize) -> Ordering {
    units[a..a + n]
        .cmp(&units[b..b + m])
        .then_with(|| units[a + n..a + 2 * n].cmp(&units[b + m..b + 2 * m]))
}

/// An entry waiting to be compared.
#[derive(Clone, Copy)]
enum Item {
    /// The same path in both snapshots.
    Both(u32, u32),
    /// Only in the older snapshot.
    Old(u32),
    /// Only in the newer snapshot.
    New(u32),
}

struct Walk<'a> {
    old: Tree<'a>,
    new: Tree<'a>,
    diff: Diff,
    /// Items still to visit, a stack per folder being walked; each stack is reversed, so
    /// `pop` gives the next item in tree order.
    frames: Vec<Vec<Item>>,
    old_keys: Keys,
    new_keys: Keys,
}

impl Walk<'_> {
    fn visit(&mut self, item: Item) {
        match item {
            Item::Both(a, b) => self.visit_both(a, b),
            Item::Old(a) => {
                if self.old.entries[a as usize].inaccessible {
                    self.inaccessible(Some(a), None);
                } else {
                    self.change(ChangeKind::Removed, Some(a), None, Fields::default());
                    self.push_children(Some(a), None, false);
                }
            }
            Item::New(b) => {
                if self.new.entries[b as usize].inaccessible {
                    self.inaccessible(None, Some(b));
                } else {
                    self.change(ChangeKind::Added, None, Some(b), Fields::default());
                    self.push_children(None, Some(b), false);
                }
            }
        }
    }

    fn visit_both(&mut self, a: u32, b: u32) {
        let (old, new) = (&self.old.entries[a as usize], &self.new.entries[b as usize]);
        if old.kind != new.kind {
            // Visited in this order: removed first, then added.
            self.frames.push(vec![Item::New(b), Item::Old(a)]);
            return;
        }
        let unknown = old.inaccessible || new.inaccessible;
        let fields = changed_fields(old, new, old.parent.is_none(), unknown);
        let is_dir = old.kind == EntryKind::Dir;
        if unknown {
            self.inaccessible(Some(a), Some(b));
        }
        if !fields.is_empty() {
            self.change(ChangeKind::Modified, Some(a), Some(b), fields);
        }
        if is_dir {
            self.push_children(Some(a), Some(b), unknown);
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
        self.diff.inaccessible.push(Inaccessible { old, new });
    }

    /// Matches the children of a folder by name and queues them. With `skip_unmatched`,
    /// children found on one side only are dropped.
    fn push_children(&mut self, old: Option<u32>, new: Option<u32>, skip_unmatched: bool) {
        if let Some(a) = old {
            self.old_keys.fill(&self.old, a);
        } else {
            self.old_keys.spans.clear();
        }
        if let Some(b) = new {
            self.new_keys.fill(&self.new, b);
        } else {
            self.new_keys.spans.clear();
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
            let item = match order {
                Ordering::Equal => {
                    let item = Item::Both(ok.spans[i].0, nk.spans[j].0);
                    i += 1;
                    j += 1;
                    item
                }
                Ordering::Less => {
                    let item = Item::Old(ok.spans[i].0);
                    i += 1;
                    item
                }
                Ordering::Greater => {
                    let item = Item::New(nk.spans[j].0);
                    j += 1;
                    item
                }
            };
            if skip_unmatched && !matches!(item, Item::Both(..)) {
                continue;
            }
            items.push(item);
        }
        items.reverse();
        self.frames.push(items);
    }
}

/// Fields that differ between two entries of the same kind. `unknown` is set when either
/// entry is inaccessible, so its version or link target may be missing.
fn changed_fields(old: &Entry, new: &Entry, is_root: bool, unknown: bool) -> Fields {
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
    if (old.attributes ^ new.attributes) & !FILE_ATTRIBUTE_ARCHIVE != 0 {
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
    fields
}

#[cfg(test)]
mod tests;
