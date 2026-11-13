//! Typical folders (TASK-36): the folders installers and programs write to, which a profile
//! can take in at once. They are known folders of Windows, looked up on this computer
//! and added to a profile as plain paths, never as variables. The folders of the other
//! users of this computer can be offered too (TASK-43).

use std::ffi::OsString;
use std::io;
use std::os::windows::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::ptr;
use std::slice;

use windows_sys::Win32::Foundation::{ERROR_MORE_DATA, ERROR_NO_MORE_ITEMS, ERROR_SUCCESS};
use windows_sys::Win32::System::Com::CoTaskMemFree;
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_LOCAL_MACHINE, KEY_READ, RRF_RT_REG_SZ, RegCloseKey, RegEnumKeyExW, RegGetValueW,
    RegOpenKeyExW,
};
use windows_sys::Win32::UI::Shell::{
    FOLDERID_Desktop, FOLDERID_Documents, FOLDERID_Profile, FOLDERID_ProgramData,
    FOLDERID_ProgramFiles, FOLDERID_ProgramFilesX86, FOLDERID_PublicDesktop, FOLDERID_Windows,
    KF_FLAG_DEFAULT, SHGetKnownFolderPath,
};
use windows_sys::core::{GUID, PWSTR};

use crate::folders::{
    Addition, LocateError, Location, ProfileFolder, check_addition, locate, real_path,
};
use crate::path;

/// A typical folder. Only the user folder holds others: the user's AppData and desktop
/// are offered too, so that they can be taken without the rest (TASK-43).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypicalFolder {
    ProgramFiles,
    ProgramFilesX86,
    /// The common Start menu is in it too.
    ProgramData,
    Windows,
    PublicDesktop,
    /// The user folder as a whole, `%USERPROFILE%` (TASK-43).
    Profile,
    /// The user's AppData as a whole: Roaming (with the user's Start menu), Local (with
    /// the user's programs, `Local\Programs`) and LocalLow.
    AppData,
    Desktop,
}

impl TypicalFolder {
    /// The typical folders, in the order they are offered: the user's three last, like the
    /// three of each other user after them (TASK-43).
    pub const ALL: [TypicalFolder; 8] = [
        Self::ProgramFiles,
        Self::ProgramFilesX86,
        Self::ProgramData,
        Self::Windows,
        Self::PublicDesktop,
        Self::Profile,
        Self::AppData,
        Self::Desktop,
    ];

    /// The folders every user has, in the order they are offered.
    pub const OF_A_USER: [TypicalFolder; 3] = [Self::Profile, Self::AppData, Self::Desktop];

    /// Where the folder is on this computer for the current user, if Windows knows it.
    pub fn path(self) -> Option<PathBuf> {
        match self {
            Self::ProgramFiles => known_folder(&FOLDERID_ProgramFiles),
            Self::ProgramFilesX86 => known_folder(&FOLDERID_ProgramFilesX86),
            Self::ProgramData => known_folder(&FOLDERID_ProgramData),
            Self::Windows => known_folder(&FOLDERID_Windows),
            Self::PublicDesktop => known_folder(&FOLDERID_PublicDesktop),
            Self::Profile => known_folder(&FOLDERID_Profile),
            // Windows names no known folder for AppData itself, only for its parts.
            Self::AppData => known_folder(&FOLDERID_Profile).map(|p| p.join("AppData")),
            Self::Desktop => known_folder(&FOLDERID_Desktop),
        }
    }
}

/// A typical folder of this computer: which it is, whose, and where.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Typical {
    pub folder: TypicalFolder,
    /// The name of another user's profile folder, or `None` for the current user.
    pub user: Option<String>,
    pub path: PathBuf,
}

impl Typical {
    /// A typical folder of the current user or of all users.
    pub fn of_this_user(folder: TypicalFolder, path: PathBuf) -> Self {
        Self {
            folder,
            user: None,
            path,
        }
    }
}

/// The typical folders of this computer, with their paths, in the order they are offered.
/// A path is written as the folder really is on the disk where it is there: Windows may
/// give `C:\WINDOWS` for `C:\Windows`.
pub fn on_this_computer() -> Vec<Typical> {
    TypicalFolder::ALL
        .into_iter()
        .filter_map(|folder| {
            let path = folder.path()?;
            Some(Typical::of_this_user(
                folder,
                real_path(&path).unwrap_or(path),
            ))
        })
        .collect()
}

