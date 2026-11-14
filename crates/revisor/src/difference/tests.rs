use super::*;
use crate::i18n::{En, Uk};
use revisor_core::compare::{FolderDifference, Side};
use revisor_core::diff::Diff;
use revisor_core::scan::{FileId, Version};
use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Instant;

/// Regional formats that write numbers and moments as given: "15.3", "t20", "d1000".
struct Plain;

impl Regional for Plain {
    fn number(&self, digits: &str, _: u32) -> String {
        digits.to_string()
    }

    fn file_time(&self, file_time: i64) -> String {
        format!("t{file_time}")
    }

    fn file_time_fraction(&self, file_time: i64, digits: u32) -> String {
        format!("t{file_time}/{digits}")
    }

    fn date_time(&self, unix_millis: i64) -> String {
        format!("d{unix_millis}")
    }

    fn attribute_letter(&self, flag: u32) -> Option<String> {
        explorer_letter(flag)
    }
}

/// Regional formats as the Ukrainian region writes numbers: digits in groups of three
/// split by a space, and a decimal comma.
struct Ukrainian;

impl Regional for Ukrainian {
    fn number(&self, digits: &str, _: u32) -> String {
        let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
        let mut grouped = String::new();
        for (i, digit) in whole.chars().enumerate() {
            if i > 0 && (whole.len() - i) % 3 == 0 {
                grouped.push(' ');
            }
            grouped.push(digit);
        }
        if fraction.is_empty() {
            grouped
        } else {
            format!("{grouped},{fraction}")
        }
    }

    fn file_time(&self, file_time: i64) -> String {
        format!("t{file_time}")
    }

    fn file_time_fraction(&self, file_time: i64, digits: u32) -> String {
        format!("t{file_time}/{digits}")
    }

    fn date_time(&self, unix_millis: i64) -> String {
        format!("d{unix_millis}")
    }

    fn attribute_letter(&self, flag: u32) -> Option<String> {
        explorer_letter(flag)
    }
}

/// The letters of the column «Атрибути» of File Explorer on Windows 10 and 11, as Windows
/// gives them; a sparse file has none.
pub(crate) fn explorer_letter(flag: u32) -> Option<String> {
    let letter = match flag {
        0x1 => "R",
        0x2 => "H",
        0x4 => "S",
        0x20 => "A",
        0x100 => "T",
        0x400 => "L",
        0x800 => "C",
        0x1000 => "O",
        0x2000 => "I",
        0x4000 => "E",
        0x8_0000 => "P",
        0x10_0000 => "U",
        0x40_0000 => "M",
        _ => return None,
    };
    Some(letter.to_string())
}

fn entry(parent: Option<u32>, name: &str, kind: EntryKind) -> Entry {
    Entry {
        parent,
        name: OsString::from(name).into_boxed_os_str(),
        kind,
        inaccessible: false,
        attributes: 0,
        size: 0,
        created: 0,
        modified: 0,
        file_id: FileId::default(),
        version: VersionInfo::NotRead,
        link_target: None,
    }
}

/// A made-up difference of one folder: its two trees and its rows, given in tree order
/// by the path below the root.
struct Made {
    path: PathBuf,
    old: Vec<Entry>,
    new: Vec<Entry>,
    diff: Diff,
}

impl Made {
    fn new(root: &str) -> Self {
        Made {
            path: PathBuf::from(root),
            old: vec![entry(None, root, EntryKind::Dir)],
            new: vec![entry(None, root, EntryKind::Dir)],
            diff: Diff::default(),
        }
    }

    /// Puts the entry at `rel` in `tree`, with the folders on the way to it.
    fn put(tree: &mut Vec<Entry>, rel: &str, kind: EntryKind) -> u32 {
        let mut parent = 0u32;
        let parts: Vec<&str> = rel.split('\\').collect();
        for (i, part) in parts.iter().enumerate() {
            let last = i == parts.len() - 1;
            let found = tree
                .iter()
                .position(|e| e.parent == Some(parent) && &*e.name == OsStr::new(part));
            parent = match found {
                Some(at) if !last => at as u32,
                _ => {
                    let kind = if last { kind } else { EntryKind::Dir };
                    tree.push(entry(Some(parent), part, kind));
                    (tree.len() - 1) as u32
                }
            };
        }
        parent
    }

    fn change(&mut self, kind: ChangeKind, old: Option<u32>, new: Option<u32>, fields: Fields) {
        self.diff.changes.push(Change {
            kind,
            old,
            new,
            fields,
            old_in: None,
        });
    }

    /// An entry in both trees that has not changed.
    fn same(&mut self, rel: &str, kind: EntryKind) -> (u32, u32) {
        (
            Self::put(&mut self.old, rel, kind),
            Self::put(&mut self.new, rel, kind),
        )
    }

    fn added(&mut self, rel: &str, kind: EntryKind, size: u64) -> u32 {
        let b = Self::put(&mut self.new, rel, kind);
        self.new[b as usize].size = size;
        self.change(ChangeKind::Added, None, Some(b), Fields::default());
        b
    }

    fn deleted(&mut self, rel: &str, kind: EntryKind, size: u64) -> u32 {
        let a = Self::put(&mut self.old, rel, kind);
        self.old[a as usize].size = size;
        self.change(ChangeKind::Deleted, Some(a), None, Fields::default());
        a
    }

    fn modified(
        &mut self,
        rel: &str,
        kind: EntryKind,
        sizes: (u64, u64),
        fields: Fields,
    ) -> (u32, u32) {
        let (a, b) = self.same(rel, kind);
        self.old[a as usize].size = sizes.0;
        self.new[b as usize].size = sizes.1;
        self.change(ChangeKind::Modified, Some(a), Some(b), fields);
        (a, b)
    }

    /// An entry inaccessible in at least one tree: in a tree with `Some`, inaccessible
    /// there as it says, absent from it with `None`.
    fn inaccessible(&mut self, rel: &str, kind: EntryKind, old: Option<bool>, new: Option<bool>) {
        let at = |tree: &mut Vec<Entry>, inaccessible: Option<bool>| {
            inaccessible.map(|flag| {
                let i = Self::put(tree, rel, kind);
                tree[i as usize].inaccessible = flag;
                i
            })
        };
        let (old, new) = (at(&mut self.old, old), at(&mut self.new, new));
        let changes_before = self.diff.changes.len() as u32;
        self.diff.inaccessible.push(Inaccessible {
            old,
            new,
            changes_before,
            old_in: None,
        });
    }

