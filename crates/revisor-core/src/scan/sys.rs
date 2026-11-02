//! Thin wrappers over the Win32 calls the scanner needs.

use std::ffi::{OsStr, OsString, c_void};
use std::fs::{File, OpenOptions};
use std::io;
use std::mem::{offset_of, size_of};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle;
use std::path::Path;
use std::ptr;

use windows_sys::Win32::Foundation::{ERROR_ACCESS_DENIED, ERROR_NO_MORE_FILES, GetLastError};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_BASIC_INFO, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
    FILE_ID_EXTD_DIR_INFO, FILE_ID_INFO, FILE_LIST_DIRECTORY, FILE_READ_ATTRIBUTES,
    FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_VER_GET_NEUTRAL, FileBasicInfo,
    FileIdExtdDirectoryInfo, FileIdInfo, GetFileInformationByHandleEx, GetFileVersionInfoExW,
    GetFileVersionInfoSizeExW, MAXIMUM_REPARSE_DATA_BUFFER_SIZE, VS_FIXEDFILEINFO, VerQueryValueW,
};
use windows_sys::Win32::System::IO::DeviceIoControl;
use windows_sys::Win32::System::Ioctl::FSCTL_GET_REPARSE_POINT;
use windows_sys::Win32::System::SystemServices::{
    IO_REPARSE_TAG_MOUNT_POINT, IO_REPARSE_TAG_SYMLINK,
};

use super::Version;

const SHARE_ALL: u32 = FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE;
const VS_FFI_SIGNATURE: u32 = 0xFEEF_04BD;
/// Set on reparse tags whose point names another file system object (junction, symlink…).
const NAME_SURROGATE_BIT: u32 = 0x2000_0000;
/// A symbolic link made in WSL: a name surrogate whose data is a version and a UTF-8 path.
const IO_REPARSE_TAG_LX_SYMLINK: u32 = 0xA000_001D;
const LIST_BUFFER_BYTES: usize = 64 * 1024;

/// Metadata of one directory entry, as the directory listing reports it.
pub struct RawEntry {
    pub name: OsString,
    pub attributes: u32,
    pub reparse_tag: u32,
    pub size: u64,
    pub created: i64,
    pub modified: i64,
    pub file_id: [u8; 16],
}

/// Metadata of a directory opened by path (the scan root).
pub struct DirInfo {
    pub attributes: u32,
    pub created: i64,
    pub modified: i64,
    pub file_id: [u8; 16],
    /// The serial number of the volume the directory is on.
    pub volume_serial: u64,
}

pub fn is_name_surrogate(reparse_tag: u32) -> bool {
    reparse_tag & NAME_SURROGATE_BIT != 0
}

/// True for the error that access rights give; an administrator with the backup privilege
/// does not get it.
pub fn is_access_denied(err: &io::Error) -> bool {
    err.raw_os_error() == Some(ERROR_ACCESS_DENIED as i32)
}

pub fn open_dir(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .access_mode(FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES)
        .share_mode(SHARE_ALL)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)
}

pub fn dir_info(dir: &File) -> io::Result<DirInfo> {
    let mut basic = FILE_BASIC_INFO::default();
    let mut id = FILE_ID_INFO::default();
    // SAFETY: each buffer is a properly sized and aligned struct of the requested class.
    unsafe {
        get_info(dir, FileBasicInfo, &mut basic)?;
        get_info(dir, FileIdInfo, &mut id)?;
    }
    Ok(DirInfo {
        attributes: basic.FileAttributes,
        created: basic.CreationTime,
        modified: basic.LastWriteTime,
        file_id: id.FileId.Identifier,
        volume_serial: id.VolumeSerialNumber,
    })
}

unsafe fn get_info<T>(file: &File, class: i32, out: &mut T) -> io::Result<()> {
    let ok = unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            class,
            (out as *mut T).cast(),
            size_of::<T>() as u32,
        )
    };
    if ok == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// Lists an open directory without opening its children. Skips `.` and `..`.
