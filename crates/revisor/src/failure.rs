//! Failures of actions on files in the terms the interface explains them (brief, section
//! 5): the causes revisor recognises get its own words, the rest keep the text and code of
//! Windows or SQLite, and an error with no code says revisor did not expect it (TASK-25).

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
        Some(code) => {
            // Rust adds " (os error N)"; the code is shown on its own.
            let text = e.to_string();
            let text = text.rfind(" (os error ").map_or(&*text, |at| &text[..at]);
            Failure::Other {
                text: without_full_stop(text),
                code: i64::from(code),
            }
        }
        None => Failure::Unexpected(without_full_stop(&e.to_string())),
    }
}

pub fn of_store(e: &StoreError) -> Failure {
    match e {
        StoreError::Io(e) => of_io(e),
        StoreError::NewerSchema(_) => Failure::NewerVersion,
        StoreError::NotAProfile => Failure::NotAProfile,
        // A busy database is what the store says it is, as for its waits (ADR-0012).
        StoreError::Sqlite(_) if e.is_busy() => Failure::InUse,
        StoreError::Sqlite(e) => of_sqlite(
            e.sqlite_error().map(|error| error.extended_code),
            e.to_string(),
            Failure::NotAProfile,
        ),
        // A cancelled snapshot (TASK-8) and an interrupted compaction (TASK-9) are told by
        // the window in their own words; anywhere else, the action was cancelled.
        StoreError::Cancelled => Failure::Cancelled,
    }
}

pub fn of_compare(e: &CompareError) -> Failure {
    match e {
        CompareError::Store(e) => of_store(e),
        CompareError::NoSuchSnapshot(_) => Failure::SnapshotDeleted,
        // A cancelled comparison (TASK-11) is told by the window in its own words.
        CompareError::Cancelled => Failure::Cancelled,
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
/// that is not a database at all amounts to: not a profile, or not the settings file. An
/// error of rusqlite itself has no code.
fn of_sqlite(code: Option<i32>, text: String, not_a_database: Failure) -> Failure {
    let Some(code) = code else {
        return Failure::Unexpected(without_full_stop(&text));
    };
    match code & 0xff {
        SQLITE_FULL => Failure::DiskFull,
        SQLITE_BUSY | SQLITE_LOCKED => Failure::InUse,
        SQLITE_PERM | SQLITE_READONLY => Failure::AccessDenied,
        SQLITE_NOTADB => not_a_database,
        _ => Failure::Other {
            text,
            code: i64::from(code),
        },
    }
}

/// The text without the full stop it may end with: it goes inside a sentence.
fn without_full_stop(text: &str) -> String {
    text.trim_end_matches(['.', ' ']).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_windows_error_keeps_its_text_and_code_and_one_without_a_code_is_unexpected() {
        let path_not_found = io::Error::from_raw_os_error(3);
        assert!(matches!(
            of_io(&path_not_found),
            Failure::Other { code: 3, ref text } if !text.ends_with('.') && !text.contains("os error")
        ));
        let own = io::Error::new(
            io::ErrorKind::InvalidData,
            "stream did not contain valid UTF-8.",
        );
        assert_eq!(
            of_io(&own),
            Failure::Unexpected("stream did not contain valid UTF-8".into())
        );
        assert_eq!(
            of_sqlite(None, "Query returned no rows".into(), Failure::NotAProfile),
            Failure::Unexpected("Query returned no rows".into())
        );
        assert_eq!(
            of_sqlite(Some(10), "disk I/O error".into(), Failure::NotAProfile),
            Failure::Other {
                text: "disk I/O error".into(),
                code: 10
            }
        );
        // A cancellation that reaches a message says so, in the interface's words.
        assert_eq!(of_store(&StoreError::Cancelled), Failure::Cancelled);
        assert_eq!(of_compare(&CompareError::Cancelled), Failure::Cancelled);
    }
}
