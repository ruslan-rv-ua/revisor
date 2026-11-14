//! Renamed and moved entries in the difference (TASK-42).

use super::tree::{outline, tree_showing};
use super::*;

const NAME: Fields = Fields::NAME;
const FOLDER: Fields = Fields::FOLDER;

impl Made {
    /// An entry that was at `was` in the earlier tree and is at `is` in the later one:
    /// renamed or moved, with the fields that say how and what else differs.
    fn moved(
        &mut self,
        was: &str,
        is: &str,
        kind: EntryKind,
        sizes: (u64, u64),
        fields: Fields,
    ) -> (u32, u32) {
        let a = Self::put(&mut self.old, was, kind);
        let b = Self::put(&mut self.new, is, kind);
        self.old[a as usize].size = sizes.0;
        self.new[b as usize].size = sizes.1;
        self.change(ChangeKind::Moved, Some(a), Some(b), fields);
        (a, b)
    }
}

/// A folder with one of each: a file and a folder renamed, a file moved and modified, a
/// link moved and renamed, a file renamed in case only, and a file moved and renamed; in
/// the order of their paths now.
fn sample() -> FolderDifference {
    let mut made = Made::new(r"C:\Data");
    made.moved(
        r"app\readme.txt",
        r"app\README.md",
        EntryKind::File,
        (5, 5),
        NAME,
    );
    made.moved(r"app\utils", r"app\tools", EntryKind::Dir, (0, 0), NAME);
    let (_, notes) = made.moved(
        "Notes.txt",
        r"docs\Notes.txt",
        EntryKind::File,
        (10, 25),
        FOLDER | Fields::SIZE | Fields::MODIFIED,
    );
    made.moved(
        "link",
        r"docs\shortcut",
        EntryKind::Link,
        (0, 0),
        NAME | FOLDER,
    );
    made.moved("setup.exe", "Setup.EXE", EntryKind::File, (1, 1), NAME);
    made.moved(
        r"app\a.exe",
        "z.exe",
        EntryKind::File,
        (2, 2),
        NAME | FOLDER,
    );
    let mut folder = made.done();
    folder.new[notes as usize].modified = 9;
    folder
}

#[test]
fn a_renamed_or_moved_entry_is_one_row_at_its_path_now_that_says_where_it_was() {
    let list = showing(difference(vec![sample()], Vec::new()));
    assert_eq!(
        paths(&list),
        [
            r"C:\Data\app\README.md",
            r"C:\Data\app\tools",
            r"C:\Data\docs\Notes.txt",
            r"C:\Data\docs\shortcut",
            r"C:\Data\Setup.EXE",
            r"C:\Data\z.exe",
        ]
    );
    // What happened, by fact (#6): another name in the same folder, also in case only;
    // another folder; or both; with the kind of a folder and of a link.
    assert_eq!(
        column(&list, Column::Change),
        [
            "Renamed",
            "Folder renamed",
            "Moved",
            "Link moved and renamed",
            "Renamed",
            "Moved and renamed",
        ]
    );
    // Only what it was: the name, the folder or the path, then the other fields (#7).
    assert_eq!(
        column(&list, Column::WhatChanged),
        [
            "from readme.txt",
            "from utils",
            r"from C:\Data; size, date modified",
            r"from C:\Data\link",
            "from setup.exe",
            r"from C:\Data\app\a.exe",
        ]
    );
    // The size and the date of the entry as it is now.
    assert_eq!(list.cell(2, Column::Size), "25 bytes");
    assert_eq!(list.cell(2, Column::DateModified), "t9");
    let mut uk = list_with(&Uk, Box::new(Ukrainian));
    uk.replace(difference(vec![sample()], Vec::new()), programs());
    assert_eq!(
        column(&uk, Column::Change),
        [
            "Перейменовано",
            "Перейменовано теку",
            "Переміщено",
            "Переміщено й перейменовано посилання",
            "Перейменовано",
            "Переміщено й перейменовано",
        ]
    );
    assert_eq!(
        column(&uk, Column::WhatChanged),
        [
            "з readme.txt",
            "з utils",
            r"з C:\Data; розмір, дата зміни",
            r"з C:\Data\link",
            "з setup.exe",
            r"з C:\Data\app\a.exe",
        ]
    );
}

