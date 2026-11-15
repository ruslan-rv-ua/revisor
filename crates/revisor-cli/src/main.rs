//! revisor-cli, the command line (TASK-47, ADR-0015): `snapshot` takes a snapshot of a
//! profile, and `compare` writes the difference of two of its snapshots to a file, in the
//! format of the export from the window. A console program, so cmd, .bat files and
//! PowerShell wait for it and get its exit code; it stands on `revisor-ui` and
//! `revisor-core`, the same report and the same core as the window, and knows no wxDragon.
//!
//! stdout carries data only: the number of a new snapshot. Messages go to stderr: a line
//! when the work starts and a summary when it ends, as the window's status bar says them;
//! errors always, everything else unless `--quiet` (#9).

mod args;
mod interrupt;
#[cfg(test)]
mod tests;

use std::ffi::OsString;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use revisor_core::compare::{self, CompareError, CompareWork, Difference};
use revisor_core::elevation;
use revisor_core::job::{Cancel, Job, Lost, Outcome, Work};
use revisor_core::path::eq_ignore_case;
use revisor_core::profiles::{self, ProfileFile};
use revisor_core::settings::{AttributeFormat, CsvDelimiter, Settings, SizeFormat};
use revisor_core::snapshot::{self, SnapshotError, SnapshotWork};
use revisor_core::store::{self, Store, StoreError};

use args::{Action, Common, CompareArgs, ReportFormat, SnapshotArgs};
use revisor_ui::data::{self, PROFILES_DIR, SETTINGS_FILE};
use revisor_ui::difference::{List, View};
use revisor_ui::export::{self, ExportWork, Exported, Format};
use revisor_ui::failure::{self, reason};
use revisor_ui::format::{self as regional, Locale, RegionalFormat};
use revisor_ui::i18n::{En, Lang, Strings, Uk};
use revisor_ui::snapshot_text;

/// The exit codes (#8). Anything but 0 is a failure, so `$?` in PowerShell holds.
pub const DONE: u8 = 0;
/// The work failed, or Ctrl+C cancelled it.
pub const FAILED: u8 = 1;
/// The call is wrong.
pub const USAGE: u8 = 2;

/// How often the work is looked at.
const TICK: Duration = Duration::from_millis(100);
/// How often `--progress` writes a line (#9): seldom, as a screen reader reads each one.
const PROGRESS_EVERY: Duration = Duration::from_secs(10);

/// Runs revisor-cli with the arguments of this process and returns its exit code.
fn main() -> ExitCode {
    // An administrator reads with the backup privilege, as the window does (TASK-8).
    let _ = elevation::enable_backup_privilege();
    let env = Env {
        data_dir: data::dir(),
        elevated: elevation::is_elevated(),
        interrupt: interrupt::install(),
        progress_every: PROGRESS_EVERY,
    };
    let code = run_with(
        std::env::args_os().skip(1),
        &env,
        &mut io::stdout().lock(),
        &mut io::stderr().lock(),
    );
    ExitCode::from(code)
}

/// Where and how revisor-cli runs.
pub struct Env {
    /// The data folder, the window's: `data` beside the exe.
    pub data_dir: PathBuf,
    /// Whether the process runs as administrator: then no advice to run it so.
    pub elevated: bool,
    /// Cancelled by Ctrl+C.
    pub interrupt: Arc<Cancel>,
    /// How often `--progress` writes a line.
    pub progress_every: Duration,
}