///
/// On an error midway the entries read so far are kept in `out`.
pub fn list_dir(dir: &File, out: &mut Vec<RawEntry>) -> io::Result<()> {
    // u64 keeps the buffer 8-byte aligned, as the records require.
    let mut buf = vec![0u64; LIST_BUFFER_BYTES / 8];
    loop {
        // SAFETY: the buffer is writable for its whole length.
        let ok = unsafe {
            GetFileInformationByHandleEx(
                dir.as_raw_handle(),
                FileIdExtdDirectoryInfo,
                buf.as_mut_ptr().cast(),
                LIST_BUFFER_BYTES as u32,
            )
        };
        if ok == 0 {
            // SAFETY: plain FFI call.
            return match unsafe { GetLastError() } {
                ERROR_NO_MORE_FILES => Ok(()),
                code => Err(io::Error::from_raw_os_error(code as i32)),
            };
        }
        let base = buf.as_ptr().cast::<u8>();
        let mut offset = 0usize;
        loop {
            // SAFETY: the system filled the buffer with a chain of FILE_ID_EXTD_DIR_INFO
            // records; NextEntryOffset and FileNameLength stay inside it.
            let (info, name) = unsafe {
                let rec = base.add(offset);
                let info = ptr::read_unaligned(rec.cast::<FILE_ID_EXTD_DIR_INFO>());
                let name_ptr = rec.add(offset_of!(FILE_ID_EXTD_DIR_INFO, FileName));
                let len = info.FileNameLength as usize / 2;
                let mut name = vec![0u16; len];
                ptr::copy_nonoverlapping(name_ptr, name.as_mut_ptr().cast::<u8>(), len * 2);
                (info, name)
            };
            if name != [b'.' as u16] && name != [b'.' as u16, b'.' as u16] {
                out.push(RawEntry {
                    name: OsString::from_wide(&name),
                    attributes: info.FileAttributes,
                    reparse_tag: info.ReparsePointTag,
                    size: info.EndOfFile as u64,
                    created: info.CreationTime,
                    modified: info.LastWriteTime,
                    file_id: info.FileId.Identifier,
                });
            }
            if info.NextEntryOffset == 0 {
                break;
            }
            offset += info.NextEntryOffset as usize;
        }
    }
}

