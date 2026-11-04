use super::*;
use crate::exclude::RecordedEnvironment;
use crate::scan::{FileId, Progress, ScanOptions, Version, VersionInfo, scan, synthetic};
use crate::store::{ProfileSettings, Scope, Snapshot, Store};
use crate::testing::{TempDir, Tree, entry, field_names, junction, rel_path, renumber};
use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::Path;

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

/// The diff of two whole trees as lines: `+ path`, `- path`, `~ path fields`, then
/// `? path` for the inaccessible entries. Paths of deleted, modified and inaccessible
/// entries are those in the older snapshot.
fn lines(old: &[Entry], new: &[Entry]) -> Vec<String> {
    lines_with(old, new, &Rules::plain())
}

/// [`lines`] of the whole trees compared by `rules`.
fn lines_with(old: &[Entry], new: &[Entry], rules: &Rules<'_>) -> Vec<String> {
    let diff = diff(
        Subtree::whole(old),
        Subtree::whole(new),
        rules,
        &AtomicBool::new(false),
    )
    .unwrap();
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
        file_ids: true,
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
        file_ids: false,
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
    let no_cancel = AtomicBool::new(false);
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
fn diff_whole(old: &[Entry], new: &[Entry], cancel: &AtomicBool) -> Result<Diff, Cancelled> {
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
        diff_whole(&big, &big, &AtomicBool::new(false))
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
    new.get("Notes.txt").name = OsStr::new("NOTES.TXT").into();
    assert_eq!(
        lines(&old.0, &new.0),
        [
            r"~ app\app.exe size",
            r"~ app\data\a.dat created",
            r"~ app\data\b.dat attributes",
            r"~ app\readme.txt modified",
            "~ Notes.txt name",
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
fn names_match_without_regard_to_case() {
    let old = sample();
    let mut new = sample();
    new.get("app").name = OsStr::new("APP").into();
    // Contents of a folder renamed in case only stay matched.
    assert_eq!(lines(&old.0, &new.0), ["~ app name"]);
    // Non-ASCII letters too.
    let mut old = Tree::new();
    old.add("Тека", EntryKind::Dir);
    let mut new = Tree::new();
    new.add("ТЕКА", EntryKind::Dir);
    assert_eq!(lines(&old.0, &new.0), ["~ Тека name"]);
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
    assert!(diff_whole(&t, &t, &AtomicBool::new(true)).is_err());
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
                e.name = OsString::from(e.name.to_string_lossy().to_uppercase()).into();
                Some("name")
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
    for prefix in ["+ ", "- ", "? "] {
        assert!(expected.iter().any(|l| l.starts_with(prefix)), "{prefix}");
    }
    for field in [
        "size",
        "modified",
        "created",
        "attributes",
        "name",
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

    let no_cancel = AtomicBool::new(false);
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
        store.load_folder(a, 0, &AtomicBool::new(false)).unwrap(),
        store.load_folder(b, 0, &AtomicBool::new(false)).unwrap(),
    );
    assert_eq!(old.len(), 1 + 10 * 4 + 2);

    let diff = diff_whole(&old, &new, &no_cancel).unwrap();
    assert!(diff.changes.is_empty(), "{:?}", lines(&old, &new));
    assert!(diff.inaccessible.is_empty());
}
