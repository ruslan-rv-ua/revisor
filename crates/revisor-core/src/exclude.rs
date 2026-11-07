//! Exclusions: patterns of paths and names whose files and folders stay out of a snapshot.

use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use crate::path::{self, Folded, fold};

const STAR: u32 = '*' as u32;
const QUESTION: u32 = '?' as u32;

/// The exclusions a new profile starts with, as ordinary exclusions it can edit. Chosen
/// from documentation and established practice only; the sources are in TASK-7.
pub const DEFAULT_EXCLUSIONS: [&str; 63] = [
    // System files and folders of volumes.
    "$Recycle.Bin",
    "System Volume Information",
    "Config.Msi",
    "pagefile.sys",
    "swapfile.sys",
    "hiberfil.sys",
    "$Windows.~BT",
    "$Windows.~WS",
    // Temporary files.
    "%TEMP%",
    r"%LOCALAPPDATA%\Temp",
    r"%SystemRoot%\Temp",
    r"%SystemRoot%\SystemTemp",
    r"%LOCALAPPDATA%\Packages\*\AC\Temp",
    "*.tmp",
    "~$*",
    // Windows logs and diagnostics.
    r"%SystemRoot%\Logs",
    r"%SystemRoot%\debug",
    r"%SystemRoot%\Panther",
    r"%SystemRoot%\INF\setupapi.*.log",
    r"%SystemRoot%\System32\winevt\Logs",
    r"%SystemRoot%\System32\LogFiles",
    r"%SystemRoot%\System32\MSDtc\MSDTC.LOG",
    r"%SystemRoot%\System32\MSDtc\trace\dtctrace.log",
    r"%SystemRoot%\Minidump",
    r"%SystemRoot%\MEMORY.DMP",
    r"%SystemRoot%\LiveKernelReports",
    r"%ProgramData%\Microsoft\Windows\WER",
    r"%LOCALAPPDATA%\Microsoft\Windows\WER",
    r"%LOCALAPPDATA%\CrashDumps",
    r"%ProgramData%\USOShared\Logs",
    // Windows services that update themselves.
    r"%SystemRoot%\Prefetch",
    r"%SystemRoot%\SoftwareDistribution",
    r"%SystemRoot%\WinSxS\ManifestCache",
    r"%ProgramData%\Microsoft\Network\Downloader",
    r"%ProgramData%\Microsoft\Search\Data",
    r"%ProgramData%\Microsoft\Windows Defender",
    r"%ProgramFiles%\Windows Defender",
    // Windows caches.
    r"%SystemRoot%\CSC",
    r"%LOCALAPPDATA%\Microsoft\Windows\Explorer",
    r"%LOCALAPPDATA%\Microsoft\Windows\INetCache",
    r"%LOCALAPPDATA%\Microsoft\Windows\INetCookies",
    r"%LOCALAPPDATA%\Microsoft\Windows\WebCache",
    r"%LOCALAPPDATA%\Microsoft\Windows\History",
    r"%APPDATA%\Microsoft\Windows\Recent",
    "CryptnetUrlCache",
    "D3DSCache",
    "Thumbs.db",
    // Registry hives of users: revisor does not snapshot the registry, and these files
    // change with almost every sign-in.
    "NTUSER.DAT*",
    "UsrClass.dat*",
    // Caches of Chromium and Electron programs, Firefox and Python.
    "Cache_Data",
    "No_Vary_Search",
    "Logical_Invalidation",
    "Code Cache",
    "GPUCache",
    "ShaderCache",
    "GrShaderCache",
    "GraphiteDawnCache",
    "DawnGraphiteCache",
    "DawnWebGPUCache",
    "DawnCache",
    "cache2",
    "startupCache",
    "__pycache__",
];

/// Why a pattern means nothing clear.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PatternError {
    /// It has `\` but starts with neither a drive (`C:\`) nor a variable (`%TEMP%`).
    NotFullPath,
    /// A "/", where paths are written with "\".
    Slash,
    /// A ":" anywhere but right after the drive letter of a full path.
    Colon,
    /// A control character.
    Control(char),
    /// One of the other characters no file or folder name has: `< > " |`.
    BadChar(char),
}

/// The first line of the exclusions text that is not a valid pattern.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineError {
    /// Counted from 1, blank lines included.
    pub line: usize,
    pub pattern: String,
    pub error: PatternError,
}

