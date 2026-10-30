use super::*;
use std::collections::BTreeSet;
use std::fs;
use std::os::windows::fs::MetadataExt;
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
    };
    scan(root, &options, &AtomicBool::new(false)).unwrap()
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
fn reads_exe_and_dll_versions() {
    let tmp = TempDir::new();
    let system = std::env::var_os("SystemRoot").unwrap();
    let kernel32 = Path::new(&system).join("System32\\kernel32.dll");
    fs::copy(&kernel32, tmp.path().join("k.DLL")).unwrap();
    fs::write(tmp.path().join("fake.exe"), b"not a PE file").unwrap();
    fs::write(tmp.path().join("plain.txt"), b"text").unwrap();

    let scan = run_scan(tmp.path(), 1);

    let version = find(&scan, "k.DLL")
        .1
        .version
        .expect("kernel32 has a version");
    assert!(version[0] >= 6, "{version:?}");
    let fake = find(&scan, "fake.exe").1;
    assert_eq!(fake.version, None);
    assert!(!fake.inaccessible);
    assert_eq!(find(&scan, "plain.txt").1.version, None);
    assert_eq!(scan.stats.versions_read, 2);
}

#[test]
fn records_junction_without_entering() {
    let tmp = TempDir::new();
    let target = tmp.path().join("target");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("inside.txt"), b"x").unwrap();
    let link = tmp.path().join("link");
    let status = Command::new("cmd")
        .arg("/c")
        .arg("mklink")
        .arg("/J")
        .arg(&link)
        .arg(&target)
        .output()
        .unwrap()
        .status;
    assert!(status.success());

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
    let system = std::env::var_os("SystemRoot").unwrap();
    fs::copy(
        Path::new(&system).join("System32\\kernel32.dll"),
        deep.join("deep.dll"),
    )
    .unwrap();

    let scan = run_scan(tmp.path(), 1);

    let rel = [segment.as_str(); 5].join("\\");
    let (_, file) = find(&scan, &format!("{rel}\\deep.txt"));
    assert_eq!(file.size, 1);
    assert!(tmp.path().join(&rel).join("deep.txt").as_os_str().len() > 260);
    let dll = find(&scan, &format!("{rel}\\deep.dll")).1;
    assert!(dll.version.is_some());
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
fn cancelled_scan_returns_cancelled() {
    let tmp = TempDir::new();
    fs::create_dir(tmp.path().join("a")).unwrap();
    let result = scan(tmp.path(), &ScanOptions::default(), &AtomicBool::new(true));
    assert!(matches!(result, Err(ScanError::Cancelled)));
}

#[test]
fn missing_root_is_an_error() {
    let tmp = TempDir::new();
    let result = scan(
        &tmp.path().join("missing"),
        &ScanOptions::default(),
        &AtomicBool::new(false),
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
