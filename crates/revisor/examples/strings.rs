//! Writes every interface string of revisor in English and Ukrainian to a text file, for
//! proofreading with a screen reader (TASK-17). A development tool, not part of revisor.
//!
//! `cargo run -p revisor --example strings [file]`; the file defaults to `strings.txt` in
//! the current folder.

use std::path::PathBuf;

fn main() {
    let file = std::env::args_os()
        .nth(1)
        .map_or_else(|| PathBuf::from("strings.txt"), PathBuf::from);
    let text = revisor::catalog();
    if let Err(error) = std::fs::write(&file, text.as_bytes()) {
        eprintln!("{} could not be written: {error}", file.display());
        std::process::exit(1);
    }
    println!("{}", file.display());
}
