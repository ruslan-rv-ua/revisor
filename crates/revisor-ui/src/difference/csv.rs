//! The difference shown as CSV (TASK-40), for Excel and scripts: the same rows as the text
//! report, in the order of their paths (#2), in a format that is the same in every
//! language of the interface and every region (#3). Scripts and the command line rely on
//! it, so from 1.0 a change that breaks it is a new major version.
//!
//! The file has a line of the names of the columns and a line for each row, with no header
//! or summary, which programs would read as data (#2). Each field of an entry is a pair of
//! columns, `_before` and `_after`, empty where the entry has no such field (#4). The
//! values are separated by the delimiter the setting «Роздільник у CSV» gives, and a value
//! that holds it, a quote or the end of a line is quoted as RFC 4180 says (#6). Moments
//! are in local time as ISO 8601 writes them, with the offset from UTC and seven digits of
//! the fraction of a second, all a FILETIME holds (#3). A link's target that Excel would
//! take for a formula gets an apostrophe before it, as OWASP advises (#7).

use std::ffi::OsStr;
use std::io::{self, Write};

use revisor_core::scan::{Entry, EntryKind, VersionInfo};
use revisor_core::settings::ChangeType;

use super::report::{BOM, NEWLINE, ROWS_PER_CHECK};
use super::{
    Export, Field, InaccessibleIn, Row, RowChange, WhatChanged, former_path, path_of_row,
    shown_entry, what_changed,
};
use crate::details::Attribute;

/// The names of the columns, in their order. A column a later revisor adds goes at the
/// end, so a script that reads the columns by name goes on working.
pub(super) const COLUMNS: [&str; 20] = [
    "path",
    "change",
    "kind",
    "what_changed",
    "path_before",
    "size_before",
    "size_after",
    "created_before",
    "created_after",
    "modified_before",
    "modified_after",
    "attributes_before",
    "attributes_after",
    "file_version_before",
    "file_version_after",
    "product_version_before",
    "product_version_after",
    "link_target_before",
    "link_target_after",
    "inaccessible_in",
];

/// The fields `what_changed` names, with their codes, in the order of the pairs of
/// columns; the case of the name has no pair, as `path` and `path_before` show it.
const FIELD_CODES: [(Field, &str); 8] = [
    (Field::Size, "size"),
    (Field::DateCreated, "created"),
    (Field::DateModified, "modified"),
    (Field::Attributes, "attributes"),
    (Field::FileVersion, "file_version"),
    (Field::ProductVersion, "product_version"),
    (Field::LinkTarget, "link_target"),
    (Field::NameCase, "name_case"),
];

/// FILETIME intervals in a second and in a day.
const TICKS_PER_SECOND: i64 = 10_000_000;
const TICKS_PER_DAY: i64 = 86_400 * TICKS_PER_SECOND;
/// Days from 1601-01-01, where FILETIME starts, to 1970-01-01.
const DAYS_TO_UNIX_EPOCH: i64 = 134_774;

impl Export {
    /// Writes the rows as CSV to `out` (#2–#7), as UTF-8 with its mark and lines ending as
    /// Windows ends them, the values separated by `delimiter`. `utc_offset` gives the
    /// offset from UTC, in minutes, of this computer's time zone at a moment given as a
    /// FILETIME; a moment it gives none for is written in UTC. `go_on` is asked as
    /// [`Export::write`] asks it.
    pub fn write_csv(
        &self,
        delimiter: char,
        utc_offset: &dyn Fn(i64) -> Option<i32>,
        out: &mut dyn Write,
        go_on: &mut dyn FnMut(usize) -> bool,
    ) -> io::Result<bool> {
        let mut line = String::new();
        out.write_all(BOM.as_bytes())?;
        write_record(out, &mut line, delimiter, COLUMNS.iter().copied())?;
        let mut values: Vec<String> = Vec::with_capacity(COLUMNS.len());
        for (written, row) in self.rows.iter().enumerate() {
            if written % ROWS_PER_CHECK == 0 && !go_on(written) {
                return Ok(false);
            }
            values.clear();
            self.values(row, utc_offset, &mut values);
            write_record(out, &mut line, delimiter, values.iter().map(String::as_str))?;
        }
        out.flush()?;
        Ok(true)
    }

