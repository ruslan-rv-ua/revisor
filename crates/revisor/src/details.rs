//! The details of a row of the difference list (TASK-14), apart from the window: every
//! property of its entry before and after, a line each and in words, so a screen reader
//! reads them one at a time and the whole text can be copied (#2).
//!
//! The lines come in a fixed order (#3): the path, before and after for an entry renamed
//! or moved (TASK-42 #8); the name, when only its case changed (#4); the change with what
//! changed; the kind; the size of a file; the dates of creation
//! and of the last write; the attributes; the file and product versions of an exe or dll;
//! the target of a link; and, for an entry inaccessible in a snapshot, in which and why. A
//! property has one value when the entry is in one snapshot only, «було …, стало …» when
//! it differs, and one value marked «(без змін)» when it does not; a difference the
//! comparison does not count is marked «(не враховано)» (#5).

use std::ffi::OsStr;

use revisor_core::compare::Side;
use revisor_core::diff::{self, Fields};
use revisor_core::scan::{Entry, EntryKind, Version, VersionInfo, has_version_extension};
use revisor_core::settings::{AttributeFormat, SizeFormat};

use crate::difference::{InaccessibleIn, Regional, RowChange, format_size};
use crate::i18n::Strings;

/// A property of an entry, in the order the details show them (#3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Property {
    Path,
    /// The name, shown only when its case changed (#4).
    Name,
    Change,
    Kind,
    Size,
    DateCreated,
    DateModified,
    Attributes,
    FileVersion,
    ProductVersion,
    Target,
    Inaccessible,
    /// What changed inside a folder of the difference tree that is no change itself
    /// (TASK-33 #9): the line after its path.
    Inside,
}

/// Why an entry could not be read in a snapshot, as far as the snapshot knows it (decided
/// 2026-11-06): it keeps whether an entry is inaccessible, not the error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unreadable {
    /// A folder that could not be listed, or only in part.
    Folder,
    /// A link whose target could not be read.
    LinkTarget,
    /// The entry is not in the snapshot, whose folder on the way to it could not be read
    /// or is a link (TASK-10): whether it was there is unknown.
    Unknown,
}

/// What the line «Недоступно» says (#3): in which snapshot the entry is inaccessible, why,
/// and, for an entry in one snapshot only, that the other one does not have it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Inaccessibility {
    pub which: InaccessibleIn,
    pub why: Unreadable,
    pub absent_in: Option<Side>,
}

/// An attribute of an entry the details name (#7, #8), in the order they name them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Attribute {
    ReadOnly,
    Hidden,
    System,
    Archive,
    Temporary,
    SparseFile,
    ReparsePoint,
    Compressed,
    Offline,
    NotContentIndexed,
    Encrypted,
    IntegrityStream,
    NoScrubData,
    RecallOnOpen,
    Pinned,
    Unpinned,
    RecallOnDataAccess,
    StrictlySequential,
}

impl Attribute {
    pub const ALL: [Attribute; 18] = [
        Self::ReadOnly,
        Self::Hidden,
        Self::System,
        Self::Archive,
        Self::Temporary,
        Self::SparseFile,
        Self::ReparsePoint,
        Self::Compressed,
        Self::Offline,
        Self::NotContentIndexed,
        Self::Encrypted,
        Self::IntegrityStream,
        Self::NoScrubData,
        Self::RecallOnOpen,
        Self::Pinned,
        Self::Unpinned,
        Self::RecallOnDataAccess,
        Self::StrictlySequential,
    ];

    /// The `FILE_ATTRIBUTE_*` flag.
    pub const fn bit(self) -> u32 {
        match self {
            Self::ReadOnly => 0x1,
            Self::Hidden => 0x2,
            Self::System => 0x4,
            Self::Archive => 0x20,
            Self::Temporary => 0x100,
            Self::SparseFile => 0x200,
            Self::ReparsePoint => 0x400,
            Self::Compressed => 0x800,
            Self::Offline => 0x1000,
            Self::NotContentIndexed => 0x2000,
            Self::Encrypted => 0x4000,
            Self::IntegrityStream => 0x8000,
            Self::NoScrubData => 0x2_0000,
            Self::RecallOnOpen => 0x4_0000,
            Self::Pinned => 0x8_0000,
            Self::Unpinned => 0x10_0000,
            Self::RecallOnDataAccess => 0x40_0000,
            Self::StrictlySequential => 0x2000_0000,
        }
    }
}

const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
const FILE_ATTRIBUTE_NORMAL: u32 = 0x80;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

/// How the details write sizes and attributes, as the settings «Розмір» and «Атрибути» say
/// (#6, #7).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Formats {
    pub size: SizeFormat,
    pub attributes: AttributeFormat,
}

