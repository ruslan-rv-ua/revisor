//! Profile folders: where a folder really is, whether revisor supports its volume, and
//! whether a new folder overlaps the folders a profile already has.

use std::ffi::OsString;
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io;
use std::os::windows::ffi::OsStringExt;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle;
use std::path::{Path, PathBuf};
use std::ptr;

use windows_sys::Win32::Storage::FileSystem::{
    FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    GETFINALPATHNAMEBYHANDLE_FLAGS, GetFinalPathNameByHandleW, GetVolumeInformationByHandleW,
    VOLUME_NAME_DOS, VOLUME_NAME_GUID, VOLUME_NAME_NT,
};

use crate::path::{self, Folded};
use crate::store::Unavailable;

/// Where a folder really is on disk: its path on its volume, with the volume named by its
/// GUID where it has one. Every path that leads to the folder — in another case, with a
/// `\` at the end, through a junction or a `subst` drive — gives the same location.
#[derive(Clone, Debug)]
pub struct Location {
    /// The parts of the real path ([`path::parts`]).
    parts: Vec<Folded>,
    /// The path as it was given and as it really is with a drive letter, in the same form:
    /// a profile folder no longer on the disk is compared with these.
    written: Vec<Vec<Folded>>,
}

/// A folder of a profile, and where it really is while it is on the disk.
#[derive(Clone, Debug)]
pub struct ProfileFolder {
    pub path: PathBuf,
    /// `None` when the folder is not on the disk any more or cannot be examined.
    pub location: Option<Location>,
}

impl ProfileFolder {
    /// Finds where the folder at `path` is now.
    pub fn new(path: PathBuf) -> Self {
        let location = locate(&path).ok();
        Self { path, location }
    }
}

#[derive(Debug)]
pub enum LocateError {
    /// revisor does not support the folder's volume: [`Unavailable::NotNtfs`] or
    /// [`Unavailable::Network`], the only reasons [`locate`] gives.
    Unavailable(Unavailable),
    /// The folder could not be opened or examined.
    Io(io::Error),
}

impl fmt::Display for LocateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LocateError::Unavailable(Unavailable::NotNtfs(fs)) => {
                write!(f, "the folder is on a {fs} volume, not NTFS")
            }
            LocateError::Unavailable(Unavailable::Network) => {
                f.write_str("the folder is on a network share")
            }
            LocateError::Unavailable(reason) => write!(f, "the folder is unavailable: {reason:?}"),
            LocateError::Io(e) => write!(f, "cannot examine the folder: {e}"),
        }
    }
}

impl std::error::Error for LocateError {}

/// What adding a folder to a profile would do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Addition {
    /// The folder can be added as it is.
    New,
    /// The folder is the profile's folder at this index or lies inside it, so its files
    /// are covered already.
    Covered(usize),
    /// The folder holds the profile's folders at these indices and can replace them.
    Replaces(Vec<usize>),
}

/// Finds where the folder at `path` really is and checks that revisor supports its volume:
/// only local NTFS volumes are.
pub fn locate(path: &Path) -> Result<Location, LocateError> {
    let file = open(path).map_err(LocateError::Io)?;
    let dos = final_path(&file, VOLUME_NAME_DOS);
    let examined = match &dos {
        Ok(dos) => dos.clone(),
        Err(_) => final_path(&file, VOLUME_NAME_NT).map_err(LocateError::Io)?,
    };
    let examined = examined.to_string_lossy();
    if is_network(&examined) {
        return Err(LocateError::Unavailable(Unavailable::Network));
    }
    let fs = file_system(&file).map_err(LocateError::Io)?;
    if let Some(reason) = volume_error(&examined, &fs) {
        return Err(LocateError::Unavailable(reason));
    }
    // A volume the Mount Manager does not know has no GUID name, only a DOS or NT one.
    let real = final_path(&file, VOLUME_NAME_GUID)
        .or_else(|_| final_path(&file, VOLUME_NAME_DOS))
        .or_else(|_| final_path(&file, VOLUME_NAME_NT))
        .map_err(LocateError::Io)?;
    let given = std::path::absolute(path).map_err(LocateError::Io)?;
    let mut written = vec![path::parts(given.as_os_str())];
    written.extend(dos.ok().map(|dos| path::parts(dos.as_os_str())));
    Ok(Location {
        parts: path::parts(real.as_os_str()),
        written,
    })
}

