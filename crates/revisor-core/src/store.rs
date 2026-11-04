//! Profile storage: one SQLite database per profile holds its settings and its snapshots
//! (ADR-0002, ADR-0006).
//!
//! A snapshot is a tree of entries given by sources (ADR-0003). The file system gives one
//! tree per folder of the profile, each with its own root, and `snapshot_folder` says where
//! each tree is in `entry` or why there is none (TASK-8). The common part of an entry —
//! position in the tree, name, kind — lives in `entry`; attributes that only make sense
//! for one source live in that source's table, now only `fs_entry`.

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs::File;
use std::io;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::types::{FromSqlError, ToSqlOutput, Value, ValueRef};
use rusqlite::{Connection, ErrorCode, OpenFlags, OptionalExtension, Transaction, params};

use crate::exclude::DEFAULT_EXCLUSIONS;
use crate::scan::{Entry, EntryKind, FileId, Version, VersionInfo};

/// Version of the database schema, kept in `PRAGMA user_version`. Until 0.1.0 is released
/// the schema changes in place, without migrations: there are no users' databases yet.
pub const SCHEMA_VERSION: i64 = 1;
/// `PRAGMA application_id` of a profile database: "RVPR", a revisor profile.
const APPLICATION_ID: i32 = 0x5256_5052;

/// `entry.source` of the file system.
const SOURCE_FS: i64 = 1;
const KIND_CONTAINER: i64 = 0;
const KIND_ITEM: i64 = 1;
/// `snapshot_folder.state` values.
const STATE_SCANNED: i64 = 0;
const STATE_MISSING: i64 = 1;
const STATE_NOT_NTFS: i64 = 2;
const STATE_NETWORK: i64 = 3;
const STATE_ERROR: i64 = 4;
const STATE_NOT_A_FOLDER: i64 = 5;
/// How many entries are written between checks for cancellation.
const CANCEL_CHECK: usize = 4096;
/// How long a connection waits for another one's write to end before giving up: a
/// snapshot's commit takes seconds, and a compaction (TASK-9) closes the database for
/// longer, so whoever needs the database then waits in its own way.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

const SCHEMA: &str = "
-- The profile's settings (ADR-0006): one row.
CREATE TABLE profile (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    name TEXT NOT NULL
);
-- The profile's folders, in the order they were added.
CREATE TABLE profile_folder (
    position INTEGER PRIMARY KEY,
    -- TEXT, or BLOB of raw UTF-16LE when the path is not valid Unicode.
    path NOT NULL
);
CREATE TABLE profile_exclusion (
    position INTEGER PRIMARY KEY,
    pattern TEXT NOT NULL
);
-- The folders each snapshot was taken with, and what the snapshot holds for each; later
-- changes of the profile leave them as they were.
CREATE TABLE snapshot_folder (
    snapshot_id INTEGER NOT NULL,
    position INTEGER NOT NULL,
    path NOT NULL,
    -- 0 = scanned; 1 = missing, though its volume was there; 2 = on a volume other than
    -- NTFS; 3 = on a network share; 4 = could not be reached (a Windows error); 5 = a
    -- file stood at its path.
    state INTEGER NOT NULL,
    -- The file system's name for state 2, the Windows error code for state 4.
    detail,
    -- For a scanned folder: the serial number of its volume, the idx of its root entry,
    -- how many entries its tree has, and where it really was (TEXT or BLOB like path):
    -- the location path exclusions were matched from, which a comparison needs too.
    volume_serial INTEGER,
    root INTEGER,
    entries INTEGER,
    real_path,
    PRIMARY KEY (snapshot_id, position)
) WITHOUT ROWID;
CREATE TABLE snapshot_exclusion (
    snapshot_id INTEGER NOT NULL,
    position INTEGER NOT NULL,
    pattern TEXT NOT NULL,
    -- The pattern as it applied: its variable expanded and the existing part of its path
    -- at its real location; NULL when it did not apply (its variable was not set).
    applied TEXT,
    PRIMARY KEY (snapshot_id, position)
) WITHOUT ROWID;
CREATE TABLE snapshot (
    -- AUTOINCREMENT: ids of deleted snapshots are never given out again.
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    -- When the snapshot was started, as Unix time in milliseconds.
    taken_at INTEGER NOT NULL,
    comment TEXT
);
-- One node of a snapshot tree; the same for every source.
CREATE TABLE entry (
    snapshot_id INTEGER NOT NULL,
    -- 1 = file system.
    source INTEGER NOT NULL,
    -- Position in the source's trees; the root of each tree is at snapshot_folder.root.
    idx INTEGER NOT NULL,
    -- idx of the parent; NULL for a root.
    parent INTEGER,
    -- TEXT, or BLOB of raw UTF-16LE when the name is not valid Unicode.
    name NOT NULL,
    -- 0 = container (folder, key), 1 = item (file, value).
    kind INTEGER NOT NULL,
    inaccessible INTEGER NOT NULL,
    PRIMARY KEY (snapshot_id, source, idx)
) WITHOUT ROWID;
-- File system attributes of an entry of source 1.
CREATE TABLE fs_entry (
    snapshot_id INTEGER NOT NULL,
    idx INTEGER NOT NULL,
    attributes INTEGER NOT NULL,
    size INTEGER NOT NULL,
    created INTEGER NOT NULL,
    modified INTEGER NOT NULL,
    file_id BLOB NOT NULL,
    -- NULL: the version was not read. Empty: read, and there is no version resource.
    -- 16 bytes: the file version, then the product version, four 16-bit parts each,
    -- most significant first.
    versions BLOB,
    -- NOT NULL marks a link; '' when its target is unknown. TEXT or BLOB like entry.name.
    link_target,
    PRIMARY KEY (snapshot_id, idx)
) WITHOUT ROWID;
";

