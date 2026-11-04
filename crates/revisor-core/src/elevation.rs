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

/// An access token of this process, closed on drop.
struct Token(HANDLE);

impl Token {
    /// The primary token of the process.
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
    enable_backup_privilege_on(&token)
}

/// The LUID of the backup privilege on this system.
fn backup_privilege() -> io::Result<LUID> {
    let mut luid = LUID {
        LowPart: 0,
        HighPart: 0,
    };
    // SAFETY: the privilege name is a NUL-terminated constant; `luid` is a valid out pointer.
    if unsafe { LookupPrivilegeValueW(ptr::null(), SE_BACKUP_NAME, &mut luid) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(luid)
}

/// Enables the backup privilege on `token`, opened with `TOKEN_ADJUST_PRIVILEGES`.
fn enable_backup_privilege_on(token: &Token) -> io::Result<bool> {
    let privileges = TOKEN_PRIVILEGES {
        PrivilegeCount: 1,
        Privileges: [LUID_AND_ATTRIBUTES {
            Luid: backup_privilege()?,
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

    use windows_sys::Win32::Security::{
        ImpersonateSelf, RevertToSelf, SecurityImpersonation, TokenPrivileges,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentThread, OpenThreadToken};

    /// The test thread impersonating the process's own identity, on a copy of the process
    /// token that is the thread's alone, reverted on drop.
    struct SelfImpersonation;

    impl SelfImpersonation {
        fn start() -> io::Result<SelfImpersonation> {
            // SAFETY: plain FFI call.
            if unsafe { ImpersonateSelf(SecurityImpersonation) } == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(SelfImpersonation)
        }

        /// The impersonation token of this thread.
        fn token(&self, access: u32) -> io::Result<Token> {
            let mut handle: HANDLE = ptr::null_mut();
            // SAFETY: the pseudo-handle of the current thread needs no closing; `handle` is
            // a valid out pointer.
            let ok = unsafe { OpenThreadToken(GetCurrentThread(), access, 1, &mut handle) };
            if ok == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(Token(handle))
        }
    }

    impl Drop for SelfImpersonation {
        fn drop(&mut self) {
            // SAFETY: plain FFI call.
            unsafe { RevertToSelf() };
        }
    }

    /// Whether `token` holds the backup privilege enabled.
    fn backup_privilege_is_enabled_on(token: &Token) -> bool {
        let luid = backup_privilege().unwrap();
        let mut needed = 0u32;
        // SAFETY: a NULL buffer of size 0 only asks for the size needed.
        unsafe { GetTokenInformation(token.0, TokenPrivileges, ptr::null_mut(), 0, &mut needed) };
        let mut buffer = vec![0u8; needed as usize];
        // SAFETY: the buffer is as large as the previous call asked for.
        let ok = unsafe {
            GetTokenInformation(
                token.0,
                TokenPrivileges,
                buffer.as_mut_ptr().cast(),
                needed,
                &mut needed,
            )
        };
        assert_ne!(ok, 0, "{}", io::Error::last_os_error());
        // SAFETY: the buffer starts with a TOKEN_PRIVILEGES whose `Privileges` array of
        // `PrivilegeCount` entries follows the count in the same buffer.
        let count = unsafe { buffer.as_ptr().cast::<TOKEN_PRIVILEGES>().read_unaligned() }
            .PrivilegeCount as usize;
        let first = size_of::<u32>();
        (0..count)
            .map(|i| {
                let at = first + i * size_of::<LUID_AND_ATTRIBUTES>();
                // SAFETY: entry `i` of `count` lies inside the buffer the system filled.
                unsafe {
                    buffer
                        .as_ptr()
                        .add(at)
                        .cast::<LUID_AND_ATTRIBUTES>()
                        .read_unaligned()
                }
            })
            .any(|p| {
                p.Luid.LowPart == luid.LowPart
                    && p.Luid.HighPart == luid.HighPart
                    && p.Attributes & SE_PRIVILEGE_ENABLED != 0
            })
    }

    #[test]
    fn an_administrator_gets_the_backup_privilege_and_nobody_fails() {
        // The privilege is enabled on a token this thread alone holds, never on the
        // process: the tests about folders closed by rights run in the same process, and
        // on an administrator's machine (the CI runner) the privilege would open those
        // folders to them whatever order or thread the tests ran in (TASK-20).
        let this_thread = SelfImpersonation::start().unwrap();
        let token = this_thread
            .token(TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY)
            .unwrap();
        // An administrator has SeBackupPrivilege and gets it enabled; a standard user
        // usually has none (a Backup Operator does), and the call still succeeds.
        let enabled = enable_backup_privilege_on(&token).unwrap();
        assert!(enabled || !is_elevated());
        assert_eq!(backup_privilege_is_enabled_on(&token), enabled);
        drop(this_thread);

        let process = Token::open(TOKEN_QUERY).unwrap();
        assert!(
            !backup_privilege_is_enabled_on(&process),
            "the process token must stay as it was"
        );
    }
}
