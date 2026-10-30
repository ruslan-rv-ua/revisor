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
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const FILE_ATTRIBUTE_ARCHIVE: u32 = 0x20;
const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
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

/// One node of the scanned tree.
#[derive(Clone, Debug)]
pub struct Entry {
    /// Index of the parent in [`Scan::entries`]; `None` only for the root.
    pub parent: Option<u32>,
    /// File name; for the root, the root path as given to [`scan`].
    pub name: Box<OsStr>,
    pub kind: EntryKind,
    /// The entry exists but could not be read: a folder that cannot be listed (or was
    /// listed only in part), a link whose target cannot be read, an exe or dll whose
    /// version cannot be read.
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
    /// Numeric file version of an exe or dll that has a version resource.
    pub version: Option<[u16; 4]>,
    /// Where a link points, as stored in the reparse point.
    pub link_target: Option<Box<OsStr>>,
}

#[derive(Clone, Debug)]
pub struct ScanOptions {
    /// Worker threads that list folders; 1 scans sequentially.
    pub threads: usize,
    /// Read versions of exe and dll files.
    pub read_versions: bool,
}

impl Default for ScanOptions {
    fn default() -> Self {
        let threads = thread::available_parallelism().map_or(4, |n| n.get().min(8));
        Self {
            threads,
            read_versions: true,
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
    pub versions_read: u64,
    /// Time spent reading versions, summed over all worker threads.
    pub version_time: Duration,
}

/// Result of a finished scan: a tree flattened into a vector. The root is entry 0, and
/// the children of each folder are stored next to each other.
#[derive(Debug)]
pub struct Scan {
    pub entries: Vec<Entry>,
    pub stats: ScanStats,
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

/// Scans the folder tree under `root`. Setting `cancel` stops the scan with
/// [`ScanError::Cancelled`].
pub fn scan(root: &Path, options: &ScanOptions, cancel: &AtomicBool) -> Result<Scan, ScanError> {
    let root_path = verbatim(root).map_err(ScanError::Root)?;
    let info = sys::open_dir(&root_path)
        .and_then(|dir| sys::dir_info(&dir))
        .map_err(ScanError::Root)?;
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
        inaccessible: false,
        attributes: info.attributes,
        size: 0,
        created: info.created,
        modified: info.modified,
        file_id: FileId(info.file_id),
        version: None,
        link_target: None,
    };

    let walker = Walker {
        entries: Mutex::new(vec![root_entry]),
        queue: Mutex::new(Queue {
            jobs: vec![Job {
                index: 0,
                path: root_path,
            }],
            active: 0,
        }),
        wake: Condvar::new(),
        cancel,
        read_versions: options.read_versions,
        versions_read: AtomicU64::new(0),
        version_nanos: AtomicU64::new(0),
    };
    thread::scope(|s| {
        for _ in 1..options.threads.max(1) {
            s.spawn(|| walker.work());
        }
        walker.work();
    });
    if cancel.load(Ordering::Relaxed) {
        return Err(ScanError::Cancelled);
    }

    let entries = walker.entries.into_inner().unwrap();
    let mut stats = ScanStats {
        versions_read: walker.versions_read.into_inner(),
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
    Ok(Scan { entries, stats })
}

/// Turns `path` into an absolute `\\?\` path, so paths longer than MAX_PATH work.
fn verbatim(path: &Path) -> io::Result<PathBuf> {
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
            EntryKind::Dir => (format!("folder {i:06}"), FILE_ATTRIBUTE_DIRECTORY, 0, None),
            _ => {
                let ext = EXTENSIONS[i % EXTENSIONS.len()];
                let version = matches!(ext, "dll" | "exe").then(|| [10, 0, (i % 30_000) as u16, 1]);
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
    cancel: &'a AtomicBool,
    read_versions: bool,
    versions_read: AtomicU64,
    version_nanos: AtomicU64,
}

impl Walker<'_> {
    fn work(&self) {
        while let Some(job) = self.next_job() {
            let (children, subdirs, failed) = self.list(&job.path);
            let base = {
                let mut entries = self.entries.lock().unwrap();
                if failed {
                    entries[job.index as usize].inaccessible = true;
                }
                let base = entries.len();
                entries.extend(children.into_iter().map(|mut e| {
                    e.parent = Some(job.index);
                    e
                }));
                base
            };
            let mut queue = self.queue.lock().unwrap();
            for (offset, path) in subdirs {
                let index = u32::try_from(base + offset).expect("more than 4G entries");
                queue.jobs.push(Job { index, path });
            }
            queue.active -= 1;
            self.wake.notify_all();
        }
    }

    /// Takes the next folder to list, or `None` when the walk is over or cancelled.
    fn next_job(&self) -> Option<Job> {
        let mut queue = self.queue.lock().unwrap();
        loop {
            if self.cancel.load(Ordering::Relaxed) {
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

    /// Lists one folder. Returns its children (parent not yet set), the positions and
    /// paths of child folders to descend into, and whether the listing failed.
    fn list(&self, path: &Path) -> (Vec<Entry>, Vec<(usize, PathBuf)>, bool) {
        let mut raw = Vec::new();
        let failed = sys::open_dir(path)
            .and_then(|dir| sys::list_dir(&dir, &mut raw))
            .is_err();

        let mut children = Vec::with_capacity(raw.len());
        let mut subdirs = Vec::new();
        for r in raw {
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
                version: None,
                link_target: None,
            };
            let full = || path.join(&*entry.name);
            match kind {
                EntryKind::Dir => subdirs.push((children.len(), full())),
                EntryKind::Link => match sys::link_target(&full()) {
                    Ok(target) => entry.link_target = target.map(OsString::into_boxed_os_str),
                    Err(_) => entry.inaccessible = true,
                },
                EntryKind::File => {
                    if self.read_versions
                        && has_version_extension(&entry.name)
                        && !self.cancel.load(Ordering::Relaxed)
                    {
                        let full = full();
                        self.read_version(&full, &mut entry);
                    }
                }
            }
            children.push(entry);
        }
        (children, subdirs, failed)
    }

    fn read_version(&self, path: &Path, entry: &mut Entry) {
        let started = Instant::now();
        match sys::file_version(path) {
            Ok(version) => entry.version = version,
            Err(_) => entry.inaccessible = true,
        }
        let nanos = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
        self.version_nanos.fetch_add(nanos, Ordering::Relaxed);
        self.versions_read.fetch_add(1, Ordering::Relaxed);
    }
}

fn has_version_extension(name: &OsStr) -> bool {
    Path::new(name)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("exe") || ext.eq_ignore_ascii_case("dll"))
}

#[cfg(test)]
mod tests;
