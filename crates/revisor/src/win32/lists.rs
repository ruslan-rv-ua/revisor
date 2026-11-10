//! Report lists (ListView) beyond what wx gives: the focused and selected rows, the
//! selection, full-row selection, the sort arrow of the column headers (TASK-13), the
//! point a keyboard context menu opens at, and the text a list shows while it has no rows
//! (TASK-9). The snapshot lists and the difference lists both use them (TASK-27, TASK-32).

use std::cell::RefCell;
use std::ffi::c_void;
use std::ptr;
use std::rc::Rc;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{ClientToScreen, InvalidateRect};
use windows_sys::Win32::UI::Controls::{
    EMF_CENTERED, HDF_SORTDOWN, HDF_SORTUP, HDI_FORMAT, HDITEMW, HDM_GETITEMCOUNT, HDM_GETITEMW,
    HDM_SETITEMW, LVIR_BOUNDS, LVIS_FOCUSED, LVIS_SELECTED, LVIS_STATEIMAGEMASK, LVITEMW,
    LVM_ENSUREVISIBLE, LVM_GETHEADER, LVM_GETITEMRECT, LVM_GETITEMSTATE, LVM_GETNEXTITEM,
    LVM_GETSELECTEDCOUNT, LVM_SETCOLUMNWIDTH, LVM_SETEXTENDEDLISTVIEWSTYLE, LVM_SETITEMSTATE,
    LVN_GETEMPTYMARKUP, LVNI_FOCUSED, LVNI_SELECTED, LVS_EX_CHECKBOXES, LVS_EX_FULLROWSELECT,
    LVS_NOSORTHEADER, NMHDR, NMLVEMPTYMARKUP,
};
use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GWL_STYLE, GetClientRect, GetWindowLongPtrW, SendMessageW, SetWindowLongPtrW, WM_NCDESTROY,
    WM_NOTIFY, WM_SIZE,
};

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

/// Whether the row `row` of a list is selected.
pub fn is_selected(list: *mut c_void, row: usize) -> bool {
    // SAFETY: a plain list-view message to a live list-view window.
    let state = unsafe { SendMessageW(list, LVM_GETITEMSTATE, row, LVIS_SELECTED as LPARAM) };
    state as u32 & LVIS_SELECTED != 0
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
    full_row_select(list);
    // SAFETY: style calls on a live list-view window.
    unsafe {
        let style = GetWindowLongPtrW(list, GWL_STYLE);
        SetWindowLongPtrW(list, GWL_STYLE, style | LVS_NOSORTHEADER as isize);
    }
}

/// Makes a report list select whole rows, not just the first column; its column headers
/// stay buttons that sort it (TASK-13).
pub fn full_row_select(list: *mut c_void) {
    // SAFETY: a plain message to a live list-view window.
    unsafe {
        SendMessageW(
            list,
            LVM_SETEXTENDEDLISTVIEWSTYLE,
            LVS_EX_FULLROWSELECT as WPARAM,
            LVS_EX_FULLROWSELECT as LPARAM,
        );
    }
}

/// Gives every row of a report list a check box, as the list of typical folders has
/// (TASK-36); Space toggles the check box of the focused row.
pub fn enable_check_boxes(list: *mut c_void) {
    let style = LVS_EX_CHECKBOXES | LVS_EX_FULLROWSELECT;
    // SAFETY: a plain message to a live list-view window.
    unsafe {
        SendMessageW(
            list,
            LVM_SETEXTENDEDLISTVIEWSTYLE,
            style as WPARAM,
            style as LPARAM,
        )
    };
}

/// The id of the fill-width subclass among a list's subclasses.
const FILL_WIDTH_SUBCLASS: usize = 2;

/// Keeps the one column of a report list as wide as the list, or `min_width` when that is
/// wider, now and whenever the list is resized: a selected row is then highlighted across
/// the list, and no row is cut off (TASK-36).
pub fn fill_width(list: *mut c_void, min_width: i32) {
    let data = usize::try_from(min_width).unwrap_or(0);
    // SAFETY: a live window of this thread; the procedure is a plain function, and its data
    // is a plain number.
    unsafe { SetWindowSubclass(list, Some(fill_width_proc), FILL_WIDTH_SUBCLASS, data) };
    set_fill_width(list, data);
}

fn set_fill_width(list: HWND, min_width: usize) {
    let mut client = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    // SAFETY: a live window and a valid RECT.
    unsafe { GetClientRect(list, &mut client) };
    let width = usize::try_from(client.right).unwrap_or(0).max(min_width);
    // SAFETY: a plain list-view message to a live list-view window.
    unsafe { SendMessageW(list, LVM_SETCOLUMNWIDTH, 0, width as LPARAM) };
}

unsafe extern "system" fn fill_width_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    data: usize,
) -> LRESULT {
    if msg == WM_NCDESTROY {
        // SAFETY: the subclass installed in `fill_width`, removed as the window goes, as
        // the subclassing rules ask.
        unsafe { RemoveWindowSubclass(hwnd, Some(fill_width_proc), FILL_WIDTH_SUBCLASS) };
    }
    // SAFETY: passes the message on to wx's own window procedure.
    let result = unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) };
    if msg == WM_SIZE {
        set_fill_width(hwnd, data);
    }
    result
}

/// The state image of a checked check box; 1 is an unchecked one.
const CHECKED: u32 = 2;

/// Checks or unchecks the check box of `row` of a list with check boxes.
pub fn set_checked(list: *mut c_void, row: usize, checked: bool) {
    let image = if checked { CHECKED } else { 1 };
    set_row_state(list, row as isize, image << 12, LVIS_STATEIMAGEMASK);
}