    /// The whole CSV as text, for the tests.
    #[cfg(test)]
    pub fn csv(&self, delimiter: char, utc_offset: &dyn Fn(i64) -> Option<i32>) -> String {
        let mut bytes = Vec::new();
        self.write_csv(delimiter, utc_offset, &mut bytes, &mut |_| true)
            .expect("a vector takes every byte");
        String::from_utf8(bytes).expect("CSV is UTF-8")
    }

    /// The values of `row`, one for each of [`COLUMNS`].
    fn values(&self, row: &Row, utc_offset: &dyn Fn(i64) -> Option<i32>, values: &mut Vec<String>) {
        let difference = &*self.difference;
        let path = path_of_row(difference, &self.moved, row);
        let (tree, at) = shown_entry(difference, row);
        let (old, new) = (row.earlier(difference), row.later(difference));
        let (fields, inaccessible_in) = match what_changed(difference, row) {
            WhatChanged::Fields(bits) | WhatChanged::Moved(bits) => (field_codes(bits), None),
            WhatChanged::Inaccessible(which) => (String::new(), Some(which)),
            WhatChanged::Nothing => (String::new(), None),
        };
        // Where the entry was, when it was elsewhere, if only in the case of its name.
        let path_before = old
            .map(|_| former_path(difference, row))
            .filter(|before| before.as_os_str() != path.as_os_str());
        values.push(text(path.as_os_str()));
        values.push(change_code(row.change_type()).to_string());
        values.push(kind_code(tree[at as usize].kind).to_string());
        values.push(fields);
        values.push(path_before.map_or_else(String::new, |p| text(p.as_os_str())));
        let time = |entry: &Entry, of: fn(&Entry) -> i64| iso_time(of(entry), utc_offset);
        let pairs: [&dyn Fn(&Entry) -> String; 7] = [
            &|e| match e.kind {
                EntryKind::File => e.size.to_string(),
                EntryKind::Dir | EntryKind::Link => String::new(),
            },
            &|e| time(e, |e| e.created),
            &|e| time(e, |e| e.modified),
            &|e| attribute_codes(e.attributes),
            &|e| version(e.version, |v| v.file),
            &|e| version(e.version, |v| v.product),
            &|e| link_target(e),
        ];
        for value in pairs {
            values.push(old.map_or_else(String::new, value));
            values.push(new.map_or_else(String::new, value));
        }
        values.push(inaccessible_in.map_or("", inaccessible_code).to_string());
        debug_assert_eq!(values.len(), COLUMNS.len());
        debug_assert!(row.kind != ChangeType::Inaccessible || inaccessible_in.is_some());
    }
}

/// Writes one line of values, each quoted when it holds the delimiter, a quote or the end
/// of a line, with the quotes in it doubled (RFC 4180).
fn write_record<'a>(
    out: &mut dyn Write,
    line: &mut String,
    delimiter: char,
    values: impl Iterator<Item = &'a str>,
) -> io::Result<()> {
    line.clear();
    for (i, value) in values.enumerate() {
        if i > 0 {
            line.push(delimiter);
        }
        if value.contains([delimiter, '"', '\r', '\n']) {
            line.push('"');
            line.push_str(&value.replace('"', "\"\""));
            line.push('"');
        } else {
            line.push_str(value);
        }
    }
    line.push_str(NEWLINE);
    out.write_all(line.as_bytes())
}

/// A path or a name as text.
fn text(s: &OsStr) -> String {
    s.to_string_lossy().into_owned()
}

/// The code of what the column «Зміна» says a row is.
fn change_code(change: RowChange) -> &'static str {
    match change {
        RowChange::Added => "added",
        RowChange::Deleted => "deleted",
        RowChange::Modified => "modified",
        RowChange::Replaced => "replaced",
        RowChange::Renamed => "renamed",
        RowChange::Moved => "moved",
        RowChange::MovedAndRenamed => "moved_renamed",
        RowChange::Inaccessible => "inaccessible",
    }
}

fn kind_code(kind: EntryKind) -> &'static str {
    match kind {
        EntryKind::File => "file",
        EntryKind::Dir => "folder",
        EntryKind::Link => "link",
    }
}

