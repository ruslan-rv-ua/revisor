//! The Win32 calls wxDragon 0.9.24 does not expose (ADR-0008): the interface language,
//! accessible names and the live region of the status bar, the focused control, list row
//! positions, a subclass of the main window that answers WM_MENUCHAR by key position and
//! makes a context menu's first item its bold default, counts, dates and sizes in the
//! regional format, the shield icon of a menu item, the restart as administrator (TASK-8),
//! and the snapshot list's rows, styles and empty text (TASK-9).

use std::cell::{Cell, RefCell};
use std::ffi::{OsStr, c_void};
use std::io;
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr;

use windows_sys::Win32::Foundation::{
    FILETIME, HWND, LPARAM, LRESULT, POINT, RECT, SYSTEMTIME, WPARAM,
};
use windows_sys::Win32::Globalization::{
    DATE_SHORTDATE, GetDateFormatEx, GetLocaleInfoEx, GetNumberFormatEx, GetTimeFormatEx,
    GetUserDefaultUILanguage, LOCALE_SDECIMAL, LOCALE_SGROUPING, LOCALE_STHOUSAND, NUMBERFMTW,
};
use windows_sys::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, ClientToScreen, CreateCompatibleDC, CreateDIBSection,
    DIB_RGB_COLORS, DeleteDC, DeleteObject, HBITMAP, InvalidateRect, RGBQUAD, SelectObject,
};
use windows_sys::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows_sys::Win32::System::Time::{FileTimeToSystemTime, SystemTimeToTzSpecificLocalTime};
use windows_sys::Win32::UI::Accessibility::{
    CLSID_AccPropServices, LiveSetting_Property_GUID, NotifyWinEvent, PROPID_ACC_NAME, Polite,
};
use windows_sys::Win32::UI::Controls::{
    EMF_CENTERED, LVIR_BOUNDS, LVIS_FOCUSED, LVIS_SELECTED, LVITEMW, LVM_ENSUREVISIBLE,
    LVM_GETITEMRECT, LVM_GETNEXTITEM, LVM_GETSELECTEDCOUNT, LVM_SETEXTENDEDLISTVIEWSTYLE,
    LVM_SETITEMSTATE, LVN_GETEMPTYMARKUP, LVNI_FOCUSED, LVNI_SELECTED, LVS_EX_FULLROWSELECT,
    LVS_NOSORTHEADER, NMHDR, NMLVEMPTYMARKUP,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetFocus, GetKeyState, GetKeyboardLayout, SetFocus, VK_MENU, VkKeyScanExW,
};
use windows_sys::Win32::UI::Shell::{
    DefSubclassProc, RemoveWindowSubclass, SHGSI_ICON, SHGSI_SMALLICON, SHGetStockIconInfo,
    SHSTOCKICONINFO, SIID_SHIELD, SetWindowSubclass, ShellExecuteW, StrCmpLogicalW,
    StrFormatByteSizeW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CHILDID_SELF, DI_NORMAL, DestroyIcon, DrawIconEx, EVENT_OBJECT_LIVEREGIONCHANGED, GWL_STYLE,
    GetClassNameW, GetMenu, GetMenuItemCount, GetMenuStringW, GetSubMenu, GetSystemMetrics,
    GetWindowLongPtrW, HICON, HMENU, MENUITEMINFOW, MF_BYPOSITION, MF_SYSMENU, MIIM_BITMAP,
    MNC_EXECUTE, OBJID_CLIENT, SM_CXSMICON, SM_CYSMICON, SW_SHOWNORMAL, SendMessageW,
    SetMenuDefaultItem, SetMenuItemInfoW, SetWindowLongPtrW, WM_INITMENUPOPUP, WM_MENUCHAR,
    WM_NCDESTROY, WM_NOTIFY,
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

/// Makes a list that has the keyboard focus announce itself again to screen readers, as
/// if the focus had just come to it from elsewhere: the focus leaves the list for a
/// moment and comes back, which raises the focus event NVDA reads the list's name and
/// its focused row on (TASK-11 #21). Replacing the rows under the focus raises none.
pub fn refocus(list: *mut c_void) {
    // SAFETY: plain focus calls on windows of this thread.
    unsafe {
        SetFocus(ptr::null_mut());
        SetFocus(list);
    }
}

/// Whether a window is a text field (an EDIT control), which needs Delete for itself.
pub fn is_text_field(hwnd: *mut c_void) -> bool {
    let mut class = [0u16; 32];
    // SAFETY: the buffer length is passed.
    let len = unsafe { GetClassNameW(hwnd, class.as_mut_ptr(), class.len() as i32) };
    len > 0 && String::from_utf16_lossy(&class[..len as usize]).eq_ignore_ascii_case("Edit")
}

/// The row of a list that has the focus rectangle: the current snapshot (TASK-9).
pub fn focused_row(list: *mut c_void) -> Option<usize> {
    // SAFETY: a plain list-view message to a live list-view window.
    let row = unsafe { SendMessageW(list, LVM_GETNEXTITEM, usize::MAX, LVNI_FOCUSED as LPARAM) };
    usize::try_from(row).ok()
}

/// The selected rows of a list, top to bottom.
pub fn selected_rows(list: *mut c_void) -> Vec<usize> {
    let mut rows = Vec::new();
    let mut row = -1isize;
    loop {
        // SAFETY: a plain list-view message to a live list-view window.
        row = unsafe {
            SendMessageW(
                list,
                LVM_GETNEXTITEM,
                row as WPARAM,
                LVNI_SELECTED as LPARAM,
            )
        };
        match usize::try_from(row) {
            Ok(row) => rows.push(row),
            Err(_) => return rows,
        }
    }
}

/// How many rows of a list are selected.
pub fn selected_count(list: *mut c_void) -> usize {
    // SAFETY: a plain list-view message to a live list-view window.
    unsafe { SendMessageW(list, LVM_GETSELECTEDCOUNT, 0, 0) }
        .try_into()
        .unwrap_or(0)
}

/// Makes `row` the one selected row of a list, with the focus rectangle, and scrolls it
/// into view; it does not move the keyboard focus to the list.
pub fn select_only(list: *mut c_void, row: usize) {
    set_row_state(list, -1, 0, LVIS_SELECTED);
    set_row_state(
        list,
        row as isize,
        LVIS_SELECTED | LVIS_FOCUSED,
        LVIS_SELECTED | LVIS_FOCUSED,
    );
    // SAFETY: a plain list-view message to a live list-view window.
    unsafe { SendMessageW(list, LVM_ENSUREVISIBLE, row, 0) };
}

/// LVM_SETITEMSTATE on `row`, or on every row with -1.
fn set_row_state(list: *mut c_void, row: isize, state: u32, mask: u32) {
    // SAFETY: an all-zero LVITEMW is valid; only the state fields are read for this message.
    let mut item: LVITEMW = unsafe { std::mem::zeroed() };
    item.state = state;
    item.stateMask = mask;
    // SAFETY: a live list-view window and a complete LVITEMW.
    unsafe {
        SendMessageW(
            list,
            LVM_SETITEMSTATE,
            row as WPARAM,
            &mut item as *mut LVITEMW as LPARAM,
        )
    };
}

/// Selects every row of a list, as Ctrl+A does in Explorer; the focus rectangle stays.
pub fn select_all(list: *mut c_void) {
    set_row_state(list, -1, LVIS_SELECTED, LVIS_SELECTED);
}

/// Takes the focus rectangle off every row, so that deleting rows moves it nowhere: the
/// list would otherwise hand it to a neighbour at each deletion, and a screen reader
/// would announce rows about to go.
pub fn unfocus_rows(list: *mut c_void) {
    set_row_state(list, -1, 0, LVIS_FOCUSED);
}

/// Puts the focus rectangle back on `row`, leaving the selection as it is.
pub fn focus_row(list: *mut c_void, row: usize) {
    set_row_state(list, row as isize, LVIS_FOCUSED, LVIS_FOCUSED);
}

/// Makes a report list select whole rows, not just the first column, and keeps its column
/// headers from looking like sort buttons (TASK-9).
pub fn full_row_select_no_sort(list: *mut c_void) {
    // SAFETY: plain messages and style calls on a live list-view window.
    unsafe {
        SendMessageW(
            list,
            LVM_SETEXTENDEDLISTVIEWSTYLE,
            LVS_EX_FULLROWSELECT as WPARAM,
            LVS_EX_FULLROWSELECT as LPARAM,
        );
        let style = GetWindowLongPtrW(list, GWL_STYLE);
        SetWindowLongPtrW(list, GWL_STYLE, style | LVS_NOSORTHEADER as isize);
    }
}

thread_local! {
    /// What each list shows while it has no rows, by the list's window: the text the
    /// list's parent answers LVN_GETEMPTYMARKUP with.
    static EMPTY_TEXTS: RefCell<Vec<(usize, Vec<u16>)>> = const { RefCell::new(Vec::new()) };
}

/// Sets the text a list shows in its area while it has no rows, as Windows draws it
/// (LVN_GETEMPTYMARKUP); the list's parent must have been given to
/// [`subclass_list_parent`]. The text is cut to what the message can carry.
pub fn set_empty_text(list: *mut c_void, text: &str) {
    const MAX_CHARS: usize = 2083;
    let wide: Vec<u16> = text.encode_utf16().take(MAX_CHARS).chain([0]).collect();
    EMPTY_TEXTS.with(|texts| {
        let mut texts = texts.borrow_mut();
        match texts.iter_mut().find(|(hwnd, _)| *hwnd == list as usize) {
            Some((_, old)) => *old = wide,
            None => texts.push((list as usize, wide)),
        }
    });
    // SAFETY: a live window; the whole client area is redrawn.
    unsafe { InvalidateRect(list, ptr::null(), 1) };
}

/// Subclasses the parent of the lists so it answers LVN_GETEMPTYMARKUP for them.
pub fn subclass_list_parent(hwnd: *mut c_void) {
    // SAFETY: a live window of this thread; the procedure is a plain function.
    unsafe { SetWindowSubclass(hwnd, Some(list_parent_proc), 1, 0) };
}

unsafe extern "system" fn list_parent_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    match msg {
        WM_NOTIFY => {
            // SAFETY: WM_NOTIFY's lparam points at an NMHDR, which for this code is the
            // start of an NMLVEMPTYMARKUP the list owns for the call.
            let header = unsafe { &*(lparam as *const NMHDR) };
            if header.code == LVN_GETEMPTYMARKUP {
                let text = EMPTY_TEXTS.with(|texts| {
                    texts
                        .borrow()
                        .iter()
                        .find(|(list, _)| *list == header.hwndFrom as usize)
                        .map(|(_, text)| text.clone())
                });
                if let Some(text) = text {
                    // SAFETY: see above; the markup struct is written within its bounds.
                    let markup = unsafe { &mut *(lparam as *mut NMLVEMPTYMARKUP) };
                    markup.dwFlags = EMF_CENTERED;
                    markup.szMarkup[..text.len()].copy_from_slice(&text);
                    return 1;
                }
            }
        }
        WM_NCDESTROY => {
            // SAFETY: the subclass installed in `subclass_list_parent`, removed as the
            // window goes, as the subclassing rules ask.
            unsafe { RemoveWindowSubclass(hwnd, Some(list_parent_proc), 1) };
        }
        _ => {}
    }
    // SAFETY: passes the message on to wx's own window procedure.
    unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
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

// --- Counts, the shield and the restart ------------------------------------------------

/// A count in the user's regional format, with its digit grouping: "12,345" or "12 345".
/// Plain digits when the format cannot be read.
pub fn format_count(count: u64) -> String {
    let plain = count.to_string();
    let locale_info = |kind: u32| -> Option<Vec<u16>> {
        let mut buf = [0u16; 32];
        // SAFETY: the user's default locale (a null name); the buffer length is passed.
        let len = unsafe { GetLocaleInfoEx(ptr::null(), kind, buf.as_mut_ptr(), buf.len() as i32) };
        // The length counts the terminating NUL, which the format needs too.
        (len > 0).then(|| buf[..len as usize].to_vec())
    };
    let (Some(mut thousand), Some(mut decimal), Some(grouping)) = (
        locale_info(LOCALE_STHOUSAND),
        locale_info(LOCALE_SDECIMAL),
        locale_info(LOCALE_SGROUPING),
    ) else {
        return plain;
    };
    let format = NUMBERFMTW {
        NumDigits: 0,
        LeadingZero: 1,
        Grouping: grouping_field(&String::from_utf16_lossy(&grouping)),
        lpDecimalSep: decimal.as_mut_ptr(),
        lpThousandSep: thousand.as_mut_ptr(),
        NegativeOrder: 1,
    };
    let value: Vec<u16> = plain.encode_utf16().chain([0]).collect();
    let mut out = [0u16; 64];
    // SAFETY: every string is NUL-terminated and the output length is passed.
    let len = unsafe {
        GetNumberFormatEx(
            ptr::null(),
            0,
            value.as_ptr(),
            &format,
            out.as_mut_ptr(),
            out.len() as i32,
        )
    };
    if len <= 1 {
        return plain;
    }
    String::from_utf16_lossy(&out[..len as usize - 1])
}

/// A moment given as Unix time in milliseconds, in the user's regional short date and
/// time with seconds, in this computer's time zone: "31.10.2026 14:05:12" or
/// "10/31/2026 2:05:12 PM" (TASK-9). A moment Windows cannot convert, or a format it
/// cannot give, falls back to the UTC time in ISO form, so a name is never empty.
pub fn format_date_time(unix_millis: i64) -> String {
    /// 100-ns intervals between 1601-01-01 and 1970-01-01.
    const EPOCH_DIFFERENCE: i64 = 116_444_736_000_000_000;
    let fallback = || iso_utc(unix_millis);
    let Some(intervals) = unix_millis
        .checked_mul(10_000)
        .and_then(|i| i.checked_add(EPOCH_DIFFERENCE))
        .filter(|&i| i >= 0)
    else {
        return fallback();
    };
    let file_time = FILETIME {
        dwLowDateTime: intervals as u32,
        dwHighDateTime: (intervals >> 32) as u32,
    };
    // SAFETY: all-zero SYSTEMTIMEs are valid out values; the calls fill them.
    let mut utc: SYSTEMTIME = unsafe { std::mem::zeroed() };
    let mut local: SYSTEMTIME = unsafe { std::mem::zeroed() };
    // SAFETY: valid pointers; a null time zone means this computer's current one.
    let converted = unsafe {
        FileTimeToSystemTime(&file_time, &mut utc) != 0
            && SystemTimeToTzSpecificLocalTime(ptr::null(), &utc, &mut local) != 0
    };
    if !converted {
        return fallback();
    }
    let mut date = [0u16; 128];
    let mut time = [0u16; 128];
    // SAFETY: the user's default locale (a null name); the buffer lengths are passed and
    // the format and calendar are the locale's own.
    let (date_len, time_len) = unsafe {
        (
            GetDateFormatEx(
                ptr::null(),
                DATE_SHORTDATE,
                &local,
                ptr::null(),
                date.as_mut_ptr(),
                date.len() as i32,
                ptr::null(),
            ),
            GetTimeFormatEx(
                ptr::null(),
                0,
                &local,
                ptr::null(),
                time.as_mut_ptr(),
                time.len() as i32,
            ),
        )
    };
    // The lengths count the terminating NUL.
    if date_len <= 1 || time_len <= 1 {
        return fallback();
    }
    format!(
        "{} {}",
        String::from_utf16_lossy(&date[..date_len as usize - 1]),
        String::from_utf16_lossy(&time[..time_len as usize - 1])
    )
}

/// "2026-10-31 12:05:12 UTC" for a Unix time in milliseconds, with no calendar library:
/// the civil-from-days algorithm of Howard Hinnant.
fn iso_utc(unix_millis: i64) -> String {
    let seconds = unix_millis.div_euclid(1000);
    let days = seconds.div_euclid(86_400);
    let of_day = seconds.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02} UTC",
        of_day / 3600,
        of_day % 3600 / 60,
        of_day % 60
    )
}

