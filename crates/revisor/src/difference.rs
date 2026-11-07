//! The difference shown in the main window, apart from the window (TASK-11, TASK-13): the
//! model of the difference list and where the focus goes when a new difference arrives,
//! also that of «Знімок + порівняння» (TASK-12).
//!
//! The model holds the difference with the profile it is of, and its rows — one per
//! change and per inaccessible entry — in tree order. Over them it keeps the view the
//! user chose: the types shown, the text filter and the order, which give the rows the
//! list shows, and the row that has the focus. It writes the text of the cells, the line
//! of the pair of snapshots, the summary and the warnings, and gives the details of a row
//! and the path of its entry (TASK-14). The window keeps the list controls: they ask the
//! model for the text of the rows they show, and put the focus where the model says after
//! the view changes.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::thread;

use revisor_core::compare::{Difference, Side, Warning};
use revisor_core::diff::{self, Change, ChangeKind, Fields, Inaccessible};
use revisor_core::path::upcase;
use revisor_core::profiles::ProfileFile;
use revisor_core::scan::{Entry, EntryKind, VersionInfo};
use revisor_core::settings::{
    AttributeFormat, ChangeType, DifferenceSort, FocusAfterComparison, SizeFormat, SortColumn,
};
use revisor_core::store::{SnapshotInfo, Unavailable};

use crate::commands::Pane;
use crate::details::{self, Formats, Inaccessibility, Unreadable};
use crate::i18n::Strings;
use crate::keyboard::Focus;

/// The user's regional formats, in which the model writes numbers and moments: those of
/// Windows in the window, plain ones in the tests.
pub trait Regional {
    /// A number written in ASCII digits, with a dot before its `decimals` decimals, as
    /// the user's region writes it: "1234567" as "1 234 567", "1.17" as "1,17".
    fn number(&self, digits: &str, decimals: u32) -> String;
    /// A moment given as a FILETIME, as a snapshot's name shows a moment: the regional
    /// short date and the time with seconds, in this computer's time zone.
    fn file_time(&self, file_time: i64) -> String;
    /// A moment as `file_time` writes it, with `digits` digits, 1 to 7, of the fraction of
    /// the second after the seconds, so two moments of one second differ (TASK-14 #9).
    fn file_time_fraction(&self, file_time: i64, digits: u32) -> String;
    /// A moment given as Unix time in milliseconds, the same way: a snapshot's name.
    fn date_time(&self, unix_millis: i64) -> String;
    /// The letter the column «Атрибути» of File Explorer writes for `flag`, a single
    /// `FILE_ATTRIBUTE_*` bit, as Windows gives it; `None` for a flag it writes none for
    /// (TASK-14 #7).
    fn attribute_letter(&self, flag: u32) -> Option<String>;
}

/// A column of the difference list, in the order the list shows them (#12): the name
/// first and the longest, the folder, last, as screen readers read a row column by
/// column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Column {
    Name,
    Change,
    WhatChanged,
    Size,
    DateModified,
    Folder,
}

impl Column {
    pub const ALL: [Column; 6] = [
        Self::Name,
        Self::Change,
        Self::WhatChanged,
        Self::Size,
        Self::DateModified,
        Self::Folder,
    ];

    /// What sorting by this column is.
    pub fn sort(self) -> SortColumn {
        match self {
            Self::Name => SortColumn::Name,
            Self::Change => SortColumn::Change,
            Self::WhatChanged => SortColumn::WhatChanged,
            Self::Size => SortColumn::Size,
            Self::DateModified => SortColumn::DateModified,
            Self::Folder => SortColumn::Folder,
        }
    }
}

/// What a row is, as the column «Зміна» names it with the kind of the entry (#14, #15):
/// the type of the row, with replaced entries apart from the other modified ones.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowChange {
    Added,
    Deleted,
    Modified,
    Replaced,
    Inaccessible,
}

/// A field of an entry that differs between the snapshots, as the column «Що змінилося»
/// names them, in that order (#13): the version first, as it tells most about a program.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    FileVersion,
    ProductVersion,
    LinkTarget,
    Size,
    DateModified,
    DateCreated,
    Attributes,
    /// The name differs in case only.
    NameCase,
}

impl Field {
    pub const ALL: [Field; 8] = [
        Self::FileVersion,
        Self::ProductVersion,
        Self::LinkTarget,
        Self::Size,
        Self::DateModified,
        Self::DateCreated,
        Self::Attributes,
        Self::NameCase,
    ];

    fn bit(self) -> u16 {
        1 << self as u16
    }
}

/// In which snapshot an inaccessible entry is so (#13).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InaccessibleIn {
    Earlier,
    Later,
    Both,
}

/// A count and its text in the user's regional format: the number picks the plural
/// form, and the text is what is said.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Counted {
    pub n: u64,
    pub text: String,
}

/// How the total size of the files changed, with the change written as the setting
/// «Розмір» says (#7).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SizeChange {
    Increased(String),
    Decreased(String),
    Unchanged,
}

/// What the summary of a difference says (#7, #8, #10, #16): how many changes of each
/// type and inaccessible entries there are, how the total size of the files changed, and
/// how many warnings the comparison gave.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SummaryText {
    pub changes: Counted,
    pub added: Counted,
    pub deleted: Counted,
    pub modified: Counted,
    /// How many of the modified entries are replaced.
    pub replaced: Counted,
    pub inaccessible: Counted,
    pub size: SizeChange,
    pub warnings: Counted,
}

/// A snapshot of the pair, as the line of the pair names it (#4, #5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotText {
    /// The snapshot's name: its date and time.
    pub name: String,
    pub comment: Option<String>,
    /// The snapshot has been deleted since.
    pub deleted: bool,
}

/// What the line of the pair says (#4, #5): the profile, and the earlier and the later
/// snapshot, as they are now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PairText {
    pub profile: String,
    /// The profile has been deleted since.
    pub profile_deleted: bool,
    pub earlier: SnapshotText,
    pub later: SnapshotText,
}

/// Which rows the list shows and in which order (#21, #22, #28): the types shown,
/// indexed by [`ChangeType`], and the order. The text filter is apart: it is not kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct View {
    pub types: [bool; 4],
    pub sort: DifferenceSort,
}

impl Default for View {
    /// Every type, by path (#28).
    fn default() -> Self {
        Self {
            types: [true; 4],
            sort: DifferenceSort::default(),
        }
    }
}