/// A row of the difference list as its details see it.
#[derive(Clone, Debug)]
pub struct Row<'a> {
    /// The whole path of the entry, as the list's columns show it.
    pub path: String,
    /// The path the entry had in the earlier snapshot, when it is at another one now: in
    /// another folder, or under a name that differs in more than case (TASK-42 #8).
    pub former_path: Option<String>,
    pub change: RowChange,
    /// The fields that differ, in the words of the column «Що змінилося», for a modified,
    /// renamed or moved row; empty otherwise.
    pub what_changed: String,
    /// The entry in the earlier and in the later snapshot; at least one of them.
    pub old: Option<&'a Entry>,
    pub new: Option<&'a Entry>,
    /// The fields of the entry that the comparison found different, as it counts them.
    pub fields: Fields,
    pub inaccessible: Option<Inaccessibility>,
}

/// The text of the details of `row`: a line per property, in the order of [`Property`].
pub fn text(s: &dyn Strings, regional: &dyn Regional, formats: Formats, row: &Row<'_>) -> String {
    let entry = row
        .new
        .or(row.old)
        .expect("a row has an entry in at least one snapshot");
    let ignored = match (row.old, row.new) {
        (Some(a), Some(b)) => diff::ignored_fields(a, b, a.inaccessible || b.inaccessible),
        _ => Fields::default(),
    };
    let values = Values {
        s,
        old: row.old,
        new: row.new,
        ignored,
    };
    let mut lines = Vec::new();
    let mut line = |property: Property, value: String| {
        lines.push(format!("{}: {value}", s.property(property)));
    };
    line(
        Property::Path,
        match &row.former_path {
            Some(former) => s.before_after(former, &row.path),
            None => row.path.clone(),
        },
    );
    // A name that differs in case only: the path says nothing of it to the ear.
    if let (Some(a), Some(b)) = (row.old, row.new)
        && row.fields.contains(Fields::NAME)
        && row.former_path.is_none()
    {
        line(
            Property::Name,
            s.before_after(&name(&a.name), &name(&b.name)),
        );
    }
    let mut change = s.change_type(row.change).to_string();
    if !row.what_changed.is_empty() {
        change.push_str("; ");
        change.push_str(&s.what_changed(&row.what_changed));
    }
    line(Property::Change, change);
    line(Property::Kind, s.kind(entry.kind).to_string());
    if entry.kind == EntryKind::File {
        line(
            Property::Size,
            values.of(Fields::SIZE, |e| size(s, regional, formats.size, e.size)),
        );
    }
    line(
        Property::DateCreated,
        values.dates(regional, Fields::CREATED, |e| e.created),
    );
    line(
        Property::DateModified,
        values.dates(regional, Fields::MODIFIED, |e| e.modified),
    );
    line(
        Property::Attributes,
        values.of(Fields::ATTRIBUTES, |e| {
            attributes(s, regional, formats.attributes, e.kind, e.attributes)
        }),
    );
    if entry.kind == EntryKind::File && has_version_extension(&entry.name) {
        line(
            Property::FileVersion,
            values.of(Fields::VERSION, |e| version(s, e.version, |v| v.file)),
        );
        line(
            Property::ProductVersion,
            values.of(Fields::VERSION, |e| version(s, e.version, |v| v.product)),
        );
    }
    if entry.kind == EntryKind::Link {
        line(
            Property::Target,
            values.of(Fields::LINK_TARGET, |e| match &e.link_target {
                Some(target) if !target.is_empty() => target.to_string_lossy().into_owned(),
                _ => s.target_unknown().to_string(),
            }),
        );
    }
    if let Some(inaccessible) = row.inaccessible {
        line(
            Property::Inaccessible,
            s.inaccessible_details(&inaccessible),
        );
    }
    lines.join("\n")
}

/// The values of the properties of an entry in the two snapshots.
struct Values<'a> {
    s: &'a dyn Strings,
    old: Option<&'a Entry>,
    new: Option<&'a Entry>,
    /// The fields that differ though the comparison does not count them.
    ignored: Fields,
}