/// Runs a call with `args`, the arguments without the name of the program, writing data to
/// `out` and messages to `err`; returns the exit code.
pub fn run_with(
    args: impl IntoIterator<Item = OsString>,
    env: &Env,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let call = args::parse(args);
    let s: &'static dyn Strings = match call.lang {
        Lang::En => &En,
        Lang::Uk => &Uk,
    };
    let version = revisor_core::version();
    let action = match call.action {
        Ok(action) => action,
        Err(e) => {
            let _ = writeln!(err, "{}", s.cli_usage_error(&e, call.command));
            return USAGE;
        }
    };
    let mut console = Console {
        s,
        out,
        err,
        quiet: false,
    };
    let result = match action {
        Action::Help(topic) => {
            let _ = write!(console.out, "{}", s.cli_help(topic, version));
            Ok(())
        }
        Action::Version => {
            let _ = writeln!(console.out, "revisor-cli {version}");
            Ok(())
        }
        Action::Snapshot(args) => {
            console.quiet = args.common.quiet;
            take_snapshot(&mut console, env, &args)
        }
        Action::Compare(args) => {
            console.quiet = args.common.quiet;
            compare(&mut console, env, &args)
        }
    };
    match result {
        Ok(()) => DONE,
        Err(Failed) => FAILED,
    }
}

/// The work failed; the reason has been told already.
#[derive(Debug)]
struct Failed;

/// Where revisor-cli writes, in the language of the call.
struct Console<'a> {
    s: &'static dyn Strings,
    out: &'a mut dyn Write,
    err: &'a mut dyn Write,
    quiet: bool,
}

impl Console<'_> {
    /// Tells a message, unless `--quiet`.
    fn say(&mut self, text: &str) {
        if !self.quiet {
            let _ = writeln!(self.err, "{text}");
        }
    }

    /// Tells why the work failed, always, and fails.
    fn fail<T>(&mut self, text: &str) -> Result<T, Failed> {
        let _ = writeln!(self.err, "{text}");
        Err(Failed)
    }
}

/// `revisor-cli snapshot` (#2).
fn take_snapshot(console: &mut Console, env: &Env, args: &SnapshotArgs) -> Result<(), Failed> {
    let s = console.s;
    let mut watch = Watch::new(&args.common);
    let Some(profile) = find_profile(console, env, &mut watch, &args.common.profile)? else {
        return cancelled_snapshot(console, &watch);
    };
    console.say(&s.cli_snapshot_started(&profile.name));
    let comment = args
        .comment
        .as_deref()
        .filter(|comment| !comment.is_empty());
    let job = Job::start(SnapshotWork::when_free(&profile.path, comment));
    let outcome = drive(
        console,
        env,
        &mut watch,
        job,
        |p| p.is_waiting(),
        |p| {
            let entries = regional::format_count(Locale::USER, p.entries());
            match p.phase() {
                snapshot::Phase::Scanning => {
                    s.snapshot_progress(p.entries(), &entries, &p.folder().to_string_lossy())
                }
                snapshot::Phase::Saving | snapshot::Phase::Saved => {
                    s.snapshot_saving(p.entries(), &entries)
                }
            }
        },
    );
    match outcome {
        Ok(Outcome::Done(report)) => {
            let _ = writeln!(console.out, "{}", report.id);
            let _ = console.out.flush();
            console.say(&snapshot_text::report(s, &report));
            if report.denied > 0 && !env.elevated {
                let denied = regional::format_count(Locale::USER, report.denied);
                console.say(&s.cli_run_as_administrator(&denied));
            }
            Ok(())
        }
        Ok(Outcome::Failed(SnapshotError::Store(e))) => {
            console.fail(&s.snapshot_failed(&reason(s, &e)))
        }
        Ok(Outcome::Failed(SnapshotError::Cancelled) | Outcome::Cancelled) => {
            cancelled_snapshot(console, &watch)
        }
        Err(Lost) => console.fail(&s.snapshot_failed(s.snapshot_stopped())),
    }
}

