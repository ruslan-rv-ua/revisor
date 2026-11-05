//! File system scanner: walks a folder tree and collects the metadata a snapshot keeps.
//!
//! Directories are listed with `FileIdExtdDirectoryInfo`, so size, times, attributes and
//! the NTFS file ID come from the listing itself and files are never opened. Only exe and
//! dll files are opened, to read their version. Junctions and symbolic links are recorded
//! with their target and never followed.

mod sys;

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::io;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::exclude::{Branch, Exclusions};
use crate::job::Cancel;

const FILE_ATTRIBUTE_ARCHIVE: u32 = 0x20;
const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
/// The data of the file is not (all) on the disk: it is kept offline, or a cloud sync
/// engine recalls it on open or on reading. Reading the file would download it.
const FILE_ATTRIBUTE_OFFLINE: u32 = 0x1000;
const FILE_ATTRIBUTE_RECALL_ON_OPEN: u32 = 0x4_0000;
const FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS: u32 = 0x40_0000;
const DATA_NOT_ON_DISK: u32 =
    FILE_ATTRIBUTE_OFFLINE | FILE_ATTRIBUTE_RECALL_ON_OPEN | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS;
/// How often idle workers wake up to notice cancellation.
const IDLE_WAIT: Duration = Duration::from_millis(50);

/// 128-bit NTFS file ID; stays the same when a file is renamed or moved within a volume.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct FileId(pub [u8; 16]);

/// What an entry is in the tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind {
    Dir,
    File,
    /// Junction, symbolic link or another name-surrogate reparse point; never followed.
    Link,
}

/// The numeric versions in the version resource of an exe or dll.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Version {
    /// The version of the file itself.
    pub file: [u16; 4],
    /// The version of the product the file ships with.
    pub product: [u16; 4],
}

/// What a snapshot knows about the version of a file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum VersionInfo {
    /// The version was not read: the file is not an exe or dll, its content could not be
    /// read (no access, or locked by another program), or its data is not on the disk.
    /// Versions are compared only when both snapshots read them.
    #[default]
    NotRead,
    /// The file was read and has no version resource.
    None,
    Some(Version),
}

/// One node of the scanned tree.
#[derive(Clone, Debug)]
pub struct Entry {
    /// Index of the parent in [`Scan::entries`]; `None` only for the root.
    pub parent: Option<u32>,
    /// File name; for the root, the root path as given to [`scan`].
    pub name: Box<OsStr>,
    pub kind: EntryKind,
    /// The entry exists but could not be read: a folder that cannot be listed (or was
    /// listed only in part), or a link whose target cannot be read.
    pub inaccessible: bool,
    /// Win32 `FILE_ATTRIBUTE_*` flags.
    pub attributes: u32,
    /// Size in bytes; 0 for folders.
    pub size: u64,
    /// Creation time as FILETIME (100 ns ticks since 1601-01-01 UTC).
    pub created: i64,
    /// Last write time as FILETIME.
    pub modified: i64,
    pub file_id: FileId,
    pub version: VersionInfo,
    /// Where a link points, as stored in the reparse point.
    pub link_target: Option<Box<OsStr>>,
}

#[derive(Clone, Debug)]
pub struct ScanOptions {
    /// Worker threads that list folders; 1 scans sequentially.
    pub threads: usize,
    /// Read versions of exe and dll files.
    pub read_versions: bool,
    /// Entries that stay out of the scan; an excluded folder is not entered. The root is
    /// never excluded.
    pub exclusions: Exclusions,
}

impl Default for ScanOptions {
    fn default() -> Self {
        let threads = thread::available_parallelism().map_or(4, |n| n.get().min(8));
        Self {
            threads,
            read_versions: true,
            exclusions: Exclusions::default(),
        }
    }
}

