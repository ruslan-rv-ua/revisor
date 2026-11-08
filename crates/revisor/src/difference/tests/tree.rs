//! The difference as a tree (TASK-33).

use super::*;

/// A list in English with plain numbers, showing `difference` as a tree.
fn tree_showing(difference: Difference) -> List {
    let mut list = showing(difference);
    list.set_layout(DifferenceView::Tree);
    list
}

/// The tree as an outline of the texts of its nodes, two spaces per level, every node
/// shown whether expanded or not; `*` marks the node with the focus.
fn outline(list: &List) -> Vec<String> {
    let mut lines = Vec::new();
    let mut stack: Vec<(NodeId, usize)> = list
        .tree_roots()
        .into_iter()
        .rev()
        .map(|n| (n, 0))
        .collect();
    while let Some((node, depth)) = stack.pop() {
        let mark = if list.tree_current() == Some(node) {
            " *"
        } else {
            ""
        };
        lines.push(format!(
            "{}{}{mark}",
            "  ".repeat(depth),
            list.tree_label(node)
        ));
        for child in list.tree_children(node).into_iter().rev() {
            stack.push((child, depth + 1));
        }
    }
    lines
}

/// The node whose text starts with `start`.
fn node(list: &List, start: &str) -> NodeId {
    let mut stack = list.tree_roots();
    while let Some(node) = stack.pop() {
        if list.tree_label(node).starts_with(start) {
            return node;
        }
        stack.extend(list.tree_children(node));
    }
    panic!("no node starts with {start:?}")
}

#[test]
fn the_tree_holds_every_row_under_its_folder_in_the_order_of_the_list() {
    let list = tree_showing(difference(vec![sample()], Vec::new()));
    // The folder of the difference on top, by its whole path; `app`, which did not
    // change, a node of its own; the kind that changed, two nodes (#1, #5).
    assert_eq!(
        outline(&list),
        [
            r"C:\Data — 6 changes, 1 inaccessible",
            "  app — 3 changes",
            "    app.exe — modified: size *",
            "    new.txt — added",
            "    old.txt — deleted",
            "  docs — inaccessible folder: in the later snapshot",
            "  link — link deleted",
            "  link — folder added",
            "  z.txt — added",
        ]
    );
    // The first change has the focus, with the way to it (#8).
    let first = node(&list, "app.exe");
    assert_eq!(list.tree_current(), Some(first));
    let path: Vec<String> = list
        .tree_path(first)
        .into_iter()
        .map(|n| list.tree_label(n))
        .collect();
    assert_eq!(
        path,
        [r"C:\Data — 6 changes, 1 inaccessible", "app — 3 changes"]
    );
    assert!(list.tree_has_children(node(&list, "app")));
    assert!(!list.tree_has_children(first));
}

#[test]
fn the_short_text_says_the_name_and_the_change_only() {
    let mut list = tree_showing(difference(vec![sample()], Vec::new()));
    list.set_node_text(TreeNodeText::Short);
    assert_eq!(
        outline(&list),
        [
            r"C:\Data",
            "  app",
            "    app.exe — modified *",
            "    new.txt — added",
            "    old.txt — deleted",
            "  docs — inaccessible folder",
            "  link — link deleted",
            "  link — folder added",
            "  z.txt — added",
        ]
    );
}

#[test]
fn the_nodes_speak_the_words_of_the_columns_in_ukrainian() {
    let mut list = list_with(&Uk, Box::new(Ukrainian));
    list.set_layout(DifferenceView::Tree);
    list.replace(difference(vec![sample()], Vec::new()), programs());
    assert_eq!(
        outline(&list),
        [
            r"C:\Data — 6 змін, недоступно 1",
            "  app — 3 зміни",
            "    app.exe — змінено: розмір *",
            "    new.txt — додано",
            "    old.txt — видалено",
            "  docs — недоступна тека: у пізнішому знімку",
            "  link — видалено посилання",
            "  link — додано теку",
            "  z.txt — додано",
        ]
    );
}

#[test]
fn a_folder_that_is_a_change_holds_what_is_inside_it() {
    let mut made = Made::new(r"C:\Data");
    // The root itself changed, and so did a folder added with its contents.
    made.change(ChangeKind::Modified, Some(0), Some(0), Fields::ATTRIBUTES);
    made.added("new", EntryKind::Dir, 0);
    made.added(r"new\a.txt", EntryKind::File, 1);
    made.added(r"new\deep\b.txt", EntryKind::File, 1);
    // A folder deleted with its file, and a file of the same name added in its place.
    made.deleted("x", EntryKind::Dir, 0);
    made.deleted(r"x\old.txt", EntryKind::File, 1);
    let b = Made::put(&mut made.new, "x", EntryKind::File);
    made.change(ChangeKind::Added, None, Some(b), Fields::default());
    let list = tree_showing(difference(vec![made.done()], Vec::new()));
    assert_eq!(
        outline(&list),
        [
            r"C:\Data — folder modified: attributes; 6 changes *",
            "  new — folder added, 2 changes",
            "    a.txt — added",
            "    deep — 1 change",
            "      b.txt — added",
            "  x — folder deleted, 1 change",
            "    old.txt — deleted",
            "  x — added",
        ]
    );
}

