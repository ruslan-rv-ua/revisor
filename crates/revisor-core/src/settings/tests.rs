use super::*;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::store::{ProfileSettings, Store};

/// A fresh folder under the system temp folder, removed on drop. The settings file goes
/// into its subfolder `data`, which does not exist at first.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("revisor-settings-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn db(&self) -> PathBuf {
        self.0.join("data").join("settings.db")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_new_settings_file_is_made_with_the_defaults() {
    let tmp = TempDir::new();
    let settings = Settings::open(&tmp.db()).unwrap();
    assert!(tmp.db().is_file(), "the file and its folder are created");
    assert_eq!(settings.last_profile().unwrap(), None);
    assert_eq!(
        settings.focus_after_snapshot().unwrap(),
        FocusAfterSnapshot::IfInList
    );
    assert!(settings.confirm_snapshot_deletion().unwrap());
    assert_eq!(settings.language().unwrap(), Language::SameAsWindows);
    let conn = rusqlite::Connection::open(tmp.db()).unwrap();
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, SCHEMA_VERSION);
}

#[test]
fn settings_survive_a_restart() {
    let tmp = TempDir::new();
    {
        let settings = Settings::open(&tmp.db()).unwrap();
        settings.set_last_profile(Some("Дім.db")).unwrap();
        settings
            .set_focus_after_snapshot(FocusAfterSnapshot::Never)
            .unwrap();
        settings.set_confirm_snapshot_deletion(false).unwrap();
        settings.set_language(Language::Ukrainian).unwrap();
    }
    let settings = Settings::open(&tmp.db()).unwrap();
    assert_eq!(settings.last_profile().unwrap().as_deref(), Some("Дім.db"));
    assert_eq!(settings.language().unwrap(), Language::Ukrainian);
    assert_eq!(
        settings.focus_after_snapshot().unwrap(),
        FocusAfterSnapshot::Never
    );
    assert!(!settings.confirm_snapshot_deletion().unwrap());

    // Each value can change again, and the last profile can be forgotten.
    settings
        .set_focus_after_snapshot(FocusAfterSnapshot::Always)
        .unwrap();
    settings.set_confirm_snapshot_deletion(true).unwrap();
    settings.set_language(Language::English).unwrap();
    settings.set_last_profile(None).unwrap();
    assert_eq!(settings.language().unwrap(), Language::English);
    assert_eq!(
        settings.focus_after_snapshot().unwrap(),
        FocusAfterSnapshot::Always
    );
    assert!(settings.confirm_snapshot_deletion().unwrap());
    assert_eq!(settings.last_profile().unwrap(), None);
}

#[test]
fn two_processes_share_one_settings_file() {
    let tmp = TempDir::new();
    let first = Settings::open(&tmp.db()).unwrap();
    let second = Settings::open(&tmp.db()).unwrap();
    first.set_last_profile(Some("Робота.db")).unwrap();
    assert_eq!(second.last_profile().unwrap().as_deref(), Some("Робота.db"));
    second.set_confirm_snapshot_deletion(false).unwrap();
    assert!(!first.confirm_snapshot_deletion().unwrap());
}

#[test]
fn a_value_no_revisor_wrote_falls_back_to_the_default() {
    let tmp = TempDir::new();
    drop(Settings::open(&tmp.db()).unwrap());
    let conn = rusqlite::Connection::open(tmp.db()).unwrap();
    conn.execute(
        "INSERT OR REPLACE INTO setting VALUES ('focus_after_snapshot', 'sometimes')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT OR REPLACE INTO setting VALUES ('confirm_snapshot_deletion', 'yes')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT OR REPLACE INTO setting VALUES ('language', 'fr')",
        [],
    )
    .unwrap();
    drop(conn);
    let settings = Settings::open(&tmp.db()).unwrap();
    assert_eq!(
        settings.focus_after_snapshot().unwrap(),
        FocusAfterSnapshot::IfInList
    );
    assert!(settings.confirm_snapshot_deletion().unwrap());
    assert_eq!(settings.language().unwrap(), Language::SameAsWindows);
}

#[test]
fn files_that_are_not_revisor_settings_are_refused() {
    let tmp = TempDir::new();
    fs::create_dir_all(tmp.db().parent().unwrap()).unwrap();

    // A text file is not a database at all.
    fs::write(tmp.db(), b"some text, not a database").unwrap();
    assert!(matches!(
        Settings::open(&tmp.db()),
        Err(SettingsError::NotSettings)
    ));
    fs::remove_file(tmp.db()).unwrap();

    // Another program's database has tables but not revisor's mark.
    let other = rusqlite::Connection::open(tmp.db()).unwrap();
    other.execute_batch("CREATE TABLE t (x)").unwrap();
    drop(other);
    assert!(matches!(
        Settings::open(&tmp.db()),
        Err(SettingsError::NotSettings)
    ));
    fs::remove_file(tmp.db()).unwrap();

    // A profile database is not the settings database either.
    drop(Store::create(&tmp.db(), &ProfileSettings::new_profile()).unwrap());
    assert!(matches!(
        Settings::open(&tmp.db()),
        Err(SettingsError::NotSettings)
    ));
}

#[test]
fn settings_from_a_newer_revisor_are_refused() {
    let tmp = TempDir::new();
    drop(Settings::open(&tmp.db()).unwrap());
    let conn = rusqlite::Connection::open(tmp.db()).unwrap();
    conn.pragma_update(None, "user_version", SCHEMA_VERSION + 1)
        .unwrap();
    drop(conn);
    assert!(matches!(
        Settings::open(&tmp.db()),
        Err(SettingsError::NewerSchema(v)) if v == SCHEMA_VERSION + 1
    ));
}

#[test]
fn the_focus_setting_names_round_trip() {
    for value in FocusAfterSnapshot::ALL {
        assert_eq!(FocusAfterSnapshot::parse(value.as_str()), Some(value));
    }
    assert_eq!(FocusAfterSnapshot::parse("sometimes"), None);
    assert_eq!(FocusAfterSnapshot::default(), FocusAfterSnapshot::IfInList);
}
