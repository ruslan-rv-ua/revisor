use super::*;
use crate::exclude::RecordedEnvironment;
use crate::scan::{FileId, Progress, ScanOptions, Version, VersionInfo, scan, synthetic};
use crate::store::{ProfileSettings, Scope, Snapshot, Store};
use crate::testing::{
    TempDir, Tree, cancelled, entry, field_names, file_id, junction, rel_path, renumber,
};
use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::Path;
use std::sync::atomic::AtomicU64;

const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
const FILE_ATTRIBUTE_NORMAL: u32 = 0x80;
const FILE_ATTRIBUTE_OFFLINE: u32 = 0x1000;
const FILE_ATTRIBUTE_RECALL_ON_OPEN: u32 = 0x4_0000;
const FILE_ATTRIBUTE_PINNED: u32 = 0x8_0000;
const FILE_ATTRIBUTE_UNPINNED: u32 = 0x10_0000;
const FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS: u32 = 0x40_0000;

/// A read version with the same file and product version.
fn version(parts: [u16; 4]) -> VersionInfo {
    VersionInfo::Some(Version {
        file: parts,
        product: parts,
    })
}

/// The diff of two whole trees as lines: `+ path`, `- path`, `~ path fields`,
/// `> path < path it had fields` for a renamed or moved entry, then `? path` for the
/// inaccessible entries. Paths of deleted, modified and inaccessible entries are those in
/// the older snapshot.
fn lines(old: &[Entry], new: &[Entry]) -> Vec<String> {
    lines_with(old, new, &Rules::plain())
}

/// [`lines`] of the whole trees compared by `rules`.
fn lines_with(old: &[Entry], new: &[Entry], rules: &Rules<'_>) -> Vec<String> {
    let diff = diff(
        Subtree::whole(old),
        Subtree::whole(new),
        rules,
        &Cancel::new(),
    )
    .unwrap();
    assert!(
        diff.changes.iter().all(|c| c.old_in.is_none()),
        "one comparison has one older tree"
    );
    let mut out: Vec<String> = diff
        .changes
        .iter()
        .map(|c| match c.kind {
            ChangeKind::Added => format!("+ {}", rel_path(new, c.new.unwrap())),
            ChangeKind::Deleted => format!("- {}", rel_path(old, c.old.unwrap())),
            ChangeKind::Modified => format!(
                "~ {} {}",
                rel_path(old, c.old.unwrap()),
                field_names(c.fields)
            ),
            ChangeKind::Moved => format!(
                "> {} < {} {}",
                rel_path(new, c.new.unwrap()),
                rel_path(old, c.old.unwrap()),
                field_names(c.fields)
            ),
        })
        .collect();
    out.extend(diff.inaccessible.iter().map(|i| match i.old {
        Some(a) => format!("? {}", rel_path(old, a)),
        None => format!("? {}", rel_path(new, i.new.unwrap())),
    }));
    out
}

/// A tree with a bit of everything.
fn sample() -> Tree {
    let mut t = Tree::new();
    t.add("app", EntryKind::Dir);
    fill_app(&mut t, r"app\");
    t.add("link", EntryKind::Link).link_target = Some(OsStr::new(r"C:\Target").into());
    t.add("Notes.txt", EntryKind::File);
    t
}

/// The contents of the `app` folder of [`sample`], under `prefix`.
fn fill_app(t: &mut Tree, prefix: &str) {
    let exe = t.add(&format!("{prefix}app.exe"), EntryKind::File);
    exe.size = 1000;
    exe.created = 10;
    exe.modified = 20;
    exe.version = version([1, 2, 3, 4]);
    t.add(&format!("{prefix}readme.txt"), EntryKind::File).size = 5;
    t.add(&format!("{prefix}data"), EntryKind::Dir);
    t.add(&format!(r"{prefix}data\a.dat"), EntryKind::File);
    t.add(&format!(r"{prefix}data\b.dat"), EntryKind::File);
}

/// Rules with these exclusions, as a snapshot recorded them, for trees whose roots are
/// at `root`.
fn rules<'a>(exclusions: &'a Exclusions, root: &str) -> Rules<'a> {
    Rules {
        filter: Some(Filter {
            exclusions,
            branch: exclusions.branch(Path::new(root)),
        }),
        volumes: [0; 2],
    }
}

#[test]
fn entries_matching_an_exclusion_are_left_out_with_their_subtrees() {
    let old = sample();
    let mut new = sample();
    // Added since, but excluded now: not added.
    new.add(r"app\x.tmp", EntryKind::File);
    new.add(r"app\data\Code Cache", EntryKind::Dir);
    new.add(r"app\data\Code Cache\f.bin", EntryKind::File);
    // Changed, but excluded by a path pattern: not modified.
    new.get(r"app\data\a.dat").size = 9;
    // Not excluded: reported as usual.
    new.remove(r"app\readme.txt");
    let exclusions = Exclusions::new(
        &["*.tmp", "Code Cache", r"C:\Data\app\data\a.dat"],
        &RecordedEnvironment,
    );
    assert_eq!(
        lines_with(&old.0, &new.0, &rules(&exclusions, r"C:\Data")),
        [r"- app\readme.txt"]
    );

    // An excluded folder goes with everything in it, whatever changed there.
    let exclusions = Exclusions::new(&["data"], &RecordedEnvironment);
    assert_eq!(
        lines_with(&old.0, &new.0, &rules(&exclusions, r"C:\Data")),
        [r"- app\readme.txt", r"+ app\x.tmp"]
    );

    // The roots are never excluded, as in a scan.
    let exclusions = Exclusions::new(&[r"C:\Data"], &RecordedEnvironment);
    assert_eq!(
        lines_with(&old.0, &new.0, &rules(&exclusions, r"C:\Data")),
        [
            r"~ app\data\a.dat size",
            r"+ app\data\Code Cache",
            r"+ app\data\Code Cache\f.bin",
            r"- app\readme.txt",
            r"+ app\x.tmp",
        ]
    );
}

#[test]
fn file_ids_are_not_compared_across_volumes() {
    let old = sample();
    let mut new = sample();
    // Another disk under the same letter: its IDs mean nothing to the other's.
    new.get(r"app\app.exe").file_id = FileId([1; 16]);
    new.get(r"app\app.exe").size = 1;
    new.get(r"app\data").file_id = FileId([2; 16]);
    let rules = Rules {
        volumes: [1, 2],
        ..Rules::plain()
    };
    assert_eq!(lines_with(&old.0, &new.0, &rules), [r"~ app\app.exe size"]);
}

