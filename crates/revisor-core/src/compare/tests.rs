use super::*;
use crate::diff::ChangeKind;
use crate::scan::{FileId, Progress, synthetic};
use crate::snapshot;
use crate::store::{FolderSnapshot, ProfileSettings, Scope, Snapshot};
use crate::testing::{TempDir, Tree, field_names, junction};
use std::ffi::OsStr;
use std::fs;

fn create(tmp: &TempDir) -> Store {
    Store::create(
        &tmp.db(),
        &ProfileSettings {
            name: "Тест".into(),
            scope: Scope::default(),
        },
    )
    .unwrap()
}

/// A scanned folder at `path`, its real location the same, holding `tree`.
fn scanned(path: &str, volume_serial: u64, tree: &Tree) -> FolderSnapshot {
    FolderSnapshot {
        path: path.into(),
        state: FolderState::Scanned {
            volume_serial,
            real_path: path.into(),
        },
        entries: tree.0.clone(),
    }
}

fn missing(path: &str) -> FolderSnapshot {
    FolderSnapshot {
        path: path.into(),
        state: FolderState::Missing,
        entries: Vec::new(),
    }
}

fn unavailable(path: &str, reason: Unavailable) -> FolderSnapshot {
    FolderSnapshot {
        path: path.into(),
        state: FolderState::Unavailable(reason),
        entries: Vec::new(),
    }
}

fn exclusion(pattern: &str, applied: Option<&str>) -> SnapshotExclusion {
    SnapshotExclusion {
        pattern: pattern.into(),
        applied: applied.map(String::from),
    }
}

/// Saves a snapshot started at `taken_at` and returns its id.
fn save(
    store: &mut Store,
    taken_at: i64,
    folders: Vec<FolderSnapshot>,
    exclusions: Vec<SnapshotExclusion>,
) -> i64 {
    store
        .save(
            &Snapshot {
                taken_at,
                comment: None,
                folders,
                exclusions,
            },
            &AtomicBool::new(false),
        )
        .unwrap()
}

fn compare_them(store: &Store, a: i64, b: i64) -> Difference {
    compare(store, a, b, &AtomicBool::new(false)).unwrap()
}

fn side_name(side: Side) -> &'static str {
    match side {
        Side::Earlier => "earlier",
        Side::Later => "later",
    }
}

fn warning_line(warning: &Warning) -> String {
    let applied = |a: &Option<String>| a.clone().unwrap_or_else(|| "-".into());
    match warning {
        Warning::FolderOnlyIn { path, side } => {
            format!("! only in {}: {}", side_name(*side), path.display())
        }
        Warning::FolderNarrowed {
            wider,
            side,
            compared,
        } => format!(
            "! {} {} compared within {}",
            side_name(*side),
            wider.display(),
            compared.display()
        ),
        Warning::FolderMissing { path, side } => {
            format!("! missing in {}: {}", side_name(*side), path.display())
        }
        Warning::FolderUnavailable { path, side, reason } => format!(
            "! unavailable in {}: {} ({reason:?})",
            side_name(*side),
            path.display()
        ),
        Warning::DifferentVolumes {
            path,
            earlier,
            later,
        } => format!("! volumes {}: {earlier} vs {later}", path.display()),
        Warning::ExclusionOnlyIn { pattern, side } => {
            format!("! exclusion only in {}: {pattern}", side_name(*side))
        }
        Warning::ExclusionAppliedDifferently {
            pattern,
            earlier,
            later,
        } => format!(
            "! exclusion {pattern}: {} vs {}",
            applied(earlier),
            applied(later)
        ),
    }
}

/// The difference as lines: for each folder `+ path`, `- path`, `~ path fields` and
/// `? path` with full paths, then `! …` for each warning. Paths of deleted, modified and
/// inaccessible entries are those in the earlier snapshot.
fn render(d: &Difference) -> Vec<String> {
    let mut out = Vec::new();
    for f in &d.folders {
        for c in &f.diff.changes {
            let path = c.path(&f.old, &f.new);
            out.push(match c.kind {
                ChangeKind::Added => format!("+ {}", path.display()),
                ChangeKind::Deleted => format!("- {}", path.display()),
                ChangeKind::Modified => {
                    format!("~ {} {}", path.display(), field_names(c.fields))
                }
            });
        }
        out.extend(f.diff.inaccessible.iter().map(|i| match i.old {
            Some(a) => format!("? {}", diff::path(&f.old, a).display()),
            None => format!("? {}", diff::path(&f.new, i.new.unwrap()).display()),
        }));
    }
    out.extend(d.warnings.iter().map(warning_line));
    out
}