#[test]
fn the_summary_names_the_renamed_or_moved_after_the_modified_and_counts_their_size() {
    let made = || {
        let mut made = Made::new(r"C:\Data");
        made.added("a.txt", EntryKind::File, 7);
        made.modified("b.txt", EntryKind::File, (5, 5), Fields::MODIFIED);
        // Moved and smaller by six bytes: one change, of the new type (#7, #10).
        made.moved(
            "c.txt",
            "d.txt",
            EntryKind::File,
            (10, 4),
            NAME | Fields::SIZE,
        );
        made.moved("e", r"sub\e", EntryKind::Dir, (0, 0), FOLDER);
        made
    };
    let mut list = list_with(&Uk, Box::new(Ukrainian));
    list.replace(difference(vec![made().done()], Vec::new()), programs());
    assert_eq!(
        list.summary().unwrap(),
        "4 зміни: додано 1, змінено 1, перейменовано чи переміщено 2. Розмір збільшився на 1 байт."
    );
    let list = showing(difference(vec![made().done()], Vec::new()));
    assert_eq!(
        list.summary().unwrap(),
        "4 changes: 1 added, 1 modified, 2 renamed or moved. Size increased by 1 byte."
    );
    // Nothing else changed: the renamed and moved alone.
    let list = showing(difference(vec![sample()], Vec::new()));
    assert_eq!(
        list.summary().unwrap(),
        "6 changes: 6 renamed or moved. Size increased by 15 bytes."
    );
}

#[test]
fn the_fourth_type_shows_or_hides_the_renamed_and_moved_and_the_modified_do_not_hold_them() {
    let mut made = Made::new(r"C:\Data");
    made.modified("a.txt", EntryKind::File, (5, 6), Fields::SIZE);
    made.moved(
        "b.txt",
        "c.txt",
        EntryKind::File,
        (5, 6),
        NAME | Fields::SIZE,
    );
    made.inaccessible("x", EntryKind::Dir, Some(false), Some(true));
    let mut list = showing(difference(vec![made.done()], Vec::new()));
    let types = |modified, moved, inaccessible| [true, true, modified, moved, inaccessible];
    assert_eq!(list.len(), 3);
    // Moved and modified is renamed or moved, not modified (#7).
    list.set_types(types(true, false, false));
    assert_eq!(paths(&list), [r"C:\Data\a.txt"]);
    list.set_types(types(false, true, false));
    assert_eq!(paths(&list), [r"C:\Data\c.txt"]);
    assert_eq!(list.showing(), "Showing 1 of 3.");
    list.set_types(types(false, false, true));
    assert_eq!(paths(&list), [r"C:\Data\x"]);
}

#[test]
fn the_text_filter_looks_in_the_path_an_entry_had_too() {
    let mut list = showing(difference(vec![sample()], Vec::new()));
    let find = |list: &mut List, text: &str| {
        list.set_text(text);
        paths(list)
    };
    // In the name and in the folder it had, and across the two.
    assert_eq!(find(&mut list, "utils"), [r"C:\Data\app\tools"]);
    assert_eq!(find(&mut list, r"data\link"), [r"C:\Data\docs\shortcut"]);
    assert_eq!(find(&mut list, r"app\a.exe"), [r"C:\Data\z.exe"]);
    assert_eq!(find(&mut list, r"Data\notes"), [r"C:\Data\docs\Notes.txt"]);
    // And where it is now, as any row.
    assert_eq!(find(&mut list, "shortcut"), [r"C:\Data\docs\shortcut"]);
    assert_eq!(
        find(&mut list, "docs"),
        [r"C:\Data\docs\Notes.txt", r"C:\Data\docs\shortcut"]
    );
    assert!(find(&mut list, "nothing").is_empty());
    assert_eq!(find(&mut list, "").len(), 6);
}

#[test]
fn sorting_puts_the_renamed_and_moved_after_the_replaced_and_by_what_they_say() {
    let mut made = Made::new(r"C:\Data");
    made.modified("a.txt", EntryKind::File, (1, 1), Fields::FILE_ID);
    made.inaccessible("b", EntryKind::Dir, Some(true), Some(true));
    made.moved(r"x\c.txt", "c.txt", EntryKind::File, (1, 1), FOLDER);
    made.moved("d.txt", "e.txt", EntryKind::File, (1, 1), NAME);
    made.moved(r"x\f.txt", "g.txt", EntryKind::File, (1, 1), NAME | FOLDER);
    made.modified("h.txt", EntryKind::File, (1, 2), Fields::SIZE);
    made.moved("i", "j", EntryKind::Dir, (0, 0), NAME);
    let mut list = showing(difference(vec![made.done()], Vec::new()));
    let sort = |list: &mut List, column: SortColumn| {
        list.set_sort(DifferenceSort {
            column,
            descending: false,
        });
        column_of(list)
    };
    fn column_of(list: &List) -> Vec<String> {
        column(list, Column::Change)
    }
    // In the order of the check boxes: modified with replaced, then the renamed, the
    // moved, those moved and renamed, files before folders, and the inaccessible last.
    assert_eq!(
        sort(&mut list, SortColumn::Change),
        [
            "Modified",
            "Replaced",
            "Renamed",
            "Folder renamed",
            "Moved",
            "Moved and renamed",
            "Inaccessible folder",
        ]
    );
    // By the words of «Що змінилося»: nothing, then the texts in their order.
    sort(&mut list, SortColumn::WhatChanged);
    assert_eq!(
        column(&list, Column::WhatChanged),
        [
            "",
            r"from C:\Data\x",
            r"from C:\Data\x\f.txt",
            "from d.txt",
            "from i",
            "in both snapshots",
            "size",
        ]
    );
}

