use super::*;
use crate::job::{GRACE, Job};
use crate::scan::{EntryKind, VersionInfo};
use crate::store::{ProfileSettings, Scope};
use crate::testing::{TempDir, cancelled, poll_until_ended};
use std::path::PathBuf;
use std::process::Command;
use std::thread;

/// A profile database in `tmp` with these folders and exclusions.
fn profile(tmp: &TempDir, folders: &[PathBuf], exclusions: &[&str]) -> Store {
    Store::create(
        &tmp.db(),
        &ProfileSettings {
            name: "Тест".into(),
            scope: Scope {
                folders: folders.to_vec(),
                exclusions: exclusions.iter().map(|p| p.to_string()).collect(),
            },
        },
    )
    .unwrap()
}

/// A drive letter no volume has, as a root: `Z:\`, or lower when Z: is taken.
fn missing_drive() -> PathBuf {
    // SAFETY: no arguments.
    let used = unsafe { windows_sys::Win32::Storage::FileSystem::GetLogicalDrives() };
    let letter = (b'A'..=b'Z')
        .rev()
        .find(|l| used & (1 << (l - b'A')) == 0)
        .expect("a free drive letter") as char;
    PathBuf::from(format!(r"{letter}:\"))
}

fn take_it(tmp: &TempDir) -> (Report, Progress) {
    let progress = Progress::default();
    let report = take(&tmp.db(), Some("test"), &progress, &Cancel::new()).unwrap();
    (report, progress)
}

#[test]
fn a_snapshot_covers_every_folder_of_the_profile_and_reports_each() {
    let tmp = TempDir::new("snapshot");
    let app = tmp.path().join("app");
    tmp.write(r"app\a.txt", b"aa");
    tmp.write(r"app\skip.tmp", b"");
    tmp.write(r"app\sub\b.txt", b"b");
    let docs = tmp.path().join("docs");
    tmp.write(r"docs\c.txt", b"c");
    let gone = tmp.path().join("gone");
    let nowhere = missing_drive().join("nowhere");
    let before = store::now_millis();
    drop(profile(
        &tmp,
        &[app.clone(), gone.clone(), nowhere.clone(), docs.clone()],
        &["*.tmp", "%TEMP%", r"%RevisorNoSuchVariable%\x"],
    ));

    let (report, progress) = take_it(&tmp);

    assert_eq!(report.entries, 4 + 2);
    assert_eq!((report.inaccessible, report.denied), (0, 0));
    let states: Vec<&FolderState> = report.folders.iter().map(|f| &f.state).collect();
    // The folder's real location is kept, so path exclusions apply from it in a comparison.
    assert!(matches!(
        states[0],
        FolderState::Scanned { volume_serial, real_path }
            if *volume_serial != 0 && *real_path == folders::real_path(&app).unwrap()
    ));
    assert_eq!(states[1], &FolderState::Missing);
    assert!(
        matches!(states[2], FolderState::Unavailable(Unavailable::Error(_))),
        "{:?}",
        states[2]
    );
    // The two scanned folders are on the same volume, each at its own real location.
    assert!(matches!(
        (states[0], states[3]),
        (
            FolderState::Scanned { volume_serial: a, real_path: app_real },
            FolderState::Scanned { volume_serial: b, real_path: docs_real },
        ) if a == b && app_real != docs_real && *docs_real == folders::real_path(&docs).unwrap()
    ));
    let not_found: Vec<&Path> = report.not_found().map(|f| f.path.as_path()).collect();
    assert_eq!(not_found, [gone.as_path(), nowhere.as_path()]);
    assert_eq!(progress.entries(), report.entries);

    let store = Store::open(&tmp.db()).unwrap();
    let saved = store.list().unwrap();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].id, report.id);
    assert_eq!(saved[0].comment.as_deref(), Some("test"));
    assert!((before..=store::now_millis()).contains(&saved[0].taken_at));
    let folders = store.snapshot_folders(report.id).unwrap();
    assert_eq!(folders, report.folders);
    assert_eq!(folders[0].path, app);
    let entries = store.load_folder(report.id, 0, &Cancel::new()).unwrap();
    let mut names: Vec<String> = entries[1..]
        .iter()
        .map(|e| e.name.to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["a.txt", "b.txt", "sub"]);
    assert_eq!(entries[0].kind, EntryKind::Dir);
    assert_eq!(entries[0].version, VersionInfo::NotRead);
    assert_eq!(
        store
            .load_folder(report.id, 3, &Cancel::new())
            .unwrap()
            .len(),
        2
    );
    for position in [1, 2] {
        assert!(
            store
                .load_folder(report.id, position, &Cancel::new())
                .unwrap()
                .is_empty()
        );
    }

    let exclusions = store.snapshot_exclusions(report.id).unwrap();
    assert_eq!(exclusions[0].applied.as_deref(), Some("*.tmp"));
    let temp = exclusions[1].applied.as_deref().expect("TEMP is set");
    assert!(temp.contains(':'), "{temp} is expanded to a path");
    assert_eq!(exclusions[2].applied, None);
    assert_eq!(
        store.snapshot_scope(report.id).unwrap().exclusions,
        ["*.tmp", "%TEMP%", r"%RevisorNoSuchVariable%\x"]
    );
}