/// Checks that a pattern means something clear. A pattern without `\` is a name; one with
/// `\` or with a leading `%VARIABLE%` is a full path.
pub fn check(pattern: &str) -> Result<(), PatternError> {
    // "/" first: it is the usual slip in a path, whatever else is wrong.
    if pattern.contains('/') {
        return Err(PatternError::Slash);
    }
    // A pattern that starts with a variable is a full path already.
    let has_variable = leading_variable(pattern).is_some();
    let bytes = pattern.as_bytes();
    let has_drive = !has_variable
        && bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && bytes[2] == b'\\';
    if !has_variable && pattern.contains('\\') && !has_drive {
        return Err(PatternError::NotFullPath);
    }
    for (i, c) in pattern.char_indices() {
        match c {
            ':' if !(has_drive && i == 1) => return Err(PatternError::Colon),
            '<' | '>' | '"' | '|' => return Err(PatternError::BadChar(c)),
            c if c.is_control() => return Err(PatternError::Control(c)),
            _ => {}
        }
    }
    Ok(())
}

/// The patterns of a text with one pattern per line: spaces at the ends of a line are
/// dropped, and so are blank lines.
pub fn parse_lines(text: &str) -> Result<Vec<String>, LineError> {
    let mut patterns = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let pattern = line.trim();
        if pattern.is_empty() {
            continue;
        }
        check(pattern).map_err(|error| LineError {
            line: i + 1,
            pattern: pattern.to_string(),
            error,
        })?;
        patterns.push(pattern.to_string());
    }
    Ok(patterns)
}

/// What the patterns need to know about the computer they are used on.
pub trait Environment {
    /// The value of an environment variable.
    fn var(&self, name: &str) -> Option<OsString>;
    /// The real location of an existing file or folder, or `None` when it does not exist.
    fn real_path(&self, path: &Path) -> Option<PathBuf>;
}

/// This computer, as the process sees it.
pub struct SystemEnvironment;

impl Environment for SystemEnvironment {
    fn var(&self, name: &str) -> Option<OsString> {
        std::env::var_os(name)
    }

    fn real_path(&self, path: &Path) -> Option<PathBuf> {
        crate::folders::real_path(path).ok()
    }
}

/// The environment of patterns a snapshot recorded as they applied ([`Expanded`]):
/// nothing is expanded or looked up again, so they apply exactly as they did then.
pub struct RecordedEnvironment;

impl Environment for RecordedEnvironment {
    fn var(&self, _: &str) -> Option<OsString> {
        None
    }

    fn real_path(&self, _: &Path) -> Option<PathBuf> {
        None
    }
}

/// How a pattern applies on a computer: its variable expanded, and the existing part of
/// its path at its real location.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expanded {
    /// A name pattern, as written: it matches a name at any depth.
    Name(String),
    /// A path pattern: the full path it stands for. A variable or a real location may
    /// bring in a name that is not valid Unicode, and it stays as it is.
    Path(OsString),
    /// The pattern starts with a variable the computer does not have, so it leaves
    /// nothing out.
    Inactive,
}

impl Expanded {
    /// The pattern as it applied, or `None` when it did not. [`Exclusions::new`] with
    /// [`RecordedEnvironment`] takes these back as they are; a name in a path that is not
    /// valid Unicode is recorded with U+FFFD in place of what is not.
    pub fn applied(&self) -> Option<String> {
        match self {
            Expanded::Name(text) => Some(text.clone()),
            Expanded::Path(path) => Some(path.to_string_lossy().into_owned()),
            Expanded::Inactive => None,
        }
    }
}