/// Counters collected while scanning.
#[derive(Clone, Debug, Default)]
pub struct ScanStats {
    pub dirs: u64,
    pub files: u64,
    pub links: u64,
    pub inaccessible: u64,
    /// Inaccessible entries that access rights kept closed; an administrator may read them.
    pub denied: u64,
    /// Exe and dll files whose version was read, whether or not they have one.
    pub versions_read: u64,
    /// Exe and dll files whose version could not be read, or was not read because their
    /// data is not on the disk.
    pub versions_unread: u64,
    /// Time spent reading versions, summed over all worker threads.
    pub version_time: Duration,
}

/// Result of a finished scan: a tree flattened into a vector. The root is entry 0, and
/// the children of each folder are stored next to each other.
#[derive(Debug)]
pub struct Scan {
    pub entries: Vec<Entry>,
    pub stats: ScanStats,
    /// The serial number of the volume the root is on; file IDs are unique within it.
    pub volume_serial: u64,
    /// Where the root really is: the location path exclusions were matched from. The
    /// root path itself when that could not be found out.
    pub real_root: PathBuf,
}

/// What a running scan reports to whoever waits for it: how many entries it has
/// collected and the folder it is reading. Shared with another thread, which reads it now
/// and then; a snapshot of several folders passes the same one to each scan, so the count
/// goes on.
#[derive(Debug, Default)]
pub struct Progress {
    entries: AtomicU64,
    folder: Mutex<PathBuf>,
}

impl Progress {
    /// How many entries have been collected so far.
    pub fn entries(&self) -> u64 {
        self.entries.load(Ordering::Relaxed)
    }

    /// The folder being read now, as a plain path.
    pub fn folder(&self) -> PathBuf {
        self.folder.lock().unwrap().clone()
    }

    pub fn add(&self, entries: u64) {
        self.entries.fetch_add(entries, Ordering::Relaxed);
    }

    fn set_folder(&self, folder: &Path) {
        let mut current = self.folder.lock().unwrap();
        current.clear();
        current.push(plain(folder));
    }
}

#[derive(Debug)]
pub enum ScanError {
    Cancelled,
    /// The root folder could not be opened or read.
    Root(io::Error),
}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScanError::Cancelled => f.write_str("scan cancelled"),
            ScanError::Root(e) => write!(f, "cannot read the root folder: {e}"),
        }
    }
}

impl std::error::Error for ScanError {}

/// Scans the folder tree under `root`, reporting to `progress` as it goes. Cancelling
/// `cancel` stops the scan with [`ScanError::Cancelled`]. A root that is a junction or a
/// symbolic link is read through its target; a root that rights keep closed is the one
/// inaccessible entry of its tree, like any such folder deeper down.
pub fn scan(
    root: &Path,
    options: &ScanOptions,
    cancel: &Cancel,
    progress: &Progress,
) -> Result<Scan, ScanError> {
    let root_path = verbatim(root).map_err(ScanError::Root)?;
    let (info, denied) = match sys::open_dir(&root_path) {
        Ok(dir) => (sys::dir_info(&dir).map_err(ScanError::Root)?, false),
        Err(e) if sys::is_access_denied(&e) => {
            let info = sys::open_dir_attributes(&root_path)
                .and_then(|dir| sys::dir_info(&dir))
                .map_err(|_| ScanError::Root(e))?;
            (info, true)
        }
        Err(e) => return Err(ScanError::Root(e)),
    };
    if info.attributes & FILE_ATTRIBUTE_DIRECTORY == 0 {
        return Err(ScanError::Root(io::Error::new(
            io::ErrorKind::NotADirectory,
            "not a folder",
        )));
    }
    let root_entry = Entry {
        parent: None,
        name: root.as_os_str().into(),
        kind: EntryKind::Dir,
        inaccessible: denied,
        attributes: info.attributes,
        size: 0,
        created: info.created,
        modified: info.modified,
        file_id: FileId(info.file_id),
        version: VersionInfo::NotRead,
        link_target: None,
    };
    progress.add(1);
    // Path patterns name real locations, so the root is matched where it really is.
    let real_root = crate::folders::real_path(root).unwrap_or_else(|_| root.to_path_buf());
    if denied {
        return Ok(Scan {
            entries: vec![root_entry],
            stats: ScanStats {
                dirs: 1,
                inaccessible: 1,
                denied: 1,
                ..ScanStats::default()
            },
            volume_serial: info.volume_serial,
            real_root,
        });
    }

    let walker = Walker {
        entries: Mutex::new(vec![root_entry]),
        queue: Mutex::new(Queue {
            jobs: vec![Job {
                index: 0,
                path: root_path,
                branch: options.exclusions.branch(&real_root),
            }],
            active: 0,
        }),
        wake: Condvar::new(),
        cancel,
        progress,
        exclusions: &options.exclusions,
        read_versions: options.read_versions,
        denied: AtomicU64::new(0),
        versions_read: AtomicU64::new(0),
        versions_unread: AtomicU64::new(0),
        version_nanos: AtomicU64::new(0),
    };
    thread::scope(|s| {
        for _ in 1..options.threads.max(1) {
            s.spawn(|| walker.work());
        }
        walker.work();
    });
    if cancel.is_cancelled() {
        return Err(ScanError::Cancelled);
    }

    let entries = walker.entries.into_inner().unwrap();
    let mut stats = ScanStats {
        denied: walker.denied.into_inner(),
        versions_read: walker.versions_read.into_inner(),
        versions_unread: walker.versions_unread.into_inner(),
        version_time: Duration::from_nanos(walker.version_nanos.into_inner()),
        ..ScanStats::default()
    };
    for e in &entries {
        match e.kind {
            EntryKind::Dir => stats.dirs += 1,
            EntryKind::File => stats.files += 1,
            EntryKind::Link => stats.links += 1,
        }
        stats.inaccessible += u64::from(e.inaccessible);
    }
    Ok(Scan {
        entries,
        stats,
        volume_serial: info.volume_serial,
        real_root,
    })
}

