//! Where revisor keeps its data: it is portable, so everything — the profiles and the
//! settings — is in `data` next to the exe, and revisor.exe and revisor-cli.exe, which stand
//! beside each other, share it (TASK-47).

use std::path::{Path, PathBuf};

/// The settings database in the data folder (ADR-0007).
pub const SETTINGS_FILE: &str = "settings.db";
/// The folder of the profiles in the data folder, a database file per profile (ADR-0006).
pub const PROFILES_DIR: &str = "profiles";

/// The data folder: `data` next to the exe.
pub fn dir() -> PathBuf {
    let exe = std::env::current_exe().unwrap_or_default();
    exe.parent().unwrap_or(Path::new("")).join("data")
}