#[derive(Debug)]
pub enum StoreError {
    Sqlite(rusqlite::Error),
    /// The database file could not be created.
    Io(io::Error),
    /// The database was written by a newer revisor.
    NewerSchema(i64),
    /// The file is not a revisor profile.
    NotAProfile,
    /// A save was cancelled, or a compaction interrupted; nothing has changed.
    Cancelled,
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Sqlite(e) => write!(f, "database error: {e}"),
            StoreError::Io(e) => write!(f, "cannot create the database: {e}"),
            StoreError::NewerSchema(v) => write!(
                f,
                "the database has schema version {v}, newer than {SCHEMA_VERSION}"
            ),
            StoreError::NotAProfile => f.write_str("the file is not a revisor profile"),
            StoreError::Cancelled => f.write_str("cancelled"),
        }
    }
}

impl std::error::Error for StoreError {}

impl StoreError {
    /// Whether the database is busy with another connection's write, such as another
    /// revisor compacting it: a reader may then wait and try again (TASK-9).
    pub fn is_busy(&self) -> bool {
        match self {
            StoreError::Sqlite(e) => matches!(
                e.sqlite_error_code(),
                Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked)
            ),
            _ => false,
        }
    }
}

impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> Self {
        StoreError::Sqlite(e)
    }
}

/// A saved snapshot, without its entries.
#[derive(Clone, Debug)]
pub struct SnapshotInfo {
    pub id: i64,
    /// When the snapshot was started, as Unix time in milliseconds.
    pub taken_at: i64,
    pub comment: Option<String>,
}

/// What a snapshot covers: the profile's folders and exclusions it was taken with.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Scope {
    pub folders: Vec<PathBuf>,
    /// Exclusion patterns as written (see [`crate::exclude`]).
    pub exclusions: Vec<String>,
}

/// The settings of a profile, kept in its database (ADR-0006).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileSettings {
    pub name: String,
    pub scope: Scope,
}

impl ProfileSettings {
    /// What a new profile starts with: no name and no folders yet, and the default
    /// exclusions as its own, which it can edit.
    pub fn new_profile() -> Self {
        Self {
            name: String::new(),
            scope: Scope {
                folders: Vec::new(),
                exclusions: DEFAULT_EXCLUSIONS.iter().map(|p| p.to_string()).collect(),
            },
        }
    }
}