/// Turns `path` into an absolute `\\?\` path, so paths longer than MAX_PATH work.
pub(crate) fn verbatim(path: &Path) -> io::Result<PathBuf> {
    let abs = std::path::absolute(path)?;
    let wide: Vec<u16> = abs.as_os_str().encode_wide().collect();
    let starts = |prefix: &str| wide.starts_with(&prefix.encode_utf16().collect::<Vec<_>>());
    let out: Vec<u16> = if starts(r"\\?\") || starts(r"\\.\") {
        return Ok(abs);
    } else if starts(r"\\") {
        r"\\?\UNC\"
            .encode_utf16()
            .chain(wide[2..].iter().copied())
            .collect()
    } else {
        r"\\?\".encode_utf16().chain(wide.iter().copied()).collect()
    };
    Ok(PathBuf::from(OsString::from_wide(&out)))
}

/// A `\\?\` path as a plain one, the way it is shown: `\\?\C:\x` is `C:\x`, and
/// `\\?\UNC\server\share` is `\\server\share`.
pub(crate) fn plain(path: &Path) -> PathBuf {
    let wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    let strip = |prefix: &str| -> Option<&[u16]> {
        let prefix: Vec<u16> = prefix.encode_utf16().collect();
        wide.starts_with(&prefix).then(|| &wide[prefix.len()..])
    };
    let out: Vec<u16> = match (strip(r"\\?\UNC\"), strip(r"\\?\")) {
        (Some(rest), _) => r"\\".encode_utf16().chain(rest.iter().copied()).collect(),
        (None, Some(rest)) => rest.to_vec(),
        (None, None) => return path.to_path_buf(),
    };
    PathBuf::from(OsString::from_wide(&out))
}

/// Builds a made-up tree of `count` entries laid out like a [`scan`] result, without
/// touching the disk; for tests and benchmarks. Every folder gets 2 subfolders and
/// 10 files until the count is reached, which is about the folder share of a real drive.
pub fn synthetic(count: usize) -> Vec<Entry> {
    const SUBDIRS: usize = 2;
    const CHILDREN: usize = 12;
    const EXTENSIONS: [&str; 6] = ["txt", "dat", "dll", "jpg", "exe", "log"];
    /// 2026-01-01 as FILETIME.
    const BASE_TIME: i64 = 134_116_992_000_000_000;

    // xorshift64: deterministic, so repeated runs produce the same tree.
    let mut state = 0x2545_F491_4F6C_DD1D_u64;
    let mut random = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let mut make = |i: usize, parent: Option<u32>, kind: EntryKind| {
        let (name, attributes, size, version) = match kind {
            EntryKind::Dir => (
                format!("folder {i:06}"),
                FILE_ATTRIBUTE_DIRECTORY,
                0,
                VersionInfo::NotRead,
            ),
            _ => {
                let ext = EXTENSIONS[i % EXTENSIONS.len()];
                let version = if matches!(ext, "dll" | "exe") {
                    VersionInfo::Some(Version {
                        file: [10, 0, (i % 30_000) as u16, 1],
                        product: [10, 0, 0, 0],
                    })
                } else {
                    VersionInfo::NotRead
                };
                (
                    format!("file {i:06}.{ext}"),
                    FILE_ATTRIBUTE_ARCHIVE,
                    random() % 10_000_000,
                    version,
                )
            }
        };
        let created = BASE_TIME + (random() % 10_000_000_000) as i64;
        let mut file_id = [0; 16];
        file_id[..8].copy_from_slice(&(i as u64).to_le_bytes());
        Entry {
            parent,
            name: OsString::from(name).into_boxed_os_str(),
            kind,
            inaccessible: false,
            attributes,
            size,
            created,
            modified: created + (random() % 10_000_000_000) as i64,
            file_id: FileId(file_id),
            version,
            link_target: None,
        }
    };

    let mut entries = Vec::with_capacity(count);
    if count == 0 {
        return entries;
    }
    let mut root = make(0, None, EntryKind::Dir);
    root.name = OsStr::new(r"C:\synthetic").into();
    entries.push(root);
    let mut dir = 0;
    while entries.len() < count {
        for k in 0..CHILDREN.min(count - entries.len()) {
            let kind = if k < SUBDIRS {
                EntryKind::Dir
            } else {
                EntryKind::File
            };
            entries.push(make(entries.len(), Some(dir as u32), kind));
        }
        dir += 1;
        while dir < entries.len() && entries[dir].kind != EntryKind::Dir {
            dir += 1;
        }
    }
    entries
}

struct Job {
    index: u32,
    path: PathBuf,
    /// Where the folder stands against the exclusions.
    branch: Branch,
}

struct Queue {
    /// Folders waiting to be listed; used as a stack to keep it short.
    jobs: Vec<Job>,
    /// Folders being listed right now; they may still add jobs.
    active: usize,
}

struct Walker<'a> {
    entries: Mutex<Vec<Entry>>,
    queue: Mutex<Queue>,
    wake: Condvar,
    cancel: &'a Cancel,
    progress: &'a Progress,
    exclusions: &'a Exclusions,
    read_versions: bool,
    denied: AtomicU64,
    versions_read: AtomicU64,
    versions_unread: AtomicU64,
    version_nanos: AtomicU64,
}