#[test]
fn a_folder_of_one_tree_is_compared_with_the_whole_of_another() {
    // The profile first had C:\Data\app, and later C:\Data, which holds app.
    let mut narrow = Tree::with_root(r"C:\Data\app");
    fill_app(&mut narrow, "");
    let mut wide = sample();
    wide.add(r"app\new.txt", EntryKind::File);
    wide.get(r"app\readme.txt").size = 7;
    let no_cancel = Cancel::new();
    let changes = |old: Subtree<'_>, new: Subtree<'_>, rules: &Rules<'_>| -> Vec<String> {
        diff(old, new, rules, &no_cancel)
            .unwrap()
            .changes
            .iter()
            .map(|c| {
                let full = c.path(old.entries, new.entries);
                format!("{:?} {} {}", c.kind, full.display(), field_names(c.fields))
            })
            .collect()
    };
    let in_wide = Subtree {
        entries: &wide.0,
        root: wide.index("app"),
    };
    // The roots are matched with each other, and their names are not compared.
    assert_eq!(
        changes(Subtree::whole(&narrow.0), in_wide, &Rules::plain()),
        [
            r"Added C:\Data\app\new.txt ",
            r"Modified C:\Data\app\readme.txt size"
        ]
    );
    assert_eq!(
        changes(in_wide, Subtree::whole(&narrow.0), &Rules::plain()),
        [
            r"Deleted C:\Data\app\new.txt ",
            r"Modified C:\Data\app\readme.txt size"
        ]
    );
    // Exclusions apply from where the compared roots stand.
    let exclusions = Exclusions::new(&[r"C:\Data\app\new.txt"], &RecordedEnvironment);
    assert_eq!(
        changes(
            Subtree::whole(&narrow.0),
            in_wide,
            &rules(&exclusions, r"C:\Data\app")
        ),
        [r"Modified C:\Data\app\readme.txt size"]
    );
}

/// The diff of two whole trees by [`Rules::plain`].
fn diff_whole(old: &[Entry], new: &[Entry], cancel: &Cancel) -> Result<Diff, Cancelled> {
    diff(
        Subtree::whole(old),
        Subtree::whole(new),
        &Rules::plain(),
        cancel,
    )
}

#[test]
fn same_snapshot_has_no_changes() {
    let t = sample();
    assert_eq!(lines(&t.0, &t.0), Vec::<String>::new());
    let big = synthetic(10_000);
    assert!(
        diff_whole(&big, &big, &Cancel::new())
            .unwrap()
            .changes
            .is_empty()
    );
}

#[test]
fn added_and_deleted_files() {
    let old = sample();
    let mut new = sample();
    new.add(r"app\new.dll", EntryKind::File);
    new.remove("Notes.txt");
    assert_eq!(lines(&old.0, &new.0), [r"+ app\new.dll", "- Notes.txt"]);
}

#[test]
fn added_and_deleted_folders_list_every_entry_inside() {
    let old = sample();
    let mut new = sample();
    new.add("plugins", EntryKind::Dir);
    new.add(r"plugins\one", EntryKind::Dir);
    new.add(r"plugins\one\p.dll", EntryKind::File);
    new.remove(r"app\data");
    assert_eq!(
        lines(&old.0, &new.0),
        [
            r"- app\data",
            r"- app\data\a.dat",
            r"- app\data\b.dat",
            "+ plugins",
            r"+ plugins\one",
            r"+ plugins\one\p.dll",
        ]
    );
}

#[test]
fn modified_fields_of_files() {
    let old = sample();
    let mut new = sample();
    new.get(r"app\app.exe").size = 2000;
    new.get(r"app\readme.txt").modified = 1;
    new.get(r"app\data\a.dat").created = 1;
    new.get(r"app\data\b.dat").attributes |= FILE_ATTRIBUTE_HIDDEN;
    assert_eq!(
        lines(&old.0, &new.0),
        [
            r"~ app\app.exe size",
            r"~ app\data\a.dat created",
            r"~ app\data\b.dat attributes",
            r"~ app\readme.txt modified",
        ]
    );
}

#[test]
fn only_the_creation_time_counts_as_a_change() {
    let old = sample();
    let mut new = sample();
    new.get(r"app\app.exe").created = 11;
    assert_eq!(lines(&old.0, &new.0), [r"~ app\app.exe created"]);
}

#[test]
fn changed_version_is_a_change() {
    let old = sample();
    let mut new = sample();
    new.get(r"app\app.exe").version = version([1, 2, 3, 5]);
    assert_eq!(lines(&old.0, &new.0), [r"~ app\app.exe version"]);
    // The product version alone changes too.
    let mut new = sample();
    new.get(r"app\app.exe").version = VersionInfo::Some(Version {
        file: [1, 2, 3, 4],
        product: [2, 0, 0, 0],
    });
    assert_eq!(lines(&old.0, &new.0), [r"~ app\app.exe version"]);
}

#[test]
fn a_version_not_read_in_either_snapshot_is_not_a_change() {
    let old = sample();
    let mut new = sample();
    new.get(r"app\app.exe").version = VersionInfo::NotRead;
    assert_eq!(lines(&old.0, &new.0), Vec::<String>::new());
    assert_eq!(lines(&new.0, &old.0), Vec::<String>::new());
    // Read in both, and the version resource is gone: a change.
    new.get(r"app\app.exe").version = VersionInfo::None;
    assert_eq!(lines(&old.0, &new.0), [r"~ app\app.exe version"]);
}

#[test]
fn archive_attribute_is_ignored() {
    let old = sample();
    let mut new = sample();
    new.get(r"app\readme.txt").attributes &= !FILE_ATTRIBUTE_ARCHIVE;
    new.get("app").attributes |= FILE_ATTRIBUTE_ARCHIVE;
    assert_eq!(lines(&old.0, &new.0), Vec::<String>::new());
}

#[test]
fn normal_and_cloud_state_attributes_are_ignored() {
    let old = sample();
    let mut new = sample();
    // Clearing the archive attribute of a file with no other attributes leaves NTFS
    // reporting "normal" in place of no attributes at all.
    new.get(r"app\readme.txt").attributes = FILE_ATTRIBUTE_NORMAL;
    // OneDrive moved one file to the cloud, pinned another, and marked a folder.
    new.get(r"app\data\a.dat").attributes |=
        FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS | FILE_ATTRIBUTE_OFFLINE | FILE_ATTRIBUTE_UNPINNED;
    new.get(r"app\data\b.dat").attributes |= FILE_ATTRIBUTE_PINNED | FILE_ATTRIBUTE_RECALL_ON_OPEN;
    new.get(r"app\data").attributes |= FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS;
    assert_eq!(lines(&old.0, &new.0), Vec::<String>::new());
    // The other attributes still count, whatever the cloud state.
    new.get(r"app\data\a.dat").attributes |= FILE_ATTRIBUTE_HIDDEN;
    assert_eq!(lines(&old.0, &new.0), [r"~ app\data\a.dat attributes"]);
}