/// A size in bytes as Explorer shows it: "1.5 MB" in the user's format; plain bytes when
/// the format cannot be given.
pub fn format_size(bytes: u64) -> String {
    let mut out = [0u16; 64];
    // SAFETY: the buffer length is passed.
    let written = unsafe {
        StrFormatByteSizeW(
            bytes.min(i64::MAX as u64) as i64,
            out.as_mut_ptr(),
            out.len() as u32,
        )
    };
    if written.is_null() {
        return bytes.to_string();
    }
    let len = out.iter().position(|&c| c == 0).unwrap_or(out.len());
    String::from_utf16_lossy(&out[..len])
}

/// The `Grouping` field of NUMBERFMTW from LOCALE_SGROUPING: "3;0" (groups of three,
/// repeating) is 3, "3;2;0" (Indic) is 32, and a list without the final 0 groups once,
/// so "3" is 30.
fn grouping_field(sgrouping: &str) -> u32 {
    let parts: Vec<&str> = sgrouping.trim_end_matches('\0').split(';').collect();
    let repeating = parts.last() == Some(&"0");
    let digits = parts
        .iter()
        .take(if repeating {
            parts.len() - 1
        } else {
            parts.len()
        })
        .filter_map(|p| p.parse::<u32>().ok())
        .fold(0, |acc, g| acc * 10 + g);
    if repeating { digits } else { digits * 10 }
}

