use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use revisor_core::job::Cancel;
use revisor_core::profiles::{self, ProfileFile};
use revisor_core::settings::CsvDelimiter;
use revisor_core::store::ProfileSettings;

use super::args::{
    Action, Call, Command, Common, CompareArgs, ReportFormat, SnapshotArgs, UsageError, parse,
};
use super::*;

fn call(args: &[&str]) -> Call {
    parse(args.iter().map(OsString::from))
}

fn action(args: &[&str]) -> Result<Action, UsageError> {
    call(args).action
}

fn common(profile: &str) -> Common {
    Common {
        profile: profile.into(),
        ..Common::default()
    }
}

#[test]
fn a_snapshot_call_takes_its_profile_and_comment() {
    assert_eq!(
        action(&["snapshot", "-p", "Програми", "--comment", "before setup"]),
        Ok(Action::Snapshot(SnapshotArgs {
            common: common("Програми"),
            comment: Some("before setup".into()),
        }))
    );
    // A value may follow "=" or the short option itself.
    assert_eq!(
        action(&["snapshot", "--profile=Home", "-q", "--timeout=30"]),
        Ok(Action::Snapshot(SnapshotArgs {
            common: Common {
                profile: "Home".into(),
                timeout: Some(Duration::from_secs(30)),
                quiet: true,
                ..Common::default()
            },
            comment: None,
        }))
    );
    assert_eq!(
        action(&["snapshot", "-pHome", "--progress"]),
        Ok(Action::Snapshot(SnapshotArgs {
            common: Common {
                profile: "Home".into(),
                progress: true,
                ..Common::default()
            },
            comment: None,
        }))
    );
}

#[test]
fn a_compare_call_takes_its_file_snapshot_format_and_delimiter() {
    assert_eq!(
        action(&[
            "compare",
            "-p",
            "Home",
            "-o",
            "report.txt",
            "--with",
            "12",
            "--format",
            "CSV",
            "--delimiter",
            "tab",
        ]),
        Ok(Action::Compare(CompareArgs {
            common: common("Home"),
            output: PathBuf::from("report.txt"),
            with: Some(12),
            format: Some(ReportFormat::Csv),
            delimiter: Some(CsvDelimiter::Tab),
        }))
    );
    // Options may come before the command.
    let call = call(&["--language", "uk", "compare", "-o", "r.csv", "-p", "Home"]);
    assert_eq!(call.lang, Lang::Uk);
    assert!(matches!(call.action, Ok(Action::Compare(_))));
}

#[test]
fn help_and_version_need_no_profile() {
    assert_eq!(action(&["--help"]), Ok(Action::Help(None)));
    assert_eq!(action(&["-h"]), Ok(Action::Help(None)));
    assert_eq!(
        action(&["compare", "--help"]),
        Ok(Action::Help(Some(Command::Compare)))
    );
    assert_eq!(
        action(&["snapshot", "-h"]),
        Ok(Action::Help(Some(Command::Snapshot)))
    );
    assert_eq!(action(&["--version"]), Ok(Action::Version));
}

#[test]
fn wrong_calls_say_what_is_wrong() {
    let cases: [(&[&str], UsageError); 14] = [
        (&[], UsageError::NoCommand),
        (&["-p", "Home"], UsageError::NoCommand),
        (&["take"], UsageError::UnknownCommand("take".into())),
        (&["snapshot", "-x"], UsageError::UnknownOption("-x".into())),
        (
            &["snapshot", "--verbose"],
            UsageError::UnknownOption("--verbose".into()),
        ),
        // -v is not --version (#11).
        (&["-v"], UsageError::UnknownOption("-v".into())),
        (
            &["snapshot", "-p", "Home", "-o", "r.txt"],
            UsageError::NotForCommand {
                option: "--output".into(),
                command: Command::Snapshot,
            },
        ),
        (
            &["compare", "-p", "Home", "-o", "r.txt", "--comment", "x"],
            UsageError::NotForCommand {
                option: "--comment".into(),
                command: Command::Compare,
            },
        ),
        (&["snapshot", "-p"], UsageError::MissingValue("-p".into())),
        (
            &["snapshot", "-p", "Home", "--quiet=yes"],
            UsageError::UnexpectedValue("--quiet".into()),
        ),
        (
            &["snapshot", "-p", "Home", "extra"],
            UsageError::UnexpectedArgument("extra".into()),
        ),
        (
            &["snapshot", "-p", "Home", "--profile", "Work"],
            UsageError::Repeated("--profile".into()),
        ),
        (&["snapshot"], UsageError::Missing("--profile".into())),
        (
            &["compare", "-p", "Home"],
            UsageError::Missing("--output".into()),
        ),
    ];
    for (args, error) in cases {
        assert_eq!(action(args), Err(error), "{args:?}");
    }
    let invalid = |option: &str, value: &str| {
        Err(UsageError::InvalidValue {
            option: option.into(),
            value: value.into(),
        })
    };
    let compare = |extra: &[&str]| {
        let mut args = vec!["compare", "-p", "Home", "-o", "r.txt"];
        args.extend_from_slice(extra);
        action(&args)
    };
    assert_eq!(compare(&["--with", "0"]), invalid("--with", "0"));
    assert_eq!(compare(&["--with", "last"]), invalid("--with", "last"));
    assert_eq!(compare(&["--format", "html"]), invalid("--format", "html"));
    assert_eq!(compare(&["--delimiter", ";"]), invalid("--delimiter", ";"));
    assert_eq!(compare(&["--timeout", "-1"]), invalid("--timeout", "-1"));
    assert_eq!(compare(&["--language", "de"]), invalid("--language", "de"));
    assert_eq!(
        compare(&["--quiet", "--progress"]),
        Err(UsageError::QuietWithProgress)
    );
}

