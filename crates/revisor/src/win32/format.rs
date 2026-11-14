//! Counts, numbers, moments and sizes as a locale writes them, and the letters of
//! attributes as File Explorer writes them. The application gives [`Locale::USER`], the
//! user's regional format in Windows — not the interface language; the tests give named
//! locales, so they hold the exact strings (TASK-32).

use std::cell::RefCell;
use std::ptr;

use windows_sys::Win32::Foundation::{FILETIME, PROPERTYKEY, SYSTEMTIME};
use windows_sys::Win32::Globalization::{
    DATE_SHORTDATE, GetDateFormatEx, GetLocaleInfoEx, GetNumberFormatEx, GetTimeFormatEx,
    LOCALE_NOUSEROVERRIDE, LOCALE_SDECIMAL, LOCALE_SGROUPING, LOCALE_SLIST, LOCALE_STHOUSAND,
    LOCALE_STIMEFORMAT, NUMBERFMTW,
};
use windows_sys::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows_sys::Win32::System::Time::{
    FileTimeToSystemTime, SystemTimeToFileTime, SystemTimeToTzSpecificLocalTime,
};
use windows_sys::Win32::System::Variant::VT_UI4;
use windows_sys::Win32::UI::Shell::PropertiesSystem::{PDFF_DEFAULT, PSFormatForDisplay};
use windows_sys::Win32::UI::Shell::StrFormatByteSizeW;
use windows_sys::core::GUID;

use super::wide;
use crate::difference::Regional;

/// The locale whose format numbers and moments are written in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Locale {
    /// The locale's name, "uk-UA"; `None` for the user's default locale.
    name: Option<&'static str>,
}

impl Locale {
    /// The user's regional format, with the changes the user made to it in Windows: what
    /// revisor shows.
    pub const USER: Locale = Locale { name: None };

    /// The standard format of the locale `name`, "uk-UA" or "en-US", without the user's
    /// changes, so a test gets the same strings on any computer.
    #[cfg(test)]
    pub const fn standard(name: &'static str) -> Self {
        Self { name: Some(name) }
    }

    /// The name as the calls take it: NUL-terminated, or `None` for a null pointer.
    fn wide_name(self) -> Option<Vec<u16>> {
        self.name.map(wide)
    }

    /// The flag that keeps a named locale from the user's changes to it.
    fn no_override(self) -> u32 {
        if self.name.is_some() {
            LOCALE_NOUSEROVERRIDE
        } else {
            0
        }
    }
}

/// A pointer to `name`, or null for none.
fn name_ptr(name: &Option<Vec<u16>>) -> *const u16 {
    name.as_ref().map_or(ptr::null(), |n| n.as_ptr())
}

/// A count in the format of `locale`, with its digit grouping: "12,345" or "12 345".
/// Plain digits when the format cannot be read.
pub fn format_count(locale: Locale, count: u64) -> String {
    format_number(locale, &count.to_string(), 0)
}

/// A number written in ASCII digits with a dot before its `decimals` decimals, in the
/// format of `locale`: grouped digits and the locale's decimal separator, "1,17" for
/// "1.17" (TASK-13). As written when the format cannot be read.
pub fn format_number(locale: Locale, digits: &str, decimals: u32) -> String {
    let plain = digits.to_string();
    // The strings keep their terminating NUL, which the format needs too.
    let (Some(mut thousand), Some(mut decimal), Some(grouping)) = (
        locale_info(locale, LOCALE_STHOUSAND),
        locale_info(locale, LOCALE_SDECIMAL),
        locale_info(locale, LOCALE_SGROUPING),
    ) else {
        return plain;
    };
    let format = NUMBERFMTW {
        NumDigits: decimals,
        LeadingZero: 1,
        Grouping: grouping_field(&String::from_utf16_lossy(&grouping)),
        lpDecimalSep: decimal.as_mut_ptr(),
        lpThousandSep: thousand.as_mut_ptr(),
        NegativeOrder: 1,
    };
    let name = locale.wide_name();
    let value = wide(&plain);
    let mut out = [0u16; 64];
    // SAFETY: every string is NUL-terminated, the name is null for the user's locale, and
    // the output length is passed.
    let len = unsafe {
        GetNumberFormatEx(
            name_ptr(&name),
            0,
            value.as_ptr(),
            &format,
            out.as_mut_ptr(),
            out.len() as i32,
        )
    };
    if len <= 1 {
        return plain;
    }
    String::from_utf16_lossy(&out[..len as usize - 1])
}

