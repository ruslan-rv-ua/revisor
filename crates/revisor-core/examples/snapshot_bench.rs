//! Measures a whole snapshot the way the application takes it (TASK-8 #29): every folder
//! of a profile scanned with the default exclusions and the result saved in the profile's
//! database, in one transaction.
//!
//! Run it twice for the two targets of the brief: the first run right after a restart is
//! the cold-cache snapshot, the second the repeated one.
//!
//! Usage: `cargo run --release -p revisor-core --example snapshot_bench -- <folder>... [--db <path>]`

use std::fs;
use std::mem::size_of;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::AtomicBool;
use std::thread;
use std::time::{Duration, Instant};

use revisor_core::job::Cancel;
use revisor_core::snapshot::{Phase, Progress, take};
use revisor_core::store::{FolderState, ProfileSettings, Store};
use windows_sys::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows_sys::Win32::System::Threading::GetCurrentProcess;

const MIB: f64 = 1024.0 * 1024.0;
/// How often the progress line is printed.
const TICK: Duration = Duration::from_secs(5);

fn main() -> ExitCode {
    let mut folders = Vec::new();
    let mut db = std::env::temp_dir().join("revisor-snapshot-bench.db");
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--db" => match args.next() {
                Some(path) => db = path.into(),
                None => return usage(),
            },
            _ => folders.push(PathBuf::from(arg)),
        }
    }
    if folders.is_empty() {
        return usage();
    }
    match run(&folders, &db) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn run(folders: &[PathBuf], db: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if !db.exists() {
        let mut settings = ProfileSettings::new_profile();
        settings.name = "snapshot_bench".into();
        settings.scope.folders = folders.to_vec();
        drop(Store::create(db, &settings)?);
    }
    println!("database:      {}", db.display());
    for folder in folders {
        println!("folder:        {}", folder.display());
    }

    let progress = Progress::default();
    let cancel = Cancel::new();
    let done = AtomicBool::new(false);
    let started = Instant::now();
    let report = thread::scope(|s| {
        s.spawn(|| {
            while !done.load(std::sync::atomic::Ordering::Relaxed) {
                thread::sleep(TICK);
                if done.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }
                let phase = match progress.phase() {
                    Phase::Scanning => "scanning",
                    Phase::Saving | Phase::Saved => "saving",
                };
                println!(
                    "  {:6.1} s  {phase}: {} entries, {}",
                    started.elapsed().as_secs_f64(),
                    progress.entries(),
                    progress.folder().display()
                );
            }
        });
        let report = take(db, None, &progress, &cancel);
        done.store(true, std::sync::atomic::Ordering::Relaxed);
        report
    })?;

    println!(
        "entries:       {} (inaccessible {}, denied {}, versions unread {})",
        report.entries, report.inaccessible, report.denied, report.versions_unread
    );
    for folder in report.not_found() {
        let state = match &folder.state {
            FolderState::Missing => "missing".to_string(),
            FolderState::Unavailable(why) => format!("unavailable: {why:?}"),
            FolderState::Scanned { .. } => unreachable!("not_found gives no scanned folder"),
        };
        println!("not found:     {} ({state})", folder.path.display());
    }
    println!(
        "time:          {:.2} s (scan and save, as the application does it)",
        report.elapsed.as_secs_f64()
    );
    println!(
        "database:      {:.1} MiB",
        fs::metadata(db)?.len() as f64 / MIB
    );
    let (working_set, commit) = peak_memory();
    println!("peak memory:   working set {working_set:.1} MiB, private commit {commit:.1} MiB");
    Ok(())
}

fn usage() -> ExitCode {
    eprintln!("usage: snapshot_bench <folder>... [--db <path>]");
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
