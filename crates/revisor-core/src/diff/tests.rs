use super::*;
use crate::scan::{FileId, ScanOptions, scan, synthetic};
use crate::store::Store;
use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::AtomicUsize;

const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;

/// Builds small trees by path, relative to the root `C:\Data`.
#[derive(Clone)]
struct Tree(Vec<Entry>);

impl Tree {
    fn new() -> Self {
        Tree(vec![entry(None, r"C:\Data", EntryKind::Dir)])
    }

    fn index(&self, rel: &str) -> u32 {
        (0..self.0.len() as u32)
            .find(|&i| rel_path(&self.0, i) == rel)
            .unwrap_or_else(|| panic!("{rel} not found"))
    }

    fn add(&mut self, rel: &str, kind: EntryKind) -> &mut Entry {
        let (parent, name) = rel.rsplit_once('\\').unwrap_or(("", rel));
        let parent = self.index(parent);
        self.0.push(entry(Some(parent), name, kind));
        self.0.last_mut().unwrap()
    }

    fn get(&mut self, rel: &str) -> &mut Entry {
        let i = self.index(rel);
        &mut self.0[i as usize]
    }

    /// Removes an entry with its subtree.
    fn remove(&mut self, rel: &str) {
        let gone = self.index(rel);
        let mut removed = vec![false; self.0.len()];
        removed[gone as usize] = true;
        for i in 0..self.0.len() {
            if let Some(p) = self.0[i].parent {
                removed[i] |= removed[p as usize];
            }
        }
        self.0 = renumber(std::mem::take(&mut self.0), &removed);
    }
}

/// Drops the entries marked in `removed` and fixes the parents of the rest. Parents must
/// come before their children.
fn renumber(entries: Vec<Entry>, removed: &[bool]) -> Vec<Entry> {
    let mut map = vec![u32::MAX; entries.len()];
    let mut kept = Vec::with_capacity(entries.len());
    for (i, e) in entries.into_iter().enumerate() {
        if !removed[i] {
            map[i] = kept.len() as u32;
            kept.push(e);
        }
    }
    for e in &mut kept {
        e.parent = e.parent.map(|p| map[p as usize]);
    }
    kept
}

fn entry(parent: Option<u32>, name: &str, kind: EntryKind) -> Entry {
    Entry {
        parent,
        name: OsStr::new(name).into(),
        kind,
        inaccessible: false,
        attributes: if kind == EntryKind::Dir {
            FILE_ATTRIBUTE_DIRECTORY
        } else {
            FILE_ATTRIBUTE_ARCHIVE
        },
        size: 0,
        created: 0,
        modified: 0,
        file_id: FileId::default(),
        version: None,
        link_target: None,
    }
}

