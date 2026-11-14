//! Ctrl+C and Ctrl+Break on the command line (#8, #10): they cancel the work rather than
//! end the process, so a snapshot saves nothing and a report being written leaves no part
//! of a file, and revisor-cli ends with exit code 1.

use std::sync::{Arc, OnceLock};

use revisor_core::job::Cancel;
use windows_sys::Win32::Foundation::{FALSE, TRUE};
use windows_sys::Win32::System::Console::{CTRL_BREAK_EVENT, CTRL_C_EVENT, SetConsoleCtrlHandler};
use windows_sys::core::BOOL;

/// The token Ctrl+C cancels.
static INTERRUPT: OnceLock<Arc<Cancel>> = OnceLock::new();

/// Handles Ctrl+C and Ctrl+Break of the console from now on; returns the token they cancel.
pub fn install() -> Arc<Cancel> {
    let token = Arc::clone(INTERRUPT.get_or_init(|| Arc::new(Cancel::new())));
    // SAFETY: the handler is a function of this program that lives as long as it does.
    unsafe { SetConsoleCtrlHandler(Some(handler), TRUE) };
    token
}

/// Called by Windows on a thread of its own: Ctrl+C and Ctrl+Break cancel the work; the
/// other signals, such as the console being closed, end the process as usual.
unsafe extern "system" fn handler(signal: u32) -> BOOL {
    match signal {
        CTRL_C_EVENT | CTRL_BREAK_EVENT => {
            if let Some(token) = INTERRUPT.get() {
                token.cancel();
            }
            TRUE
        }
        _ => FALSE,
    }
}
