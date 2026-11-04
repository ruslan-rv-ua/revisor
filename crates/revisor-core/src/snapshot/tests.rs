use super::*;
use crate::scan::{EntryKind, VersionInfo};
use crate::store::{ProfileSettings, Scope};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::AtomicUsize;

/// A fresh folder under the system temp folder, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("revisor-snapshot-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn db(&self) -> PathBuf {
        self.0.join("profile.db")
    }

    /// Writes a file under this folder, making its folders.
    fn write(&self, rel: &str, content: &[u8]) -> PathBuf {
        let path = self.0.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, content).unwrap();
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

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
    let report = take(&tmp.db(), Some("test"), &progress, &AtomicBool::new(false)).unwrap();
    (report, progress)
}

#[test]
fn a_snapshot_covers_every_folder_of_the_profile_and_reports_each() {
    let tmp = TempDir::new();
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
    assert!(progress.saving());
    assert!(progress.saved());

    let store = Store::open(&tmp.db()).unwrap();
    let saved = store.list().unwrap();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].id, report.id);
    assert_eq!(saved[0].comment.as_deref(), Some("test"));
    assert!((before..=store::now_millis()).contains(&saved[0].taken_at));
    let folders = store.snapshot_folders(report.id).unwrap();
    assert_eq!(folders, report.folders);
    assert_eq!(folders[0].path, app);
    let entries = store
        .load_folder(report.id, 0, &AtomicBool::new(false))
        .unwrap();
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
            .load_folder(report.id, 3, &AtomicBool::new(false))
            .unwrap()
            .len(),
        2
    );
    for position in [1, 2] {
        assert!(
            store
                .load_folder(report.id, position, &AtomicBool::new(false))
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
    let tmp = TempDir::new();
    tmp.write(r"app\a.txt", b"a");
    drop(profile(&tmp, &[tmp.path().join("app")], &[]));

    let result = take(
        &tmp.db(),
        None,
        &Progress::default(),
        &AtomicBool::new(true),
    );

    assert!(
        matches!(result, Err(SnapshotError::Cancelled)),
        "{result:?}"
    );
    assert!(Store::open(&tmp.db()).unwrap().list().unwrap().is_empty());
}

#[test]
fn a_snapshot_that_cannot_be_saved_names_the_cause_and_spares_the_others() {
    let tmp = TempDir::new();
    tmp.write(r"app\a.txt", b"a");
    let mut store = profile(&tmp, &[tmp.path().join("app")], &[]);
    let kept = store
        .save(
            &Snapshot::of_tree(crate::scan::synthetic(10)),
            &AtomicBool::new(false),
        )
        .unwrap();
    drop(store);
    let mut perms = fs::metadata(tmp.db()).unwrap().permissions();
    perms.set_readonly(true);
    fs::set_permissions(tmp.db(), perms).unwrap();

    let result = take(
        &tmp.db(),
        None,
        &Progress::default(),
        &AtomicBool::new(false),
    );

    let mut perms = fs::metadata(tmp.db()).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    perms.set_readonly(false);
    fs::set_permissions(tmp.db(), perms).unwrap();
    assert!(matches!(result, Err(SnapshotError::Store(_))), "{result:?}");
    let store = Store::open(&tmp.db()).unwrap();
    let ids: Vec<i64> = store.list().unwrap().iter().map(|s| s.id).collect();
    assert_eq!(ids, [kept]);
    assert_eq!(
        store
            .load_folder(kept, 0, &AtomicBool::new(false))
            .unwrap()
            .len(),
        10
    );
}

#[test]
fn a_folder_closed_by_rights_counts_as_denied() {
    let tmp = TempDir::new();
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

    let result = take(
        &tmp.db(),
        None,
        &Progress::default(),
        &AtomicBool::new(false),
    );
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
    let tree = store
        .load_folder(report.id, 0, &AtomicBool::new(false))
        .unwrap();
    assert_eq!(tree.len(), 1);
    assert!(tree[0].inaccessible);
    assert_eq!(report.not_found().count(), 0);
}

#[test]
fn a_file_at_a_folders_path_is_not_a_folder() {
    let tmp = TempDir::new();
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