#[test]
fn a_different_file_id_at_the_same_path_is_a_replacement() {
    let old = sample();
    let mut new = sample();
    // Deleted and made again: NTFS gives the new file another ID, even where tunneling
    // hands it the old creation time.
    new.get(r"app\app.exe").file_id = FileId([1; 16]);
    new.get(r"app\data").file_id = FileId([2; 16]);
    new.get("link").file_id = FileId([3; 16]);
    assert_eq!(
        lines(&old.0, &new.0),
        [
            r"~ app\app.exe file_id",
            r"~ app\data file_id",
            "~ link file_id"
        ]
    );
    // Other differences are reported with it.
    new.get(r"app\app.exe").size = 1;
    assert_eq!(lines(&old.0, &new.0)[0], r"~ app\app.exe size,file_id");
}

#[test]
fn last_write_time_of_folders_is_ignored() {
    let old = sample();
    let mut new = sample();
    new.get("app").modified = 99;
    new.0[0].modified = 99;
    assert_eq!(lines(&old.0, &new.0), Vec::<String>::new());

    new.get("app").created = 99;
    new.get(r"app\data").attributes |= FILE_ATTRIBUTE_HIDDEN;
    assert_eq!(
        lines(&old.0, &new.0),
        ["~ app created", r"~ app\data attributes"]
    );
}

#[test]
fn the_differences_the_comparison_does_not_count_are_known_apart() {
    let file = |attributes| Entry {
        attributes,
        ..entry(Some(0), "a.exe", EntryKind::File)
    };
    // The archive, normal and cloud state attributes alone are not counted (TASK-14 #5).
    let archived = file(FILE_ATTRIBUTE_ARCHIVE);
    let pinned = file(FILE_ATTRIBUTE_NORMAL | FILE_ATTRIBUTE_PINNED);
    assert_eq!(
        ignored_fields(&archived, &pinned, false),
        Fields::ATTRIBUTES
    );
    // With another attribute too, the difference counts.
    let hidden = file(FILE_ATTRIBUTE_HIDDEN);
    assert_eq!(ignored_fields(&archived, &hidden, false), Fields::default());
    // The last write time of a folder, but not of a file.
    let dir = |modified| Entry {
        modified,
        ..entry(Some(0), "d", EntryKind::Dir)
    };
    assert_eq!(ignored_fields(&dir(1), &dir(2), false), Fields::MODIFIED);
    let written = |modified| Entry {
        modified,
        ..entry(Some(0), "f.txt", EntryKind::File)
    };
    assert_eq!(
        ignored_fields(&written(1), &written(2), false),
        Fields::default()
    );
    // A version one snapshot did not read; the version and the target of an entry
    // inaccessible in either tree.
    let exe = |version| Entry {
        version,
        ..entry(Some(0), "a.exe", EntryKind::File)
    };
    let (one, two) = (exe(version([1, 0, 0, 0])), exe(version([2, 0, 0, 0])));
    assert_eq!(
        ignored_fields(&one, &exe(VersionInfo::NotRead), false),
        Fields::VERSION
    );
    assert_eq!(ignored_fields(&one, &two, false), Fields::default());
    assert_eq!(ignored_fields(&one, &two, true), Fields::VERSION);
    let link = |target: &str| Entry {
        link_target: Some(OsStr::new(target).into()),
        ..entry(Some(0), "l", EntryKind::Link)
    };
    assert_eq!(
        ignored_fields(&link("a"), &link("b"), true),
        Fields::LINK_TARGET
    );
    assert_eq!(
        ignored_fields(&link("a"), &link("b"), false),
        Fields::default()
    );
    // Sizes and creation times always count; names and file IDs are not among them.
    let mut renamed = entry(Some(0), "A.EXE", EntryKind::File);
    renamed.size = 5;
    renamed.created = 7;
    renamed.file_id = FileId([9; 16]);
    assert_eq!(
        ignored_fields(&archived, &renamed, false),
        Fields::default()
    );
}

#[test]
fn names_match_without_regard_to_case_and_a_name_of_another_case_is_renamed() {
    let old = sample();
    let mut new = sample();
    new.get("app").name = OsStr::new("APP").into();
    // Contents of a folder renamed in case only stay matched (TASK-42: renamed, where
    // TASK-10 #12 had it modified).
    assert_eq!(lines(&old.0, &new.0), ["> APP < app name"]);
    // Non-ASCII letters too.
    let mut lower = Tree::new();
    lower.add("Тека", EntryKind::Dir);
    let mut upper = Tree::new();
    upper.add("ТЕКА", EntryKind::Dir);
    assert_eq!(lines(&lower.0, &upper.0), ["> ТЕКА < Тека name"]);
    // What else differs is said with it, in the one change.
    let mut new = sample();
    let notes = new.get("Notes.txt");
    notes.name = OsStr::new("NOTES.TXT").into();
    notes.size = 7;
    assert_eq!(lines(&old.0, &new.0), ["> NOTES.TXT < Notes.txt name,size"]);
    // Where file IDs are not compared, the entry at the same path is the same entry.
    let other_volume = Rules {
        volumes: [1, 2],
        ..Rules::plain()
    };
    assert_eq!(
        lines_with(&old.0, &new.0, &other_volume),
        ["> NOTES.TXT < Notes.txt name,size"]
    );
    // Another file under the name of another case is replaced, not renamed.
    new.get("NOTES.TXT").file_id = FileId([7; 16]);
    assert_eq!(lines(&old.0, &new.0), ["~ Notes.txt name,size,file_id"]);
}

#[test]
fn the_same_name_is_matched_before_a_name_of_another_case() {
    // A folder made case-sensitive, for WSL, may hold both.
    let mut old = Tree::new();
    old.add("a.txt", EntryKind::File);
    let mut new = Tree::new();
    new.add("A.txt", EntryKind::File);
    new.add("a.txt", EntryKind::File);
    assert_eq!(lines(&old.0, &new.0), ["+ A.txt"]);
    assert_eq!(lines(&new.0, &old.0), ["- A.txt"]);

    // Both in both trees, in another order: each is matched with its own.
    let mut old = Tree::new();
    old.add("A.txt", EntryKind::File).size = 1;
    old.add("a.txt", EntryKind::File).size = 2;
    let mut new = Tree::new();
    new.add("a.txt", EntryKind::File).size = 2;
    new.add("A.txt", EntryKind::File).size = 1;
    assert_eq!(lines(&old.0, &new.0), Vec::<String>::new());

    // One of them gone: the other is still matched with its own, not renamed.
    let mut new = Tree::new();
    new.add("A.txt", EntryKind::File).size = 1;
    assert_eq!(lines(&old.0, &new.0), ["- a.txt"]);
}

#[test]
fn changed_link_target_is_a_change() {
    let old = sample();
    let mut new = sample();
    new.get("link").link_target = Some(OsStr::new(r"C:\Other").into());
    assert_eq!(lines(&old.0, &new.0), ["~ link target"]);
    // The times and attributes of a link count too, as those of a file do.
    let link = new.get("link");
    link.created = 1;
    link.modified = 2;
    link.attributes |= FILE_ATTRIBUTE_HIDDEN;
    assert_eq!(
        lines(&old.0, &new.0),
        ["~ link created,modified,attributes,target"]
    );
}

