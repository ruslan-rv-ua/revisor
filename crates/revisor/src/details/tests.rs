use super::*;
use crate::i18n::{En, Uk};
use revisor_core::scan::FileId;
use std::ffi::OsString;

/// Regional formats that write moments to the second, as Windows does — "s20" for any
/// moment of the twentieth second — and numbers as the Ukrainian region does: digits in
/// groups of three split by a space, and a decimal comma.
struct Seconds;

impl Regional for Seconds {
    fn number(&self, digits: &str, _: u32) -> String {
        let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
        let mut grouped = String::new();
        for (i, digit) in whole.chars().enumerate() {
            if i > 0 && (whole.len() - i) % 3 == 0 {
                grouped.push(' ');
            }
            grouped.push(digit);
        }
        if fraction.is_empty() {
            grouped
        } else {
            format!("{grouped},{fraction}")
        }
    }

    fn file_time(&self, file_time: i64) -> String {
        format!("s{}", file_time.div_euclid(TICKS))
    }

    fn file_time_fraction(&self, file_time: i64, digits: u32) -> String {
        let fraction = format!("{:07}", file_time.rem_euclid(TICKS));
        format!(
            "s{},{}",
            file_time.div_euclid(TICKS),
            &fraction[..digits as usize]
        )
    }

    fn date_time(&self, unix_millis: i64) -> String {
        format!("d{unix_millis}")
    }

    fn attribute_letter(&self, flag: u32) -> Option<String> {
        crate::difference::tests::explorer_letter(flag)
    }
}

/// 100-ns ticks in a second.
const TICKS: i64 = 10_000_000;

const ARCHIVE: u32 = 0x20;
const HIDDEN: u32 = 0x2;
const READ_ONLY: u32 = 0x1;
const SYSTEM: u32 = 0x4;
const PINNED: u32 = 0x8_0000;

fn entry(name: &str, kind: EntryKind) -> Entry {
    Entry {
        parent: Some(0),
        name: OsString::from(name).into_boxed_os_str(),
        kind,
        inaccessible: false,
        attributes: match kind {
            EntryKind::Dir => FILE_ATTRIBUTE_DIRECTORY,
            EntryKind::Link => FILE_ATTRIBUTE_REPARSE_POINT | FILE_ATTRIBUTE_DIRECTORY,
            EntryKind::File => ARCHIVE,
        },
        size: 0,
        created: 5 * TICKS,
        modified: 10 * TICKS,
        file_id: FileId::default(),
        version: VersionInfo::NotRead,
        link_target: None,
    }
}

fn version(file: [u16; 4], product: [u16; 4]) -> VersionInfo {
    VersionInfo::Some(Version { file, product })
}

/// A row of a change of `change` on `old` and `new`.
fn row<'a>(
    path: &str,
    change: RowChange,
    old: Option<&'a Entry>,
    new: Option<&'a Entry>,
) -> Row<'a> {
    Row {
        path: path.into(),
        change,
        what_changed: String::new(),
        old,
        new,
        fields: Fields::default(),
        inaccessible: None,
    }
}

/// The details in Ukrainian, with sizes as File Explorer writes them and attributes in
/// words, the defaults.
fn uk(row: &Row<'_>) -> String {
    text(&Uk, &Seconds, Formats::default(), row)
}

fn lines(text: &str) -> Vec<&str> {
    text.lines().collect()
}