/// What adding the folder at `new` to a profile with the folders `existing` would do.
pub fn check_addition(existing: &[ProfileFolder], new: &Location) -> Addition {
    let mut replaced = Vec::new();
    for (i, folder) in existing.iter().enumerate() {
        let (inside, holds) = match &folder.location {
            Some(location) => (
                new.parts.starts_with(&location.parts),
                location.parts.starts_with(&new.parts),
            ),
            // A folder that is not on the disk any more is compared by its path, with the
            // new folder's path as given and as it really is.
            None => {
                let parts = path::parts(folder.path.as_os_str());
                (
                    new.written.iter().any(|w| w.starts_with(&parts)),
                    new.written.iter().any(|w| parts.starts_with(w)),
                )
            }
        };
        if inside {
            return Addition::Covered(i);
        }
        if holds {
            replaced.push(i);
        }
    }
    if replaced.is_empty() {
        Addition::New
    } else {
        Addition::Replaces(replaced)
    }
}

/// The real location of an existing file or folder as a plain path with a drive letter:
/// junctions, symbolic links, `subst` drives and 8.3 names are resolved.
pub fn real_path(path: &Path) -> io::Result<PathBuf> {
    Ok(path::plain(&final_path(&open(path)?, VOLUME_NAME_DOS)?))
}

/// Why revisor does not support a folder whose final path and file system are these, if
/// it does not.
fn volume_error(final_path: &str, file_system: &str) -> Option<Unavailable> {
    if is_network(final_path) {
        return Some(Unavailable::Network);
    }
    (!file_system.eq_ignore_ascii_case("NTFS"))
        .then(|| Unavailable::NotNtfs(file_system.to_string()))
}

/// Whether a final path names a network share; such a share reports the file system of
/// the server's disk, often NTFS.
fn is_network(final_path: &str) -> bool {
    let upper = final_path.to_ascii_uppercase();
    upper.starts_with(r"\\?\UNC\") || upper.starts_with(r"\DEVICE\MUP\")
}

/// Opens a file or folder only to ask about it: no access rights are needed for that.
fn open(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .access_mode(0)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(path::verbatim(&std::path::absolute(path)?))
}

fn final_path(file: &File, flags: GETFINALPATHNAMEBYHANDLE_FLAGS) -> io::Result<PathBuf> {
    let mut buf = vec![0u16; 512];
    loop {
        // SAFETY: the buffer is writable for its whole length.
        let len = unsafe {
            GetFinalPathNameByHandleW(
                file.as_raw_handle(),
                buf.as_mut_ptr(),
                buf.len() as u32,
                flags,
            )
        } as usize;
        if len == 0 {
            return Err(io::Error::last_os_error());
        }
        if len < buf.len() {
            return Ok(PathBuf::from(OsString::from_wide(&buf[..len])));
        }
        // Too small: `len` is the size needed, the terminating null included.
        buf.resize(len, 0);
    }
}

/// The name of the file system of the volume a file is on, such as "NTFS".
fn file_system(file: &File) -> io::Result<String> {
    let mut name = [0u16; 261];
    // SAFETY: the buffer is writable for the length passed; the other outputs are optional.
    let ok = unsafe {
        GetVolumeInformationByHandleW(
            file.as_raw_handle(),
            ptr::null_mut(),
            0,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            name.as_mut_ptr(),
            name.len() as u32,
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    let len = name.iter().position(|&c| c == 0).unwrap_or(name.len());
    Ok(String::from_utf16_lossy(&name[..len]))
}

#[cfg(test)]
mod tests;