#[test]
fn changed_kind_is_deleted_and_added() {
    let old = sample();
    let mut new = sample();
    let notes = new.get("Notes.txt");
    notes.kind = EntryKind::Dir;
    notes.attributes = FILE_ATTRIBUTE_DIRECTORY;
    // Links have no children: drop those of the old folder.
    new.remove(r"app\data\a.dat");
    new.remove(r"app\data\b.dat");
    let data = new.get(r"app\data");
    data.kind = EntryKind::Link;
    data.link_target = Some(OsStr::new(r"D:\data").into());
    assert_eq!(
        lines(&old.0, &new.0),
        [
            r"- app\data",
            r"- app\data\a.dat",
            r"- app\data\b.dat",
            r"+ app\data",
            "- Notes.txt",
            "+ Notes.txt",
        ]
    );
}

#[test]
fn inaccessible_entries_are_listed_apart() {
    let old = sample();
    let mut new = sample();
    // An exe whose version cannot be read: the version is unknown, not changed.
    let exe = new.get(r"app\app.exe");
    exe.inaccessible = true;
    exe.version = VersionInfo::NotRead;
    // A link whose target cannot be read.
    let link = new.get("link");
    link.inaccessible = true;
    link.link_target = None;
    assert_eq!(lines(&old.0, &new.0), [r"? app\app.exe", "? link"]);
    // Inaccessible on both sides, nothing changed: still listed, but no changes.
    assert_eq!(lines(&new.0, &new.0), [r"? app\app.exe", "? link"]);
}

#[test]
fn known_fields_of_inaccessible_entries_are_compared() {
    let old = sample();
    let mut new = sample();
    let exe = new.get(r"app\app.exe");
    exe.inaccessible = true;
    exe.version = VersionInfo::NotRead;
    exe.size = 1;
    assert_eq!(
        lines(&old.0, &new.0),
        [r"~ app\app.exe size", r"? app\app.exe"]
    );
}

#[test]
fn inaccessible_entries_are_never_added_or_deleted() {
    let mut old = sample();
    old.add("locked", EntryKind::Dir).inaccessible = true;
    let mut new = sample();
    new.add("secret.sys", EntryKind::File).inaccessible = true;
    assert_eq!(lines(&old.0, &new.0), ["? locked", "? secret.sys"]);
}

#[test]
fn inaccessible_entries_say_where_they_stand_among_the_changes() {
    // Tree order: app, app\app.exe, app\readme.txt, …, link, locked, Notes.txt, z.txt.
    let old = sample();
    let mut new = sample();
    new.get(r"app\app.exe").size = 1;
    let exe = new.get(r"app\app.exe");
    exe.inaccessible = true;
    exe.version = VersionInfo::NotRead;
    new.add("locked", EntryKind::Dir).inaccessible = true;
    new.get("Notes.txt").size = 7;
    new.add("z.txt", EntryKind::File);
    let diff = diff(
        Subtree::whole(&old.0),
        Subtree::whole(&new.0),
        &Rules::plain(),
        &Cancel::new(),
    )
    .unwrap();
    let changes: Vec<String> = diff
        .changes
        .iter()
        .map(|c| rel_path(&new.0, c.new.unwrap()))
        .collect();
    assert_eq!(changes, [r"app\app.exe", "Notes.txt", "z.txt"]);
    // The exe is inaccessible and modified: listed before its change. The locked
    // folder stands between the exe and Notes.txt.
    let placed: Vec<(String, u32)> = diff
        .inaccessible
        .iter()
        .map(|i| (rel_path(&new.0, i.new.unwrap()), i.changes_before))
        .collect();
    assert_eq!(
        placed,
        [(r"app\app.exe".to_string(), 0), ("locked".to_string(), 1)]
    );
}

#[test]
fn children_of_an_unreadable_folder_are_not_added_or_deleted() {
    let old = sample();
    // The folder could not be listed this time: only part of it is known.
    let mut new = sample();
    new.get("app").inaccessible = true;
    new.remove(r"app\readme.txt");
    new.add(r"app\extra.txt", EntryKind::File);
    new.get(r"app\app.exe").size = 1;
    assert_eq!(lines(&old.0, &new.0), [r"~ app\app.exe size", "? app"]);
    // And the other way round.
    assert_eq!(lines(&new.0, &old.0), [r"~ app\app.exe size", "? app"]);
}

#[test]
fn order_of_entries_does_not_matter() {
    let old = sample();
    // The same tree with the entries in reverse order, except the root.
    let n = old.0.len() as u32;
    let map = |i: u32| if i == 0 { 0 } else { n - i };
    let mut reversed = vec![old.0[0].clone()];
    for i in (1..n).rev() {
        let mut e = old.0[i as usize].clone();
        e.parent = e.parent.map(map);
        reversed.push(e);
    }
    assert_eq!(lines(&old.0, &reversed), Vec::<String>::new());
}

#[test]
fn empty_snapshots() {
    let t = sample();
    assert_eq!(lines(&[], &[]), Vec::<String>::new());
    assert_eq!(lines(&[], &t.0[..1]), ["+ "]);
}

#[test]
fn cancelled_diff_returns_cancelled() {
    let t = synthetic(1000);
    assert!(diff_whole(&t, &t, &cancelled()).is_err());
}

