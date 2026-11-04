use super::*;
use crate::exclude::SystemEnvironment;
use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use std::process::Command;
use std::sync::atomic::AtomicUsize;

/// A fresh folder under the system temp folder, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("revisor-scan-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(verbatim(&self.0).unwrap());
    }
}

fn run_scan(root: &Path, threads: usize) -> Scan {
    let options = ScanOptions {
        threads,
        read_versions: true,
        exclusions: Exclusions::default(),
    };
    scan(
        root,
        &options,
        &AtomicBool::new(false),
        &Progress::default(),
    )
    .unwrap()
}

/// Copies kernel32.dll, which has a version resource, to `to`.
fn copy_kernel32(to: &Path) {
    let system = std::env::var_os("SystemRoot").unwrap();
    fs::copy(Path::new(&system).join("System32\\kernel32.dll"), to).unwrap();
}

/// Makes `link` a junction to `target`.
fn junction(link: &Path, target: &Path) {
    let status = Command::new("cmd")
        .arg("/c")
        .arg("mklink")
        .arg("/J")
        .arg(link)
        .arg(target)
        .output()
        .unwrap()
        .status;
    assert!(status.success());
}

/// Path of an entry relative to the root, with `\` separators.
fn rel_path(scan: &Scan, mut index: usize) -> String {
    let mut parts = Vec::new();
    while let Some(parent) = scan.entries[index].parent {
        parts.push(scan.entries[index].name.to_string_lossy().into_owned());
        index = parent as usize;
    }
    parts.reverse();
    parts.join("\\")
}

fn find<'a>(scan: &'a Scan, rel: &str) -> (usize, &'a Entry) {
    let index = (0..scan.entries.len())
        .find(|&i| rel_path(scan, i) == rel)
        .unwrap_or_else(|| panic!("{rel} not found"));
    (index, &scan.entries[index])
}

#[test]
fn collects_tree_and_metadata() {
    let tmp = TempDir::new();
    fs::create_dir_all(tmp.path().join("a\\b")).unwrap();
    let file = tmp.path().join("a\\b\\x.txt");
    fs::write(&file, b"hello").unwrap();
    let mut perms = fs::metadata(&file).unwrap().permissions();
    perms.set_readonly(true);
    fs::set_permissions(&file, perms).unwrap();

    let scan = run_scan(tmp.path(), 1);
    let meta = fs::metadata(&file).unwrap();

    assert_eq!(scan.entries[0].parent, None);
    assert_eq!(&*scan.entries[0].name, tmp.path().as_os_str());
    let (_, x) = find(&scan, "a\\b\\x.txt");
    assert_eq!(x.kind, EntryKind::File);
    assert_eq!(x.size, 5);
    assert_ne!(x.attributes & 0x1, 0, "read-only attribute");
    assert_eq!(x.created as u64, meta.creation_time());
    assert_eq!(x.modified as u64, meta.last_write_time());
    assert!(!x.inaccessible);

    let (_, b) = find(&scan, "a\\b");
    assert_eq!(b.kind, EntryKind::Dir);
    let ids: BTreeSet<FileId> = scan.entries.iter().map(|e| e.file_id).collect();
    assert_eq!(ids.len(), scan.entries.len(), "file IDs are unique");
    assert!(!ids.contains(&FileId::default()));
    assert_eq!((scan.stats.dirs, scan.stats.files), (3, 1));

    // Make the file writable again so the folder can be removed.
    let mut perms = meta.permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    perms.set_readonly(false);
    fs::set_permissions(&file, perms).unwrap();
}

#[test]
fn reads_file_and_product_versions_of_exe_and_dll_only() {
    let tmp = TempDir::new();
    copy_kernel32(&tmp.path().join("k.DLL"));
    fs::write(tmp.path().join("fake.exe"), b"not a PE file").unwrap();
    fs::write(tmp.path().join("plain.txt"), b"text").unwrap();

    let scan = run_scan(tmp.path(), 1);

    let VersionInfo::Some(version) = find(&scan, "k.DLL").1.version else {
        panic!("kernel32 has a version");
    };
    assert!(version.file[0] >= 6, "{version:?}");
    assert!(version.product[0] >= 6, "{version:?}");
    let fake = find(&scan, "fake.exe").1;
    assert_eq!(fake.version, VersionInfo::None);
    assert!(!fake.inaccessible);
    // Not read, rather than absent: a later revisor may read versions of more kinds of
    // files, and old snapshots must not show that as a change.
    assert_eq!(find(&scan, "plain.txt").1.version, VersionInfo::NotRead);
    assert_eq!(scan.stats.versions_read, 2);
    assert_eq!(scan.stats.versions_unread, 0);
}