/// A profile of a user, as `ProfileList` in the registry records it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileRecord {
    pub sid: String,
    pub path: PathBuf,
}

/// The folders of the other users of this computer (TASK-43): three for each, in the order
/// of their names.
pub fn of_other_users() -> Vec<Typical> {
    let Some(current) = known_folder(&FOLDERID_Profile) else {
        return Vec::new();
    };
    other_users(&profile_list(), &current)
}

/// The folders of the users among `records` but the one whose profile is `current`:
/// only real users — local, of a domain or of Microsoft Entra — whose folder is on the
/// disk. Each is named after its profile folder, which it has even when Windows cannot
/// name the account any more; the users are in the order of these names, each with its
/// three folders in a row.
pub fn other_users(records: &[ProfileRecord], current: &Path) -> Vec<Typical> {
    let mut users: Vec<(String, &Path)> = Vec::new();
    for record in records {
        let path = record.path.as_path();
        let same = |other: &Path| path::eq_ignore_case_os(other.as_os_str(), path.as_os_str());
        if !is_user_sid(&record.sid)
            || same(current)
            || users.iter().any(|&(_, p)| same(p))
            || !path.is_dir()
        {
            continue;
        }
        let Some(name) = path.file_name() else {
            continue;
        };
        users.push((name.to_string_lossy().into_owned(), path));
    }
    users.sort_by(|(a, _), (b, _)| {
        path::fold(a.as_ref())
            .chars()
            .cmp(path::fold(b.as_ref()).chars())
            .then_with(|| a.cmp(b))
    });
    users
        .into_iter()
        .flat_map(|(name, profile)| {
            TypicalFolder::OF_A_USER.map(|folder| Typical {
                folder,
                user: Some(name.clone()),
                path: match folder {
                    TypicalFolder::AppData => profile.join("AppData"),
                    TypicalFolder::Desktop => profile.join("Desktop"),
                    _ => profile.to_path_buf(),
                },
            })
        })
        .collect()
}

/// Whether a SID is a real user's: local or of a domain (`S-1-5-21-…`) or of Microsoft
/// Entra (`S-1-12-1-…`), not a system account or a `.bak` copy of a profile.
fn is_user_sid(sid: &str) -> bool {
    let rest = sid
        .strip_prefix("S-1-5-21-")
        .or_else(|| sid.strip_prefix("S-1-12-1-"));
    rest.is_some_and(|rest| {
        rest.split('-')
            .all(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
    })
}

/// A typical folder a profile can take in.
#[derive(Clone, Debug)]
pub struct Offered {
    pub folder: TypicalFolder,
    /// The name of another user's profile folder, or `None` for the current user.
    pub user: Option<String>,
    pub path: PathBuf,
    location: Location,
}

impl Offered {
    /// Whether the folder is checked when the list opens (TASK-43): the user folder and
    /// Windows are large and seldom written to, and other users' folders are taken only
    /// on purpose.
    pub fn checked_at_first(&self) -> bool {
        self.user.is_none()
            && !matches!(self.folder, TypicalFolder::Profile | TypicalFolder::Windows)
    }

    fn as_profile_folder(&self) -> ProfileFolder {
        ProfileFolder {
            path: self.path.clone(),
            location: Some(self.location.clone()),
        }
    }
}

/// Which of the `candidates` a profile with the folders `existing` can take in: not those
/// that are not on the disk or that revisor does not support, not those the profile has
/// already or covers with a wider folder, and each place only once. A folder inside
/// another candidate is offered too (TASK-43).
pub fn offer(existing: &[ProfileFolder], candidates: &[Typical]) -> Vec<Offered> {
    let mut offered: Vec<Offered> = Vec::new();
    for candidate in candidates {
        let Ok(location) = locate(&candidate.path) else {
            continue;
        };
        if matches!(check_addition(existing, &location), Addition::Covered(_)) {
            continue;
        }
        if offered.iter().any(|o| o.location.parts == location.parts) {
            continue;
        }
        offered.push(Offered {
            folder: candidate.folder,
            user: candidate.user.clone(),
            path: candidate.path.clone(),
            location,
        });
    }
    offered
}

/// Whether some of the other users' folders among `candidates` are not offered because
/// revisor has no right to open them: without administrator's rights it cannot (TASK-43).
pub fn others_out_of_reach(candidates: &[Typical]) -> bool {
    candidates
        .iter()
        .filter(|c| c.user.is_some() && c.folder == TypicalFolder::Profile)
        .any(|c| {
            matches!(
                locate(&c.path),
                Err(LocateError::Io(e)) if e.kind() == io::ErrorKind::PermissionDenied
            )
        })
}

/// Adds the `chosen` typical folders to the profile's folders `entries`, at the end. A
/// typical folder that holds folders of the profile replaces them without asking: it was
/// chosen as a whole; a chosen folder inside another one chosen is left out (TASK-43).
/// Gives the index of the first folder added, if any was.
pub fn add(entries: &mut Vec<ProfileFolder>, chosen: &[Offered]) -> Option<usize> {
    let mut added: Vec<&Path> = Vec::new();
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
        added.push(&offered.path);
    }
    // A folder added first may have been replaced by a wider one added after it.
    entries
        .iter()
        .position(|f| added.contains(&f.path.as_path()))
}