/// Why a folder of the profile could not be read for a snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unavailable {
    /// The folder is on a volume other than NTFS; holds the file system's name.
    NotNtfs(String),
    /// The folder is on a network share.
    Network,
    /// The folder or its volume could not be reached: a Windows error code.
    Error(i32),
    /// A file stands at the folder's path.
    NotAFolder,
}

/// What a snapshot holds for one folder of the profile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FolderState {
    /// The folder was scanned: its tree is in the snapshot, and the file IDs of its
    /// entries are unique within the volume with this serial number.
    Scanned {
        volume_serial: u64,
        /// Where the folder really was, as [`crate::scan::Scan::real_root`]: path
        /// exclusions were matched from there, and a comparison matches them from there
        /// too.
        real_path: PathBuf,
    },
    /// The folder was not on the disk, though its volume was. A comparison takes it as
    /// empty, so what it used to hold shows as deleted.
    Missing,
    /// The folder could not be read, for this reason. A comparison knows nothing about
    /// what it holds.
    Unavailable(Unavailable),
}

/// A folder of a saved snapshot: the profile's folder and what the snapshot holds for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotFolder {
    pub path: PathBuf,
    pub state: FolderState,
}

impl From<&FolderSnapshot> for SnapshotFolder {
    fn from(folder: &FolderSnapshot) -> Self {
        SnapshotFolder {
            path: folder.path.clone(),
            state: folder.state.clone(),
        }
    }
}

/// An exclusion a snapshot was taken with: the pattern as written, and as it applied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotExclusion {
    pub pattern: String,
    /// The pattern as it applied (see [`crate::exclude::Expanded`]), or `None` when it did
    /// not apply.
    pub applied: Option<String>,
}

/// A folder of the profile as its scan collected it, to be saved.
#[derive(Debug)]
pub struct FolderSnapshot {
    pub path: PathBuf,
    pub state: FolderState,
    /// The tree, laid out like [`crate::scan::Scan::entries`]; empty unless scanned.
    pub entries: Vec<Entry>,
}

/// A snapshot to save: what the scans of the profile's folders collected.
#[derive(Debug)]
pub struct Snapshot {
    /// When the snapshot was started, as Unix time in milliseconds.
    pub taken_at: i64,
    pub comment: Option<String>,
    pub folders: Vec<FolderSnapshot>,
    pub exclusions: Vec<SnapshotExclusion>,
}

impl Snapshot {
    /// A snapshot of one scanned tree, whose root names the folder, with no comment and
    /// no exclusions, started now; for tests and benchmarks.
    pub fn of_tree(entries: Vec<Entry>) -> Snapshot {
        let folders = match entries.first() {
            Some(root) => {
                let path = PathBuf::from(&*root.name);
                vec![FolderSnapshot {
                    path: path.clone(),
                    state: FolderState::Scanned {
                        volume_serial: 0,
                        real_path: path,
                    },
                    entries,
                }]
            }
            None => Vec::new(),
        };
        Snapshot {
            taken_at: now_millis(),
            comment: None,
            folders,
            exclusions: Vec::new(),
        }
    }
}

/// One profile: its settings and its snapshots.
pub struct Store {
    conn: Connection,
    path: PathBuf,
}

/// Interrupts what a [`Store`] is doing on another thread, such as a compaction; the
/// interrupted call ends with [`StoreError::Cancelled`] and leaves the database as it was.
pub struct Interrupt(rusqlite::InterruptHandle);

impl Interrupt {
    pub fn interrupt(&self) {
        self.0.interrupt();
    }
}

/// What a compaction did: the size of the database file before and after it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Compacted {
    pub before: u64,
    pub after: u64,
}