#[test]
fn a_wrong_call_is_told_in_the_language_it_asked_for_and_points_to_its_command() {
    let call = call(&["--language", "uk", "compare", "-p", "Home"]);
    assert_eq!(call.lang, Lang::Uk);
    assert_eq!(call.command, Some(Command::Compare));
    assert!(call.action.is_err());
    assert_eq!(super::args::parse(Vec::<OsString>::new()).lang, Lang::En);
}

#[test]
fn the_extension_picks_the_format_unless_format_does() {
    let args = |output: &str, format: Option<ReportFormat>| CompareArgs {
        common: common("Home"),
        output: PathBuf::from(output),
        with: None,
        format,
        delimiter: None,
    };
    assert_eq!(args("r.csv", None).report_format(), ReportFormat::Csv);
    assert_eq!(args("R.CSV", None).report_format(), ReportFormat::Csv);
    assert_eq!(args("r.txt", None).report_format(), ReportFormat::Text);
    assert_eq!(args("report", None).report_format(), ReportFormat::Text);
    assert_eq!(args("r.csv.txt", None).report_format(), ReportFormat::Text);
    assert_eq!(
        args("r.txt", Some(ReportFormat::Csv)).report_format(),
        ReportFormat::Csv
    );
    assert_eq!(
        args("r.csv", Some(ReportFormat::Text)).report_format(),
        ReportFormat::Text
    );
}

#[test]
fn the_latest_snapshot_is_compared_with_the_one_before_it_or_with_the_one_chosen() {
    assert_eq!(choose(&[3, 9, 5], None), Ok((5, 9)));
    assert_eq!(choose(&[3, 9, 5], Some(3)), Ok((3, 9)));
    assert_eq!(choose(&[], None), Err(Choice::TooFew));
    assert_eq!(choose(&[4], None), Err(Choice::TooFew));
    assert_eq!(choose(&[4], Some(4)), Err(Choice::Latest(4)));
    assert_eq!(choose(&[3, 9], Some(9)), Err(Choice::Latest(9)));
    assert_eq!(choose(&[3, 9], Some(7)), Err(Choice::Unknown(7)));
    assert_eq!(choose(&[], Some(7)), Err(Choice::TooFew));
}

/// A data folder of revisor with profiles, removed on drop.
struct Data {
    dir: PathBuf,
    interrupt: Arc<Cancel>,
}

