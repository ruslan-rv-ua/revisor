//! The profiles in the data folder: each one is a single database file named after the
//! profile (ADR-0006, TASK-7). The name the interface shows is the one in the database;
//! the file name only helps to tell the files apart.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use rusqlite::ErrorCode;

use crate::names::eq_ignore_case;
use crate::store::{ProfileSettings, SnapshotInfo, Store, StoreError};

/// Names Windows keeps for devices, with or without an extension.
const DEVICE_NAMES: [&str; 28] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "COM¹", "COM²", "COM³", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8",
    "LPT9", "LPT¹", "LPT²", "LPT³",
];
/// The longest file name, without its extension, made from a profile name.
const MAX_STEM: usize = 80;

/// A profile in the folder of profiles.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileFile {
    pub path: PathBuf,
    pub name: String,
}

/// A database file in the folder of profiles that could not be read as a profile.
#[derive(Debug)]
pub struct Unreadable {
    pub path: PathBuf,
    pub error: StoreError,
}

#[derive(Debug)]
pub enum ProfileError {
    /// Another profile has this name already, regardless of case.
    NameTaken,
    Store(StoreError),
    Io(io::Error),
}

impl fmt::Display for ProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProfileError::NameTaken => f.write_str("another profile has this name"),
            ProfileError::Store(e) => e.fmt(f),
            ProfileError::Io(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for ProfileError {}

impl From<StoreError> for ProfileError {
    fn from(e: StoreError) -> Self {
        ProfileError::Store(e)
    }
}

/// The profiles in `dir`, in no particular order, and the database files there that are
/// not profiles revisor can open. A folder that does not exist holds no profiles.
pub fn list(dir: &Path) -> io::Result<(Vec<ProfileFile>, Vec<Unreadable>)> {
    let mut profiles = Vec::new();
    let mut unreadable = Vec::new();
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok((profiles, unreadable)),
        Err(e) => return Err(e),
    };
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let is_db = path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("db"));
        if !is_db || !entry.file_type()?.is_file() {
            continue;
        }
        match settings_of(&path) {
            Ok(settings) => profiles.push(ProfileFile {
                path,
                name: settings.name,
            }),
            Err(error) => unreadable.push(Unreadable { path, error }),
        }
    }
    Ok((profiles, unreadable))
}

/// The settings of the profile in the file at `path`.
pub fn settings_of(path: &Path) -> Result<ProfileSettings, StoreError> {
    Store::open(path)?.settings()
}

/// How many snapshots the profile in the file at `path` has.
pub fn snapshot_count(path: &Path) -> Result<usize, StoreError> {
    Store::open(path)?.snapshot_count()
}

/// The snapshots of the profile in the file at `path`, oldest first.
pub fn snapshots_of(path: &Path) -> Result<Vec<SnapshotInfo>, StoreError> {
    Store::open(path)?.list()
}

/// Whether a store error says the database is busy with another connection's write, such
/// as another revisor compacting it: a reader may then wait and try again (TASK-9).
pub fn is_busy(error: &StoreError) -> bool {
    match error {
        StoreError::Sqlite(e) => matches!(
            e.sqlite_error_code(),
            Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked)
        ),
        _ => false,
    }
}

/// Whether a profile other than the one in the file `except` has `name`, regardless of
/// case.
pub fn name_taken(profiles: &[ProfileFile], name: &str, except: Option<&Path>) -> bool {
    profiles
        .iter()
        .any(|p| !same_file_name(&p.path, except) && eq_ignore_case(&p.name, name))
}

/// Creates a new profile in `dir`, making the folder when needed.
pub fn create(dir: &Path, settings: &ProfileSettings) -> Result<ProfileFile, ProfileError> {
    fs::create_dir_all(dir).map_err(ProfileError::Io)?;
    let (profiles, _) = list(dir).map_err(ProfileError::Io)?;
    if name_taken(&profiles, &settings.name, None) {
        return Err(ProfileError::NameTaken);
    }
    let path = free_path(dir, &stem(&settings.name), None);
    Store::create(&path, settings)?;
    Ok(ProfileFile {
        path,
        name: settings.name.clone(),
    })
}

/// Saves new settings of the profile in the file at `path`. A new name renames the file
/// too, unless the file is in use (by another revisor, say): then the file keeps its old
/// name, which is harmless, as the name shown is the one in the database.
pub fn update(path: &Path, settings: &ProfileSettings) -> Result<ProfileFile, ProfileError> {
    let dir = path.parent().unwrap_or(Path::new(""));
    let (profiles, _) = list(dir).map_err(ProfileError::Io)?;
    if name_taken(&profiles, &settings.name, Some(path)) {
        return Err(ProfileError::NameTaken);
    }
    let mut store = Store::open(path)?;
    let old_name = store.settings()?.name;
    store.set_settings(settings)?;
    drop(store);

    let mut path = path.to_path_buf();
    if old_name != settings.name {
        let target = free_path(dir, &stem(&settings.name), Some(&path));
        if target != path && fs::rename(&path, &target).is_ok() {
            path = target;
        }
    }
    Ok(ProfileFile {
        path,
        name: settings.name.clone(),
    })
}

/// Deletes the profile in the file at `path`, with all its snapshots.
pub fn delete(path: &Path) -> io::Result<()> {
    fs::remove_file(path)?;
    // A journal left behind by a crash must not stay to be taken for the journal of a new
    // profile with the same file name.
    let _ = fs::remove_file(journal(path));
    Ok(())
}

/// A file name, without extension, for a profile called `name`: the characters a file
/// name cannot have become "_", dots and spaces at the end go, and a device name gets a
/// "_".
fn stem(name: &str) -> String {
    let fitted: String = name
        .trim()
        .chars()
        .map(|c| {
            if c < ' ' || r#"\/:*?"<>|"#.contains(c) {
                '_'
            } else {
                c
            }
        })
        .take(MAX_STEM)
        .collect();
    // Windows drops dots and spaces at the end of a file name.
    let fitted = fitted.trim_end_matches(['.', ' ']);
    if fitted.is_empty() {
        return "profile".into();
    }
    let (base, rest) = fitted.split_at(fitted.find('.').unwrap_or(fitted.len()));
    if DEVICE_NAMES
        .iter()
        .any(|device| device.eq_ignore_ascii_case(base.trim_end()))
    {
        format!("{base}_{rest}")
    } else {
        fitted.to_string()
    }
}

/// The first of "<stem>.db", "<stem> (2).db", … in `dir` that no other file takes; the
/// file `current` may keep its own name.
fn free_path(dir: &Path, stem: &str, current: Option<&Path>) -> PathBuf {
    (1..)
        .map(|n| match n {
            1 => dir.join(format!("{stem}.db")),
            n => dir.join(format!("{stem} ({n}).db")),
        })
        .find(|path| same_file_name(path, current) || (!path.exists() && !journal(path).exists()))
        .expect("some number is free")
}

fn same_file_name(path: &Path, other: Option<&Path>) -> bool {
    let name = |p: &Path| p.file_name().map(|n| n.to_string_lossy().into_owned());
    match (name(path), other.and_then(name)) {
        (Some(a), Some(b)) => eq_ignore_case(&a, &b),
        _ => false,
    }
}

/// The rollback journal SQLite keeps next to a database during a write.
fn journal(path: &Path) -> PathBuf {
    let mut journal = path.as_os_str().to_os_string();
    journal.push("-journal");
    PathBuf::from(journal)
}

#[cfg(test)]
mod tests;
