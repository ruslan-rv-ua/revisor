//! The interface language of Windows and the order of names by the user's alphabet.

use windows_sys::Win32::Globalization::GetUserDefaultUILanguage;
use windows_sys::Win32::UI::Shell::StrCmpLogicalW;

use super::wide;
use crate::i18n::Lang;

/// The interface language of Windows: Ukrainian gives Ukrainian, any other English.
pub fn ui_language() -> Lang {
    const LANG_UKRAINIAN: u16 = 0x22;
    // SAFETY: no arguments; it only reads the user's setting.
    let lang_id = unsafe { GetUserDefaultUILanguage() };
    if lang_id & 0x3ff == LANG_UKRAINIAN {
        Lang::Uk
    } else {
        Lang::En
    }
}

/// Orders two names as File Explorer does: by the user's alphabet regardless of case, with
/// numbers in names compared by their value ("Profile 2" before "Profile 10").
pub fn compare_names(a: &str, b: &str) -> std::cmp::Ordering {
    let (a, b) = (wide(a), wide(b));
    // SAFETY: both strings are NUL-terminated.
    unsafe { StrCmpLogicalW(a.as_ptr(), b.as_ptr()) }.cmp(&0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    #[test]
    fn names_are_ordered_as_file_explorer_orders_them() {
        assert_eq!(compare_names("профіль 2", "Профіль 10"), Ordering::Less);
        assert_eq!(compare_names("Дім", "дім"), Ordering::Equal);
        assert_eq!(compare_names("Home", "home 1"), Ordering::Less);
    }
}