/// Expands `pattern` for `env`: see [`Expanded`].
pub fn expand(pattern: &str, env: &dyn Environment) -> Expanded {
    let path = match leading_variable(pattern) {
        Some((name, rest)) => {
            let Some(mut path) = env.var(name) else {
                return Expanded::Inactive;
            };
            path.push(rest);
            Some(path)
        }
        None => pattern.contains('\\').then(|| OsString::from(pattern)),
    };
    match path {
        Some(path) => Expanded::Path(pattern_parts(&path, env).join(OsStr::new(r"\"))),
        None => Expanded::Name(pattern.to_string()),
    }
}

/// Exclusions ready to be matched against the entries of a tree.
#[derive(Clone, Debug, Default)]
pub struct Exclusions {
    /// Name patterns without wildcards, folded.
    names: HashSet<Folded>,
    /// Name patterns with `*` or `?`, folded.
    wild_names: Vec<Folded>,
    /// Path patterns, each a list of folded parts ([`path::parts`]); the first part is the
    /// drive.
    paths: Vec<Vec<Folded>>,
}

/// Where a walk stands in the tree.
#[derive(Clone, Debug, Default)]
pub struct Branch {
    /// How many parts the path of the current folder has.
    depth: usize,
    /// The path patterns that the path of the current folder matches so far: they are
    /// longer than it, and their first parts match its parts.
    matching: Vec<u32>,
}

impl Exclusions {
    /// Prepares `patterns` for matching on this computer: a leading environment variable is
    /// expanded now, and a pattern whose variable is not set leaves nothing out.
    pub fn new<S: AsRef<str>>(patterns: &[S], env: &dyn Environment) -> Exclusions {
        let expanded: Vec<Expanded> = patterns
            .iter()
            .map(|pattern| expand(pattern.as_ref(), env))
            .collect();
        Self::from_expanded(&expanded)
    }

    /// Prepares patterns already expanded with [`expand`] for matching.
    pub fn from_expanded(expanded: &[Expanded]) -> Exclusions {
        let mut ex = Exclusions::default();
        for pattern in expanded {
            match pattern {
                Expanded::Path(path) => ex.paths.push(path::parts(path)),
                Expanded::Name(name) => {
                    let folded = fold(OsStr::new(name));
                    if folded.chars().iter().any(|&c| c == STAR || c == QUESTION) {
                        ex.wild_names.push(folded);
                    } else {
                        ex.names.insert(folded);
                    }
                }
                Expanded::Inactive => {}
            }
        }
        ex
    }

    /// Where a walk of the tree under `root` starts. The root itself is never excluded.
    pub fn branch(&self, root: &Path) -> Branch {
        let root = path::parts(root.as_os_str());
        let matching = self
            .paths
            .iter()
            .enumerate()
            .filter(|(_, p)| p.len() > root.len() && p.iter().zip(&root).all(|(p, r)| glob(p, r)))
            .map(|(i, _)| i as u32)
            .collect();
        Branch {
            depth: root.len(),
            matching,
        }
    }

    /// The branch of the child `name` of `parent`, or `None` when the child is excluded.
    pub fn child(&self, parent: &Branch, name: &OsStr) -> Option<Branch> {
        if self.names.is_empty() && self.wild_names.is_empty() && parent.matching.is_empty() {
            return Some(Branch {
                depth: parent.depth + 1,
                matching: Vec::new(),
            });
        }
        let name = fold(name);
        if self.names.contains(&name) || self.wild_names.iter().any(|p| glob(p, &name)) {
            return None;
        }
        let mut matching = Vec::new();
        for &i in &parent.matching {
            let pattern = &self.paths[i as usize];
            if glob(&pattern[parent.depth], &name) {
                if pattern.len() == parent.depth + 1 {
                    return None;
                }
                matching.push(i);
            }
        }
        Some(Branch {
            depth: parent.depth + 1,
            matching,
        })
    }
}

/// The environment variable a pattern starts with, `%NAME%`, and the rest of the pattern.
fn leading_variable(pattern: &str) -> Option<(&str, &str)> {
    let rest = pattern.strip_prefix('%')?;
    let end = rest.find('%')?;
    (end > 0).then(|| (&rest[..end], &rest[end + 1..]))
}

/// The parts of a path pattern. The longest part of it without wildcards that exists is
/// taken at its real location, the way the scanner sees it: Windows keeps TEMP in the 8.3
/// form, and a folder may be reached through a junction or a `subst` drive.
fn pattern_parts(path: &OsStr, env: &dyn Environment) -> Vec<OsString> {
    let mut parts = path::split(path);
    let wild = |unit: u16| u32::from(unit) == STAR || u32::from(unit) == QUESTION;
    let literal = parts
        .iter()
        .take_while(|part| !part.encode_wide().any(wild))
        .count();
    for len in (1..=literal).rev() {
        let mut prefix = parts[..len].join(OsStr::new(r"\"));
        if len == 1 {
            // "C:" alone would name the current folder of drive C.
            prefix.push(r"\");
        }
        if let Some(real) = env.real_path(Path::new(&prefix)) {
            let mut resolved = path::split(real.as_os_str());
            resolved.extend(parts.drain(len..));
            parts = resolved;
            break;
        }
    }
    parts
}

/// Whether a folded name matches a folded pattern, where `*` stands for any run of
/// characters and `?` for one character.
fn glob(pattern: &Folded, name: &Folded) -> bool {
    let (pattern, name) = (pattern.chars(), name.chars());
    let (mut p, mut n) = (0, 0);
    // Where the last `*` was, and the part of the name it has taken so far.
    let mut star = None;
    while n < name.len() {
        match pattern.get(p) {
            Some(&c) if c == QUESTION || c == name[n] => {
                p += 1;
                n += 1;
            }
            Some(&STAR) => {
                star = Some((p, n));
                p += 1;
            }
            _ => match star {
                // Let the last `*` take one more character and try again.
                Some((sp, sn)) => {
                    star = Some((sp, sn + 1));
                    p = sp + 1;
                    n = sn + 1;
                }
                None => return false,
            },
        }
    }
    pattern[p..].iter().all(|&c| c == STAR)
}

#[cfg(test)]
mod tests;
