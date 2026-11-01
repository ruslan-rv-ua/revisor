//! The exe's VERSIONINFO, checked against the version Cargo carries.
//!
//! A version bump is a `Cargo.toml` edit, and `crates/revisor/resources/app.rc` is the file
//! that is easy to forget. The check is pure text over two files read at compile time, so it
//! lives here, in the crate that links no wxWidgets (ADR-0005), rather than in the binary.
//!
//! The version compared against is this crate's, which is the binary's because both take
//! `version.workspace = true`; that inheritance is asserted, not assumed.

const RC: &str = include_str!("../../revisor/resources/app.rc");
const BIN_MANIFEST: &str = include_str!("../../revisor/Cargo.toml");

/// The workspace version, which every crate here inherits.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The `.rc`'s statements with their `//` comments removed, so prose about a field cannot be
/// mistaken for the field.
fn statements() -> impl Iterator<Item = &'static str> {
    RC.lines()
        .map(|line| line.split("//").next().unwrap_or("").trim())
        .filter(|line| !line.is_empty())
}

/// The text of `VALUE "<field>", "<text>"`.
fn string_value(field: &str) -> &'static str {
    let prefix = format!("VALUE \"{field}\"");
    let statement = statements()
        .find(|statement| statement.starts_with(&prefix))
        .unwrap_or_else(|| panic!("app.rc has no VALUE {field:?}"));
    statement
        .split('"')
        .nth(3)
        .unwrap_or_else(|| panic!("app.rc's {field:?} has no quoted value: {statement:?}"))
}

/// The comma-separated numbers of a `FILEVERSION` or `PRODUCTVERSION` statement, without the
/// spacing.
fn binary_version(statement_name: &str) -> String {
    let statement = statements()
        .find(|statement| statement.starts_with(statement_name))
        .unwrap_or_else(|| panic!("app.rc has no {statement_name} statement"));
    statement[statement_name.len()..]
        .split(',')
        .map(str::trim)
        .collect::<Vec<&str>>()
        .join(",")
}

#[test]
fn the_binary_takes_the_workspace_version() {
    assert!(
        BIN_MANIFEST
            .lines()
            .any(|line| line.trim() == "version.workspace = true"),
        "crates/revisor/Cargo.toml must inherit the workspace version"
    );
}

#[test]
fn the_versioninfo_carries_the_crate_version() {
    let [major, minor, patch] = <[&str; 3]>::try_from(VERSION.split('.').collect::<Vec<&str>>())
        .expect("the workspace version is major.minor.patch");
    let quad = format!("{major},{minor},{patch},0");

    assert_eq!(binary_version("FILEVERSION"), quad);
    assert_eq!(binary_version("PRODUCTVERSION"), quad);
    assert_eq!(string_value("FileVersion"), VERSION);
    assert_eq!(string_value("ProductVersion"), VERSION);
}

#[test]
fn the_versioninfo_names_revisor_and_its_author() {
    assert_eq!(string_value("CompanyName"), "Ruslan Iskov");
    assert_eq!(string_value("ProductName"), "revisor");
    assert_eq!(string_value("InternalName"), "revisor");
    assert_eq!(string_value("OriginalFilename"), "revisor.exe");
}
