//! How names compare: without regard to case, the way NTFS compares them.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

/// Upper-cases one UTF-16 unit the way NTFS compares names: one unit to one unit, and
/// units with no single-unit upper case stay as they are.
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

/// Whether two names are the same regardless of case, the way NTFS compares them.
pub fn eq_ignore_case(a: &str, b: &str) -> bool {
    a.encode_utf16()
        .map(upcase)
        .eq(b.encode_utf16().map(upcase))
}

/// [`eq_ignore_case`] for names that need not be valid Unicode.
pub fn eq_ignore_case_os(a: &OsStr, b: &OsStr) -> bool {
    a.encode_wide().map(upcase).eq(b.encode_wide().map(upcase))
}