/// Reads where a junction or symbolic link points, without following it.
///
/// Returns `None` for other kinds of reparse points.
pub fn link_target(path: &Path) -> io::Result<Option<OsString>> {
    let file = OpenOptions::new()
        .access_mode(FILE_READ_ATTRIBUTES)
        .share_mode(SHARE_ALL)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    let mut buf = vec![0u8; MAXIMUM_REPARSE_DATA_BUFFER_SIZE as usize];
    let mut returned = 0u32;
    // SAFETY: the output buffer is writable for its whole length.
    let ok = unsafe {
        DeviceIoControl(
            file.as_raw_handle(),
            FSCTL_GET_REPARSE_POINT,
            ptr::null(),
            0,
            buf.as_mut_ptr().cast(),
            buf.len() as u32,
            &mut returned,
            ptr::null_mut(),
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(parse_reparse_buffer(&buf[..returned as usize]))
}

/// Parses REPARSE_DATA_BUFFER (ntifs.h) for mount points, symbolic links and WSL
/// symbolic links.
fn parse_reparse_buffer(buf: &[u8]) -> Option<OsString> {
    let u16_at = |at: usize| -> Option<u16> {
        buf.get(at..at + 2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
    };
    let tag = u32::from_le_bytes(buf.get(0..4)?.try_into().ok()?);
    // Header: tag (4), data length (2), reserved (2); then four u16 name offsets/lengths.
    let path_buffer = match tag {
        IO_REPARSE_TAG_MOUNT_POINT => 16,
        IO_REPARSE_TAG_SYMLINK => 20, // plus a u32 Flags field
        IO_REPARSE_TAG_LX_SYMLINK => {
            // The data is a u32 version (2) followed by the target in UTF-8.
            let len = usize::from(u16_at(4)?);
            let target = buf.get(12..8 + len)?;
            return Some(String::from_utf8_lossy(target).into_owned().into());
        }
        _ => return None,
    };
    let name = |off_at: usize| -> Option<Vec<u16>> {
        let start = path_buffer + usize::from(u16_at(off_at)?);
        let len = usize::from(u16_at(off_at + 2)?);
        let bytes = buf.get(start..start + len)?;
        Some(
            bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|&c| u16::from_le_bytes(c))
                .collect(),
        )
    };
    let print = name(12)?;
    let target = if print.is_empty() {
        let substitute = name(8)?;
        // Drop the NT namespace prefix "\??\" so the target reads like a normal path.
        match substitute.strip_prefix(&[92, 63, 63, 92][..]) {
            Some(rest) => rest.to_vec(),
            None => substitute,
        }
    } else {
        print
    };
    Some(OsString::from_wide(&target))
}

/// Reads the numeric versions (VS_FIXEDFILEINFO) of an exe or dll.
///
/// `Ok(None)` means the file has no version resource; `Err` means it could not be read.
pub fn file_version(path: &Path) -> io::Result<Option<Version>> {
    let wide = to_wide(path.as_os_str());
    let mut handle = 0u32;
    // SAFETY: `wide` is NUL-terminated.
    let size =
        unsafe { GetFileVersionInfoSizeExW(FILE_VER_GET_NEUTRAL, wide.as_ptr(), &mut handle) };
    if size == 0 {
        return no_version(path);
    }
    let mut data = vec![0u8; size as usize];
    // SAFETY: `data` is writable for `size` bytes.
    let ok = unsafe {
        GetFileVersionInfoExW(
            FILE_VER_GET_NEUTRAL,
            wide.as_ptr(),
            0,
            size,
            data.as_mut_ptr().cast(),
        )
    };
    if ok == 0 {
        return no_version(path);
    }
    let mut fixed: *mut c_void = ptr::null_mut();
    let mut len = 0u32;
    let root = [b'\\' as u16, 0];
    // SAFETY: `data` holds the version block filled above; the returned pointer points into it.
    let ok = unsafe { VerQueryValueW(data.as_ptr().cast(), root.as_ptr(), &mut fixed, &mut len) };
    if ok == 0 || fixed.is_null() || (len as usize) < size_of::<VS_FIXEDFILEINFO>() {
        return Ok(None);
    }
    // SAFETY: checked above that the block is large enough.
    let info = unsafe { ptr::read_unaligned(fixed.cast::<VS_FIXEDFILEINFO>()) };
    if info.dwSignature != VS_FFI_SIGNATURE {
        return Ok(None);
    }
    let parts = |ms: u32, ls: u32| [(ms >> 16) as u16, ms as u16, (ls >> 16) as u16, ls as u16];
    Ok(Some(Version {
        file: parts(info.dwFileVersionMS, info.dwFileVersionLS),
        product: parts(info.dwProductVersionMS, info.dwProductVersionLS),
    }))
}

/// Tells a file without a version resource from one that could not be read. The version
/// calls report a file they cannot open (locked by another program, or closed by rights)
/// as one without a resource section, so the file is opened for reading to see which it
/// is: readable means there is no version, and the error otherwise says why.
fn no_version(path: &Path) -> io::Result<Option<Version>> {
    OpenOptions::new()
        .read(true)
        .share_mode(SHARE_ALL)
        .open(path)
        .map(|_| None)
}

fn to_wide(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reparse_buffer(tag: u32, flags: Option<u32>, substitute: &str, print: &str) -> Vec<u8> {
        let sub: Vec<u16> = substitute.encode_utf16().collect();
        let prn: Vec<u16> = print.encode_utf16().collect();
        let mut b = Vec::new();
        b.extend(tag.to_le_bytes());
        b.extend(0u16.to_le_bytes()); // data length, unused by the parser
        b.extend(0u16.to_le_bytes());
        b.extend(0u16.to_le_bytes());
        b.extend(((sub.len() * 2) as u16).to_le_bytes());
        b.extend(((sub.len() * 2) as u16).to_le_bytes());
        b.extend(((prn.len() * 2) as u16).to_le_bytes());
        if let Some(f) = flags {
            b.extend(f.to_le_bytes());
        }
        for c in sub.iter().chain(&prn) {
            b.extend(c.to_le_bytes());
        }
        b
    }

    #[test]
    fn parses_symlink_print_name() {
        let b = reparse_buffer(
            IO_REPARSE_TAG_SYMLINK,
            Some(0),
            r"\??\C:\target",
            r"C:\target",
        );
        assert_eq!(parse_reparse_buffer(&b), Some(OsString::from(r"C:\target")));
    }

    #[test]
    fn falls_back_to_substitute_name_without_nt_prefix() {
        let b = reparse_buffer(IO_REPARSE_TAG_MOUNT_POINT, None, r"\??\D:\data", "");
        assert_eq!(parse_reparse_buffer(&b), Some(OsString::from(r"D:\data")));
    }

    #[test]
    fn parses_the_utf8_target_of_a_wsl_symlink() {
        let target = "../bin/файл".as_bytes();
        let mut b = Vec::new();
        b.extend(IO_REPARSE_TAG_LX_SYMLINK.to_le_bytes());
        b.extend(((4 + target.len()) as u16).to_le_bytes());
        b.extend(0u16.to_le_bytes());
        b.extend(2u32.to_le_bytes());
        b.extend(target);
        // Whatever the driver appends after the data is not the target.
        b.extend(b"junk");
        assert_eq!(
            parse_reparse_buffer(&b),
            Some(OsString::from("../bin/файл"))
        );
        assert!(is_name_surrogate(IO_REPARSE_TAG_LX_SYMLINK));
    }

    #[test]
    fn ignores_other_reparse_tags() {
        let b = reparse_buffer(0x8000_0017, None, "x", "x");
        assert_eq!(parse_reparse_buffer(&b), None);
    }
}
