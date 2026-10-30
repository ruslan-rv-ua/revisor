//! Snapshot storage: one SQLite database per profile (ADR-0002).
//!
//! A snapshot is a tree of entries given by sources (ADR-0003). The common part of an
//! entry — position in the tree, name, kind — lives in `entry`; attributes that only
//! make sense for one source live in that source's table, now only `fs_entry`.

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::types::{FromSqlError, ToSqlOutput, Value, ValueRef};
use rusqlite::{Connection, params};

use crate::scan::{Entry, EntryKind, FileId};

/// Version of the database schema, kept in `PRAGMA user_version`.
pub const SCHEMA_VERSION: i64 = 1;

/// `entry.source` of the file system.
const SOURCE_FS: i64 = 1;
const KIND_CONTAINER: i64 = 0;
const KIND_ITEM: i64 = 1;
/// How many entries are written between checks for cancellation.
const CANCEL_CHECK: usize = 4096;

const SCHEMA: &str = "
CREATE TABLE snapshot (
    -- AUTOINCREMENT: ids of deleted snapshots are never given out again.
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    -- Unix time in milliseconds.
    taken_at INTEGER NOT NULL,
    comment TEXT
);
-- One node of a snapshot tree; the same for every source.
CREATE TABLE entry (
    snapshot_id INTEGER NOT NULL,
    -- 1 = file system.
    source INTEGER NOT NULL,
    -- Position in the source's tree; the root is 0.
    idx INTEGER NOT NULL,
    -- idx of the parent; NULL for the root.
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
    -- Four 16-bit parts packed into one integer, most significant first.
    version INTEGER,
    -- NOT NULL marks a link; '' when its target is unknown. TEXT or BLOB like entry.name.
    link_target,
    PRIMARY KEY (snapshot_id, idx)
) WITHOUT ROWID;
";

#[derive(Debug)]
pub enum StoreError {
    Sqlite(rusqlite::Error),
    /// The database was written by a newer revisor.
    NewerSchema(i64),
    Cancelled,
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Sqlite(e) => write!(f, "database error: {e}"),
            StoreError::NewerSchema(v) => write!(
                f,
                "the database has schema version {v}, newer than {SCHEMA_VERSION}"
            ),
            StoreError::Cancelled => f.write_str("saving cancelled"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> Self {
        StoreError::Sqlite(e)
    }
}

/// A saved snapshot, without its entries.
#[derive(Clone, Debug)]
pub struct SnapshotInfo {
    pub id: i64,
    /// When the snapshot was saved, as Unix time in milliseconds.
    pub taken_at: i64,
    pub comment: Option<String>,
}

/// Snapshots of one profile.
pub struct Store {
    conn: Connection,
}

impl Store {
    /// Opens the database at `path`, creating it if it does not exist.
    pub fn open(path: &Path) -> Result<Store, StoreError> {
        let mut conn = Connection::open(path)?;
        let tx = conn.transaction()?;
        let version: i64 = tx.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version > SCHEMA_VERSION {
            return Err(StoreError::NewerSchema(version));
        }
        if version == 0 {
            tx.execute_batch(SCHEMA)?;
            tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        }
        tx.commit()?;
        Ok(Store { conn })
    }

    /// Saves a file system snapshot in one transaction and returns its id. `entries` is a
    /// tree laid out like [`crate::scan::Scan::entries`]. Setting `cancel` stops with
    /// [`StoreError::Cancelled`] and leaves nothing of the snapshot behind.
    pub fn save(
        &mut self,
        comment: Option<&str>,
        entries: &[Entry],
        cancel: &AtomicBool,
    ) -> Result<i64, StoreError> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO snapshot (taken_at, comment) VALUES (?1, ?2)",
            params![now_millis(), comment],
        )?;
        let id = tx.last_insert_rowid();
        {
            let mut insert_entry =
                tx.prepare("INSERT INTO entry VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)")?;
            let mut insert_fs =
                tx.prepare("INSERT INTO fs_entry VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)")?;
            for (idx, e) in entries.iter().enumerate() {
                if idx % CANCEL_CHECK == 0 && cancel.load(Ordering::Relaxed) {
                    return Err(StoreError::Cancelled);
                }
                let kind = match e.kind {
                    EntryKind::Dir => KIND_CONTAINER,
                    EntryKind::File | EntryKind::Link => KIND_ITEM,
                };
                insert_entry.execute(params![
                    id,
                    SOURCE_FS,
                    idx as i64,
                    e.parent,
                    os_to_sql(&e.name),
                    kind,
                    e.inaccessible,
                ])?;
                let link_target = (e.kind == EntryKind::Link)
                    .then(|| os_to_sql(e.link_target.as_deref().unwrap_or_default()));
                insert_fs.execute(params![
                    id,
                    idx as i64,
                    e.attributes,
                    e.size as i64,
                    e.created,
                    e.modified,
                    &e.file_id.0[..],
                    e.version.map(pack_version),
                    link_target,
                ])?;
            }
        }
        tx.commit()?;
        Ok(id)
    }

    /// Loads the entries of a file system snapshot, in the order they were saved. A
    /// snapshot that does not exist has no entries.
    pub fn load(&self, id: i64) -> Result<Vec<Entry>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT e.parent, e.name, e.kind, e.inaccessible, f.attributes, f.size, f.created,
                    f.modified, f.file_id, f.version, f.link_target
             FROM entry e JOIN fs_entry f ON f.snapshot_id = e.snapshot_id AND f.idx = e.idx
             WHERE e.snapshot_id = ?1 AND e.source = ?2
             ORDER BY e.idx",
        )?;
        let rows = stmt.query_map(params![id, SOURCE_FS], |r| {
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
                parent: r.get(0)?,
                name: os_from_sql(r.get_ref(1)?)?.into_boxed_os_str(),
                kind,
                inaccessible: r.get(3)?,
                attributes: r.get(4)?,
                size: r.get::<_, i64>(5)? as u64,
                created: r.get(6)?,
                modified: r.get(7)?,
                file_id: FileId(file_id),
                version: r.get::<_, Option<i64>>(9)?.map(unpack_version),
                link_target: link_target
                    .filter(|t| !t.is_empty())
                    .map(OsString::into_boxed_os_str),
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Deletes a snapshot with all its entries. The freed space stays in the database
    /// file until [`Store::vacuum`].
    pub fn delete(&mut self, id: i64) -> Result<(), StoreError> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM fs_entry WHERE snapshot_id = ?1", [id])?;
        tx.execute("DELETE FROM entry WHERE snapshot_id = ?1", [id])?;
        tx.execute("DELETE FROM snapshot WHERE id = ?1", [id])?;
        tx.commit()?;
        Ok(())
    }

    /// Rebuilds the database file, returning the space of deleted snapshots to the disk.
    pub fn vacuum(&self) -> Result<(), StoreError> {
        self.conn.execute_batch("VACUUM")?;
        Ok(())
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
}

fn now_millis() -> i64 {
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

fn pack_version(v: [u16; 4]) -> i64 {
    v.iter()
        .fold(0u64, |acc, &part| acc << 16 | u64::from(part)) as i64
}

fn unpack_version(packed: i64) -> [u16; 4] {
    let packed = packed as u64;
    [48, 32, 16, 0].map(|shift| (packed >> shift) as u16)
}

#[cfg(test)]
mod tests;
