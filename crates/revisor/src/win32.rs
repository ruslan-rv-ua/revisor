//! The Win32 calls wxDragon 0.9.24 does not expose (ADR-0008): the interface language,
//! accessible names and the live region of the status bar, the focused control, list row
//! positions, and a subclass of the main window that answers WM_MENUCHAR by key position
//! and makes a context menu's first item its bold default.

use std::cell::Cell;
use std::ffi::c_void;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Globalization::GetUserDefaultUILanguage;
use windows_sys::Win32::Graphics::Gdi::ClientToScreen;
use windows_sys::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows_sys::Win32::UI::Accessibility::{
    CLSID_AccPropServices, LiveSetting_Property_GUID, NotifyWinEvent, PROPID_ACC_NAME, Polite,
};
use windows_sys::Win32::UI::Controls::{
    LVIR_BOUNDS, LVM_GETITEMRECT, LVM_GETNEXTITEM, LVNI_FOCUSED,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetFocus, GetKeyState, GetKeyboardLayout, VK_MENU, VkKeyScanExW,
};
use windows_sys::Win32::UI::Shell::{
    DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass, StrCmpLogicalW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CHILDID_SELF, EVENT_OBJECT_LIVEREGIONCHANGED, GetClassNameW, GetMenuItemCount, GetMenuStringW,
    HMENU, MF_BYPOSITION, MF_SYSMENU, MNC_EXECUTE, OBJID_CLIENT, SendMessageW, SetMenuDefaultItem,
    WM_INITMENUPOPUP, WM_MENUCHAR, WM_NCDESTROY,
};
use windows_sys::core::{GUID, HRESULT};

use crate::i18n::Lang;
use crate::menus::find_access_key;

/// The interface language of Windows: Ukrainian gives Ukrainian, any other English.
pub fn ui_language() -> Lang {
    const LANG_UKRAINIAN: u16 = 0x22;
    // SAFETY: no arguments; it only reads the user's setting.
    let lang_id = unsafe { GetUserDefaultUILanguage() };
    if lang_id & 0x3ff == LANG_UKRAINIAN {
        Lang::Uk
    } else {
        Lang::En
    }
}

/// Orders two names as File Explorer does: by the user's alphabet regardless of case, with
/// numbers in names compared by their value ("Profile 2" before "Profile 10").
pub fn compare_names(a: &str, b: &str) -> std::cmp::Ordering {
    let wide = |s: &str| -> Vec<u16> { s.encode_utf16().chain([0]).collect() };
    let (a, b) = (wide(a), wide(b));
    // SAFETY: both strings are NUL-terminated.
    unsafe { StrCmpLogicalW(a.as_ptr(), b.as_ptr()) }.cmp(&0)
}

// --- IAccPropServices -------------------------------------------------------------------

/// The part of the IAccPropServices vtable revisor calls, in oleacc.idl order.
#[repr(C)]
struct AccPropServicesVtbl {
    query_interface: usize,
    add_ref: usize,
    release: usize,
    set_prop_value: usize,
    set_prop_server: usize,
    clear_props: usize,
    set_hwnd_prop: unsafe extern "system" fn(*mut c_void, HWND, u32, u32, GUID, Variant) -> HRESULT,
    set_hwnd_prop_str:
        unsafe extern "system" fn(*mut c_void, HWND, u32, u32, GUID, *const u16) -> HRESULT,
}

/// A VARIANT holding a VT_I4, laid out like the 24-byte Win32 VARIANT.
#[repr(C, align(8))]
struct Variant {
    vt: u16,
    reserved: [u16; 3],
    value: i32,
    padding: [u32; 3],
}

// The x64 VARIANT; revisor builds only for x86_64-pc-windows-msvc.
const _: () = assert!(size_of::<Variant>() == 24);

const IID_IACCPROPSERVICES: GUID = GUID::from_u128(0x6e26e776_04f0_495d_80e4_3330352e3169);
const VT_I4: u16 = 3;

/// The GUI thread's IAccPropServices, created on first use there, where wx has already
/// initialised COM. It is never released: the overrides it holds must last as long as the
/// windows they name.
struct AccProps(*mut c_void);

thread_local! {
    static PROPS: &'static Option<AccProps> = Box::leak(Box::new(create_acc_props()));
}

fn acc_props() -> Option<&'static AccProps> {
    PROPS.with(|props| props.as_ref())
}

fn create_acc_props() -> Option<AccProps> {
    let mut ptr = std::ptr::null_mut();
    // SAFETY: valid CLSID/IID and out pointer.
    let hr = unsafe {
        CoCreateInstance(
            &CLSID_AccPropServices,
            std::ptr::null_mut(),
            CLSCTX_INPROC_SERVER,
            &IID_IACCPROPSERVICES,
            &mut ptr,
        )
    };
    (hr >= 0 && !ptr.is_null()).then_some(AccProps(ptr))
}

impl AccProps {
    fn vtbl(&self) -> &AccPropServicesVtbl {
        // SAFETY: a COM object starts with a pointer to its vtable.
        unsafe { &**(self.0 as *const *const AccPropServicesVtbl) }
    }