    fn done(self) -> FolderDifference {
        FolderDifference {
            path: self.path,
            old: self.old,
            new: self.new,
            diff: self.diff,
        }
    }
}

fn info(id: i64, comment: Option<&str>) -> SnapshotInfo {
    SnapshotInfo {
        id,
        taken_at: id * 1000,
        comment: comment.map(str::to_string),
    }
}

fn difference(folders: Vec<FolderDifference>, warnings: Vec<Warning>) -> Difference {
    Difference {
        earlier: info(1, Some("перед встановленням X")),
        later: info(2, None),
        folders,
        warnings,
    }
}

fn programs() -> ProfileFile {
    ProfileFile {
        path: PathBuf::from(r"C:\revisor\data\profiles\Програми.db"),
        name: "Програми".into(),
    }
}

fn list_with(s: &'static dyn Strings, regional: Box<dyn Regional>) -> List {
    List::new(s, regional, View::default(), SizeFormat::Explorer)
}

/// A list in English with plain numbers, showing `difference`.
fn showing(difference: Difference) -> List {
    let mut list = list_with(&En, Box::new(Plain));
    list.replace(difference, programs());
    list
}

/// The paths of the rows the list shows, in their order.
fn paths(list: &List) -> Vec<String> {
    (0..list.len()).map(|i| list.path_of(i)).collect()
}

fn column(list: &List, column: Column) -> Vec<String> {
    (0..list.len()).map(|i| list.cell(i, column)).collect()
}

/// A folder with a bit of everything, in tree order: an exe modified, a file added and
/// one deleted under `app`, a folder inaccessible later, a link that became a folder, a
/// file added at the end.
fn sample() -> FolderDifference {
    let mut made = Made::new(r"C:\Data");
    made.same("app", EntryKind::Dir);
    made.modified(r"app\app.exe", EntryKind::File, (100, 150), Fields::SIZE);
    made.added(r"app\new.txt", EntryKind::File, 7);
    made.deleted(r"app\old.txt", EntryKind::File, 10);
    made.inaccessible("docs", EntryKind::Dir, Some(false), Some(true));
    made.deleted("link", EntryKind::Link, 0);
    made.added("link", EntryKind::Dir, 0);
    made.added("z.txt", EntryKind::File, 3);
    made.done()
}

#[test]
fn the_rows_follow_the_tree_with_the_inaccessible_entries_in_their_places() {
    let list = showing(difference(vec![sample()], Vec::new()));
    assert_eq!(
        paths(&list),
        [
            r"C:\Data\app\app.exe",
            r"C:\Data\app\new.txt",
            r"C:\Data\app\old.txt",
            r"C:\Data\docs",
            r"C:\Data\link",
            r"C:\Data\link",
            r"C:\Data\z.txt",
        ]
    );
    // A kind that changed: deleted, then added, each saying what it was (#14, #20).
    assert_eq!(
        column(&list, Column::Change),
        [
            "Modified",
            "Added",
            "Deleted",
            "Inaccessible folder",
            "Link deleted",
            "Folder added",
            "Added",
        ]
    );
    assert_eq!(list.len(), 7);
    assert_eq!(list.total(), 7);
}

#[test]
fn the_folders_of_a_difference_come_by_their_paths() {
    let mut work = Made::new(r"D:\Work");
    work.added("a.txt", EntryKind::File, 1);
    let mut users = Made::new(r"C:\Users\me");
    users.added("b.txt", EntryKind::File, 1);
    let mut users_x = Made::new(r"C:\Users X");
    users_x.added("c.txt", EntryKind::File, 1);
    // In the order of the later snapshot's folders, as the comparison gives them.
    let list = showing(difference(
        vec![work.done(), users_x.done(), users.done()],
        Vec::new(),
    ));
    // By path, part by part: «C:\Users X» comes after «C:\Users\…», as in a tree.
    assert_eq!(
        paths(&list),
        [r"C:\Users\me\b.txt", r"C:\Users X\c.txt", r"D:\Work\a.txt"]
    );
}