impl Store {
    /// Creates the database of a new profile at `path`. An existing file is never taken
    /// over, and a database that could not be made is not left behind.
    pub fn create(path: &Path, settings: &ProfileSettings) -> Result<Store, StoreError> {
        File::create_new(path).map_err(StoreError::Io)?;
        let made = Connection::open(path)
            .map_err(StoreError::from)
            .and_then(|mut conn| {
                let tx = conn.transaction()?;
                tx.execute_batch(SCHEMA)?;
                tx.pragma_update(None, "application_id", APPLICATION_ID)?;
                tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
                tx.execute(
                    "INSERT INTO profile (id, name) VALUES (1, ?1)",
                    [&settings.name],
                )?;
                write_scope(&tx, settings)?;
                tx.commit()?;
                Ok(conn)
            });
        match made {
            Ok(conn) => Ok(Store {
                conn,
                path: path.to_path_buf(),
            }),
            Err(e) => {
                let _ = std::fs::remove_file(path);
                Err(e)
            }
        }
    }

    /// Opens the database of an existing profile. While another connection writes to it,
    /// such as a snapshot being saved on another thread, reads and writes wait for it
    /// rather than fail at once.
    pub fn open(path: &Path) -> Result<Store, StoreError> {
        Self::open_with_timeout(path, BUSY_TIMEOUT)
    }

    /// Opens the database of an existing profile, waiting up to `busy_timeout` for another
    /// connection's write to end: a comment written while a snapshot is being saved waits
    /// for the save (TASK-9), which is a matter of seconds.
    pub fn open_with_timeout(path: &Path, busy_timeout: Duration) -> Result<Store, StoreError> {
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        conn.busy_timeout(busy_timeout)?;
        // A file that is not a database at all fails here, when it is first read. Other
        // errors stay errors: a profile another revisor is writing is still a profile.
        let app: i32 = conn
            .pragma_query_value(None, "application_id", |r| r.get(0))
            .map_err(|e| match e.sqlite_error_code() {
                Some(ErrorCode::NotADatabase) => StoreError::NotAProfile,
                _ => StoreError::Sqlite(e),
            })?;
        if app != APPLICATION_ID {
            return Err(StoreError::NotAProfile);
        }
        let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version > SCHEMA_VERSION {
            return Err(StoreError::NewerSchema(version));
        }
        Ok(Store {
            conn,
            path: path.to_path_buf(),
        })
    }