/// Whether the check box of `row` of a list with check boxes is checked.
pub fn is_checked(list: *mut c_void, row: usize) -> bool {
    // SAFETY: a plain list-view message to a live list-view window.
    let state = unsafe { SendMessageW(list, LVM_GETITEMSTATE, row, LVIS_STATEIMAGEMASK as LPARAM) };
    (state as u32 & LVIS_STATEIMAGEMASK) >> 12 == CHECKED
}

/// Shows the sort arrow of a list's column headers on `column`, up for ascending and down
/// for descending (TASK-13 #21), and on no column with `None`.
pub fn set_sort_arrow(list: *mut c_void, column: Option<(usize, bool)>) {
    // SAFETY: plain list-view and header messages to live windows; the HDITEMW asks for
    // the format only, which the header fills in and takes back.
    unsafe {
        let header = SendMessageW(list, LVM_GETHEADER, 0, 0) as *mut c_void;
        if header.is_null() {
            return;
        }
        let count = SendMessageW(header, HDM_GETITEMCOUNT, 0, 0);
        for i in 0..count.max(0) as usize {
            let mut item = HDITEMW {
                mask: HDI_FORMAT,
                ..Default::default()
            };
            if SendMessageW(header, HDM_GETITEMW, i, &mut item as *mut HDITEMW as LPARAM) == 0 {
                continue;
            }
            let arrow = match column {
                Some((sorted, false)) if sorted == i => HDF_SORTUP,
                Some((sorted, true)) if sorted == i => HDF_SORTDOWN,
                _ => 0,
            };
            item.fmt = (item.fmt & !(HDF_SORTUP | HDF_SORTDOWN)) | arrow;
            SendMessageW(header, HDM_SETITEMW, i, &mut item as *mut HDITEMW as LPARAM);
        }
    }
}

/// Where a context menu opened from the keyboard goes: the screen point at the bottom
/// left of the list's focused row, or the list's top left corner when no row has focus.
pub fn context_menu_point(list: *mut c_void) -> (i32, i32) {
    let mut point = POINT { x: 0, y: 0 };
    if let Some(focused) = focused_row(list) {
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

/// What each list shows while it has no rows, by the list's window.
type Texts = RefCell<Vec<(usize, Vec<u16>)>>;

/// The empty texts of the lists of one parent window: Windows asks a list's parent for
/// the text (LVN_GETEMPTYMARKUP), so the parent is subclassed to answer. Only
/// [`EmptyTexts::subclass`] makes one, so no text can be set before the parent answers
/// (TASK-32). The texts are shared with the subclass, which lets them go with the window.
#[derive(Clone)]
pub struct EmptyTexts {
    texts: Rc<Texts>,
}

/// The id of the empty-text subclass among a window's subclasses.
const EMPTY_TEXTS_SUBCLASS: usize = 1;

impl EmptyTexts {
    /// Subclasses `parent`, a live window of this thread, so it answers
    /// LVN_GETEMPTYMARKUP for its lists. Once for each parent: a second subclass of the
    /// same parent would take the place of the first, whose texts then go unread.
    pub fn subclass(parent: *mut c_void) -> Self {
        let texts = Rc::new(Texts::default());
        let data = Rc::into_raw(Rc::clone(&texts)) as usize;
        // SAFETY: a live window of this thread; the procedure is a plain function, and its
        // data is a strong count of `texts`, given back as the window goes.
        unsafe { SetWindowSubclass(parent, Some(list_parent_proc), EMPTY_TEXTS_SUBCLASS, data) };
        Self { texts }
    }

    /// Sets the text the list `list`, a child of the parent, shows in its area while it has
    /// no rows, as Windows draws it. The text is cut to what the message can carry.
    pub fn set(&self, list: *mut c_void, text: &str) {
        const MAX_CHARS: usize = 2083;
        let wide: Vec<u16> = text.encode_utf16().take(MAX_CHARS).chain([0]).collect();
        {
            let mut texts = self.texts.borrow_mut();
            match texts.iter_mut().find(|(hwnd, _)| *hwnd == list as usize) {
                Some((_, old)) => *old = wide,
                None => texts.push((list as usize, wide)),
            }
        }
        // SAFETY: a live window; the whole client area is redrawn.
        unsafe { InvalidateRect(list, ptr::null(), 1) };
    }
}

unsafe extern "system" fn list_parent_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    data: usize,
) -> LRESULT {
    match msg {
        WM_NOTIFY => {
            // SAFETY: WM_NOTIFY's lparam points at an NMHDR, which for this code is the
            // start of an NMLVEMPTYMARKUP the list owns for the call.
            let header = unsafe { &*(lparam as *const NMHDR) };
            if header.code == LVN_GETEMPTYMARKUP {
                // SAFETY: the data is the strong count `EmptyTexts::subclass` gave, alive
                // until WM_NCDESTROY.
                let texts = unsafe { &*(data as *const Texts) };
                let text = texts
                    .borrow()
                    .iter()
                    .find(|(list, _)| *list == header.hwndFrom as usize)
                    .map(|(_, text)| text.clone());
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
            // SAFETY: the subclass installed in `EmptyTexts::subclass`, removed as the
            // window goes, as the subclassing rules ask; then its strong count is given
            // back, once.
            unsafe {
                RemoveWindowSubclass(hwnd, Some(list_parent_proc), EMPTY_TEXTS_SUBCLASS);
                drop(Rc::from_raw(data as *const Texts));
            }
        }
        _ => {}
    }
    // SAFETY: passes the message on to wx's own window procedure.
    unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
}