/// What the window's state knows of the difference list (`AppState::of`, ADR-0013):
/// whether it shows a difference, how many rows it shows and its view.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ListState {
    pub has_difference: bool,
    pub rows: usize,
    pub view: View,
}

/// `bytes` as `format` writes them (#17): as File Explorer does — `StrFormatByteSize`,
/// whose table and Wine's implementation this follows — three significant digits of the
/// largest unit below a thousand, with the rest cut off, not rounded: "1,17 МБ" for
/// 1 234 567 bytes; with units of 1000, "1,23 МБ"; or every byte, "1 234 567 байтів".
pub fn format_size(
    bytes: u64,
    format: SizeFormat,
    s: &dyn Strings,
    regional: &dyn Regional,
) -> String {
    let base: u128 = match format {
        SizeFormat::Explorer => 1024,
        SizeFormat::Decimal => 1000,
        SizeFormat::Bytes => u128::MAX,
    };
    let value = u128::from(bytes);
    if value < base {
        return s.bytes(bytes, &regional.number(&bytes.to_string(), 0));
    }
    let mut unit = base;
    for power in 1..=6 {
        // Above a thousand units, the next unit: "0,98 МБ", never "1004 КБ".
        if value < 1000 * unit || power == 6 {
            let decimals: u32 = if value < 10 * unit {
                2
            } else if value < 100 * unit {
                1
            } else {
                0
            };
            let scale = 10u128.pow(decimals);
            let scaled = value * scale / unit;
            let digits = match decimals {
                0 => scaled.to_string(),
                _ => format!(
                    "{}.{:0width$}",
                    scaled / scale,
                    scaled % scale,
                    width = decimals as usize
                ),
            };
            return format!(
                "{} {}",
                regional.number(&digits, decimals),
                s.size_unit(power)
            );
        }
        unit *= base;
    }
    unreachable!("an exabyte holds any 64-bit count of bytes")
}

/// The index of an entry not in a tree.
const NONE: u32 = u32::MAX;

/// The separator of the parts of a path in the keys the list sorts and filters by: lower
/// than any character a name may hold, so keys compare part by part, as the tree orders
/// paths.
const SEPARATOR: u16 = 1;

/// Whether `unit` separates the parts of a path.
fn is_separator(unit: u16) -> bool {
    unit == u16::from(b'\\') || unit == u16::from(b'/')
}

/// The key of a path as the list sorts and filters by it: upper-cased as NTFS compares
/// names (TASK-10), its parts split by [`SEPARATOR`].
fn path_key(path: &OsStr) -> Vec<u16> {
    path.encode_wide()
        .map(|unit| {
            if is_separator(unit) {
                SEPARATOR
            } else {
                upcase(unit)
            }
        })
        .collect()
}

/// Appends a part, upper-cased, to the key of the folder it is in.
fn push_part(key: &mut Vec<u16>, name: &OsStr) {
    if key.last().is_some_and(|&unit| unit != SEPARATOR) {
        key.push(SEPARATOR);
    }
    key.extend(name.encode_wide().map(upcase));
}

/// Whether `hay` holds `needle`.
fn contains(hay: &[u16], needle: &[u16]) -> bool {
    let Some((&first, rest)) = needle.split_first() else {
        return true;
    };
    hay.len() >= needle.len()
        && hay[..=hay.len() - needle.len()]
            .iter()
            .enumerate()
            .any(|(i, &unit)| unit == first && hay[i + 1..i + needle.len()] == *rest)
}

/// The fewest rows a thread of the text filter checks: fewer are checked at once on one.
const ROWS_PER_THREAD: usize = 32_768;

/// A row of the difference list: a change or an inaccessible entry of the trees of one
/// folder of the difference, by the entry's indices in them.
#[derive(Clone, Copy, Debug)]
struct Row {
    /// The folder of the difference.
    folder: u32,
    /// The entry in the earlier tree, or [`NONE`].
    old: u32,
    /// The entry in the later tree, or [`NONE`].
    new: u32,
    kind: ChangeType,
    /// What differs; empty unless the row is a modified entry.
    fields: Fields,
}

impl Row {
    fn change(folder: u32, change: &Change) -> Self {
        Self {
            folder,
            old: change.old.unwrap_or(NONE),
            new: change.new.unwrap_or(NONE),
            kind: match change.kind {
                ChangeKind::Added => ChangeType::Added,
                ChangeKind::Deleted => ChangeType::Deleted,
                ChangeKind::Modified => ChangeType::Modified,
            },
            fields: change.fields,
        }
    }

    fn inaccessible(folder: u32, entry: &Inaccessible) -> Self {
        Self {
            folder,
            old: entry.old.unwrap_or(NONE),
            new: entry.new.unwrap_or(NONE),
            kind: ChangeType::Inaccessible,
            fields: Fields::default(),
        }
    }

    fn replaced(&self) -> bool {
        self.kind == ChangeType::Modified && self.fields.contains(Fields::FILE_ID)
    }

    /// What the column «Зміна» says the row is.
    fn change_type(&self) -> RowChange {
        match self.kind {
            ChangeType::Added => RowChange::Added,
            ChangeType::Deleted => RowChange::Deleted,
            ChangeType::Modified if self.replaced() => RowChange::Replaced,
            ChangeType::Modified => RowChange::Modified,
            ChangeType::Inaccessible => RowChange::Inaccessible,
        }
    }
}

/// The rows of `difference` in tree order (#16, #20): the folders of the difference by
/// their paths, and in each the changes and the inaccessible entries merged in the order
/// the comparison walked the trees — each folder before its contents, the contents by
/// name regardless of case, and an entry whose kind changed deleted before added.
fn rows_of(difference: &Difference) -> Vec<Row> {
    let mut folders: Vec<usize> = (0..difference.folders.len()).collect();
    folders.sort_by_cached_key(|&f| path_key(difference.folders[f].path.as_os_str()));
    let mut rows = Vec::new();
    for f in folders {
        let diff = &difference.folders[f].diff;
        let folder = f as u32;
        let mut inaccessible = diff.inaccessible.iter().peekable();
        for (at, change) in diff.changes.iter().enumerate() {
            while let Some(entry) = inaccessible.next_if(|i| i.changes_before as usize <= at) {
                rows.push(Row::inaccessible(folder, entry));
            }
            rows.push(Row::change(folder, change));
        }
        rows.extend(inaccessible.map(|entry| Row::inaccessible(folder, entry)));
    }
    rows
}