#[test]
fn a_locked_exe_keeps_its_metadata_with_its_version_unread() {
    let tmp = TempDir::new();
    let exe = tmp.path().join("busy.exe");
    copy_kernel32(&exe);
    // Opened with no sharing, as a running program may hold its data file.
    let _lock = OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&exe)
        .unwrap();

    let scan = run_scan(tmp.path(), 1);

    let busy = find(&scan, "busy.exe").1;
    assert_eq!(busy.version, VersionInfo::NotRead);
    assert!(
        !busy.inaccessible,
        "a version not read is not inaccessibility"
    );
    assert_eq!(busy.size, fs::metadata(&exe).unwrap().len());
    assert_eq!(scan.stats.versions_unread, 1);
    assert_eq!(scan.stats.inaccessible, 0);
}

#[test]
fn a_file_kept_in_the_cloud_only_is_not_opened_for_its_version() {
    let tmp = TempDir::new();
    let dll = tmp.path().join("cloud.dll");
    copy_kernel32(&dll);
    // The offline attribute marks data that is not on the disk, as cloud files are.
    let status = Command::new("attrib").arg("+O").arg(&dll).status().unwrap();
    assert!(status.success());

    let scan = run_scan(tmp.path(), 1);

    let cloud = find(&scan, "cloud.dll").1;
    assert_eq!(cloud.version, VersionInfo::NotRead);
    assert_ne!(cloud.attributes & FILE_ATTRIBUTE_OFFLINE, 0);
    assert!(!cloud.inaccessible);
    assert_eq!(scan.stats.versions_read, 0);
    assert_eq!(scan.stats.versions_unread, 1);
}

#[test]
fn progress_counts_the_entries_and_names_the_folder_being_read() {
    let tmp = TempDir::new();
    fs::create_dir_all(tmp.path().join("a\\b")).unwrap();
    fs::write(tmp.path().join("a\\b\\x.txt"), b"x").unwrap();
    fs::write(tmp.path().join("y.txt"), b"y").unwrap();
    let progress = Progress::default();
    progress.add(10);

    let scan = scan(
        tmp.path(),
        &ScanOptions::default(),
        &AtomicBool::new(false),
        &progress,
    )
    .unwrap();

    // The count goes on from where it was: a snapshot of several folders shares it.
    assert_eq!(progress.entries(), 10 + scan.entries.len() as u64);
    let folder = progress.folder();
    assert!(
        folder.starts_with(tmp.path()),
        "{} is not under {}",
        folder.display(),
        tmp.path().display()
    );
    assert!(!folder.to_string_lossy().starts_with(r"\\?\"));
}

#[test]
fn reports_the_serial_number_of_the_root_volume() {
    let tmp = TempDir::new();
    fs::create_dir(tmp.path().join("inner")).unwrap();
    let outer = run_scan(tmp.path(), 1);
    let inner = run_scan(&tmp.path().join("inner"), 1);
    assert_ne!(outer.volume_serial, 0);
    assert_eq!(outer.volume_serial, inner.volume_serial);
    assert_eq!(
        outer.real_root,
        crate::folders::real_path(tmp.path()).unwrap()
    );
}

#[test]
fn a_root_that_is_a_junction_is_read_through_its_target() {
    let tmp = TempDir::new();
    let target = tmp.path().join("target");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("inside.txt"), b"x").unwrap();
    let link = tmp.path().join("link");
    junction(&link, &target);

    let scan = run_scan(&link, 1);

    assert_eq!(scan.entries[0].kind, EntryKind::Dir);
    assert_eq!(&*scan.entries[0].name, link.as_os_str());
    // Path exclusions were matched where the folder really is.
    assert_eq!(scan.real_root, crate::folders::real_path(&target).unwrap());
    find(&scan, "inside.txt");
    assert_eq!(scan.stats.files, 1);
}

#[test]
fn records_junction_without_entering() {
    let tmp = TempDir::new();
    let target = tmp.path().join("target");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("inside.txt"), b"x").unwrap();
    let link = tmp.path().join("link");
    junction(&link, &target);

    let scan = run_scan(tmp.path(), 1);

    let (index, entry) = find(&scan, "link");
    assert_eq!(entry.kind, EntryKind::Link);
    assert!(!entry.inaccessible);
    assert_eq!(entry.link_target.as_deref(), Some(target.as_os_str()));
    assert!(scan.entries.iter().all(|e| e.parent != Some(index as u32)));
    assert_eq!(scan.stats.files, 1);
}

