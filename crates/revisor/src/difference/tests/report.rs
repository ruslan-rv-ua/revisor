//! The text report of the difference (TASK-39 #3–#7, #10).

use super::*;
use revisor_core::job::Outcome;

/// The difference of two folders, given out of the order of their paths, with a change of
/// every type, in tree order: `C:\Data` with a modified, a replaced, an added and a
/// deleted file under `app`, a folder added there, a folder inaccessible later, a file
/// moved into `notes`, and a file whose sizes the setting «Розмір» writes the same; and
/// `C:\Apps` with a file added.
fn report_sample() -> Difference {
    let mut data = Made::new(r"C:\Data");
    data.same("app", EntryKind::Dir);
    data.modified(r"app\app.exe", EntryKind::File, (1024, 2048), Fields::SIZE);
    data.modified(
        r"app\lib.dll",
        EntryKind::File,
        (100, 150),
        Fields::FILE_ID | Fields::SIZE,
    );
    data.added(r"app\new.txt", EntryKind::File, 7);
    data.deleted(r"app\old.txt", EntryKind::File, 10);
    data.added(r"app\plugins", EntryKind::Dir, 0);
    data.inaccessible("docs", EntryKind::Dir, Some(false), Some(true));
    let a = Made::put(&mut data.old, "readme.txt", EntryKind::File);
    let b = Made::put(&mut data.new, r"notes\readme.txt", EntryKind::File);
    data.change(ChangeKind::Moved, Some(a), Some(b), Fields::FOLDER);
    let (a, b) = data.modified(
        "same.bin",
        EntryKind::File,
        (1_234_567, 1_234_600),
        Fields::SIZE | Fields::MODIFIED,
    );
    let mut data = data.done();
    data.old[a as usize].modified = 1;
    data.new[b as usize].modified = 2;
    let mut apps = Made::new(r"C:\Apps");
    apps.added("tool.exe", EntryKind::File, 5);
    difference(vec![data, apps.done()], Vec::new())
}

/// A list in Ukrainian with the Ukrainian region's numbers, showing the sample.
fn ukrainian_list() -> List {
    let mut list = list_with(&Uk, Box::new(Ukrainian));
    list.replace(report_sample(), programs());
    list
}

/// The report of what `list` shows, in Ukrainian.
fn report(list: &List) -> String {
    list.export()
        .expect("the list shows a difference")
        .text(&Uk, &Ukrainian)
}

/// The report without its mark of UTF-8, split into lines.
fn lines(report: &str) -> Vec<&str> {
    report
        .strip_prefix('\u{feff}')
        .expect("the report starts with the mark of UTF-8")
        .strip_suffix("\r\n")
        .expect("the last line ends too")
        .split("\r\n")
        .collect()
}

/// The lines of the report's header without a filter: the title, the field «Порівняння»
/// as the window shows it, and the folders by their paths.
fn header(list: &List) -> Vec<String> {
    let mut header = vec![format!(
        "Звіт про різницю, revisor {}",
        revisor_core::version()
    )];
    header.extend(list.comparison_text().unwrap().lines().map(str::to_string));
    header
}

#[test]
fn the_report_lists_every_change_by_type_and_path_after_its_header() {
    // #4–#6.
    let list = ukrainian_list();
    let text = report(&list);
    let mut expected = header(&list);
    expected.extend(
        [
            r"Тека: C:\Apps",
            r"Тека: C:\Data",
            "",
            "",
            "Додано: 3",
            "",
            r"C:\Apps\tool.exe",
            r"C:\Data\app\new.txt",
            r"C:\Data\app\plugins — тека",
            "",
            "",
            "Видалено: 1",
            "",
            r"C:\Data\app\old.txt",
            "",
            "",
            "Змінено: 3",
            "",
            r"C:\Data\app\app.exe — розмір: було 1,00 КБ, стало 2,00 КБ",
            r"C:\Data\app\lib.dll — замінено; розмір: було 100 байтів, стало 150 байтів",
            "C:\\Data\\same.bin — розмір: було 1,17 МБ (1 234 567 байтів), стало 1,17 МБ \
             (1 234 600 байтів); дата зміни: було t1, стало t2",
            "",
            "",
            "Перейменовано чи переміщено: 1",
            "",
            r"C:\Data\notes\readme.txt — з C:\Data\readme.txt",
            "",
            "",
            "Недоступно: 1",
            "",
            r"C:\Data\docs — тека; недоступно в пізнішому знімку — вміст теки не вдалося прочитати",
        ]
        .map(str::to_string),
    );
    assert_eq!(lines(&text), expected);
}

#[test]
fn the_header_names_the_version_the_pair_and_the_summary_without_a_filter_line() {
    // #4: no filter applies, so the summary shows no «Показано» and no line names one.
    let list = ukrainian_list();
    let text = report(&list);
    let lines = lines(&text);
    assert_eq!(
        lines[0],
        format!("Звіт про різницю, revisor {}", revisor_core::version())
    );
    assert!(lines[1].starts_with("Програми: з d1000, «перед встановленням X», до d2000"));
    assert!(lines[2].starts_with("8 змін: "), "{}", lines[2]);
    assert!(!text.contains("Показано"));
    assert!(!text.contains("Фільтр:"));
}

