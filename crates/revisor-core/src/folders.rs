//! Profile folders: where a folder really is, whether revisor supports its volume, and
//! whether a new folder overlaps the folders a profile already has.

use std::ffi::OsString;
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle;
use std::path::{Path, PathBuf};
use std::ptr;

use windows_sys::Win32::Storage::FileSystem::{
    FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    GETFINALPATHNAMEBYHANDLE_FLAGS, GetFinalPathNameByHandleW, GetVolumeInformationByHandleW,
    VOLUME_NAME_DOS, VOLUME_NAME_GUID, VOLUME_NAME_NT,
};

use crate::names::upcase;
use crate::scan::verbatim;

/// Where a folder really is on disk: its path on its volume, with the volume named by its
/// GUID where it has one. Every path that leads to the folder — in another case, with a
/// `\` at the end, through a junction or a `subst` drive — gives the same location.
#[derive(Clone, Debug)]
pub struct Location {
    /// The parts of the real path, upper-cased the way NTFS compares names.
    parts: Vec<Vec<u16>>,
    /// The path as it was given and as it really is with a drive letter, in the same form:
    /// a profile folder no longer on the disk is compared with these.
    written: Vec<Vec<Vec<u16>>>,
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
    /// The folder is on a volume other than NTFS; holds the file system's name, such as
    /// "FAT32".
    NotNtfs(String),
    /// The folder is on a network share.
    Network,
    /// The folder could not be opened or examined.
    Io(io::Error),
}

impl fmt::Display for LocateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LocateError::NotNtfs(fs) => write!(f, "the folder is on a {fs} volume, not NTFS"),
            LocateError::Network => f.write_str("the folder is on a network share"),
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
        return Err(LocateError::Network);
    }
    let fs = file_system(&file).map_err(LocateError::Io)?;
    if let Some(e) = volume_error(&examined, &fs) {
        return Err(e);
    }
    // A volume the Mount Manager does not know has no GUID name, only a DOS or NT one.
    let real = final_path(&file, VOLUME_NAME_GUID)
        .or_else(|_| final_path(&file, VOLUME_NAME_DOS))
        .or_else(|_| final_path(&file, VOLUME_NAME_NT))
        .map_err(LocateError::Io)?;
    let given = std::path::absolute(path).map_err(LocateError::Io)?;
    let mut written = vec![parts(&given)];
    written.extend(dos.ok().map(|dos| parts(&dos)));
    Ok(Location {
        parts: parts(&real),
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
                let path = parts(&folder.path);
                (
                    new.written.iter().any(|w| w.starts_with(&path)),
                    new.written.iter().any(|w| path.starts_with(w)),
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
    let real = final_path(&open(path)?, VOLUME_NAME_DOS)?;
    let wide: Vec<u16> = real.as_os_str().encode_wide().collect();
    let plain = match strip(&wide, r"\\?\UNC\") {
        Some(rest) => r"\\".encode_utf16().chain(rest.iter().copied()).collect(),
        None => strip(&wide, r"\\?\").unwrap_or(&wide).to_vec(),
    };
    Ok(PathBuf::from(OsString::from_wide(&plain)))
}

/// Why revisor does not support a folder whose final path and file system are these, if
/// it does not.
fn volume_error(final_path: &str, file_system: &str) -> Option<LocateError> {
    if is_network(final_path) {
        return Some(LocateError::Network);
    }
    (!file_system.eq_ignore_ascii_case("NTFS"))
        .then(|| LocateError::NotNtfs(file_system.to_string()))
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
        .open(verbatim(path)?)
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

/// The parts of a path, without its `\\?\` prefix and empty parts, upper-cased the way
/// NTFS compares names: two paths name the same folder when their parts are equal.
pub(crate) fn parts(path: &Path) -> Vec<Vec<u16>> {
    let wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    let wide = strip(&wide, r"\\?\").unwrap_or(&wide);
    wide.split(|&c| c == u16::from(b'\\'))
        .filter(|part| !part.is_empty())
        .map(|part| part.iter().map(|&c| upcase(c)).collect())
        .collect()
}

fn strip<'a>(wide: &'a [u16], prefix: &str) -> Option<&'a [u16]> {
    let prefix: Vec<u16> = prefix.encode_utf16().collect();
    let head = wide.get(..prefix.len())?;
    head.iter()
        .zip(&prefix)
        .all(|(&a, &b)| upcase(a) == b)
        .then(|| &wide[prefix.len()..])
}

#[cfg(test)]
mod tests;
