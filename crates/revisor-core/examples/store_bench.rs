//! Measures how long saving and loading a snapshot takes, how large the database gets
//! and how much space VACUUM returns after a snapshot is deleted.
//!
//! Usage: `cargo run --release -p revisor-core --example store_bench -- (--scan <root> | --generate N) [--wal] [--db <path>]`

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use revisor_core::scan::{Progress, ScanOptions, scan, synthetic};
use revisor_core::store::{ProfileSettings, Scope, Snapshot, Store};

const MIB: f64 = 1024.0 * 1024.0;

enum Input {
    Scan(PathBuf),
    Generate(usize),
}

fn main() -> ExitCode {
    let mut input = None;
    let mut wal = false;
    let mut db = std::env::temp_dir().join("revisor-store-bench.db");
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
            "--wal" => wal = true,
            "--db" => match args.next() {
                Some(path) => db = path.into(),
                None => return usage(),
            },
            _ => return usage(),
        }
    }
    let Some(input) = input else {
        return usage();
    };
    match run(&input, wal, &db) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn run(input: &Input, wal: bool, db: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let no_cancel = AtomicBool::new(false);
    let started = Instant::now();
    let entries = match input {
        Input::Scan(root) => {
            println!("input:         scan of {}", root.display());
            scan(
                root,
                &ScanOptions::default(),
                &no_cancel,
                &Progress::default(),
            )?
            .entries
        }
        Input::Generate(n) => {
            println!("input:         {n} generated entries");
            synthetic(*n)
        }
    };
    println!(
        "entries:       {} ({:.2} s to collect)",
        entries.len(),
        started.elapsed().as_secs_f64()
    );
    let count = entries.len();
    let snapshot = Snapshot::of_tree(entries);

    for suffix in ["", "-journal", "-wal", "-shm"] {
        let _ = fs::remove_file(with_suffix(db, suffix));
    }
    let settings = ProfileSettings {
        name: "store_bench".into(),
        scope: Scope::default(),
    };
    drop(Store::create(db, &settings)?);
    if wal {
        rusqlite::Connection::open(db)?.pragma_update(None, "journal_mode", "WAL")?;
    }
    println!(
        "database:      {} (journal {})",
        db.display(),
        if wal { "WAL" } else { "rollback" }
    );
    let mut store = Store::open(db)?;

    let started = Instant::now();
    let first = store.save(&snapshot, &no_cancel)?;
    let save_time = started.elapsed();
    let one = db_size(db);
    println!(
        "save:          {:.2} s, database {:.1} MiB ({:.0} B per entry)",
        save_time.as_secs_f64(),
        one as f64 / MIB,
        one as f64 / count as f64
    );

    let started = Instant::now();
    let loaded = store.load_folder(first, 0, &AtomicBool::new(false))?;
    println!(
        "load:          {:.2} s, {} entries",
        started.elapsed().as_secs_f64(),
        loaded.len()
    );
    drop(loaded);

    let started = Instant::now();
    store.save(&snapshot, &no_cancel)?;
    let two = db_size(db);
    println!(
        "second save:   {:.2} s, database {:.1} MiB",
        started.elapsed().as_secs_f64(),
        two as f64 / MIB
    );

    let started = Instant::now();
    store.delete(first)?;
    println!(
        "delete first:  {:.2} s, database {:.1} MiB",
        started.elapsed().as_secs_f64(),
        db_size(db) as f64 / MIB
    );

    let started = Instant::now();
    let compacted = store.compact()?;
    println!(
        "vacuum:        {:.2} s, database {:.1} MiB, {:.1} MiB returned",
        started.elapsed().as_secs_f64(),
        compacted.after as f64 / MIB,
        two.saturating_sub(compacted.after) as f64 / MIB
    );
    Ok(())
}

fn usage() -> ExitCode {
    eprintln!("usage: store_bench (--scan <root> | --generate N) [--wal] [--db <path>]");
    ExitCode::FAILURE
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(suffix);
    s.into()
}

/// Size of the database file together with its write-ahead log, in bytes. The log is
/// checkpointed and truncated first, so in WAL mode the size is that of the data alone.
fn db_size(db: &Path) -> u64 {
    if let Ok(conn) = rusqlite::Connection::open(db) {
        // A no-op in rollback mode.
        let _ = conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()));
    }
    ["", "-wal"]
        .iter()
        .filter_map(|suffix| fs::metadata(with_suffix(db, suffix)).ok())
        .map(|m| m.len())
        .sum()
}