#[test]
fn the_folders_of_the_difference_are_the_top_level_by_their_paths() {
    let mut work = Made::new(r"D:\Work");
    work.added("a.txt", EntryKind::File, 1);
    let mut users = Made::new(r"C:\Users\me");
    users.added(r"docs\b.txt", EntryKind::File, 1);
    let list = tree_showing(difference(vec![work.done(), users.done()], Vec::new()));
    assert_eq!(
        outline(&list),
        [
            r"C:\Users\me — 1 change",
            "  docs — 1 change",
            "    b.txt — added *",
            r"D:\Work — 1 change",
            "  a.txt — added",
        ]
    );
}

#[test]
fn the_filter_shows_the_same_rows_in_the_tree_and_keeps_the_focus_where_it_can() {
    let mut list = tree_showing(difference(vec![sample()], Vec::new()));
    list.tree_focus(node(&list, "new.txt"));
    // The added rows go; the focus goes to the next node shown (#4).
    list.set_types([false, true, true, true]);
    assert_eq!(
        outline(&list),
        [
            r"C:\Data — 3 changes, 1 inaccessible",
            "  app — 2 changes",
            "    app.exe — modified: size",
            "    old.txt — deleted *",
            "  docs — inaccessible folder: in the later snapshot",
            "  link — link deleted",
        ]
    );
    assert_eq!(list.len(), 4, "the list shows the same rows");
    // The text filter too, and a focus that stays.
    list.set_types([true; 4]);
    assert_eq!(list.tree_current(), Some(node(&list, "old.txt")));
    list.set_text("txt");
    assert_eq!(
        outline(&list),
        [
            r"C:\Data — 3 changes",
            "  app — 2 changes",
            "    new.txt — added",
            "    old.txt — deleted *",
            "  z.txt — added",
        ]
    );
    // The last node hidden: the focus goes to the one before it.
    list.tree_focus(node(&list, "z.txt"));
    list.set_text("app");
    assert_eq!(list.tree_current(), Some(node(&list, "old.txt")));
    // Nothing shown: no node, no focus.
    list.set_text("nothing like it");
    assert!(list.tree_roots().is_empty());
    assert_eq!(list.tree_current(), None);
    assert_eq!(list.list_name(), "No changes match the filter");
    list.set_text("");
    assert_eq!(list.tree_current(), Some(node(&list, "app.exe")));
}

#[test]
fn switching_the_view_keeps_the_focus_on_the_same_entry() {
    let mut list = showing(difference(vec![sample()], Vec::new()));
    // From the row of the list to its node.
    list.focus(2);
    list.set_layout(DifferenceView::Tree);
    assert_eq!(list.view().layout, DifferenceView::Tree);
    assert_eq!(list.tree_current(), Some(node(&list, "old.txt")));
    // From a folder that is no row to the first row shown inside it.
    list.tree_focus(node(&list, "app —"));
    list.set_layout(DifferenceView::List);
    assert_eq!(list.current(), Some(0));
    // From a node back to its row, also when the list is sorted otherwise.
    list.set_sort(DifferenceSort {
        column: SortColumn::Name,
        descending: true,
    });
    list.set_layout(DifferenceView::Tree);
    list.tree_focus(node(&list, "z.txt"));
    list.set_layout(DifferenceView::List);
    let current = list.current().unwrap();
    assert_eq!(list.cell(current, Column::Name), "z.txt");
    // The order of the list stays as it was (#5).
    assert_eq!(list.view().sort.column, SortColumn::Name);
}

#[test]
fn a_folder_that_is_no_change_has_its_path_and_the_summary_of_its_contents() {
    let mut list = tree_showing(difference(vec![sample()], Vec::new()));
    list.tree_focus(node(&list, "app —"));
    let details = list.current_details().unwrap();
    assert_eq!(details.name, "app");
    assert_eq!(
        details.text,
        "Path: C:\\Data\\app\n\
         Inside: 3 changes: 1 added, 1 deleted, 1 modified. Size increased by 47 bytes."
    );
    assert_eq!(
        list.current_location(),
        Some(Location::Folder(PathBuf::from(r"C:\Data\app")))
    );
    // The filter hides some: said how many of them the tree shows.
    list.set_types([false, true, true, true]);
    list.tree_focus(node(&list, "app —"));
    assert!(
        list.current_details()
            .unwrap()
            .text
            .ends_with("Size increased by 47 bytes. Showing 2 of 3."),
    );
    // The top level by its whole path.
    list.tree_focus(node(&list, r"C:\Data"));
    assert_eq!(list.current_details().unwrap().name, r"C:\Data");
}