#[test]
fn records_symlink_without_entering() {
    let tmp = TempDir::new();
    let target = tmp.path().join("target");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("inside.txt"), b"x").unwrap();
    let link = tmp.path().join("link");
    if std::os::windows::fs::symlink_dir(&target, &link).is_err() {
        eprintln!("skipped: creating symbolic links needs Developer Mode or admin rights");
        return;
    }

    let scan = run_scan(tmp.path(), 1);

    let (index, entry) = find(&scan, "link");
    assert_eq!(entry.kind, EntryKind::Link);
    assert_eq!(entry.link_target.as_deref(), Some(target.as_os_str()));
    assert!(scan.entries.iter().all(|e| e.parent != Some(index as u32)));
}

#[test]
fn files_with_other_kinds_of_reparse_point_are_ordinary_files() {
    // The aliases of Store apps (AppExecLink) are reparse points that name no other
    // object: the scan keeps them as files and never takes them for links.
    let Some(local) = std::env::var_os("LOCALAPPDATA") else {
        return;
    };
    let aliases = Path::new(&local).join(r"Microsoft\WindowsApps");
    if !aliases.is_dir() {
        eprintln!("skipped: no WindowsApps folder");
        return;
    }

    let scan = run_scan(&aliases, 2);

    let reparse: Vec<&Entry> = scan.entries[1..]
        .iter()
        .filter(|e| e.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0)
        .collect();
    if reparse.is_empty() {
        eprintln!("skipped: no app aliases installed");
        return;
    }
    for alias in reparse {
        assert_eq!(alias.kind, EntryKind::File, "{:?}", alias.name);
        assert_eq!(alias.link_target, None, "{:?}", alias.name);
        assert!(!alias.inaccessible, "{:?}", alias.name);
    }
}

#[test]
fn marks_unreadable_folder_and_goes_on() {
    let tmp = TempDir::new();
    let locked = tmp.path().join("locked");
    fs::create_dir(&locked).unwrap();
    fs::write(locked.join("hidden.txt"), b"x").unwrap();
    fs::create_dir(tmp.path().join("open")).unwrap();
    fs::write(tmp.path().join("open\\seen.txt"), b"x").unwrap();
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

    let scan = run_scan(tmp.path(), 2);
    icacls(&["/remove:d", "*S-1-1-0"]);

    let (index, entry) = find(&scan, "locked");
    assert!(entry.inaccessible);
    assert!(scan.entries.iter().all(|e| e.parent != Some(index as u32)));
    find(&scan, "open\\seen.txt");
    assert_eq!(scan.stats.inaccessible, 1);
    assert_eq!(
        scan.stats.denied, 1,
        "denied by rights, which an administrator has"
    );
}

#[test]
fn a_root_closed_by_rights_is_its_own_inaccessible_entry() {
    let tmp = TempDir::new();
    let locked = tmp.path().join("locked");
    fs::create_dir(&locked).unwrap();
    fs::write(locked.join("hidden.txt"), b"x").unwrap();
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

    let scan = run_scan(&locked, 1);
    icacls(&["/remove:d", "*S-1-1-0"]);

    assert_eq!(scan.entries.len(), 1);
    assert!(scan.entries[0].inaccessible);
    assert_eq!(scan.entries[0].kind, EntryKind::Dir);
    assert_ne!(scan.entries[0].file_id, FileId::default());
    assert_eq!(
        (scan.stats.dirs, scan.stats.inaccessible, scan.stats.denied),
        (1, 1, 1)
    );
    assert_ne!(scan.volume_serial, 0);
}

#[test]
fn scans_paths_longer_than_max_path() {
    let tmp = TempDir::new();
    let segment = "d".repeat(60);
    let mut deep = verbatim(tmp.path()).unwrap();
    for _ in 0..5 {
        deep.push(&segment);
    }
    fs::create_dir_all(&deep).unwrap();
    fs::write(deep.join("deep.txt"), b"x").unwrap();
    copy_kernel32(&deep.join("deep.dll"));

    let scan = run_scan(tmp.path(), 1);

    let rel = [segment.as_str(); 5].join("\\");
    let (_, file) = find(&scan, &format!("{rel}\\deep.txt"));
    assert_eq!(file.size, 1);
    assert!(tmp.path().join(&rel).join("deep.txt").as_os_str().len() > 260);
    let dll = find(&scan, &format!("{rel}\\deep.dll")).1;
    assert!(matches!(dll.version, VersionInfo::Some(_)));
}