#[test]
fn a_modified_exe_says_every_property_a_line_each_in_the_agreed_order() {
    // The sample agreed with the author on 2026-11-06.
    let mut old = entry("app.exe", EntryKind::File);
    old.size = 1_234_567;
    old.version = version([1, 2, 3, 4], [1, 2, 0, 0]);
    let mut new = old.clone();
    new.size = 1_236_992;
    new.modified = 20 * TICKS;
    new.version = version([1, 2, 4, 0], [1, 2, 0, 0]);
    let row = Row {
        what_changed: "версія файлу, розмір, дата зміни".into(),
        fields: Fields::SIZE | Fields::MODIFIED | Fields::VERSION,
        ..row(
            r"C:\Program Files\App\app.exe",
            RowChange::Modified,
            Some(&old),
            Some(&new),
        )
    };
    assert_eq!(
        lines(&uk(&row)),
        [
            r"Шлях: C:\Program Files\App\app.exe",
            "Зміна: змінено; змінилося: версія файлу, розмір, дата зміни",
            "Вид: файл",
            "Розмір: було 1,17 МБ (1 234 567 байтів), стало 1,17 МБ (1 236 992 байти)",
            "Дата створення: s5 (без змін)",
            "Дата зміни: було s10, стало s20",
            "Атрибути: архівний (без змін)",
            "Версія файлу: було 1.2.3.4, стало 1.2.4.0",
            "Версія продукту: 1.2.0.0 (без змін)",
        ]
    );
    // The same lines in English (#2).
    assert_eq!(
        lines(&text(
            &En,
            &Seconds,
            Formats::default(),
            &Row {
                what_changed: "file version, size, date modified".into(),
                ..row
            }
        )),
        [
            r"Path: C:\Program Files\App\app.exe",
            "Change: modified; what changed: file version, size, date modified",
            "Kind: file",
            "Size: before 1,17 MB (1 234 567 bytes), after 1,17 MB (1 236 992 bytes)",
            "Date created: s5 (unchanged)",
            "Date modified: before s10, after s20",
            "Attributes: archive (unchanged)",
            "File version: before 1.2.3.4, after 1.2.4.0",
            "Product version: 1.2.0.0 (unchanged)",
        ]
    );
}

#[test]
fn an_inaccessible_folder_says_in_which_snapshot_and_why() {
    // The other sample agreed on 2026-11-06: the folder's last write time is not counted.
    let old = entry("docs", EntryKind::Dir);
    let mut new = old.clone();
    new.modified = 20 * TICKS;
    new.inaccessible = true;
    let row = Row {
        inaccessible: Some(Inaccessibility {
            which: InaccessibleIn::Later,
            why: Unreadable::Folder,
            absent_in: None,
        }),
        ..row(
            r"C:\Data\docs",
            RowChange::Inaccessible,
            Some(&old),
            Some(&new),
        )
    };
    assert_eq!(
        lines(&uk(&row)),
        [
            r"Шлях: C:\Data\docs",
            "Зміна: недоступно",
            "Вид: тека",
            "Дата створення: s5 (без змін)",
            "Дата зміни: було s10, стало s20 (не враховано)",
            "Атрибути: без атрибутів (без змін)",
            "Недоступно: у пізнішому знімку — вміст теки не вдалося прочитати.",
        ]
    );
}

#[test]
fn an_added_or_deleted_entry_has_one_value_a_property() {
    let mut file = entry("new.txt", EntryKind::File);
    file.size = 7;
    file.attributes = ARCHIVE | HIDDEN;
    assert_eq!(
        lines(&uk(&row(
            r"C:\Data\new.txt",
            RowChange::Added,
            None,
            Some(&file)
        ))),
        [
            r"Шлях: C:\Data\new.txt",
            "Зміна: додано",
            "Вид: файл",
            "Розмір: 7 байтів",
            "Дата створення: s5",
            "Дата зміни: s10",
            "Атрибути: прихований, архівний",
        ]
    );
    let folder = entry("old", EntryKind::Dir);
    assert_eq!(
        lines(&uk(&row(
            r"C:\Data\old",
            RowChange::Deleted,
            Some(&folder),
            None
        ))),
        [
            r"Шлях: C:\Data\old",
            "Зміна: видалено",
            "Вид: тека",
            "Дата створення: s5",
            "Дата зміни: s10",
            "Атрибути: без атрибутів",
        ]
    );
}

#[test]
fn a_name_whose_case_changed_has_its_own_line_after_the_path() {
    let old = entry("app.exe", EntryKind::File);
    let new = entry("App.exe", EntryKind::File);
    let row = Row {
        what_changed: "регістр імені".into(),
        fields: Fields::NAME,
        ..row(
            r"C:\Data\App.exe",
            RowChange::Modified,
            Some(&old),
            Some(&new),
        )
    };
    let text = uk(&row);
    assert_eq!(
        lines(&text)[..3],
        [
            r"Шлях: C:\Data\App.exe",
            "Ім'я: було app.exe, стало App.exe",
            "Зміна: змінено; змінилося: регістр імені",
        ]
    );
    // No name line when the name did not change.
    let same = Row {
        fields: Fields::default(),
        ..row
    };
    assert!(!uk(&same).contains("Ім'я"), "{}", uk(&same));
}