#[test]
fn a_cancelled_snapshot_is_not_saved() {
    let tmp = TempDir::new("snapshot");
    tmp.write(r"app\a.txt", b"a");
    drop(profile(&tmp, &[tmp.path().join("app")], &[]));

    let result = take(&tmp.db(), None, &Progress::default(), &cancelled());

    assert!(
        matches!(result, Err(SnapshotError::Cancelled)),
        "{result:?}"
    );
    assert!(Store::open(&tmp.db()).unwrap().list().unwrap().is_empty());
}

#[test]
fn a_snapshot_that_cannot_be_saved_names_the_cause_and_spares_the_others() {
    let tmp = TempDir::new("snapshot");
    tmp.write(r"app\a.txt", b"a");
    let mut store = profile(&tmp, &[tmp.path().join("app")], &[]);
    let kept = store
        .save(
            &Snapshot::of_tree(crate::scan::synthetic(10)),
            &Cancel::new(),
        )
        .unwrap();
    drop(store);
    let mut perms = fs::metadata(tmp.db()).unwrap().permissions();
    perms.set_readonly(true);
    fs::set_permissions(tmp.db(), perms).unwrap();

    let result = take(&tmp.db(), None, &Progress::default(), &Cancel::new());

    let mut perms = fs::metadata(tmp.db()).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    perms.set_readonly(false);
    fs::set_permissions(tmp.db(), perms).unwrap();
    assert!(matches!(result, Err(SnapshotError::Store(_))), "{result:?}");
    let store = Store::open(&tmp.db()).unwrap();
    let ids: Vec<i64> = store.list().unwrap().iter().map(|s| s.id).collect();
    assert_eq!(ids, [kept]);
    assert_eq!(
        store.load_folder(kept, 0, &Cancel::new()).unwrap().len(),
        10
    );
}

#[test]
fn a_folder_closed_by_rights_counts_as_denied() {
    let tmp = TempDir::new("snapshot");
    let locked = tmp.path().join("locked");
    tmp.write(r"locked\hidden.txt", b"x");
    tmp.write(r"open\seen.txt", b"x");
    drop(profile(
        &tmp,
        &[locked.clone(), tmp.path().join("open")],
        &[],
    ));
    // Deny "list folder" to Everyone; the owner can still change the ACL back.
    let icacls = |args: &[&str]| {
        let status = Command::new("icacls")
            .arg(&locked)
            .args(args)
            .output()
            .unwrap()
            .status;
        assert!(status.success());
    };
    icacls(&["/deny", "*S-1-1-0:(RD)"]);

    let result = take(&tmp.db(), None, &Progress::default(), &Cancel::new());
    icacls(&["/remove:d", "*S-1-1-0"]);

    let report = result.unwrap();
    // The folder is in the snapshot as its own inaccessible entry, like a closed folder
    // deeper down: its contents are unknown, not absent.
    assert_eq!((report.denied, report.inaccessible), (1, 1));
    assert!(matches!(
        report.folders[0].state,
        FolderState::Scanned { .. }
    ));
    assert!(matches!(
        report.folders[1].state,
        FolderState::Scanned { .. }
    ));
    assert_eq!(report.entries, 1 + 2);
    let store = Store::open(&tmp.db()).unwrap();
    let tree = store.load_folder(report.id, 0, &Cancel::new()).unwrap();
    assert_eq!(tree.len(), 1);
    assert!(tree[0].inaccessible);
    assert_eq!(report.not_found().count(), 0);
}