/// What the column «Що змінилося» says for a row, as a small code, so rows can be sorted
/// by it without writing it for each (#21).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum WhatChanged {
    Nothing,
    Inaccessible(InaccessibleIn),
    /// The [`Field`] bits of the fields that differ.
    Fields(u16),
}

/// The counts the summary is made of, counted once when the difference arrives.
#[derive(Clone, Copy, Debug, Default)]
struct Counts {
    added: u64,
    deleted: u64,
    modified: u64,
    replaced: u64,
    inaccessible: u64,
    /// How the total size of the files changed: the added ones' sizes minus the deleted
    /// ones' plus how the modified ones' changed (#7).
    size: i128,
}

/// A snapshot of the pair as it is now.
#[derive(Clone, Debug)]
struct PairSnapshot {
    info: SnapshotInfo,
    deleted: bool,
}

/// The profile and the snapshots of the difference as they are now (#5).
#[derive(Clone, Debug)]
struct Pair {
    profile: ProfileFile,
    profile_deleted: bool,
    snapshots: [PairSnapshot; 2],
}

/// The folders the rows' entries are in, each once, with the keys of their paths.
#[derive(Default)]
struct Folders {
    /// By the folder of the difference, the tree (whether the later one) and the entry;
    /// the entry [`NONE`] stands for the folder a tree's root lies in.
    ids: HashMap<(u32, bool, u32), u32>,
    /// The keys, one after another, and where each starts and how long it is.
    keys: Vec<u16>,
    spans: Vec<(u32, u32)>,
}

impl Folders {
    fn key(&self, id: u32) -> &[u16] {
        let (start, len) = self.spans[id as usize];
        &self.keys[start as usize..(start + len) as usize]
    }

    fn add(&mut self, at: (u32, bool, u32), key: &[u16]) -> u32 {
        let id = self.spans.len() as u32;
        self.spans.push((self.keys.len() as u32, key.len() as u32));
        self.keys.extend_from_slice(key);
        self.ids.insert(at, id);
        id
    }

    /// The folder that is the entry `entry` of `tree`, the tree of `side` of the folder
    /// `folder` of the difference.
    fn of_entry(&mut self, tree: &[Entry], folder: u32, side: bool, entry: u32) -> u32 {
        if let Some(&id) = self.ids.get(&(folder, side, entry)) {
            return id;
        }
        let at = &tree[entry as usize];
        let key = match at.parent {
            Some(parent) => {
                let parent = self.of_entry(tree, folder, side, parent);
                let mut key = self.key(parent).to_vec();
                push_part(&mut key, &at.name);
                key
            }
            // A root is named by its whole path.
            None => path_key(&at.name),
        };
        self.add((folder, side, entry), &key)
    }

    /// The folder the root of a tree lies in: the tree of `side` of the folder `folder` of
    /// the difference, whose root is named by its whole path, `root`.
    fn of_root(&mut self, root: &OsStr, folder: u32, side: bool) -> u32 {
        if let Some(&id) = self.ids.get(&(folder, side, NONE)) {
            return id;
        }
        let key = Path::new(root)
            .parent()
            .map(|parent| path_key(parent.as_os_str()))
            .unwrap_or_default();
        self.add((folder, side, NONE), &key)
    }
}

/// The keys of the rows' names and folders, which sorting by name or folder and the text
/// filter need; made when they are first needed, as a difference of a million rows takes
/// a moment to index.
struct Index {
    /// The upper-cased name of each row's entry, one after another; row `r`'s is
    /// `names[name_at[r]..name_at[r + 1]]`.
    names: Vec<u16>,
    name_at: Vec<u32>,
    /// The folder each row's entry is in.
    parent: Vec<u32>,
    folders: Folders,
    /// The place of each folder in path order; folders with the same path share it.
    folder_rank: Vec<u32>,
}

impl Index {
    fn new(rows: &[Row], difference: &Difference) -> Self {
        let mut names = Vec::new();
        let mut name_at = Vec::with_capacity(rows.len() + 1);
        let mut parent = Vec::with_capacity(rows.len());
        let mut folders = Folders::default();
        for row in rows {
            let (tree, side, entry) = shown_entry(difference, row);
            let at = &tree[entry as usize];
            name_at.push(names.len() as u32);
            match at.parent {
                Some(p) => {
                    names.extend(at.name.encode_wide().map(upcase));
                    parent.push(folders.of_entry(tree, row.folder, side, p));
                }
                None => {
                    names.extend(root_name(&at.name).encode_wide().map(upcase));
                    parent.push(folders.of_root(&at.name, row.folder, side));
                }
            }
        }
        name_at.push(names.len() as u32);
        let mut by_path: Vec<u32> = (0..folders.spans.len() as u32).collect();
        by_path.sort_unstable_by(|&a, &b| folders.key(a).cmp(folders.key(b)));
        let mut folder_rank = vec![0; by_path.len()];
        let mut rank = 0;
        for (i, &id) in by_path.iter().enumerate() {
            if i > 0 && folders.key(id) != folders.key(by_path[i - 1]) {
                rank += 1;
            }
            folder_rank[id as usize] = rank;
        }
        Self {
            names,
            name_at,
            parent,
            folders,
            folder_rank,
        }
    }

    fn name(&self, row: u32) -> &[u16] {
        let row = row as usize;
        &self.names[self.name_at[row] as usize..self.name_at[row + 1] as usize]
    }
}

/// What the text filter knows of the folders: whether the path of each holds the text,
/// and the end of each path with a separator after it, where the text may start and go
/// on into a name.
struct FolderTexts {
    holds: Vec<bool>,
    tails: Vec<u16>,
    tail_at: Vec<u32>,
}

impl FolderTexts {
    fn new(folders: &Folders, needle: &[u16]) -> Self {
        let count = folders.spans.len();
        let keep = needle.len().saturating_sub(1);
        let mut holds = Vec::with_capacity(count);
        let mut tails = Vec::new();
        let mut tail_at = Vec::with_capacity(count + 1);
        for id in 0..count as u32 {
            let key = folders.key(id);
            holds.push(contains(key, needle));
            tail_at.push(tails.len() as u32);
            tails.extend_from_slice(&key[key.len().saturating_sub(keep)..]);
            if key.last().is_some_and(|&unit| unit != SEPARATOR) {
                tails.push(SEPARATOR);
            }
        }
        tail_at.push(tails.len() as u32);
        Self {
            holds,
            tails,
            tail_at,
        }
    }

