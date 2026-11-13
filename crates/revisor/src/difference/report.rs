//! The text report of the difference shown (TASK-39): what the export writes for reading,
//! in the interface's language and the user's regional formats.
//!
//! The report holds the rows the list shows — the types and the text of the filter apply —
//! always in the order of their paths, whichever order or view the window shows (#3). Its
//! header says it is a report of revisor and its version, what the field «Порівняння» says
//! — the pair, the summary with how many rows are shown, the warnings —, which filter
//! applies, and which folders were compared (#4). The changes follow in groups by type, a
//! group's name on a line of its own with its count, two empty lines before it and one
//! after; there is no empty group (#5). Each change is one line, which starts with the
//! whole path (#6). There are no lines of repeated characters, arrows, bullets or columns
//! lined up by spaces, which screen readers read aloud or out of order (#7).
//!
//! The window takes an [`Export`] of the list, which holds the difference and the rows,
//! and a job writes it on its own thread, so the window answers while a large one is
//! written (#8).

use std::io::{self, Write};
use std::sync::Arc;

use revisor_core::compare::Difference;
use revisor_core::scan::{Entry, EntryKind};
use revisor_core::settings::ChangeType;

use super::{
    Field, List, MovedFolders, Row, field_bits, former_path, inaccessibility, path_key,
    path_of_row, shown_entry,
};
use crate::details::{self, Formats, fraction_digits};
use crate::difference::{Regional, format_size};
use crate::i18n::{Lang, Strings};

/// How the lines of the report end: as Windows ends them, so every editor of Windows
/// reads them (decided 2026-11-13).
const NEWLINE: &str = "\r\n";

/// The byte order mark of UTF-8, which tells every editor of Windows the encoding of the
/// report (decided 2026-11-13).
const BOM: &str = "\u{feff}";

/// How many rows are written between two looks at whether the export goes on.
const ROWS_PER_CHECK: usize = 4096;

/// What the export of the difference shown writes, taken from the list as it shows it
/// (TASK-39 #3, #4): the header, and the rows the filter lets through in the order of
/// their paths, with the difference they are of.
pub struct Export {
    lang: Lang,
    header: Vec<String>,
    difference: Arc<Difference>,
    moved: MovedFolders,
    rows: Vec<Row>,
    formats: Formats,
}

impl Export {
    /// The language the report is in: the strings [`Export::write`] takes are of it.
    pub fn lang(&self) -> Lang {
        self.lang
    }

    /// Writes the report to `out`, as UTF-8 with its mark and lines ending as Windows
    /// ends them. `go_on` is asked, with how many rows are written so far, every few
    /// thousand rows; when it says no, the writing stops and `Ok(false)` comes back.
    pub fn write(
        &self,
        s: &dyn Strings,
        regional: &dyn Regional,
        out: &mut dyn Write,
        go_on: &mut dyn FnMut(usize) -> bool,
    ) -> io::Result<bool> {
        out.write_all(BOM.as_bytes())?;
        for line in &self.header {
            write_line(out, line)?;
        }
        let mut written = 0;
        for kind in ChangeType::ALL {
            let count = self.rows.iter().filter(|row| row.kind == kind).count();
            if count == 0 {
                continue;
            }
            write_line(out, "")?;
            write_line(out, "")?;
            let count = regional.number(&count.to_string(), 0);
            write_line(out, &s.report_group(kind, &count))?;
            write_line(out, "")?;
            for row in self.rows.iter().filter(|row| row.kind == kind) {
                if written % ROWS_PER_CHECK == 0 && !go_on(written) {
                    return Ok(false);
                }
                write_line(out, &self.line(s, regional, row))?;
                written += 1;
            }
        }
        out.flush()?;
        Ok(true)
    }