    /// The profile's settings. A database without its profile row is not a profile.
    pub fn settings(&self) -> Result<ProfileSettings, StoreError> {
        let name = self
            .conn
            .query_row("SELECT name FROM profile WHERE id = 1", [], |r| r.get(0))
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => StoreError::NotAProfile,
                e => StoreError::Sqlite(e),
            })?;
        let mut stmt = self
            .conn
            .prepare("SELECT path FROM profile_folder ORDER BY position")?;
        let folders = stmt
            .query_map([], |r| Ok(PathBuf::from(os_from_sql(r.get_ref(0)?)?)))?
            .collect::<Result<_, _>>()?;
        let mut stmt = self
            .conn
            .prepare("SELECT pattern FROM profile_exclusion ORDER BY position")?;
        let exclusions = stmt
            .query_map([], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(ProfileSettings {
            name,
            scope: Scope {
                folders,
                exclusions,
            },
        })
    }

    /// Replaces the profile's settings in one transaction.
    pub fn set_settings(&mut self, settings: &ProfileSettings) -> Result<(), StoreError> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "UPDATE profile SET name = ?1 WHERE id = 1",
            [&settings.name],
        )?;
        tx.execute("DELETE FROM profile_folder", [])?;
        tx.execute("DELETE FROM profile_exclusion", [])?;
        write_scope(&tx, settings)?;
        tx.commit()?;
        Ok(())
    }

    /// The folders and exclusions a snapshot was taken with, as written; empty for a
    /// snapshot that does not exist.
    pub fn snapshot_scope(&self, id: i64) -> Result<Scope, StoreError> {
        let folders = self
            .snapshot_folders(id)?
            .into_iter()
            .map(|f| f.path)
            .collect();
        let exclusions = self
            .snapshot_exclusions(id)?
            .into_iter()
            .map(|e| e.pattern)
            .collect();
        Ok(Scope {
            folders,
            exclusions,
        })
    }

    /// The folders of a snapshot with what it holds for each, in the profile's order;
    /// empty for a snapshot that does not exist.
    pub fn snapshot_folders(&self, id: i64) -> Result<Vec<SnapshotFolder>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT path, state, detail, volume_serial, real_path FROM snapshot_folder
             WHERE snapshot_id = ?1 ORDER BY position",
        )?;
        let rows = stmt.query_map([id], |r| {
            let state = match r.get::<_, i64>(1)? {
                STATE_SCANNED => FolderState::Scanned {
                    volume_serial: r.get::<_, i64>(3)? as u64,
                    real_path: PathBuf::from(os_from_sql(r.get_ref(4)?)?),
                },
                STATE_MISSING => FolderState::Missing,
                STATE_NOT_NTFS => FolderState::Unavailable(Unavailable::NotNtfs(r.get(2)?)),
                STATE_NETWORK => FolderState::Unavailable(Unavailable::Network),
                STATE_ERROR => FolderState::Unavailable(Unavailable::Error(r.get(2)?)),
                STATE_NOT_A_FOLDER => FolderState::Unavailable(Unavailable::NotAFolder),
                other => {
                    return Err(rusqlite::Error::FromSqlConversionFailure(
                        1,
                        rusqlite::types::Type::Integer,
                        format!("unknown folder state {other}").into(),
                    ));
                }
            };
            Ok(SnapshotFolder {
                path: PathBuf::from(os_from_sql(r.get_ref(0)?)?),
                state,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// A saved snapshot by its id, without its entries; `None` when there is no such
    /// snapshot: it was deleted, or never existed.
    pub fn snapshot(&self, id: i64) -> Result<Option<SnapshotInfo>, StoreError> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, taken_at, comment FROM snapshot WHERE id = ?1",
                [id],
                |r| {
                    Ok(SnapshotInfo {
                        id: r.get(0)?,
                        taken_at: r.get(1)?,
                        comment: r.get(2)?,
                    })
                },
            )
            .optional()?)
    }

    /// The exclusions a snapshot was taken with, as written and as they applied.
    pub fn snapshot_exclusions(&self, id: i64) -> Result<Vec<SnapshotExclusion>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT pattern, applied FROM snapshot_exclusion WHERE snapshot_id = ?1
             ORDER BY position",
        )?;
        let rows = stmt.query_map([id], |r| {
            Ok(SnapshotExclusion {
                pattern: r.get(0)?,
                applied: r.get(1)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Saves a snapshot in one transaction and returns its id. Setting `cancel` stops
    /// with [`StoreError::Cancelled`] and leaves nothing of the snapshot behind.
    pub fn save(&mut self, snapshot: &Snapshot, cancel: &AtomicBool) -> Result<i64, StoreError> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO snapshot (taken_at, comment) VALUES (?1, ?2)",
            params![snapshot.taken_at, snapshot.comment],
        )?;
        let id = tx.last_insert_rowid();
        {
            let mut insert_folder = tx.prepare(
                "INSERT INTO snapshot_folder VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            )?;
            let mut insert_entry =
                tx.prepare("INSERT INTO entry VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)")?;
            let mut insert_fs =
                tx.prepare("INSERT INTO fs_entry VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)")?;
            let mut base = 0usize;
            let mut written = 0usize;
            for (position, folder) in snapshot.folders.iter().enumerate() {
                let (state, detail, scanned) = match &folder.state {
                    FolderState::Scanned { volume_serial, .. } => (
                        STATE_SCANNED,
                        Value::Null,
                        Some((
                            *volume_serial as i64,
                            base as i64,
                            folder.entries.len() as i64,
                        )),
                    ),
                    FolderState::Missing => (STATE_MISSING, Value::Null, None),
                    FolderState::Unavailable(Unavailable::NotNtfs(fs)) => {
                        (STATE_NOT_NTFS, Value::Text(fs.clone()), None)
                    }
                    FolderState::Unavailable(Unavailable::Network) => {
                        (STATE_NETWORK, Value::Null, None)
                    }
                    FolderState::Unavailable(Unavailable::Error(code)) => {
                        (STATE_ERROR, Value::Integer(i64::from(*code)), None)
                    }
                    FolderState::Unavailable(Unavailable::NotAFolder) => {
                        (STATE_NOT_A_FOLDER, Value::Null, None)
                    }
                };
                let (volume_serial, root, count) = match scanned {
                    Some((serial, root, count)) => (Some(serial), Some(root), Some(count)),
                    None => (None, None, None),
                };
                let real_path = match &folder.state {
                    FolderState::Scanned { real_path, .. } => {
                        Some(os_to_sql(real_path.as_os_str()))
                    }
                    _ => None,
                };
                insert_folder.execute(params![
                    id,
                    position as i64,
                    os_to_sql(folder.path.as_os_str()),
                    state,
                    detail,
                    volume_serial,
                    root,
                    count,
                    real_path,
                ])?;
                if scanned.is_none() {
                    continue;
                }
                for (i, e) in folder.entries.iter().enumerate() {
                    if written.is_multiple_of(CANCEL_CHECK) && cancel.load(Ordering::Relaxed) {
                        return Err(StoreError::Cancelled);
                    }
                    written += 1;
                    let idx = (base + i) as i64;
                    let kind = match e.kind {
                        EntryKind::Dir => KIND_CONTAINER,
                        EntryKind::File | EntryKind::Link => KIND_ITEM,
                    };
                    insert_entry.execute(params![
                        id,
                        SOURCE_FS,
                        idx,
                        e.parent.map(|p| p as i64 + base as i64),
                        os_to_sql(&e.name),
                        kind,
                        e.inaccessible,
                    ])?;
                    let link_target = (e.kind == EntryKind::Link)
                        .then(|| os_to_sql(e.link_target.as_deref().unwrap_or_default()));
                    insert_fs.execute(params![
                        id,
                        idx,
                        e.attributes,
                        e.size as i64,
                        e.created,
                        e.modified,
                        &e.file_id.0[..],
                        versions_to_sql(e.version),
                        link_target,
                    ])?;
                }
                base += folder.entries.len();
            }
        }
        for (position, exclusion) in snapshot.exclusions.iter().enumerate() {
            tx.execute(
                "INSERT INTO snapshot_exclusion VALUES (?1, ?2, ?3, ?4)",
                params![id, position as i64, exclusion.pattern, exclusion.applied],
            )?;
        }
        tx.commit()?;
        Ok(id)
    }

    /// Loads the tree of the folder at `position` of a snapshot, laid out like
    /// [`crate::scan::Scan::entries`]: the root first, with parents as indices into the
    /// result. Empty for a folder that was not scanned, or a snapshot that does not exist.
    /// Setting `cancel` stops with [`StoreError::Cancelled`]: loading is the longer part
    /// of a comparison.
    pub fn load_folder(
        &self,
        id: i64,
        position: usize,
        cancel: &AtomicBool,
    ) -> Result<Vec<Entry>, StoreError> {
        let range: Option<(i64, i64)> = self
            .conn
            .query_row(
                "SELECT root, entries FROM snapshot_folder WHERE snapshot_id = ?1 AND position = ?2",
                params![id, position as i64],
                |r| Ok(r.get::<_, Option<i64>>(0)?.zip(r.get::<_, Option<i64>>(1)?)),
            )
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                e => Err(e),
            })?;
        let Some((root, count)) = range else {
            return Ok(Vec::new());
        };
        let mut stmt = self.conn.prepare(
            "SELECT e.parent, e.name, e.kind, e.inaccessible, f.attributes, f.size, f.created,
                    f.modified, f.file_id, f.versions, f.link_target
             FROM entry e JOIN fs_entry f ON f.snapshot_id = e.snapshot_id AND f.idx = e.idx
             WHERE e.snapshot_id = ?1 AND e.source = ?2 AND e.idx >= ?3 AND e.idx < ?4
             ORDER BY e.idx",
        )?;
        let rows = stmt.query_map(params![id, SOURCE_FS, root, root + count], |r| {
            let link_target = match r.get_ref(10)? {
                ValueRef::Null => None,
                v => Some(os_from_sql(v)?),
            };
            let kind = if link_target.is_some() {
                EntryKind::Link
            } else if r.get::<_, i64>(2)? == KIND_CONTAINER {
                EntryKind::Dir
            } else {
                EntryKind::File
            };
            let file_id: [u8; 16] = r
                .get_ref(8)?
                .as_blob()?
                .try_into()
                .map_err(|e| FromSqlError::Other(Box::new(e)))?;
            Ok(Entry {
                parent: r.get::<_, Option<i64>>(0)?.map(|p| (p - root) as u32),
                name: os_from_sql(r.get_ref(1)?)?.into_boxed_os_str(),
                kind,
                inaccessible: r.get(3)?,
                attributes: r.get(4)?,
                size: r.get::<_, i64>(5)? as u64,
                created: r.get(6)?,
                modified: r.get(7)?,
                file_id: FileId(file_id),
                version: versions_from_sql(r.get_ref(9)?)?,
                link_target: link_target
                    .filter(|t| !t.is_empty())
                    .map(OsString::into_boxed_os_str),
            })
        })?;
        let mut entries = Vec::with_capacity(count as usize);
        for row in rows {
            if entries.len().is_multiple_of(CANCEL_CHECK) && cancel.load(Ordering::Relaxed) {
                return Err(StoreError::Cancelled);
            }
            entries.push(row?);
        }
        Ok(entries)
    }

    /// Changes the comment of a snapshot, or removes it with `None`. A snapshot that is
    /// gone is not an error: there is nothing left to comment on.
    pub fn set_comment(&mut self, id: i64, comment: Option<&str>) -> Result<(), StoreError> {
        self.conn.execute(
            "UPDATE snapshot SET comment = ?1 WHERE id = ?2",
            params![comment, id],
        )?;
        Ok(())
    }

    /// Deletes a snapshot with all its entries. The freed space stays in the database
    /// file until [`Store::compact`].
    pub fn delete(&mut self, id: i64) -> Result<(), StoreError> {
        self.delete_many(&[id])
    }

    /// Deletes several snapshots with all their entries in one transaction, so either all
    /// of them go or none. Ids of snapshots that are gone already are skipped.
    pub fn delete_many(&mut self, ids: &[i64]) -> Result<(), StoreError> {
        let tx = self.conn.transaction()?;
        for &id in ids {
            tx.execute("DELETE FROM fs_entry WHERE snapshot_id = ?1", [id])?;
            tx.execute("DELETE FROM entry WHERE snapshot_id = ?1", [id])?;
            tx.execute("DELETE FROM snapshot_folder WHERE snapshot_id = ?1", [id])?;
            tx.execute(
                "DELETE FROM snapshot_exclusion WHERE snapshot_id = ?1",
                [id],
            )?;
            tx.execute("DELETE FROM snapshot WHERE id = ?1", [id])?;
        }
        tx.commit()?;
        Ok(())
    }

    /// A handle that interrupts a [`Store::compact`] running on another thread.
    pub fn interrupt(&self) -> Interrupt {
        Interrupt(self.conn.get_interrupt_handle())
    }

    /// Rebuilds the database file (VACUUM), returning the space of deleted snapshots to
    /// the disk; the file is closed to every other connection meanwhile. Interrupted
    /// through [`Store::interrupt`], it ends with [`StoreError::Cancelled`] and leaves the
    /// file as it was.
    pub fn compact(&self) -> Result<Compacted, StoreError> {
        let size = || std::fs::metadata(&self.path).map_or(0, |m| m.len());
        let before = size();
        self.conn
            .execute_batch("VACUUM")
            .map_err(|e| match e.sqlite_error_code() {
                Some(ErrorCode::OperationInterrupted) => StoreError::Cancelled,
                _ => StoreError::Sqlite(e),
            })?;
        Ok(Compacted {
            before,
            after: size(),
        })
    }

    /// Saved snapshots, oldest first.
    pub fn list(&self) -> Result<Vec<SnapshotInfo>, StoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, taken_at, comment FROM snapshot ORDER BY id")?;
        let rows = stmt.query_map([], |r| {
            Ok(SnapshotInfo {
                id: r.get(0)?,
                taken_at: r.get(1)?,
                comment: r.get(2)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// How many snapshots the profile has.
    pub fn snapshot_count(&self) -> Result<usize, StoreError> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM snapshot", [], |r| r.get(0))?;
        Ok(count as usize)
    }
}

/// Writes the profile's folders and exclusions; the tables are empty before.
fn write_scope(tx: &Transaction<'_>, settings: &ProfileSettings) -> Result<(), StoreError> {
    for (position, folder) in settings.scope.folders.iter().enumerate() {
        tx.execute(
            "INSERT INTO profile_folder VALUES (?1, ?2)",
            params![position as i64, os_to_sql(folder.as_os_str())],
        )?;
    }
    for (position, pattern) in settings.scope.exclusions.iter().enumerate() {
        tx.execute(
            "INSERT INTO profile_exclusion VALUES (?1, ?2)",
            params![position as i64, pattern],
        )?;
    }
    Ok(())
}

/// Now, as Unix time in milliseconds: the time a snapshot records.
pub fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

/// A name as TEXT, or as a BLOB of raw UTF-16LE when it is not valid Unicode.
fn os_to_sql(name: &OsStr) -> ToSqlOutput<'_> {
    match name.to_str() {
        Some(s) => ToSqlOutput::Borrowed(ValueRef::Text(s.as_bytes())),
        None => {
            let bytes = name.encode_wide().flat_map(u16::to_le_bytes).collect();
            ToSqlOutput::Owned(Value::Blob(bytes))
        }
    }
}

fn os_from_sql(value: ValueRef<'_>) -> Result<OsString, FromSqlError> {
    match value {
        ValueRef::Text(_) => Ok(value.as_str()?.into()),
        ValueRef::Blob(bytes) if bytes.len() % 2 == 0 => {
            let wide: Vec<u16> = bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|&c| u16::from_le_bytes(c))
                .collect();
            Ok(OsString::from_wide(&wide))
        }
        _ => Err(FromSqlError::InvalidType),
    }
}

/// The `fs_entry.versions` value: NULL, empty, or the two versions in 16 bytes.
fn versions_to_sql(version: VersionInfo) -> Option<Vec<u8>> {
    match version {
        VersionInfo::NotRead => None,
        VersionInfo::None => Some(Vec::new()),
        VersionInfo::Some(v) => Some(
            v.file
                .iter()
                .chain(&v.product)
                .flat_map(|part| part.to_be_bytes())
                .collect(),
        ),
    }
}

fn versions_from_sql(value: ValueRef<'_>) -> Result<VersionInfo, FromSqlError> {
    let parts = |bytes: &[u8]| -> [u16; 4] {
        let mut parts = [0u16; 4];
        for (part, pair) in parts.iter_mut().zip(bytes.as_chunks::<2>().0) {
            *part = u16::from_be_bytes(*pair);
        }
        parts
    };
    match value {
        ValueRef::Null => Ok(VersionInfo::NotRead),
        ValueRef::Blob([]) => Ok(VersionInfo::None),
        ValueRef::Blob(bytes) if bytes.len() == 16 => Ok(VersionInfo::Some(Version {
            file: parts(&bytes[..8]),
            product: parts(&bytes[8..]),
        })),
        _ => Err(FromSqlError::InvalidType),
    }
}

#[cfg(test)]
mod tests;
