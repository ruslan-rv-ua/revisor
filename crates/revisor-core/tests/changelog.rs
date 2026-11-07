//! `CHANGELOG.md`, checked against the version Cargo carries.
//!
//! The file is written by hand and read by the release workflow, which publishes a version's
//! section as the text of its release page (`.github/workflows/release.yml`). A version bump
//! that leaves the changelog behind, or a section left empty, is no compile error, so the
//! pairing is held here. Like `versioninfo.rs`, the check is pure text over a file read at
//! compile time, so it lives in the crate that links no wxWidgets (ADR-0005).
//!
//! **One grammar, read in two places.** A version heading is a line beginning `## [`, and its
//! version is the text up to the `]`. A section runs from its heading to the next heading or to
//! the end of the file, less the link references at the foot, which follow the oldest section.
//! The workflow reads the same rule in PowerShell; a second grammar would let this test and the
//! release disagree about the text a tag publishes.
//!
//! The version compared against is this crate's, which is the binary's too: `versioninfo.rs`
//! asserts that both take `version.workspace = true`.

/// The changelog, read at compile time, so the tests need no file system.
const CHANGELOG: &str = include_str!("../../../CHANGELOG.md");

/// The workspace version, which every crate here inherits.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Where the link references at the foot point. Written out rather than read from the file: a
/// link block copied from another project would agree with itself and still point elsewhere.
const REPOSITORY: &str = "https://github.com/ruslan-rv-ua/revisor";

/// The heading over changes not released yet, the one heading that names no version.
const UNRELEASED: &str = "Unreleased";

/// The version of every heading, in the order the file lists them.
///
/// A heading that never closes its `]` panics rather than being passed over: the workflow ends
/// a section at `## [` without looking for the `]`, so skipping such a line here would make it
/// a heading to the release and not to this test.
fn version_headings(changelog: &str) -> Vec<&str> {
    changelog
        .lines()
        .filter_map(|line| line.strip_prefix("## ["))
        .map(|rest| match rest.split_once(']') {
            Some((version, _)) => version,
            None => panic!("CHANGELOG.md's heading `## [{rest}` never closes its `]`"),
        })
        .collect()
}

/// Whether `line` is a link reference definition, like those at the foot of the file: the
/// workflow's `^\[[^\]]+\]: `.
fn is_link_reference(line: &str) -> bool {
    line.strip_prefix('[')
        .and_then(|rest| rest.split_once(']'))
        .is_some_and(|(label, after)| !label.is_empty() && after.starts_with(": "))
}

/// The text a release of `version` publishes, as the workflow extracts it; `None` without a
/// heading for that version.
fn section_text(changelog: &str, version: &str) -> Option<String> {
    let heading = format!("## [{version}]");
    let mut lines = changelog.lines();
    lines.find(|line| line.starts_with(&heading))?;
    let body: Vec<&str> = lines.take_while(|line| !line.starts_with("## [")).collect();
    let end = body
        .iter()
        .rposition(|line| !line.trim().is_empty() && !is_link_reference(line))
        .map_or(0, |last| last + 1);
    Some(body[..end].join("\n").trim().to_owned())
}