    /// The whole report as text, for the tests.
    #[cfg(test)]
    pub fn text(&self, s: &dyn Strings, regional: &dyn Regional) -> String {
        let mut bytes = Vec::new();
        self.write(s, regional, &mut bytes, &mut |_| true)
            .expect("a vector takes every byte");
        String::from_utf8(bytes).expect("the report is UTF-8")
    }

    /// The line of `row` (#6): the whole path; for a folder or a link, what it is; for a
    /// replaced entry, that it is; for a renamed or moved one, the path it had; for a
    /// modified, renamed or moved one, each field that differs with its value before and
    /// after; for an inaccessible one, in which snapshot and why.
    fn line(&self, s: &dyn Strings, regional: &dyn Regional, row: &Row) -> String {
        let difference = &*self.difference;
        let path = path_of_row(difference, &self.moved, row)
            .to_string_lossy()
            .into_owned();
        let (tree, at) = shown_entry(difference, row);
        let kind = tree[at as usize].kind;
        let (old, new) = (row.earlier(difference), row.later(difference));
        let mut parts: Vec<String> = Vec::new();
        if kind != EntryKind::File {
            parts.push(s.kind(kind).to_string());
        }
        if row.replaced() {
            parts.push(s.report_replaced().to_string());
        }
        match row.kind {
            ChangeType::Moved => {
                let former = former_path(difference, row);
                parts.push(s.moved_from(&former.to_string_lossy()));
                parts.extend(self.fields(s, regional, row, old, new));
            }
            ChangeType::Modified => parts.extend(self.fields(s, regional, row, old, new)),
            ChangeType::Inaccessible => {
                if let Some(why) = inaccessibility(row.kind, old, new) {
                    parts.push(s.report_inaccessible(&why));
                }
            }
            ChangeType::Added | ChangeType::Deleted => {}
        }
        if parts.is_empty() {
            path
        } else {
            format!("{path} — {}", parts.join("; "))
        }
    }

    /// Each field of a modified, renamed or moved row that differs, as the column «Що
    /// змінилося» names it, with its value before and after: «розмір: було 1 024 байти,
    /// стало 2 048 байтів».
    fn fields(
        &self,
        s: &dyn Strings,
        regional: &dyn Regional,
        row: &Row,
        old: Option<&Entry>,
        new: Option<&Entry>,
    ) -> Vec<String> {
        let (Some(a), Some(b)) = (old, new) else {
            return Vec::new();
        };
        let bits = field_bits(&self.difference, row);
        let formats = self.formats;
        Field::ALL
            .into_iter()
            .filter(|field| bits & field.bit() != 0)
            .map(|field| {
                let (before, after) = match field {
                    Field::FileVersion => (
                        details::version(s, a.version, |v| v.file),
                        details::version(s, b.version, |v| v.file),
                    ),
                    Field::ProductVersion => (
                        details::version(s, a.version, |v| v.product),
                        details::version(s, b.version, |v| v.product),
                    ),
                    Field::LinkTarget => (link_target(s, a), link_target(s, b)),
                    Field::Size => sizes(s, regional, formats, a.size, b.size),
                    Field::DateModified => dates(regional, a.modified, b.modified),
                    Field::DateCreated => dates(regional, a.created, b.created),
                    Field::Attributes => (
                        details::attributes(s, regional, formats.attributes, a.kind, a.attributes),
                        details::attributes(s, regional, formats.attributes, b.kind, b.attributes),
                    ),
                    Field::NameCase => (
                        a.name.to_string_lossy().into_owned(),
                        b.name.to_string_lossy().into_owned(),
                    ),
                };
                format!("{}: {}", s.field(field), s.before_after(&before, &after))
            })
            .collect()
    }
}

/// Writes `line` and the end of a line.
fn write_line(out: &mut dyn Write, line: &str) -> io::Result<()> {
    out.write_all(line.as_bytes())?;
    out.write_all(NEWLINE.as_bytes())
}

