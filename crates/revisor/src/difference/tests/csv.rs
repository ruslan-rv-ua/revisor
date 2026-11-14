//! The difference as CSV (TASK-40 #2–#7, #9).

use super::*;
use crate::difference::csv::{COLUMNS, iso_time};
use crate::export::windows_delimiter;
use revisor_core::job::Outcome;

/// A moment of no time zone: the start of FILETIME, in UTC.
const ZERO: &str = "1601-01-01 00:00:00.0000000+00:00";

/// 2026-11-13 12:05:31.1234567 UTC as a FILETIME.
const MOMENT: i64 = 134_390_451_311_234_567;

/// In UTC.
fn utc(_: i64) -> Option<i32> {
    Some(0)
}

/// The version of a file and of its product.
fn version(file: [u16; 4], product: [u16; 4]) -> VersionInfo {
    VersionInfo::Some(Version { file, product })
}

/// A difference with a row of every kind of change, under `C:\Data`: a modified file with
/// its versions, its date of change and its attributes, a replaced one, an added one whose
/// name holds a comma and a semicolon, a deleted folder, a folder inaccessible later, one
/// inaccessible in both snapshots and one only in the later, two added links, one whose
/// target starts as a formula and one whose target holds quotes, a file moved, one renamed,
/// one moved and renamed, and one whose name changed in case only.
fn csv_sample() -> Difference {
    let mut data = Made::new(r"C:\Data");
    data.same("app", EntryKind::Dir);
    let (app_a, app_b) = data.modified(
        r"app\app.exe",
        EntryKind::File,
        (1024, 2048),
        Fields::SIZE | Fields::VERSION | Fields::MODIFIED,
    );
    data.modified(r"app\lib.dll", EntryKind::File, (100, 100), Fields::FILE_ID);
    data.added(r"app\new, file; 1.txt", EntryKind::File, 7);
    data.deleted(r"app\old", EntryKind::Dir, 0);
    let a = Made::put(&mut data.old, "Case.txt", EntryKind::File);
    let b = Made::put(&mut data.new, "CASE.txt", EntryKind::File);
    data.change(ChangeKind::Modified, Some(a), Some(b), Fields::NAME);
    data.inaccessible("docs", EntryKind::Dir, Some(false), Some(true));
    let formula = data.added("formula", EntryKind::Link, 0);
    data.inaccessible("lost", EntryKind::Dir, Some(true), Some(true));
    let a = Made::put(&mut data.old, "readme.txt", EntryKind::File);
    let b = Made::put(&mut data.new, r"notes\readme.txt", EntryKind::File);
    data.change(ChangeKind::Moved, Some(a), Some(b), Fields::FOLDER);
    let quoted = data.added("quoted", EntryKind::Link, 0);
    let a = Made::put(&mut data.old, "a.txt", EntryKind::File);
    let b = Made::put(&mut data.new, "b.txt", EntryKind::File);
    data.change(ChangeKind::Moved, Some(a), Some(b), Fields::NAME);
    data.inaccessible("secret", EntryKind::Dir, None, Some(true));
    let a = Made::put(&mut data.old, "c.txt", EntryKind::File);
    let b = Made::put(&mut data.new, r"z\d.txt", EntryKind::File);
    data.change(
        ChangeKind::Moved,
        Some(a),
        Some(b),
        Fields::NAME | Fields::FOLDER,
    );
    let mut data = data.done();
    let (old, new) = (&mut data.old[app_a as usize], &mut data.new[app_b as usize]);
    old.version = version([1, 0, 0, 0], [1, 0, 0, 0]);
    new.version = version([1, 2, 3, 4], [1, 2, 0, 0]);
    old.modified = 10_000_000;
    new.modified = 15_000_001;
    old.attributes = 0x20;
    new.attributes = 0x20 | 0x1;
    data.new[formula as usize].link_target = Some(OsStr::new("=HYPERLINK(1)").into());
    data.new[quoted as usize].link_target = Some(OsStr::new(r#"C:\a "b""#).into());
    difference(vec![data], Vec::new())
}

/// A list in `s` with `regional`, showing the sample.
fn csv_list(s: &'static dyn Strings, regional: Box<dyn Regional>) -> List {
    let mut list = list_with(s, regional);
    list.replace(csv_sample(), programs());
    list
}

/// The CSV of what `list` shows, separated by `delimiter`, in UTC.
fn csv(list: &List, delimiter: char) -> String {
    list.export()
        .expect("the list shows a difference")
        .csv(delimiter, &utc)
}

/// The lines of `csv` without its mark of UTF-8.
fn lines(csv: &str) -> Vec<&str> {
    csv.strip_prefix('\u{feff}')
        .expect("CSV starts with the mark of UTF-8")
        .strip_suffix("\r\n")
        .expect("the last line ends too")
        .split("\r\n")
        .collect()
}

/// A line of the values of the columns, given by name; the others are empty.
fn line(values: &[(&str, &str)]) -> String {
    let mut line = vec![String::new(); COLUMNS.len()];
    for &(column, value) in values {
        let at = COLUMNS.iter().position(|&c| c == column).expect(column);
        line[at] = value.to_string();
    }
    line.join(",")
}

/// The values of an entry of the sample in both snapshots that has the dates of no time
/// zone: its size, as a file, and its dates.
fn both(size: Option<(&'static str, &'static str)>) -> Vec<(&'static str, &'static str)> {
    let mut values = vec![
        ("created_before", ZERO),
        ("created_after", ZERO),
        ("modified_before", ZERO),
        ("modified_after", ZERO),
    ];
    if let Some((before, after)) = size {
        values.extend([("size_before", before), ("size_after", after)]);
    }
    values
}

/// The same for an entry of one snapshot: `side` is "before" or "after".
fn one(side: &str, size: Option<&'static str>) -> Vec<(&'static str, &'static str)> {
    let pick = |before: &'static str, after: &'static str| {
        if side == "before" { before } else { after }
    };
    let mut values = vec![
        (pick("created_before", "created_after"), ZERO),
        (pick("modified_before", "modified_after"), ZERO),
    ];
    if let Some(size) = size {
        values.push((pick("size_before", "size_after"), size));
    }
    values
}

/// `fixed` with `more`.
fn with(
    fixed: &[(&'static str, &'static str)],
    more: Vec<(&'static str, &'static str)>,
) -> Vec<(&'static str, &'static str)> {
    let mut all = fixed.to_vec();
    all.extend(more);
    all
}

#[test]
fn the_first_line_names_the_columns_in_english() {
    // #2, #3.
    let text = csv(&csv_list(&Uk, Box::new(Ukrainian)), ',');
    assert_eq!(
        lines(&text)[0],
        "path,change,kind,what_changed,path_before,size_before,size_after,created_before,\
         created_after,modified_before,modified_after,attributes_before,attributes_after,\
         file_version_before,file_version_after,product_version_before,\
         product_version_after,link_target_before,link_target_after,inaccessible_in"
    );
}

#[test]
fn each_row_shown_is_a_line_in_the_order_of_paths_with_its_codes_and_values() {
    // #2–#4, #7, #9: a line of every type of change, kind and field.
    let list = csv_list(&Uk, Box::new(Ukrainian));
    let text = csv(&list, ',');
    let lines = lines(&text);
    let expected: Vec<String> = [
        with(
            &[
                (r"path", r"C:\Data\app\app.exe"),
                ("change", "modified"),
                ("kind", "file"),
                ("what_changed", "size modified file_version product_version"),
                ("size_before", "1024"),
                ("size_after", "2048"),
                ("created_before", ZERO),
                ("created_after", ZERO),
                ("modified_before", "1601-01-01 00:00:01.0000000+00:00"),
                ("modified_after", "1601-01-01 00:00:01.5000001+00:00"),
                ("attributes_before", "archive"),
                ("attributes_after", "readonly archive"),
                ("file_version_before", "1.0.0.0"),
                ("file_version_after", "1.2.3.4"),
                ("product_version_before", "1.0.0.0"),
                ("product_version_after", "1.2.0.0"),
            ],
            Vec::new(),
        ),
        with(
            &[
                ("path", r"C:\Data\app\lib.dll"),
                ("change", "replaced"),
                ("kind", "file"),
            ],
            both(Some(("100", "100"))),
        ),
        with(
            &[
                ("path", r#""C:\Data\app\new, file; 1.txt""#),
                ("change", "added"),
                ("kind", "file"),
            ],
            one("after", Some("7")),
        ),
        with(
            &[
                ("path", r"C:\Data\app\old"),
                ("change", "deleted"),
                ("kind", "folder"),
            ],
            one("before", None),
        ),
        with(
            &[
                ("path", r"C:\Data\b.txt"),
                ("change", "renamed"),
                ("kind", "file"),
                ("path_before", r"C:\Data\a.txt"),
            ],
            both(Some(("0", "0"))),
        ),
        with(
            &[
                ("path", r"C:\Data\CASE.txt"),
                ("change", "modified"),
                ("kind", "file"),
                ("what_changed", "name_case"),
                ("path_before", r"C:\Data\Case.txt"),
            ],
            both(Some(("0", "0"))),
        ),
        with(
            &[
                ("path", r"C:\Data\docs"),
                ("change", "inaccessible"),
                ("kind", "folder"),
                ("inaccessible_in", "later"),
            ],
            both(None),
        ),
        with(
            &[
                ("path", r"C:\Data\formula"),
                ("change", "added"),
                ("kind", "link"),
                ("link_target_after", "'=HYPERLINK(1)"),
            ],
            one("after", None),
        ),
        with(
            &[
                ("path", r"C:\Data\lost"),
                ("change", "inaccessible"),
                ("kind", "folder"),
                ("inaccessible_in", "both"),
            ],
            both(None),
        ),
        with(
            &[
                ("path", r"C:\Data\notes\readme.txt"),
                ("change", "moved"),
                ("kind", "file"),
                ("path_before", r"C:\Data\readme.txt"),
            ],
            both(Some(("0", "0"))),
        ),
        with(
            &[
                ("path", r"C:\Data\quoted"),
                ("change", "added"),
                ("kind", "link"),
                ("link_target_after", r#""C:\a ""b""""#),
            ],
            one("after", None),
        ),
        with(
            &[
                ("path", r"C:\Data\secret"),
                ("change", "inaccessible"),
                ("kind", "folder"),
                ("inaccessible_in", "later"),
            ],
            one("after", None),
        ),
        with(
            &[
                ("path", r"C:\Data\z\d.txt"),
                ("change", "moved_renamed"),
                ("kind", "file"),
                ("path_before", r"C:\Data\c.txt"),
            ],
            both(Some(("0", "0"))),
        ),
    ]
    .iter()
    .map(|values| line(values))
    .collect();
    // The rows come in the order of the paths the list shows.
    let shown: Vec<String> = paths(&list)
        .iter()
        .map(|p| {
            if p.contains(',') {
                format!("\"{p}\"")
            } else {
                p.clone()
            }
        })
        .collect();
    let mut got: Vec<&str> = lines[1..].to_vec();
    let mut want: Vec<&str> = expected.iter().map(String::as_str).collect();
    got.sort_unstable();
    want.sort_unstable();
    assert_eq!(got, want);
    let first: Vec<&str> = lines[1..]
        .iter()
        .map(|l| match l.strip_prefix('"') {
            Some(rest) => &l[..rest.find('"').unwrap() + 2],
            None => l.split(',').next().unwrap(),
        })
        .collect();
    assert_eq!(first, shown);
}

#[test]
fn the_csv_is_the_same_in_every_language_and_region() {
    // #3.
    let uk = csv(&csv_list(&Uk, Box::new(Ukrainian)), ';');
    let en = csv(&csv_list(&En, Box::new(Plain)), ';');
    assert_eq!(uk, en);
}

#[test]
fn the_csv_holds_only_the_rows_the_filter_shows_in_the_order_of_paths() {
    // #2: as the text report, whatever the order or the view of the window.
    let mut list = csv_list(&Uk, Box::new(Ukrainian));
    list.set_types([true, false, true, true, false]);
    list.set_text("app");
    let filtered = csv(&list, ',');
    let paths: Vec<&str> = lines(&filtered)[1..]
        .iter()
        .map(|l| l.split(",added").next().unwrap())
        .map(|l| l.split(",modified").next().unwrap())
        .map(|l| l.split(",replaced").next().unwrap())
        .collect();
    assert_eq!(
        paths,
        [
            r"C:\Data\app\app.exe",
            r"C:\Data\app\lib.dll",
            r#""C:\Data\app\new, file; 1.txt""#,
        ]
    );
    list.set_sort(DifferenceSort {
        column: SortColumn::Size,
        descending: true,
    });
    list.set_layout(DifferenceView::Tree);
    assert_eq!(csv(&list, ','), filtered);
}

#[test]
fn a_value_is_quoted_only_when_it_holds_the_delimiter_a_quote_or_an_end_of_line() {
    // #6 for each delimiter: the name with a comma and a semicolon is quoted where one of
    // them separates the values, and the target with quotes always, its quotes doubled.
    let list = csv_list(&Uk, Box::new(Ukrainian));
    for (delimiter, quoted) in [(',', true), (';', true), ('\t', false)] {
        let text = csv(&list, delimiter);
        let lines = lines(&text);
        assert_eq!(lines[0], COLUMNS.join(&delimiter.to_string()));
        let name = r"C:\Data\app\new, file; 1.txt";
        let expected = if quoted {
            format!("\"{name}\"{delimiter}added")
        } else {
            format!("{name}{delimiter}added")
        };
        assert!(
            lines.iter().any(|l| l.starts_with(&expected)),
            "{delimiter:?}: {lines:#?}"
        );
        let target = format!(r#"{delimiter}"C:\a ""b"""{delimiter}"#);
        assert!(text.contains(&target), "{delimiter:?}");
        assert!(
            lines
                .iter()
                .all(|l| l.split(delimiter).count() >= COLUMNS.len())
        );
    }
}

#[test]
fn moments_are_local_time_with_the_offset_and_seven_digits_of_the_second() {
    // #3.
    assert_eq!(
        iso_time(MOMENT, &|_| Some(120)),
        "2026-11-13 14:05:31.1234567+02:00"
    );
    assert_eq!(
        iso_time(MOMENT, &|_| Some(-330)),
        "2026-11-13 06:35:31.1234567-05:30"
    );
    // A moment Windows gives no offset for is in UTC.
    assert_eq!(
        iso_time(MOMENT, &|_| None),
        "2026-11-13 12:05:31.1234567+00:00"
    );
    // Across a day, a month and a leap day.
    let leap = 133_537_230_000_000_000; // 2024-02-29 23:30 UTC
    assert_eq!(
        iso_time(leap, &|_| Some(60)),
        "2024-03-01 00:30:00.0000000+01:00"
    );
    assert_eq!(iso_time(0, &utc), ZERO);
    // The offset is asked for the moment written.
    let asked = std::cell::Cell::new(0);
    iso_time(MOMENT, &|moment| {
        asked.set(moment);
        Some(0)
    });
    assert_eq!(asked.get(), MOMENT);
}

#[test]
fn a_link_target_that_starts_as_a_formula_gets_an_apostrophe() {
    // #7.
    for (target, written) in [
        ("=cmd|' /C calc'!A0", "'=cmd|' /C calc'!A0"),
        ("+1", "'+1"),
        ("-1", "'-1"),
        ("@SUM(1)", "'@SUM(1)"),
        (r"C:\Program Files", r"C:\Program Files"),
        (r"\\?\Volume{1}\", r"\\?\Volume{1}\"),
    ] {
        let mut data = Made::new(r"C:\Data");
        let link = data.added("link", EntryKind::Link, 0);
        let mut data = data.done();
        data.new[link as usize].link_target = Some(OsStr::new(target).into());
        let list = showing(difference(vec![data], Vec::new()));
        let text = csv(&list, '\t');
        let fields: Vec<&str> = lines(&text)[1].split('\t').collect();
        let at = COLUMNS
            .iter()
            .position(|&c| c == "link_target_after")
            .unwrap();
        assert_eq!(fields[at], written, "{target}");
    }
}

#[test]
fn the_delimiter_as_in_windows_is_the_list_separator_unless_it_cannot_be() {
    // #5, decided 2026-11-14: as FreeFileSync chooses.
    let cases = [
        (Some(";"), Some(","), ';'),
        (Some(","), Some("."), ','),
        (Some("|"), Some("."), '|'),
        (Some("\t"), Some(","), '\t'),
        // The same as the decimal separator.
        (Some(","), Some(","), ';'),
        (Some("."), Some("."), ','),
        // Longer than a character, empty, a quote, the end of a line, or unread.
        (Some(";;"), Some(","), ';'),
        (Some(";;"), Some("."), ','),
        (Some(""), Some(","), ';'),
        (Some("\""), Some("."), ','),
        (Some("\n"), Some(","), ';'),
        (None, None, ','),
    ];
    for (list, decimal, delimiter) in cases {
        assert_eq!(
            windows_delimiter(list, decimal),
            delimiter,
            "{list:?} {decimal:?}"
        );
    }
}

#[test]
fn the_export_writes_csv_as_utf8_with_its_mark_and_lines_ending_in_crlf() {
    // #6, #8: Excel reads the mark and keeps Cyrillic.
    let folder = std::env::temp_dir().join(format!("revisor-csv-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    let path = folder.join("різниця.csv");
    let mut data = Made::new(r"C:\Дані");
    data.added("звіт.txt", EntryKind::File, 1);
    let list = showing(difference(vec![data.done()], Vec::new()));
    let export = list.export().unwrap();
    let written = crate::export::write_csv_file(&export, ';', &utc, &path, &|| false);
    assert!(
        matches!(&written, Outcome::Done(p) if *p == path),
        "{written:?}"
    );
    let bytes = std::fs::read(&path).unwrap();
    assert!(bytes.starts_with(b"\xEF\xBB\xBF"));
    let text = String::from_utf8(bytes).unwrap();
    assert_eq!(text, export.csv(';', &utc));
    assert_eq!(text.matches("\r\n").count(), 2);
    assert_eq!(text.matches('\n').count(), 2);
    assert!(text.contains("\r\nC:\\Дані\\звіт.txt;added;file;"));
    // A cancelled export leaves no file.
    let other = folder.join("інша.csv");
    let written = crate::export::write_csv_file(&export, ';', &utc, &other, &|| true);
    assert!(matches!(written, Outcome::Cancelled), "{written:?}");
    assert!(!other.exists());
    let _ = std::fs::remove_dir_all(&folder);
}

#[test]
fn the_export_job_writes_csv_when_asked() {
    use crate::export::Format;
    use crate::jobs::{Event, Jobs};
    let folder = std::env::temp_dir().join(format!("revisor-csv-job-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    let path = folder.join("звіт.txt");
    let mut jobs = Jobs::default();
    let list = csv_list(&Uk, Box::new(Ukrainian));
    jobs.start_export(
        list.export().unwrap(),
        path.clone(),
        Format::Csv { delimiter: ';' },
    );
    let start = std::time::Instant::now();
    let events = loop {
        let events = jobs.poll();
        if !events.is_empty() || start.elapsed().as_secs() > 10 {
            break events;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    assert!(
        matches!(events.as_slice(), [Event::Export(_, Ok(Outcome::Done(_)))]),
        "{events:?}"
    );
    // The type chosen says the format, whatever the extension.
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.starts_with(&format!("\u{feff}{}\r\n", COLUMNS.join(";"))));
    let _ = std::fs::remove_dir_all(&folder);
}