#[test]
fn a_file_at_a_folders_path_is_not_a_folder() {
    let tmp = TempDir::new("snapshot");
    let file = tmp.write("was-a-folder", b"now a file");
    tmp.write(r"app\a.txt", b"a");
    drop(profile(&tmp, &[file.clone(), tmp.path().join("app")], &[]));

    let (report, _) = take_it(&tmp);

    assert_eq!(
        report.folders[0].state,
        FolderState::Unavailable(Unavailable::NotAFolder)
    );
    assert_eq!(report.not_found().count(), 1);
    assert_eq!((report.entries, report.denied), (2, 0));
    assert_eq!(
        Store::open(&tmp.db())
            .unwrap()
            .snapshot_folders(report.id)
            .unwrap()[0]
            .state,
        FolderState::Unavailable(Unavailable::NotAFolder)
    );
}

/// A write lock on the database of the profile in `tmp`, held until dropped: a snapshot
/// being saved meanwhile waits for it in [`Phase::Saving`].
struct WriteLock(rusqlite::Connection);

impl WriteLock {
    fn take(tmp: &TempDir) -> Self {
        let conn = rusqlite::Connection::open(tmp.db()).unwrap();
        conn.execute_batch("BEGIN IMMEDIATE").unwrap();
        WriteLock(conn)
    }
}

impl Drop for WriteLock {
    fn drop(&mut self) {
        self.0.execute_batch("ROLLBACK").unwrap();
    }
}

/// Waits until the snapshot of `job` is in `phase`.
fn wait_for_phase(job: &Job<SnapshotWork>, phase: Phase) {
    let deadline = Instant::now() + Duration::from_secs(60);
    while job.progress().phase() != phase {
        assert!(
            Instant::now() < deadline,
            "the snapshot never got to {phase:?}"
        );
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn a_snapshot_job_saves_its_snapshot_and_reports_it_once_polled() {
    let tmp = TempDir::new("snapshot");
    tmp.write(r"app\a.txt", b"a");
    tmp.write(r"app\sub\b.txt", b"b");
    drop(profile(&tmp, &[tmp.path().join("app")], &[]));

    let mut job = Job::start(SnapshotWork::new(&tmp.db(), Some("job")));
    let outcome = poll_until_ended(&mut job);

    let Ok(Outcome::Done(report)) = outcome else {
        panic!("{outcome:?}");
    };
    assert_eq!(report.entries, 4);
    assert_eq!(job.progress().entries(), report.entries);
    assert_eq!(job.progress().phase(), Phase::Saved);
    let saved = Store::open(&tmp.db()).unwrap().list().unwrap();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].id, report.id);
    assert_eq!(saved[0].comment.as_deref(), Some("job"));
}

#[test]
fn a_snapshot_scans_saves_and_ends_a_moment_after_its_save() {
    let tmp = TempDir::new("snapshot");
    tmp.write(r"app\a.txt", b"a");
    drop(profile(&tmp, &[tmp.path().join("app")], &[]));
    assert_eq!(Progress::default().phase(), Phase::Scanning);

    let lock = WriteLock::take(&tmp);
    let mut job = Job::start(SnapshotWork::new(&tmp.db(), None));
    // The scan is over, and the save waits for the database.
    wait_for_phase(&job, Phase::Saving);
    assert_eq!(job.progress().entries(), 2);
    assert!(!SnapshotWork::about_to_end(job.progress()));
    assert!(job.poll().is_none());
    drop(lock);

    // Once the save is over, the result is a moment away, and the poll that finds the
    // save over waits for it (TASK-21).
    wait_for_phase(&job, Phase::Saved);
    assert!(SnapshotWork::about_to_end(job.progress()));
    let polling = Instant::now();
    let polled = job.poll();
    assert!(polling.elapsed() < GRACE);
    assert!(matches!(polled, Some(Ok(Outcome::Done(_)))), "{polled:?}");
}

#[test]
fn a_snapshot_job_cancelled_while_saving_saves_nothing() {
    let tmp = TempDir::new("snapshot");
    tmp.write(r"app\a.txt", b"a");
    drop(profile(&tmp, &[tmp.path().join("app")], &[]));

    let lock = WriteLock::take(&tmp);
    let job = Job::start(SnapshotWork::new(&tmp.db(), None));
    wait_for_phase(&job, Phase::Saving);
    job.cancel();
    drop(lock);
    let outcome = job.wait();

    assert!(matches!(outcome, Ok(Outcome::Cancelled)), "{outcome:?}");
    assert!(Store::open(&tmp.db()).unwrap().list().unwrap().is_empty());
}
