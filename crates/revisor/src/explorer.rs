//! «Показати в Провіднику» (TASK-14 #11, #12), apart from Windows' shell: what File
//! Explorer is asked to show for the path of an entry, from what the disk has there now.
//!
//! The entry on disk is selected in its folder: a file, a folder or a link itself, not the
//! link's target. An entry no longer on disk leaves its folder to open without a
//! selection, or, when that is gone too, the nearest folder above that is there; when no
//! folder of the path is on disk — a drive not connected — nothing opens.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// What the disk has at a path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnDisk {
    /// Nothing, nor the folder above it, nor maybe the drive.
    Missing,
    /// A folder, or a link to one whose target is there.
    Folder,
    /// Something else: a file, or a link whose target is gone.
    Other,
    /// Something Windows does not tell about, as when rights keep its folder closed:
    /// File Explorer is let try all the same.
    Unknown,
}

/// What File Explorer is asked to show for an entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reveal {
    /// The entry is on disk: its folder, with the entry selected (#11).
    Select(PathBuf),
    /// The entry is gone: the nearest folder of its path on disk, nothing selected (#12).
    Open(PathBuf),
    /// No folder of the path is on disk (#12).
    Nowhere,
}

/// What to show for the entry at `path`, by what `on_disk` says the disk has.
pub fn reveal(path: &Path, on_disk: impl Fn(&Path) -> OnDisk) -> Reveal {
    if on_disk(path) != OnDisk::Missing {
        return Reveal::Select(path.to_path_buf());
    }
    let mut folder = path.parent();
    while let Some(at) = folder.filter(|at| !at.as_os_str().is_empty()) {
        match on_disk(at) {
            OnDisk::Folder | OnDisk::Unknown => return Reveal::Open(at.to_path_buf()),
            OnDisk::Missing | OnDisk::Other => folder = at.parent(),
        }
    }
    Reveal::Nowhere
}

/// What the disk has at `path` now. A link counts as there even when its target is gone,
/// as File Explorer shows the link itself.
pub fn on_disk(path: &Path) -> OnDisk {
    match fs::metadata(path) {
        Ok(found) if found.is_dir() => OnDisk::Folder,
        Ok(_) => OnDisk::Other,
        Err(e) if is_missing(&e) => match fs::symlink_metadata(path) {
            Ok(_) => OnDisk::Other,
            Err(e) if is_missing(&e) => OnDisk::Missing,
            Err(_) => OnDisk::Unknown,
        },
        Err(_) => OnDisk::Unknown,
    }
}

/// Whether an error says there is nothing at a path: the file, the path, the drive or its
/// medium, the network share, a path Windows cannot even name, or one that goes on below
/// a file.
fn is_missing(error: &io::Error) -> bool {
    const ERROR_FILE_NOT_FOUND: i32 = 2;
    const ERROR_PATH_NOT_FOUND: i32 = 3;
    const ERROR_INVALID_DRIVE: i32 = 15;
    const ERROR_NOT_READY: i32 = 21;
    const ERROR_BAD_NETPATH: i32 = 53;
    const ERROR_BAD_NET_NAME: i32 = 67;
    const ERROR_INVALID_NAME: i32 = 123;
    const ERROR_BAD_PATHNAME: i32 = 161;
    const ERROR_DIRECTORY: i32 = 267;
    matches!(
        error.raw_os_error(),
        Some(
            ERROR_FILE_NOT_FOUND
                | ERROR_PATH_NOT_FOUND
                | ERROR_INVALID_DRIVE
                | ERROR_NOT_READY
                | ERROR_BAD_NETPATH
                | ERROR_BAD_NET_NAME
                | ERROR_INVALID_NAME
                | ERROR_BAD_PATHNAME
                | ERROR_DIRECTORY
        )
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A disk that has the folders and files given, and nothing else.
    fn disk<'a>(folders: &'a [&str], files: &'a [&str]) -> impl Fn(&Path) -> OnDisk + 'a {
        move |path: &Path| {
            let path = path.to_string_lossy();
            if folders.iter().any(|f| *f == path) {
                OnDisk::Folder
            } else if files.iter().any(|f| *f == path) {
                OnDisk::Other
            } else {
                OnDisk::Missing
            }
        }
    }

    #[test]
    fn an_entry_on_disk_is_selected_in_its_folder() {
        let on_disk = disk(&[r"C:\", r"C:\App", r"C:\App\data"], &[r"C:\App\app.exe"]);
        for entry in [r"C:\App\app.exe", r"C:\App\data"] {
            assert_eq!(
                reveal(Path::new(entry), &on_disk),
                Reveal::Select(entry.into())
            );
        }
        // What Windows does not tell about is shown all the same.
        let closed = |_: &Path| OnDisk::Unknown;
        assert_eq!(
            reveal(Path::new(r"C:\Secret\x"), closed),
            Reveal::Select(r"C:\Secret\x".into())
        );
    }

    #[test]
    fn a_gone_entry_opens_the_nearest_folder_of_its_path() {
        let on_disk = disk(&[r"C:\", r"C:\App"], &[r"C:\App\old"]);
        // Its own folder.
        assert_eq!(
            reveal(Path::new(r"C:\App\gone.txt"), &on_disk),
            Reveal::Open(r"C:\App".into())
        );
        // Its folder is gone too, or a file took its place: the folder above.
        assert_eq!(
            reveal(Path::new(r"C:\App\plugins\x\gone.dll"), &on_disk),
            Reveal::Open(r"C:\App".into())
        );
        assert_eq!(
            reveal(Path::new(r"C:\App\old\gone.txt"), &on_disk),
            Reveal::Open(r"C:\App".into())
        );
        // Up to the root of the drive.
        assert_eq!(
            reveal(Path::new(r"C:\Games\setup.exe"), &on_disk),
            Reveal::Open(r"C:\".into())
        );
    }

    #[test]
    fn nothing_opens_when_no_folder_of_the_path_is_on_disk() {
        let on_disk = disk(&[r"C:\"], &[]);
        assert_eq!(
            reveal(Path::new(r"E:\Backup\photo.jpg"), &on_disk),
            Reveal::Nowhere
        );
        assert_eq!(reveal(Path::new(r"E:\"), &on_disk), Reveal::Nowhere);
    }

    #[test]
    fn the_disk_says_what_is_there() {
        let dir = std::env::temp_dir().join(format!("revisor-explorer-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("folder")).unwrap();
        fs::write(dir.join("file.txt"), b"x").unwrap();
        assert_eq!(on_disk(&dir.join("folder")), OnDisk::Folder);
        assert_eq!(on_disk(&dir.join("file.txt")), OnDisk::Other);
        assert_eq!(on_disk(&dir.join("gone.txt")), OnDisk::Missing);
        assert_eq!(on_disk(&dir.join(r"gone\deeper.txt")), OnDisk::Missing);
        // Under a file, nothing is there either.
        assert_eq!(on_disk(&dir.join(r"file.txt\x")), OnDisk::Missing);
        assert_eq!(
            reveal(&dir.join(r"gone\deeper.txt"), on_disk),
            Reveal::Open(dir.clone())
        );
        assert_eq!(
            reveal(&dir.join("file.txt"), on_disk),
            Reveal::Select(dir.join("file.txt"))
        );
        fs::remove_dir_all(&dir).unwrap();
    }
}