#[test]
fn parallel_scan_finds_the_same_tree() {
    let tmp = TempDir::new();
    for i in 0..20 {
        for j in 0..5 {
            let dir = tmp.path().join(format!("d{i}\\e{j}"));
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("f.txt"), b"x").unwrap();
        }
    }

    let paths = |scan: &Scan| -> BTreeSet<String> {
        (0..scan.entries.len()).map(|i| rel_path(scan, i)).collect()
    };
    let one = run_scan(tmp.path(), 1);
    let many = run_scan(tmp.path(), 8);
    assert_eq!(one.entries.len(), 1 + 20 + 100 + 100);
    assert_eq!(paths(&one), paths(&many));
}

#[test]
fn excluded_files_and_folders_stay_out_with_all_they_hold() {
    let tmp = TempDir::new();
    let write = |rel: &str| {
        let path = tmp.path().join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"x").unwrap();
    };
    write(r"app\Code Cache\deep\x.bin");
    write(r"app\report.TMP");
    write(r"app\keep.txt");
    write(r"logs\old\a.log");
    write(r"logs\new.log");
    // The temp folder may be written with 8.3 names; the pattern still finds the folder.
    let old_logs = tmp.path().join(r"logs\old");
    let patterns = ["code cache", "*.tmp", &old_logs.to_string_lossy()];
    let options = ScanOptions {
        threads: 2,
        read_versions: false,
        exclusions: Exclusions::new(&patterns, &SystemEnvironment),
    };

    let scan = scan(
        tmp.path(),
        &options,
        &AtomicBool::new(false),
        &Progress::default(),
    )
    .unwrap();

    let paths: BTreeSet<String> = (1..scan.entries.len())
        .map(|i| rel_path(&scan, i))
        .collect();
    let expected = ["app", r"app\keep.txt", "logs", r"logs\new.log"];
    assert_eq!(paths, expected.map(String::from).into());
    assert_eq!((scan.stats.dirs, scan.stats.files), (3, 2));
}

#[test]
fn cancelled_scan_returns_cancelled() {
    let tmp = TempDir::new();
    fs::create_dir(tmp.path().join("a")).unwrap();
    let result = scan(
        tmp.path(),
        &ScanOptions::default(),
        &AtomicBool::new(true),
        &Progress::default(),
    );
    assert!(matches!(result, Err(ScanError::Cancelled)));
}

#[test]
fn missing_root_is_an_error() {
    let tmp = TempDir::new();
    let result = scan(
        &tmp.path().join("missing"),
        &ScanOptions::default(),
        &AtomicBool::new(false),
        &Progress::default(),
    );
    assert!(matches!(result, Err(ScanError::Root(_))));
}

#[test]
fn verbatim_paths() {
    assert_eq!(verbatim(Path::new(r"C:\")).unwrap(), Path::new(r"\\?\C:\"));
    assert_eq!(
        verbatim(Path::new(r"C:\a\..\b")).unwrap(),
        Path::new(r"\\?\C:\b")
    );
    assert_eq!(
        verbatim(Path::new(r"\\server\share\x")).unwrap(),
        Path::new(r"\\?\UNC\server\share\x")
    );
    assert_eq!(
        verbatim(Path::new(r"\\?\C:\x")).unwrap(),
        Path::new(r"\\?\C:\x")
    );
}

#[test]
fn plain_paths_drop_the_verbatim_prefix() {
    assert_eq!(plain(Path::new(r"\\?\C:\a\b")), Path::new(r"C:\a\b"));
    assert_eq!(
        plain(Path::new(r"\\?\UNC\server\share\x")),
        Path::new(r"\\server\share\x")
    );
    assert_eq!(plain(Path::new(r"C:\a")), Path::new(r"C:\a"));
}

#[test]
fn synthetic_tree_has_requested_size_and_valid_parents() {
    for count in [1, 2, 11, 1000] {
        let entries = synthetic(count);
        assert_eq!(entries.len(), count);
        assert_eq!(entries[0].parent, None);
        assert_eq!(entries[0].kind, EntryKind::Dir);
        for (i, e) in entries.iter().enumerate().skip(1) {
            let parent = e.parent.expect("only the root has no parent") as usize;
            assert!(parent < i, "entry {i} has parent {parent}");
            assert_eq!(entries[parent].kind, EntryKind::Dir);
        }
    }
    let entries = synthetic(1000);
    assert!(entries.iter().any(|e| e.kind == EntryKind::File));
    assert!(entries[1..].iter().any(|e| e.kind == EntryKind::Dir));
    assert!(
        entries
            .iter()
            .any(|e| matches!(e.version, VersionInfo::Some(_)))
    );
}