#[test]
fn the_details_say_the_path_before_and_after_and_the_rest_as_of_a_modified_entry() {
    let mut list = showing(difference(vec![sample()], Vec::new()));
    let lines = |list: &List, index: usize| -> Vec<String> {
        let text = list.details(index).unwrap().text;
        text.lines().map(str::to_string).collect()
    };
    // Renamed: the two paths, and nothing else changed (#8).
    assert_eq!(list.details(0).unwrap().name, "README.md");
    assert_eq!(
        lines(&list, 0),
        [
            r"Path: before C:\Data\app\readme.txt, after C:\Data\app\README.md",
            "Change: renamed",
            "Kind: file",
            "Size: 5 bytes (unchanged)",
            "Date created: t0 (unchanged)",
            "Date modified: t0 (unchanged)",
            "Attributes: no attributes (unchanged)",
        ]
    );
    // Moved and modified: what else changed after the change, and the values (#7).
    assert_eq!(
        lines(&list, 2)[..5],
        [
            r"Path: before C:\Data\Notes.txt, after C:\Data\docs\Notes.txt",
            "Change: moved; what changed: size, date modified",
            "Kind: file",
            "Size: before 10 bytes, after 25 bytes",
            "Date created: t0 (unchanged)",
        ]
    );
    assert_eq!(lines(&list, 2)[5], "Date modified: before t0, after t9");
    // Only the case of the name: the path as it is, and the name before and after.
    assert_eq!(
        lines(&list, 4)[..3],
        [
            r"Path: C:\Data\Setup.EXE",
            "Name: before setup.exe, after Setup.EXE",
            "Change: renamed",
        ]
    );
    // File Explorer shows the entry where it is now (#8).
    assert_eq!(list.entry_path(5), Some(PathBuf::from(r"C:\Data\z.exe")));
    list.focus(3);
    assert_eq!(
        list.current_location(),
        Some(Location::Entry(PathBuf::from(r"C:\Data\docs\shortcut")))
    );
    let mut uk = list_with(&Uk, Box::new(Plain));
    uk.replace(difference(vec![sample()], Vec::new()), programs());
    let text = uk.details(2).unwrap().text;
    assert_eq!(
        text.lines().take(2).collect::<Vec<_>>(),
        [
            r"Шлях: було C:\Data\Notes.txt, стало C:\Data\docs\Notes.txt",
            "Зміна: переміщено; змінилося: розмір, дата зміни",
        ]
    );
    let text = uk.details(4).unwrap().text;
    assert_eq!(
        text.lines().take(3).collect::<Vec<_>>(),
        [
            r"Шлях: C:\Data\Setup.EXE",
            "Ім'я: було setup.exe, стало Setup.EXE",
            "Зміна: перейменовано",
        ]
    );
}

#[test]
fn the_node_of_a_renamed_or_moved_entry_is_where_the_entry_is_now() {
    let mut list = tree_showing(difference(vec![sample()], Vec::new()));
    assert_eq!(
        outline(&list),
        [
            r"C:\Data — 6 changes",
            "  app — 2 changes",
            "    README.md — renamed: from readme.txt *",
            "    tools — folder renamed: from utils",
            "  docs — 2 changes",
            r"    Notes.txt — moved: from C:\Data; size, date modified",
            r"    shortcut — link moved and renamed: from C:\Data\link",
            "  Setup.EXE — renamed: from setup.exe",
            r"  z.exe — moved and renamed: from C:\Data\app\a.exe",
        ]
    );
    list.set_node_text(TreeNodeText::Short);
    assert_eq!(outline(&list)[2], "    README.md — renamed *");
    // The filter by text leaves what was at a path with the text (#9).
    list.set_node_text(TreeNodeText::Long);
    list.set_text("utils");
    assert_eq!(
        outline(&list),
        [
            r"C:\Data — 1 change",
            "  app — 1 change",
            "    tools — folder renamed: from utils *",
        ]
    );
    let mut uk = list_with(&Uk, Box::new(Ukrainian));
    uk.set_layout(DifferenceView::Tree);
    uk.replace(difference(vec![sample()], Vec::new()), programs());
    assert_eq!(
        outline(&uk)[..4],
        [
            r"C:\Data — 6 змін",
            "  app — 2 зміни",
            "    README.md — перейменовано: з readme.txt *",
            "    tools — перейменовано теку: з utils",
        ]
    );
}