/// Puts the system's shield icon, which marks actions that need administrator rights, on
/// the item with command id `command_id` of the frame's menu at `menu_position`. The icon
/// is left out when it cannot be drawn; the command works all the same.
pub fn mark_with_shield(frame: *mut c_void, menu_position: i32, command_id: i32) {
    let mut info = SHSTOCKICONINFO {
        cbSize: size_of::<SHSTOCKICONINFO>() as u32,
        hIcon: ptr::null_mut(),
        iSysImageIndex: 0,
        iIcon: 0,
        szPath: [0; 260],
    };
    // SAFETY: `info` says its own size, as the call requires.
    let hr = unsafe { SHGetStockIconInfo(SIID_SHIELD, SHGSI_ICON | SHGSI_SMALLICON, &mut info) };
    if hr < 0 || info.hIcon.is_null() {
        return;
    }
    let bitmap = icon_to_bitmap(info.hIcon);
    // SAFETY: the icon came from SHGetStockIconInfo and is no longer needed.
    unsafe { DestroyIcon(info.hIcon) };
    let Some(bitmap) = bitmap else {
        return;
    };
    // SAFETY: a live frame window; the menu handles come from the system.
    let menu = unsafe { GetSubMenu(GetMenu(frame), menu_position) };
    if menu.is_null() {
        // SAFETY: the bitmap was made by `icon_to_bitmap` and nothing shows it.
        unsafe { DeleteObject(bitmap) };
        return;
    }
    // SAFETY: an all-zero MENUITEMINFOW is valid; the fields set are the ones the mask names.
    let mut item: MENUITEMINFOW = unsafe { std::mem::zeroed() };
    item.cbSize = size_of::<MENUITEMINFOW>() as u32;
    item.fMask = MIIM_BITMAP;
    item.hbmpItem = bitmap;
    // The bitmap stays alive for as long as the menu shows it, which is the window's life.
    // SAFETY: the menu is live and `item` is a complete MENUITEMINFOW.
    unsafe { SetMenuItemInfoW(menu, command_id as u32, 0, &item) };
}

