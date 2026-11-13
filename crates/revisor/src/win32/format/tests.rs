use super::*;
use crate::difference::format_size as list_size;
use crate::i18n::{En, Uk};
use revisor_core::settings::SizeFormat;

const UK: Locale = Locale::standard("uk-UA");
const EN: Locale = Locale::standard("en-US");
/// The group separator of uk-UA: a no-break space.
const NBSP: char = '\u{a0}';

/// A local time, as `local_time` gives it.
fn time(year: u16, month: u16, day: u16, hour: u16, minute: u16, second: u16) -> SYSTEMTIME {
    SYSTEMTIME {
        wYear: year,
        wMonth: month,
        wDayOfWeek: 0,
        wDay: day,
        wHour: hour,
        wMinute: minute,
        wSecond: second,
        wMilliseconds: 0,
    }
}

#[test]
fn counts_and_numbers_are_written_exactly_as_the_locale_writes_them() {
    assert_eq!(format_count(UK, 1_234_567), format!("1{NBSP}234{NBSP}567"));
    assert_eq!(format_count(EN, 1_234_567), "1,234,567");
    assert_eq!(format_count(UK, 999), "999");
    assert_eq!(format_count(EN, 0), "0");
    // The decimal separator, and the decimals asked for.
    assert_eq!(format_number(UK, "1.17", 2), "1,17");
    assert_eq!(format_number(EN, "1.17", 2), "1.17");
    assert_eq!(format_number(UK, "1234.5", 1), format!("1{NBSP}234,5"));
    assert_eq!(format_number(EN, "1234.5", 1), "1,234.5");
}

#[test]
fn a_moment_is_written_in_the_short_date_and_the_time_with_seconds_of_the_locale() {
    let afternoon = time(2026, 10, 31, 14, 5, 12);
    let morning = time(2026, 1, 5, 9, 5, 7);
    let at = |locale, time: &SYSTEMTIME| format_local(locale, time, None).unwrap();
    assert_eq!(at(UK, &afternoon), "31.10.2026 14:05:12");
    assert_eq!(at(EN, &afternoon), "10/31/2026 2:05:12 PM");
    assert_eq!(at(UK, &morning), "05.01.2026 9:05:07");
    assert_eq!(at(EN, &morning), "1/5/2026 9:05:07 AM");
}

#[test]
fn the_fraction_of_a_second_follows_the_seconds_in_the_locales_separator() {
    let afternoon = time(2026, 10, 31, 14, 5, 12);
    let at = |locale, fraction| format_local(locale, &afternoon, Some(fraction)).unwrap();
    assert_eq!(at(UK, "1"), "31.10.2026 14:05:12,1");
    assert_eq!(at(UK, "1234567"), "31.10.2026 14:05:12,1234567");
    // Before AM and PM, which stay where the locale puts them.
    assert_eq!(at(EN, "1"), "10/31/2026 2:05:12.1 PM");
    assert_eq!(at(EN, "25"), "10/31/2026 2:05:12.25 PM");
}

#[test]
fn sizes_in_the_lists_and_the_details_are_written_exactly_in_the_locale() {
    let uk = |bytes, format| list_size(bytes, format, &Uk, &RegionalFormat(UK));
    let en = |bytes, format| list_size(bytes, format, &En, &RegionalFormat(EN));
    assert_eq!(uk(1_234_567, SizeFormat::Explorer), "1,17 МБ");
    assert_eq!(uk(1_234_567, SizeFormat::Decimal), "1,23 МБ");
    assert_eq!(
        uk(1_234_567, SizeFormat::Bytes),
        format!("1{NBSP}234{NBSP}567 байтів")
    );
    assert_eq!(en(1_234_567, SizeFormat::Explorer), "1.17 MB");
    assert_eq!(en(1_234_567, SizeFormat::Decimal), "1.23 MB");
    assert_eq!(en(1_234_567, SizeFormat::Bytes), "1,234,567 bytes");
    assert_eq!(uk(15_360, SizeFormat::Explorer), "15,0 КБ");
    assert_eq!(en(15_360, SizeFormat::Explorer), "15.0 KB");
}

#[test]
fn counts_keep_their_digits_in_the_users_format() {
    // The separator depends on the user's region; the digits do not.
    let digits = |text: String| -> String { text.chars().filter(char::is_ascii_digit).collect() };
    let count = |n| format_count(Locale::USER, n);
    assert_eq!(count(0), "0");
    assert_eq!(count(7), "7");
    assert_eq!(digits(count(1_234_567)), "1234567");
    assert!(count(1_234_567).len() > 7, "the thousands are grouped");
}

#[test]
fn a_snapshot_name_is_the_local_time_in_the_users_format() {
    // 2026-10-31 12:05:12 UTC; the local hour depends on this computer's time zone.
    let unix_millis = 1_793_448_312_000;
    let file_time = unix_millis * 10_000 + EPOCH_DIFFERENCE;
    let name = format_date_time(Locale::USER, unix_millis);
    let local = local_time(file_time).expect("the moment converts");
    assert_eq!(
        Some(name.clone()),
        format_local(Locale::USER, &local, None),
        "{name}"
    );
    let later = format_date_time(Locale::USER, unix_millis + 1000);
    assert_ne!(name, later, "seconds tell two snapshots of a minute apart");
    // The same moment as a FILETIME.
    assert_eq!(format_file_time(Locale::USER, file_time), name);
    assert_eq!(iso_utc(unix_millis), "2026-10-31 12:05:12 UTC");
    assert_eq!(iso_utc(0), "1970-01-01 00:00:00 UTC");
    assert_eq!(iso_utc(-1000), "1969-12-31 23:59:59 UTC");
    // A moment before 1601 cannot be a FILETIME; the ISO form stands in.
    assert_eq!(
        format_date_time(UK, -20_000_000_000_000),
        "1336-03-23 12:26:40 UTC"
    );
    assert_eq!(format_file_time(EN, -1), "1600-12-31 23:59:59 UTC");
}

