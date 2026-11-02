use super::*;
use crate::scan::synthetic;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicUsize;
use std::thread;
use std::time::{Duration, Instant};

const FILE_ATTRIBUTE_ARCHIVE: u32 = 0x20;
const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;

/// A fresh folder under the system temp folder, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("revisor-store-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn db(&self) -> PathBuf {
        self.0.join("profile.db")
    }

    /// The rollback journal; it exists while a write transaction is open.
    fn journal(&self) -> PathBuf {
        self.0.join("profile.db-journal")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

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

/// Saves with the profile's scope, without a comment and without cancellation.
fn save(store: &mut Store, entries: &[Entry]) -> i64 {
    let scope = store.settings().unwrap().scope;
    store
        .save(&scope, None, entries, &AtomicBool::new(false))
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
    let tmp = TempDir::new();
    create(&tmp.db());
    assert_eq!(user_version(&tmp.db()), 1);
}

#[test]
fn database_from_newer_revisor_is_refused() {
    let tmp = TempDir::new();
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
    let tmp = TempDir::new();
    drop(create(&tmp.db()));
    assert_eq!(
        Store::open(&tmp.db()).unwrap().settings().unwrap(),
        settings()
    );
}

#[test]
fn changed_settings_replace_the_old_ones() {
    let tmp = TempDir::new();
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

#[test]
fn a_snapshot_keeps_the_folders_and_exclusions_it_was_taken_with() {
    let tmp = TempDir::new();
    let mut store = create(&tmp.db());
    let before = save(&mut store, &sample());
    let mut changed = settings();
    changed.scope.folders.remove(0);
    changed.scope.exclusions.push("Code Cache".into());
    store.set_settings(&changed).unwrap();
    let after = save(&mut store, &sample());

    assert_eq!(store.snapshot_scope(before).unwrap(), settings().scope);
    assert_eq!(store.snapshot_scope(after).unwrap(), changed.scope);
    store.delete(before).unwrap();
    assert_eq!(store.snapshot_scope(before).unwrap(), Scope::default());
}

#[test]
fn only_an_existing_profile_database_opens() {
    let tmp = TempDir::new();
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
    let tmp = TempDir::new();
    fs::write(tmp.db(), b"some text, not a database at all").unwrap();
    assert!(matches!(
        Store::open(&tmp.db()),
        Err(StoreError::NotAProfile)
    ));
}

#[test]
fn a_profile_database_without_its_profile_row_is_not_a_profile() {
    let tmp = TempDir::new();
    drop(create(&tmp.db()));
    let conn = rusqlite::Connection::open(tmp.db()).unwrap();
    conn.execute("DELETE FROM profile", []).unwrap();
    drop(conn);
    let store = Store::open(&tmp.db()).unwrap();
    assert!(matches!(store.settings(), Err(StoreError::NotAProfile)));
}

#[test]
fn creating_a_profile_never_takes_over_an_existing_file() {
    let tmp = TempDir::new();
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
        version: None,
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
    dll.version = Some([65535, 0, 1, 65535]);

    let mut locked = entry(Some(0), "locked".into(), EntryKind::Dir);
    locked.inaccessible = true;
    locked.attributes = FILE_ATTRIBUTE_DIRECTORY;

    let mut junction = entry(Some(0), "junction".into(), EntryKind::Link);
    junction.link_target = Some(OsStr::new(r"C:\Elsewhere").into());

    let mut unreadable_link = entry(Some(0), "broken".into(), EntryKind::Link);
    unreadable_link.inaccessible = true;

    // Names on NTFS may hold unpaired surrogates, which are not valid Unicode.
    let odd = OsString::from_wide(&[0x61, 0xD800, 0x62]);
    let mut odd_link = entry(Some(2), odd.clone(), EntryKind::Link);
    odd_link.link_target = Some(odd.into_boxed_os_str());

    vec![root, dll, locked, junction, unreadable_link, odd_link]
}

fn assert_same(loaded: &[Entry], expected: &[Entry]) {
    assert_eq!(loaded.len(), expected.len());
    for (i, (a, b)) in loaded.iter().zip(expected).enumerate() {
        assert_eq!(format!("{a:?}"), format!("{b:?}"), "entry {i}");
    }
}

#[test]
fn saved_snapshot_loads_back_unchanged() {
    let tmp = TempDir::new();
    let mut store = create(&tmp.db());
    let id = store
        .save(
            &settings().scope,
            Some("before install"),
            &sample(),
            &AtomicBool::new(false),
        )
        .unwrap();
    drop(store);

    let store = Store::open(&tmp.db()).unwrap();
    assert_same(&store.load(id).unwrap(), &sample());
    let list = store.list().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, id);
    assert_eq!(list[0].comment.as_deref(), Some("before install"));
}

#[test]
fn snapshots_are_kept_apart() {
    let tmp = TempDir::new();
    let mut store = create(&tmp.db());
    let small = save(&mut store, &sample());
    let big = save(&mut store, &synthetic(5000));

    assert_same(&store.load(small).unwrap(), &sample());
    assert_same(&store.load(big).unwrap(), &synthetic(5000));
    assert_eq!(ids(&store), [small, big]);
    assert_eq!(store.list().unwrap()[0].comment, None);
}

#[test]
fn cancelled_save_leaves_nothing_behind() {
    let tmp = TempDir::new();
    let mut store = create(&tmp.db());
    let kept = save(&mut store, &sample());

    let entries = synthetic(1_000_000);
    let cancel = AtomicBool::new(false);
    let saving = AtomicBool::new(true);
    let result = thread::scope(|s| {
        s.spawn(|| {
            // Let the save write a good part of its entries before cancelling.
            wait_for_journal(&tmp, || saving.load(Ordering::Relaxed));
            thread::sleep(Duration::from_millis(50));
            cancel.store(true, Ordering::Relaxed);
        });
        let result = store.save(&settings().scope, Some("cancelled"), &entries, &cancel);
        saving.store(false, Ordering::Relaxed);
        result
    });
    assert!(matches!(result, Err(StoreError::Cancelled)), "{result:?}");

    assert_eq!(ids(&store), [kept]);
    assert_same(&store.load(kept).unwrap(), &sample());
    assert!(store.load(kept + 1).unwrap().is_empty());
    assert_eq!(store.snapshot_scope(kept + 1).unwrap(), Scope::default());
}

#[test]
fn deleted_snapshot_is_gone_and_vacuum_returns_its_space() {
    let tmp = TempDir::new();
    let mut store = create(&tmp.db());
    let old = save(&mut store, &synthetic(20_000));
    let new = save(&mut store, &sample());
    let full = fs::metadata(tmp.db()).unwrap().len();

    store.delete(old).unwrap();
    assert!(store.load(old).unwrap().is_empty());
    assert_eq!(ids(&store), [new]);
    assert_same(&store.load(new).unwrap(), &sample());
    // Freed pages stay in the file until VACUUM.
    assert_eq!(fs::metadata(tmp.db()).unwrap().len(), full);

    store.vacuum().unwrap();
    let vacuumed = fs::metadata(tmp.db()).unwrap().len();
    assert!(vacuumed * 10 < full, "{vacuumed} of {full} bytes left");
    assert_same(&store.load(new).unwrap(), &sample());
}

#[test]
fn id_of_deleted_snapshot_is_never_reused() {
    let tmp = TempDir::new();
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
    let entries = synthetic(2_000_000);
    let mut store = Store::open(Path::new(&db)).unwrap();
    store
        .save(
            &settings().scope,
            Some("partial"),
            &entries,
            &AtomicBool::new(false),
        )
        .unwrap();
}

#[test]
fn killed_save_leaves_no_partial_snapshot() {
    let tmp = TempDir::new();
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
    assert!(store.load(kept + 1).unwrap().is_empty());
    assert_same(&store.load(kept).unwrap(), &sample());
}
