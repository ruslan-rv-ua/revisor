//! Renamed and moved entries (TASK-42): which entries the differences show as deleted,
//! replaced or added are one entry at another path.
//!
//! An older entry that is deleted, inaccessible and gone from the newer tree, or replaced
//! at its path, and a newer entry that is added, or inaccessible and new, are the two
//! candidates of a pair when they lie on the same volume and have the same NTFS file ID,
//! the same kind and the same creation time. The file IDs 0 and all ones mean no ID
//! (MS-FSCC 2.1.10) and pair nothing. A file ID that several candidates of one side have —
//! the other names of a hard link — pairs nothing either: a move missed leaves the entries
//! deleted and added, as they were before TASK-42, while a move made up would mislead.
//!
//! The pairs are taken from the top of the newer trees down: a pair whose newer entry lies
//! inside the newer folder of another pair waits until that folder is compared with what
//! it held before, as that may match the entry by its path. So the blocks of the
//! differences that hold the entries of the pairs taken are walked again, and what the
//! differences then show is looked through again, until no pair is found.

use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};

use super::{Block, CANCEL_CHECK, Cancelled, ChangeKind, Fields, Forest, Pair, Walked};
use crate::job::Cancel;
use crate::scan::{Entry, EntryKind, FileId};

/// An entry of a tree of one of the comparisons made together: the comparison, and the
/// index of the entry in its older or in its newer tree.
pub(super) type Site = (u32, u32);

/// A hasher for the keys of the maps here, which are a few integers each: the hash of
/// rustc's own tables, far cheaper than the default one, which guards against keys chosen
/// to collide.
#[derive(Default)]
pub(super) struct IdHasher(u64);

impl IdHasher {
    fn add(&mut self, word: u64) {
        self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
}

impl Hasher for IdHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for chunk in bytes.chunks(8) {
            let mut word = [0; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            self.add(u64::from_le_bytes(word));
        }
    }

    fn write_u32(&mut self, n: u32) {
        self.add(n.into());
    }

    fn write_u64(&mut self, n: u64) {
        self.add(n);
    }

    fn write_usize(&mut self, n: usize) {
        self.add(n as u64);
    }
}

type Hash = BuildHasherDefault<IdHasher>;

/// The pairs found so far: which newer entries are older entries renamed or moved.
#[derive(Default)]
pub(super) struct Moves {
    /// The older entry of each newer entry that is one renamed or moved.
    old_of: HashMap<Site, Site, Hash>,
    /// The older entries that were renamed or moved.
    moved: HashSet<Site, Hash>,
}

impl Moves {
    /// The older entry that the newer entry `new` is, renamed or moved.
    pub(super) fn old_of(&self, new: Site) -> Option<Site> {
        if self.old_of.is_empty() {
            return None;
        }
        self.old_of.get(&new).copied()
    }

    /// Whether the older entry `old` is now at another path.
    pub(super) fn is_moved(&self, old: Site) -> bool {
        !self.moved.is_empty() && self.moved.contains(&old)
    }
}

/// An entry that may be one of a pair, with the difference that shows it and the block of
/// that difference to walk again once the entry is one of a pair, if any.
#[derive(Clone, Copy)]
struct Candidate {
    site: Site,
    shown_in: u32,
    block: Option<u32>,
}

/// The candidates that have one file ID on one volume.
#[derive(Clone, Copy, Default)]
struct Slot {
    olds: u32,
    news: u32,
    /// The last of the older ones and of the newer ones, by their places among the
    /// candidates: the only one where there is one.
    old: usize,
    new: usize,
}

/// Whether a file ID names a file: 0 and all ones do not (MS-FSCC 2.1.10).
fn names_a_file(id: FileId) -> bool {
    id.0 != [0; 16] && id.0 != [0xFF; 16]
}

/// The innermost block around each place of a list of a difference — its changes, or its
/// inaccessible entries —, for places asked in rising order.
struct Around<'b> {
    blocks: &'b [Block],
    /// Where a block starts and ends in the list.
    range: fn(&Block) -> (u32, u32),
    /// The blocks not looked at yet start here.
    next: usize,
    /// The blocks that started at or before the last place asked and may hold it.
    open: Vec<u32>,
}

