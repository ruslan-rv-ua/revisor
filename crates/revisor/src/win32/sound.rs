//! Plays revisor's own sounds (TASK-46): the WAV data is made once and stays in memory,
//! as an asynchronous `PlaySound` reads it while it plays; nothing is read from the sound
//! scheme of Windows or written anywhere.

use std::sync::OnceLock;

use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};

use crate::sound::{Chime, notes, wav};

/// Plays `chime` without waiting for it. A sound that cannot play — no sound device, for
/// one — is let go without a word (TASK-46 #9).
pub fn play(chime: Chime) {
    static DONE: OnceLock<Vec<u8>> = OnceLock::new();
    static NO_RESULT: OnceLock<Vec<u8>> = OnceLock::new();
    let data = match chime {
        Chime::Done => DONE.get_or_init(|| wav(notes(Chime::Done))),
        Chime::NoResult => NO_RESULT.get_or_init(|| wav(notes(Chime::NoResult))),
    };
    // SAFETY: with SND_MEMORY the first argument points at a whole WAV file, which lives
    // in a static for the rest of the process, so it outlasts the asynchronous play.
    unsafe {
        PlaySoundW(
            data.as_ptr().cast(),
            std::ptr::null_mut(),
            SND_MEMORY | SND_ASYNC | SND_NODEFAULT,
        )
    };
}
