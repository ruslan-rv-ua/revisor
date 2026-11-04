//! The application's own settings (ADR-0007): one SQLite database in the data folder, apart
//! from the profiles, for what belongs to no profile — the profile opened last, what the
//! focus does when a snapshot ends, whether deleting snapshots asks first (TASK-9), the
//! interface language (TASK-16). Each
//! setting is one row of a key–value table, read and written through typed methods, so a
//! later setting needs no change of the schema.

use std::fmt;
use std::fs;
use std::io;
use std::path::Path;
use std::time::Duration;

use rusqlite::types::Value;
use rusqlite::{Connection, ErrorCode, OpenFlags, OptionalExtension, params};

/// Version of the settings schema, kept in `PRAGMA user_version`. Until 0.1.0 is released
/// the schema changes in place, without migrations, like the profiles' (brief, section 5).
pub const SCHEMA_VERSION: i64 = 1;
/// `PRAGMA application_id` of the settings database: "RVST", revisor settings; a profile
/// database carries another mark, so the two are never taken for each other.
const APPLICATION_ID: i32 = 0x5256_5354;
/// How long a read or write waits for another revisor's write to end.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

const SCHEMA: &str = "
CREATE TABLE setting (
    key TEXT PRIMARY KEY,
    value
) WITHOUT ROWID;
";

const KEY_LAST_PROFILE: &str = "last_profile";
const KEY_FOCUS_AFTER_SNAPSHOT: &str = "focus_after_snapshot";
const KEY_CONFIRM_SNAPSHOT_DELETION: &str = "confirm_snapshot_deletion";
const KEY_LANGUAGE: &str = "language";

#[derive(Debug)]
pub enum SettingsError {
    Sqlite(rusqlite::Error),
    /// The data folder could not be made.
    Io(io::Error),
    /// The database was written by a newer revisor.
    NewerSchema(i64),
    /// The file is not revisor's settings database.
    NotSettings,
}

impl fmt::Display for SettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SettingsError::Sqlite(e) => write!(f, "database error: {e}"),
            SettingsError::Io(e) => write!(f, "cannot make the data folder: {e}"),
            SettingsError::NewerSchema(v) => write!(
                f,
                "the settings have schema version {v}, newer than {SCHEMA_VERSION}"
            ),
            SettingsError::NotSettings => f.write_str("the file is not a revisor settings file"),
        }
    }
}

impl std::error::Error for SettingsError {}

impl From<rusqlite::Error> for SettingsError {
    fn from(e: rusqlite::Error) -> Self {
        SettingsError::Sqlite(e)
    }
}

/// What happens to the focus when a snapshot taken in the background ends (TASK-9).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FocusAfterSnapshot {
    /// The new snapshot becomes current, and the focus moves to it only when it was in
    /// the snapshot list of its profile or on the snapshot's progress.
    #[default]
    IfInList,
    /// The focus always moves to the new snapshot, opening its profile when needed.
    Always,
    /// The new snapshot only appears at the end of its list.
    Never,
}

impl FocusAfterSnapshot {
    /// Every value, in the order the settings window lists them.
    pub const ALL: [Self; 3] = [Self::IfInList, Self::Always, Self::Never];

    /// The value as the database stores it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::IfInList => "if_in_list",
            Self::Always => "always",
            Self::Never => "never",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.as_str() == text)
    }
}

/// The interface language the user chose (TASK-16). The application maps it to a language
/// of its strings when it starts; a new value takes effect at the next start.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Language {
    /// The interface language of Windows: Ukrainian gives Ukrainian, any other English.
    #[default]
    SameAsWindows,
    English,
    Ukrainian,
}

impl Language {
    /// Every value, in the order the settings window lists them.
    pub const ALL: [Self; 3] = [Self::SameAsWindows, Self::English, Self::Ukrainian];

    /// The value as the database stores it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SameAsWindows => "windows",
            Self::English => "en",
            Self::Ukrainian => "uk",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.as_str() == text)
    }
}

/// The settings database, open for the life of the window.
pub struct Settings {
    conn: Connection,
}