#[test]
fn two_dates_of_one_second_show_as_many_digits_of_it_as_tell_them_apart() {
    let mut old = entry("a.txt", EntryKind::File);
    let mut new = old.clone();
    old.modified = 20 * TICKS + 1_000_000;
    new.modified = 20 * TICKS + 9_000_000;
    let changed = Row {
        fields: Fields::MODIFIED,
        ..row(r"C:\a.txt", RowChange::Modified, Some(&old), Some(&new))
    };
    assert!(
        lines(&uk(&changed)).contains(&"Дата зміни: було s20,1, стало s20,9"),
        "{}",
        uk(&changed)
    );
    old.modified = 20 * TICKS + 1_234_567;
    new.modified = 20 * TICKS + 1_234_999;
    let changed = Row {
        fields: Fields::MODIFIED,
        ..row(r"C:\a.txt", RowChange::Modified, Some(&old), Some(&new))
    };
    assert!(
        lines(&uk(&changed)).contains(&"Дата зміни: було s20,12345, стало s20,12349"),
        "{}",
        uk(&changed)
    );
    // Dates of different seconds need no fraction.
    new.modified = 21 * TICKS;
    let changed = Row {
        fields: Fields::MODIFIED,
        ..row(r"C:\a.txt", RowChange::Modified, Some(&old), Some(&new))
    };
    assert!(
        lines(&uk(&changed)).contains(&"Дата зміни: було s20, стало s21"),
        "{}",
        uk(&changed)
    );
}

#[test]
fn the_digits_of_a_fraction_are_the_fewest_that_differ() {
    assert_eq!(fraction_digits(1_000_000, 9_000_000), 1);
    assert_eq!(fraction_digits(1_230_000, 1_240_000), 3);
    assert_eq!(fraction_digits(1_234_567, 1_234_568), 7);
    // Seconds apart, the fraction is the same: all seven digits, which cannot help.
    assert_eq!(fraction_digits(TICKS + 5, 2 * TICKS + 5), 7);
}

#[test]
fn differences_the_comparison_does_not_count_are_marked() {
    // The archive attribute alone (TASK-10 #10, #11).
    let old = entry("a.txt", EntryKind::File);
    let mut new = old.clone();
    new.attributes = 0;
    let unchanged = row(r"C:\a.txt", RowChange::Inaccessible, Some(&old), Some(&new));
    assert!(
        lines(&uk(&unchanged))
            .contains(&"Атрибути: було архівний, стало без атрибутів (не враховано)"),
        "{}",
        uk(&unchanged)
    );
    // The cloud state alone too; another attribute counts.
    new.attributes = ARCHIVE | PINNED;
    assert!(
        uk(&row(
            r"C:\a.txt",
            RowChange::Modified,
            Some(&old),
            Some(&new)
        ))
        .contains("(не враховано)")
    );
    new.attributes = ARCHIVE | HIDDEN;
    let hidden = uk(&row(
        r"C:\a.txt",
        RowChange::Modified,
        Some(&old),
        Some(&new),
    ));
    assert!(
        lines(&hidden).contains(&"Атрибути: було архівний, стало прихований, архівний"),
        "{hidden}"
    );
    // «Звичайний» is never named: NTFS reports it in place of no attributes.
    new.attributes = FILE_ATTRIBUTE_NORMAL;
    let normal = uk(&row(
        r"C:\a.txt",
        RowChange::Modified,
        Some(&old),
        Some(&new),
    ));
    assert!(
        lines(&normal).contains(&"Атрибути: було архівний, стало без атрибутів (не враховано)"),
        "{normal}"
    );
}

