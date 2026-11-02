//! The rights of the process: whether it runs as administrator, and the backup privilege
//! with which an administrator reads folders closed even to administrators (TASK-8).

use std::io;
use std::mem::size_of;
use std::ptr;

use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_NOT_ALL_ASSIGNED, GetLastError, HANDLE, LUID,
};
use windows_sys::Win32::Security::{
    AdjustTokenPrivileges, GetTokenInformation, LUID_AND_ATTRIBUTES, LookupPrivilegeValueW,
    SE_BACKUP_NAME, SE_PRIVILEGE_ENABLED, TOKEN_ADJUST_PRIVILEGES, TOKEN_ELEVATION,
    TOKEN_PRIVILEGES, TOKEN_QUERY, TokenElevation,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

/// The access token of this process, closed on drop.
struct Token(HANDLE);

impl Token {
    fn open(access: u32) -> io::Result<Token> {
        let mut handle: HANDLE = ptr::null_mut();
        // SAFETY: the pseudo-handle of the current process needs no closing; `handle` is a
        // valid out pointer.
        let ok = unsafe { OpenProcessToken(GetCurrentProcess(), access, &mut handle) };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Token(handle))
    }
}

impl Drop for Token {
    fn drop(&mut self) {
        // SAFETY: the handle was opened by `Token::open` and is closed once.
        unsafe { CloseHandle(self.0) };
    }
}

/// Whether the process runs with administrator rights (an elevated token, which UAC gives
/// after its consent). A process that cannot tell counts as not elevated.
pub fn is_elevated() -> bool {
    let Ok(token) = Token::open(TOKEN_QUERY) else {
        return false;
    };
    let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
    let mut returned = 0u32;
    // SAFETY: the buffer is a TOKEN_ELEVATION, the class asked for.
    let ok = unsafe {
        GetTokenInformation(
            token.0,
            TokenElevation,
            (&mut elevation as *mut TOKEN_ELEVATION).cast(),
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut returned,
        )
    };
    ok != 0 && elevation.TokenIsElevated != 0
}

/// Enables the backup privilege (SeBackupPrivilege) for the process, with which a folder
/// opened for reading with `FILE_FLAG_BACKUP_SEMANTICS` is readable whatever its access
/// rights say. Returns whether the privilege is enabled now: administrators have it and
/// get it enabled, standard users do not have it at all.
pub fn enable_backup_privilege() -> io::Result<bool> {
    let token = Token::open(TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY)?;
    let mut luid = LUID {
        LowPart: 0,
        HighPart: 0,
    };
    // SAFETY: the privilege name is a NUL-terminated constant; `luid` is a valid out pointer.
    if unsafe { LookupPrivilegeValueW(ptr::null(), SE_BACKUP_NAME, &mut luid) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let privileges = TOKEN_PRIVILEGES {
        PrivilegeCount: 1,
        Privileges: [LUID_AND_ATTRIBUTES {
            Luid: luid,
            Attributes: SE_PRIVILEGE_ENABLED,
        }],
    };
    // SAFETY: `privileges` holds the one privilege it declares; no previous state is asked
    // for.
    let ok = unsafe {
        AdjustTokenPrivileges(token.0, 0, &privileges, 0, ptr::null_mut(), ptr::null_mut())
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    // The call succeeds even when the token does not hold the privilege; only the last
    // error tells.
    // SAFETY: plain FFI call.
    Ok(unsafe { GetLastError() } != ERROR_NOT_ALL_ASSIGNED)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_backup_privilege_is_enabled_exactly_for_administrators() {
        // A standard user has no SeBackupPrivilege to enable; an administrator has it.
        assert_eq!(enable_backup_privilege().unwrap(), is_elevated());
    }
}