/// Two snapshots with known changes, made from a generated tree; returns them with the
/// expected [`lines`].
fn known_changes(count: usize) -> (Vec<Entry>, Vec<Entry>, HashSet<String>) {
    let mut old = synthetic(count);
    // Some files are links in both snapshots.
    for (i, e) in old.iter_mut().enumerate() {
        if e.kind == EntryKind::File && i % 100 == 10 {
            e.kind = EntryKind::Link;
            e.version = VersionInfo::NotRead;
            e.link_target = Some(OsString::from(format!(r"C:\target {i}")).into_boxed_os_str());
        }
    }
    let n = old.len();
    let mut expected = HashSet::new();
    let p = |entries: &[Entry], i: usize| rel_path(entries, i as u32);
    let is_dir = |e: &Entry| e.kind == EntryKind::Dir;

    // Parents come before their children, so one pass sees the parent's fate first.
    // Deleted folders go with their subtrees. Folders that cannot be listed in the newer
    // snapshot lose their files there; those are skipped by the diff, so nothing is
    // expected for them, and no folder is deleted right under such a folder.
    let mut new = old.clone();
    let mut deleted = vec![false; n];
    let mut taken = vec![false; n];
    for i in 1..n {
        let parent = old[i].parent.unwrap() as usize;
        if deleted[parent] {
            deleted[i] = true;
        } else if new[parent].inaccessible {
            if !is_dir(&old[i]) {
                deleted[i] = true;
            }
        } else if is_dir(&old[i]) && i % 50 == 3 {
            deleted[i] = true;
        } else if is_dir(&old[i]) && i % 50 == 4 {
            new[i].inaccessible = true;
            expected.insert(format!("? {}", p(&old, i)));
        }
        if deleted[i] {
            taken[i] = true;
            if !new[parent].inaccessible {
                expected.insert(format!("- {}", p(&old, i)));
            }
        }
    }

    for i in 1..n {
        if taken[i] {
            continue;
        }
        let path = p(&old, i);
        let e = &mut new[i];
        if is_dir(e) {
            match i % 50 {
                1 => e.modified += 1,
                2 => {
                    e.attributes |= FILE_ATTRIBUTE_HIDDEN;
                    expected.insert(format!("~ {path} attributes"));
                }
                5 => {
                    let mut file =
                        entry(Some(i as u32), &format!("added {i}.txt"), EntryKind::File);
                    file.size = 1;
                    new.push(file);
                    let dir_index = new.len() as u32;
                    new.push(entry(
                        Some(i as u32),
                        &format!("new dir {i}"),
                        EntryKind::Dir,
                    ));
                    new.push(entry(Some(dir_index), "inside.txt", EntryKind::File));
                    for rel in [
                        format!("added {i}.txt"),
                        format!("new dir {i}"),
                        format!(r"new dir {i}\inside.txt"),
                    ] {
                        expected.insert(format!("+ {path}\\{rel}"));
                    }
                }
                _ => {}
            }
            continue;
        }
        let change = match i % 100 {
            1 => {
                e.size += 1;
                Some("size")
            }
            2 => {
                e.modified += 1;
                Some("modified")
            }
            3 => {
                e.created -= 1;
                Some("created")
            }
            4 => {
                e.attributes |= FILE_ATTRIBUTE_HIDDEN;
                Some("attributes")
            }
            5 => {
                e.attributes &= !FILE_ATTRIBUTE_ARCHIVE;
                None
            }
            6 => {
                // Renamed in case only: one change, at the newer path.
                let upper = e.name.to_string_lossy().to_uppercase();
                let renamed = match path.rsplit_once('\\') {
                    Some((folder, _)) => format!("{folder}\\{upper}"),
                    None => upper.clone(),
                };
                expected.insert(format!("> {renamed} < {path} name"));
                e.name = OsString::from(upper).into();
                None
            }
            7 => {
                deleted[i] = true;
                expected.insert(format!("- {path}"));
                None
            }
            8 => {
                e.kind = EntryKind::Dir;
                e.attributes = FILE_ATTRIBUTE_DIRECTORY;
                e.size = 0;
                e.version = VersionInfo::NotRead;
                expected.insert(format!("- {path}"));
                expected.insert(format!("+ {path}"));
                None
            }
            9 => {
                old[i].inaccessible = true;
                old[i].version = VersionInfo::NotRead;
                expected.insert(format!("? {path}"));
                None
            }
            10 if i % 200 == 10 => {
                e.link_target = Some(OsStr::new(r"C:\moved").into());
                Some("target")
            }
            20 => {
                // Renamed: the same file ID under another name.
                let renamed = match path.rsplit_once('\\') {
                    Some((folder, _)) => format!("{folder}\\renamed {i}.bin"),
                    None => format!("renamed {i}.bin"),
                };
                expected.insert(format!("> {renamed} < {path} name"));
                e.name = OsString::from(format!("renamed {i}.bin")).into();
                None
            }
            14 if matches!(e.version, VersionInfo::Some(_)) => {
                e.version = version([11, 0, 0, 0]);
                Some("version")
            }
            _ => None,
        };
        if let Some(field) = change {
            expected.insert(format!("~ {path} {field}"));
        }
    }

    // Drop deleted entries from the newer snapshot and renumber the rest.
    deleted.resize(new.len(), false);
    (old, renumber(new, &deleted), expected)
}

#[test]
fn generated_changes_give_exactly_the_expected_diff() {
    let (old, new, expected) = known_changes(50_000);
    let got = lines(&old, &new);
    let got_set: HashSet<String> = got.iter().cloned().collect();
    assert_eq!(got.len(), got_set.len(), "no entry is reported twice");
    let missing: Vec<_> = expected.difference(&got_set).take(10).collect();
    let extra: Vec<_> = got_set.difference(&expected).take(10).collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "missing {missing:?}, extra {extra:?}"
    );
    // Every kind of change is there.
    for prefix in ["+ ", "- ", "? ", "> "] {
        assert!(expected.iter().any(|l| l.starts_with(prefix)), "{prefix}");
    }
    for field in [
        "size",
        "modified",
        "created",
        "attributes",
        "target",
        "version",
    ] {
        assert!(
            expected
                .iter()
                .any(|l| l.starts_with("~ ") && l.ends_with(field)),
            "{field}"
        );
    }
}

#[test]
fn unchanged_folder_scanned_twice_has_no_changes() {
    let tmp = TempDir::new("diff");
    let root = tmp.path().join("tree");
    for i in 0..10 {
        let dir = root.join(format!("d{i}\\Sub Folder"));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(format!("f{i}.txt")), vec![b'x'; i * 100]).unwrap();
        fs::write(root.join(format!("d{i}\\Файл {i}.dat")), b"data").unwrap();
    }
    let system = std::env::var_os("SystemRoot").unwrap();
    fs::copy(
        Path::new(&system).join("System32\\kernel32.dll"),
        root.join("k.dll"),
    )
    .unwrap();
    junction(&root.join("link"), &root.join("d0"));

    let no_cancel = Cancel::new();
    let settings = ProfileSettings {
        name: "Diff".into(),
        scope: Scope {
            folders: vec![root.clone()],
            exclusions: vec![],
        },
    };
    let mut store = Store::create(&tmp.db(), &settings).unwrap();
    let progress = Progress::default();
    let first = scan(&root, &ScanOptions::default(), &no_cancel, &progress).unwrap();
    let a = store
        .save(&Snapshot::of_tree(first.entries), &no_cancel)
        .unwrap();
    let second = scan(&root, &ScanOptions::default(), &no_cancel, &progress).unwrap();
    let b = store
        .save(&Snapshot::of_tree(second.entries), &no_cancel)
        .unwrap();
    let (old, new) = (
        store.load_folder(a, 0, &Cancel::new()).unwrap(),
        store.load_folder(b, 0, &Cancel::new()).unwrap(),
    );
    assert_eq!(old.len(), 1 + 10 * 4 + 2);

    let diff = diff_whole(&old, &new, &no_cancel).unwrap();
    assert!(diff.changes.is_empty(), "{:?}", lines(&old, &new));
    assert!(diff.inaccessible.is_empty());
}