#[test]
fn each_column_says_its_part_of_the_entry_of_the_later_snapshot_or_the_earlier() {
    let mut made = Made::new(r"C:\Data");
    // The root's own row: the Data folder itself, whose attributes changed.
    made.change(ChangeKind::Modified, Some(0), Some(0), Fields::ATTRIBUTES);
    let gone = made.deleted("gone.dll", EntryKind::File, 2048);
    let (kept_before, kept) =
        made.modified("kept.txt", EntryKind::File, (5, 1_234_567), Fields::SIZE);
    let mut folder = made.done();
    folder.old[gone as usize].modified = 11;
    folder.new[kept as usize].modified = 22;
    folder.old[kept_before as usize].modified = 21;
    let list = showing(difference(vec![folder], Vec::new()));
    assert_eq!(
        column(&list, Column::Name),
        ["Data", "gone.dll", "kept.txt"]
    );
    assert_eq!(
        column(&list, Column::Folder),
        [r"C:\", r"C:\Data", r"C:\Data"]
    );
    assert_eq!(
        column(&list, Column::Change),
        ["Folder modified", "Deleted", "Modified"]
    );
    // Sizes only for files; the date of the later snapshot, of the earlier one for the
    // deleted file (#13).
    assert_eq!(column(&list, Column::Size), ["", "2.00 KB", "1.17 MB"]);
    assert_eq!(column(&list, Column::DateModified), ["t0", "t11", "t22"]);
    assert_eq!(
        column(&list, Column::WhatChanged),
        ["attributes", "", "size"]
    );
    // Out of the rows, nothing.
    assert_eq!(list.cell(3, Column::Name), "");
}

#[test]
fn what_changed_names_the_fields_the_version_first_and_says_replaced_apart() {
    let mut made = Made::new(r"C:\Data");
    let version = |file: u16, product: u16| {
        VersionInfo::Some(Version {
            file: [1, 0, file, 0],
            product: [1, 0, product, 0],
        })
    };
    let (a, b) = made.modified(
        "app.exe",
        EntryKind::File,
        (1, 2),
        Fields::SIZE | Fields::MODIFIED | Fields::VERSION | Fields::NAME,
    );
    let (c, d) = made.modified(
        "lib.dll",
        EntryKind::File,
        (1, 1),
        Fields::VERSION | Fields::FILE_ID | Fields::CREATED,
    );
    made.modified("same.dll", EntryKind::File, (1, 1), Fields::FILE_ID);
    made.modified("stuff", EntryKind::Dir, (0, 0), Fields::FILE_ID);
    made.modified(
        "to",
        EntryKind::Link,
        (0, 0),
        Fields::LINK_TARGET | Fields::FILE_ID,
    );
    let mut folder = made.done();
    folder.old[a as usize].version = version(1, 1);
    folder.new[b as usize].version = version(2, 1);
    folder.old[c as usize].version = version(1, 1);
    folder.new[d as usize].version = version(1, 2);
    let list = showing(difference(vec![folder], Vec::new()));
    assert_eq!(
        column(&list, Column::WhatChanged),
        [
            "file version, size, date modified, case of the name",
            "product version, date created",
            "",
            "",
            "link target",
        ]
    );
    assert_eq!(
        column(&list, Column::Change),
        [
            "Modified",
            "Replaced",
            "Replaced",
            "Folder replaced",
            "Link replaced"
        ]
    );
}

#[test]
fn the_details_and_the_path_are_of_the_row_at_a_place_of_the_list() {
    // TASK-14: a modified file, folders inaccessible in one snapshot, unknown in one, and
    // inaccessible in one and gone from the other, then a file added.
    let mut made = Made::new(r"C:\Data");
    made.modified("app.exe", EntryKind::File, (100, 150), Fields::SIZE);
    made.inaccessible("docs", EntryKind::Dir, Some(false), Some(true));
    made.inaccessible("plugins", EntryKind::Dir, Some(false), None);
    made.inaccessible("locked", EntryKind::Dir, Some(true), None);
    made.added("z.txt", EntryKind::File, 3);
    let mut list = showing(difference(vec![made.done()], Vec::new()));
    let details = list.details(0).unwrap();
    assert_eq!(details.name, "app.exe");
    assert_eq!(
        details.text.lines().collect::<Vec<_>>(),
        [
            r"Path: C:\Data\app.exe",
            "Change: modified; what changed: size",
            "Kind: file",
            "Size: before 100 bytes, after 150 bytes",
            "Date created: t0 (unchanged)",
            "Date modified: t0 (unchanged)",
            "Attributes: no attributes (unchanged)",
            "File version: version not read (unchanged)",
            "Product version: version not read (unchanged)",
        ]
    );
    let last_line = |list: &List, index: usize| {
        let text = list.details(index).unwrap().text;
        text.lines().last().unwrap().to_string()
    };
    assert_eq!(
        last_line(&list, 1),
        "Inaccessible: in the later snapshot—the contents of the folder could not be read."
    );
    assert_eq!(
        last_line(&list, 2),
        "Inaccessible: in the later snapshot—whether it was there is unknown: a folder on \
         the way to it could not be read or is a link."
    );
    assert_eq!(
        last_line(&list, 3),
        "Inaccessible: in the earlier snapshot—the contents of the folder could not be \
         read; the later snapshot does not have it."
    );
    // An added file is not inaccessible; nor is a modified one readable in both.
    assert_eq!(last_line(&list, 4), "Attributes: no attributes");
    // The path of the entry, from the later snapshot where it is there.
    assert_eq!(list.entry_path(4), Some(PathBuf::from(r"C:\Data\z.txt")));
    assert_eq!(list.entry_path(3), Some(PathBuf::from(r"C:\Data\locked")));
    assert_eq!(list.entry_path(5), None);
    assert_eq!(list.details(5), None);
    // The row at a place of the list as filtered: the inaccessible ones only.
    list.set_types([false, false, false, false, true]);
    assert_eq!(list.details(0).unwrap().name, "docs");
    assert_eq!(list.entry_path(0), Some(PathBuf::from(r"C:\Data\docs")));
}

#[test]
fn the_path_to_copy_is_the_whole_path_of_the_change_in_focus() {
    // TASK-41 #2, #5: from the later snapshot where the entry is there, else from the
    // earlier, with no quotes; renamed and moved entries are in `tests::moved`.
    let mut list = showing(difference(vec![sample()], Vec::new()));
    let mut copied = |index| {
        list.focus(index);
        list.current_path()
    };
    let path = |text: &str| Some(PathBuf::from(text));
    assert_eq!(copied(0), path(r"C:\Data\app\app.exe"), "modified");
    assert_eq!(copied(1), path(r"C:\Data\app\new.txt"), "added");
    assert_eq!(copied(2), path(r"C:\Data\app\old.txt"), "deleted");
    assert_eq!(copied(3), path(r"C:\Data\docs"), "inaccessible");
    assert_eq!(copied(4), path(r"C:\Data\link"), "a link deleted");
    assert_eq!(
        copied(5),
        path(r"C:\Data\link"),
        "a folder added in its place"
    );
    // An entry inaccessible in the earlier snapshot only, absent from the later.
    let mut made = Made::new(r"C:\Data");
    made.inaccessible("locked", EntryKind::Dir, Some(true), None);
    let list = showing(difference(vec![made.done()], Vec::new()));
    assert_eq!(list.current_path(), path(r"C:\Data\locked"));
    // No difference, nothing to copy.
    assert_eq!(list_with(&En, Box::new(Plain)).current_path(), None);
}

#[test]
fn an_inaccessible_entry_that_is_also_renamed_shows_what_changed_in_its_details() {
    // A folder inaccessible later whose name changed case is two rows: inaccessible, and
    // renamed (TASK-42); the details of both name the change of the name (TASK-14 #4).
    let mut made = Made::new(r"C:\Data");
    made.inaccessible("docs", EntryKind::Dir, Some(false), Some(true));
    let at = |tree: &[Entry]| {
        tree.iter()
            .position(|e| &*e.name == OsStr::new("docs"))
            .unwrap() as u32
    };
    let (a, b) = (at(&made.old), at(&made.new));
    made.new[b as usize].name = OsString::from("Docs").into_boxed_os_str();
    made.change(ChangeKind::Moved, Some(a), Some(b), Fields::NAME);
    let list = showing(difference(vec![made.done()], Vec::new()));
    assert_eq!(list.len(), 2);
    assert_eq!(
        column(&list, Column::Change),
        ["Inaccessible folder", "Folder renamed"]
    );
    for index in 0..2 {
        let text = list.details(index).unwrap().text;
        assert_eq!(
            text.lines().nth(1),
            Some("Name: before docs, after Docs"),
            "{text}"
        );
    }
}

#[test]
fn the_details_write_attributes_as_the_setting_says() {
    let mut made = Made::new(r"C:\Data");
    let b = made.added("secret.txt", EntryKind::File, 1);
    made.new[b as usize].attributes = 0x2 | 0x20;
    let mut list = showing(difference(vec![made.done()], Vec::new()));
    let attributes = |list: &List| {
        let text = list.details(0).unwrap().text;
        text.lines().last().unwrap().to_string()
    };
    assert_eq!(attributes(&list), "Attributes: hidden, archive");
    list.set_attribute_format(AttributeFormat::Letters);
    assert_eq!(list.attribute_format(), AttributeFormat::Letters);
    assert_eq!(attributes(&list), "Attributes: HA");
    list.set_attribute_format(AttributeFormat::LettersWithCommas);
    assert_eq!(attributes(&list), "Attributes: H, A");
}

#[test]
fn an_inaccessible_entry_says_in_which_snapshot_it_is_so() {
    let mut made = Made::new(r"C:\Data");
    made.inaccessible("a", EntryKind::Dir, Some(true), Some(false));
    made.inaccessible("b", EntryKind::Dir, Some(false), Some(true));
    made.inaccessible("c", EntryKind::Dir, Some(true), Some(true));
    // Only in one tree: inaccessible there, or unknown in the other snapshot.
    made.inaccessible("d", EntryKind::Link, None, Some(true));
    made.inaccessible("e", EntryKind::File, None, Some(false));
    made.inaccessible("f", EntryKind::File, Some(false), None);
    let list = showing(difference(vec![made.done()], Vec::new()));
    assert_eq!(
        column(&list, Column::WhatChanged),
        [
            "in the earlier snapshot",
            "in the later snapshot",
            "in both snapshots",
            "in the later snapshot",
            "in the earlier snapshot",
            "in the later snapshot",
        ]
    );
    assert_eq!(
        column(&list, Column::Change),
        [
            "Inaccessible folder",
            "Inaccessible folder",
            "Inaccessible folder",
            "Inaccessible link",
            "Inaccessible",
            "Inaccessible",
        ]
    );
    let mut uk = list_with(&Uk, Box::new(Plain));
    let mut made = Made::new(r"C:\Data");
    made.inaccessible("a", EntryKind::Dir, Some(true), Some(false));
    uk.replace(difference(vec![made.done()], Vec::new()), programs());
    assert_eq!(uk.cell(0, Column::Change), "Недоступна тека");
    assert_eq!(uk.cell(0, Column::WhatChanged), "у ранішому знімку");
}

/// The difference of the example of the task: 12 added, 3 deleted, 5 modified, 2 of them
/// replaced, 2 inaccessible, 15.3 MB more.
fn example(warnings: Vec<Warning>) -> Difference {
    let mut made = Made::new(r"C:\Data");
    for i in 0..12 {
        made.added(&format!("a{i:02}.bin"), EntryKind::File, 1_400_000);
    }
    for i in 0..3 {
        made.deleted(&format!("d{i}.bin"), EntryKind::File, 200_000);
    }
    for i in 0..5 {
        let fields = if i < 2 {
            Fields::SIZE | Fields::FILE_ID
        } else {
            Fields::SIZE
        };
        made.modified(
            &format!("m{i}.bin"),
            EntryKind::File,
            (100_000, 80_000),
            fields,
        );
    }
    made.inaccessible("x", EntryKind::Dir, Some(false), Some(true));
    made.inaccessible("y", EntryKind::Dir, Some(true), Some(true));
    difference(vec![made.done()], warnings)
}

#[test]
fn the_summary_counts_the_changes_by_type_and_the_size_they_add() {
    let mut list = list_with(&Uk, Box::new(Ukrainian));
    list.replace(example(Vec::new()), programs());
    // 20 changes, and 2 inaccessible entries, which are not changes (#16).
    let summary = "20 змін: додано 12, видалено 3, змінено 5, з них замінено 2. Недоступно 2. \
         Розмір збільшився на 15,3 МБ.";
    assert_eq!(list.summary().as_deref(), Some(summary));
    // The list is named by it (#3).
    assert_eq!(list.list_name(), summary);
    let mut list = list_with(&En, Box::new(Plain));
    list.replace(example(Vec::new()), programs());
    assert_eq!(
        list.summary().unwrap(),
        "20 changes: 12 added, 3 deleted, 5 modified, 2 of them replaced. 2 inaccessible. \
         Size increased by 15.3 MB."
    );
}

#[test]
fn the_summary_names_only_the_types_there_are_and_how_the_size_went() {
    let summary = |made: Made| {
        let mut list = list_with(&Uk, Box::new(Ukrainian));
        list.replace(difference(vec![made.done()], Vec::new()), programs());
        list.summary().unwrap()
    };
    let mut made = Made::new(r"C:\Data");
    made.deleted("a.txt", EntryKind::File, 1536);
    assert_eq!(
        summary(made),
        "1 зміна: видалено 1. Розмір зменшився на 1,50 КБ."
    );
    let mut made = Made::new(r"C:\Data");
    made.modified("a.txt", EntryKind::File, (5, 5), Fields::MODIFIED);
    made.modified("b.txt", EntryKind::File, (5, 5), Fields::MODIFIED);
    made.added("c", EntryKind::Dir, 0);
    assert_eq!(
        summary(made),
        "3 зміни: додано 1, змінено 2. Розмір не змінився."
    );
    // No changes: said so, also when there are inaccessible entries, which the list then
    // holds alone (#10).
    assert_eq!(summary(Made::new(r"C:\Data")), "Змін немає.");
    let mut made = Made::new(r"C:\Data");
    made.inaccessible("x", EntryKind::Dir, Some(true), Some(true));
    made.inaccessible("y", EntryKind::Dir, Some(true), Some(true));
    assert_eq!(summary(made), "Змін немає. Недоступно 2.");
}

/// One warning of each kind, as the comparison gives them.
fn warnings() -> Vec<Warning> {
    vec![
        Warning::FolderOnlyIn {
            path: PathBuf::from(r"D:\Games"),
            side: Side::Earlier,
        },
        Warning::FolderNarrowed {
            wider: PathBuf::from(r"C:\Users"),
            side: Side::Later,
            compared: PathBuf::from(r"C:\Users\me"),
        },
        Warning::FolderMissing {
            path: PathBuf::from(r"E:\Data"),
            side: Side::Later,
        },
        Warning::FolderUnavailable {
            path: PathBuf::from(r"F:\Old"),
            side: Side::Earlier,
            reason: revisor_core::store::Unavailable::Network,
        },
        Warning::DifferentVolumes {
            path: PathBuf::from(r"G:\"),
            earlier: 1,
            later: 2,
        },
        Warning::ExclusionOnlyIn {
            pattern: "*.tmp".into(),
            side: Side::Later,
        },
        Warning::ExclusionAppliedDifferently {
            pattern: r"%TEMP%\*".into(),
            earlier: Some(r"C:\Users\a\Temp\*".into()),
            later: None,
        },
    ]
}

#[test]
fn the_field_holds_the_pair_the_summary_and_a_line_for_each_warning() {
    let mut list = list_with(&Uk, Box::new(Ukrainian));
    assert_eq!(list.comparison_text(), None);
    assert_eq!(list.summary(), None);
    assert_eq!(list.list_name(), "Різниці ще немає");
    list.replace(example(warnings()), programs());
    let text = list.comparison_text().unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(
        lines,
        [
            "Програми: з d1000, «перед встановленням X», до d2000",
            "20 змін: додано 12, видалено 3, змінено 5, з них замінено 2. Недоступно 2. \
             Розмір збільшився на 15,3 МБ. 7 попереджень.",
            r"Тека «D:\Games» є лише в ранішому знімку, тож її не порівнювали.",
            r"У пізнішому знімку тека «C:\Users» ширша, тож порівнювали лише її частину «C:\Users\me», яка є в обох знімках.",
            r"Теки «E:\Data» немає в пізнішому знімку, тож її вміст показано видаленим.",
            r"Теку «F:\Old» не вдалося прочитати в ранішому знімку (тека на мережевому ресурсі), тож її вміст не порівнювали.",
            r"Тека «G:\» у знімках лежить на різних томах, тож змін у ній може бути багато.",
            "Виняток «*.tmp» є лише в пізнішому знімку, і його застосовано до обох.",
            r"Виняток «%TEMP%\*» у ранішому знімку означав «C:\Users\a\Temp\*», а в пізнішому не діяв; застосовано обидва.",
        ]
    );
    // The count of warnings ends the summary, which names the list (#8).
    assert!(list.list_name().ends_with("7 попереджень."));
}

#[test]
fn the_line_of_the_pair_says_how_the_profile_and_the_snapshots_are_now() {
    let mut list = list_with(&Uk, Box::new(Plain));
    list.replace(difference(vec![sample()], Vec::new()), programs());
    let pair = |list: &List| {
        list.comparison_text()
            .unwrap()
            .lines()
            .next()
            .unwrap()
            .to_string()
    };
    assert_eq!(
        pair(&list),
        "Програми: з d1000, «перед встановленням X», до d2000"
    );
    // A comment changes, the earlier snapshot is deleted, the profile renamed (#5).
    list.set_snapshots(&[info(2, Some("після"))]);
    assert_eq!(
        pair(&list),
        "Програми: з d1000 (знімок видалено), «перед встановленням X», до d2000, «після»"
    );
    let renamed = ProfileFile {
        path: PathBuf::from(r"C:\revisor\data\profiles\Ігри.db"),
        name: "Ігри".into(),
    };
    list.set_profile(Some(renamed.clone()));
    assert_eq!(list.profile(), Some(&renamed));
    assert!(pair(&list).starts_with("Ігри: з d1000 (знімок видалено)"));
    // The profile deleted: its name stays, marked.
    list.set_profile(None);
    assert!(pair(&list).starts_with("Ігри (профіль видалено): з "));
    // The difference itself does not change.
    assert_eq!(list.total(), 7);
    let mut en = list_with(&En, Box::new(Plain));
    en.replace(difference(vec![sample()], Vec::new()), programs());
    en.set_snapshots(&[info(1, None)]);
    en.set_profile(None);
    assert_eq!(
        pair(&en),
        "Програми (profile deleted): from d1000 to d2000 (snapshot deleted)"
    );
}

#[test]
fn the_types_filter_the_rows_and_the_modified_hold_the_replaced() {
    let mut list = showing(example(Vec::new()));
    assert_eq!(list.len(), 22);
    let types =
        |added, deleted, modified, inaccessible| [added, deleted, modified, true, inaccessible];
    list.set_types(types(false, true, true, false));
    assert_eq!(list.len(), 8);
    assert!(
        column(&list, Column::Change)
            .iter()
            .all(|c| ["Deleted", "Modified", "Replaced"].contains(&c.as_str()))
    );
    list.set_types(types(false, false, false, true));
    assert_eq!(column(&list, Column::Change), ["Inaccessible folder"; 2]);
    // The summary still tells of the whole difference, and of how much shows (#25).
    assert_eq!(
        list.summary().unwrap(),
        "20 changes: 12 added, 3 deleted, 5 modified, 2 of them replaced. 2 inaccessible. \
         Size increased by 15.3 MB. Showing 2 of 22."
    );
    assert_eq!(list.showing(), "Showing 2 of 22.");
    list.set_types([false; 5]);
    assert_eq!(list.len(), 0);
    assert_eq!(list.list_name(), "No changes match the filter");
    assert_eq!(list.view().types, [false; 5]);
    list.set_types([true; 5]);
    assert_eq!(list.len(), 22);
    assert!(!list.summary().unwrap().contains("Showing"));
    assert_eq!(list.showing(), "Showing 22 of 22.");
}

#[test]
fn the_text_filter_finds_the_text_anywhere_in_the_path_regardless_of_case() {
    let mut list = showing(difference(vec![sample()], Vec::new()));
    let find = |list: &mut List, text: &str| {
        list.set_text(text);
        paths(list)
    };
    assert_eq!(
        find(&mut list, "APP"),
        [
            r"C:\Data\app\app.exe",
            r"C:\Data\app\new.txt",
            r"C:\Data\app\old.txt"
        ]
    );
    // Across the folder and the name, and in the folder part alone.
    assert_eq!(find(&mut list, r"p\n"), [r"C:\Data\app\new.txt"]);
    assert_eq!(find(&mut list, r"data\l"), [r"C:\Data\link"; 2]);
    assert_eq!(find(&mut list, r"c:\data\Z"), [r"C:\Data\z.txt"]);
    // * and ? are themselves, not patterns (#24).
    assert!(find(&mut list, "*.txt").is_empty());
    assert!(find(&mut list, "z?txt").is_empty());
    assert_eq!(list.list_name(), "No changes match the filter");
    assert_eq!(list.text(), "z?txt");
    // An empty field filters nothing.
    assert_eq!(find(&mut list, "").len(), 7);
    // With the types.
    list.set_types([true, false, false, false, false]);
    assert_eq!(
        find(&mut list, ".txt"),
        [r"C:\Data\app\new.txt", r"C:\Data\z.txt"]
    );
}

#[test]
fn the_text_filter_of_cyrillic_names_regardless_of_case() {
    let mut made = Made::new(r"C:\Дані");
    made.added(r"Звіт.docx", EntryKind::File, 1);
    made.added(r"Їжак.txt", EntryKind::File, 1);
    let mut list = showing(difference(vec![made.done()], Vec::new()));
    list.set_text("звіт");
    assert_eq!(paths(&list), [r"C:\Дані\Звіт.docx"]);
    list.set_text("їЖАК");
    assert_eq!(paths(&list), [r"C:\Дані\Їжак.txt"]);
    list.set_text(r"ДАНІ\");
    assert_eq!(list.len(), 2);
}

#[test]
fn the_focus_stays_on_its_change_or_goes_to_the_nearest_one_shown() {
    let mut list = showing(difference(vec![sample()], Vec::new()));
    assert_eq!(
        list.current(),
        Some(0),
        "a new difference has the first row"
    );
    // On «app\new.txt», which is added: hiding the added rows moves the focus to the
    // next row shown (#27).
    list.focus(1);
    assert_eq!(list.set_types([false, true, true, true, true]), Some(1));
    assert_eq!(list.path_of(1), r"C:\Data\app\old.txt");
    // Showing them again leaves it there.
    assert_eq!(list.set_types([true; 5]), Some(2));
    assert_eq!(list.path_of(2), r"C:\Data\app\old.txt");
    // The last row hidden: the one before it.
    list.focus(6);
    assert_eq!(list.path_of(6), r"C:\Data\z.txt");
    assert_eq!(list.set_text("link"), Some(1));
    assert_eq!(list.current(), Some(1));
    // Nothing shown: no focus; then the first row again.
    assert_eq!(list.set_text("nothing"), None);
    assert_eq!(list.set_text(""), Some(0));
}

#[test]
fn sorting_by_a_column_keeps_the_tree_order_of_equal_values() {
    let mut made = Made::new(r"C:\Data");
    made.same("b", EntryKind::Dir);
    let x = made.added(r"b\x.txt", EntryKind::File, 30);
    made.added(r"b\zeta", EntryKind::Dir, 0);
    made.same("c", EntryKind::Dir);
    made.added(r"c\x.txt", EntryKind::File, 10);
    made.deleted("Alpha.txt", EntryKind::File, 20);
    let mut folder = made.done();
    folder.new[x as usize].modified = 5;
    let mut list = showing(difference(vec![folder], Vec::new()));
    let sort = |list: &mut List, column: SortColumn, descending: bool| {
        list.set_sort(DifferenceSort { column, descending });
        paths(list)
    };
    let tree = [
        r"C:\Data\b\x.txt",
        r"C:\Data\b\zeta",
        r"C:\Data\c\x.txt",
        r"C:\Data\Alpha.txt",
    ];
    // The tree: the folders' contents come with them — the deleted Alpha.txt was in the
    // order of the comparison's walk, which this difference was given in.
    assert_eq!(sort(&mut list, SortColumn::Path, false), tree);
    let mut reversed = tree;
    reversed.reverse();
    assert_eq!(sort(&mut list, SortColumn::Path, true), reversed);
    // By name regardless of case; the two x.txt in tree order, whichever the direction.
    assert_eq!(
        sort(&mut list, SortColumn::Name, false),
        [
            r"C:\Data\Alpha.txt",
            r"C:\Data\b\x.txt",
            r"C:\Data\c\x.txt",
            r"C:\Data\b\zeta"
        ]
    );
    assert_eq!(
        sort(&mut list, SortColumn::Name, true),
        [
            r"C:\Data\b\zeta",
            r"C:\Data\b\x.txt",
            r"C:\Data\c\x.txt",
            r"C:\Data\Alpha.txt"
        ]
    );
    // By size: the folder has none, so it comes first.
    assert_eq!(
        sort(&mut list, SortColumn::Size, false),
        [
            r"C:\Data\b\zeta",
            r"C:\Data\c\x.txt",
            r"C:\Data\Alpha.txt",
            r"C:\Data\b\x.txt"
        ]
    );
    // By change: added files, an added folder, deleted files.
    assert_eq!(
        sort(&mut list, SortColumn::Change, false),
        [
            r"C:\Data\b\x.txt",
            r"C:\Data\c\x.txt",
            r"C:\Data\b\zeta",
            r"C:\Data\Alpha.txt"
        ]
    );
    // By folder: C:\Data, then b, then c; within one, in tree order.
    assert_eq!(
        sort(&mut list, SortColumn::Folder, false),
        [
            r"C:\Data\Alpha.txt",
            r"C:\Data\b\x.txt",
            r"C:\Data\b\zeta",
            r"C:\Data\c\x.txt"
        ]
    );
    // By date: the one dated 5 last, the others, dated 0, in tree order.
    assert_eq!(
        sort(&mut list, SortColumn::DateModified, false),
        [
            r"C:\Data\b\zeta",
            r"C:\Data\c\x.txt",
            r"C:\Data\Alpha.txt",
            r"C:\Data\b\x.txt"
        ]
    );
}

#[test]
fn sorting_keeps_the_focus_on_its_change() {
    let mut list = showing(difference(vec![sample()], Vec::new()));
    list.focus(6);
    assert_eq!(list.path_of(6), r"C:\Data\z.txt");
    let at = list.set_sort(DifferenceSort {
        column: SortColumn::Path,
        descending: true,
    });
    assert_eq!(at, Some(0));
    assert_eq!(list.path_of(0), r"C:\Data\z.txt");
    let at = list.set_sort(DifferenceSort {
        column: SortColumn::WhatChanged,
        descending: false,
    });
    assert_eq!(list.path_of(at.unwrap()), r"C:\Data\z.txt");
    // By the words of «Що змінилося»: nothing first, then «in the later snapshot», then
    // «size».
    assert_eq!(
        column(&list, Column::WhatChanged),
        ["", "", "", "", "", "in the later snapshot", "size"]
    );
}

#[test]
fn a_new_difference_keeps_the_view_but_not_the_text_filter() {
    let mut list = showing(difference(vec![sample()], Vec::new()));
    let sort = DifferenceSort {
        column: SortColumn::Name,
        descending: true,
    };
    list.set_types([true, true, false, true, true]);
    list.set_sort(sort);
    list.set_text("app");
    list.focus(1);
    assert_eq!(
        list.replace(difference(vec![sample()], Vec::new()), programs()),
        Some(0)
    );
    assert_eq!(list.text(), "");
    assert_eq!(list.view().types, [true, true, false, true, true]);
    assert_eq!(list.view().sort, sort);
    assert_eq!(list.len(), 6);
    assert_eq!(list.current(), Some(0));
    // A difference with no rows has no focus.
    assert_eq!(
        list.replace(difference(Vec::new(), Vec::new()), programs()),
        None
    );
    assert_eq!(list.list_name(), "No changes.");
}

#[test]
fn sizes_are_written_as_the_setting_says() {
    let size = |bytes, format| format_size(bytes, format, &Uk, &Ukrainian);
    let explorer = |bytes| size(bytes, SizeFormat::Explorer);
    // Bytes below a kilobyte, with their plural forms.
    assert_eq!(explorer(0), "0 байтів");
    assert_eq!(explorer(1), "1 байт");
    assert_eq!(explorer(3), "3 байти");
    assert_eq!(explorer(1023), "1 023 байти");
    // Three significant digits, cut off, as StrFormatByteSize's own table shows: 1340,
    // 23506, 2400016 and 2400000000 bytes.
    assert_eq!(explorer(1024), "1,00 КБ");
    assert_eq!(explorer(1340), "1,30 КБ");
    assert_eq!(explorer(23_506), "22,9 КБ");
    assert_eq!(explorer(2_400_016), "2,28 МБ");
    assert_eq!(explorer(2_400_000_000), "2,23 ГБ");
    // The examples of the task (#17), and a thousand kilobytes or more as megabytes.
    assert_eq!(explorer(1_234_567), "1,17 МБ");
    assert_eq!(size(1_234_567, SizeFormat::Decimal), "1,23 МБ");
    assert_eq!(size(1_234_567, SizeFormat::Bytes), "1 234 567 байтів");
    assert_eq!(explorer(1_030_000), "0,98 МБ");
    assert_eq!(explorer(1_000_000), "976 КБ");
    assert_eq!(size(999, SizeFormat::Decimal), "999 байтів");
    assert_eq!(size(1000, SizeFormat::Decimal), "1,00 КБ");
    assert_eq!(explorer(u64::MAX), "15,9 ЕБ");
    assert_eq!(size(21, SizeFormat::Bytes), "21 байт");
    let en = |bytes, format| format_size(bytes, format, &En, &Plain);
    assert_eq!(en(1, SizeFormat::Bytes), "1 byte");
    assert_eq!(en(2, SizeFormat::Explorer), "2 bytes");
    assert_eq!(en(1_234_567, SizeFormat::Explorer), "1.17 MB");
    assert_eq!(en(5 << 40, SizeFormat::Explorer), "5.00 TB");
}

#[test]
fn the_size_setting_writes_the_column_and_the_summary() {
    let mut list = list_with(&Uk, Box::new(Ukrainian));
    list.replace(example(Vec::new()), programs());
    list.set_size_format(SizeFormat::Bytes);
    assert_eq!(list.size_format(), SizeFormat::Bytes);
    assert!(
        list.summary()
            .unwrap()
            .ends_with("Розмір збільшився на 16 100 000 байтів.")
    );
    assert_eq!(list.cell(0, Column::Size), "1 400 000 байтів");
}

/// A difference of `rows` rows over a generated tree: added, deleted, modified and
/// inaccessible in turn, each type with every kind of entry.
fn generated(rows: usize) -> Difference {
    let tree = revisor_core::scan::synthetic(rows + 1);
    let mut diff = Diff::default();
    for i in 1..=rows as u32 {
        let (kind, old, new) = match i % 4 {
            0 => (Some(ChangeKind::Added), None, Some(i)),
            1 => (Some(ChangeKind::Deleted), Some(i), None),
            2 => (Some(ChangeKind::Modified), Some(i), Some(i)),
            _ => (None, Some(i), Some(i)),
        };
        match kind {
            Some(kind) => diff.changes.push(Change {
                kind,
                old,
                new,
                fields: if kind == ChangeKind::Modified {
                    Fields::SIZE | Fields(1 << (i % 8))
                } else {
                    Fields::default()
                },
                old_in: None,
            }),
            None => diff.inaccessible.push(Inaccessible {
                old,
                new,
                changes_before: diff.changes.len() as u32,
                old_in: None,
            }),
        }
    }
    let path = PathBuf::from(&*tree[0].name);
    difference(
        vec![FolderDifference {
            path,
            old: tree.clone(),
            new: tree,
            diff,
        }],
        Vec::new(),
    )
}

#[test]
fn the_text_filter_finds_the_same_rows_on_several_threads() {
    // Enough rows for the filter to check them on several threads.
    let mut list = showing(generated(4 * ROWS_PER_THREAD + 5));
    let every: Vec<String> = paths(&list);
    for text in ["000123", r"c\folder 0", "LOG", r"0\file"] {
        let wanted: Vec<&String> = every
            .iter()
            .filter(|path| path.to_uppercase().contains(&text.to_uppercase()))
            .collect();
        list.set_text(text);
        let found = paths(&list);
        assert_eq!(found.len(), wanted.len(), "{text}");
        assert!(found.iter().zip(&wanted).all(|(a, b)| a == *b), "{text}");
    }
}

/// Times what the list does on ~1 million rows (#29), in a release build:
/// `cargo test --release -p revisor -- --ignored --nocapture the_list_keeps_up`.
#[test]
#[ignore = "a measurement, for a release build"]
fn the_list_keeps_up_with_a_million_rows() {
    let rows = 1_000_000;
    let difference = generated(rows);
    let mut list = list_with(&Uk, Box::new(Ukrainian));
    let time = |what: &str, act: &mut dyn FnMut()| {
        let started = Instant::now();
        act();
        let took = started.elapsed();
        println!("{what}: {:.3} s", took.as_secs_f64());
        took
    };
    let mut worst = std::time::Duration::ZERO;
    let mut note = |took| worst = worst.max(took);
    time("arrival", &mut || {
        list.replace(generated(0), programs());
    });
    let mut difference = Some(difference);
    note(time("arrival of a million rows", &mut || {
        list.replace(difference.take().unwrap(), programs());
    }));
    assert_eq!(list.len(), rows);
    // The first letter typed indexes the names and the folders.
    note(time("first letter, indexing", &mut || {
        list.set_text("f");
    }));
    list.set_text("");
    list.focus(rows / 2);
    for column in SortColumn::ALL {
        for descending in [false, true] {
            note(time(
                &format!("sort by {column:?}, descending {descending}"),
                &mut || {
                    list.set_sort(DifferenceSort { column, descending });
                },
            ));
        }
    }
    list.set_sort(DifferenceSort::default());
    for types in [
        [false, true, true, true, true],
        [true, false, false, false, false],
        [true; 5],
    ] {
        note(time(&format!("types {types:?}"), &mut || {
            list.set_types(types);
        }));
    }
    // Typing a name letter by letter: the first letter indexes the names and folders.
    let mut typed = String::new();
    for letter in "file 0004".chars() {
        typed.push(letter);
        let text = typed.clone();
        note(time(&format!("text {text:?}"), &mut || {
            list.set_text(&text);
        }));
    }
    println!("shown: {} of {}", list.len(), list.total());
    note(time("text cleared", &mut || {
        list.set_text("");
    }));
    note(time("a screen of cells", &mut || {
        for row in 0..40 {
            for column in Column::ALL {
                list.cell(row, column);
            }
        }
    }));
    println!("worst: {:.3} s", worst.as_secs_f64());
    assert!(worst.as_secs_f64() < 2.0, "{worst:?}");
}

// --- Where the focus goes when a difference arrives (TASK-11 #12, #13, #21; TASK-12) ---

mod arrival_of_a_difference {
    use super::super::{Arrival, arrival, arrival_after_snapshot};
    use crate::commands::Pane;
    use crate::keyboard::{Control, Focus};
    use revisor_core::settings::FocusAfterComparison::{self, *};

    const DIFFERENCE_LIST: Focus = Focus::new(Some(Pane::Difference), Control::MainList);
    const UNMOVED: Focus = Focus {
        unmoved: true,
        ..Focus::new(Some(Pane::Snapshots), Control::MainList)
    };
    const PROGRESS: Focus = Focus {
        progress: true,
        ..Focus::new(None, Control::Other)
    };
    const ELSEWHERE: Focus = Focus::new(Some(Pane::Snapshots), Control::MainList);
    const SETTINGS: [FocusAfterComparison; 3] = [Never, IfNotMoved, Always];

    #[test]
    fn a_difference_takes_the_focus_as_the_setting_says() {
        for setting in SETTINGS {
            // In the difference list the focus comes to the new list in any case (#21).
            assert_eq!(arrival(setting, DIFFERENCE_LIST), Arrival::Focus);
        }
        for focus in [UNMOVED, PROGRESS, ELSEWHERE] {
            assert_eq!(arrival(Always, focus), Arrival::Focus);
            assert_eq!(arrival(Never, focus), Arrival::Replace);
        }
        assert_eq!(arrival(IfNotMoved, UNMOVED), Arrival::Focus);
        assert_eq!(arrival(IfNotMoved, PROGRESS), Arrival::Focus);
        assert_eq!(arrival(IfNotMoved, ELSEWHERE), Arrival::Replace);
    }

    #[test]
    fn the_difference_of_snapshot_and_compare_opens_its_profile_only_with_always() {
        // Of the open profile: as after any comparison.
        for setting in SETTINGS {
            for focus in [DIFFERENCE_LIST, UNMOVED, PROGRESS, ELSEWHERE] {
                assert_eq!(
                    arrival_after_snapshot(setting, focus, true),
                    arrival(setting, focus)
                );
            }
        }
        // Another profile is open: «Завжди» opens the snapshot's profile again; else the
        // focus goes to the difference only from the difference list.
        for focus in [DIFFERENCE_LIST, UNMOVED, PROGRESS, ELSEWHERE] {
            assert_eq!(
                arrival_after_snapshot(Always, focus, false),
                Arrival::OpenProfileAndFocus
            );
        }
        for setting in [Never, IfNotMoved] {
            assert_eq!(
                arrival_after_snapshot(setting, DIFFERENCE_LIST, false),
                Arrival::Focus
            );
            for focus in [UNMOVED, PROGRESS, ELSEWHERE] {
                assert_eq!(
                    arrival_after_snapshot(setting, focus, false),
                    Arrival::Replace
                );
            }
        }
    }
}

// --- The difference as a tree (TASK-33) ---------------------------------------------

mod tree;

// --- Renamed and moved entries (TASK-42) -----------------------------------------------

mod moved;

// --- The text report (TASK-39) ----------------------------------------------------

mod report;

// --- CSV (TASK-40) -------------------------------------------------------------------

mod csv;
