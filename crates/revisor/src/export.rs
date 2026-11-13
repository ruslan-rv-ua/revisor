//! The export of the difference shown (TASK-39) as a background job (ADR-0011): the work
//! writes the report of an [`Export`] to a file on its own thread, so the window answers
//! while a large one is written (#8).
//!
//! The report goes to a new file beside the one chosen, which takes the chosen one's place
//! only once it is whole: an export that is cancelled or fails leaves no part of a file,
//! and a file it was to overwrite stays as it was (#8).

use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use revisor_core::job::{Cancel, Outcome, Work};

use crate::difference::{Export, Regional};
use crate::i18n::{En, Lang, Strings, Uk};
use crate::win32::format::{Locale, RegionalFormat};

/// How an export ended: the file written, or why not.
pub type Exported = Outcome<PathBuf, io::Error>;

/// The work of exporting `export` to the file at `path`.
pub struct ExportWork {
    export: Export,
    path: PathBuf,
}

impl ExportWork {
    pub fn new(export: Export, path: PathBuf) -> Self {
        Self { export, path }
    }
}

impl Work for ExportWork {
    type Progress = ();
    type Output = Exported;

    fn run(self, _progress: &(), cancel: &Cancel) -> Exported {
        let s: &dyn Strings = match self.export.lang() {
            Lang::En => &En,
            Lang::Uk => &Uk,
        };
        // The letters of the attributes come from the property system of Windows.
        let _com = Com::init();
        let regional = RegionalFormat(Locale::USER);
        write_file(&self.export, s, &regional, &self.path, &|| {
            cancel.is_cancelled()
        })
    }
}

/// COM on the job's thread, for as long as the value lives.
struct Com(bool);

impl Com {
    fn init() -> Self {
        use windows_sys::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
        // SAFETY: called on the job's own thread, with no reserved pointer.
        let hr = unsafe { CoInitializeEx(std::ptr::null(), COINIT_MULTITHREADED as _) };
        Self(hr >= 0)
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: balances the successful CoInitializeEx of this thread.
            unsafe { windows_sys::Win32::System::Com::CoUninitialize() };
        }
    }
}

/// Writes the report of `export` to `path`, through a new file beside it that replaces it
/// once whole; `cancelled` is asked every few thousand rows. The file written comes back.
pub fn write_file(
    export: &Export,
    s: &dyn Strings,
    regional: &dyn Regional,
    path: &Path,
    cancelled: &dyn Fn() -> bool,
) -> Exported {
    let (temporary, file) = match create_beside(path) {
        Ok(created) => created,
        Err(e) => return Outcome::Failed(e),
    };
    let mut out = BufWriter::new(file);
    let written = export
        .write(s, regional, &mut out, &mut |_| !cancelled())
        .and_then(|whole| {
            let file = out.into_inner().map_err(io::IntoInnerError::into_error)?;
            file.sync_all()?;
            drop(file);
            Ok(whole)
        });
    let outcome = match written {
        Ok(true) => match fs::rename(&temporary, path) {
            Ok(()) => return Outcome::Done(path.to_path_buf()),
            Err(e) => Outcome::Failed(e),
        },
        Ok(false) => Outcome::Cancelled,
        Err(e) => Outcome::Failed(e),
    };
    let _ = fs::remove_file(&temporary);
    outcome
}

/// A new file in the folder of `path`, named after it, which no other file has.
fn create_beside(path: &Path) -> io::Result<(PathBuf, File)> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let name = path.file_name().unwrap_or_default();
    for attempt in 0u32.. {
        let mut temporary_name = OsString::from(name);
        temporary_name.push(format!(".{}-{stamp}-{attempt}.tmp", std::process::id()));
        let temporary = path.with_file_name(temporary_name);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => return Ok((temporary, file)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists && attempt < 100 => {}
            Err(e) => return Err(e),
        }
    }
    unreachable!("the loop returns by its hundredth attempt")
}

/// Opens `path` in the program Windows assigns to its type (TASK-39 #9).
pub fn open(path: &Path) -> io::Result<()> {
    crate::win32::shell::open_file(path)
}