/// Draws an icon into a 32-bit bitmap with alpha, the kind a themed menu draws right.
fn icon_to_bitmap(icon: HICON) -> Option<HBITMAP> {
    // SAFETY: plain FFI calls.
    let (width, height) = unsafe { (GetSystemMetrics(SM_CXSMICON), GetSystemMetrics(SM_CYSMICON)) };
    // SAFETY: an all-zero BITMAPINFO is valid; the header fields are filled below.
    let mut info: BITMAPINFO = unsafe { std::mem::zeroed() };
    info.bmiHeader = BITMAPINFOHEADER {
        biSize: size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: width,
        // Negative: top-down rows, the layout the menu expects.
        biHeight: -height,
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB,
        biSizeImage: 0,
        biXPelsPerMeter: 0,
        biYPelsPerMeter: 0,
        biClrUsed: 0,
        biClrImportant: 0,
    };
    info.bmiColors = [RGBQUAD {
        rgbBlue: 0,
        rgbGreen: 0,
        rgbRed: 0,
        rgbReserved: 0,
    }];
    let mut bits: *mut c_void = ptr::null_mut();
    // SAFETY: a memory DC of the screen; `info` describes the bitmap; the DC and the old
    // bitmap are restored and the DC deleted before returning.
    unsafe {
        let dc = CreateCompatibleDC(ptr::null_mut());
        if dc.is_null() {
            return None;
        }
        let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, ptr::null_mut(), 0);
        if bitmap.is_null() {
            DeleteDC(dc);
            return None;
        }
        let old = SelectObject(dc, bitmap);
        let drawn = DrawIconEx(dc, 0, 0, icon, width, height, 0, ptr::null_mut(), DI_NORMAL);
        SelectObject(dc, old);
        DeleteDC(dc);
        if drawn == 0 {
            DeleteObject(bitmap);
            return None;
        }
        Some(bitmap)
    }
}

