//! Measures how long comparing two snapshots takes, loading included, and how much
//! memory it needs.
//!
//! The two snapshots are saved to a database first: two scans of `<root>`, or `N`
//! generated entries and a copy with known changes. Then the same exe runs again as a
//! child process that only loads both snapshots and compares them, so its peak memory
//! is that of the comparison alone.
//!
//! Usage: `cargo run --release -p revisor-core --example diff_bench -- (--scan <root> | --generate N) [--db <path>]`

use std::ffi::OsString;
use std::fs;
use std::mem::size_of;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use revisor_core::diff::{ChangeKind, diff, path};
use revisor_core::scan::{Entry, EntryKind, ScanOptions, scan, synthetic};
use revisor_core::store::{ProfileSettings, Scope, Store};
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
        (true, _) => compare(&db),
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
    let no_cancel = AtomicBool::new(false);
    let started = Instant::now();
    let (old, new) = match input {
        Input::Scan(root) => {
            println!("input:         two scans of {}", root.display());
            let options = ScanOptions::default();
            let old = scan(root, &options, &no_cancel)?.entries;
            (old, scan(root, &options, &no_cancel)?.entries)
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
    let started = Instant::now();
    store.save(&settings.scope, Some("old"), &old, &no_cancel)?;
    store.save(&settings.scope, Some("new"), &new, &no_cancel)?;
    println!(
        "save both:     {:.2} s to {}",
        started.elapsed().as_secs_f64(),
        db.display()
    );
    drop((store, old, new));

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

fn compare(db: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open(db)?;
    let list = store.list()?;
    let [.., a, b] = list.as_slice() else {
        return Err("the database has fewer than two snapshots".into());
    };

    let started = Instant::now();
    let old = store.load(a.id)?;
    let new = store.load(b.id)?;
    let load_time = started.elapsed();
    let (loaded_ws, loaded_commit) = peak_memory();
    println!(
        "load both:     {:.2} s, {} and {} entries, peak working set {loaded_ws:.1} MiB, private commit {loaded_commit:.1} MiB",
        load_time.as_secs_f64(),
        old.len(),
        new.len()
    );

    let started = Instant::now();
    let diff = diff(&old, &new, &AtomicBool::new(false))?;
    let diff_time = started.elapsed();
    let count = |kind| diff.changes.iter().filter(|c| c.kind == kind).count();
    println!(
        "diff:          {:.2} s; added {}, removed {}, modified {}, inaccessible {}",
        diff_time.as_secs_f64(),
        count(ChangeKind::Added),
        count(ChangeKind::Removed),
        count(ChangeKind::Modified),
        diff.inaccessible.len()
    );
    println!(
        "load + diff:   {:.2} s",
        (load_time + diff_time).as_secs_f64()
    );
    for c in diff.changes.iter().take(SHOWN) {
        let path = match (c.old, c.new) {
            (Some(a), _) => path(&old, a),
            (None, Some(b)) => path(&new, b),
            (None, None) => unreachable!("a change is in at least one snapshot"),
        };
        println!("  {:?} {} {:#04x}", c.kind, path.display(), c.fields.0);
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