#[test]
fn a_file_time_with_a_fraction_is_its_local_time_with_the_digits_asked_for() {
    // 2026-10-31 12:05:12.1234567 UTC.
    let file_time = 1_793_448_312_000 * 10_000 + EPOCH_DIFFERENCE + 1_234_567;
    let local = local_time(file_time).expect("the moment converts");
    for locale in [UK, EN, Locale::USER] {
        assert_eq!(
            Some(format_file_time_fraction(locale, file_time, 1)),
            format_local(locale, &local, Some("1"))
        );
        assert_eq!(
            Some(format_file_time_fraction(locale, file_time, 7)),
            format_local(locale, &local, Some("1234567"))
        );
    }
    // A moment Windows cannot convert keeps its fraction in the ISO form.
    assert_eq!(
        format_file_time_fraction(UK, -1, 3),
        "1600-12-31 23:59:59.999 UTC"
    );
}

#[test]
fn windows_gives_the_letters_of_the_column_of_file_explorer() {
    use windows_sys::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
    // SAFETY: the test's thread joins an apartment for the shell's property system, as
    // the window's thread has.
    unsafe { CoInitializeEx(ptr::null(), COINIT_APARTMENTTHREADED as u32) };
    // The flags Microsoft documents a letter for: R, S, A, C, O and I (PSFormatForDisplay,
    // whose example 0x1801 is "RCO"). Their letters are one each, whatever the language.
    for flag in [0x1, 0x4, 0x20, 0x800, 0x1000, 0x2000] {
        let letter = attribute_letter(flag);
        assert!(
            letter.as_ref().is_some_and(|l| l.chars().count() == 1),
            "{flag:#x}: {letter:?}"
        );
        // Asked again, the same.
        assert_eq!(attribute_letter(flag), letter);
    }
}

#[test]
fn the_fraction_of_a_second_goes_right_after_the_seconds() {
    assert_eq!(after_seconds("H:mm:ss", ",1"), "H:mm:ss',1'");
    assert_eq!(after_seconds("h:mm:ss tt", ".25"), "h:mm:ss'.25' tt");
    assert_eq!(after_seconds("H:m:s", ",5"), "H:m:s',5'");
    // An s in quotes is text, not the seconds.
    assert_eq!(
        after_seconds("HH' h 'mm' min 'ss' s'", ",3"),
        "HH' h 'mm' min 'ss',3'' s'"
    );
    // Without seconds, at the end; a quote in the text is doubled.
    assert_eq!(after_seconds("H:mm", ",5"), "H:mm',5'");
    assert_eq!(after_seconds("H:mm:ss", "'7"), "H:mm:ss'''7'");
}

#[test]
fn sizes_are_shown_as_explorer_shows_them() {
    // The unit and the decimal separator follow the user's language and region:
    // "1.43 MB" or "1,43 МБ".
    let size = format_size(1_500_000);
    assert!(size.starts_with("1"), "{size}");
    assert!(size.contains(' '), "{size}");
    assert_ne!(size, "1500000");
    assert!(format_size(0).starts_with('0'));
}

#[test]
fn grouping_follows_the_locale_string() {
    assert_eq!(grouping_field("3;0"), 3);
    assert_eq!(grouping_field("3;2;0"), 32);
    assert_eq!(grouping_field("3"), 30);
    assert_eq!(grouping_field("0"), 0);
}

#[test]
fn the_list_separator_is_that_of_the_locale() {
    // TASK-40 #5, #8: a semicolon in the Ukrainian region, a comma in the English one, and
    // so is the delimiter of CSV «as in Windows».
    let (list, decimal) = list_and_decimal_separators(UK);
    assert_eq!(
        (list.as_deref(), decimal.as_deref()),
        (Some(";"), Some(","))
    );
    assert_eq!(
        crate::export::windows_delimiter(list.as_deref(), decimal.as_deref()),
        ';'
    );
    let (list, decimal) = list_and_decimal_separators(EN);
    assert_eq!(
        (list.as_deref(), decimal.as_deref()),
        (Some(","), Some("."))
    );
    assert_eq!(
        crate::export::windows_delimiter(list.as_deref(), decimal.as_deref()),
        ','
    );
}

#[test]
fn the_offset_from_utc_is_whole_minutes_of_the_local_time() {
    // TASK-40 #3: whatever the time zone of the computer, the offset takes the moment to
    // the local time Windows gives for it.
    let moment = 134_390_451_311_234_567; // 2026-11-13 12:05:31.1234567 UTC
    let offset = utc_offset_minutes(moment).expect("Windows converts the moment");
    assert!(offset.abs() <= 14 * 60, "{offset}");
    let local = local_time(moment).unwrap();
    let written = crate::difference::iso_time_for_tests(moment, &|_| Some(offset));
    assert!(
        written.starts_with(&format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.1234567",
            local.wYear, local.wMonth, local.wDay, local.wHour, local.wMinute, local.wSecond
        )),
        "{written}"
    );
}
