//! The application's own settings (ADR-0007): one SQLite database in the data folder, apart
//! from the profiles, for what belongs to no profile — the profile opened last, what the
//! focus does when a snapshot ends, whether deleting snapshots asks first (TASK-9), the
//! interface language (TASK-16), what the focus does when a comparison ends (TASK-11), how
//! sizes are written, which rows the difference list shows in which order (TASK-13), how
//! the details of a change write attributes, whether showing a change in File Explorer
//! warns when it is no longer on disk (TASK-14), whether the difference shows as a list or
//! as a tree, with how much its nodes say (TASK-33), and the theme of the interface
//! (TASK-34).
//! Each setting is one row of a key–value table, read and written through typed methods,
//! so a later setting needs no change of the schema.

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
const KEY_FOCUS_AFTER_COMPARISON: &str = "focus_after_comparison";
const KEY_SIZE_FORMAT: &str = "size_format";
const KEY_DIFFERENCE_TYPES: &str = "difference_types";
const KEY_DIFFERENCE_SORT: &str = "difference_sort";
const KEY_DIFFERENCE_DESCENDING: &str = "difference_descending";
const KEY_ATTRIBUTE_FORMAT: &str = "attribute_format";
const KEY_WARN_MISSING_ENTRY: &str = "warn_missing_entry";
const KEY_DIFFERENCE_VIEW: &str = "difference_view";
const KEY_TREE_NODE_TEXT: &str = "tree_node_text";
const KEY_THEME: &str = "theme";

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

/// What happens to the focus when a comparison made in the background ends (TASK-11).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FocusAfterComparison {
    /// The focus moves to the difference only when it is still where it was when the
    /// comparison started, or on the comparison's progress.
    #[default]
    IfNotMoved,
    /// The focus always moves to the difference.
    Always,
    /// The difference only replaces the one shown.
    Never,
}

impl FocusAfterComparison {
    /// Every value, in the order the settings window lists them.
    pub const ALL: [Self; 3] = [Self::IfNotMoved, Self::Always, Self::Never];

    /// The value as the database stores it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::IfNotMoved => "if_not_moved",
            Self::Always => "always",
            Self::Never => "never",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.as_str() == text)
    }
}

/// How sizes are written in the difference list and its summary (TASK-13 #17).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SizeFormat {
    /// As File Explorer writes them: three significant digits of KB, MB, GB and so on, of
    /// 1024 bytes each, so the numbers agree with those of Windows.
    #[default]
    Explorer,
    /// The same with units of 1000 bytes.
    Decimal,
    /// In bytes, with the digits grouped.
    Bytes,
}

impl SizeFormat {
    /// Every value, in the order the settings window lists them.
    pub const ALL: [Self; 3] = [Self::Explorer, Self::Decimal, Self::Bytes];

    /// The value as the database stores it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Explorer => "explorer",
            Self::Decimal => "decimal",
            Self::Bytes => "bytes",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.as_str() == text)
    }
}

/// How the details of a change write the attributes of an entry (TASK-14 #7): in words, or
/// in the letters of the column «Атрибути» of File Explorer, run together as it writes
/// them or split by commas (decided 2026-11-06).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AttributeFormat {
    /// In words, "read-only, hidden", which is clearer to the ear.
    #[default]
    Words,
    /// In File Explorer's letters, run together as its column writes them: "RH".
    Letters,
    /// The same letters split by commas, so a screen reader says each: "R, H".
    LettersWithCommas,
}

impl AttributeFormat {
    /// Every value, in the order the settings window lists them.
    pub const ALL: [Self; 3] = [Self::Words, Self::Letters, Self::LettersWithCommas];

    /// The value as the database stores it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Words => "words",
            Self::Letters => "letters",
            Self::LettersWithCommas => "letters_with_commas",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.as_str() == text)
    }
}

/// A type of the rows of the difference list, by which its filter shows them (TASK-13
/// #22), in the order of the filter's check boxes: added, deleted and modified entries,
/// replaced ones among the modified, and inaccessible entries, which are not changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChangeType {
    Added,
    Deleted,
    Modified,
    Inaccessible,
}

impl ChangeType {
    /// Every type, in the order of the check boxes.
    pub const ALL: [Self; 4] = [
        Self::Added,
        Self::Deleted,
        Self::Modified,
        Self::Inaccessible,
    ];

