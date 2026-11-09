//! Whether a contrast theme of Windows is on (TASK-34): it always wins over the theme of
//! revisor, so the interface keeps the colours the user chose for legibility.

use std::mem::size_of;

use windows_sys::Win32::UI::Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW};
use windows_sys::Win32::UI::WindowsAndMessaging::{SPI_GETHIGHCONTRAST, SystemParametersInfoW};

/// Whether a contrast theme of Windows is on now; `false` when Windows does not say.
pub fn high_contrast() -> bool {
    let mut contrast = HIGHCONTRASTW {
        cbSize: size_of::<HIGHCONTRASTW>() as u32,
        ..Default::default()
    };
    // SAFETY: a HIGHCONTRASTW with its size set, as SPI_GETHIGHCONTRAST fills it.
    let read = unsafe {
        SystemParametersInfoW(
            SPI_GETHIGHCONTRAST,
            contrast.cbSize,
            (&raw mut contrast).cast(),
            0,
        )
    };
    read != 0 && contrast.dwFlags & HCF_HIGHCONTRASTON != 0
}