/// `revisor-cli compare` (#3).
fn compare(console: &mut Console, env: &Env, args: &CompareArgs) -> Result<(), Failed> {
    let s = console.s;
    let mut watch = Watch::new(&args.common);
    let Some(profile) = find_profile(console, env, &mut watch, &args.common.profile)? else {
        return cancelled_comparison(console, &watch);
    };
    let failed = |e: &dyn failure::Cause| s.comparison_failed(&reason(s, e));

    // The snapshots there are, read as the comparison reads them: waiting while busy.
    let listing = Job::start(ListWork(profile.path.clone()));
    let ids = match drive(
        console,
        env,
        &mut watch,
        listing,
        |w| w.load(Ordering::Relaxed),
        |_| s.waiting_for_database().to_string(),
    ) {
        Ok(Ok(ids)) => ids,
        Ok(Err(StoreError::Cancelled)) => return cancelled_comparison(console, &watch),
        Ok(Err(e)) => return console.fail(&failed(&e)),
        Err(Lost) => return console.fail(&s.comparison_failed(s.comparison_stopped())),
    };
    let (earlier, later) = match choose(&ids, args.with) {
        Ok(pair) => pair,
        Err(Choice::TooFew) => return console.fail(&s.cli_too_few_snapshots(&profile.name)),
        Err(Choice::Unknown(id)) => {
            return console.fail(&s.cli_unknown_snapshot(&profile.name, id));
        }
        Err(Choice::Latest(id)) => return console.fail(&s.cli_with_latest(id)),
    };
    console.say(&s.cli_compare_started(&profile.name, earlier, later));

    let job = Job::start(CompareWork::new(&profile.path, earlier, later));
    let difference: Difference = match drive(
        console,
        env,
        &mut watch,
        job,
        |p| p.phase() == compare::Phase::Waiting,
        |p| s.comparing_percent(p.percent()),
    ) {
        Ok(Outcome::Done(difference)) => difference,
        Ok(Outcome::Failed(CompareError::Cancelled) | Outcome::Cancelled) => {
            return cancelled_comparison(console, &watch);
        }
        Ok(Outcome::Failed(e)) => return console.fail(&failed(&e)),
        Err(Lost) => return console.fail(&s.comparison_failed(s.comparison_stopped())),
    };

    // The whole difference, in the formats of the window (#3, #5).
    let prefs = preferences(console, &env.data_dir);
    let mut list = List::new(
        s,
        Box::new(RegionalFormat(Locale::USER)),
        View::default(),
        prefs.size,
    );
    list.set_attribute_format(prefs.attributes);
    list.replace(difference, profile);
    let (Some(export), Some(summary)) = (list.export(), list.summary()) else {
        unreachable!("a list with a difference has its export and its summary")
    };
    let format = match args.report_format() {
        ReportFormat::Text => Format::Text,
        ReportFormat::Csv => Format::Csv {
            delimiter: export::csv_delimiter(args.delimiter.unwrap_or(prefs.delimiter)),
        },
    };
    let path = std::path::absolute(&args.output).unwrap_or_else(|_| args.output.clone());
    let job = Job::start(ExportWork::new(export, path, format));
    let exported: Exported = match drive(
        console,
        env,
        &mut watch,
        job,
        |_| false,
        |_| s.exporting().to_string(),
    ) {
        Ok(exported) => exported,
        Err(Lost) => return console.fail(&s.export_failed(s.export_stopped())),
    };
    match exported {
        Outcome::Done(path) => {
            console.say(&s.exported(&path.to_string_lossy()));
            console.say(&summary);
            Ok(())
        }
        Outcome::Failed(e) => console.fail(&s.export_failed(&reason(s, &e))),
        Outcome::Cancelled => console.fail(s.export_cancelled()),
    }
}

/// Fails a snapshot that was cancelled: by Ctrl+C, or by `--timeout`.
fn cancelled_snapshot(console: &mut Console, watch: &Watch) -> Result<(), Failed> {
    let s = console.s;
    match watch.timed_out {
        Some(limit) => console.fail(&s.snapshot_failed(&s.cli_busy_too_long(limit.as_secs()))),
        None => console.fail(s.snapshot_cancelled()),
    }
}

/// Fails a comparison that was cancelled: by Ctrl+C, or by `--timeout`.
fn cancelled_comparison(console: &mut Console, watch: &Watch) -> Result<(), Failed> {
    let s = console.s;
    match watch.timed_out {
        Some(limit) => console.fail(&s.comparison_failed(&s.cli_busy_too_long(limit.as_secs()))),
        None => console.fail(s.comparison_cancelled()),
    }
}