/// A moment given as Unix time in milliseconds, in the short date and the time with
/// seconds of `locale`, in this computer's time zone: "31.10.2026 14:05:12" or
/// "10/31/2026 2:05:12 PM" (TASK-9). A moment Windows cannot convert, or a format it
/// cannot give, falls back to the UTC time in ISO form, so a name is never empty.
pub fn format_date_time(locale: Locale, unix_millis: i64) -> String {
    let file_time = unix_millis
        .checked_mul(10_000)
        .and_then(|i| i.checked_add(EPOCH_DIFFERENCE))
        .filter(|&i| i >= 0);
    match file_time {
        Some(file_time) => format_file_time(locale, file_time),
        None => iso_utc(unix_millis),
    }
}

/// A moment given as Unix time in milliseconds, in this computer's time zone, as a name
/// of a file can hold it, with no colons: "2026-11-13 14-05-33" (TASK-39 #2). A moment
/// Windows cannot convert is in UTC.
pub fn file_name_time(unix_millis: i64) -> String {
    let local = unix_millis
        .checked_mul(10_000)
        .and_then(|i| i.checked_add(EPOCH_DIFFERENCE))
        .and_then(local_time);
    match local {
        Some(t) => format!(
            "{:04}-{:02}-{:02} {:02}-{:02}-{:02}",
            t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond
        ),
        None => iso_utc(unix_millis).replace(':', "-").replace(" UTC", ""),
    }
}

/// 100-ns intervals between 1601-01-01 and 1970-01-01.
const EPOCH_DIFFERENCE: i64 = 116_444_736_000_000_000;

/// A moment given as a FILETIME — 100-ns intervals since 1601-01-01 UTC, as entries keep
/// their times — the way [`format_date_time`] writes it (TASK-13 #18).
pub fn format_file_time(locale: Locale, intervals: i64) -> String {
    local_time(intervals)
        .and_then(|time| format_local(locale, &time, None))
        .unwrap_or_else(|| iso_utc(unix_millis_of(intervals)))
}

/// A moment as [`format_file_time`] writes it, with `digits` digits, 1 to 7, of the
/// fraction of the second after the seconds, in the locale's decimal separator:
/// "06.11.2026 0:24:40,1" (TASK-14 #9). The fraction goes into the locale's own time
/// format, right after its seconds, so "2:05:12,1 PM" keeps its AM and PM.
pub fn format_file_time_fraction(locale: Locale, intervals: i64, digits: u32) -> String {
    let ticks = format!("{:07}", intervals.rem_euclid(10_000_000));
    let fraction = &ticks[..digits.clamp(1, 7) as usize];
    local_time(intervals)
        .and_then(|time| format_local(locale, &time, Some(fraction)))
        .unwrap_or_else(|| {
            iso_utc(unix_millis_of(intervals)).replace(" UTC", &format!(".{fraction} UTC"))
        })
}

/// Unix time in milliseconds of a FILETIME.
fn unix_millis_of(intervals: i64) -> i64 {
    intervals
        .saturating_sub(EPOCH_DIFFERENCE)
        .div_euclid(10_000)
}

/// A string of `locale` of the kind `kind`, with its terminating NUL.
fn locale_info(locale: Locale, kind: u32) -> Option<Vec<u16>> {
    let name = locale.wide_name();
    let mut buf = [0u16; 128];
    // SAFETY: the name is NUL-terminated or null for the user's locale; the buffer length
    // is passed.
    let len = unsafe {
        GetLocaleInfoEx(
            name_ptr(&name),
            kind | locale.no_override(),
            buf.as_mut_ptr(),
            buf.len() as i32,
        )
    };
    (len > 0).then(|| buf[..len as usize].to_vec())
}

/// A string of `locale` of the kind `kind`, without its terminating NUL.
fn locale_text(locale: Locale, kind: u32) -> Option<String> {
    locale_info(locale, kind).map(|s| String::from_utf16_lossy(&s[..s.len() - 1]))
}

/// The time format picture `picture` with `literal` right after its seconds, as quoted
/// text; at its end when it has no seconds. Text in quotes is not taken for seconds.
fn after_seconds(picture: &str, literal: &str) -> String {
    let quoted = format!("'{}'", literal.replace('\'', "''"));
    let mut in_quotes = false;
    let mut end = None;
    let mut chars = picture.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        match c {
            '\'' => in_quotes = !in_quotes,
            's' if !in_quotes => {
                let mut at = i + 1;
                while let Some(&(j, 's')) = chars.peek() {
                    at = j + 1;
                    chars.next();
                }
                end = Some(at);
            }
            _ => {}
        }
    }
    let at = end.unwrap_or(picture.len());
    format!("{}{quoted}{}", &picture[..at], &picture[at..])
}

