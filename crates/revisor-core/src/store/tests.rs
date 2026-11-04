use super::*;
use crate::scan::synthetic;
use crate::testing::TempDir;
use std::fs;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const FILE_ATTRIBUTE_ARCHIVE: u32 = 0x20;
const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;

fn user_version(path: &Path) -> i64 {
    rusqlite::Connection::open(path)
        .unwrap()
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap()
}

fn settings() -> ProfileSettings {
    ProfileSettings {
        name: "Дім".into(),
        scope: Scope {
            folders: vec![r"C:\Program Files".into(), r"D:\Робота".into()],
            exclusions: vec!["*.tmp".into(), r"%TEMP%".into()],
        },
    }
}

/// A new profile database with `settings()`.
fn create(path: &Path) -> Store {
    Store::create(path, &settings()).unwrap()
}

/// Saves one tree without cancellation.
fn save(store: &mut Store, entries: &[Entry]) -> i64 {
    store
        .save(
            &Snapshot::of_tree(entries.to_vec()),
            &AtomicBool::new(false),
        )
        .unwrap()
}

/// Loads one tree without cancellation.
fn load(store: &Store, id: i64, position: usize) -> Vec<Entry> {
    store
        .load_folder(id, position, &AtomicBool::new(false))
        .unwrap()
}

fn ids(store: &Store) -> Vec<i64> {
    store.list().unwrap().iter().map(|s| s.id).collect()
}