    fn tail(&self, folder: u32) -> &[u16] {
        let f = folder as usize;
        &self.tails[self.tail_at[f] as usize..self.tail_at[f + 1] as usize]
    }
}

/// Which rows' paths hold `needle` (#24), by row: the path of the folder, the name, or
/// the two across the separator. A million rows are checked on a few threads at once, so
/// that typing in the field «Фільтр:» does not lag (#29).
fn text_matches(index: &Index, needle: &[u16]) -> Vec<bool> {
    let folders = FolderTexts::new(&index.folders, needle);
    let check = |first: usize, out: &mut [bool]| {
        let mut across = Vec::new();
        for (i, out) in out.iter_mut().enumerate() {
            let row = (first + i) as u32;
            let folder = index.parent[row as usize];
            let name = index.name(row);
            *out = folders.holds[folder as usize] || contains(name, needle) || {
                across.clear();
                across.extend_from_slice(folders.tail(folder));
                across.extend_from_slice(&name[..name.len().min(needle.len() - 1)]);
                contains(&across, needle)
            };
        }
    };
    let mut matches = vec![false; index.parent.len()];
    let threads = thread::available_parallelism().map_or(1, |n| n.get().min(8));
    let chunk = matches.len().div_ceil(threads).max(ROWS_PER_THREAD);
    if matches.len() <= chunk {
        check(0, &mut matches);
    } else {
        thread::scope(|scope| {
            for (k, out) in matches.chunks_mut(chunk).enumerate() {
                let check = &check;
                scope.spawn(move || check(k * chunk, out));
            }
        });
    }
    matches
}

/// The tree of the entry a row shows and its index there: in the later tree where the
/// entry is there, else in the earlier one (#13); `true` for the later tree.
fn shown_entry<'a>(difference: &'a Difference, row: &Row) -> (&'a [Entry], bool, u32) {
    let folder = &difference.folders[row.folder as usize];
    if row.new != NONE {
        (&folder.new, true, row.new)
    } else {
        (&folder.old, false, row.old)
    }
}

/// The name of a root, which is named by its whole path: its last part, or the whole
/// path for the root of a drive.
fn root_name(path: &OsStr) -> &OsStr {
    Path::new(path).file_name().unwrap_or(path)
}

/// A difference shown, with all the model knows of it.
struct Shown {
    difference: Difference,
    pair: Pair,
    /// Every row, in tree order.
    rows: Vec<Row>,
    counts: Counts,
    /// The rows in the order of the view.
    order: Vec<u32>,
    /// The rows the filter lets through, in that order: what the list shows.
    visible: Vec<u32>,
    /// The row that has the focus, and its place in `visible`.
    current: Option<(u32, usize)>,
    index: Option<Index>,
}

/// The details of a row (TASK-14): the name of its entry, which the window's title shows,
/// and the text, a line per property.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Details {
    pub name: String,
    pub text: String,
}

/// The model of the difference list (TASK-13).
pub struct List {
    s: &'static dyn Strings,
    regional: Box<dyn Regional>,
    size_format: SizeFormat,
    /// How the details write attributes (TASK-14 #7).
    attribute_format: AttributeFormat,
    view: View,
    /// The text filter as typed, and as the keys of paths hold it.
    text: String,
    needle: Vec<u16>,
    shown: Option<Shown>,
}

impl List {
    /// A list with no difference yet, which will show `view` and write sizes as
    /// `size_format` says.
    pub fn new(
        s: &'static dyn Strings,
        regional: Box<dyn Regional>,
        view: View,
        size_format: SizeFormat,
    ) -> Self {
        Self {
            s,
            regional,
            size_format,
            attribute_format: AttributeFormat::default(),
            view,
            text: String::new(),
            needle: Vec::new(),
            shown: None,
        }
    }

    pub fn view(&self) -> View {
        self.view
    }

    /// The text filter, as typed.
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn size_format(&self) -> SizeFormat {
        self.size_format
    }

    /// Writes sizes as `format` says from now on (#17).
    pub fn set_size_format(&mut self, format: SizeFormat) {
        self.size_format = format;
    }

    pub fn attribute_format(&self) -> AttributeFormat {
        self.attribute_format
    }

    /// The details write attributes as `format` says from now on (TASK-14 #7).
    pub fn set_attribute_format(&mut self, format: AttributeFormat) {
        self.attribute_format = format;
    }

    pub fn has_difference(&self) -> bool {
        self.shown.is_some()
    }

    /// Shows `difference`, of `profile`, in place of the one shown (#28): with the view
    /// as it is, but without the text filter, which was about the other difference. The
    /// focus is on the first row; its place comes back.
    pub fn replace(&mut self, difference: Difference, profile: ProfileFile) -> Option<usize> {
        let rows = rows_of(&difference);
        let counts = count(&rows, &difference);
        let snapshot = |info: &SnapshotInfo| PairSnapshot {
            info: info.clone(),
            deleted: false,
        };
        let pair = Pair {
            profile,
            profile_deleted: false,
            snapshots: [snapshot(&difference.earlier), snapshot(&difference.later)],
        };
        self.text.clear();
        self.needle.clear();
        self.shown = Some(Shown {
            difference,
            pair,
            rows,
            counts,
            order: Vec::new(),
            visible: Vec::new(),
            current: None,
            index: None,
        });
        self.sort();
        self.filter()
    }

    /// What the window's state knows of the list.
    pub fn state(&self) -> ListState {
        ListState {
            has_difference: self.has_difference(),
            rows: self.len(),
            view: self.view,
        }
    }

    /// How many rows the list shows.
    pub fn len(&self) -> usize {
        self.shown.as_ref().map_or(0, |shown| shown.visible.len())
    }

    /// How many rows the difference has, shown or not.
    pub fn total(&self) -> usize {
        self.shown.as_ref().map_or(0, |shown| shown.rows.len())
    }