impl Data {
    fn new(test: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("revisor-cli-{test}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("files")).unwrap();
        Self {
            dir,
            interrupt: Arc::new(Cancel::new()),
        }
    }

    /// A profile of the folder `files` of this data folder.
    fn profile(&self, name: &str) -> ProfileFile {
        let mut settings = ProfileSettings::new_profile();
        settings.name = name.into();
        settings.scope.folders = vec![self.files()];
        profiles::create(&self.dir.join("profiles"), &settings).unwrap()
    }

    fn files(&self) -> PathBuf {
        self.dir.join("files")
    }

    fn write(&self, name: &str, content: &str) {
        fs::write(self.files().join(name), content).unwrap();
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    /// Runs revisor-cli with `args`: its exit code, stdout and stderr.
    fn run(&self, args: &[&str]) -> (u8, String, String) {
        let env = Env {
            data_dir: self.dir.clone(),
            elevated: true,
            interrupt: Arc::clone(&self.interrupt),
            progress_every: Duration::ZERO,
        };
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run_with(args.iter().map(OsString::from), &env, &mut out, &mut err);
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    /// Takes a snapshot of `profile` and returns its number.
    fn snapshot(&self, profile: &str) -> i64 {
        let (code, out, err) = self.run(&["snapshot", "-p", profile]);
        assert_eq!(code, DONE, "{err}");
        out.trim().parse().unwrap()
    }
}

impl Drop for Data {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn as_str(path: &Path) -> &str {
    path.to_str().unwrap()
}

#[test]
fn a_snapshot_writes_its_number_alone_to_the_output_and_its_summary_to_errors() {
    let data = Data::new("snapshot");
    data.profile("Програми");
    data.write("a.txt", "a");

    let (code, out, err) = data.run(&["snapshot", "-p", "програми", "--comment", "до"]);
    assert_eq!(code, DONE, "{err}");
    let id: i64 = out
        .trim_end()
        .parse()
        .expect("the output is the number alone");
    assert_eq!(out, format!("{id}\n"));
    let lines: Vec<&str> = err.lines().collect();
    assert_eq!(lines.len(), 2, "{err}");
    assert_eq!(lines[0], "Taking a snapshot of the profile “Програми”…");
    assert!(lines[1].starts_with("Snapshot saved: "), "{err}");

    let saved = profiles::snapshots_of(&data.dir.join("profiles").join("Програми.db"))
        .unwrap_or_else(|_| {
            let (profiles, _) = profiles::list(&data.dir.join("profiles")).unwrap();
            profiles::snapshots_of(&profiles[0].path).unwrap()
        });
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].id, id);
    assert_eq!(saved[0].comment.as_deref(), Some("до"));

    // The next snapshot gets a number of its own.
    assert!(data.snapshot("Програми") > id);
}

#[test]
fn quiet_leaves_errors_only_and_the_language_is_the_one_asked_for() {
    let data = Data::new("quiet");
    data.profile("Home");
    let (code, out, err) = data.run(&["snapshot", "-p", "Home", "-q"]);
    assert_eq!((code, err.as_str()), (DONE, ""));
    assert!(out.trim().parse::<i64>().is_ok());

    let (code, _, err) = data.run(&["snapshot", "-p", "Home", "--language", "uk"]);
    assert_eq!(code, DONE);
    assert!(
        err.starts_with("Знімок профілю «Home»…\nЗнімок збережено: "),
        "{err}"
    );

    let (code, _, err) = data.run(&["compare", "-p", "Nobody", "-o", "r.txt", "-q"]);
    assert_eq!(code, FAILED);
    assert!(err.contains("Nobody"), "errors are told even when quiet");
}

#[test]
fn an_unknown_profile_names_the_profiles_there_are() {
    let data = Data::new("unknown");
    let (code, out, err) = data.run(&["snapshot", "-p", "Home"]);
    assert_eq!((code, out.as_str()), (FAILED, ""));
    assert_eq!(
        err,
        "There is no profile “Home”, and no profiles at all yet.\n"
    );
    data.profile("Work");
    data.profile("apps");
    let (code, _, err) = data.run(&["snapshot", "-p", "Home"]);
    assert_eq!(code, FAILED);
    assert_eq!(
        err,
        "There is no profile “Home”. Profiles: “apps”, “Work”.\n"
    );
}

#[test]
fn a_wrong_call_ends_with_code_2_and_help_and_version_with_0() {
    let data = Data::new("usage");
    let (code, out, err) = data.run(&["snapshot"]);
    assert_eq!((code, out.as_str()), (USAGE, ""));
    assert_eq!(
        err,
        "revisor-cli: the option --profile is required.\nFor help: revisor-cli snapshot --help\n"
    );

    let (code, out, err) = data.run(&["--help"]);
    assert_eq!((code, err.as_str()), (DONE, ""));
    assert!(out.starts_with("revisor-cli "), "{out}");
    let (code, out, _) = data.run(&["compare", "--help", "--language", "uk"]);
    assert_eq!(code, DONE);
    assert!(out.starts_with("revisor-cli compare: "), "{out}");
    let (code, out, _) = data.run(&["--version"]);
    assert_eq!(
        (code, out),
        (DONE, format!("revisor-cli {}\n", revisor_core::version()))
    );
}

#[test]
fn the_help_has_no_columns_aligned_with_spaces() {
    for lang in [&En as &dyn Strings, &Uk] {
        for topic in [None, Some(Command::Snapshot), Some(Command::Compare)] {
            let help = lang.cli_help(topic, "0.5.0");
            for line in help.lines() {
                assert!(!line.contains("  "), "two spaces in {line:?}");
                assert!(!line.starts_with(' ') && !line.contains('\t'), "{line:?}");
            }
            for option in ["--profile", "--language", "--timeout", "--quiet", "--help"] {
                assert!(help.contains(option), "{option} in {topic:?}");
            }
        }
    }
}

#[test]
fn compare_writes_the_whole_difference_in_the_format_the_file_asks_for() {
    let data = Data::new("compare");
    data.profile("Home");
    data.write("kept.txt", "kept");
    data.write("gone.txt", "gone");
    let before = data.snapshot("Home");
    data.write("new.txt", "new");
    fs::remove_file(data.files().join("gone.txt")).unwrap();
    let after = data.snapshot("Home");

    // CSV by the extension, with the delimiter asked for.
    let csv = data.path("report.CSV");
    let (code, out, err) = data.run(&[
        "compare",
        "-p",
        "Home",
        "-o",
        as_str(&csv),
        "--delimiter",
        "semicolon",
    ]);
    assert_eq!((code, out.as_str()), (DONE, ""), "{err}");
    let lines: Vec<&str> = err.lines().collect();
    assert_eq!(
        lines[0],
        format!("Comparing snapshots {before} and {after} of the profile “Home”…")
    );
    assert_eq!(lines[1], format!("Difference exported: {}.", csv.display()));
    assert!(
        lines[2].contains("1 added") && lines[2].contains("1 deleted"),
        "{err}"
    );
    let text = fs::read_to_string(&csv).unwrap();
    assert!(
        text.starts_with('\u{feff}'),
        "CSV starts with the byte order mark"
    );
    let header = text.lines().next().unwrap();
    assert!(header.contains(';') && !header.contains(','), "{header}");
    assert!(text.contains("new.txt") && text.contains("gone.txt"));

    // The text report, in Ukrainian; a file already there is replaced whole.
    let report = data.path("report.txt");
    fs::write(&report, "old").unwrap();
    let (code, _, err) = data.run(&[
        "compare",
        "-p",
        "Home",
        "-o",
        as_str(&report),
        "--language",
        "uk",
    ]);
    assert_eq!(code, DONE, "{err}");
    let text = fs::read_to_string(&report).unwrap();
    assert!(text.starts_with("\u{feff}Звіт про різницю"), "{text}");
    assert!(text.contains("new.txt"));
    assert!(err.contains("Різницю експортовано: "), "{err}");

    // --format overrides the extension.
    let (code, _, err) = data.run(&[
        "compare",
        "-p",
        "Home",
        "-o",
        as_str(&report),
        "--format",
        "csv",
        "--delimiter",
        "comma",
    ]);
    assert_eq!(code, DONE, "{err}");
    let text = fs::read_to_string(&report).unwrap();
    assert!(!text.contains("Звіт"), "{text}");
    assert!(text.lines().next().unwrap().contains(','));
    // No temporary file is left beside the reports.
    let left: Vec<_> = fs::read_dir(&data.dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".tmp"))
        .collect();
    assert!(left.is_empty(), "{left:?}");
}

#[test]
fn compare_with_compares_the_latest_snapshot_with_the_one_chosen() {
    let data = Data::new("with");
    data.profile("Home");
    let first = data.snapshot("Home");
    data.write("a.txt", "a");
    let second = data.snapshot("Home");
    data.write("b.txt", "b");
    let third = data.snapshot("Home");

    let report = data.path("r.txt");
    let with = first.to_string();
    let (code, _, err) = data.run(&[
        "compare",
        "-p",
        "Home",
        "-o",
        as_str(&report),
        "--with",
        &with,
    ]);
    assert_eq!(code, DONE, "{err}");
    assert!(err.starts_with(&format!("Comparing snapshots {first} and {third} ")));
    let text = fs::read_to_string(&report).unwrap();
    assert!(text.contains("a.txt") && text.contains("b.txt"));

    let (code, _, err) = data.run(&["compare", "-p", "Home", "-o", as_str(&report)]);
    assert_eq!(code, DONE, "{err}");
    assert!(err.starts_with(&format!("Comparing snapshots {second} and {third} ")));
    let text = fs::read_to_string(&report).unwrap();
    assert!(!text.contains("a.txt") && text.contains("b.txt"));

    let latest = third.to_string();
    let (code, _, err) = data.run(&["compare", "-p", "Home", "-o", "x.txt", "--with", &latest]);
    assert_eq!(code, FAILED);
    assert!(
        err.contains(&format!("snapshot {third} is the latest")),
        "{err}"
    );
    let (code, _, err) = data.run(&["compare", "-p", "Home", "-o", "x.txt", "--with", "999"]);
    assert_eq!(code, FAILED);
    assert!(err.contains("has no snapshot 999"), "{err}");
}

#[test]
fn compare_needs_two_snapshots() {
    let data = Data::new("too-few");
    data.profile("Home");
    let report = data.path("r.txt");
    let (code, _, err) = data.run(&["compare", "-p", "Home", "-o", as_str(&report)]);
    assert_eq!(code, FAILED);
    assert_eq!(
        err,
        "The snapshots were not compared: the profile “Home” has fewer than two.\n"
    );
    data.snapshot("Home");
    let (code, _, _) = data.run(&["compare", "-p", "Home", "-o", as_str(&report)]);
    assert_eq!(code, FAILED);
    assert!(!report.exists());
}

/// A write lock on the database of `profile`, held until the connection is dropped.
fn lock(profile: &ProfileFile) -> rusqlite::Connection {
    let conn = rusqlite::Connection::open(&profile.path).unwrap();
    conn.execute_batch("BEGIN EXCLUSIVE").unwrap();
    conn
}

#[test]
fn a_busy_profile_is_waited_for_once_told_and_given_up_past_the_timeout() {
    let data = Data::new("busy");
    let profile = data.profile("Home");
    data.snapshot("Home");
    data.snapshot("Home");

    // Another snapshot being saved: the profile reads, and the save waits.
    let saving = rusqlite::Connection::open(&profile.path).unwrap();
    saving.execute_batch("BEGIN IMMEDIATE").unwrap();
    let (code, out, err) = data.run(&["snapshot", "-p", "Home", "--timeout", "0"]);
    assert_eq!((code, out.as_str()), (FAILED, ""));
    assert_eq!(
        err,
        "Taking a snapshot of the profile “Home”…\n\
         Another revisor action holds the profile; waiting for it. Ctrl+C cancels.\n\
         The snapshot was not saved: the profile is busy (--timeout 0).\n"
    );
    drop(saving);

    // A compaction: the profile cannot even be read, nor found by its name.
    let held = lock(&profile);
    let (code, _, err) = data.run(&["snapshot", "-p", "Home", "--timeout", "0"]);
    assert_eq!(code, FAILED);
    assert_eq!(
        err,
        "Another revisor action holds the profile; waiting for it. Ctrl+C cancels.\n\
         The snapshot was not saved: the profile is busy (--timeout 0).\n"
    );
    let (code, _, err) = data.run(&["compare", "-p", "Home", "-o", "r.txt", "--timeout", "1"]);
    assert_eq!(code, FAILED);
    assert!(
        err.ends_with(
            "The snapshots were not compared: the profile was still busy after 1 s (--timeout).\n"
        ),
        "{err}"
    );

    // Without a timeout, the wait lasts until the profile is free.
    let freeing = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(500));
        drop(held);
    });
    let (code, out, err) = data.run(&["snapshot", "-p", "Home", "--progress"]);
    freeing.join().unwrap();
    assert_eq!(code, DONE, "{err}");
    assert!(out.trim().parse::<i64>().is_ok());
    let waits = err.matches("waiting for it").count();
    assert_eq!(waits, 1, "{err}");
}

#[test]
fn ctrl_c_cancels_the_work_with_code_1() {
    let data = Data::new("interrupt");
    let profile = data.profile("Home");
    // A snapshot being saved: the profile reads, and the save waits.
    let held = rusqlite::Connection::open(&profile.path).unwrap();
    held.execute_batch("BEGIN IMMEDIATE").unwrap();
    data.interrupt.cancel();
    let (code, out, err) = data.run(&["snapshot", "-p", "Home"]);
    drop(held);
    assert_eq!((code, out.as_str()), (FAILED, ""));
    assert!(err.ends_with("Snapshot canceled.\n"), "{err}");
    assert!(profiles::snapshots_of(&profile.path).unwrap().is_empty());
}
