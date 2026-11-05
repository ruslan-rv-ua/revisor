//! Failures of actions on files in the terms the interface explains them (brief, section
//! 5): the causes revisor recognises get its own words, the rest keep the text and code of
//! Windows or SQLite.

use std::io;

use revisor_core::compare::CompareError;
use revisor_core::settings::SettingsError;
use revisor_core::store::StoreError;

use crate::i18n::Failure;

// Win32 error codes.
const ERROR_ACCESS_DENIED: i32 = 5;
const ERROR_SHARING_VIOLATION: i32 = 32;
const ERROR_LOCK_VIOLATION: i32 = 33;
const ERROR_HANDLE_DISK_FULL: i32 = 39;
const ERROR_DISK_FULL: i32 = 112;
const ERROR_CANCELLED: i32 = 1223;
// SQLite primary result codes.
const SQLITE_PERM: i32 = 3;
const SQLITE_BUSY: i32 = 5;
const SQLITE_LOCKED: i32 = 6;
const SQLITE_READONLY: i32 = 8;
const SQLITE_FULL: i32 = 13;
const SQLITE_NOTADB: i32 = 26;

pub fn of_io(e: &io::Error) -> Failure {
    match e.raw_os_error() {
        Some(ERROR_ACCESS_DENIED) => Failure::AccessDenied,
        Some(ERROR_SHARING_VIOLATION | ERROR_LOCK_VIOLATION) => Failure::InUse,
        Some(ERROR_DISK_FULL | ERROR_HANDLE_DISK_FULL) => Failure::DiskFull,
        Some(ERROR_CANCELLED) => Failure::Cancelled,
        code => {
            // Rust adds " (os error N)"; the code is shown on its own.
            let text = e.to_string();
            let text = text.rfind(" (os error ").map_or(&*text, |at| &text[..at]);
            Failure::Other {
                text: text.trim_end_matches(['.', ' ']).to_string(),
                code: code.map(i64::from),
            }
        }
    }
}

pub fn of_store(e: &StoreError) -> Failure {
    match e {
        StoreError::Io(e) => of_io(e),
        StoreError::NewerSchema(_) => Failure::NewerVersion,
        StoreError::NotAProfile => Failure::NotAProfile,
        StoreError::Sqlite(e) => of_sqlite(
            e.sqlite_error().map(|error| error.extended_code),
            e.to_string(),
            Failure::NotAProfile,
        ),
        // A cancelled snapshot (TASK-8) and an interrupted compaction (TASK-9) say so in
        // their own words.
        StoreError::Cancelled => Failure::Other {
            text: e.to_string(),
            code: None,
        },
    }
}

pub fn of_compare(e: &CompareError) -> Failure {
    match e {
        CompareError::Store(e) => of_store(e),
        CompareError::NoSuchSnapshot(_) => Failure::SnapshotDeleted,
        // A cancelled comparison (TASK-11) says so in its own words.
        CompareError::Cancelled => Failure::Other {
            text: e.to_string(),
            code: None,
        },
    }
}

pub fn of_settings(e: &SettingsError) -> Failure {
    match e {
        SettingsError::Io(e) => of_io(e),
        SettingsError::NewerSchema(_) => Failure::NewerVersion,
        SettingsError::NotSettings => Failure::NotSettings,
        SettingsError::Sqlite(e) => of_sqlite(
            e.sqlite_error().map(|error| error.extended_code),
            e.to_string(),
            Failure::NotSettings,
        ),
    }
}

/// An SQLite error by its extended result code and text; `not_a_database` is what a file
/// that is not a database at all amounts to: not a profile, or not the settings file.
fn of_sqlite(code: Option<i32>, text: String, not_a_database: Failure) -> Failure {
    match code.map(|code| code & 0xff) {
        Some(SQLITE_FULL) => Failure::DiskFull,
        Some(SQLITE_BUSY | SQLITE_LOCKED) => Failure::InUse,
        Some(SQLITE_PERM | SQLITE_READONLY) => Failure::AccessDenied,
        Some(SQLITE_NOTADB) => not_a_database,
        _ => Failure::Other {
            text,
            code: code.map(i64::from),
        },
    }
}
