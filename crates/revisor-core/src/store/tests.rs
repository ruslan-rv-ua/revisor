use super::*;
use crate::job::Job;
use crate::scan::synthetic;
use crate::testing::{TempDir, cancelled, poll_until_ended};
use std::fs;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
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
        .save(&Snapshot::of_tree(entries.to_vec()), &Cancel::new())
        .unwrap()
}

/// Loads one tree without cancellation.
fn load(store: &Store, id: i64, position: usize) -> Vec<Entry> {
    store.load_folder(id, position, &Cancel::new()).unwrap()
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
        .save(&snapshot_of_settings(None), &Cancel::new())
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
    let id = store.save(&snapshot, &Cancel::new()).unwrap();
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
    let id = store.save(&snapshot, &Cancel::new()).unwrap();
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
            &Cancel::new(),
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
    let cancelled = store.load_folder(id, 0, &cancelled());
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
    let cancel = Cancel::new();
    let saving = AtomicBool::new(true);
    let result = thread::scope(|s| {
        s.spawn(|| {
            // Let the save write a good part of its entries before cancelling.
            wait_for_journal(&tmp, || saving.load(Ordering::Relaxed));
            thread::sleep(Duration::from_millis(50));
            cancel.cancel();
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
            writer.save(&snapshot, &Cancel::new()).unwrap();
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

    let compacted = store.compact(&Cancel::new()).unwrap();
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

    let probe = Store::open_with_timeout(&tmp.db(), Duration::ZERO).unwrap();
    let cancel = Cancel::new();
    // The store goes to the compacting thread, as it does in a job, and comes back.
    let (store, result) = thread::scope(|s| {
        let cancel = &cancel;
        let compaction = s.spawn(move || {
            let result = store.compact(cancel);
            (store, result)
        });
        // Cancelled once the rebuild runs, so that it is the rebuild that is interrupted:
        // a token cancelled before would keep it from starting at all.
        wait_for_rebuild(&probe, || !compaction.is_finished());
        while !compaction.is_finished() {
            cancel.cancel();
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
    assert_whole(&tmp, full);
    // The interrupt is spent: the next compaction goes through.
    assert!(store.compact(&Cancel::new()).unwrap().after < full);
}

/// Waits until the database `probe` is open on is closed to it, as it is while a
/// compaction rebuilds it; `still_running` says whether the compaction may yet do so.
fn wait_for_rebuild(probe: &Store, mut still_running: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(60);
    while !probe.snapshot_count().is_err_and(|e| e.is_busy()) {
        assert!(Instant::now() < deadline, "the rebuild never started");
        assert!(
            still_running(),
            "the compaction ended before it was cancelled"
        );
        thread::sleep(Duration::from_millis(1));
    }
}

/// Checks that the database in `tmp` is whole and as large as `size`: a compaction left
/// nothing of its rebuild behind.
fn assert_whole(tmp: &TempDir, size: u64) {
    assert_eq!(fs::metadata(tmp.db()).unwrap().len(), size);
    let check: String = rusqlite::Connection::open(tmp.db())
        .unwrap()
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(check, "ok");
}

#[test]
fn a_compaction_cancelled_before_it_starts_leaves_the_database_as_it_was() {
    let tmp = TempDir::new("store");
    let mut store = create(&tmp.db());
    let old = save(&mut store, &synthetic(20_000));
    let new = save(&mut store, &sample());
    store.delete(old).unwrap();
    let full = fs::metadata(tmp.db()).unwrap().len();

    let result = store.compact(&cancelled());

    assert!(matches!(result, Err(StoreError::Cancelled)), "{result:?}");
    assert_whole(&tmp, full);
    assert_same(&load(&store, new, 0), &sample());
}

#[test]
fn a_compaction_job_gives_the_space_back_once_polled() {
    let tmp = TempDir::new("store");
    let mut store = create(&tmp.db());
    let old = save(&mut store, &synthetic(20_000));
    let new = save(&mut store, &sample());
    store.delete(old).unwrap();
    let full = fs::metadata(tmp.db()).unwrap().len();

    let mut job = Job::start(Compaction::new(store));
    let outcome = poll_until_ended(&mut job);

    let Ok(Outcome::Done(compacted)) = outcome else {
        panic!("{outcome:?}");
    };
    assert_eq!(compacted.before, full);
    assert!(compacted.after * 10 < compacted.before, "{compacted:?}");
    let store = Store::open(&tmp.db()).unwrap();
    assert_eq!(ids(&store), [new]);
    assert_same(&load(&store, new, 0), &sample());
}

#[test]
fn a_compaction_job_cancelled_while_it_rebuilds_leaves_the_database_whole() {
    let tmp = TempDir::new("store");
    let mut store = create(&tmp.db());
    let small = save(&mut store, &sample());
    let kept = save(&mut store, &synthetic(200_000));
    store.delete(small).unwrap();
    let full = fs::metadata(tmp.db()).unwrap().len();
    let probe = Store::open_with_timeout(&tmp.db(), Duration::ZERO).unwrap();

    let job = Job::start(Compaction::new(store));
    wait_for_rebuild(&probe, || true);
    job.cancel();
    let outcome = job.wait();

    assert!(matches!(outcome, Ok(Outcome::Cancelled)), "{outcome:?}");
    assert_whole(&tmp, full);
    let store = Store::open(&tmp.db()).unwrap();
    assert_eq!(ids(&store), [kept]);
    assert_eq!(load(&store, kept, 0).len(), 200_000);
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
    store.save(&snapshot, &Cancel::new()).unwrap();
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

#[test]
fn the_entries_of_a_snapshot_are_counted_over_its_folders() {
    let tmp = TempDir::new("store");
    let mut store = create(&tmp.db());
    let id = save(&mut store, &synthetic(1_000));
    assert_eq!(store.snapshot_entries(id).unwrap(), 1_000);
    // A folder that was not scanned holds nothing.
    let missing = store
        .save(
            &Snapshot {
                taken_at: 1,
                comment: None,
                folders: vec![FolderSnapshot {
                    path: r"C:\Gone".into(),
                    state: FolderState::Missing,
                    entries: Vec::new(),
                }],
                exclusions: Vec::new(),
            },
            &Cancel::new(),
        )
        .unwrap();
    assert_eq!(store.snapshot_entries(missing).unwrap(), 0);
    assert_eq!(store.snapshot_entries(missing + 100).unwrap(), 0);
}

#[test]
fn loading_counts_the_entries_as_they_are_read() {
    let tmp = TempDir::new("store");
    let mut store = create(&tmp.db());
    let id = save(&mut store, &synthetic(10_000));
    let loaded = AtomicU64::new(0);
    let entries = store
        .load_folder_counting(id, 0, &Cancel::new(), &loaded)
        .unwrap();
    assert_eq!(entries.len(), 10_000);
    assert_eq!(loaded.load(Ordering::Relaxed), 10_000);
}

/// An exclusive lock on the database in `tmp`, held until the connection is dropped: no
/// other connection can read the database meanwhile.
fn lock_exclusively(tmp: &TempDir) -> rusqlite::Connection {
    let conn = rusqlite::Connection::open(tmp.db()).unwrap();
    conn.execute_batch("BEGIN EXCLUSIVE").unwrap();
    conn
}

#[test]
fn an_access_to_a_free_database_is_made_once_without_waiting() {
    let tmp = TempDir::new("store");
    let store = create(&tmp.db());
    let mut waits = Vec::new();
    let mut tries = 0;
    let count = retry_while_busy(
        &Cancel::new(),
        |w| waits.push(w),
        || {
            tries += 1;
            store.snapshot_count()
        },
    )
    .unwrap();
    assert_eq!((count, tries), (0, 1));
    assert!(waits.is_empty());
}

#[test]
fn an_access_waits_for_a_busy_database_and_goes_through_once_it_is_free() {
    let tmp = TempDir::new("store");
    drop(create(&tmp.db()));
    let lock = lock_exclusively(&tmp);
    let unlocking = thread::spawn(move || {
        thread::sleep(Duration::from_millis(300));
        drop(lock);
    });
    let mut waits = Vec::new();
    let waiting = Instant::now();
    let store = retry_while_busy(
        &Cancel::new(),
        |w| waits.push(w),
        || Store::open_with_timeout(&tmp.db(), Duration::ZERO),
    )
    .unwrap();
    assert!(
        waiting.elapsed() >= Duration::from_millis(150),
        "{:?}",
        waiting.elapsed()
    );
    assert_eq!(waits, [true, false]);
    assert_eq!(store.snapshot_count().unwrap(), 0);
    unlocking.join().unwrap();
}

#[test]
fn a_cancel_ends_the_wait_for_a_busy_database() {
    let tmp = TempDir::new("store");
    drop(create(&tmp.db()));
    let _lock = lock_exclusively(&tmp);
    let cancel = Arc::new(Cancel::new());
    let cancelling = {
        let cancel = Arc::clone(&cancel);
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(200));
            cancel.cancel();
        })
    };
    let mut waits = Vec::new();
    let result = retry_while_busy(
        &cancel,
        |w| waits.push(w),
        || Store::open_with_timeout(&tmp.db(), Duration::ZERO),
    );
    assert!(
        matches!(result, Err(StoreError::Cancelled)),
        "the wait was not cancelled"
    );
    assert_eq!(waits, [true]);
    cancelling.join().unwrap();
}

#[test]
fn a_cancel_during_the_pause_ends_the_wait_before_the_next_try() {
    let tmp = TempDir::new("store");
    drop(create(&tmp.db()));
    let _lock = lock_exclusively(&tmp);
    let cancel = Arc::new(Cancel::new());
    let cancelling = {
        let cancel = Arc::clone(&cancel);
        // In the middle of the first pause.
        thread::spawn(move || {
            thread::sleep(BUSY_RETRY / 2);
            cancel.cancel();
        })
    };
    let mut tries = 0;
    let result = retry_while_busy(
        &cancel,
        |_| {},
        || {
            tries += 1;
            Store::open_with_timeout(&tmp.db(), Duration::ZERO)
        },
    );
    assert!(matches!(result, Err(StoreError::Cancelled)));
    assert_eq!(tries, 1, "a cancelled wait tried again");
    cancelling.join().unwrap();
}

/// Polls `job` until its comment waits for the database.
fn wait_until_waiting(job: &Job<CommentWork>) {
    let deadline = Instant::now() + Duration::from_secs(60);
    while !job.progress().is_waiting() {
        assert!(job.is_running(), "the comment ended without waiting");
        assert!(Instant::now() < deadline, "the comment never waited");
        thread::sleep(Duration::from_millis(5));
    }
}

fn comment_of(db: &Path, id: i64) -> Option<String> {
    let store = Store::open(db).unwrap();
    let snapshot = store.snapshot(id).unwrap().unwrap();
    snapshot.comment
}

#[test]
fn a_comment_on_a_free_database_is_written_at_once() {
    let tmp = TempDir::new("store");
    let mut store = create(&tmp.db());
    let id = save(&mut store, &synthetic(10));
    drop(store);
    write_comment(&tmp.db(), id, Some("Перед оновленням")).unwrap();
    assert_eq!(
        comment_of(&tmp.db(), id).as_deref(),
        Some("Перед оновленням")
    );
    write_comment(&tmp.db(), id, None).unwrap();
    assert_eq!(comment_of(&tmp.db(), id), None);
}

#[test]
fn a_comment_on_a_busy_database_fails_at_once_as_busy() {
    let tmp = TempDir::new("store");
    let mut store = create(&tmp.db());
    let id = save(&mut store, &synthetic(10));
    drop(store);
    let _lock = lock_exclusively(&tmp);
    let trying = Instant::now();
    let error = write_comment(&tmp.db(), id, Some("x")).unwrap_err();
    assert!(error.is_busy(), "{error:?}");
    assert!(trying.elapsed() < Duration::from_secs(1), "SQLite waited");
}

#[test]
fn a_comment_waits_for_a_busy_database_and_is_written_once_it_is_free() {
    let tmp = TempDir::new("store");
    let mut store = create(&tmp.db());
    let id = save(&mut store, &synthetic(10));
    drop(store);
    let lock = lock_exclusively(&tmp);
    let mut job = Job::start(CommentWork::new(&tmp.db(), id, Some("Після".into())));
    wait_until_waiting(&job);
    // Held for longer than SQLite's own wait would be, the database is still waited for.
    thread::sleep(BUSY_RETRY * 3);
    assert!(job.poll().is_none(), "the comment did not wait");
    drop(lock);
    assert!(matches!(poll_until_ended(&mut job), Ok(Outcome::Done(()))));
    assert!(!job.progress().is_waiting());
    assert_eq!(comment_of(&tmp.db(), id).as_deref(), Some("Після"));
}

#[test]
fn a_comment_cancelled_while_it_waits_is_not_written() {
    let tmp = TempDir::new("store");
    let mut store = create(&tmp.db());
    let id = save(&mut store, &synthetic(10));
    store.set_comment(id, Some("Раніший")).unwrap();
    drop(store);
    let lock = lock_exclusively(&tmp);
    let mut job = Job::start(CommentWork::new(&tmp.db(), id, Some("Новий".into())));
    wait_until_waiting(&job);
    job.cancel();
    assert!(matches!(poll_until_ended(&mut job), Ok(Outcome::Cancelled)));
    drop(lock);
    assert_eq!(comment_of(&tmp.db(), id).as_deref(), Some("Раніший"));
}
