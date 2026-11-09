//! The Win32 calls wxDragon 0.9.24 does not expose (ADR-0008), one submodule per concern
//! (TASK-32):
//!
//! - [`language`]: the interface language of Windows and the order of names;
//! - [`accessibility`]: accessible names, the live region of the status bar;
//! - [`focus`]: the window with the keyboard focus and the state of Alt;
//! - [`lists`]: the rows, the selection, the styles, the sort arrow and the empty text of
//!   report lists — the snapshot lists and the difference lists alike;
//! - [`frame`]: the subclass of the main window, for access keys by key position, the bold
//!   first item of a context menu and the changes of the system colours;
//! - [`theme`]: whether a contrast theme of Windows is on;
//! - [`format`]: counts, numbers, moments and sizes in a regional format, and the letters
//!   of attributes;
//! - [`shell`]: the shield icon of a menu item, File Explorer and the restart as
//!   administrator.

pub mod accessibility;
pub mod focus;
pub mod format;
pub mod frame;
pub mod language;
pub mod lists;
pub mod shell;
pub mod theme;

/// `text` as a NUL-terminated UTF-16 string, as the Win32 calls take it.
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}