    /// The type as the database stores it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Deleted => "deleted",
            Self::Modified => "modified",
            Self::Inaccessible => "inaccessible",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.as_str() == text)
    }
}

/// What the difference list is sorted by (TASK-13 #21): the path, as a tree, which is the
/// default, or one of its columns.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SortColumn {
    #[default]
    Path,
    Name,
    Change,
    WhatChanged,
    Size,
    DateModified,
    Folder,
}

impl SortColumn {
    /// Every value, in the order of the menu «Сортувати за».
    pub const ALL: [Self; 7] = [
        Self::Path,
        Self::Name,
        Self::Change,
        Self::WhatChanged,
        Self::Size,
        Self::DateModified,
        Self::Folder,
    ];

    /// The value as the database stores it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Path => "path",
            Self::Name => "name",
            Self::Change => "change",
            Self::WhatChanged => "what_changed",
            Self::Size => "size",
            Self::DateModified => "date_modified",
            Self::Folder => "folder",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.as_str() == text)
    }
}

/// The order of the difference list (TASK-13 #21): what it is sorted by, and which way.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DifferenceSort {
    pub column: SortColumn,
    pub descending: bool,
}

/// How the difference is shown (TASK-33): as the list of TASK-13, or as a tree of its
/// folders in the same place.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DifferenceView {
    #[default]
    List,
    Tree,
}

impl DifferenceView {
    /// Every value, in the order of the menu «Різниця».
    pub const ALL: [Self; 2] = [Self::List, Self::Tree];

    /// The value as the database stores it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::List => "list",
            Self::Tree => "tree",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.as_str() == text)
    }
}

/// What a node of the difference tree says (TASK-33 #6): its name, its change and what
/// changed, and for a folder how many changes it holds; or its name and change only.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TreeNodeText {
    #[default]
    Long,
    Short,
}

impl TreeNodeText {
    /// Every value, in the order the settings window lists them.
    pub const ALL: [Self; 2] = [Self::Long, Self::Short];

    /// The value as the database stores it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Long => "long",
            Self::Short => "short",
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

/// The theme of the interface the user chose (TASK-34). It takes effect at once, and a
/// contrast theme of Windows always wins over it while it is on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Theme {
    /// Light or dark as Windows is, following it as it switches.
    #[default]
    SameAsWindows,
    Light,
    Dark,
}

impl Theme {
    /// Every value, in the order the settings window lists them.
    pub const ALL: [Self; 3] = [Self::SameAsWindows, Self::Light, Self::Dark];

    /// The value as the database stores it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SameAsWindows => "windows",
            Self::Light => "light",
            Self::Dark => "dark",
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

    /// What the focus does when a comparison ends; the default when nothing is stored.
    pub fn focus_after_comparison(&self) -> Result<FocusAfterComparison, SettingsError> {
        Ok(match self.get(KEY_FOCUS_AFTER_COMPARISON)? {
            Some(Value::Text(text)) => FocusAfterComparison::parse(&text).unwrap_or_default(),
            _ => FocusAfterComparison::default(),
        })
    }

    pub fn set_focus_after_comparison(
        &self,
        value: FocusAfterComparison,
    ) -> Result<(), SettingsError> {
        self.set(KEY_FOCUS_AFTER_COMPARISON, value.as_str())
    }

    /// How sizes are written; the default when nothing is stored.
    pub fn size_format(&self) -> Result<SizeFormat, SettingsError> {
        Ok(match self.get(KEY_SIZE_FORMAT)? {
            Some(Value::Text(text)) => SizeFormat::parse(&text).unwrap_or_default(),
            _ => SizeFormat::default(),
        })
    }

    pub fn set_size_format(&self, value: SizeFormat) -> Result<(), SettingsError> {
        self.set(KEY_SIZE_FORMAT, value.as_str())
    }

    /// Which types of rows the difference list shows, indexed by [`ChangeType`]; all of
    /// them when nothing is stored (TASK-13 #28).
    pub fn difference_types(&self) -> Result<[bool; 4], SettingsError> {
        Ok(match self.get(KEY_DIFFERENCE_TYPES)? {
            Some(Value::Text(text)) => {
                let mut shown = [false; 4];
                for name in text.split(',') {
                    if let Some(kind) = ChangeType::parse(name) {
                        shown[kind as usize] = true;
                    }
                }
                shown
            }
            _ => [true; 4],
        })
    }