    fn set_name(&self, hwnd: HWND, name: &str) {
        let wide: Vec<u16> = name.encode_utf16().chain([0]).collect();
        // SAFETY: a live window, a NUL-terminated string the call copies.
        unsafe {
            (self.vtbl().set_hwnd_prop_str)(
                self.0,
                hwnd,
                OBJID_CLIENT as u32,
                CHILDID_SELF,
                PROPID_ACC_NAME,
                wide.as_ptr(),
            );
        }
    }

    fn set_live_polite(&self, hwnd: HWND) {
        let polite = Variant {
            vt: VT_I4,
            reserved: [0; 3],
            value: Polite,
            padding: [0; 3],
        };
        // SAFETY: a live window and a VT_I4 VARIANT.
        unsafe {
            (self.vtbl().set_hwnd_prop)(
                self.0,
                hwnd,
                OBJID_CLIENT as u32,
                CHILDID_SELF,
                LiveSetting_Property_GUID,
                polite,
            );
        }
    }
}

/// Gives a window the name screen readers announce for it, without a visible label.
pub fn set_accessible_name(hwnd: *mut c_void, name: &str) {
    if let Some(props) = acc_props() {
        props.set_name(hwnd, name);
    }
}

/// Marks a window as a polite live region (UIA LiveSetting), for its `announce` calls.
pub fn make_live_region(hwnd: *mut c_void) {
    if let Some(props) = acc_props() {
        props.set_live_polite(hwnd);
    }
}

/// Tells screen readers that a status bar's first field has changed. NVDA reads the name
/// of the object the event names, and the field's name is its text; the status bar's own
/// name stays empty, or NVDA+End would read the message twice, as the bar's name and as
/// the field's. The event is raised even when the text is the same as before, so a
/// repeated message is read again.
pub fn announce_status(status_bar: *mut c_void) {
    const FIRST_FIELD: i32 = 1;
    // SAFETY: a live window; the call only queues an accessibility event.
    unsafe {
        NotifyWinEvent(
            EVENT_OBJECT_LIVEREGIONCHANGED,
            status_bar,
            OBJID_CLIENT,
            FIRST_FIELD,
        )
    };
}

// --- Focus and list rows ----------------------------------------------------------------

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

/// Where a context menu opened from the keyboard goes: the screen point at the bottom
/// left of the list's focused row, or the list's top left corner when no row has focus.
pub fn context_menu_point(list: *mut c_void) -> (i32, i32) {
    // SAFETY: plain list-view messages to a live list-view window.
    let focused =
        unsafe { SendMessageW(list, LVM_GETNEXTITEM, usize::MAX, LVNI_FOCUSED as LPARAM) };
    let mut point = POINT { x: 0, y: 0 };
    if focused >= 0 {
        let mut rect = RECT {
            left: LVIR_BOUNDS as i32,
            top: 0,
            right: 0,
            bottom: 0,
        };
        // SAFETY: LVM_GETITEMRECT fills the RECT whose `left` names the part wanted.
        if unsafe {
            SendMessageW(
                list,
                LVM_GETITEMRECT,
                focused as WPARAM,
                &mut rect as *mut RECT as LPARAM,
            )
        } != 0
        {
            point = POINT {
                x: rect.left,
                y: rect.bottom,
            };
        }
    }
    // SAFETY: a live window and a valid POINT.
    unsafe { ClientToScreen(list, &mut point) };
    (point.x, point.y)
}

// --- The main window subclass -----------------------------------------------------------

thread_local! {
    /// Set right before a context menu opens whose first item is the list's Enter action.
    static BOLD_FIRST_ITEM: Cell<bool> = const { Cell::new(false) };
}

/// The next popup menu the main window opens gets its first item as the bold default.
pub fn bold_first_item_of_next_popup(bold: bool) {
    BOLD_FIRST_ITEM.with(|b| b.set(bold));
}

/// Subclasses the main window: WM_MENUCHAR finds access keys by key position, and
/// WM_INITMENUPOPUP bolds the default item of a context menu.
pub fn subclass_main_window(hwnd: *mut c_void) {
    // SAFETY: a live window of this thread; the procedure is a plain function.
    unsafe { SetWindowSubclass(hwnd, Some(main_window_proc), 1, 0) };
}

unsafe extern "system" fn main_window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    match msg {
        WM_MENUCHAR if (wparam >> 16) as u32 & MF_SYSMENU == 0 => {
            if let Some(position) =
                menu_item_by_key_position((wparam & 0xffff) as u16, lparam as HMENU)
            {
                return ((MNC_EXECUTE as usize) << 16 | position) as LRESULT;
            }
        }
        WM_INITMENUPOPUP if BOLD_FIRST_ITEM.with(|b| b.replace(false)) => {
            // SAFETY: wx sets the menu up first; then the first item of the menu Windows
            // passed in becomes the default, by position.
            let result = unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) };
            unsafe { SetMenuDefaultItem(wparam as HMENU, 0, 1) };
            return result;
        }
        WM_NCDESTROY => {
            // SAFETY: the subclass installed in `subclass_main_window`, removed as the
            // window goes, as the subclassing rules ask.
            unsafe { RemoveWindowSubclass(hwnd, Some(main_window_proc), 1) };
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    #[test]
    fn names_are_ordered_as_file_explorer_orders_them() {
        assert_eq!(compare_names("профіль 2", "Профіль 10"), Ordering::Less);
        assert_eq!(compare_names("Дім", "дім"), Ordering::Equal);
        assert_eq!(compare_names("Home", "home 1"), Ordering::Less);
    }
}