#[test]
fn the_entries_visited_are_counted_once_each() {
    let mut old = Tree::new();
    old.add("a.txt", EntryKind::File);
    old.add("sub", EntryKind::Dir);
    old.add(r"sub\b.txt", EntryKind::File);
    old.add("same", EntryKind::Dir);
    old.add(r"same\c.txt", EntryKind::File);
    let mut new = old.clone();
    // A kind change: visited as a deleted folder with its contents, then as an added file.
    new.remove("sub");
    new.add("sub", EntryKind::File);
    new.add("d.txt", EntryKind::File);
    let visited = AtomicU64::new(0);
    diff_counting(
        Subtree::whole(&old.0),
        Subtree::whole(&new.0),
        &Rules::plain(),
        &Cancel::new(),
        &visited,
    )
    .unwrap();
    assert_eq!(
        visited.load(std::sync::atomic::Ordering::Relaxed),
        (old.0.len() + new.0.len()) as u64
    );
    // A folder that moved is walked again with what it held, which counts nothing more.
    let old = sample().with_ids();
    let mut new = old.clone();
    new.rename("app", "app2");
    let visited = AtomicU64::new(0);
    let diff = diff_counting(
        Subtree::whole(&old.0),
        Subtree::whole(&new.0),
        &Rules::plain(),
        &Cancel::new(),
        &visited,
    )
    .unwrap();
    assert_eq!(diff.changes.len(), 1);
    assert_eq!(
        visited.load(std::sync::atomic::Ordering::Relaxed),
        (old.0.len() + new.0.len()) as u64
    );
}

// --- Renamed and moved entries (TASK-42) ---------------------------------------------
//
// The trees are those of one volume: `with_ids` gives every entry of the older one a file
// ID of its own, the newer one is a copy, and `rename` moves an entry in it as a volume
// does, with its file ID and its creation time.

#[test]
fn a_renamed_or_moved_file_is_one_change_at_its_newer_path() {
    let old = sample().with_ids();
    // Another name in the same folder.
    let mut new = old.clone();
    new.rename(r"app\readme.txt", r"app\README.md");
    assert_eq!(
        lines(&old.0, &new.0),
        [r"> app\README.md < app\readme.txt name"]
    );
    // Another folder, the same name.
    let mut new = old.clone();
    new.rename("Notes.txt", r"app\data\Notes.txt");
    assert_eq!(
        lines(&old.0, &new.0),
        [r"> app\data\Notes.txt < Notes.txt folder"]
    );
    // Both.
    let mut new = old.clone();
    new.rename(r"app\app.exe", "tool.exe");
    assert_eq!(
        lines(&old.0, &new.0),
        [r"> tool.exe < app\app.exe name,folder"]
    );
    // The other way round it is the same entry at its other path.
    assert_eq!(
        lines(&new.0, &old.0),
        [r"> app\app.exe < tool.exe name,folder"]
    );
}

#[test]
fn a_renamed_or_moved_link_or_folder_is_one_change_whatever_it_holds() {
    let old = sample().with_ids();
    let mut new = old.clone();
    new.rename("link", r"app\shortcut");
    assert_eq!(
        lines(&old.0, &new.0),
        [r"> app\shortcut < link name,folder"]
    );
    // A folder goes with what it holds, which is compared with what it held by the paths
    // below it: nothing inside changed, so the folder is the only change.
    let mut new = old.clone();
    new.rename(r"app\data", "archive");
    assert_eq!(lines(&old.0, &new.0), [r"> archive < app\data name,folder"]);
    let mut new = old.clone();
    new.rename("app", "App2");
    assert_eq!(lines(&old.0, &new.0), ["> App2 < app name"]);
    let mut new = old.clone();
    new.add("backup", EntryKind::Dir);
    new.rename("app", r"backup\app");
    assert_eq!(
        lines(&old.0, &new.0),
        ["+ backup", r"> backup\app < app folder"]
    );
}

#[test]
fn only_what_changed_inside_a_moved_folder_shows() {
    let mut tree = sample();
    tree.add(r"app\data\c.dat", EntryKind::File);
    tree.add(r"app\data\sub", EntryKind::Dir);
    tree.add(r"app\data\sub\d.dat", EntryKind::File);
    let old = tree.with_ids();
    let mut new = old.clone();
    new.rename(r"app\data", "archive");
    // Inside it a file was modified, one renamed, one deleted, one moved from a folder of
    // it to the folder itself, and one added.
    new.get(r"archive\a.dat").size = 7;
    new.rename(r"archive\b.dat", r"archive\B2.dat");
    new.remove(r"archive\c.dat");
    new.rename(r"archive\sub\d.dat", r"archive\d.dat");
    new.add(r"archive\new.dat", EntryKind::File);
    assert_eq!(
        lines(&old.0, &new.0),
        [
            r"> archive < app\data name,folder",
            r"~ app\data\a.dat size",
            // In the same folder, which is the one that moved: renamed, not moved.
            r"> archive\B2.dat < app\data\b.dat name",
            r"- app\data\c.dat",
            r"> archive\d.dat < app\data\sub\d.dat folder",
            r"+ archive\new.dat",
        ]
    );
    // An entry moved out of the folder, and one moved into it from outside.
    let mut new = old.clone();
    new.rename(r"app\data", "archive");
    new.rename(r"archive\a.dat", "a.dat");
    new.rename("Notes.txt", r"archive\Notes.txt");
    assert_eq!(
        lines(&old.0, &new.0),
        [
            r"> a.dat < app\data\a.dat folder",
            r"> archive < app\data name,folder",
            r"> archive\Notes.txt < Notes.txt folder",
        ]
    );
}

#[test]
fn entries_renamed_inside_folders_that_are_changes_themselves_are_found_too() {
    let old = sample().with_ids();
    // A folder moved into a new one, and inside it a file renamed and one deleted: the
    // new folder is added with what it holds, of which the moved one is compared with what
    // it held, and what is left over inside that is looked through again.
    let mut new = old.clone();
    new.add("backup", EntryKind::Dir);
    new.rename("app", r"backup\app");
    new.rename(r"backup\app\readme.txt", r"backup\app\README.md");
    new.remove(r"backup\app\data\b.dat");
    assert_eq!(
        lines(&old.0, &new.0),
        [
            "+ backup",
            r"> backup\app < app folder",
            r"- app\data\b.dat",
            r"> backup\app\README.md < app\readme.txt name",
        ]
    );
    // A folder replaced at its path — deleted and made again, while the older one is
    // nowhere else — is compared with the older one by path; a file renamed in it is
    // renamed, between the changes around it.
    let mut new = old.clone();
    new.get("app").file_id = file_id(100);
    new.get(r"app\app.exe").size = 1;
    new.rename(r"app\readme.txt", r"app\notes.txt");
    new.add(r"app\z.txt", EntryKind::File);
    assert_eq!(
        lines(&old.0, &new.0),
        [
            "~ app file_id",
            r"~ app\app.exe size",
            r"> app\notes.txt < app\readme.txt name",
            r"+ app\z.txt",
        ]
    );
    // The inaccessible entries keep their places among the changes after that: the
    // folder that could not be listed stands before the file added after it.
    new.add("locked", EntryKind::Dir).inaccessible = true;
    let diff = diff_whole(&old.0, &new.0, &Cancel::new()).unwrap();
    let placed: Vec<(String, u32)> = diff
        .inaccessible
        .iter()
        .map(|i| (rel_path(&new.0, i.new.unwrap()), i.changes_before))
        .collect();
    assert_eq!(placed, [("locked".to_string(), 4)]);
    assert_eq!(diff.changes.len(), 4);
}