    pub fn set_difference_types(&self, shown: [bool; 4]) -> Result<(), SettingsError> {
        let names: Vec<&str> = ChangeType::ALL
            .into_iter()
            .filter(|&kind| shown[kind as usize])
            .map(ChangeType::as_str)
            .collect();
        self.set(KEY_DIFFERENCE_TYPES, names.join(","))
    }

    /// The order of the difference list; by path, ascending, when nothing is stored.
    pub fn difference_sort(&self) -> Result<DifferenceSort, SettingsError> {
        let column = match self.get(KEY_DIFFERENCE_SORT)? {
            Some(Value::Text(text)) => SortColumn::parse(&text).unwrap_or_default(),
            _ => SortColumn::default(),
        };
        let descending = matches!(
            self.get(KEY_DIFFERENCE_DESCENDING)?,
            Some(Value::Integer(n)) if n != 0
        );
        Ok(DifferenceSort { column, descending })
    }

    pub fn set_difference_sort(&self, sort: DifferenceSort) -> Result<(), SettingsError> {
        self.set(KEY_DIFFERENCE_SORT, sort.column.as_str())?;
        self.set(KEY_DIFFERENCE_DESCENDING, i64::from(sort.descending))
    }

    /// How the details of a change write attributes; in words when nothing is stored.
    pub fn attribute_format(&self) -> Result<AttributeFormat, SettingsError> {
        Ok(match self.get(KEY_ATTRIBUTE_FORMAT)? {
            Some(Value::Text(text)) => AttributeFormat::parse(&text).unwrap_or_default(),
            _ => AttributeFormat::default(),
        })
    }

    pub fn set_attribute_format(&self, value: AttributeFormat) -> Result<(), SettingsError> {
        self.set(KEY_ATTRIBUTE_FORMAT, value.as_str())
    }

    /// Whether «Показати в Провіднику» asks first when the entry is no longer on disk; off
    /// by default, as an action that harms nothing needs no confirmation (TASK-14 #13).
    pub fn warn_missing_entry(&self) -> Result<bool, SettingsError> {
        Ok(matches!(
            self.get(KEY_WARN_MISSING_ENTRY)?,
            Some(Value::Integer(n)) if n != 0
        ))
    }

    pub fn set_warn_missing_entry(&self, warn: bool) -> Result<(), SettingsError> {
        self.set(KEY_WARN_MISSING_ENTRY, i64::from(warn))
    }

    /// Whether the difference shows as a list or as a tree; as a list when nothing is
    /// stored (TASK-33).
    pub fn difference_view(&self) -> Result<DifferenceView, SettingsError> {
        Ok(match self.get(KEY_DIFFERENCE_VIEW)? {
            Some(Value::Text(text)) => DifferenceView::parse(&text).unwrap_or_default(),
            _ => DifferenceView::default(),
        })
    }

    pub fn set_difference_view(&self, value: DifferenceView) -> Result<(), SettingsError> {
        self.set(KEY_DIFFERENCE_VIEW, value.as_str())
    }

    /// What the nodes of the difference tree say; the long text when nothing is stored
    /// (TASK-33 #6).
    pub fn tree_node_text(&self) -> Result<TreeNodeText, SettingsError> {
        Ok(match self.get(KEY_TREE_NODE_TEXT)? {
            Some(Value::Text(text)) => TreeNodeText::parse(&text).unwrap_or_default(),
            _ => TreeNodeText::default(),
        })
    }

    pub fn set_tree_node_text(&self, value: TreeNodeText) -> Result<(), SettingsError> {
        self.set(KEY_TREE_NODE_TEXT, value.as_str())
    }

    /// The theme of the interface; «Same as Windows» when nothing is stored (TASK-34).
    pub fn theme(&self) -> Result<Theme, SettingsError> {
        Ok(match self.get(KEY_THEME)? {
            Some(Value::Text(text)) => Theme::parse(&text).unwrap_or_default(),
            _ => Theme::default(),
        })
    }

    pub fn set_theme(&self, value: Theme) -> Result<(), SettingsError> {
        self.set(KEY_THEME, value.as_str())
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