    /// The focus has moved to the row at `index` of the list.
    pub fn focus(&mut self, index: usize) {
        if let Some(shown) = &mut self.shown
            && let Some(&row) = shown.visible.get(index)
        {
            shown.current = Some((row, index));
        }
    }

    /// The place of the row that has the focus.
    pub fn current(&self) -> Option<usize> {
        self.shown
            .as_ref()
            .and_then(|shown| shown.current)
            .map(|(_, index)| index)
    }

    /// Shows the rows of the types `types` (#22); the focus stays on its row, or goes to
    /// the nearest row shown when its own is hidden (#27). Its place comes back.
    pub fn set_types(&mut self, types: [bool; 4]) -> Option<usize> {
        self.view.types = types;
        self.filter()
    }

    /// Shows the rows whose paths hold `text` (#24), regardless of case, with `*` and `?`
    /// as they are; the focus as with [`List::set_types`].
    pub fn set_text(&mut self, text: &str) -> Option<usize> {
        self.text = text.to_string();
        self.needle = text
            .encode_utf16()
            .map(|unit| {
                if unit == u16::from(b'\\') {
                    SEPARATOR
                } else {
                    upcase(unit)
                }
            })
            .collect();
        self.filter()
    }

    /// Puts the rows in the order `sort` (#21); the focus stays on its row, whose new
    /// place comes back.
    pub fn set_sort(&mut self, sort: DifferenceSort) -> Option<usize> {
        self.view.sort = sort;
        self.sort();
        self.filter()
    }