#[test]
fn the_inaccessible_entries_keep_their_places_when_a_part_is_walked_again() {
    // Tree order: app, …, link, locked, Notes.txt → renamed to Zed.txt, z.txt.
    let mut tree = sample();
    tree.add("locked", EntryKind::Dir).inaccessible = true;
    let old = tree.with_ids();
    let mut new = old.clone();
    new.get(r"app\app.exe").size = 1;
    new.rename("Notes.txt", "Zed.txt");
    new.add("z.txt", EntryKind::File);
    let diff = diff_whole(&old.0, &new.0, &Cancel::new()).unwrap();
    let changes: Vec<String> = diff
        .changes
        .iter()
        .map(|c| rel_path(&new.0, c.new.unwrap()))
        .collect();
    assert_eq!(changes, [r"app\app.exe", "z.txt", "Zed.txt"]);
    // The locked folder stands after the exe and before the files that follow it, though
    // the two rows of the renamed file became one.
    let placed: Vec<u32> = diff.inaccessible.iter().map(|i| i.changes_before).collect();
    assert_eq!(placed, [1]);
}

#[test]
fn a_moved_folder_inaccessible_in_a_snapshot_has_nothing_added_or_deleted_inside() {
    let old = sample().with_ids();
    // Moved, and it could not be listed this time: only part of it is known (TASK-10 #16).
    let mut new = old.clone();
    new.rename(r"app\data", "archive");
    new.get("archive").inaccessible = true;
    new.remove(r"archive\b.dat");
    new.add(r"archive\extra.dat", EntryKind::File);
    new.get(r"archive\a.dat").size = 1;
    assert_eq!(
        lines(&old.0, &new.0),
        [
            r"> archive < app\data name,folder",
            r"~ app\data\a.dat size",
            r"? app\data",
        ]
    );
    // And the other way round: it could not be listed before it moved.
    assert_eq!(
        lines(&new.0, &old.0),
        [
            r"> app\data < archive name,folder",
            r"~ archive\a.dat size",
            "? archive",
        ]
    );
    // Not listed at all in either snapshot: still the same folder at another path.
    let mut before = old.clone();
    before.remove(r"app\data\a.dat");
    before.remove(r"app\data\b.dat");
    before.get(r"app\data").inaccessible = true;
    let mut after = before.clone();
    after.rename(r"app\data", "archive");
    assert_eq!(
        lines(&before.0, &after.0),
        [r"> archive < app\data name,folder", r"? app\data"]
    );
}

#[test]
fn a_file_put_in_the_place_of_one_that_moved_away_is_a_replacement() {
    // A program saves with a backup: the file takes the backup's name, and a new one,
    // with another file ID, takes the file's.
    let old = sample().with_ids();
    let mut new = old.clone();
    new.rename("Notes.txt", "Notes.bak");
    let saved = new.add("Notes.txt", EntryKind::File);
    saved.file_id = file_id(100);
    saved.size = 12;
    assert_eq!(
        lines(&old.0, &new.0),
        ["> Notes.bak < Notes.txt name", "~ Notes.txt size,file_id"]
    );
    // The file went to a path that was there already: it replaced the one there, and its
    // own path is gone.
    let mut new = old.clone();
    new.remove(r"app\data\b.dat");
    new.rename(r"app\data\a.dat", r"app\data\b.dat");
    assert_eq!(
        lines(&old.0, &new.0),
        [r"- app\data\a.dat", r"~ app\data\b.dat file_id"]
    );
}

#[test]
fn a_folder_in_the_place_of_one_that_moved_away_holds_nothing_of_it() {
    // An installer renames the folder of the old version and makes a new one in its
    // place (decided 2026-11-13).
    let mut tree = Tree::new();
    tree.add("Tool", EntryKind::Dir);
    tree.add(r"Tool\tool.exe", EntryKind::File);
    tree.add(r"Tool\readme.txt", EntryKind::File);
    let old = tree.with_ids();
    let mut new = old.clone();
    new.rename("Tool", "Tool.bak");
    new.get(r"Tool.bak\readme.txt").size = 5;
    for (rel, kind, id) in [
        ("Tool", EntryKind::Dir, 10),
        (r"Tool\tool.exe", EntryKind::File, 11),
        (r"Tool\new.dll", EntryKind::File, 12),
    ] {
        new.add(rel, kind).file_id = file_id(id);
    }
    assert_eq!(
        lines(&old.0, &new.0),
        [
            // The new folder is another one: what it holds is added, whatever its name.
            "~ Tool file_id",
            r"+ Tool\new.dll",
            r"+ Tool\tool.exe",
            // What the old one held is compared where it is now, once.
            "> Tool.bak < Tool name",
            r"~ Tool\readme.txt size",
        ]
    );
    // A file put back from the old folder into the new one, under its name, is where it
    // was: no change at all.
    let mut new = old.clone();
    new.rename("Tool", "Tool.bak");
    new.add("Tool", EntryKind::Dir).file_id = file_id(10);
    new.rename(r"Tool.bak\readme.txt", r"Tool\readme.txt");
    assert_eq!(
        lines(&old.0, &new.0),
        ["~ Tool file_id", "> Tool.bak < Tool name"]
    );
    // Put back and changed: modified, at its path.
    new.get(r"Tool\readme.txt").size = 9;
    assert_eq!(
        lines(&old.0, &new.0),
        [
            "~ Tool file_id",
            r"~ Tool\readme.txt size",
            "> Tool.bak < Tool name"
        ]
    );
}

#[test]
fn two_entries_that_swapped_their_names_are_both_replaced() {
    // Neither is at a path that was not there before, so neither moved.
    let old = sample().with_ids();
    let mut new = old.clone();
    new.rename(r"app\data\a.dat", r"app\data\x");
    new.rename(r"app\data\b.dat", r"app\data\a.dat");
    new.rename(r"app\data\x", r"app\data\b.dat");
    assert_eq!(
        lines(&old.0, &new.0),
        [r"~ app\data\a.dat file_id", r"~ app\data\b.dat file_id"]
    );
}