/// A link's target as the details write it.
fn link_target(s: &dyn Strings, entry: &Entry) -> String {
    match &entry.link_target {
        Some(target) if !target.is_empty() => target.to_string_lossy().into_owned(),
        _ => s.target_unknown().to_string(),
    }
}

/// Two sizes as the setting «Розмір» writes them, and, when it writes them the same, also
/// in bytes, as the details do (decided 2026-11-13).
fn sizes(
    s: &dyn Strings,
    regional: &dyn Regional,
    formats: Formats,
    a: u64,
    b: u64,
) -> (String, String) {
    let (before, after) = (
        format_size(a, formats.size, s, regional),
        format_size(b, formats.size, s, regional),
    );
    if before == after {
        (
            details::size(s, regional, formats.size, a),
            details::size(s, regional, formats.size, b),
        )
    } else {
        (before, after)
    }
}

/// Two moments as a snapshot's name writes them, and, when they are written the same,
/// with as many digits of the fraction of a second as tell them apart, as the details
/// write them (TASK-14 #9).
fn dates(regional: &dyn Regional, a: i64, b: i64) -> (String, String) {
    let (before, after) = (regional.file_time(a), regional.file_time(b));
    if before == after && a != b {
        let digits = fraction_digits(a, b);
        (
            regional.file_time_fraction(a, digits),
            regional.file_time_fraction(b, digits),
        )
    } else {
        (before, after)
    }
}

/// The characters a name of a file cannot hold, which the name the save dialog offers
/// writes as «_».
const NOT_IN_FILE_NAMES: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

impl List {
    /// The name of the file the save dialog offers for the difference shown (TASK-39 #2):
    /// the profile and the moments of both snapshots, as `time` writes them for a name of
    /// a file — «Програми 2026-11-13 14-05-33 — 2026-11-13 14-20-10.txt»; `None` before
    /// the first comparison.
    pub fn export_name(&self, time: &dyn Fn(i64) -> String) -> Option<String> {
        let pair = &self.shown.as_ref()?.pair;
        let [earlier, later] = &pair.snapshots;
        let name = format!(
            "{} {} — {}.txt",
            pair.profile.name,
            time(earlier.info.taken_at),
            time(later.info.taken_at)
        );
        Some(
            name.chars()
                .map(|c| {
                    if NOT_IN_FILE_NAMES.contains(&c) || c.is_control() {
                        '_'
                    } else {
                        c
                    }
                })
                .collect(),
        )
    }

    /// What the export writes of the difference shown, as the list shows it now (TASK-39
    /// #3, #4); `None` before the first comparison.
    pub fn export(&self) -> Option<Export> {
        let shown = self.shown.as_ref()?;
        let s = self.s;
        let mut header = vec![s.report_title(revisor_core::version())];
        header.extend(self.comparison_text()?.lines().map(str::to_string));
        let all_types = self.view.types.iter().all(|&shown| shown);
        if !all_types || !self.text.is_empty() {
            let types: Vec<ChangeType> = ChangeType::ALL
                .into_iter()
                .filter(|&kind| self.view.types[kind as usize])
                .collect();
            header.push(s.report_filter(
                (!all_types).then_some(types.as_slice()),
                (!self.text.is_empty()).then_some(self.text.as_str()),
            ));
        }
        let mut folders: Vec<_> = shown.difference.folders.iter().map(|f| &f.path).collect();
        folders.sort_by_cached_key(|path| path_key(path.as_os_str()));
        header.extend(
            folders
                .into_iter()
                .map(|path| s.report_folder(&path.to_string_lossy())),
        );
        let rows = shown
            .rows
            .iter()
            .zip(&shown.passed)
            .filter(|&(_, &passed)| passed)
            .map(|(row, _)| *row)
            .collect();
        Some(Export {
            lang: s.lang(),
            header,
            difference: Arc::clone(&shown.difference),
            moved: shown.moved.clone(),
            rows,
            formats: Formats {
                size: self.size_format,
                attributes: self.attribute_format,
            },
        })
    }
}
