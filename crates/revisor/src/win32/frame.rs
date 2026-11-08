//! The subclass of the main window: WM_MENUCHAR finds a menu's access keys by the key's
//! position, so "а" in the Ukrainian layout opens «Файл (&F)», WM_INITMENUPOPUP makes the
//! first item of a context menu its bold default when the menu asks for it, and a change of
//! the system colours is noted for the theme (TASK-34).

use std::cell::Cell;
use std::ffi::c_void;
use std::rc::Rc;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetKeyboardLayout, VkKeyScanExW};
use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetMenuItemCount, GetMenuStringW, HMENU, MF_BYPOSITION, MF_SYSMENU, MNC_EXECUTE,
    SPI_SETHIGHCONTRAST, SetMenuDefaultItem, WM_INITMENUPOPUP, WM_MENUCHAR, WM_NCDESTROY,
    WM_SETTINGCHANGE, WM_SYSCOLORCHANGE,
};

use crate::menus::find_access_key;

/// The main window, subclassed. Its context menus open through [`MainFrame::popup`], which
/// says whether the first item is bold and takes it back once the menu is closed, so no
/// other popup can find the mark left over (TASK-32).
pub struct MainFrame {
    shared: Rc<Shared>,
}

/// What the window and its subclass share.
#[derive(Default)]
struct Shared {
    /// Whether the popup opening now gets its first item as the bold default; the subclass
    /// reads it at WM_INITMENUPOPUP and clears it.
    bold_first: Cell<bool>,
    /// Whether the system colours changed since the window last asked: a contrast theme of
    /// Windows switched on or off, or revisor's own theme applied (TASK-34 #6).
    colours_changed: Cell<bool>,
}

/// The id of this subclass among the window's subclasses.
const MAIN_FRAME_SUBCLASS: usize = 1;

/// Subclasses `hwnd`, the main window, a live window of this thread.
pub fn subclass(hwnd: *mut c_void) -> MainFrame {
    let shared = Rc::new(Shared::default());
    let data = Rc::into_raw(Rc::clone(&shared)) as usize;
    // SAFETY: a live window of this thread; the procedure is a plain function, and its
    // data is a strong count of `shared`, given back as the window goes.
    unsafe { SetWindowSubclass(hwnd, Some(main_window_proc), MAIN_FRAME_SUBCLASS, data) };
    MainFrame { shared }
}

impl MainFrame {
    /// Runs `popup`, which shows a popup menu of the window and returns once it is closed;
    /// with `bold_first`, the menu's first item is its bold default.
    pub fn popup<R>(&self, bold_first: bool, popup: impl FnOnce() -> R) -> R {
        self.shared.bold_first.set(bold_first);
        let result = popup();
        self.shared.bold_first.set(false);
        result
    }

    /// Whether the system colours changed since the last call, which clears the mark.
    pub fn colours_changed(&self) -> bool {
        self.shared.colours_changed.replace(false)
    }
}

unsafe extern "system" fn main_window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    data: usize,
) -> LRESULT {
    // SAFETY: the data is the strong count `subclass` gave, alive until WM_NCDESTROY.
    let shared = || unsafe { &*(data as *const Shared) };
    match msg {
        WM_MENUCHAR if (wparam >> 16) as u32 & MF_SYSMENU == 0 => {
            if let Some(position) =
                menu_item_by_key_position((wparam & 0xffff) as u16, lparam as HMENU)
            {
                return ((MNC_EXECUTE as usize) << 16 | position) as LRESULT;
            }
        }
        WM_INITMENUPOPUP if shared().bold_first.replace(false) => {
            // SAFETY: wx sets the menu up first; then the first item of the menu Windows
            // passed in becomes the default, by position.
            let result = unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) };
            unsafe { SetMenuDefaultItem(wparam as HMENU, 0, 1) };
            return result;
        }
        // Noted only: the window looks at the theme once the message is handled, outside
        // it, since applying a theme sends this very message again.
        WM_SYSCOLORCHANGE => shared().colours_changed.set(true),
        WM_SETTINGCHANGE if wparam as u32 == SPI_SETHIGHCONTRAST => {
            shared().colours_changed.set(true)
        }
        WM_NCDESTROY => {
            // SAFETY: the subclass installed in `subclass`, removed as the window goes, as
            // the subclassing rules ask; then its strong count is given back, once.
            unsafe {
                RemoveWindowSubclass(hwnd, Some(main_window_proc), MAIN_FRAME_SUBCLASS);
                drop(Rc::from_raw(data as *const Shared));
            }
        }
        _ => {}
    }
    // SAFETY: passes the message on to wx's own window procedure.
    unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
}

/// The menu item whose access key sits on the key that typed `ch` in the current layout:
/// "а" in the Ukrainian layout is the F key, so it finds "Файл (&F)".
fn menu_item_by_key_position(ch: u16, menu: HMENU) -> Option<usize> {
    // SAFETY: reads the current thread's keyboard layout.
    let scan = unsafe { VkKeyScanExW(ch, GetKeyboardLayout(0)) };
    if scan == -1 {
        return None;
    }
    let vk = (scan & 0xff) as u8;
    if !vk.is_ascii_uppercase() {
        return None;
    }
    // SAFETY: the menu Windows passed with WM_MENUCHAR, which is open now.
    let count = unsafe { GetMenuItemCount(menu) };
    let items: Vec<String> = (0..count.max(0) as u32)
        .map(|i| {
            let mut text = [0u16; 256];
            // SAFETY: the same menu; the buffer length is passed.
            let len = unsafe {
                GetMenuStringW(menu, i, text.as_mut_ptr(), text.len() as i32, MF_BYPOSITION)
            };
            String::from_utf16_lossy(&text[..len.max(0) as usize])
        })
        .collect();
    find_access_key(&items, vk)
}
