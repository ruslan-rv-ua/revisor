//! Paths and names as NTFS compares them: the `\\?\` prefix put on and taken off, the parts
//! of a path, and names folded so that two names NTFS takes for the same one fold equal.
//!
//! This module works with the text of a path only: it opens no files and asks Windows
//! nothing, so whatever needs the disk — where a folder really is, what an absolute path
//! is — happens before it is called. It has one behaviour for every caller (TASK-29):
//!
//! - the `\\?\` prefix is recognised in any case, and only [`plain`] takes it off;
//! - `\\?\UNC\server\share` and `\\server\share` have the same parts;
//! - a name that is not valid Unicode keeps its unpaired surrogates, never replaced.

use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

const VERBATIM: &str = r"\\?\";
const VERBATIM_UNC: &str = r"\\?\UNC\";
const DEVICE: &str = r"\\.\";
const UNC: &str = r"\\";
const SEPARATOR: u16 = b'\\' as u16;

/// An absolute path in the `\\?\` form, so paths longer than MAX_PATH work: `C:\x` is
/// `\\?\C:\x`, `\\server\share` is `\\?\UNC\server\share`, and a path that starts with
/// `\\?\` or `\\.\` stays as it is.
///
/// Windows takes a `\\?\` path as it is, without resolving `..` or `.`, so the path must be
/// absolute already: [`std::path::absolute`] makes it so.
pub fn verbatim(absolute: &Path) -> PathBuf {
    debug_assert!(absolute.is_absolute(), "{absolute:?}");
    let wide = wide(absolute.as_os_str());
    if strip(&wide, VERBATIM).is_some() || strip(&wide, DEVICE).is_some() {
        return absolute.to_path_buf();
    }
    let out: Vec<u16> = match strip(&wide, UNC) {
        Some(rest) => VERBATIM_UNC
            .encode_utf16()
            .chain(rest.iter().copied())
            .collect(),
        None => VERBATIM
            .encode_utf16()
            .chain(wide.iter().copied())
            .collect(),
    };
    PathBuf::from(OsString::from_wide(&out))
}

/// A path without its `\\?\` prefix, the way it is shown: `\\?\C:\x` is `C:\x`, and
/// `\\?\UNC\server\share` is `\\server\share`. Any other path stays as it is.
pub fn plain(path: &Path) -> PathBuf {
    let wide = wide(path.as_os_str());
    let out: Vec<u16> = if let Some(rest) = strip(&wide, VERBATIM_UNC) {
        UNC.encode_utf16().chain(rest.iter().copied()).collect()
    } else if let Some(rest) = strip(&wide, VERBATIM) {
        rest.to_vec()
    } else {
        return path.to_path_buf();
    };
    PathBuf::from(OsString::from_wide(&out))
}

/// The parts of a path as written, without its `\\?\` prefix and without empty parts:
/// `\\?\UNC\server\share\x` and `\\server\share\x` both have `server`, `share` and `x`.
pub fn split(path: &OsStr) -> Vec<OsString> {
    wide(plain(Path::new(path)).as_os_str())
        .split(|&unit| unit == SEPARATOR)
        .filter(|part| !part.is_empty())
        .map(OsString::from_wide)
        .collect()
}

/// The parts of a path ([`split`]), each folded: two paths name the same file or folder
/// when their parts are equal.
pub fn parts(path: &OsStr) -> Vec<Folded> {
    split(path).iter().map(|part| fold(part)).collect()
}

/// A name folded the way NTFS compares names: its characters as code points, upper-cased
/// one UTF-16 unit to one unit; an unpaired surrogate stays as it is. A whole character per
/// item, so a `?` of a pattern stands for one character even outside the BMP.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Folded(Vec<u32>);

impl Folded {
    /// The folded characters.
    pub fn chars(&self) -> &[u32] {
        &self.0
    }
}

/// Folds a name: see [`Folded`].
pub fn fold(name: &OsStr) -> Folded {
    Folded(folded_chars(name.encode_wide()).collect())
}

/// The characters of UTF-16 text, folded as in [`Folded`].
fn folded_chars(units: impl Iterator<Item = u16>) -> impl Iterator<Item = u32> {
    char::decode_utf16(units).map(|unit| match unit {
        Ok(c) => match u16::try_from(u32::from(c)) {
            Ok(unit) => upcase(unit).into(),
            Err(_) => c.into(),
        },
        Err(e) => e.unpaired_surrogate().into(),
    })
}

/// Upper-cases one UTF-16 unit the way NTFS compares names: one unit to one unit, and
/// units with no single-unit upper case stay as they are. Folding a name ([`fold`]) does
/// this to each of its characters inside the BMP.
pub fn upcase(unit: u16) -> u16 {
    if unit < 0x80 {
        return (unit as u8).to_ascii_uppercase().into();
    }
    let Some(c) = char::from_u32(unit.into()) else {
        return unit;
    };
    let mut upper = c.to_uppercase();
    match (upper.next(), upper.next()) {
        (Some(u), None) if (u as u32) < 0x1_0000 => u as u16,
        _ => unit,
    }
}

/// Whether two names are the same regardless of case: whether they fold equal.
pub fn eq_ignore_case(a: &str, b: &str) -> bool {
    folded_chars(a.encode_utf16()).eq(folded_chars(b.encode_utf16()))
}

/// [`eq_ignore_case`] for names that need not be valid Unicode.
pub fn eq_ignore_case_os(a: &OsStr, b: &OsStr) -> bool {
    folded_chars(a.encode_wide()).eq(folded_chars(b.encode_wide()))
}

fn wide(text: &OsStr) -> Vec<u16> {
    text.encode_wide().collect()
}

/// The rest of `wide` after `prefix`, which matches in any case.
fn strip<'a>(wide: &'a [u16], prefix: &str) -> Option<&'a [u16]> {
    let prefix: Vec<u16> = prefix.encode_utf16().collect();
    let head = wide.get(..prefix.len())?;
    head.iter()
        .zip(&prefix)
        .all(|(&a, &b)| upcase(a) == b)
        .then(|| &wide[prefix.len()..])
}

#[cfg(test)]
mod tests;
