//! The window with the keyboard focus, and whether Alt is held down.

use std::ffi::c_void;

use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetFocus, GetKeyState, VK_MENU};
use windows_sys::Win32::UI::WindowsAndMessaging::GetClassNameW;

/// Whether Alt is held down, as far as the messages handled so far tell.
pub fn alt_down() -> bool {
    // SAFETY: no arguments but the key.
    unsafe { GetKeyState(i32::from(VK_MENU)) < 0 }
}

/// The window that has the keyboard focus in this thread, or null.
pub fn focused_window() -> *mut c_void {
    // SAFETY: no arguments.
    unsafe { GetFocus() }
}

/// Whether a window is a text field (an EDIT control), which needs Delete for itself.
pub fn is_text_field(hwnd: *mut c_void) -> bool {
    let mut class = [0u16; 32];
    // SAFETY: the buffer length is passed.
    let len = unsafe { GetClassNameW(hwnd, class.as_mut_ptr(), class.len() as i32) };
    len > 0 && String::from_utf16_lossy(&class[..len as usize]).eq_ignore_ascii_case("Edit")
}
