//! Measures how long comparing two snapshots takes, loading included, and how much
//! memory it needs.
//!
//! The two snapshots are saved to a database first: two scans of `<root>`, or `N`
//! generated entries and a copy with known changes. Then the same exe runs again as a
//! child process that only compares them, loading included, the way the application
//! does — as a background job — so its peak memory is that of the comparison alone. The
//! target: ~1 million entries in 3 s (TASK-11 #19).
//!
//! Usage: `cargo run --release -p revisor-core --example diff_bench -- (--scan <root> | --generate N) [--db <path>]`

use std::ffi::OsString;
use std::fs;
use std::mem::size_of;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Instant;

use revisor_core::compare::CompareWork;
use revisor_core::diff::ChangeKind;
use revisor_core::exclude::{DEFAULT_EXCLUSIONS, SystemEnvironment, expand};
use revisor_core::job::{Cancel, Job, Outcome};
use revisor_core::scan::{Entry, EntryKind, Progress, ScanOptions, scan, synthetic};
use revisor_core::store::{ProfileSettings, Scope, Snapshot, SnapshotExclusion, Store};
use windows_sys::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows_sys::Win32::System::Threading::GetCurrentProcess;

const MIB: f64 = 1024.0 * 1024.0;
const FILE_ATTRIBUTE_ARCHIVE: u32 = 0x20;
/// How many changes are printed, to check by eye that they are real.
const SHOWN: usize = 20;

enum Input {
    Scan(PathBuf),
    Generate(usize),
}

