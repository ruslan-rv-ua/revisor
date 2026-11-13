//! What screen readers are told beyond what wx tells them: a window's accessible name, a
//! property page (TASK-44), a polite live region (IAccPropServices), and the change of the
//! status bar's message.

use std::ffi::c_void;
use std::mem::size_of;

use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows_sys::Win32::UI::Accessibility::{
    CLSID_AccPropServices, LiveSetting_Property_GUID, NotifyWinEvent, PROPID_ACC_NAME,
    PROPID_ACC_ROLE, Polite, ROLE_SYSTEM_PROPERTYPAGE,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CHILDID_SELF, EVENT_OBJECT_LIVEREGIONCHANGED, OBJID_CLIENT,
};
use windows_sys::core::{GUID, HRESULT};

use super::wide;

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
    /// A thread's own: a COM object belongs to the apartment of the thread that made it,
    /// and only the GUI thread names its windows. Kept for the program's life, so the
    /// callers need not carry it (TASK-32).
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
        let wide = wide(name);
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
        self.set_i4(hwnd, LiveSetting_Property_GUID, Polite);
    }

    fn set_role(&self, hwnd: HWND, role: u32) {
        self.set_i4(hwnd, PROPID_ACC_ROLE, role as i32);
    }

    fn set_i4(&self, hwnd: HWND, property: GUID, value: i32) {
        let value = Variant {
            vt: VT_I4,
            reserved: [0; 3],
            value,
            padding: [0; 3],
        };
        // SAFETY: a live window and a VT_I4 VARIANT.
        unsafe {
            (self.vtbl().set_hwnd_prop)(
                self.0,
                hwnd,
                OBJID_CLIENT as u32,
                CHILDID_SELF,
                property,
                value,
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

/// Makes a window the property page `name` of a tab control, as the pages of Windows' own
/// property sheets are: NVDA then says the page when the focus moves onto it, as Ctrl+Tab
/// moves it. A wx page is a plain window, which NVDA passes over (TASK-44 #7).
pub fn make_property_page(hwnd: *mut c_void, name: &str) {
    if let Some(props) = acc_props() {
        props.set_name(hwnd, name);
        props.set_role(hwnd, ROLE_SYSTEM_PROPERTYPAGE);
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