#[test]
fn a_node_that_is_a_change_has_the_details_and_the_place_of_its_row() {
    let mut list = tree_showing(difference(vec![sample()], Vec::new()));
    list.tree_focus(node(&list, "old.txt"));
    let details = list.current_details().unwrap();
    assert_eq!(details, list.details(2).unwrap());
    assert_eq!(
        list.current_location(),
        Some(Location::Entry(PathBuf::from(r"C:\Data\app\old.txt")))
    );
    // In the list, the row's.
    list.set_layout(DifferenceView::List);
    assert_eq!(list.current_details(), list.details(2));
    assert_eq!(
        list.current_location(),
        Some(Location::Entry(PathBuf::from(r"C:\Data\app\old.txt")))
    );
}

#[test]
fn a_new_difference_opens_the_tree_on_its_first_change() {
    let mut list = tree_showing(difference(vec![sample()], Vec::new()));
    list.tree_focus(node(&list, "z.txt"));
    let mut made = Made::new(r"C:\Data");
    made.added(r"a\b\c.txt", EntryKind::File, 1);
    list.replace(difference(vec![made.done()], Vec::new()), programs());
    assert_eq!(
        outline(&list),
        [
            r"C:\Data — 1 change",
            "  a — 1 change",
            "    b — 1 change",
            "      c.txt — added *",
        ]
    );
}

#[test]
fn a_difference_without_changes_has_an_empty_tree() {
    let list = tree_showing(difference(vec![Made::new(r"C:\Data").done()], Vec::new()));
    assert!(list.tree_roots().is_empty());
    assert_eq!(list.tree_current(), None);
    assert_eq!(list.current_details(), None);
    assert_eq!(list.list_name(), "No changes.");
}

#[test]
fn the_tree_shows_what_the_list_shows_of_many_rows() {
    // Enough rows for the text filter to run on several threads.
    let mut list = tree_showing(generated(4 * ROWS_PER_THREAD + 5));
    let shown_by_tree = |list: &List| -> usize {
        let (_, tree) = list.tree().unwrap();
        list.tree_roots()
            .iter()
            .map(|root| {
                let counts = tree.shown(root.0);
                (counts.changes + counts.inaccessible) as usize
            })
            .sum()
    };
    {
        let (shown, tree) = list.tree().unwrap();
        assert!(tree.len() > shown.rows.len(), "the folders have nodes too");
    }
    assert_eq!(shown_by_tree(&list), list.len());
    for text in ["000123", r"0\file", "LOG"] {
        list.set_text(text);
        assert_eq!(shown_by_tree(&list), list.len(), "{text}");
    }
    // Every row is one node: from the list to the tree and back, the same entry.
    for place in [0, list.len() / 2, list.len() - 1] {
        list.set_layout(DifferenceView::List);
        list.focus(place);
        list.set_layout(DifferenceView::Tree);
        list.set_layout(DifferenceView::List);
        assert_eq!(list.current(), Some(place));
    }
}

/// Times the tree on ~1 million rows (#2), in a release build:
/// `cargo test --release -p revisor -- --ignored --nocapture the_tree_keeps_up`.
#[test]
#[ignore = "a measurement, for a release build"]
fn the_tree_keeps_up_with_a_million_rows() {
    let rows = 1_000_000;
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
    let mut difference = Some(generated(rows));
    note(time("arrival of a million rows as a list", &mut || {
        list.replace(difference.take().unwrap(), programs());
    }));
    note(time("the tree made from the list", &mut || {
        list.set_layout(DifferenceView::Tree);
    }));
    note(time("the top level and its texts", &mut || {
        for root in list.tree_roots() {
            list.tree_label(root);
        }
    }));
    let mut difference = Some(generated(rows));
    note(time("arrival of a million rows as a tree", &mut || {
        list.replace(difference.take().unwrap(), programs());
    }));
    note(time("the way to the first change", &mut || {
        let first = list.tree_current().unwrap();
        for node in list.tree_path(first) {
            for child in list.tree_children(node) {
                list.tree_label(child);
            }
        }
    }));
    for types in [[false, true, true, true], [true; 4]] {
        note(time(&format!("types {types:?}"), &mut || {
            list.set_types(types);
        }));
    }
    let mut typed = String::new();
    for letter in "file 0004".chars() {
        typed.push(letter);
        let text = typed.clone();
        note(time(&format!("text {text:?}"), &mut || {
            list.set_text(&text);
        }));
    }
    note(time("back to the list", &mut || {
        list.set_layout(DifferenceView::List);
    }));
    println!("worst: {:.3} s", worst.as_secs_f64());
    assert!(worst.as_secs_f64() < 2.0, "{worst:?}");
}
