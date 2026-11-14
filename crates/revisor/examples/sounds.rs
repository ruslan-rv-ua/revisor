//! Writes revisor's two sounds as WAV files, to listen to them apart from revisor
//! (TASK-46 #8): `cargo run -p revisor --example sounds <dir>`. The author chose them on
//! 2026-11-14 from four pairs: two notes up for «готово», two lower ones down for «без
//! результату».

use std::path::PathBuf;

use revisor::sound::{Chime, notes, wav};

fn main() {
    let dir = PathBuf::from(std::env::args().nth(1).expect("usage: sounds <dir>"));
    std::fs::create_dir_all(&dir).expect("cannot create the folder");
    for (name, chime) in [
        ("done.wav", Chime::Done),
        ("no-result.wav", Chime::NoResult),
    ] {
        let path = dir.join(name);
        std::fs::write(&path, wav(notes(chime))).expect("cannot write the file");
        println!("{}", path.display());
    }
}