/// Waits until a write transaction on the database in `tmp` is open.
fn wait_for_journal(tmp: &TempDir, mut still_running: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(60);
    while !tmp.journal().exists() {
        assert!(Instant::now() < deadline, "saving never started");
        assert!(still_running(), "saving ended before it was interrupted");
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn new_database_has_schema_version_1() {
    let tmp = TempDir::new("store");
    create(&tmp.db());
    assert_eq!(user_version(&tmp.db()), 1);
}

#[test]
fn database_from_newer_revisor_is_refused() {
    let tmp = TempDir::new("store");
    drop(create(&tmp.db()));
    let conn = rusqlite::Connection::open(tmp.db()).unwrap();
    conn.pragma_update(None, "user_version", 2).unwrap();
    drop(conn);
    assert!(matches!(
        Store::open(&tmp.db()),
        Err(StoreError::NewerSchema(2))
    ));
}

#[test]
fn a_profile_keeps_its_settings_in_its_database() {
    let tmp = TempDir::new("store");
    drop(create(&tmp.db()));
    assert_eq!(
        Store::open(&tmp.db()).unwrap().settings().unwrap(),
        settings()
    );
}

#[test]
fn changed_settings_replace_the_old_ones() {
    let tmp = TempDir::new("store");
    let mut store = create(&tmp.db());
    let changed = ProfileSettings {
        name: "Робота".into(),
        scope: Scope {
            folders: vec![r"E:\".into()],
            exclusions: vec![],
        },
    };
    store.set_settings(&changed).unwrap();
    drop(store);
    assert_eq!(Store::open(&tmp.db()).unwrap().settings().unwrap(), changed);
}

/// A snapshot of the folders of `settings()`: the first scanned as `sample()`, the second
/// missing; with the exclusions of `settings()` as they applied.
fn snapshot_of_settings(comment: Option<&str>) -> Snapshot {
    Snapshot {
        taken_at: 1_793_000_000_000,
        comment: comment.map(String::from),
        folders: vec![
            FolderSnapshot {
                path: r"C:\Program Files".into(),
                state: FolderState::Scanned {
                    volume_serial: 0x1234_5678_9ABC_DEF0,
                    real_path: r"C:\Program Files".into(),
                },
                entries: sample(),
            },
            FolderSnapshot {
                path: r"D:\Робота".into(),
                state: FolderState::Missing,
                entries: Vec::new(),
            },
        ],
        exclusions: vec![
            SnapshotExclusion {
                pattern: "*.tmp".into(),
                applied: Some("*.tmp".into()),
            },
            SnapshotExclusion {
                pattern: "%TEMP%".into(),
                applied: Some(r"C:\Users\Руслан\AppData\Local\Temp".into()),
            },
        ],
    }
}

#[test]
fn a_snapshot_keeps_the_folders_and_exclusions_it_was_taken_with() {
    let tmp = TempDir::new("store");
    let mut store = create(&tmp.db());
    let before = store
        .save(&snapshot_of_settings(None), &AtomicBool::new(false))
        .unwrap();
    let mut changed = settings();
    changed.scope.folders.remove(0);
    changed.scope.exclusions.push("Code Cache".into());
    store.set_settings(&changed).unwrap();
    let after = save(&mut store, &sample());

    assert_eq!(store.snapshot_scope(before).unwrap(), settings().scope);
    assert_eq!(
        store.snapshot_scope(after).unwrap(),
        Scope {
            folders: vec![r"C:\Data".into()],
            exclusions: vec![],
        }
    );
    store.delete(before).unwrap();
    assert_eq!(store.snapshot_scope(before).unwrap(), Scope::default());
}

#[test]
fn each_folder_of_a_snapshot_keeps_its_state_and_its_tree() {
    let tmp = TempDir::new("store");
    let mut store = create(&tmp.db());
    let mut snapshot = snapshot_of_settings(Some("states"));
    let unavailable = [
        Unavailable::NotNtfs("FAT32".into()),
        Unavailable::Network,
        Unavailable::Error(21),
        Unavailable::NotAFolder,
    ];
    for (i, reason) in unavailable.iter().enumerate() {
        snapshot.folders.push(FolderSnapshot {
            path: format!(r"E:\folder {i}").into(),
            state: FolderState::Unavailable(reason.clone()),
            entries: Vec::new(),
        });
    }
    snapshot.folders.push(FolderSnapshot {
        path: r"F:\Big".into(),
        // Reached through a junction: its real location is elsewhere.
        state: FolderState::Scanned {
            volume_serial: 7,
            real_path: r"G:\Real\Big".into(),
        },
        entries: synthetic(100),
    });
    let id = store.save(&snapshot, &AtomicBool::new(false)).unwrap();
    drop(store);

    let store = Store::open(&tmp.db()).unwrap();
    let folders = store.snapshot_folders(id).unwrap();
    let expected: Vec<SnapshotFolder> = snapshot.folders.iter().map(SnapshotFolder::from).collect();
    assert_eq!(folders, expected);
    assert_same(&load(&store, id, 0), &sample());
    for position in 1..6 {
        assert!(load(&store, id, position).is_empty());
    }
    // The second tree comes back with its parents counted from its own root.
    assert_same(&load(&store, id, 6), &synthetic(100));
    assert!(load(&store, id, 7).is_empty());
    assert!(load(&store, id + 1, 0).is_empty());
    assert!(store.snapshot_folders(id + 1).unwrap().is_empty());
}

#[test]
fn exclusions_keep_how_they_applied() {
    let tmp = TempDir::new("store");
    let mut store = create(&tmp.db());
    let mut snapshot = snapshot_of_settings(None);
    snapshot.exclusions.push(SnapshotExclusion {
        pattern: r"%NoSuchVariable%\x".into(),
        applied: None,
    });
    let id = store.save(&snapshot, &AtomicBool::new(false)).unwrap();
    assert_eq!(store.snapshot_exclusions(id).unwrap(), snapshot.exclusions);
    assert_eq!(
        store.snapshot_scope(id).unwrap().exclusions,
        ["*.tmp", "%TEMP%", r"%NoSuchVariable%\x"]
    );
}

#[test]
fn only_an_existing_profile_database_opens() {
    let tmp = TempDir::new("store");
    assert!(Store::open(&tmp.db()).is_err());
    assert!(!tmp.db().exists(), "opening creates nothing");

    // Another program's SQLite database is not a profile.
    let other = rusqlite::Connection::open(tmp.db()).unwrap();
    other.execute_batch("CREATE TABLE t (x)").unwrap();
    drop(other);
    assert!(matches!(
        Store::open(&tmp.db()),
        Err(StoreError::NotAProfile)
    ));
}

#[test]
fn a_file_that_is_not_a_database_is_not_a_profile() {
    let tmp = TempDir::new("store");
    fs::write(tmp.db(), b"some text, not a database at all").unwrap();
    assert!(matches!(
        Store::open(&tmp.db()),
        Err(StoreError::NotAProfile)
    ));
}

#[test]
fn a_profile_database_without_its_profile_row_is_not_a_profile() {
    let tmp = TempDir::new("store");
    drop(create(&tmp.db()));
    let conn = rusqlite::Connection::open(tmp.db()).unwrap();
    conn.execute("DELETE FROM profile", []).unwrap();
    drop(conn);
    let store = Store::open(&tmp.db()).unwrap();
    assert!(matches!(store.settings(), Err(StoreError::NotAProfile)));
}

#[test]
fn creating_a_profile_never_takes_over_an_existing_file() {
    let tmp = TempDir::new("store");
    fs::write(tmp.db(), b"someone's file").unwrap();
    assert!(Store::create(&tmp.db(), &settings()).is_err());
    assert_eq!(fs::read(tmp.db()).unwrap(), b"someone's file");
}

fn entry(parent: Option<u32>, name: OsString, kind: EntryKind) -> Entry {
    Entry {
        parent,
        name: name.into_boxed_os_str(),
        kind,
        inaccessible: false,
        attributes: FILE_ATTRIBUTE_ARCHIVE,
        size: 0,
        created: 0,
        modified: 0,
        file_id: FileId::default(),
        version: VersionInfo::NotRead,
        link_target: None,
    }
}

/// A small tree with every kind of value a scan can produce.
fn sample() -> Vec<Entry> {
    let mut root = entry(None, r"C:\Data".into(), EntryKind::Dir);
    root.attributes = FILE_ATTRIBUTE_DIRECTORY;
    root.created = 133_000_000_000_000_000;
    root.modified = 134_000_000_000_000_000;
    root.file_id = FileId([0xAB; 16]);

    let mut dll = entry(Some(0), "Тест.dll".into(), EntryKind::File);
    dll.size = u64::MAX >> 1;
    dll.created = -1;
    dll.modified = i64::MAX;
    dll.file_id = FileId(*b"0123456789abcdef");
    dll.version = VersionInfo::Some(Version {
        file: [65535, 0, 1, 65535],
        product: [1, 2, 3, 4],
    });

    // Read, but without a version resource.
    let mut plain_exe = entry(Some(0), "plain.exe".into(), EntryKind::File);
    plain_exe.version = VersionInfo::None;

    let mut locked = entry(Some(0), "locked".into(), EntryKind::Dir);
    locked.inaccessible = true;
    locked.attributes = FILE_ATTRIBUTE_DIRECTORY;

    let mut junction = entry(Some(0), "junction".into(), EntryKind::Link);
    junction.link_target = Some(OsStr::new(r"C:\Elsewhere").into());

    let mut unreadable_link = entry(Some(0), "broken".into(), EntryKind::Link);
    unreadable_link.inaccessible = true;

    // Names on NTFS may hold unpaired surrogates, which are not valid Unicode.
    let odd = OsString::from_wide(&[0x61, 0xD800, 0x62]);
    let mut odd_link = entry(Some(3), odd.clone(), EntryKind::Link);
    odd_link.link_target = Some(odd.into_boxed_os_str());

    vec![
        root,
        dll,
        plain_exe,
        locked,
        junction,
        unreadable_link,
        odd_link,
    ]
}

fn assert_same(loaded: &[Entry], expected: &[Entry]) {
    assert_eq!(loaded.len(), expected.len());
    for (i, (a, b)) in loaded.iter().zip(expected).enumerate() {
        assert_eq!(format!("{a:?}"), format!("{b:?}"), "entry {i}");
    }
}

#[test]
fn saved_snapshot_loads_back_unchanged() {
    let tmp = TempDir::new("store");
    let mut store = create(&tmp.db());
    let id = store
        .save(
            &snapshot_of_settings(Some("before install")),
            &AtomicBool::new(false),
        )
        .unwrap();
    drop(store);

    let store = Store::open(&tmp.db()).unwrap();
    assert_same(&load(&store, id, 0), &sample());
    let list = store.list().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, id);
    assert_eq!(list[0].taken_at, 1_793_000_000_000);
    assert_eq!(list[0].comment.as_deref(), Some("before install"));
}

#[test]
fn snapshots_are_kept_apart() {
    let tmp = TempDir::new("store");
    let mut store = create(&tmp.db());
    let small = save(&mut store, &sample());
    let big = save(&mut store, &synthetic(5000));

    assert_same(&load(&store, small, 0), &sample());
    assert_same(&load(&store, big, 0), &synthetic(5000));
    assert_eq!(ids(&store), [small, big]);
    assert_eq!(store.list().unwrap()[0].comment, None);
}

#[test]
fn a_cancelled_load_stops_at_once() {
    let tmp = TempDir::new("store");
    let mut store = create(&tmp.db());
    let id = save(&mut store, &synthetic(10_000));
    let cancelled = store.load_folder(id, 0, &AtomicBool::new(true));
    assert!(
        matches!(cancelled, Err(StoreError::Cancelled)),
        "{cancelled:?}"
    );
    assert_eq!(load(&store, id, 0).len(), 10_000);
}

#[test]
fn cancelled_save_leaves_nothing_behind() {
    let tmp = TempDir::new("store");
    let mut store = create(&tmp.db());
    let kept = save(&mut store, &sample());

    let snapshot = Snapshot::of_tree(synthetic(1_000_000));
    let cancel = AtomicBool::new(false);
    let saving = AtomicBool::new(true);
    let result = thread::scope(|s| {
        s.spawn(|| {
            // Let the save write a good part of its entries before cancelling.
            wait_for_journal(&tmp, || saving.load(Ordering::Relaxed));
            thread::sleep(Duration::from_millis(50));
            cancel.store(true, Ordering::Relaxed);
        });
        let result = store.save(&snapshot, &cancel);
        saving.store(false, Ordering::Relaxed);
        result
    });
    assert!(matches!(result, Err(StoreError::Cancelled)), "{result:?}");

    assert_eq!(ids(&store), [kept]);
    assert_same(&load(&store, kept, 0), &sample());
    assert!(load(&store, kept + 1, 0).is_empty());
    assert!(store.snapshot_folders(kept + 1).unwrap().is_empty());
    assert_eq!(store.snapshot_scope(kept + 1).unwrap(), Scope::default());
}

#[test]
fn a_reader_waits_while_a_snapshot_is_being_saved() {
    let tmp = TempDir::new("store");
    let mut writer = create(&tmp.db());
    let reader = Store::open(&tmp.db()).unwrap();
    let snapshot = Snapshot::of_tree(synthetic(300_000));
    let saving = AtomicBool::new(true);
    thread::scope(|s| {
        s.spawn(|| {
            writer.save(&snapshot, &AtomicBool::new(false)).unwrap();
            saving.store(false, Ordering::Relaxed);
        });
        wait_for_journal(&tmp, || saving.load(Ordering::Relaxed));
        // Reads during the write, its commit included, wait for it instead of failing
        // with "database is busy".
        let mut reads = 0;
        while saving.load(Ordering::Relaxed) {
            reader.snapshot_count().unwrap();
            reads += 1;
        }
        assert!(reads > 0);
    });
    assert_eq!(reader.snapshot_count().unwrap(), 1);
}

#[test]
fn deleted_snapshot_is_gone_but_its_space_stays_until_compaction() {
    let tmp = TempDir::new("store");
    let mut store = create(&tmp.db());
    let old = save(&mut store, &synthetic(20_000));
    let new = save(&mut store, &sample());
    let full = fs::metadata(tmp.db()).unwrap().len();

    store.delete(old).unwrap();
    assert!(load(&store, old, 0).is_empty());
    assert_eq!(ids(&store), [new]);
    assert_same(&load(&store, new, 0), &sample());
    // Freed pages stay in the file until VACUUM.
    assert_eq!(fs::metadata(tmp.db()).unwrap().len(), full);
}

#[test]
fn a_comment_can_be_set_changed_and_removed() {
    let tmp = TempDir::new("store");
    let mut store = create(&tmp.db());
    let id = save(&mut store, &sample());
    let other = save(&mut store, &sample());
    let comment = |store: &Store, id: i64| {
        store
            .list()
            .unwrap()
            .into_iter()
            .find(|s| s.id == id)
            .unwrap()
            .comment
    };

    store.set_comment(id, Some("before install")).unwrap();
    assert_eq!(comment(&store, id).as_deref(), Some("before install"));
    assert_eq!(comment(&store, other), None, "only that snapshot changes");
    store.set_comment(id, Some("after install")).unwrap();
    assert_eq!(comment(&store, id).as_deref(), Some("after install"));
    store.set_comment(id, None).unwrap();
    assert_eq!(comment(&store, id), None);
    // A snapshot that is gone is not an error: nothing is left to comment on.
    store.set_comment(other + 100, Some("x")).unwrap();
    assert_eq!(ids(&store), [id, other]);
}

#[test]
fn several_snapshots_are_deleted_in_one_go() {
    let tmp = TempDir::new("store");
    let mut store = create(&tmp.db());
    let first = save(&mut store, &sample());
    let second = save(&mut store, &synthetic(100));
    let third = save(&mut store, &sample());
    let fourth = save(&mut store, &sample());

    store.delete_many(&[first, third, fourth + 100]).unwrap();
    assert_eq!(ids(&store), [second, fourth]);
    assert!(load(&store, first, 0).is_empty());
    assert!(store.snapshot_folders(third).unwrap().is_empty());
    assert_same(&load(&store, second, 0), &synthetic(100));
    store.delete_many(&[]).unwrap();
    assert_eq!(ids(&store), [second, fourth]);
}

#[test]
fn compaction_returns_the_space_of_deleted_snapshots() {
    let tmp = TempDir::new("store");
    let mut store = create(&tmp.db());
    let old = save(&mut store, &synthetic(20_000));
    let new = save(&mut store, &sample());
    let full = fs::metadata(tmp.db()).unwrap().len();
    store.delete(old).unwrap();

    let compacted = store.compact().unwrap();
    assert_eq!(compacted.before, full);
    assert_eq!(compacted.after, fs::metadata(tmp.db()).unwrap().len());
    assert!(
        compacted.after * 10 < compacted.before,
        "{} of {} bytes left",
        compacted.after,
        compacted.before
    );
    assert_same(&load(&store, new, 0), &sample());
}

#[test]
fn an_interrupted_compaction_leaves_the_database_whole() {
    let tmp = TempDir::new("store");
    let mut store = create(&tmp.db());
    // A big snapshot stays, so the rebuild has ~100 MiB to copy and takes a while.
    let small = save(&mut store, &sample());
    let kept = save(&mut store, &synthetic(1_000_000));
    store.delete(small).unwrap();
    let full = fs::metadata(tmp.db()).unwrap().len();

    let interrupt = store.interrupt();
    // The store goes to the compacting thread, as it does in the window, and comes back.
    let (store, result) = thread::scope(|s| {
        let compaction = s.spawn(move || {
            let result = store.compact();
            (store, result)
        });
        // An interrupt before the rebuild starts is lost, so it is repeated until the
        // rebuild has answered it.
        while !compaction.is_finished() {
            interrupt.interrupt();
            thread::sleep(Duration::from_millis(10));
        }
        compaction.join().unwrap()
    });
    assert!(matches!(result, Err(StoreError::Cancelled)), "{result:?}");
    drop(store);

    // Nothing of the rebuild is left behind, and the data is as it was.
    let store = Store::open(&tmp.db()).unwrap();
    assert_eq!(ids(&store), [kept]);
    assert_eq!(load(&store, kept, 0).len(), 1_000_000);
    assert_eq!(fs::metadata(tmp.db()).unwrap().len(), full);
    let check: String = rusqlite::Connection::open(tmp.db())
        .unwrap()
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(check, "ok");
    // The interrupt is spent: the next compaction goes through.
    assert!(store.compact().unwrap().after < full);
}

#[test]
fn id_of_deleted_snapshot_is_never_reused() {
    let tmp = TempDir::new("store");
    let mut store = create(&tmp.db());
    let first = save(&mut store, &sample());
    store.delete(first).unwrap();
    assert_ne!(save(&mut store, &sample()), first);
}

/// Set for the child process of [`killed_save_leaves_no_partial_snapshot`].
const CHILD_DB_VAR: &str = "REVISOR_STORE_CHILD_DB";

/// Runs only as the child of [`killed_save_leaves_no_partial_snapshot`]: starts saving a
/// large snapshot and gets killed in the middle.
#[test]
#[ignore = "child process of killed_save_leaves_no_partial_snapshot"]
fn killed_save_child() {
    let Some(db) = std::env::var_os(CHILD_DB_VAR) else {
        return;
    };
    let snapshot = Snapshot::of_tree(synthetic(2_000_000));
    let mut store = Store::open(Path::new(&db)).unwrap();
    store.save(&snapshot, &AtomicBool::new(false)).unwrap();
}

#[test]
fn killed_save_leaves_no_partial_snapshot() {
    let tmp = TempDir::new("store");
    let kept = save(&mut create(&tmp.db()), &sample());

    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["store::tests::killed_save_child", "--exact", "--ignored"])
        .env(CHILD_DB_VAR, tmp.db())
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    wait_for_journal(&tmp, || child.try_wait().unwrap().is_none());
    child.kill().unwrap();
    assert!(!child.wait().unwrap().success());
    assert!(tmp.journal().exists(), "the save finished before the kill");

    let store = Store::open(&tmp.db()).unwrap();
    assert_eq!(ids(&store), [kept]);
    assert!(load(&store, kept + 1, 0).is_empty());
    assert_same(&load(&store, kept, 0), &sample());
}