impl Settings {
    /// Opens the settings database at `path`, making it, and its folder, when it is not
    /// there. A file that is not revisor's settings database is never taken over.
    pub fn open(path: &Path) -> Result<Settings, SettingsError> {
        if let Some(folder) = path.parent() {
            fs::create_dir_all(folder).map_err(SettingsError::Io)?;
        }
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        conn.busy_timeout(BUSY_TIMEOUT)?;
        let app: i32 = conn
            .pragma_query_value(None, "application_id", |r| r.get(0))
            .map_err(|e| match e.sqlite_error_code() {
                Some(ErrorCode::NotADatabase) => SettingsError::NotSettings,
                _ => SettingsError::Sqlite(e),
            })?;
        let settings = Settings { conn };
        if app == APPLICATION_ID {
            let version: i64 = settings
                .conn
                .pragma_query_value(None, "user_version", |r| r.get(0))?;
            if version > SCHEMA_VERSION {
                return Err(SettingsError::NewerSchema(version));
            }
            return Ok(settings);
        }
        // Without revisor's mark the file is either brand new and empty, or someone
        // else's database.
        let tables: i64 =
            settings
                .conn
                .query_row("SELECT COUNT(*) FROM sqlite_master", [], |r| r.get(0))?;
        if app != 0 || tables != 0 {
            return Err(SettingsError::NotSettings);
        }
        let conn = &settings.conn;
        conn.execute_batch("BEGIN")?;
        let made = conn
            .execute_batch(SCHEMA)
            .and_then(|()| conn.pragma_update(None, "application_id", APPLICATION_ID))
            .and_then(|()| conn.pragma_update(None, "user_version", SCHEMA_VERSION));
        match made {
            Ok(()) => conn.execute_batch("COMMIT")?,
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK");
                return Err(e.into());
            }
        }
        Ok(settings)
    }

    /// The file name of the profile opened last, in the folder of profiles.
    pub fn last_profile(&self) -> Result<Option<String>, SettingsError> {
        Ok(match self.get(KEY_LAST_PROFILE)? {
            Some(Value::Text(name)) => Some(name),
            _ => None,
        })
    }

    pub fn set_last_profile(&self, file_name: Option<&str>) -> Result<(), SettingsError> {
        match file_name {
            Some(name) => self.set(KEY_LAST_PROFILE, name),
            None => self.remove(KEY_LAST_PROFILE),
        }
    }

    /// What the focus does when a snapshot ends; the default when nothing is stored.
    pub fn focus_after_snapshot(&self) -> Result<FocusAfterSnapshot, SettingsError> {
        Ok(match self.get(KEY_FOCUS_AFTER_SNAPSHOT)? {
            Some(Value::Text(text)) => FocusAfterSnapshot::parse(&text).unwrap_or_default(),
            _ => FocusAfterSnapshot::default(),
        })
    }

    pub fn set_focus_after_snapshot(&self, value: FocusAfterSnapshot) -> Result<(), SettingsError> {
        self.set(KEY_FOCUS_AFTER_SNAPSHOT, value.as_str())
    }

    /// Whether deleting snapshots asks for confirmation first; on by default.
    pub fn confirm_snapshot_deletion(&self) -> Result<bool, SettingsError> {
        Ok(match self.get(KEY_CONFIRM_SNAPSHOT_DELETION)? {
            Some(Value::Integer(n)) => n != 0,
            _ => true,
        })
    }

    pub fn set_confirm_snapshot_deletion(&self, confirm: bool) -> Result<(), SettingsError> {
        self.set(KEY_CONFIRM_SNAPSHOT_DELETION, i64::from(confirm))
    }

    /// The interface language the user chose; «Same as Windows» when nothing is stored.
    pub fn language(&self) -> Result<Language, SettingsError> {
        Ok(match self.get(KEY_LANGUAGE)? {
            Some(Value::Text(text)) => Language::parse(&text).unwrap_or_default(),
            _ => Language::default(),
        })
    }

    pub fn set_language(&self, value: Language) -> Result<(), SettingsError> {
        self.set(KEY_LANGUAGE, value.as_str())
    }

    fn get(&self, key: &str) -> Result<Option<Value>, SettingsError> {
        Ok(self
            .conn
            .query_row("SELECT value FROM setting WHERE key = ?1", [key], |r| {
                r.get(0)
            })
            .optional()?)
    }

    fn set(&self, key: &str, value: impl rusqlite::ToSql) -> Result<(), SettingsError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO setting (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    fn remove(&self, key: &str) -> Result<(), SettingsError> {
        self.conn
            .execute("DELETE FROM setting WHERE key = ?1", [key])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