    /// The text of the cell of `column` in the row at `index` of the list.
    pub fn cell(&self, index: usize, column: Column) -> String {
        let s = self.s;
        let Some(shown) = &self.shown else {
            return String::new();
        };
        let Some(&r) = shown.visible.get(index) else {
            return String::new();
        };
        let row = &shown.rows[r as usize];
        let (tree, _, at) = shown_entry(&shown.difference, row);
        let entry = &tree[at as usize];
        match column {
            Column::Name => match entry.parent {
                Some(_) => entry.name.to_string_lossy().into_owned(),
                None => root_name(&entry.name).to_string_lossy().into_owned(),
            },
            Column::Change => s.change(row.change_type(), entry.kind).to_string(),
            Column::WhatChanged => what_changed_words(s, what_changed(&shown.difference, row)),
            Column::Size => match entry.kind {
                EntryKind::File => format_size(entry.size, self.size_format, s, &*self.regional),
                _ => String::new(),
            },
            Column::DateModified => self.regional.file_time(entry.modified),
            Column::Folder => match entry.parent {
                Some(parent) => diff::path(tree, parent).to_string_lossy().into_owned(),
                None => Path::new(&*entry.name)
                    .parent()
                    .map(|parent| parent.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            },
        }
    }

    /// The whole path of the entry of the row at `index` of the list, as its columns show
    /// it: in the later snapshot, else in the earlier (TASK-14 #11).
    pub fn entry_path(&self, index: usize) -> Option<PathBuf> {
        let shown = self.shown.as_ref()?;
        let &r = shown.visible.get(index)?;
        let (tree, _, at) = shown_entry(&shown.difference, &shown.rows[r as usize]);
        Some(diff::path(tree, at))
    }

    /// The details of the row at `index` of the list (TASK-14 #2–#10): the entry's name
    /// for the window's title, and a line per property.
    pub fn details(&self, index: usize) -> Option<Details> {
        let shown = self.shown.as_ref()?;
        let &r = shown.visible.get(index)?;
        let row = &shown.rows[r as usize];
        let folder = &shown.difference.folders[row.folder as usize];
        let old = (row.old != NONE).then(|| &folder.old[row.old as usize]);
        let new = (row.new != NONE).then(|| &folder.new[row.new as usize]);
        let fields = match row.kind {
            // An entry inaccessible in a tree may be modified too: its change says how.
            ChangeType::Inaccessible if old.is_some() && new.is_some() => folder
                .diff
                .changes
                .iter()
                .find(|c| c.old == Some(row.old) && c.new == Some(row.new))
                .map_or_else(Fields::default, |c| c.fields),
            _ => row.fields,
        };
        let what_changed = match row.kind {
            ChangeType::Modified => {
                what_changed_words(self.s, what_changed(&shown.difference, row))
            }
            _ => String::new(),
        };
        let subject = details::Row {
            path: self.entry_path(index)?.to_string_lossy().into_owned(),
            change: row.change_type(),
            what_changed,
            old,
            new,
            fields,
            inaccessible: inaccessibility(row.kind, old, new),
        };
        let formats = Formats {
            size: self.size_format,
            attributes: self.attribute_format,
        };
        Some(Details {
            name: self.cell(index, Column::Name),
            text: details::text(self.s, &*self.regional, formats, &subject),
        })
    }

    /// The profile of the difference shown, as last known.
    pub fn profile(&self) -> Option<&ProfileFile> {
        self.shown.as_ref().map(|shown| &shown.pair.profile)
    }

    /// The profile of the difference shown is now `profile` — renamed, its file maybe
    /// too — or, with `None`, has been deleted (#5).
    pub fn set_profile(&mut self, profile: Option<ProfileFile>) {
        if let Some(shown) = &mut self.shown {
            match profile {
                Some(profile) => {
                    shown.pair.profile = profile;
                    shown.pair.profile_deleted = false;
                }
                None => shown.pair.profile_deleted = true,
            }
        }
    }

    /// The snapshots the profile of the difference shown has now: those of the pair that
    /// are not among them have been deleted, and the others take their comments (#5).
    pub fn set_snapshots(&mut self, snapshots: &[SnapshotInfo]) {
        if let Some(shown) = &mut self.shown {
            for pair in &mut shown.pair.snapshots {
                match snapshots.iter().find(|s| s.id == pair.info.id) {
                    Some(now) => {
                        pair.info = now.clone();
                        pair.deleted = false;
                    }
                    None => pair.deleted = true,
                }
            }
        }
    }

    /// The text of the field «Порівняння» (#1): the line of the pair, the summary, and a
    /// line for each warning; `None` before the first comparison.
    pub fn comparison_text(&self) -> Option<String> {
        let shown = self.shown.as_ref()?;
        let mut lines = vec![self.pair_line(&shown.pair), self.summary_line(shown)];
        lines.extend(shown.difference.warnings.iter().map(|warning| {
            let reason = match warning {
                Warning::FolderUnavailable { reason, .. } => unavailable_reason(self.s, reason),
                _ => String::new(),
            };
            self.s.warning(warning, &reason)
        }));
        Some(lines.join("\n"))
    }

    /// The summary, which names the list (#3, #7, #8, #10, #25); `None` before the first
    /// comparison. The window takes it as [`List::list_name`] and in
    /// [`List::comparison_text`].
    #[cfg_attr(not(test), expect(dead_code))]
    pub fn summary(&self) -> Option<String> {
        self.shown.as_ref().map(|shown| self.summary_line(shown))
    }

    /// "Показано 12 із 22.": how many rows the list shows, of all of the difference's
    /// (#25, #26).
    pub fn showing(&self) -> String {
        let shown = self.len() as u64;
        let total = self.total() as u64;
        self.s
            .showing(&self.count_text(shown), &self.count_text(total))
    }

    /// What the list is named, for screen readers, and what its empty area says (#3,
    /// #10, #11, #25): the summary; before the first comparison that there is no
    /// difference yet; and when the filter has hidden every row, that none matches it.
    pub fn list_name(&self) -> String {
        match &self.shown {
            None => self.s.no_difference_yet().to_string(),
            Some(shown) if shown.visible.is_empty() && !shown.rows.is_empty() => {
                self.s.no_match().to_string()
            }
            Some(shown) => self.summary_line(shown),
        }
    }

    /// The path of the row at `index` of the list, for tests, which name rows by their
    /// paths.
    #[cfg(test)]
    fn path_of(&self, index: usize) -> String {
        let folder = self.cell(index, Column::Folder);
        let name = self.cell(index, Column::Name);
        Path::new(&folder).join(name).to_string_lossy().into_owned()
    }

    fn count_text(&self, n: u64) -> String {
        self.regional.number(&n.to_string(), 0)
    }

    fn counted(&self, n: u64) -> Counted {
        Counted {
            n,
            text: self.count_text(n),
        }
    }

    fn summary_line(&self, shown: &Shown) -> String {
        let c = shown.counts;
        let size = |change: i128| {
            let bytes = u64::try_from(change.unsigned_abs()).unwrap_or(u64::MAX);
            format_size(bytes, self.size_format, self.s, &*self.regional)
        };
        let summary = SummaryText {
            changes: self.counted(c.added + c.deleted + c.modified),
            added: self.counted(c.added),
            deleted: self.counted(c.deleted),
            modified: self.counted(c.modified),
            replaced: self.counted(c.replaced),
            inaccessible: self.counted(c.inaccessible),
            size: match c.size.cmp(&0) {
                Ordering::Greater => SizeChange::Increased(size(c.size)),
                Ordering::Less => SizeChange::Decreased(size(c.size)),
                Ordering::Equal => SizeChange::Unchanged,
            },
            warnings: self.counted(shown.difference.warnings.len() as u64),
        };
        let mut line = self.s.summary(&summary);
        if shown.visible.len() < shown.rows.len() {
            line.push(' ');
            line.push_str(&self.showing());
        }
        line
    }

    fn pair_line(&self, pair: &Pair) -> String {
        let snapshot = |p: &PairSnapshot| SnapshotText {
            name: self.regional.date_time(p.info.taken_at),
            comment: p.info.comment.clone(),
            deleted: p.deleted,
        };
        self.s.pair(&PairText {
            profile: pair.profile.name.clone(),
            profile_deleted: pair.profile_deleted,
            earlier: snapshot(&pair.snapshots[0]),
            later: snapshot(&pair.snapshots[1]),
        })
    }

    /// Puts every row in the order of the view: by the key of its column, the rows with
    /// the same key in tree order, whichever the direction (#21).
    fn sort(&mut self) {
        let sort = self.view.sort;
        let Some(shown) = &mut self.shown else {
            return;
        };
        let n = shown.rows.len() as u32;
        if sort.column == SortColumn::Path {
            shown.order = if sort.descending {
                (0..n).rev().collect()
            } else {
                (0..n).collect()
            };
            return;
        }
        let direction = |order: Ordering| {
            if sort.descending {
                order.reverse()
            } else {
                order
            }
        };
        let mut order: Vec<u32> = (0..n).collect();
        match sort.column {
            SortColumn::Path => unreachable!("the tree order needs no sorting"),
            SortColumn::Name => {
                let index = shown
                    .index
                    .get_or_insert_with(|| Index::new(&shown.rows, &shown.difference));
                order.sort_unstable_by(|&a, &b| {
                    direction(index.name(a).cmp(index.name(b))).then(a.cmp(&b))
                });
            }
            SortColumn::Folder => {
                let index = shown
                    .index
                    .get_or_insert_with(|| Index::new(&shown.rows, &shown.difference));
                let rank = |r: u32| index.folder_rank[index.parent[r as usize] as usize];
                order.sort_unstable_by(|&a, &b| direction(rank(a).cmp(&rank(b))).then(a.cmp(&b)));
            }
            SortColumn::Change => {
                let keys: Vec<u8> = shown
                    .rows
                    .iter()
                    .map(|row| change_rank(&shown.difference, row))
                    .collect();
                sort_by_key(&mut order, &keys, sort.descending);
            }
            SortColumn::WhatChanged => {
                let codes: Vec<WhatChanged> = shown
                    .rows
                    .iter()
                    .map(|row| what_changed(&shown.difference, row))
                    .collect();
                // Each text once, ordered as the tree orders names.
                let mut texts: Vec<(Vec<u16>, WhatChanged)> = Vec::new();
                for &code in &codes {
                    if !texts.iter().any(|(_, c)| *c == code) {
                        let text = what_changed_words(self.s, code);
                        texts.push((text.encode_utf16().map(upcase).collect(), code));
                    }
                }
                texts.sort_by(|a, b| a.0.cmp(&b.0));
                let rank: HashMap<WhatChanged, u32> = texts
                    .iter()
                    .enumerate()
                    .map(|(i, (_, code))| (*code, i as u32))
                    .collect();
                let keys: Vec<u32> = codes.iter().map(|code| rank[code]).collect();
                sort_by_key(&mut order, &keys, sort.descending);
            }
            SortColumn::Size => {
                let keys: Vec<Option<u64>> = shown
                    .rows
                    .iter()
                    .map(|row| {
                        let (tree, _, at) = shown_entry(&shown.difference, row);
                        let entry = &tree[at as usize];
                        // Folders and links have no size, so they come before the files.
                        (entry.kind == EntryKind::File).then_some(entry.size)
                    })
                    .collect();
                sort_by_key(&mut order, &keys, sort.descending);
            }
            SortColumn::DateModified => {
                let keys: Vec<i64> = shown
                    .rows
                    .iter()
                    .map(|row| {
                        let (tree, _, at) = shown_entry(&shown.difference, row);
                        tree[at as usize].modified
                    })
                    .collect();
                sort_by_key(&mut order, &keys, sort.descending);
            }
        }
        shown.order = order;
    }

    /// Lets the rows of the order through the filter: the types shown and the text. The
    /// focus stays on its row, or, when the filter hides it, goes to the next row shown,
    /// else to the one before it (#27); without a focus, to the first row. Its place
    /// comes back.
    fn filter(&mut self) -> Option<usize> {
        let types = self.view.types;
        let needle = &self.needle;
        let shown = self.shown.as_mut()?;
        let text = (!needle.is_empty()).then(|| {
            let index = shown
                .index
                .get_or_insert_with(|| Index::new(&shown.rows, &shown.difference));
            text_matches(index, needle)
        });
        let passes = |r: u32| {
            types[shown.rows[r as usize].kind as usize]
                && text.as_ref().is_none_or(|text| text[r as usize])
        };
        let focused = shown.current.map(|(row, _)| row);
        let mut visible = Vec::with_capacity(shown.order.len());
        let mut place = None;
        let mut after = None;
        let mut passed = false;
        for &r in &shown.order {
            let is_focused = Some(r) == focused;
            if passes(r) {
                if is_focused {
                    place = Some(visible.len());
                } else if passed && after.is_none() && place.is_none() {
                    after = Some(visible.len());
                }
                visible.push(r);
            }
            passed |= is_focused;
        }
        let place = match focused {
            Some(_) => place.or(after).or(visible.len().checked_sub(1)),
            None => (!visible.is_empty()).then_some(0),
        };
        shown.current = place.map(|i| (visible[i], i));
        shown.visible = visible;
        place
    }
}

/// The words of the column «Що змінилося» for `what`: the fields that differ, in the order
/// of [`Field`], or in which snapshot an entry is inaccessible.
fn what_changed_words(s: &dyn Strings, what: WhatChanged) -> String {
    match what {
        WhatChanged::Nothing => String::new(),
        WhatChanged::Inaccessible(which) => s.inaccessible_in(which).to_string(),
        WhatChanged::Fields(bits) => {
            let names: Vec<&str> = Field::ALL
                .into_iter()
                .filter(|field| bits & field.bit() != 0)
                .map(|field| s.field(field))
                .collect();
            names.join(", ")
        }
    }
}

/// In which snapshot the entry of a row of `kind` is inaccessible, and why, as far as the
/// snapshots know (TASK-14 #3, decided 2026-11-06): an entry marked so could not be read
/// there — a folder's contents, or a link's target —; an entry of an inaccessible row
/// that is in one tree only and not marked is unknown in the other, whose folder on the
/// way could not be read or is a link (TASK-10). `None` for an entry readable in both.
fn inaccessibility(
    kind: ChangeType,
    old: Option<&Entry>,
    new: Option<&Entry>,
) -> Option<Inaccessibility> {
    let why = |entry: &Entry| match entry.kind {
        EntryKind::Link => Unreadable::LinkTarget,
        // Only folders and links are ever marked: files are never listed.
        EntryKind::Dir | EntryKind::File => Unreadable::Folder,
    };
    let in_snapshot = |side: Side| match side {
        Side::Earlier => InaccessibleIn::Earlier,
        Side::Later => InaccessibleIn::Later,
    };
    match (old, new) {
        (Some(a), Some(b)) => {
            let which = match (a.inaccessible, b.inaccessible) {
                (true, false) => InaccessibleIn::Earlier,
                (false, true) => InaccessibleIn::Later,
                (true, true) => InaccessibleIn::Both,
                (false, false) => return None,
            };
            let marked = if a.inaccessible { a } else { b };
            Some(Inaccessibility {
                which,
                why: why(marked),
                absent_in: None,
            })
        }
        (Some(entry), None) | (None, Some(entry)) if kind == ChangeType::Inaccessible => {
            let (here, other) = if old.is_some() {
                (Side::Earlier, Side::Later)
            } else {
                (Side::Later, Side::Earlier)
            };
            Some(if entry.inaccessible {
                Inaccessibility {
                    which: in_snapshot(here),
                    why: why(entry),
                    absent_in: Some(other),
                }
            } else {
                Inaccessibility {
                    which: in_snapshot(other),
                    why: Unreadable::Unknown,
                    absent_in: None,
                }
            })
        }
        _ => None,
    }
}

/// Why a folder of a snapshot could not be read, in the interface's words (TASK-8,
/// TASK-13 #9): the one place that says it, for the warning of a comparison and for the
/// folders the snapshot did not find (TASK-25). The warning already says the folder could
/// not be read, so a Windows error comes as its reason alone (TASK-17).
pub fn unavailable_reason(s: &dyn Strings, reason: &Unavailable) -> String {
    match reason {
        Unavailable::NotNtfs(file_system) => s.folder_volume_not_ntfs(file_system),
        Unavailable::Network => s.folder_volume_network().into(),
        Unavailable::NoDrive => s.folder_drive_not_connected().into(),
        Unavailable::NotAFolder => s.folder_is_a_file().into(),
        Unavailable::Error(code) => {
            let error = std::io::Error::from_raw_os_error(*code);
            crate::failure::reason(s, &error)
        }
    }
}

/// Sorts `order` by `keys`, indexed by row, the rows with the same key in tree order.
fn sort_by_key<K: Ord>(order: &mut [u32], keys: &[K], descending: bool) {
    order.sort_unstable_by(|&a, &b| {
        let by_key = keys[a as usize].cmp(&keys[b as usize]);
        let by_key = if descending { by_key.reverse() } else { by_key };
        by_key.then(a.cmp(&b))
    });
}

/// The place of a row's change among the others when sorted by «Зміна» (#21): by type in
/// the order of the check boxes, replaced right after modified, then by the kind of the
/// entry — file, folder, link.
fn change_rank(difference: &Difference, row: &Row) -> u8 {
    let (tree, _, at) = shown_entry(difference, row);
    let kind = match tree[at as usize].kind {
        EntryKind::File => 0,
        EntryKind::Dir => 1,
        EntryKind::Link => 2,
    };
    let change = match row.change_type() {
        RowChange::Added => 0,
        RowChange::Deleted => 1,
        RowChange::Modified => 2,
        RowChange::Replaced => 3,
        RowChange::Inaccessible => 4,
    };
    change * 3 + kind
}

/// What the column «Що змінилося» says for `row` (#13, #15): the fields of a modified
/// entry that differ, without the file ID, which «Замінено» says; for an inaccessible
/// entry, in which snapshot it is so; nothing for an added or deleted one.
fn what_changed(difference: &Difference, row: &Row) -> WhatChanged {
    let folder = &difference.folders[row.folder as usize];
    let old = (row.old != NONE).then(|| &folder.old[row.old as usize]);
    let new = (row.new != NONE).then(|| &folder.new[row.new as usize]);
    match row.kind {
        ChangeType::Added | ChangeType::Deleted => WhatChanged::Nothing,
        ChangeType::Inaccessible => WhatChanged::Inaccessible(match (old, new) {
            (Some(a), Some(b)) => match (a.inaccessible, b.inaccessible) {
                (true, false) => InaccessibleIn::Earlier,
                (false, true) => InaccessibleIn::Later,
                _ => InaccessibleIn::Both,
            },
            // An entry of one tree only: inaccessible there, or, when it is not, unknown
            // in the other snapshot, whose folder above it could not be read.
            (Some(a), None) if a.inaccessible => InaccessibleIn::Earlier,
            (Some(_), None) => InaccessibleIn::Later,
            (None, Some(b)) if b.inaccessible => InaccessibleIn::Later,
            (None, Some(_)) => InaccessibleIn::Earlier,
            (None, None) => InaccessibleIn::Both,
        }),
        ChangeType::Modified => {
            let fields = row.fields;
            let mut bits = 0u16;
            let mut set = |field: Field| bits |= field.bit();
            if fields.contains(Fields::VERSION) {
                match (old.map(|e| e.version), new.map(|e| e.version)) {
                    (Some(VersionInfo::Some(a)), Some(VersionInfo::Some(b))) => {
                        if a.file != b.file {
                            set(Field::FileVersion);
                        }
                        if a.product != b.product {
                            set(Field::ProductVersion);
                        }
                    }
                    // The version resource came or went.
                    _ => set(Field::FileVersion),
                }
            }
            for (flag, field) in [
                (Fields::LINK_TARGET, Field::LinkTarget),
                (Fields::SIZE, Field::Size),
                (Fields::MODIFIED, Field::DateModified),
                (Fields::CREATED, Field::DateCreated),
                (Fields::ATTRIBUTES, Field::Attributes),
                (Fields::NAME, Field::NameCase),
            ] {
                if fields.contains(flag) {
                    set(field);
                }
            }
            WhatChanged::Fields(bits)
        }
    }
}

/// The counts of the summary (#7, #10, #16): changes by type, the replaced among the
/// modified, the inaccessible entries apart, and the change of the total size of files.
fn count(rows: &[Row], difference: &Difference) -> Counts {
    let mut counts = Counts::default();
    for row in rows {
        let folder = &difference.folders[row.folder as usize];
        let file_size = |tree: &[Entry], at: u32| -> i128 {
            match tree.get(at as usize) {
                Some(entry) if entry.kind == EntryKind::File => i128::from(entry.size),
                _ => 0,
            }
        };
        match row.kind {
            ChangeType::Added => {
                counts.added += 1;
                counts.size += file_size(&folder.new, row.new);
            }
            ChangeType::Deleted => {
                counts.deleted += 1;
                counts.size -= file_size(&folder.old, row.old);
            }
            ChangeType::Modified => {
                counts.modified += 1;
                counts.replaced += u64::from(row.replaced());
                counts.size += file_size(&folder.new, row.new) - file_size(&folder.old, row.old);
            }
            ChangeType::Inaccessible => counts.inaccessible += 1,
        }
    }
    counts
}

/// What the window does with a difference that has just arrived.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arrival {
    /// The difference takes the place of the one shown; the focus stays where it is, and
    /// the end is said without it.
    Replace,
    /// The difference takes the place of the one shown, and the keyboard focus moves to
    /// its first row: into a new list, which is announced as on coming to it from
    /// elsewhere, also when the focus was in the old one (#21).
    Focus,
    /// The profile of the snapshot of «Знімок + порівняння» is opened in place of another
    /// one, and the difference arrives as with `Focus` (TASK-12 #7).
    OpenProfileAndFocus,
}