#[test]
fn the_difference_goes_from_the_earlier_snapshot_whichever_is_asked_first() {
    let tmp = TempDir::new("compare");
    let mut store = create(&tmp);
    let mut before = Tree::new();
    before.add("a.txt", EntryKind::File);
    let mut after = before.clone();
    after.add("b.txt", EntryKind::File);
    // The clock was set back between the snapshots: the order they were taken in counts.
    let first = save(
        &mut store,
        2000,
        vec![scanned(r"C:\Data", 1, &before)],
        vec![],
    );
    let second = save(
        &mut store,
        1000,
        vec![scanned(r"C:\Data", 1, &after)],
        vec![],
    );

    for (a, b) in [(first, second), (second, first)] {
        let d = compare_them(&store, a, b);
        assert_eq!((d.earlier.id, d.later.id), (first, second));
        assert_eq!((d.earlier.taken_at, d.later.taken_at), (2000, 1000));
        assert_eq!(render(&d), [r"+ C:\Data\b.txt"]);
        assert_eq!(d.folders[0].path, Path::new(r"C:\Data"));
    }
    // A snapshot against itself: nothing.
    assert!(render(&compare_them(&store, first, first)).is_empty());
}

#[test]
fn a_missing_folder_holds_nothing() {
    let tmp = TempDir::new("compare");
    let mut store = create(&tmp);
    let mut tree = Tree::new();
    tree.add("a.txt", EntryKind::File);
    tree.add("sub", EntryKind::Dir);
    tree.add(r"sub\b.txt", EntryKind::File);
    let there = save(&mut store, 1, vec![scanned(r"C:\Data", 1, &tree)], vec![]);
    let gone = save(&mut store, 2, vec![missing(r"C:\Data")], vec![]);
    let back = save(&mut store, 3, vec![scanned(r"C:\Data", 1, &tree)], vec![]);
    let gone_again = save(&mut store, 4, vec![missing(r"C:\Data")], vec![]);

    // The folder was deleted: it is gone with everything in it.
    assert_eq!(
        render(&compare_them(&store, there, gone)),
        [
            r"- C:\Data",
            r"- C:\Data\a.txt",
            r"- C:\Data\sub",
            r"- C:\Data\sub\b.txt",
            r"! missing in later: C:\Data",
        ]
    );
    // Made again.
    assert_eq!(
        render(&compare_them(&store, gone, back)),
        [
            r"+ C:\Data",
            r"+ C:\Data\a.txt",
            r"+ C:\Data\sub",
            r"+ C:\Data\sub\b.txt",
            r"! missing in earlier: C:\Data",
        ]
    );
    // Missing both times: nothing to compare, and both snapshots say so.
    assert_eq!(
        render(&compare_them(&store, gone, gone_again)),
        [
            r"! missing in earlier: C:\Data",
            r"! missing in later: C:\Data"
        ]
    );
}