impl Values<'_> {
    /// A property `field` as `show` writes it: one value for an entry in one snapshot only,
    /// and for one in both, the value marked unchanged when it is written the same, else
    /// before and after, marked when the comparison does not count the difference.
    fn of(&self, field: Fields, show: impl Fn(&Entry) -> String) -> String {
        match (self.old, self.new) {
            (Some(a), Some(b)) => {
                let (before, after) = (show(a), show(b));
                if before == after {
                    self.s.unchanged(&before)
                } else {
                    self.changed(field, &before, &after)
                }
            }
            (Some(entry), None) | (None, Some(entry)) => show(entry),
            (None, None) => unreachable!("a row has an entry in at least one snapshot"),
        }
    }

    /// A date `field` (#9): as a snapshot's name writes moments, and, when two different
    /// dates are written the same, both with as many digits of the fraction of a second as
    /// tell them apart.
    fn dates(
        &self,
        regional: &dyn Regional,
        field: Fields,
        date: impl Fn(&Entry) -> i64,
    ) -> String {
        match (self.old, self.new) {
            (Some(a), Some(b)) => {
                let (a, b) = (date(a), date(b));
                if a == b {
                    return self.s.unchanged(&regional.file_time(a));
                }
                let (mut before, mut after) = (regional.file_time(a), regional.file_time(b));
                if before == after {
                    let digits = fraction_digits(a, b);
                    before = regional.file_time_fraction(a, digits);
                    after = regional.file_time_fraction(b, digits);
                }
                self.changed(field, &before, &after)
            }
            (Some(entry), None) | (None, Some(entry)) => regional.file_time(date(entry)),
            (None, None) => unreachable!("a row has an entry in at least one snapshot"),
        }
    }

    fn changed(&self, field: Fields, before: &str, after: &str) -> String {
        let text = self.s.before_after(before, after);
        if self.ignored.contains(field) {
            self.s.not_counted(&text)
        } else {
            text
        }
    }
}

/// How many digits of the fraction of a second tell two moments of the same second apart:
/// the fewest, from 1 to all 7 of the 100-ns ticks NTFS keeps.
pub fn fraction_digits(a: i64, b: i64) -> u32 {
    const TICKS: i64 = 10_000_000;
    let (a, b) = (a.rem_euclid(TICKS), b.rem_euclid(TICKS));
    (1..=7)
        .find(|&digits| {
            let unit = 10i64.pow(7 - digits);
            a / unit != b / unit
        })
        .unwrap_or(7)
}

/// A name as the details write it.
fn name(name: &OsStr) -> String {
    name.to_string_lossy().into_owned()
}

/// A size as the setting «Розмір» writes it, and, when that is not in bytes, also in bytes
/// in parentheses, as the properties of a file in File Explorer (#6).
fn size(s: &dyn Strings, regional: &dyn Regional, format: SizeFormat, bytes: u64) -> String {
    let shown = format_size(bytes, format, s, regional);
    let exact = format_size(bytes, SizeFormat::Bytes, s, regional);
    if shown == exact {
        shown
    } else {
        format!("{shown} ({exact})")
    }
}

/// The attributes of an entry of `kind`, as the setting «Атрибути» writes them (#7, #8):
/// without the ones the kind repeats — the folder's, and a link's reparse point and folder
/// — and without «звичайний», which NTFS reports in place of none; «без атрибутів» when
/// none is left. In words, in the order of the flags; in letters, File Explorer's letters
/// as Windows gives them, run together as its column writes them or split by commas
/// (decided 2026-11-06), then, in words, the attributes it has no letter for. Flags
/// revisor has no word for are written as they are, in hexadecimal.
pub fn attributes(
    s: &dyn Strings,
    regional: &dyn Regional,
    format: AttributeFormat,
    kind: EntryKind,
    flags: u32,
) -> String {
    let mut hidden = FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_NORMAL;
    if kind == EntryKind::Link {
        hidden |= FILE_ATTRIBUTE_REPARSE_POINT;
    }
    let flags = flags & !hidden;
    if flags == 0 {
        return s.no_attributes().to_string();
    }
    let mut letters = Vec::new();
    let mut words = Vec::new();
    let mut unknown = 0;
    for bit in (0..32).map(|i| 1u32 << i).filter(|bit| flags & bit != 0) {
        let letter = match format {
            AttributeFormat::Words => None,
            AttributeFormat::Letters | AttributeFormat::LettersWithCommas => {
                regional.attribute_letter(bit)
            }
        };
        match (letter, Attribute::ALL.into_iter().find(|a| a.bit() == bit)) {
            (Some(letter), _) => letters.push(letter),
            (None, Some(attribute)) => words.push(s.attribute(attribute).to_string()),
            (None, None) => unknown |= bit,
        }
    }
    let mut parts = Vec::new();
    if !letters.is_empty() {
        parts.push(match format {
            AttributeFormat::LettersWithCommas => letters.join(", "),
            _ => letters.concat(),
        });
    }
    parts.extend(words);
    if unknown != 0 {
        parts.push(format!("0x{unknown:08X}"));
    }
    parts.join(", ")
}

/// A version as four numbers, or what the snapshot knows instead (#10); `part` picks the
/// file or the product version.
fn version(s: &dyn Strings, info: VersionInfo, part: impl Fn(&Version) -> [u16; 4]) -> String {
    match info {
        VersionInfo::Some(version) => {
            let [a, b, c, d] = part(&version);
            format!("{a}.{b}.{c}.{d}")
        }
        VersionInfo::None => s.no_version().to_string(),
        VersionInfo::NotRead => s.version_not_read().to_string(),
    }
}

#[cfg(test)]
mod tests;