#[test]
fn a_moved_entry_that_changed_too_is_one_change() {
    let old = sample().with_ids();
    let mut new = old.clone();
    new.rename(r"app\app.exe", r"app\data\app.exe");
    let exe = new.get(r"app\data\app.exe");
    exe.size = 2000;
    exe.modified = 30;
    exe.attributes |= FILE_ATTRIBUTE_HIDDEN;
    exe.version = version([2, 0, 0, 0]);
    assert_eq!(
        lines(&old.0, &new.0),
        [r"> app\data\app.exe < app\app.exe size,modified,attributes,version,folder"]
    );
    let mut new = old.clone();
    new.rename("link", "Link 2");
    new.get("Link 2").link_target = Some(OsStr::new(r"C:\Other").into());
    assert_eq!(lines(&old.0, &new.0), ["> Link 2 < link name,target"]);
    // The last write time of a folder is not a change, moved or not.
    let mut new = old.clone();
    new.rename(r"app\data", "archive");
    new.get("archive").modified = 99;
    assert_eq!(lines(&old.0, &new.0), [r"> archive < app\data name,folder"]);
}

#[test]
fn a_file_id_that_several_candidates_have_pairs_nothing() {
    // The names of a hard link have one file ID, as most files of C:\Windows do.
    let link = file_id(50);
    let names = |names: &[&str]| {
        let mut tree = Tree::new().with_ids();
        for name in names {
            tree.add(name, EntryKind::File).file_id = link;
        }
        tree.0
    };
    // Two names gone and one new: which of the two it was is unknown.
    assert_eq!(
        lines(&names(&["one.dll", "two.dll"]), &names(&["three.dll"])),
        ["- one.dll", "+ three.dll", "- two.dll"]
    );
    // One name gone and two new.
    assert_eq!(
        lines(&names(&["one.dll"]), &names(&["three.dll", "two.dll"])),
        ["- one.dll", "+ three.dll", "+ two.dll"]
    );
    // A name that stays where it was is no candidate, so the other one is renamed.
    assert_eq!(
        lines(
            &names(&["keep.dll", "one.dll"]),
            &names(&["keep.dll", "two.dll"])
        ),
        ["> two.dll < one.dll name"]
    );
    // A new name of a file that stays is added, not moved.
    assert_eq!(
        lines(&names(&["keep.dll"]), &names(&["keep.dll", "two.dll"])),
        ["+ two.dll"]
    );
}

#[test]
fn an_entry_of_another_creation_time_or_kind_is_not_the_same_entry() {
    let old = sample().with_ids();
    // NTFS gave the file ID of a deleted file to a new one, made later (MS-FSCC).
    let mut new = old.clone();
    new.rename("Notes.txt", "Other.txt");
    new.get("Other.txt").created = 5;
    assert_eq!(lines(&old.0, &new.0), ["- Notes.txt", "+ Other.txt"]);
    let mut new = old.clone();
    new.rename("Notes.txt", "Other");
    let other = new.get("Other");
    other.kind = EntryKind::Dir;
    other.attributes = FILE_ATTRIBUTE_DIRECTORY;
    assert_eq!(lines(&old.0, &new.0), ["- Notes.txt", "+ Other"]);
}

#[test]
fn entries_of_trees_on_different_volumes_are_not_paired() {
    let old = sample().with_ids();
    let mut new = old.clone();
    new.rename("Notes.txt", "Other.txt");
    new.rename(r"app\data", "archive");
    let other_volume = Rules {
        volumes: [1, 2],
        ..Rules::plain()
    };
    assert_eq!(
        lines_with(&old.0, &new.0, &other_volume),
        [
            r"- app\data",
            r"- app\data\a.dat",
            r"- app\data\b.dat",
            "+ archive",
            r"+ archive\a.dat",
            r"+ archive\b.dat",
            "- Notes.txt",
            "+ Other.txt",
        ]
    );
}

#[test]
fn the_file_ids_zero_and_all_ones_pair_nothing() {
    // They mean that the file has no ID (MS-FSCC 2.1.10).
    for id in [FileId([0; 16]), FileId([0xFF; 16])] {
        let mut old = Tree::new();
        old.add("a.txt", EntryKind::File).file_id = id;
        let mut new = Tree::new();
        new.add("b.txt", EntryKind::File).file_id = id;
        assert_eq!(lines(&old.0, &new.0), ["- a.txt", "+ b.txt"]);
    }
}

#[test]
fn an_entry_can_move_to_the_path_of_an_entry_of_another_kind() {
    // The file was deleted and the folder took its name: an entry whose kind changed is
    // deleted and added, so the added one may be an entry that moved.
    let mut tree = Tree::new();
    tree.add("x", EntryKind::File);
    tree.add("y", EntryKind::Dir);
    tree.add(r"y\f.txt", EntryKind::File);
    let old = tree.with_ids();
    let mut new = old.clone();
    new.remove("x");
    new.rename("y", "x");
    assert_eq!(lines(&old.0, &new.0), ["- x", "> x < y name"]);
}

#[test]
fn inside_a_moved_folder_what_is_excluded_at_either_place_is_left_out() {
    let old = sample().with_ids();
    let mut new = old.clone();
    new.rename("app", "app2");
    new.add(r"app2\cache", EntryKind::Dir);
    new.add(r"app2\cache\c.bin", EntryKind::File);
    // The snapshots left `cache` out where the folder was, so the older tree has none; the
    // pattern does not name the place the folder is at now, so the newer tree has it.
    let exclusions = Exclusions::new(&[r"C:\Data\app\cache"], &RecordedEnvironment);
    assert_eq!(
        lines_with(&old.0, &new.0, &rules(&exclusions, r"C:\Data")),
        ["> app2 < app name"]
    );
    assert_eq!(
        lines(&old.0, &new.0),
        ["> app2 < app name", r"+ app2\cache", r"+ app2\cache\c.bin"]
    );
    // Excluded where the folder is now: what it held there before is not deleted.
    let exclusions = Exclusions::new(&[r"C:\Data\app2\data"], &RecordedEnvironment);
    assert_eq!(
        lines_with(&old.0, &new.0, &rules(&exclusions, r"C:\Data")),
        ["> app2 < app name", r"+ app2\cache", r"+ app2\cache\c.bin"]
    );
}

#[test]
fn a_cancelled_search_for_moved_entries_returns_cancelled() {
    // The flag is honoured between the walks too.
    let old = synthetic(2000);
    let mut new = old.clone();
    new[1].name = OsStr::new("renamed").into();
    assert!(diff_whole(&old, &new, &cancelled()).is_err());
    let moved = diff_whole(&old, &new, &Cancel::new()).unwrap();
    assert_eq!(
        moved.changes.len(),
        1,
        "one folder renamed with all it holds"
    );
    assert_eq!(moved.changes[0].kind, ChangeKind::Moved);
}