/// Path relative to the root, with `\` separators; "" for the root.
fn rel_path(entries: &[Entry], index: u32) -> String {
    let full = path(entries, index);
    let root = Path::new(&*entries[0].name);
    full.strip_prefix(root)
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

fn field_names(fields: Fields) -> String {
    let names = [
        (Fields::NAME, "name"),
        (Fields::SIZE, "size"),
        (Fields::CREATED, "created"),
        (Fields::MODIFIED, "modified"),
        (Fields::ATTRIBUTES, "attributes"),
        (Fields::VERSION, "version"),
        (Fields::LINK_TARGET, "target"),
    ];
    let present: Vec<&str> = names
        .iter()
        .filter(|(f, _)| fields.contains(*f))
        .map(|(_, n)| *n)
        .collect();
    present.join(",")
}

/// The diff as lines: `+ path`, `- path`, `~ path fields`, then `? path` for the
/// inaccessible entries. Paths of removed, modified and inaccessible entries are those in
/// the older snapshot.
fn lines(old: &[Entry], new: &[Entry]) -> Vec<String> {
    let diff = diff(old, new, &AtomicBool::new(false)).unwrap();
    let mut out: Vec<String> = diff
        .changes
        .iter()
        .map(|c| match c.kind {
            ChangeKind::Added => format!("+ {}", rel_path(new, c.new.unwrap())),
            ChangeKind::Removed => format!("- {}", rel_path(old, c.old.unwrap())),
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
    let exe = t.add(r"app\app.exe", EntryKind::File);
    exe.size = 1000;
    exe.created = 10;
    exe.modified = 20;
    exe.version = Some([1, 2, 3, 4]);
    t.add(r"app\readme.txt", EntryKind::File).size = 5;
    t.add(r"app\data", EntryKind::Dir);
    t.add(r"app\data\a.dat", EntryKind::File);
    t.add(r"app\data\b.dat", EntryKind::File);
    t.add("link", EntryKind::Link).link_target = Some(OsStr::new(r"C:\Target").into());
    t.add("Notes.txt", EntryKind::File);
    t
}

#[test]
fn same_snapshot_has_no_changes() {
    let t = sample();
    assert_eq!(lines(&t.0, &t.0), Vec::<String>::new());
    let big = synthetic(10_000);
    assert!(
        diff(&big, &big, &AtomicBool::new(false))
            .unwrap()
            .changes
            .is_empty()
    );
}

#[test]
fn added_and_removed_files() {
    let old = sample();
    let mut new = sample();
    new.add(r"app\new.dll", EntryKind::File);
    new.remove("Notes.txt");
    assert_eq!(lines(&old.0, &new.0), [r"+ app\new.dll", "- Notes.txt"]);
}

#[test]
fn added_and_removed_folders_list_every_entry_inside() {
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
    new.get(r"app\app.exe").version = Some([1, 2, 3, 5]);
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
fn changed_link_target_is_a_change() {
    let old = sample();
    let mut new = sample();
    new.get("link").link_target = Some(OsStr::new(r"C:\Other").into());
    assert_eq!(lines(&old.0, &new.0), ["~ link target"]);
}

#[test]
fn changed_kind_is_removed_and_added() {
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
    exe.version = None;
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
    exe.version = None;
    exe.size = 1;
    assert_eq!(
        lines(&old.0, &new.0),
        [r"~ app\app.exe size", r"? app\app.exe"]
    );
}

#[test]
fn inaccessible_entries_are_never_added_or_removed() {
    let mut old = sample();
    old.add("locked", EntryKind::Dir).inaccessible = true;
    let mut new = sample();
    new.add("secret.sys", EntryKind::File).inaccessible = true;
    assert_eq!(lines(&old.0, &new.0), ["? locked", "? secret.sys"]);
}

#[test]
fn children_of_an_unreadable_folder_are_not_added_or_removed() {
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
    assert!(diff(&t, &t, &AtomicBool::new(true)).is_err());
}

/// Two snapshots with known changes, made from a generated tree; returns them with the
/// expected [`lines`].
fn known_changes(count: usize) -> (Vec<Entry>, Vec<Entry>, HashSet<String>) {
    let mut old = synthetic(count);
    // Some files are links in both snapshots.
    for (i, e) in old.iter_mut().enumerate() {
        if e.kind == EntryKind::File && i % 100 == 10 {
            e.kind = EntryKind::Link;
            e.version = None;
            e.link_target = Some(OsString::from(format!(r"C:\target {i}")).into_boxed_os_str());
        }
    }
    let n = old.len();
    let mut expected = HashSet::new();
    let p = |entries: &[Entry], i: usize| rel_path(entries, i as u32);
    let is_dir = |e: &Entry| e.kind == EntryKind::Dir;

    // Parents come before their children, so one pass sees the parent's fate first.
    // Removed folders go with their subtrees. Folders that cannot be listed in the newer
    // snapshot lose their files there; those are skipped by the diff, so nothing is
    // expected for them, and no folder is removed right under such a folder.
    let mut new = old.clone();
    let mut removed = vec![false; n];
    let mut taken = vec![false; n];
    for i in 1..n {
        let parent = old[i].parent.unwrap() as usize;
        if removed[parent] {
            removed[i] = true;
        } else if new[parent].inaccessible {
            if !is_dir(&old[i]) {
                removed[i] = true;
            }
        } else if is_dir(&old[i]) && i % 50 == 3 {
            removed[i] = true;
        } else if is_dir(&old[i]) && i % 50 == 4 {
            new[i].inaccessible = true;
            expected.insert(format!("? {}", p(&old, i)));
        }
        if removed[i] {
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
                removed[i] = true;
                expected.insert(format!("- {path}"));
                None
            }
            8 => {
                e.kind = EntryKind::Dir;
                e.attributes = FILE_ATTRIBUTE_DIRECTORY;
                e.size = 0;
                e.version = None;
                expected.insert(format!("- {path}"));
                expected.insert(format!("+ {path}"));
                None
            }
            9 => {
                old[i].inaccessible = true;
                old[i].version = None;
                expected.insert(format!("? {path}"));
                None
            }
            10 if i % 200 == 10 => {
                e.link_target = Some(OsStr::new(r"C:\moved").into());
                Some("target")
            }
            14 if e.version.is_some() => {
                e.version = Some([11, 0, 0, 0]);
                Some("version")
            }
            _ => None,
        };
        if let Some(field) = change {
            expected.insert(format!("~ {path} {field}"));
        }
    }

    // Drop removed entries from the newer snapshot and renumber the rest.
    removed.resize(new.len(), false);
    (old, renumber(new, &removed), expected)
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

/// A fresh folder under the system temp folder, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("revisor-diff-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn unchanged_folder_scanned_twice_has_no_changes() {
    let tmp = TempDir::new();
    let root = tmp.0.join("tree");
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
    let status = Command::new("cmd")
        .arg("/c")
        .arg("mklink")
        .arg("/J")
        .arg(root.join("link"))
        .arg(root.join("d0"))
        .output()
        .unwrap()
        .status;
    assert!(status.success());

    let no_cancel = AtomicBool::new(false);
    let mut store = Store::open(&tmp.0.join("profile.db")).unwrap();
    let first = scan(&root, &ScanOptions::default(), &no_cancel).unwrap();
    let a = store.save(None, &first.entries, &no_cancel).unwrap();
    let second = scan(&root, &ScanOptions::default(), &no_cancel).unwrap();
    let b = store.save(None, &second.entries, &no_cancel).unwrap();
    let (old, new) = (store.load(a).unwrap(), store.load(b).unwrap());
    assert_eq!(old.len(), 1 + 10 * 4 + 2);

    let diff = diff(&old, &new, &no_cancel).unwrap();
    assert!(diff.changes.is_empty(), "{:?}", lines(&old, &new));
    assert!(diff.inaccessible.is_empty());
}
