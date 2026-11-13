//! The export of the difference shown (TASK-39) as a background job (ADR-0011): the work
//! writes an [`Export`] to a file on its own thread, as the text report or as CSV
//! (TASK-40), so the window answers while a large one is written (#8).
//!
//! The file goes to a new file beside the one chosen, which takes the chosen one's place
//! only once it is whole: an export that is cancelled or fails leaves no part of a file,
//! and a file it was to overwrite stays as it was (#8).

use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use revisor_core::job::{Cancel, Outcome, Work};
use revisor_core::settings::CsvDelimiter;

use crate::difference::{Export, Regional};
use crate::i18n::{En, Lang, Strings, Uk};
use crate::win32::format::{self, Locale, RegionalFormat};

/// How an export ended: the file written, or why not.
pub type Exported = Outcome<PathBuf, io::Error>;

/// What an export writes: the text report, for reading (TASK-39), or CSV with the values
/// separated by `delimiter`, for Excel and scripts (TASK-40).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Text,
    Csv { delimiter: char },
}

/// The work of exporting `export` to the file at `path` in `format`.
pub struct ExportWork {
    export: Export,
    path: PathBuf,
    format: Format,
}

impl ExportWork {
    pub fn new(export: Export, path: PathBuf, format: Format) -> Self {
        Self {
            export,
            path,
            format,
        }
    }
}

impl Work for ExportWork {
    type Progress = ();
    type Output = Exported;

    fn run(self, _progress: &(), cancel: &Cancel) -> Exported {
        let cancelled = || cancel.is_cancelled();
        if let Format::Csv { delimiter } = self.format {
            return write_csv_file(
                &self.export,
                delimiter,
                &format::utc_offset_minutes,
                &self.path,
                &cancelled,
            );
        }
        let s: &dyn Strings = match self.export.lang() {
            Lang::En => &En,
            Lang::Uk => &Uk,
        };
        // The letters of the attributes come from the property system of Windows.
        let _com = Com::init();
        let regional = RegionalFormat(Locale::USER);
        write_file(&self.export, s, &regional, &self.path, &cancelled)
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
    write_through(path, &mut |out| {
        export.write(s, regional, out, &mut |_| !cancelled())
    })
}

/// Writes `export` as CSV to `path` with the values separated by `delimiter` and the
/// moments at the offsets from UTC `utc_offset` gives, as [`write_file`] writes the report.
pub fn write_csv_file(
    export: &Export,
    delimiter: char,
    utc_offset: &dyn Fn(i64) -> Option<i32>,
    path: &Path,
    cancelled: &dyn Fn() -> bool,
) -> Exported {
    write_through(path, &mut |out| {
        export.write_csv(delimiter, utc_offset, out, &mut |_| !cancelled())
    })
}

/// Has `write` write to a new file beside `path`, which replaces it once whole; `write`
/// says whether it wrote everything or stopped.
fn write_through(
    path: &Path,
    write: &mut dyn FnMut(&mut dyn Write) -> io::Result<bool>,
) -> Exported {
    let (temporary, file) = match create_beside(path) {
        Ok(created) => created,
        Err(e) => return Outcome::Failed(e),
    };
    let mut out = BufWriter::new(file);
    let written = write(&mut out).and_then(|whole| {
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

/// The delimiter of CSV that the setting «Роздільник у CSV» gives (TASK-40 #5): its
/// character, or the list separator of the user's regional settings in Windows.
pub fn csv_delimiter(setting: CsvDelimiter) -> char {
    setting.char().unwrap_or_else(|| {
        let (list, decimal) = format::list_and_decimal_separators(Locale::USER);
        windows_delimiter(list.as_deref(), decimal.as_deref())
    })
}

/// The delimiter «as in Windows» (TASK-40 #5, decided 2026-11-14): the list separator
/// `list`, so Excel on the same computer opens the file in columns; when it is not a
/// single character, is a quote or the end of a line, or is the decimal separator
/// `decimal`, a semicolon where the decimal separator is a comma and a comma elsewhere,
/// as FreeFileSync chooses.
pub fn windows_delimiter(list: Option<&str>, decimal: Option<&str>) -> char {
    let mut chars = list.unwrap_or_default().chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if !matches!(c, '"' | '\r' | '\n') && decimal != list => c,
        _ if decimal == Some(",") => ';',
        _ => ',',
    }
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