/// The list separator and the decimal separator of `locale` (TASK-40 #5), which choose
/// the delimiter of CSV «as in Windows»; `None` for one that cannot be read.
pub fn list_and_decimal_separators(locale: Locale) -> (Option<String>, Option<String>) {
    (
        locale_text(locale, LOCALE_SLIST),
        locale_text(locale, LOCALE_SDECIMAL),
    )
}

/// The offset from UTC, in minutes, of this computer's time zone at the moment given as a
/// FILETIME, as revisor writes the moments of CSV (TASK-40 #3); `None` when Windows cannot
/// convert it.
pub fn utc_offset_minutes(intervals: i64) -> Option<i32> {
    let local = local_time(intervals)?;
    let mut back = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    // SAFETY: valid pointers to a SYSTEMTIME and a FILETIME.
    if unsafe { SystemTimeToFileTime(&local, &mut back) } == 0 {
        return None;
    }
    let local = (i64::from(back.dwHighDateTime) << 32) | i64::from(back.dwLowDateTime);
    // A SYSTEMTIME holds milliseconds, so the offset is what is left after them.
    let utc_millis = intervals.div_euclid(10_000);
    let minutes = (local.div_euclid(10_000) - utc_millis).div_euclid(60_000);
    i32::try_from(minutes).ok()
}

/// A moment given as a FILETIME in this computer's time zone; `None` when Windows cannot
/// convert it.
fn local_time(intervals: i64) -> Option<SYSTEMTIME> {
    if intervals < 0 {
        return None;
    }
    let file_time = FILETIME {
        dwLowDateTime: intervals as u32,
        dwHighDateTime: (intervals >> 32) as u32,
    };
    // SAFETY: all-zero SYSTEMTIMEs are valid out values; the calls fill them.
    let mut utc: SYSTEMTIME = unsafe { std::mem::zeroed() };
    let mut local: SYSTEMTIME = unsafe { std::mem::zeroed() };
    // SAFETY: valid pointers; a null time zone means this computer's current one.
    let converted = unsafe {
        FileTimeToSystemTime(&file_time, &mut utc) != 0
            && SystemTimeToTzSpecificLocalTime(ptr::null(), &utc, &mut local) != 0
    };
    converted.then_some(local)
}

/// A local time in the short date and the time of `locale`, with `fraction`, the digits
/// of the fraction of the second, right after the seconds when given; `None` when Windows
/// cannot format it.
fn format_local(locale: Locale, time: &SYSTEMTIME, fraction: Option<&str>) -> Option<String> {
    // The locale's own time format, with the fraction in the locale's decimal separator.
    let picture = match fraction {
        None => None,
        Some(fraction) => {
            let separator = locale_text(locale, LOCALE_SDECIMAL).unwrap_or_else(|| ".".into());
            let picture = locale_text(locale, LOCALE_STIMEFORMAT)?;
            Some(wide(&after_seconds(
                &picture,
                &format!("{separator}{fraction}"),
            )))
        }
    };
    let name = locale.wide_name();
    // The flag that keeps a named locale's own formats goes only with no picture.
    let time_flags = if picture.is_none() {
        locale.no_override()
    } else {
        0
    };
    let mut date = [0u16; 128];
    let mut text = [0u16; 128];
    // SAFETY: the name is NUL-terminated or null for the user's locale; the buffer lengths
    // are passed, the picture is NUL-terminated or null for the locale's own format, and
    // the calendar is the locale's own.
    let (date_len, time_len) = unsafe {
        (
            GetDateFormatEx(
                name_ptr(&name),
                DATE_SHORTDATE | locale.no_override(),
                time,
                ptr::null(),
                date.as_mut_ptr(),
                date.len() as i32,
                ptr::null(),
            ),
            GetTimeFormatEx(
                name_ptr(&name),
                time_flags,
                time,
                name_ptr(&picture),
                text.as_mut_ptr(),
                text.len() as i32,
            ),
        )
    };
    // The lengths count the terminating NUL.
    if date_len <= 1 || time_len <= 1 {
        return None;
    }
    Some(format!(
        "{} {}",
        String::from_utf16_lossy(&date[..date_len as usize - 1]),
        String::from_utf16_lossy(&text[..time_len as usize - 1])
    ))
}

/// "2026-10-31 12:05:12 UTC" for a Unix time in milliseconds, with no calendar library:
/// the civil-from-days algorithm of Howard Hinnant.
fn iso_utc(unix_millis: i64) -> String {
    let seconds = unix_millis.div_euclid(1000);
    let days = seconds.div_euclid(86_400);
    let of_day = seconds.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02} UTC",
        of_day / 3600,
        of_day % 3600 / 60,
        of_day % 60
    )
}

/// System.FileAttributes, the shell property of the column «Атрибути» of File Explorer.
const PKEY_FILE_ATTRIBUTES: PROPERTYKEY = PROPERTYKEY {
    fmtid: GUID::from_u128(0xB725F130_47EF_101A_A5F1_02608C9EEBAC),
    pid: 13,
};