impl<'b> Around<'b> {
    fn new(blocks: &'b [Block], range: fn(&Block) -> (u32, u32)) -> Self {
        Around {
            blocks,
            range,
            next: 0,
            open: Vec::new(),
        }
    }

    fn at(&mut self, place: u32) -> Option<u32> {
        while let Some(block) = self.blocks.get(self.next)
            && (self.range)(block).0 <= place
        {
            self.open.push(self.next as u32);
            self.next += 1;
        }
        // The blocks are one inside another or apart, so the last one that started and
        // has not ended is the innermost around the place.
        while let Some(&last) = self.open.last()
            && (self.range)(&self.blocks[last as usize]).1 <= place
        {
            self.open.pop();
        }
        self.open.last().copied()
    }
}

/// Looks through what the differences of `pairs` so far, `walked`, show as deleted,
/// replaced and added, for entries renamed or moved, and adds those found to `moves`.
/// Says, for each comparison, which blocks of its difference hold an entry of a pair just
/// found, and so must be walked again: none for a comparison that has no such block.
pub(super) fn find(
    forest: &Forest<'_>,
    pairs: &[Pair<'_>],
    walked: &[Walked],
    moves: &mut Moves,
    cancel: &Cancel,
) -> Result<Vec<Vec<bool>>, Cancelled> {
    let mut again = vec![Vec::new(); walked.len()];
    let (mut olds, mut news) = (Vec::new(), Vec::new());
    for (shown_in, walked) in walked.iter().enumerate() {
        let shown_in = shown_in as u32;
        let old_entry = |tree: Option<u32>, index: u32| {
            &forest.old[tree.unwrap_or(shown_in) as usize].entries[index as usize]
        };
        let mut old = |tree: Option<u32>, index: u32, block: Option<u32>| {
            let site = (tree.unwrap_or(shown_in), index);
            if names_a_file(old_entry(tree, index).file_id) && !moves.is_moved(site) {
                olds.push(Candidate {
                    site,
                    shown_in,
                    block,
                });
            }
        };
        let mut new = |index: u32, block: Option<u32>| {
            let entry = &forest.new[shown_in as usize].entries[index as usize];
            if names_a_file(entry.file_id) {
                news.push(Candidate {
                    site: (shown_in, index),
                    shown_in,
                    block,
                });
            }
        };
        let mut around = Around::new(&walked.blocks, |block| block.changes);
        for (at, change) in walked.diff.changes.iter().enumerate() {
            let at = at as u32;
            match (change.kind, change.old, change.new) {
                (ChangeKind::Deleted, Some(a), _) => old(change.old_in, a, around.at(at)),
                (ChangeKind::Added, _, Some(b)) => new(b, around.at(at)),
                (ChangeKind::Modified, Some(a), _) if change.fields.contains(Fields::FILE_ID) => {
                    // Replaced at its path, where it stays so once it is found elsewhere;
                    // only a folder is walked again then, as what it held went with it.
                    let is_dir = old_entry(change.old_in, a).kind == EntryKind::Dir;
                    old(change.old_in, a, is_dir.then(|| around.at(at)).flatten());
                }
                _ => {}
            }
        }
        let mut around = Around::new(&walked.blocks, |block| block.inaccessible);
        for (at, entry) in walked.diff.inaccessible.iter().enumerate() {
            let at = at as u32;
            match (entry.old, entry.new) {
                (Some(a), None) => old(entry.old_in, a, around.at(at)),
                (None, Some(b)) => new(b, around.at(at)),
                _ => {}
            }
        }
    }
    if olds.is_empty() || news.is_empty() {
        return Ok(again);
    }
    if cancel.is_cancelled() {
        return Err(Cancelled);
    }

    let old_entry = |c: &Candidate| &forest.old[c.site.0 as usize].entries[c.site.1 as usize];
    let new_entry = |c: &Candidate| &forest.new[c.site.0 as usize].entries[c.site.1 as usize];
    let old_key = |c: &Candidate| {
        let volume = pairs[c.site.0 as usize].rules.volumes[0];
        (volume, old_entry(c).file_id)
    };
    let new_key = |c: &Candidate| {
        let volume = pairs[c.site.0 as usize].rules.volumes[1];
        (volume, new_entry(c).file_id)
    };
    // The fewer candidates make the table; those of the other side only find their own
    // there, as a file ID that only one side has pairs nothing.
    let mut index: HashMap<(u64, FileId), Slot, Hash> =
        HashMap::with_capacity_and_hasher(olds.len().min(news.len()), Hash::default());
    if olds.len() <= news.len() {
        for (i, c) in olds.iter().enumerate() {
            let slot = index.entry(old_key(c)).or_default();
            (slot.olds, slot.old) = (slot.olds + 1, i);
        }
        for (i, c) in news.iter().enumerate() {
            if let Some(slot) = index.get_mut(&new_key(c)) {
                (slot.news, slot.new) = (slot.news + 1, i);
            }
        }
    } else {
        for (i, c) in news.iter().enumerate() {
            let slot = index.entry(new_key(c)).or_default();
            (slot.news, slot.new) = (slot.news + 1, i);
        }
        for (i, c) in olds.iter().enumerate() {
            if let Some(slot) = index.get_mut(&old_key(c)) {
                (slot.olds, slot.old) = (slot.olds + 1, i);
            }
        }
    }
    if cancel.is_cancelled() {
        return Err(Cancelled);
    }

    let same_entry = |old: &Entry, new: &Entry| old.kind == new.kind && old.created == new.created;
    let found: Vec<(Candidate, Candidate)> = index
        .values()
        .filter(|slot| slot.olds == 1 && slot.news == 1)
        .map(|slot| (olds[slot.old], news[slot.new]))
        .filter(|(old, new)| same_entry(old_entry(old), new_entry(new)))
        .collect();

    // The newer entries of the pairs found, marked in their trees, so that a pair inside
    // the newer folder of another one is told from one that is not.
    let mut marks: Vec<Vec<u8>> = vec![Vec::new(); pairs.len()];
    for (_, new) in &found {
        let (tree, index) = (new.site.0 as usize, new.site.1 as usize);
        if marks[tree].is_empty() {
            marks[tree] = vec![0; forest.new[tree].entries.len()];
        }
        marks[tree][index] = NEWER_OF_A_PAIR;
    }
    for (n, (old, new)) in found.iter().enumerate() {
        if n.is_multiple_of(CANCEL_CHECK) && cancel.is_cancelled() {
            return Err(Cancelled);
        }
        let tree = new.site.0 as usize;
        if inside_a_pair(&mut marks[tree], forest.new[tree].entries, new.site.1) {
            continue;
        }
        moves.old_of.insert(new.site, old.site);
        moves.moved.insert(old.site);
        for candidate in [old, new] {
            let Some(block) = candidate.block else {
                continue;
            };
            let shown_in = candidate.shown_in as usize;
            if again[shown_in].is_empty() {
                again[shown_in] = vec![false; walked[shown_in].blocks.len()];
            }
            again[shown_in][block as usize] = true;
        }
    }
    Ok(again)
}

/// The entry is the newer one of a pair found.
const NEWER_OF_A_PAIR: u8 = 1;
/// A folder above the entry is the newer one of a pair found.
const INSIDE: u8 = 2;
/// No folder above the entry is.
const OUTSIDE: u8 = 4;

/// Whether a folder above the entry `index` is the newer one of a pair found; `marks`
/// keeps what is found out on the way, so that every entry is looked at once.
fn inside_a_pair(marks: &mut [u8], entries: &[Entry], index: u32) -> bool {
    let mut climbed = Vec::new();
    let mut at = entries[index as usize].parent;
    let inside = loop {
        let Some(folder) = at else { break false };
        let mark = marks[folder as usize];
        if mark & (NEWER_OF_A_PAIR | INSIDE) != 0 {
            break true;
        }
        if mark & OUTSIDE != 0 {
            break false;
        }
        climbed.push(folder);
        at = entries[folder as usize].parent;
    };
    for folder in climbed {
        marks[folder as usize] |= if inside { INSIDE } else { OUTSIDE };
    }
    inside
}
