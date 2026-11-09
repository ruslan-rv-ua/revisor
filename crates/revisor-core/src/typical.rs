//! Typical folders (TASK-36): the folders installers and programs write to, which a profile
//! can take in at once. They are known folders of Windows, looked up on this computer
//! and added to a profile as plain paths, never as variables.

use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::ptr;
use std::slice;

use windows_sys::Win32::System::Com::CoTaskMemFree;
use windows_sys::Win32::UI::Shell::{
    FOLDERID_Desktop, FOLDERID_Profile, FOLDERID_ProgramData, FOLDERID_ProgramFiles,
    FOLDERID_ProgramFilesX86, FOLDERID_PublicDesktop, FOLDERID_Windows, KF_FLAG_DEFAULT,
    SHGetKnownFolderPath,
};
use windows_sys::core::{GUID, PWSTR};

use crate::folders::{Addition, Location, ProfileFolder, check_addition, locate, real_path};

/// A typical folder. None of them holds another.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypicalFolder {
    ProgramFiles,
    ProgramFilesX86,
    /// The common Start menu is in it too.
    ProgramData,
    /// The user's AppData as a whole: Roaming (with the user's Start menu), Local (with
    /// the user's programs, `Local\Programs`) and LocalLow.
    AppData,
    Windows,
    Desktop,
    PublicDesktop,
}

impl TypicalFolder {
    /// The typical folders, in the order they are offered.
    pub const ALL: [TypicalFolder; 7] = [
        Self::ProgramFiles,
        Self::ProgramFilesX86,
        Self::ProgramData,
        Self::AppData,
        Self::Windows,
        Self::Desktop,
        Self::PublicDesktop,
    ];

    /// Where the folder is on this computer for the current user, if Windows knows it.
    pub fn path(self) -> Option<PathBuf> {
        match self {
            Self::ProgramFiles => known_folder(&FOLDERID_ProgramFiles),
            Self::ProgramFilesX86 => known_folder(&FOLDERID_ProgramFilesX86),
            Self::ProgramData => known_folder(&FOLDERID_ProgramData),
            // Windows names no known folder for AppData itself, only for its parts.
            Self::AppData => known_folder(&FOLDERID_Profile).map(|p| p.join("AppData")),
            Self::Windows => known_folder(&FOLDERID_Windows),
            Self::Desktop => known_folder(&FOLDERID_Desktop),
            Self::PublicDesktop => known_folder(&FOLDERID_PublicDesktop),
        }
    }
}

/// The typical folders of this computer, with their paths, in the order they are offered.
/// A path is written as the folder really is on the disk where it is there: Windows may
/// give `C:\WINDOWS` for `C:\Windows`.
pub fn on_this_computer() -> Vec<(TypicalFolder, PathBuf)> {
    TypicalFolder::ALL
        .into_iter()
        .filter_map(|folder| {
            let path = folder.path()?;
            Some((folder, real_path(&path).unwrap_or(path)))
        })
        .collect()
}

/// A typical folder a profile can take in.
#[derive(Clone, Debug)]
pub struct Offered {
    pub folder: TypicalFolder,
    pub path: PathBuf,
    location: Location,
}

/// Which of the `candidates` a profile with the folders `existing` can take in: not those
/// that are not on the disk or that revisor does not support, not those the profile has
/// already or covers with a wider folder, and each place only once.
pub fn offer(existing: &[ProfileFolder], candidates: &[(TypicalFolder, PathBuf)]) -> Vec<Offered> {
    let mut offered: Vec<Offered> = Vec::new();
    for (folder, path) in candidates {
        let Ok(location) = locate(path) else {
            continue;
        };
        if matches!(check_addition(existing, &location), Addition::Covered(_)) {
            continue;
        }
        let seen: Vec<ProfileFolder> = offered.iter().map(Offered::as_profile_folder).collect();
        if matches!(check_addition(&seen, &location), Addition::Covered(_)) {
            continue;
        }
        offered.push(Offered {
            folder: *folder,
            path: path.clone(),
            location,
        });
    }
    offered
}

/// Adds the `chosen` typical folders to the profile's folders `entries`, at the end. A
/// typical folder that holds folders of the profile replaces them without asking: it was
/// chosen as a whole. Gives the index of the first folder added, if any was.
pub fn add(entries: &mut Vec<ProfileFolder>, chosen: &[Offered]) -> Option<usize> {
    let mut first: Option<&Path> = None;
    for offered in chosen {
        match check_addition(entries, &offered.location) {
            Addition::Covered(_) => continue,
            Addition::Replaces(inside) => {
                for &i in inside.iter().rev() {
                    entries.remove(i);
                }
            }
            Addition::New => {}
        }
        entries.push(offered.as_profile_folder());
        first.get_or_insert(&offered.path);
    }
    let first = first?;
    entries.iter().position(|f| f.path == first)
}

impl Offered {
    fn as_profile_folder(&self) -> ProfileFolder {
        ProfileFolder {
            path: self.path.clone(),
            location: Some(self.location.clone()),
        }
    }
}

/// The path of a known folder of Windows for the current user.
fn known_folder(id: &GUID) -> Option<PathBuf> {
    let mut path: PWSTR = ptr::null_mut();
    // SAFETY: `id` is a valid GUID and `path` receives a string Windows allocates, which
    // is freed below whether or not the call succeeds.
    let hr = unsafe { SHGetKnownFolderPath(id, KF_FLAG_DEFAULT as _, ptr::null_mut(), &mut path) };
    let result = (hr >= 0 && !path.is_null()).then(|| {
        // SAFETY: on success `path` is a null-terminated wide string.
        let len = unsafe { (0..).take_while(|&i| *path.add(i) != 0).count() };
        // SAFETY: `len` wide characters before the terminating null are readable.
        let wide = unsafe { slice::from_raw_parts(path, len) };
        PathBuf::from(OsString::from_wide(wide))
    });
    // SAFETY: the string was allocated by SHGetKnownFolderPath (null is allowed).
    unsafe { CoTaskMemFree(path as *const _) };
    result.filter(|p| !p.as_os_str().is_empty())
}

#[cfg(test)]
mod tests;