/// Starts `exe` with `args` as administrator, through the UAC prompt. An error means it
/// did not start; declining the prompt is `ERROR_CANCELLED`.
pub fn run_as_admin(exe: &Path, args: &str) -> io::Result<()> {
    let wide = |s: &OsStr| -> Vec<u16> { s.encode_wide().chain([0]).collect() };
    let verb = wide(OsStr::new("runas"));
    let exe = wide(exe.as_os_str());
    let args = wide(OsStr::new(args));
    // SAFETY: every string is NUL-terminated; no window owns the prompt.
    let result = unsafe {
        ShellExecuteW(
            ptr::null_mut(),
            verb.as_ptr(),
            exe.as_ptr(),
            args.as_ptr(),
            ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    // A value above 32 means success; the rest are error codes, with the detail in the
    // last error.
    if result as usize > 32 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
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

    #[test]
    fn counts_keep_their_digits_in_the_regional_format() {
        // The separator depends on the user's region; the digits do not.
        let digits =
            |text: String| -> String { text.chars().filter(char::is_ascii_digit).collect() };
        assert_eq!(format_count(0), "0");
        assert_eq!(format_count(7), "7");
        assert_eq!(digits(format_count(1_234_567)), "1234567");
        assert!(
            format_count(1_234_567).len() > 7,
            "the thousands are grouped"
        );
        assert!(
            !format_count(1_234_567).contains(['.', ',']) || !format_count(1).contains(['.', ','])
        );
    }

    #[test]
    fn a_snapshot_name_holds_the_regional_date_and_the_time_with_seconds() {
        // 2026-10-31 12:05:12 UTC; the local hour depends on this computer's time zone,
        // but the minutes, the seconds and the day's digits do not.
        let name = format_date_time(1_793_448_312_000);
        // The minutes shift in a half-hour time zone and the separator is the region's,
        // so only the seconds are looked for.
        assert!(name.contains("12"), "{name}");
        assert!(name.contains("2026") || name.contains("26"), "{name}");
        assert!(name.contains(' '), "{name}");
        let later = format_date_time(1_793_448_313_000);
        assert_ne!(name, later, "seconds tell two snapshots of a minute apart");
        assert_eq!(iso_utc(1_793_448_312_000), "2026-10-31 12:05:12 UTC");
        assert_eq!(iso_utc(0), "1970-01-01 00:00:00 UTC");
        assert_eq!(iso_utc(-1000), "1969-12-31 23:59:59 UTC");
        // A moment before 1601 cannot be a FILETIME; the ISO form stands in.
        assert_eq!(
            format_date_time(-20_000_000_000_000),
            "1336-03-23 12:26:40 UTC"
        );
    }

    #[test]
    fn sizes_are_shown_as_explorer_shows_them() {
        // The unit and the decimal separator follow the user's language and region:
        // "1.43 MB" or "1,43 МБ".
        let size = format_size(1_500_000);
        assert!(size.starts_with("1"), "{size}");
        assert!(size.contains(' '), "{size}");
        assert_ne!(size, "1500000");
        assert!(format_size(0).starts_with('0'));
    }

    #[test]
    fn grouping_follows_the_locale_string() {
        assert_eq!(grouping_field("3;0"), 3);
        assert_eq!(grouping_field("3;2;0"), 32);
        assert_eq!(grouping_field("3"), 30);
        assert_eq!(grouping_field("0"), 0);
    }
}