fn main() -> ExitCode {
    let mut input = None;
    let mut compare_only = false;
    let mut db = std::env::temp_dir().join("revisor-diff-bench.db");
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--scan" => match args.next() {
                Some(root) => input = Some(Input::Scan(root.into())),
                None => return usage(),
            },
            "--generate" => match args.next().and_then(|n| n.parse().ok()) {
                Some(n) => input = Some(Input::Generate(n)),
                None => return usage(),
            },
            "--db" => match args.next() {
                Some(path) => db = path.into(),
                None => return usage(),
            },
            // Used by the child process.
            "--compare-only" => compare_only = true,
            _ => return usage(),
        }
    }
    let result = match (compare_only, input) {
        (true, _) => compare_last_two(&db),
        (false, Some(input)) => prepare(&input, &db),
        (false, None) => return usage(),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn prepare(input: &Input, db: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let no_cancel = Cancel::new();
    let started = Instant::now();
    let (old, new) = match input {
        Input::Scan(root) => {
            println!("input:         two scans of {}", root.display());
            let options = ScanOptions::default();
            let progress = Progress::default();
            let old = scan(root, &options, &no_cancel, &progress)?.entries;
            (old, scan(root, &options, &no_cancel, &progress)?.entries)
        }
        Input::Generate(n) => {
            println!("input:         {n} generated entries and a changed copy");
            let old = synthetic(*n);
            let new = with_changes(&old);
            (old, new)
        }
    };
    println!(
        "entries:       {} and {} ({:.2} s to collect)",
        old.len(),
        new.len(),
        started.elapsed().as_secs_f64()
    );

    let _ = fs::remove_file(db);
    let settings = ProfileSettings {
        name: "diff_bench".into(),
        scope: Scope::default(),
    };
    let mut store = Store::create(db, &settings)?;
    // The default exclusions, as a profile has them: the comparison matches every entry
    // against them, which costs time too.
    let exclusions: Vec<SnapshotExclusion> = DEFAULT_EXCLUSIONS
        .iter()
        .map(|pattern| SnapshotExclusion {
            pattern: pattern.to_string(),
            applied: expand(pattern, &SystemEnvironment)
                .applied()
                .map(String::from),
        })
        .collect();
    let started = Instant::now();
    for (comment, entries) in [("old", old), ("new", new)] {
        let mut snapshot = Snapshot::of_tree(entries);
        snapshot.comment = Some(comment.into());
        snapshot.exclusions = exclusions.clone();
        store.save(&snapshot, &no_cancel)?;
    }
    println!(
        "save both:     {:.2} s to {}",
        started.elapsed().as_secs_f64(),
        db.display()
    );
    drop(store);

    let status = Command::new(std::env::current_exe()?)
        .arg("--compare-only")
        .arg("--db")
        .arg(db)
        .status()?;
    if !status.success() {
        return Err("the comparing process failed".into());
    }
    Ok(())
}

/// A copy of a generated tree with about 1% of its files changed and 1000 files added.
fn with_changes(old: &[Entry]) -> Vec<Entry> {
    let mut new = old.to_vec();
    for (i, e) in new.iter_mut().enumerate() {
        if e.kind == EntryKind::File && i % 100 == 1 {
            e.size += 1;
            e.modified += 1;
        }
    }
    let dirs: Vec<u32> = (0..old.len() as u32)
        .filter(|&i| old[i as usize].kind == EntryKind::Dir)
        .collect();
    for k in 0..1000 {
        new.push(Entry {
            parent: Some(dirs[k * dirs.len() / 1000]),
            name: OsString::from(format!("added {k}.txt")).into_boxed_os_str(),
            kind: EntryKind::File,
            attributes: FILE_ATTRIBUTE_ARCHIVE,
            size: 1,
            ..old[0].clone()
        });
    }
    new
}

/// Compares the last two snapshots of the database the way the application does: as a
/// background job that loads both from the database and compares them in one go.
fn compare_last_two(db: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let list = Store::open(db)?.list()?;
    let [.., a, b] = list.as_slice() else {
        return Err("the database has fewer than two snapshots".into());
    };

    let started = Instant::now();
    let job = Job::start(CompareWork::new(db, a.id, b.id));
    let difference = match job.wait()? {
        Outcome::Done(difference) => difference,
        Outcome::Failed(e) => return Err(e.into()),
        Outcome::Cancelled => return Err("the comparison was cancelled".into()),
    };
    let elapsed = started.elapsed();
    let changes = || {
        difference
            .folders
            .iter()
            .flat_map(|f| f.diff.changes.iter())
    };
    let count = |kind| changes().filter(|c| c.kind == kind).count();
    let loaded: usize = difference
        .folders
        .iter()
        .map(|f| f.old.len() + f.new.len())
        .sum();
    let inaccessible: usize = difference
        .folders
        .iter()
        .map(|f| f.diff.inaccessible.len())
        .sum();
    println!(
        "load + diff:   {:.2} s, {loaded} entries loaded; added {}, deleted {}, modified {}, inaccessible {inaccessible}, warnings {}",
        elapsed.as_secs_f64(),
        count(ChangeKind::Added),
        count(ChangeKind::Deleted),
        count(ChangeKind::Modified),
        difference.warnings.len()
    );
    for folder in &difference.folders {
        for c in folder.diff.changes.iter().take(SHOWN) {
            let path = c.path(&folder.old, &folder.new);
            println!("  {:?} {} {:#04x}", c.kind, path.display(), c.fields.0);
        }
    }
    let (working_set, commit) = peak_memory();
    println!("peak memory:   working set {working_set:.1} MiB, private commit {commit:.1} MiB");
    Ok(())
}

fn usage() -> ExitCode {
    eprintln!("usage: diff_bench (--scan <root> | --generate N) [--db <path>]");
    ExitCode::FAILURE
}

/// Peak working set and peak private commit of this process, in MiB.
fn peak_memory() -> (f64, f64) {
    let mut counters = PROCESS_MEMORY_COUNTERS {
        cb: size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        ..Default::default()
    };
    // SAFETY: `counters` is a valid, correctly sized out parameter.
    let ok = unsafe { GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb) };
    if ok == 0 {
        return (0.0, 0.0);
    }
    (
        counters.PeakWorkingSetSize as f64 / MIB,
        counters.PeakPagefileUsage as f64 / MIB,
    )
}