#[test]
fn the_grammar_reads_what_it_claims_to() {
    // Self-sensitivity: the tests below are only as good as these two readers, and what they
    // must not match is invisible in a passing run.
    let sample = "# Changelog\n\
                  \n\
                  ## [Unreleased]\n\
                  \n\
                  ### Added\n\
                  \n\
                  - something\n\
                  \n\
                  ## [0.2.0] - 2026-12-01\n\
                  \n\
                  ## Not a version heading\n\
                  \n\
                  ## [0.1.0] - 2026-11-07\n\
                  \n\
                  The first release.\n\
                  \n\
                  [Unreleased]: https://example.invalid/compare/v0.2.0...HEAD\n\
                  [0.2.0]: https://example.invalid/compare/v0.1.0...v0.2.0\n\
                  [0.1.0]: https://example.invalid/releases/tag/v0.1.0\n";
    assert_eq!(version_headings(sample), [UNRELEASED, "0.2.0", "0.1.0"]);
    assert_eq!(
        section_text(sample, UNRELEASED).as_deref(),
        Some("### Added\n\n- something")
    );
    // A `##` line that is not `## [` stays inside its section.
    assert_eq!(
        section_text(sample, "0.2.0").as_deref(),
        Some("## Not a version heading")
    );
    // The oldest section ends before the link references at the foot.
    assert_eq!(
        section_text(sample, "0.1.0").as_deref(),
        Some("The first release.")
    );
    // A version is matched whole: 0.1 is not 0.1.0.
    assert_eq!(section_text(sample, "0.1"), None);
    assert!(!is_link_reference("[] : nothing"));
    assert!(!is_link_reference("- [0.1.0] is a version"));
}

#[test]
#[should_panic(expected = "never closes its")]
fn a_heading_that_never_closes_its_bracket_fails_rather_than_being_skipped() {
    version_headings("## [Unreleased]\n\n## [0.2.0 - 2026-12-01\n");
}

#[test]
fn the_newest_released_section_carries_the_crate_version() {
    // Read as a sequence, because the order is half of the claim: [Unreleased] on top, and
    // right under it the newest release, which the release commit wrote.
    let headings = version_headings(CHANGELOG);
    assert_eq!(
        headings.first().copied(),
        Some(UNRELEASED),
        "CHANGELOG.md must open on its [{UNRELEASED}] section: the next notable change goes there"
    );
    assert_eq!(
        headings.get(1).copied(),
        Some(VERSION),
        "the newest released section of CHANGELOG.md must be the crate version {VERSION}: a \
         release changes Cargo.toml, app.rc and CHANGELOG.md together"
    );
}

#[test]
fn the_newest_released_section_has_text_to_publish() {
    let text = section_text(CHANGELOG, VERSION)
        .unwrap_or_else(|| panic!("CHANGELOG.md has no [{VERSION}] section"));
    assert!(
        !text.is_empty(),
        "CHANGELOG.md's [{VERSION}] section is empty: the release page would be blank"
    );
}

#[test]
fn every_released_heading_has_its_date() {
    for line in CHANGELOG.lines().filter(|line| line.starts_with("## [")) {
        if line == format!("## [{UNRELEASED}]") {
            continue;
        }
        let date = line
            .split_once("] - ")
            .map(|(_, date)| date)
            .unwrap_or_else(|| panic!("heading {line:?} is not `## [x.y.z] - YYYY-MM-DD`"));
        let shaped = date.len() == 10
            && date.bytes().enumerate().all(|(i, byte)| match i {
                4 | 7 => byte == b'-',
                _ => byte.is_ascii_digit(),
            });
        assert!(shaped, "heading {line:?} has no YYYY-MM-DD date");
    }
}

#[test]
fn every_version_heading_carries_its_link_reference() {
    // Keep a Changelog's own foot: [Unreleased] compares the newest release with HEAD, each
    // release compares itself with the one before, and the oldest links to its tag.
    let headings = version_headings(CHANGELOG);
    let released = headings
        .get(1..)
        .filter(|released| !released.is_empty())
        .expect("CHANGELOG.md has a released section under [Unreleased]");
    let mut expected = vec![format!(
        "[{UNRELEASED}]: {REPOSITORY}/compare/v{}...HEAD",
        released[0]
    )];
    expected.extend(released.windows(2).map(|pair| {
        let (newer, older) = (pair[0], pair[1]);
        format!("[{newer}]: {REPOSITORY}/compare/v{older}...v{newer}")
    }));
    let oldest = released[released.len() - 1];
    expected.push(format!("[{oldest}]: {REPOSITORY}/releases/tag/v{oldest}"));
    for reference in expected {
        assert!(
            CHANGELOG.lines().any(|line| line == reference),
            "CHANGELOG.md has no link reference {reference:?} at its foot"
        );
    }
}
