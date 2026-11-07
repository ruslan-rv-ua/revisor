//! The shell: the shield icon of a menu item that needs administrator rights, File
//! Explorer opened on an entry (TASK-14), and the restart as administrator (TASK-8).

use std::ffi::{OsStr, c_void};
use std::io;
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr;

use windows_sys::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS,
    DeleteDC, DeleteObject, HBITMAP, RGBQUAD, SelectObject,
};
use windows_sys::Win32::UI::Shell::Common::ITEMIDLIST;
use windows_sys::Win32::UI::Shell::{
    ILFree, SHGSI_ICON, SHGSI_SMALLICON, SHGetStockIconInfo, SHOpenFolderAndSelectItems,
    SHParseDisplayName, SHSTOCKICONINFO, SIID_SHIELD, ShellExecuteW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DI_NORMAL, DestroyIcon, DrawIconEx, GetMenu, GetSubMenu, GetSystemMetrics, HICON,
    MENUITEMINFOW, MIIM_BITMAP, SM_CXSMICON, SM_CYSMICON, SW_SHOWNORMAL, SetMenuItemInfoW,
};
use windows_sys::core::HRESULT;

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

/// `s` as a NUL-terminated UTF-16 string.
fn wide_os(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain([0]).collect()
}

/// Opens the folder of `path` in File Explorer with the entry itself selected — a file, a
/// folder or a link, not the link's target (TASK-14 #11): the shell's item of the path,
/// given alone, makes File Explorer open its parent folder and select it.
pub fn select_in_explorer(path: &Path) -> io::Result<()> {
    let wide = wide_os(path.as_os_str());
    let mut item: *mut ITEMIDLIST = ptr::null_mut();
    // SAFETY: a NUL-terminated path and an out pointer; no bind context, no attributes.
    let parsed = unsafe {
        SHParseDisplayName(
            wide.as_ptr(),
            ptr::null_mut(),
            &mut item,
            0,
            ptr::null_mut(),
        )
    };
    if parsed < 0 || item.is_null() {
        return Err(hresult_error(parsed));
    }
    // SAFETY: the item just parsed; with no items of its own (cidl 0), the call opens the
    // item's folder and selects the item.
    let opened = unsafe { SHOpenFolderAndSelectItems(item, 0, ptr::null(), 0) };
    // SAFETY: the item came from SHParseDisplayName and is freed once.
    unsafe { ILFree(item) };
    if opened < 0 {
        Err(hresult_error(opened))
    } else {
        Ok(())
    }
}

/// Opens `folder` in File Explorer with nothing selected (TASK-14 #12), by the verb
/// «explore», as browsers do when an item cannot be selected.
pub fn open_in_explorer(folder: &Path) -> io::Result<()> {
    let verb = wide_os(OsStr::new("explore"));
    let folder = wide_os(folder.as_os_str());
    // SAFETY: every string is NUL-terminated; no window owns an error box.
    let result = unsafe {
        ShellExecuteW(
            ptr::null_mut(),
            verb.as_ptr(),
            folder.as_ptr(),
            ptr::null(),
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

/// Starts `exe` with `args` as administrator, through the UAC prompt. An error means it
/// did not start; declining the prompt is `ERROR_CANCELLED`.
pub fn run_as_admin(exe: &Path, args: &str) -> io::Result<()> {
    let verb = wide_os(OsStr::new("runas"));
    let exe = wide_os(exe.as_os_str());
    let args = wide_os(OsStr::new(args));
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

/// A failed HRESULT as an I/O error: the Windows error it carries, or the HRESULT itself,
/// whose text Windows knows too.
fn hresult_error(hr: HRESULT) -> io::Error {
    const FACILITY_WIN32: u32 = 0x8007_0000;
    let code = hr as u32;
    if code & 0xFFFF_0000 == FACILITY_WIN32 {
        io::Error::from_raw_os_error((code & 0xFFFF) as i32)
    } else {
        io::Error::from_raw_os_error(hr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_hresult_gives_the_win32_error_it_carries_or_its_own_code() {
        use crate::failure::of_io;
        use crate::i18n::Failure;
        let failure = |hr: u32| of_io(&hresult_error(hr.cast_signed()));
        // E_FAIL carries no Win32 error: its own code, negative as a number, which the
        // interface writes as 0x80004005, with the text Windows has for it (TASK-26).
        let Failure::Other { text, code } = failure(0x8000_4005) else {
            panic!("E_FAIL gives no failure with a code");
        };
        assert_eq!(code, i64::from(0x8000_4005_u32.cast_signed()));
        assert!(!text.is_empty() && !text.contains("os error"), "{text}");
        // An HRESULT made of a Win32 error is that error: a known cause, or its code.
        assert_eq!(failure(0x8007_0005), Failure::AccessDenied);
        assert!(matches!(
            failure(0x8007_0003),
            Failure::Other { code: 3, .. }
        ));
    }
}