/// A folder moved with what it holds, in which a file was then modified and one deleted:
/// the rows as the comparison gives them, the folder first.
fn moved_folder() -> FolderDifference {
    let mut made = Made::new(r"C:\Data");
    made.same("new", EntryKind::Dir);
    made.moved(r"old\docs", r"new\docs", EntryKind::Dir, (0, 0), FOLDER);
    let was = Made::put(&mut made.old, r"old\docs\a.txt", EntryKind::File);
    let is = Made::put(&mut made.new, r"new\docs\a.txt", EntryKind::File);
    made.new[is as usize].size = 4;
    made.change(ChangeKind::Modified, Some(was), Some(is), Fields::SIZE);
    made.deleted(r"old\docs\gone.txt", EntryKind::File, 3);
    made.done()
}

#[test]
fn the_path_to_copy_of_a_renamed_or_moved_entry_is_where_it_is_now() {
    // TASK-41 #2, #5: as File Explorer shows it (#8), and in the tree as in the list.
    let mut list = showing(difference(vec![sample()], Vec::new()));
    list.focus(2);
    assert_eq!(
        list.current_path(),
        Some(PathBuf::from(r"C:\Data\docs\Notes.txt"))
    );
    list.focus(4);
    assert_eq!(
        list.current_path(),
        Some(PathBuf::from(r"C:\Data\Setup.EXE"))
    );
    // What was deleted inside a moved folder: where the folder is now (#4); the folder
    // it is in now, which did not change itself, by its path in the tree.
    let mut list = showing(difference(vec![moved_folder()], Vec::new()));
    list.focus(2);
    let gone = Some(PathBuf::from(r"C:\Data\new\docs\gone.txt"));
    assert_eq!(list.current_path(), gone);
    list.set_layout(DifferenceView::Tree);
    assert_eq!(list.current_path(), gone);
    let new = list
        .tree_roots()
        .into_iter()
        .flat_map(|root| list.tree_children(root))
        .find(|&n| list.tree_label(n).starts_with("new —"))
        .expect("the folder new is a node of its own");
    list.tree_focus(new);
    assert_eq!(list.current_path(), Some(PathBuf::from(r"C:\Data\new")));
}

#[test]
fn what_was_deleted_inside_a_moved_folder_shows_where_the_folder_is_now() {
    // #4: the folder is one row, and what changed inside it stands at the new paths.
    let mut list = showing(difference(vec![moved_folder()], Vec::new()));
    assert_eq!(
        paths(&list),
        [
            r"C:\Data\new\docs",
            r"C:\Data\new\docs\a.txt",
            r"C:\Data\new\docs\gone.txt",
        ]
    );
    assert_eq!(
        column(&list, Column::Change),
        ["Folder moved", "Modified", "Deleted"]
    );
    assert_eq!(list.cell(0, Column::WhatChanged), r"from C:\Data\old");
    assert_eq!(list.cell(2, Column::Folder), r"C:\Data\new\docs");
    assert_eq!(
        list.entry_path(2),
        Some(PathBuf::from(r"C:\Data\new\docs\gone.txt"))
    );
    assert_eq!(
        list.details(2).unwrap().text.lines().next(),
        Some(r"Path: C:\Data\new\docs\gone.txt")
    );
    // By folder and by text they go with the folder as it is now; the folder itself is
    // found by the path it had too.
    list.set_text(r"new\docs\g");
    assert_eq!(paths(&list), [r"C:\Data\new\docs\gone.txt"]);
    list.set_text("old");
    assert_eq!(paths(&list), [r"C:\Data\new\docs"]);
    list.set_text("");
    list.set_sort(DifferenceSort {
        column: SortColumn::Folder,
        descending: false,
    });
    assert_eq!(
        column(&list, Column::Folder),
        [r"C:\Data\new", r"C:\Data\new\docs", r"C:\Data\new\docs"]
    );
    // In the tree the folder holds them, and nothing stands where it was.
    list.set_sort(DifferenceSort::default());
    list.set_layout(DifferenceView::Tree);
    assert_eq!(
        outline(&list),
        [
            r"C:\Data — 3 changes",
            "  new — 3 changes",
            r"    docs — folder moved: from C:\Data\old; 2 changes *",
            "      a.txt — modified: size",
            "      gone.txt — deleted",
        ]
    );
}