#[test]
fn versions_are_four_numbers_or_say_why_not() {
    let mut old = entry("lib.dll", EntryKind::File);
    old.version = version([10, 0, 1, 2], [10, 0, 0, 0]);
    // A snapshot that did not read the version (#10): the difference is not counted.
    let mut new = old.clone();
    new.version = VersionInfo::NotRead;
    let not_read = uk(&row(
        r"C:\lib.dll",
        RowChange::Modified,
        Some(&old),
        Some(&new),
    ));
    assert!(
        lines(&not_read)
            .contains(&"Версія файлу: було 10.0.1.2, стало версію не прочитано (не враховано)"),
        "{not_read}"
    );
    // A file without a version resource.
    new.version = VersionInfo::None;
    let none = uk(&row(
        r"C:\lib.dll",
        RowChange::Modified,
        Some(&old),
        Some(&new),
    ));
    assert!(
        lines(&none).contains(&"Версія продукту: було 10.0.0.0, стало немає"),
        "{none}"
    );
    // Only exe and dll files have version lines, whatever the case of the extension.
    let exe = entry("SETUP.EXE", EntryKind::File);
    assert!(uk(&row(r"C:\SETUP.EXE", RowChange::Added, None, Some(&exe))).contains("Версія"));
    let text = entry("readme.txt", EntryKind::File);
    assert!(!uk(&row(r"C:\readme.txt", RowChange::Added, None, Some(&text))).contains("Версія"));
}

#[test]
fn a_link_names_its_target_and_not_the_attributes_its_kind_repeats() {
    let mut old = entry("link", EntryKind::Link);
    old.link_target = Some(OsString::from(r"C:\Old").into_boxed_os_str());
    let mut new = old.clone();
    new.link_target = Some(OsString::from(r"C:\New").into_boxed_os_str());
    let changed = Row {
        what_changed: "ціль посилання".into(),
        fields: Fields::LINK_TARGET,
        ..row(r"C:\Data\link", RowChange::Modified, Some(&old), Some(&new))
    };
    assert_eq!(
        lines(&uk(&changed)),
        [
            r"Шлях: C:\Data\link",
            "Зміна: змінено; змінилося: ціль посилання",
            "Вид: посилання",
            "Дата створення: s5 (без змін)",
            "Дата зміни: s10 (без змін)",
            "Атрибути: без атрибутів (без змін)",
            r"Ціль: було C:\Old, стало C:\New",
        ]
    );
    // A target that could not be read.
    let mut unread = entry("link", EntryKind::Link);
    unread.inaccessible = true;
    let row = Row {
        inaccessible: Some(Inaccessibility {
            which: InaccessibleIn::Earlier,
            why: Unreadable::LinkTarget,
            absent_in: Some(Side::Later),
        }),
        ..row(
            r"C:\Data\link",
            RowChange::Inaccessible,
            Some(&unread),
            None,
        )
    };
    let text = uk(&row);
    assert!(lines(&text).contains(&"Ціль: невідома"), "{text}");
    assert!(
        lines(&text).contains(
            &"Недоступно: у ранішому знімку — ціль посилання не вдалося прочитати; у \
              пізнішому знімку його немає."
        ),
        "{text}"
    );
}

#[test]
fn an_entry_unknown_in_a_snapshot_says_so() {
    let folder = entry("Plugins", EntryKind::Dir);
    let row = Row {
        inaccessible: Some(Inaccessibility {
            which: InaccessibleIn::Later,
            why: Unreadable::Unknown,
            absent_in: None,
        }),
        ..row(
            r"C:\App\Plugins",
            RowChange::Inaccessible,
            Some(&folder),
            None,
        )
    };
    assert_eq!(
        lines(&uk(&row)).last(),
        Some(
            &"Недоступно: у пізнішому знімку — невідомо, чи він там був: теку на шляху до \
              нього не вдалося прочитати або вона є посиланням."
        )
    );
    assert_eq!(
        lines(&text(&En, &Seconds, Formats::default(), &row)).last(),
        Some(
            &"Inaccessible: in the later snapshot — whether it was there is unknown: a folder \
              on the way to it could not be read or is a link."
        )
    );
}

#[test]
fn sizes_follow_the_setting_with_the_bytes_when_they_are_rounded() {
    let mut file = entry("big.bin", EntryKind::File);
    file.size = 1_234_567;
    let size_line = |format: SizeFormat, file: &Entry| -> String {
        let formats = Formats {
            size: format,
            ..Formats::default()
        };
        let text = text(
            &Uk,
            &Seconds,
            formats,
            &row(r"C:\big.bin", RowChange::Added, None, Some(file)),
        );
        lines(&text)[3].to_string()
    };
    assert_eq!(
        size_line(SizeFormat::Explorer, &file),
        "Розмір: 1,17 МБ (1 234 567 байтів)"
    );
    assert_eq!(
        size_line(SizeFormat::Decimal, &file),
        "Розмір: 1,23 МБ (1 234 567 байтів)"
    );
    assert_eq!(
        size_line(SizeFormat::Bytes, &file),
        "Розмір: 1 234 567 байтів"
    );
    // Below a unit, the bytes are the size, once.
    file.size = 512;
    assert_eq!(size_line(SizeFormat::Explorer, &file), "Розмір: 512 байтів");
}