/// What happens when a comparison ends under the setting «Фокус після порівняння», with
/// the focus at `focus` (TASK-11 #12, #13, #21).
/// The focus still where it was when the comparison started, or on its progress row,
/// which goes away with it, counts as not moved.
pub fn arrival(setting: FocusAfterComparison, focus: Focus) -> Arrival {
    match setting {
        _ if focus.in_list(Pane::Difference) => Arrival::Focus,
        FocusAfterComparison::Always => Arrival::Focus,
        FocusAfterComparison::Never => Arrival::Replace,
        FocusAfterComparison::IfNotMoved if focus.unmoved || focus.progress => Arrival::Focus,
        FocusAfterComparison::IfNotMoved => Arrival::Replace,
    }
}

/// What happens when the comparison of «Знімок + порівняння» ends (TASK-12 #5, #7): as
/// after any comparison while the snapshot's profile is open, `of_open_profile`, with the
/// focus counted from the command's start. Another profile opened meanwhile has moved the
/// focus, so it goes to the difference only with «Завжди», which opens the snapshot's
/// profile again, or when it is in the difference list already (TASK-11 #21).
pub fn arrival_after_snapshot(
    setting: FocusAfterComparison,
    focus: Focus,
    of_open_profile: bool,
) -> Arrival {
    match setting {
        _ if of_open_profile => arrival(setting, focus),
        FocusAfterComparison::Always => Arrival::OpenProfileAndFocus,
        _ if focus.in_list(Pane::Difference) => Arrival::Focus,
        _ => Arrival::Replace,
    }
}

#[cfg(test)]
pub(crate) mod tests;