/// The current user's folder «Документи», where the save dialog of the export starts
/// (TASK-39 #2); `None` when Windows does not know it.
pub fn documents() -> Option<PathBuf> {
    known_folder(&FOLDERID_Documents)
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

/// The key of the registry that lists the profiles of the users of this computer.
const PROFILE_LIST: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\ProfileList";

/// The profiles `ProfileList` records, with their folders as plain paths; nothing when it
/// cannot be read.
fn profile_list() -> Vec<ProfileRecord> {
    let list = wide_z(PROFILE_LIST);
    let mut key: HKEY = ptr::null_mut();
    // SAFETY: `list` is a null-terminated wide string and `key` receives the opened key.
    let status = unsafe { RegOpenKeyExW(HKEY_LOCAL_MACHINE, list.as_ptr(), 0, KEY_READ, &mut key) };
    if status != ERROR_SUCCESS {
        return Vec::new();
    }
    let mut records = Vec::new();
    for index in 0.. {
        // A SID is far shorter than the longest name of a key, 255 characters.
        let mut name = [0u16; 256];
        let mut len = name.len() as u32;
        // SAFETY: `key` is open, and `name` holds `len` wide characters.
        let status = unsafe {
            RegEnumKeyExW(
                key,
                index,
                name.as_mut_ptr(),
                &mut len,
                ptr::null(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
            )
        };
        if status == ERROR_NO_MORE_ITEMS {
            break;
        }
        if status != ERROR_SUCCESS {
            continue;
        }
        let sid = &name[..len as usize];
        if let Some(path) = profile_image_path(key, sid) {
            records.push(ProfileRecord {
                sid: String::from_utf16_lossy(sid),
                path,
            });
        }
    }
    // SAFETY: `key` was opened above and is closed once.
    unsafe { RegCloseKey(key) };
    records
}

/// `ProfileImagePath` of the profile `sid` under the open key `list`, with the variables
/// in it expanded.
fn profile_image_path(list: HKEY, sid: &[u16]) -> Option<PathBuf> {
    let sub: Vec<u16> = sid.iter().copied().chain([0]).collect();
    let value = wide_z("ProfileImagePath");
    let mut buffer: Vec<u16> = vec![0; 260];
    loop {
        let mut size = (buffer.len() * 2) as u32;
        // SAFETY: the names are null-terminated, and `buffer` holds `size` bytes. A
        // REG_EXPAND_SZ value comes back expanded, as RRF_NOEXPAND is not given.
        let status = unsafe {
            RegGetValueW(
                list,
                sub.as_ptr(),
                value.as_ptr(),
                RRF_RT_REG_SZ,
                ptr::null_mut(),
                buffer.as_mut_ptr().cast(),
                &mut size,
            )
        };
        match status {
            ERROR_SUCCESS => {
                let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
                let path = PathBuf::from(OsString::from_wide(&buffer[..len]));
                return (!path.as_os_str().is_empty()).then_some(path);
            }
            ERROR_MORE_DATA => buffer.resize((size as usize).div_ceil(2) + 1, 0),
            _ => return None,
        }
    }
}

/// `text` as a null-terminated wide string.
fn wide_z(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}

#[cfg(test)]
mod tests;