fn inaccessible_code(which: InaccessibleIn) -> &'static str {
    match which {
        InaccessibleIn::Earlier => "earlier",
        InaccessibleIn::Later => "later",
        InaccessibleIn::Both => "both",
    }
}

/// The codes of the fields whose [`Field`] bits are `bits`, split by spaces.
fn field_codes(bits: u16) -> String {
    let codes: Vec<&str> = FIELD_CODES
        .iter()
        .filter(|(field, _)| bits & field.bit() != 0)
        .map(|&(_, code)| code)
        .collect();
    codes.join(" ")
}

/// The code of an attribute.
fn attribute_code(attribute: Attribute) -> &'static str {
    match attribute {
        Attribute::ReadOnly => "readonly",
        Attribute::Hidden => "hidden",
        Attribute::System => "system",
        Attribute::Archive => "archive",
        Attribute::Temporary => "temporary",
        Attribute::SparseFile => "sparse_file",
        Attribute::ReparsePoint => "reparse_point",
        Attribute::Compressed => "compressed",
        Attribute::Offline => "offline",
        Attribute::NotContentIndexed => "not_content_indexed",
        Attribute::Encrypted => "encrypted",
        Attribute::IntegrityStream => "integrity_stream",
        Attribute::NoScrubData => "no_scrub_data",
        Attribute::RecallOnOpen => "recall_on_open",
        Attribute::Pinned => "pinned",
        Attribute::Unpinned => "unpinned",
        Attribute::RecallOnDataAccess => "recall_on_data_access",
        Attribute::StrictlySequential => "strictly_sequential",
    }
}

/// The codes of the attributes set in `flags`, in the order the details name them, split
/// by spaces; that of a folder is not among them, as `kind` says it.
fn attribute_codes(flags: u32) -> String {
    let codes: Vec<&str> = Attribute::ALL
        .into_iter()
        .filter(|a| flags & a.bit() != 0)
        .map(attribute_code)
        .collect();
    codes.join(" ")
}

/// A version as four numbers, "1.2.3.4"; empty when the snapshot has none.
fn version(info: VersionInfo, of: fn(&revisor_core::scan::Version) -> [u16; 4]) -> String {
    match info {
        VersionInfo::Some(v) => {
            let [a, b, c, d] = of(&v);
            format!("{a}.{b}.{c}.{d}")
        }
        VersionInfo::NotRead | VersionInfo::None => String::new(),
    }
}

/// A link's target, with an apostrophe before it when it starts as a formula does in
/// Excel (#7); empty for an entry that is not a link or whose target is unknown.
fn link_target(entry: &Entry) -> String {
    match &entry.link_target {
        Some(target) if !target.is_empty() => {
            let target = text(target);
            if target.starts_with(['=', '+', '-', '@']) {
                format!("'{target}")
            } else {
                target
            }
        }
        _ => String::new(),
    }
}

/// A moment given as a FILETIME in local time as ISO 8601 writes it, with all seven
/// digits of the fraction of a second and the offset from UTC:
/// "2026-11-13 14:05:31.1234567+02:00" (#3).
pub(super) fn iso_time(file_time: i64, utc_offset: &dyn Fn(i64) -> Option<i32>) -> String {
    let offset = utc_offset(file_time).unwrap_or(0);
    let local = file_time.saturating_add(i64::from(offset) * 60 * TICKS_PER_SECOND);
    let days = local.div_euclid(TICKS_PER_DAY);
    let in_day = local.rem_euclid(TICKS_PER_DAY);
    let (year, month, day) = civil_from_days(days - DAYS_TO_UNIX_EPOCH);
    let seconds = in_day / TICKS_PER_SECOND;
    let fraction = in_day % TICKS_PER_SECOND;
    let sign = if offset < 0 { '-' } else { '+' };
    let offset = offset.unsigned_abs();
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02}.{fraction:07}{sign}{:02}:{:02}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60,
        offset / 60,
        offset % 60
    )
}

/// The date of the day `days` days after 1970-01-01 in the proleptic Gregorian calendar
/// (Howard Hinnant's `civil_from_days`).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}