#[test]
fn the_report_holds_only_the_rows_the_filter_shows_and_says_which_filter() {
    // #3, #4.
    let mut list = ukrainian_list();
    list.set_types([true, false, true, true, true]);
    list.set_text("app");
    let text = report(&list);
    let lines = lines(&text);
    let summary = &lines[2];
    assert!(summary.ends_with(" Показано 5 із 9."), "{summary}");
    assert_eq!(
        lines[3],
        "Фільтр: додані, змінені, перейменовані чи переміщені, недоступні; текст «app»."
    );
    assert_eq!(lines[4..6], [r"Тека: C:\Apps", r"Тека: C:\Data"]);
    let changes: Vec<&str> = lines[6..]
        .iter()
        .copied()
        .filter(|l| l.starts_with("C:"))
        .collect();
    assert_eq!(
        changes,
        [
            r"C:\Apps\tool.exe",
            r"C:\Data\app\new.txt",
            r"C:\Data\app\plugins — тека",
            r"C:\Data\app\app.exe — розмір: було 1,00 КБ, стало 2,00 КБ",
            r"C:\Data\app\lib.dll — замінено; розмір: було 100 байтів, стало 150 байтів",
        ]
    );
    // No group is empty: the deleted, renamed and inaccessible ones have no row here.
    assert!(!text.contains("Видалено:"));
    assert!(!text.contains("Недоступно:"));
}

#[test]
fn the_filter_line_names_the_types_alone_or_the_text_alone() {
    let mut list = ukrainian_list();
    list.set_types([true, false, false, false, false]);
    assert!(lines(&report(&list)).contains(&"Фільтр: додані."));
    list.set_types([true; 5]);
    list.set_text("tool");
    assert!(lines(&report(&list)).contains(&"Фільтр: текст «tool»."));
}

#[test]
fn the_report_keeps_the_order_of_paths_whatever_the_order_or_view_of_the_window() {
    // #3.
    let mut list = ukrainian_list();
    let by_path = report(&list);
    list.set_sort(DifferenceSort {
        column: SortColumn::Size,
        descending: true,
    });
    assert_eq!(report(&list), by_path);
    list.set_layout(DifferenceView::Tree);
    assert_eq!(report(&list), by_path);
}

#[test]
fn the_report_has_no_rules_arrows_bullets_or_columns_lined_up_by_spaces() {
    // #7.
    let text = report(&ukrainian_list());
    for mark in ["==", "--", "→", "->", "•", "  ", "\t"] {
        assert!(!text.contains(mark), "{mark:?} in the report");
    }
    for line in lines(&text) {
        assert!(!line.starts_with(['-', '*']), "{line}");
    }
}

#[test]
fn the_file_name_offered_holds_the_profile_and_both_moments_without_colons() {
    // #2.
    let list = ukrainian_list();
    let name = list
        .export_name(&|millis| format!("{millis}:00"))
        .expect("the list shows a difference");
    assert_eq!(name, "Програми 1000_00 — 2000_00");
}

// --- The file (#8) ------------------------------------------------------------------

/// A new empty folder for the test `name`, which goes when the value does.
struct Folder(PathBuf);

impl Folder {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("revisor-export-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    /// The names of the files in the folder.
    fn files(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(&self.0)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn the_export_writes_the_report_to_the_file_chosen_and_nothing_else() {
    let folder = Folder::new("written");
    let path = folder.0.join("звіт.txt");
    std::fs::write(&path, "old").unwrap();
    let list = ukrainian_list();
    let export = list.export().unwrap();
    let written = crate::export::write_file(&export, &Uk, &Ukrainian, &path, &|| false);
    assert!(
        matches!(&written, Outcome::Done(p) if *p == path),
        "{written:?}"
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), report(&list));
    assert_eq!(folder.files(), ["звіт.txt"]);
}

#[test]
fn a_cancelled_export_leaves_no_file_and_the_file_it_was_to_overwrite_whole() {
    let folder = Folder::new("cancelled");
    let path = folder.0.join("звіт.txt");
    std::fs::write(&path, "old").unwrap();
    let export = ukrainian_list().export().unwrap();
    let written = crate::export::write_file(&export, &Uk, &Ukrainian, &path, &|| true);
    assert!(matches!(written, Outcome::Cancelled), "{written:?}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "old");
    assert_eq!(folder.files(), ["звіт.txt"]);
    let new = folder.0.join("новий.txt");
    let written = crate::export::write_file(&export, &Uk, &Ukrainian, &new, &|| true);
    assert!(matches!(written, Outcome::Cancelled), "{written:?}");
    assert_eq!(folder.files(), ["звіт.txt"]);
}

#[test]
fn an_export_that_fails_says_why_and_leaves_no_file() {
    let folder = Folder::new("failed");
    let path = folder.0.join("немає").join("звіт.txt");
    let export = ukrainian_list().export().unwrap();
    let written = crate::export::write_file(&export, &Uk, &Ukrainian, &path, &|| false);
    assert!(
        matches!(&written, Outcome::Failed(e) if e.kind() == std::io::ErrorKind::NotFound),
        "{written:?}"
    );
    assert!(folder.files().is_empty());
}

#[test]
fn the_export_job_runs_one_at_a_time_and_ends_with_the_file_written() {
    // #8: a short export ends before the progress row would show it.
    use crate::jobs::{Event, Jobs, Kind};
    let folder = Folder::new("job");
    let path = folder.0.join("звіт.txt");
    let mut jobs = Jobs::default();
    jobs.start_export(
        ukrainian_list().export().unwrap(),
        path.clone(),
        crate::export::Format::Text,
    );
    assert!(jobs.runs(Kind::Export));
    assert!(jobs.flags(None).export_running);
    assert!(jobs.shown().is_none());
    let start = std::time::Instant::now();
    let events = loop {
        let events = jobs.poll();
        if !events.is_empty() || start.elapsed().as_secs() > 10 {
            break events;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    match events.as_slice() {
        [Event::Export(to, Ok(Outcome::Done(written)))] => {
            assert_eq!(to, &path);
            assert_eq!(written, &path);
        }
        other => panic!("{other:?}"),
    }
    assert!(!jobs.any());
    assert!(path.exists());
}
