//! Helpers shared by the tests of several modules: a temporary folder and small trees of
//! entries built by path.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crate::diff::{self, Fields};
use crate::job::{Cancel, Job, Lost, Work};
use crate::path::verbatim;
use crate::scan::{Entry, EntryKind, FileId, VersionInfo};

const FILE_ATTRIBUTE_ARCHIVE: u32 = 0x20;
const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;

/// A fresh folder under the system temp folder, removed on drop.
pub struct TempDir(PathBuf);

impl TempDir {
    /// A folder named after `prefix`, this process and a counter, so tests do not share
    /// one.
    pub fn new(prefix: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("revisor-{prefix}-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    /// A profile database in this folder.
    pub fn db(&self) -> PathBuf {
        self.0.join("profile.db")
    }

    /// The rollback journal of [`TempDir::db`]; it exists while a write transaction is
    /// open.
    pub fn journal(&self) -> PathBuf {
        self.0.join("profile.db-journal")
    }

    /// Creates a folder under this one and returns its path.
    pub fn folder(&self, rel: &str) -> PathBuf {
        let path = self.0.join(rel);
        fs::create_dir_all(&path).unwrap();
        path
    }

    /// Writes a file under this folder, making its folders.
    pub fn write(&self, rel: &str, content: &[u8]) -> PathBuf {
        let path = self.0.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, content).unwrap();
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(verbatim(&self.0));
    }
}

/// A token cancelled already: what an action finds when it was cancelled before it began.
pub fn cancelled() -> Cancel {
    let cancel = Cancel::new();
    cancel.cancel();
    cancel
}

/// Polls `job` until it ends, as a front end does on its timer, and gives its result.
pub fn poll_until_ended<W: Work>(job: &mut Job<W>) -> Result<W::Output, Lost> {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(result) = job.poll() {
            return result;
        }
        assert!(Instant::now() < deadline, "the job never ended");
        thread::sleep(Duration::from_millis(5));
    }
}

/// Makes `link` a junction to `target`.
pub fn junction(link: &Path, target: &Path) {
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

/// Builds small trees by path relative to the root, laid out like a scan.
#[derive(Clone)]
pub struct Tree(pub Vec<Entry>);

impl Tree {
    /// A tree rooted at `C:\Data`.
    pub fn new() -> Self {
        Self::with_root(r"C:\Data")
    }

    pub fn with_root(path: &str) -> Self {
        Tree(vec![entry(None, path, EntryKind::Dir)])
    }

    pub fn index(&self, rel: &str) -> u32 {
        (0..self.0.len() as u32)
            .find(|&i| rel_path(&self.0, i) == rel)
            .unwrap_or_else(|| panic!("{rel} not found"))
    }

    pub fn add(&mut self, rel: &str, kind: EntryKind) -> &mut Entry {
        let (parent, name) = rel.rsplit_once('\\').unwrap_or(("", rel));
        let parent = self.index(parent);
        self.0.push(entry(Some(parent), name, kind));
        self.0.last_mut().unwrap()
    }

    pub fn get(&mut self, rel: &str) -> &mut Entry {
        let i = self.index(rel);
        &mut self.0[i as usize]
    }

    /// Removes an entry with its subtree.
    pub fn remove(&mut self, rel: &str) {
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
pub fn renumber(entries: Vec<Entry>, removed: &[bool]) -> Vec<Entry> {
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

/// An entry as a scan would make it, with no times and no version.
pub fn entry(parent: Option<u32>, name: &str, kind: EntryKind) -> Entry {
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
        version: VersionInfo::NotRead,
        link_target: None,
    }
}

/// Path of an entry relative to the root, with `\` separators; "" for the root.
pub fn rel_path(entries: &[Entry], index: u32) -> String {
    let full = diff::path(entries, index);
    let root = Path::new(&*entries[0].name);
    full.strip_prefix(root)
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

/// The changed fields by name, comma-separated.
pub fn field_names(fields: Fields) -> String {
    let names = [
        (Fields::NAME, "name"),
        (Fields::SIZE, "size"),
        (Fields::CREATED, "created"),
        (Fields::MODIFIED, "modified"),
        (Fields::ATTRIBUTES, "attributes"),
        (Fields::VERSION, "version"),
        (Fields::LINK_TARGET, "target"),
        (Fields::FILE_ID, "file_id"),
    ];
    let present: Vec<&str> = names
        .iter()
        .filter(|(f, _)| fields.contains(*f))
        .map(|(_, n)| *n)
        .collect();
    present.join(",")
}