/// What listing one folder found.
struct Listing {
    /// The children, with their parent not yet set.
    children: Vec<Entry>,
    /// Child folders to descend into: position among the children, path and branch.
    subdirs: Vec<(usize, PathBuf, Branch)>,
    /// Why the listing failed, if it did.
    failed: Option<io::Error>,
}

impl Walker<'_> {
    fn work(&self) {
        while let Some(job) = self.next_job() {
            self.progress.set_folder(&job.path);
            let listing = self.list(&job);
            if let Some(e) = &listing.failed {
                self.note_denied(e);
            }
            self.progress.add(listing.children.len() as u64);
            let base = {
                let mut entries = self.entries.lock().unwrap();
                if listing.failed.is_some() {
                    entries[job.index as usize].inaccessible = true;
                }
                let base = entries.len();
                entries.extend(listing.children.into_iter().map(|mut e| {
                    e.parent = Some(job.index);
                    e
                }));
                base
            };
            let mut queue = self.queue.lock().unwrap();
            for (offset, path, branch) in listing.subdirs {
                let index = u32::try_from(base + offset).expect("more than 4G entries");
                queue.jobs.push(Job {
                    index,
                    path,
                    branch,
                });
            }
            queue.active -= 1;
            self.wake.notify_all();
        }
    }

    /// Takes the next folder to list, or `None` when the walk is over or cancelled.
    fn next_job(&self) -> Option<Job> {
        let mut queue = self.queue.lock().unwrap();
        loop {
            if self.cancel.is_cancelled() {
                return None;
            }
            if let Some(job) = queue.jobs.pop() {
                queue.active += 1;
                return Some(job);
            }
            if queue.active == 0 {
                return None;
            }
            queue = self.wake.wait_timeout(queue, IDLE_WAIT).unwrap().0;
        }
    }

    /// Lists one folder, leaving out the excluded children.
    fn list(&self, job: &Job) -> Listing {
        let path = &job.path;
        let mut raw = Vec::new();
        let failed = sys::open_dir(path)
            .and_then(|dir| sys::list_dir(&dir, &mut raw))
            .err();

        let mut children = Vec::with_capacity(raw.len());
        let mut subdirs = Vec::new();
        for r in raw {
            let Some(branch) = self.exclusions.child(&job.branch, &r.name) else {
                continue;
            };
            let is_reparse = r.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0;
            let kind = if is_reparse && sys::is_name_surrogate(r.reparse_tag) {
                EntryKind::Link
            } else if r.attributes & FILE_ATTRIBUTE_DIRECTORY != 0 {
                EntryKind::Dir
            } else {
                EntryKind::File
            };
            let mut entry = Entry {
                parent: None,
                name: r.name.into_boxed_os_str(),
                kind,
                inaccessible: false,
                attributes: r.attributes,
                size: if kind == EntryKind::Dir { 0 } else { r.size },
                created: r.created,
                modified: r.modified,
                file_id: FileId(r.file_id),
                version: VersionInfo::NotRead,
                link_target: None,
            };
            let full = || path.join(&*entry.name);
            match kind {
                EntryKind::Dir => subdirs.push((children.len(), full(), branch)),
                EntryKind::Link => match sys::link_target(&full()) {
                    Ok(target) => entry.link_target = target.map(OsString::into_boxed_os_str),
                    Err(e) => {
                        self.note_denied(&e);
                        entry.inaccessible = true;
                    }
                },
                EntryKind::File => {
                    if self.read_versions && has_version_extension(&entry.name) {
                        if entry.attributes & DATA_NOT_ON_DISK != 0 {
                            // Reading it would download it from the cloud.
                            self.versions_unread.fetch_add(1, Ordering::Relaxed);
                        } else if !self.cancel.is_cancelled() {
                            let full = full();
                            self.read_version(&full, &mut entry);
                        }
                    }
                }
            }
            children.push(entry);
        }
        Listing {
            children,
            subdirs,
            failed,
        }
    }

    fn read_version(&self, path: &Path, entry: &mut Entry) {
        let started = Instant::now();
        match sys::file_version(path) {
            Ok(Some(version)) => {
                entry.version = VersionInfo::Some(version);
                self.versions_read.fetch_add(1, Ordering::Relaxed);
            }
            Ok(None) => {
                entry.version = VersionInfo::None;
                self.versions_read.fetch_add(1, Ordering::Relaxed);
            }
            // The file itself is still in the snapshot with all the listing knows.
            Err(_) => {
                self.versions_unread.fetch_add(1, Ordering::Relaxed);
            }
        }
        let nanos = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
        self.version_nanos.fetch_add(nanos, Ordering::Relaxed);
    }

    /// Counts an inaccessible entry that rights kept closed.
    fn note_denied(&self, error: &io::Error) {
        if sys::is_access_denied(error) {
            self.denied.fetch_add(1, Ordering::Relaxed);
        }
    }
}

fn has_version_extension(name: &OsStr) -> bool {
    Path::new(name)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("exe") || ext.eq_ignore_ascii_case("dll"))
}

#[cfg(test)]
mod tests;