/// Why the snapshots to compare cannot be chosen (#7).
#[derive(Debug, PartialEq, Eq)]
pub enum Choice {
    /// The profile has fewer than two snapshots.
    TooFew,
    /// `--with` names no snapshot of the profile.
    Unknown(i64),
    /// `--with` names the latest snapshot, the one it is to be compared with.
    Latest(i64),
}

/// The snapshots `compare` compares, from the numbers `ids` of a profile's snapshots, the
/// earlier first (#3): the latest with the one before it, or with `with`. Numbers are
/// given out in the order the snapshots are taken and never again, so the latest is the
/// greatest.
pub fn choose(ids: &[i64], with: Option<i64>) -> Result<(i64, i64), Choice> {
    let mut sorted = ids.to_vec();
    sorted.sort_unstable();
    let Some(&latest) = sorted.last() else {
        return Err(Choice::TooFew);
    };
    match with {
        None => match sorted.len() {
            0 | 1 => Err(Choice::TooFew),
            n => Ok((sorted[n - 2], latest)),
        },
        Some(id) if id == latest => Err(Choice::Latest(id)),
        Some(id) if sorted.binary_search(&id).is_ok() => Ok((id.min(latest), id.max(latest))),
        Some(id) => Err(Choice::Unknown(id)),
    }
}

/// The profile named `name`, in any case (#7): its name as the window shows it; `None`
/// when the search was cancelled while it waited for a busy profile.
fn find_profile(
    console: &mut Console,
    env: &Env,
    watch: &mut Watch,
    name: &str,
) -> Result<Option<ProfileFile>, Failed> {
    let s = console.s;
    let dir = env.data_dir.join(PROFILES_DIR);
    let job = Job::start(ProfilesWork(dir.clone()));
    let waiting = |w: &AtomicBool| w.load(Ordering::Relaxed);
    let progress = |_: &AtomicBool| s.waiting_for_database().to_string();
    let mut found = match drive(console, env, watch, job, waiting, progress) {
        Ok(Ok(found)) => found,
        Ok(Err(Listing::Cancelled)) => return Ok(None),
        Ok(Err(Listing::Failed(e))) => {
            let folder = dir.to_string_lossy();
            return console.fail(&s.cli_profiles_not_read(&folder, &reason(s, &e)));
        }
        Err(Lost) => {
            let folder = dir.to_string_lossy();
            return console.fail(&s.cli_profiles_not_read(&folder, s.comparison_stopped()));
        }
    };
    if let Some(at) = found.iter().position(|p| eq_ignore_case(&p.name, name)) {
        return Ok(Some(found.swap_remove(at)));
    }
    let mut names: Vec<String> = found.into_iter().map(|p| p.name).collect();
    names.sort_by_key(|name| name.to_lowercase());
    console.fail(&s.cli_unknown_profile(name, &names))
}

/// How reading the profiles ended without them.
enum Listing {
    Failed(io::Error),
    Cancelled,
}

/// Reading the profiles in a folder as a job: a profile another revisor holds is waited
/// for by the rule of the store (ADR-0012), and the progress says while it waits, so the
/// wait is told and `--timeout` limits it (#10). A file that is no profile is left out,
/// as the window leaves it out of its list.
struct ProfilesWork(PathBuf);

impl Work for ProfilesWork {
    type Progress = AtomicBool;
    type Output = Result<Vec<ProfileFile>, Listing>;

    fn run(self, progress: &AtomicBool, cancel: &Cancel) -> Self::Output {
        let read_now = |path: &Path| Store::open_with_timeout(path, Duration::ZERO)?.settings();
        // Every file is read at once first; only the busy ones are waited for.
        let (mut found, unreadable) =
            profiles::list_when_free(&self.0, cancel, |_| true).map_err(Listing::Failed)?;
        for file in unreadable.into_iter().filter(|file| file.error.is_busy()) {
            let waiting = |waiting: bool| progress.store(waiting, Ordering::Relaxed);
            match store::retry_while_busy(cancel, waiting, || read_now(&file.path)) {
                Ok(settings) => found.push(ProfileFile {
                    path: file.path,
                    name: settings.name,
                }),
                Err(StoreError::Cancelled) => return Err(Listing::Cancelled),
                Err(_) => {}
            }
        }
        Ok(found)
    }
}