#[test]
fn an_unavailable_folder_is_not_compared() {
    let tmp = TempDir::new("compare");
    let mut store = create(&tmp);
    let mut tree = Tree::new();
    tree.add("a.txt", EntryKind::File);
    let there = save(&mut store, 1, vec![scanned(r"C:\Data", 1, &tree)], vec![]);
    let unplugged = save(
        &mut store,
        2,
        vec![unavailable(r"C:\Data", Unavailable::Error(21))],
        vec![],
    );
    let fat = save(
        &mut store,
        3,
        vec![unavailable(
            r"C:\Data",
            Unavailable::NotNtfs("FAT32".into()),
        )],
        vec![],
    );
    let gone = save(&mut store, 4, vec![missing(r"C:\Data")], vec![]);

    let d = compare_them(&store, there, unplugged);
    assert!(d.folders.is_empty());
    assert_eq!(render(&d), [r"! unavailable in later: C:\Data (Error(21))"]);
    assert_eq!(
        render(&compare_them(&store, unplugged, fat)),
        [
            r"! unavailable in earlier: C:\Data (Error(21))",
            r#"! unavailable in later: C:\Data (NotNtfs("FAT32"))"#,
        ]
    );
    // Unavailable on one side and missing on the other: still unknown, so nothing is
    // added or deleted.
    assert_eq!(
        render(&compare_them(&store, fat, gone)),
        [r#"! unavailable in earlier: C:\Data (NotNtfs("FAT32"))"#]
    );
}

#[test]
fn folders_are_matched_by_path_without_regard_to_case_in_the_later_snapshots_order() {
    let tmp = TempDir::new("compare");
    let mut store = create(&tmp);
    let mut apps = Tree::with_root(r"C:\Apps");
    apps.add("a.txt", EntryKind::File);
    let mut apps_after = Tree::with_root(r"c:\apps\");
    apps_after.add("a.txt", EntryKind::File).size = 1;
    let docs = Tree::with_root(r"D:\Docs");
    let mut docs_after = Tree::with_root(r"D:\DOCS");
    docs_after.add("d.txt", EntryKind::File);
    let earlier = save(
        &mut store,
        1,
        vec![
            scanned(r"D:\Docs", 2, &docs),
            scanned(r"C:\Apps", 1, &apps),
            scanned(r"E:\Old", 3, &Tree::with_root(r"E:\Old")),
        ],
        vec![],
    );
    let later = save(
        &mut store,
        2,
        vec![
            scanned(r"c:\apps\", 1, &apps_after),
            scanned(r"F:\New", 4, &Tree::with_root(r"F:\New")),
            scanned(r"D:\DOCS", 2, &docs_after),
        ],
        vec![],
    );

    let d = compare_them(&store, earlier, later);
    let paths: Vec<&Path> = d.folders.iter().map(|f| f.path.as_path()).collect();
    assert_eq!(paths, [Path::new(r"c:\apps\"), Path::new(r"D:\DOCS")]);
    assert_eq!(
        render(&d),
        [
            r"~ C:\Apps\a.txt size",
            r"+ D:\DOCS\d.txt",
            r"! only in later: F:\New",
            r"! only in earlier: E:\Old",
        ]
    );
}

#[test]
fn a_folder_inside_a_folder_of_the_other_snapshot_is_compared_within_it() {
    let tmp = TempDir::new("compare");
    let mut store = create(&tmp);
    let mut wide = Tree::new();
    wide.add("app", EntryKind::Dir);
    wide.add(r"app\a.txt", EntryKind::File);
    wide.add("other.txt", EntryKind::File);
    let mut narrow = Tree::with_root(r"C:\Data\app");
    narrow.add("a.txt", EntryKind::File);
    narrow.add("new.txt", EntryKind::File);
    let wide_id = save(&mut store, 1, vec![scanned(r"C:\Data", 1, &wide)], vec![]);
    let narrow_id = save(
        &mut store,
        2,
        vec![scanned(r"C:\Data\app", 1, &narrow)],
        vec![],
    );

    // The profile had C:\Data, then C:\Data\app instead: only app is compared.
    let d = compare_them(&store, wide_id, narrow_id);
    assert_eq!(d.folders[0].path, Path::new(r"C:\Data\app"));
    assert_eq!(
        render(&d),
        [
            r"+ C:\Data\app\new.txt",
            r"! earlier C:\Data compared within C:\Data\app",
        ]
    );
    // And the other way round.
    let wide_again = save(&mut store, 3, vec![scanned(r"C:\Data", 1, &wide)], vec![]);
    assert_eq!(
        render(&compare_them(&store, narrow_id, wide_again)),
        [
            r"- C:\Data\app\new.txt",
            r"! later C:\Data compared within C:\Data\app",
        ]
    );

    // Two folders of one snapshot inside one folder of the other; the second is not in
    // the wider folder at all, so it was made since.
    let mut docs = Tree::with_root(r"C:\Data\docs");
    docs.add("d.txt", EntryKind::File);
    let two = save(
        &mut store,
        4,
        vec![
            scanned(r"C:\Data\app", 1, &narrow),
            scanned(r"C:\Data\docs", 1, &docs),
        ],
        vec![],
    );
    assert_eq!(
        render(&compare_them(&store, wide_again, two)),
        [
            r"+ C:\Data\app\new.txt",
            r"+ C:\Data\docs",
            r"+ C:\Data\docs\d.txt",
            r"! earlier C:\Data compared within C:\Data\app",
            r"! earlier C:\Data compared within C:\Data\docs",
        ]
    );

    // The wider folder could not be listed: whether the nested ones were there is unknown.
    let mut dark = Tree::new();
    dark.0[0].inaccessible = true;
    let dark_id = save(&mut store, 5, vec![scanned(r"C:\Data", 1, &dark)], vec![]);
    assert_eq!(
        render(&compare_them(&store, two, dark_id)),
        [
            r"? C:\Data\app",
            r"? C:\Data\docs",
            r"! later C:\Data compared within C:\Data\app",
            r"! later C:\Data compared within C:\Data\docs",
        ]
    );

    // The nested folder matches an exclusion of the wider snapshot, whose scan left it
    // out: only the other snapshot holds it.
    let excluded = save(
        &mut store,
        6,
        vec![scanned(r"C:\Data", 1, &wide)],
        vec![exclusion("app", Some("app"))],
    );
    assert_eq!(
        render(&compare_them(&store, two, excluded)),
        [
            r"- C:\Data\docs",
            r"- C:\Data\docs\d.txt",
            r"! only in earlier: C:\Data\app",
            r"! later C:\Data compared within C:\Data\docs",
            r"! exclusion only in later: app",
        ]
    );

    // The nested folder matches an exclusion of its own snapshot: a scan never leaves
    // its root out, so the folder is compared all the same.
    let own_root_excluded = save(
        &mut store,
        7,
        vec![scanned(r"C:\Data\app", 1, &narrow)],
        vec![exclusion("app", Some("app"))],
    );
    assert_eq!(
        render(&compare_them(&store, wide_again, own_root_excluded)),
        [
            r"+ C:\Data\app\new.txt",
            r"! earlier C:\Data compared within C:\Data\app",
            r"! exclusion only in later: app",
        ]
    );

    // The nested folder is a junction in the wider folder, which the wider scan did not
    // follow: what it held then is unknown.
    let mut with_junction = Tree::new();
    with_junction.add("app", EntryKind::Link).link_target = Some(OsStr::new(r"D:\app").into());
    let junction_id = save(
        &mut store,
        8,
        vec![scanned(r"C:\Data", 1, &with_junction)],
        vec![],
    );
    assert_eq!(
        render(&compare_them(&store, narrow_id, junction_id)),
        [
            r"? C:\Data\app",
            r"! later C:\Data compared within C:\Data\app",
        ]
    );
}

#[test]
fn exclusions_of_both_snapshots_apply_to_both() {
    let tmp = TempDir::new("compare");
    let mut store = create(&tmp);
    let mut before = Tree::new();
    for (folder, file) in [
        ("Temp", "t.txt"),
        ("Code Cache", "c.bin"),
        ("OtherTemp", "o.txt"),
    ] {
        before.add(folder, EntryKind::Dir);
        before.add(&format!(r"{folder}\{file}"), EntryKind::File);
    }
    before.add("keep.txt", EntryKind::File);
    let mut after = before.clone();
    after.add("x.tmp", EntryKind::File);
    after.add("new.txt", EntryKind::File);
    for rel in [
        r"Temp\t.txt",
        r"Code Cache\c.bin",
        r"OtherTemp\o.txt",
        "keep.txt",
    ] {
        after.get(rel).size = 1;
    }
    let earlier = save(
        &mut store,
        1,
        vec![scanned(r"C:\Data", 1, &before)],
        vec![
            exclusion("*.tmp", Some("*.tmp")),
            exclusion("%TEMP%", Some(r"C:\Data\Temp")),
        ],
    );
    // Taken under another account: TEMP expanded elsewhere.
    let later = save(
        &mut store,
        2,
        vec![scanned(r"C:\Data", 1, &after)],
        vec![
            exclusion("Code Cache", Some("Code Cache")),
            exclusion("%TEMP%", Some(r"C:\Data\OtherTemp")),
            exclusion(r"%NoSuchVariable%\x", None),
        ],
    );

    assert_eq!(
        render(&compare_them(&store, earlier, later)),
        [
            r"~ C:\Data\keep.txt size",
            r"+ C:\Data\new.txt",
            r"! exclusion only in earlier: *.tmp",
            r"! exclusion %TEMP%: C:\Data\Temp vs C:\Data\OtherTemp",
            r"! exclusion only in later: Code Cache",
            r"! exclusion only in later: %NoSuchVariable%\x",
        ]
    );
    // The same exclusions in another case and order are the same exclusions.
    let same = save(
        &mut store,
        3,
        vec![scanned(r"C:\Data", 1, &after)],
        vec![
            exclusion("%temp%", Some(r"c:\data\otherTemp")),
            exclusion(r"%NoSuchVariable%\x", None),
            exclusion("CODE CACHE", Some("CODE CACHE")),
        ],
    );
    assert_eq!(
        render(&compare_them(&store, later, same)),
        Vec::<String>::new()
    );
}

#[test]
fn file_ids_are_compared_only_within_one_volume() {
    let tmp = TempDir::new("compare");
    let mut store = create(&tmp);
    let mut before = Tree::new();
    before.add("a.txt", EntryKind::File);
    let mut after = before.clone();
    after.get("a.txt").file_id = FileId([9; 16]);
    let one = save(&mut store, 1, vec![scanned(r"C:\Data", 1, &before)], vec![]);
    let same_volume = save(&mut store, 2, vec![scanned(r"C:\Data", 1, &after)], vec![]);
    let other_volume = save(&mut store, 3, vec![scanned(r"C:\Data", 2, &after)], vec![]);

    assert_eq!(
        render(&compare_them(&store, one, same_volume)),
        [r"~ C:\Data\a.txt file_id"]
    );
    // Another disk under the same letter, or the disk formatted: the IDs mean nothing to
    // each other, and the warning says where a mass of changes may come from.
    assert_eq!(
        render(&compare_them(&store, one, other_volume)),
        [r"! volumes C:\Data: 1 vs 2"]
    );
}

#[test]
fn identical_snapshots_give_an_empty_difference_even_with_inaccessible_entries() {
    let tmp = TempDir::new("compare");
    let mut store = create(&tmp);
    let mut tree = Tree::new();
    tree.add("a.txt", EntryKind::File);
    tree.add("locked", EntryKind::Dir).inaccessible = true;
    let exclusions = || vec![exclusion("*.tmp", Some("*.tmp"))];
    let a = save(
        &mut store,
        1,
        vec![scanned(r"C:\Data", 1, &tree)],
        exclusions(),
    );
    let b = save(
        &mut store,
        2,
        vec![scanned(r"C:\Data", 1, &tree)],
        exclusions(),
    );

    let d = compare_them(&store, a, b);
    assert!(d.folders.iter().all(|f| f.diff.changes.is_empty()));
    assert!(d.warnings.is_empty());
    assert_eq!(render(&d), [r"? C:\Data\locked"]);
}

#[test]
fn a_deleted_snapshot_cannot_be_compared() {
    let tmp = TempDir::new("compare");
    let mut store = create(&tmp);
    let tree = Tree::new();
    let a = save(&mut store, 1, vec![scanned(r"C:\Data", 1, &tree)], vec![]);
    let b = save(&mut store, 2, vec![scanned(r"C:\Data", 1, &tree)], vec![]);
    store.delete(a).unwrap();
    let result = compare(&store, b, a, &AtomicBool::new(false));
    assert!(
        matches!(result, Err(CompareError::NoSuchSnapshot(id)) if id == a),
        "{result:?}"
    );
}

#[test]
fn a_cancelled_comparison_stops_at_once() {
    // Loading honours the same flag: `store::tests::a_cancelled_load_stops_at_once`.
    let tmp = TempDir::new("compare");
    let mut store = create(&tmp);
    let no_cancel = AtomicBool::new(false);
    let a = store
        .save(&Snapshot::of_tree(synthetic(10_000)), &no_cancel)
        .unwrap();
    let b = store
        .save(&Snapshot::of_tree(synthetic(10_000)), &no_cancel)
        .unwrap();
    let result = compare(&store, a, b, &AtomicBool::new(true));
    assert!(matches!(result, Err(CompareError::Cancelled)), "{result:?}");
}

#[test]
fn two_snapshots_of_an_unchanged_folder_give_an_empty_difference() {
    let tmp = TempDir::new("compare");
    let root = tmp.path().join("tree");
    for i in 0..10 {
        tmp.write(
            &format!(r"tree\d{i}\Sub Folder\f{i}.txt"),
            &vec![b'x'; i * 100],
        );
        tmp.write(&format!(r"tree\d{i}\Файл {i}.dat"), b"data");
    }
    tmp.write(r"tree\skip.tmp", b"");
    let system = std::env::var_os("SystemRoot").unwrap();
    fs::copy(
        Path::new(&system).join("System32\\kernel32.dll"),
        root.join("k.dll"),
    )
    .unwrap();
    junction(&root.join("link"), &root.join("d0"));
    drop(
        Store::create(
            &tmp.db(),
            &ProfileSettings {
                name: "Diff".into(),
                scope: Scope {
                    folders: vec![root],
                    exclusions: vec!["*.tmp".into(), "%TEMP%".into()],
                },
            },
        )
        .unwrap(),
    );
    let no_cancel = AtomicBool::new(false);
    let take = || {
        snapshot::take(&tmp.db(), None, &Progress::default(), &no_cancel)
            .unwrap()
            .id
    };
    let (first, second) = (take(), take());

    let store = Store::open(&tmp.db()).unwrap();
    let d = compare_them(&store, first, second);
    assert_eq!(render(&d), Vec::<String>::new());
    assert_eq!(d.folders.len(), 1);
    assert_eq!(d.folders[0].old.len(), 1 + 10 * 4 + 2);
}
