//! Measures how long a scan takes and how much memory it needs.
//!
//! Usage: `cargo run --release -p revisor-core --example scan_bench -- <root> [--threads N] [--no-versions]`

use std::mem::size_of;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use revisor_core::scan::{Entry, ScanOptions, scan};
use windows_sys::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows_sys::Win32::System::Threading::GetCurrentProcess;

const MIB: f64 = 1024.0 * 1024.0;

fn main() -> ExitCode {
    let mut root = None;
    let mut options = ScanOptions::default();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--threads" => {
                options.threads = match args.next().and_then(|n| n.parse().ok()) {
                    Some(n) => n,
                    None => return usage(),
                }
            }
            "--no-versions" => options.read_versions = false,
            _ if root.is_none() => root = Some(PathBuf::from(arg)),
            _ => return usage(),
        }
    }
    let Some(root) = root else {
        return usage();
    };

    let started = Instant::now();
    let result = scan(&root, &options, &AtomicBool::new(false));
    let elapsed = started.elapsed();
    let scan = match result {
        Ok(scan) => scan,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let s = &scan.stats;
    let names: usize = scan.entries.iter().map(|e| e.name.len()).sum();

    println!("root:          {}", root.display());
    println!(
        "options:       threads={}, versions={}",
        options.threads, options.read_versions
    );
    println!(
        "entries:       {} (dirs {}, files {}, links {}, inaccessible {})",
        scan.entries.len(),
        s.dirs,
        s.files,
        s.links,
        s.inaccessible
    );
    println!("time:          {:.2} s", elapsed.as_secs_f64());
    println!(
        "versions:      {} files, {:.2} s summed over threads ({:.1}% of wall time x threads)",
        s.versions_read,
        s.version_time.as_secs_f64(),
        100.0 * s.version_time.as_secs_f64() / (elapsed.as_secs_f64() * options.threads as f64)
    );
    println!(
        "entry size:    {} B + names {:.1} MiB",
        size_of::<Entry>(),
        names as f64 / MIB
    );
    let (working_set, commit) = peak_memory();
    println!("peak memory:   working set {working_set:.1} MiB, private commit {commit:.1} MiB");
    ExitCode::SUCCESS
}

fn usage() -> ExitCode {
    eprintln!("usage: scan_bench <root> [--threads N] [--no-versions]");
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