/// The settings of the window the command line follows: the formats of the text report
/// (#5) and the delimiter of CSV (#6).
struct Preferences {
    size: SizeFormat,
    attributes: AttributeFormat,
    delimiter: CsvDelimiter,
}

/// The settings of the window, or their defaults when there are none yet; a settings file
/// that cannot be read is told, and its defaults are taken, as the window does.
fn preferences(console: &mut Console, data_dir: &Path) -> Preferences {
    let path = data_dir.join(SETTINGS_FILE);
    let defaults = Preferences {
        size: SizeFormat::default(),
        attributes: AttributeFormat::default(),
        delimiter: CsvDelimiter::default(),
    };
    if !path.exists() {
        return defaults;
    }
    match Settings::open(&path) {
        Ok(settings) => Preferences {
            size: settings.size_format().unwrap_or_default(),
            attributes: settings.attribute_format().unwrap_or_default(),
            delimiter: settings.csv_delimiter().unwrap_or_default(),
        },
        Err(e) => {
            let s = console.s;
            console.say(&s.settings_not_opened(&path.to_string_lossy(), &reason(s, &e)));
            defaults
        }
    }
}

/// What the waits for a busy profile came to over a command (#10).
struct Watch {
    /// The limit of `--timeout`.
    limit: Option<Duration>,
    progress: bool,
    /// Whether the wait has been told: once per command.
    told: bool,
    /// How long the command has waited so far, over all its waits.
    waited: Duration,
    /// The limit, once the wait went past it and the work was cancelled.
    timed_out: Option<Duration>,
}

impl Watch {
    fn new(common: &Common) -> Self {
        Self {
            limit: common.timeout,
            progress: common.progress,
            told: false,
            waited: Duration::ZERO,
            timed_out: None,
        }
    }
}

/// Waits for `job` to end, looking at it every [`TICK`]: Ctrl+C cancels it; a wait for a
/// busy profile, which `waiting` tells from its progress, is told once and cancels it
/// past `--timeout`; and `--progress` writes what `progress` says every so often.
fn drive<W: Work>(
    console: &mut Console,
    env: &Env,
    watch: &mut Watch,
    mut job: Job<W>,
    waiting: impl Fn(&W::Progress) -> bool,
    progress: impl Fn(&W::Progress) -> String,
) -> Result<W::Output, Lost> {
    let s = console.s;
    let mut last = Instant::now();
    let mut said = Instant::now();
    loop {
        if let Some(output) = job.poll_within(TICK) {
            return output;
        }
        let now = Instant::now();
        if env.interrupt.is_cancelled() {
            job.cancel();
        }
        if waiting(job.progress()) {
            if !watch.told {
                watch.told = true;
                console.say(s.cli_waiting());
            }
            // Every wait past the limit is cancelled, not only the first: a job whose
            // database frees in the very cycle it is cancelled in goes on, and the next
            // wait of the command is still limited.
            if let Some(limit) = watch.limit
                && watch.waited >= limit
            {
                watch.timed_out = Some(limit);
                job.cancel();
            }
            watch.waited += now - last;
        }
        last = now;
        if watch.progress && now - said >= env.progress_every {
            said = now;
            console.say(&progress(job.progress()));
        }
    }
}

/// Reading the numbers of a profile's snapshots as a job, waiting while the database is
/// busy by the rule of the store (ADR-0012); the progress says while it waits.
struct ListWork(PathBuf);

impl Work for ListWork {
    type Progress = AtomicBool;
    type Output = Result<Vec<i64>, StoreError>;

    fn run(self, progress: &AtomicBool, cancel: &Cancel) -> Self::Output {
        let waiting = |waiting: bool| progress.store(waiting, Ordering::Relaxed);
        store::retry_while_busy(cancel, waiting, || {
            let store = Store::open_with_timeout(&self.0, Duration::ZERO)?;
            Ok(store.list()?.into_iter().map(|info| info.id).collect())
        })
    }
}