thread_local! {
    /// The letters Windows gave for each attribute flag so far. A cache only, kept per
    /// thread as the window's thread is the one that asks, so it needs no lock; the shell
    /// gives the same letters to any thread (TASK-32).
    static ATTRIBUTE_LETTERS: RefCell<Vec<(u32, Option<String>)>> =
        const { RefCell::new(Vec::new()) };
}

/// The letter the column «Атрибути» of File Explorer writes for the attribute `flag`, a
/// single `FILE_ATTRIBUTE_*` bit, as this Windows writes it (TASK-14 #7, decided
/// 2026-11-06): the shell formats the column's property for display, in the language of
/// Windows, so it takes no locale; `None` for a flag it writes no letter for, or when the
/// shell cannot say.
pub fn attribute_letter(flag: u32) -> Option<String> {
    if let Some(known) = ATTRIBUTE_LETTERS.with(|letters| {
        letters
            .borrow()
            .iter()
            .find(|(f, _)| *f == flag)
            .map(|(_, letter)| letter.clone())
    }) {
        return known;
    }
    // A zeroed PROPVARIANT is VT_EMPTY; its type and its 32-bit value make it a VT_UI4.
    let mut value = PROPVARIANT::default();
    value.Anonymous.Anonymous.vt = VT_UI4;
    value.Anonymous.Anonymous.Anonymous.ulVal = flag;
    let mut text = [0u16; 32];
    // SAFETY: the key and the value are valid for the call, and the buffer length is passed.
    let hr = unsafe {
        PSFormatForDisplay(
            &PKEY_FILE_ATTRIBUTES,
            &value,
            PDFF_DEFAULT,
            text.as_mut_ptr(),
            text.len() as u32,
        )
    };
    let len = text.iter().position(|&c| c == 0).unwrap_or(text.len());
    let letter = (hr >= 0 && len > 0).then(|| String::from_utf16_lossy(&text[..len]));
    ATTRIBUTE_LETTERS.with(|letters| letters.borrow_mut().push((flag, letter.clone())));
    letter
}

/// The formats of a locale, for the difference list and the details (TASK-13 #18), and
/// the letters of the attributes as File Explorer writes them (TASK-14 #7). The window
/// gives [`Locale::USER`].
pub struct RegionalFormat(pub Locale);

impl Regional for RegionalFormat {
    fn number(&self, digits: &str, decimals: u32) -> String {
        format_number(self.0, digits, decimals)
    }

    fn file_time(&self, file_time: i64) -> String {
        format_file_time(self.0, file_time)
    }

    fn file_time_fraction(&self, file_time: i64, digits: u32) -> String {
        format_file_time_fraction(self.0, file_time, digits)
    }

    fn date_time(&self, unix_millis: i64) -> String {
        format_date_time(self.0, unix_millis)
    }

    fn attribute_letter(&self, flag: u32) -> Option<String> {
        attribute_letter(flag)
    }
}

/// A size in bytes as File Explorer shows it: "1.5 MB" in the user's format; plain bytes
/// when the format cannot be given. `StrFormatByteSizeW` takes no locale — the units come
/// from the language of Windows and the separator from the user's region — so neither
/// does this; the difference list and the details write sizes with
/// `difference::format_size` in a [`Locale`] (decided 2026-11-07, TASK-32).
pub fn format_size(bytes: u64) -> String {
    let mut out = [0u16; 64];
    // SAFETY: the buffer length is passed.
    let written = unsafe {
        StrFormatByteSizeW(
            bytes.min(i64::MAX as u64) as i64,
            out.as_mut_ptr(),
            out.len() as u32,
        )
    };
    if written.is_null() {
        return bytes.to_string();
    }
    let len = out.iter().position(|&c| c == 0).unwrap_or(out.len());
    String::from_utf16_lossy(&out[..len])
}

/// The `Grouping` field of NUMBERFMTW from LOCALE_SGROUPING: "3;0" (groups of three,
/// repeating) is 3, "3;2;0" (Indic) is 32, and a list without the final 0 groups once,
/// so "3" is 30.
fn grouping_field(sgrouping: &str) -> u32 {
    let parts: Vec<&str> = sgrouping.trim_end_matches('\0').split(';').collect();
    let repeating = parts.last() == Some(&"0");
    let digits = parts
        .iter()
        .take(if repeating {
            parts.len() - 1
        } else {
            parts.len()
        })
        .filter_map(|p| p.parse::<u32>().ok())
        .fold(0, |acc, g| acc * 10 + g);
    if repeating { digits } else { digits * 10 }
}

#[cfg(test)]
mod tests;
