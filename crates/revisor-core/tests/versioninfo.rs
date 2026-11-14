//! The VERSIONINFO of both exes, checked against the version Cargo carries.
//!
//! A version bump is a `Cargo.toml` edit, and `crates/revisor/resources/app.rc` and
//! `crates/revisor-cli/resources/cli.rc` (TASK-47) are the files that are easy to forget. The check is pure text over two files read at compile time, so it
//! lives here, in the crate that links no wxWidgets (ADR-0005), rather than in the binary.
//!
//! The version compared against is this crate's, which is the binary's because both take
//! `version.workspace = true`; that inheritance is asserted, not assumed.

const APP_RC: &str = include_str!("../../revisor/resources/app.rc");
const CLI_RC: &str = include_str!("../../revisor-cli/resources/cli.rc");
/// Each `.rc` with its name and the names of its exe.
const RCS: [(&str, &str, &str); 2] = [
    ("app.rc", APP_RC, "revisor"),
    ("cli.rc", CLI_RC, "revisor-cli"),
];
const BIN_MANIFEST: &str = include_str!("../../revisor/Cargo.toml");
const CORE_MANIFEST: &str = include_str!("../Cargo.toml");

/// The workspace version, which every crate here inherits.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The `.rc`'s statements with their `//` comments removed, so prose about a field cannot be
/// mistaken for the field.
fn statements(rc: &'static str) -> impl Iterator<Item = &'static str> {
    rc.lines()
        .map(|line| line.split("//").next().unwrap_or("").trim())
        .filter(|line| !line.is_empty())
}

/// The text of `VALUE "<field>", "<text>"`.
fn string_value(rc: &'static str, field: &str) -> &'static str {
    let prefix = format!("VALUE \"{field}\"");
    let statement = statements(rc)
        .find(|statement| statement.starts_with(&prefix))
        .unwrap_or_else(|| panic!("the .rc has no VALUE {field:?}"));
    statement
        .split('"')
        .nth(3)
        .unwrap_or_else(|| panic!("the .rc's {field:?} has no quoted value: {statement:?}"))
}

/// The comma-separated numbers of a `FILEVERSION` or `PRODUCTVERSION` statement, without the
/// spacing.
fn binary_version(rc: &'static str, statement_name: &str) -> String {
    let statement = statements(rc)
        .find(|statement| statement.starts_with(statement_name))
        .unwrap_or_else(|| panic!("the .rc has no {statement_name} statement"));
    statement[statement_name.len()..]
        .split(',')
        .map(str::trim)
        .collect::<Vec<&str>>()
        .join(",")
}

#[test]
fn both_crates_take_the_workspace_version() {
    for (path, manifest) in [
        ("crates/revisor/Cargo.toml", BIN_MANIFEST),
        ("crates/revisor-core/Cargo.toml", CORE_MANIFEST),
    ] {
        assert!(
            manifest
                .lines()
                .any(|line| line.trim() == "version.workspace = true"),
            "{path} must inherit the workspace version"
        );
    }
}

#[test]
fn the_versioninfo_carries_the_crate_version() {
    let [major, minor, patch] = <[&str; 3]>::try_from(VERSION.split('.').collect::<Vec<&str>>())
        .expect("the workspace version is major.minor.patch");
    let quad = format!("{major},{minor},{patch},0");

    for (name, rc, _) in RCS {
        assert_eq!(binary_version(rc, "FILEVERSION"), quad, "{name}");
        assert_eq!(binary_version(rc, "PRODUCTVERSION"), quad, "{name}");
        assert_eq!(string_value(rc, "FileVersion"), VERSION, "{name}");
        assert_eq!(string_value(rc, "ProductVersion"), VERSION, "{name}");
    }
}

#[test]
fn the_versioninfo_names_revisor_and_its_author() {
    for (name, rc, exe) in RCS {
        assert_eq!(string_value(rc, "CompanyName"), "Ruslan Iskov", "{name}");
        assert_eq!(string_value(rc, "ProductName"), "revisor", "{name}");
        assert_eq!(string_value(rc, "InternalName"), exe, "{name}");
        assert_eq!(
            string_value(rc, "OriginalFilename"),
            format!("{exe}.exe"),
            "{name}"
        );
    }
}