#[test]
fn a_folder_deleted_inside_a_moved_folder_holds_what_it_held() {
    // What is left only in the earlier tree is a subtree of it, under the place the
    // moved folder is at now.
    let mut made = Made::new(r"C:\Data");
    made.moved("old", "new", EntryKind::Dir, (0, 0), NAME);
    made.deleted(r"old\sub", EntryKind::Dir, 0);
    made.deleted(r"old\sub\deep", EntryKind::Dir, 0);
    made.deleted(r"old\sub\deep\x.txt", EntryKind::File, 1);
    let list = tree_showing(difference(vec![made.done()], Vec::new()));
    assert_eq!(
        outline(&list),
        [
            r"C:\Data — 4 changes",
            "  new — folder renamed: from old; 3 changes *",
            "    sub — folder deleted, 2 changes",
            "      deep — folder deleted, 1 change",
            "        x.txt — deleted",
        ]
    );
    assert_eq!(
        paths(&list),
        [
            r"C:\Data\new",
            r"C:\Data\new\sub",
            r"C:\Data\new\sub\deep",
            r"C:\Data\new\sub\deep\x.txt",
        ]
    );
}

#[test]
fn an_entry_moved_between_folders_of_the_difference_is_a_row_of_the_folder_it_is_in_now() {
    // #1: the entry it was is in the earlier tree of the other folder.
    let mut a = Made::new(r"C:\A");
    let was = Made::put(&mut a.old, "x.txt", EntryKind::File);
    let mut b = Made::new(r"C:\B");
    let is = Made::put(&mut b.new, "x.txt", EntryKind::File);
    b.diff.changes.push(Change {
        kind: ChangeKind::Moved,
        old: Some(was),
        new: Some(is),
        fields: FOLDER,
        old_in: Some(0),
    });
    let mut list = showing(difference(vec![a.done(), b.done()], Vec::new()));
    assert_eq!(paths(&list), [r"C:\B\x.txt"]);
    assert_eq!(list.cell(0, Column::Change), "Moved");
    assert_eq!(list.cell(0, Column::WhatChanged), r"from C:\A");
    assert_eq!(
        list.details(0).unwrap().text.lines().next(),
        Some(r"Path: before C:\A\x.txt, after C:\B\x.txt")
    );
    assert_eq!(
        list.summary().unwrap(),
        "1 change: 1 renamed or moved. Size did not change."
    );
    list.set_text(r"a\x");
    assert_eq!(list.len(), 1);
    list.set_text("");
    list.set_layout(DifferenceView::Tree);
    assert_eq!(
        outline(&list),
        [r"C:\B — 1 change", r"  x.txt — moved: from C:\A *"]
    );
}

#[test]
fn an_inaccessible_entry_that_moved_says_both_paths_in_its_details() {
    // A folder renamed that could not be listed in the later snapshot: inaccessible, and
    // renamed; the details of both rows have the two paths.
    let mut made = Made::new(r"C:\Data");
    let a = Made::put(&mut made.old, "locked", EntryKind::Dir);
    let b = Made::put(&mut made.new, "Locked 2", EntryKind::Dir);
    made.new[b as usize].inaccessible = true;
    made.diff.inaccessible.push(Inaccessible {
        old: Some(a),
        new: Some(b),
        changes_before: 0,
        old_in: None,
    });
    made.change(ChangeKind::Moved, Some(a), Some(b), NAME);
    let list = showing(difference(vec![made.done()], Vec::new()));
    assert_eq!(
        column(&list, Column::Change),
        ["Inaccessible folder", "Folder renamed"]
    );
    assert_eq!(
        column(&list, Column::WhatChanged),
        ["in the later snapshot", "from locked"]
    );
    for index in 0..2 {
        let text = list.details(index).unwrap().text;
        assert_eq!(
            text.lines().next(),
            Some(r"Path: before C:\Data\locked, after C:\Data\Locked 2"),
            "{text}"
        );
        assert!(
            text.ends_with("the contents of the folder could not be read."),
            "{text}"
        );
    }
}