#[test]
fn attributes_are_written_as_the_setting_says_without_the_kinds_own() {
    let words = |kind, flags| attributes(&Uk, &Seconds, AttributeFormat::Words, kind, flags);
    let letters = |kind, flags| attributes(&Uk, &Seconds, AttributeFormat::Letters, kind, flags);
    let commas = |kind, flags| {
        attributes(
            &Uk,
            &Seconds,
            AttributeFormat::LettersWithCommas,
            kind,
            flags,
        )
    };
    let all = READ_ONLY | HIDDEN | SYSTEM | ARCHIVE;
    assert_eq!(
        words(EntryKind::File, all),
        "лише для читання, прихований, системний, архівний"
    );
    // File Explorer's letters, as Windows gives them: run together, or split by commas
    // (decided 2026-11-06).
    assert_eq!(letters(EntryKind::File, all), "RHSA");
    assert_eq!(commas(EntryKind::File, all), "R, H, S, A");
    assert_eq!(
        attributes(
            &En,
            &Seconds,
            AttributeFormat::Words,
            EntryKind::File,
            HIDDEN | ARCHIVE
        ),
        "hidden, archive"
    );
    // An attribute File Explorer writes no letter for, as a sparse file's now, in words.
    const SPARSE: u32 = 0x200;
    assert_eq!(
        letters(EntryKind::File, READ_ONLY | ARCHIVE | SPARSE),
        "RA, розріджений"
    );
    assert_eq!(
        commas(EntryKind::File, READ_ONLY | ARCHIVE | SPARSE),
        "R, A, розріджений"
    );
    assert_eq!(letters(EntryKind::File, SPARSE), "розріджений");
    // A folder's own attribute, a link's reparse point and folder, and «звичайний».
    assert_eq!(
        words(EntryKind::Dir, FILE_ATTRIBUTE_DIRECTORY),
        "без атрибутів"
    );
    assert_eq!(
        words(
            EntryKind::Link,
            FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT | HIDDEN
        ),
        "прихований"
    );
    assert_eq!(
        letters(
            EntryKind::Link,
            FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT | HIDDEN
        ),
        "H"
    );
    assert_eq!(
        words(EntryKind::File, FILE_ATTRIBUTE_NORMAL),
        "без атрибутів"
    );
    assert_eq!(letters(EntryKind::File, 0), "без атрибутів");
    assert_eq!(
        commas(EntryKind::Dir, FILE_ATTRIBUTE_DIRECTORY),
        "без атрибутів"
    );
    // A file or folder with another reparse point, as OneDrive's, names it.
    assert_eq!(
        words(EntryKind::File, FILE_ATTRIBUTE_REPARSE_POINT | ARCHIVE),
        "архівний, точка повторного аналізу"
    );
    assert_eq!(
        letters(EntryKind::File, FILE_ATTRIBUTE_REPARSE_POINT | ARCHIVE),
        "AL"
    );
    // A flag revisor has no word for is written as it is.
    assert_eq!(
        words(EntryKind::File, ARCHIVE | 0x0020_0000),
        "архівний, 0x00200000"
    );
    assert_eq!(
        letters(EntryKind::File, ARCHIVE | 0x0020_0000),
        "A, 0x00200000"
    );
}

#[test]
fn every_attribute_has_its_own_flag_and_a_name_in_each_language() {
    for (i, a) in Attribute::ALL.into_iter().enumerate() {
        assert_eq!(a.bit().count_ones(), 1, "{a:?}");
        assert!(
            Attribute::ALL[..i].iter().all(|b| b.bit() != a.bit()),
            "{a:?}"
        );
        assert!(!Uk.attribute(a).is_empty() && !En.attribute(a).is_empty());
        assert_ne!(
            a.bit() & (FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_NORMAL),
            a.bit(),
            "{a:?} is never named"
        );
    }
}
