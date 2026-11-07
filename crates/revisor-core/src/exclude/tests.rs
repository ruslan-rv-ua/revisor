use super::*;
use std::collections::HashMap;

/// A made-up computer: its variables, and the paths that exist on it with their real
/// locations.
#[derive(Default)]
struct FakeEnv {
    vars: HashMap<String, String>,
    real: HashMap<String, String>,
}

impl FakeEnv {
    fn with(vars: &[(&str, &str)]) -> Self {
        Self {
            vars: vars
                .iter()
                .map(|&(k, v)| (k.to_ascii_uppercase(), v.to_string()))
                .collect(),
            real: HashMap::new(),
        }
    }

    fn existing(mut self, path: &str, real: &str) -> Self {
        self.real.insert(path.to_uppercase(), real.to_string());
        self
    }
}

impl Environment for FakeEnv {
    fn var(&self, name: &str) -> Option<OsString> {
        self.vars
            .get(&name.to_ascii_uppercase())
            .map(OsString::from)
    }

    fn real_path(&self, path: &Path) -> Option<PathBuf> {
        let key = path.to_string_lossy().to_uppercase();
        self.real.get(&key).map(PathBuf::from)
    }
}

fn exclusions(patterns: &[&str]) -> Exclusions {
    Exclusions::new(patterns, &FakeEnv::default())
}

/// Whether the entry at `rel` under `root` is left out, walking to it the way the scanner
/// does: an excluded folder is never entered.
fn excluded(ex: &Exclusions, root: &str, rel: &str) -> bool {
    let mut branch = ex.branch(Path::new(root));
    for name in rel.split('\\') {
        match ex.child(&branch, OsStr::new(name)) {
            Some(next) => branch = next,
            None => return true,
        }
    }
    false
}

#[test]
fn a_name_pattern_leaves_out_that_name_at_any_depth() {
    let ex = exclusions(&["Code Cache"]);
    assert!(excluded(&ex, r"C:\Data", "Code Cache"));
    assert!(excluded(&ex, r"C:\Data", r"app\x\code cache"));
    assert!(!excluded(&ex, r"C:\Data", r"app\Code Cache 2"));
    assert!(!excluded(&ex, r"C:\Data", r"app\Code"));
}

#[test]
fn wildcards_in_a_name_pattern_match_within_the_name() {
    let ex = exclusions(&["*.tmp", "~$*", "NTUSER.DAT*", "a?c"]);
    assert!(excluded(&ex, r"C:\Data", r"x\report.TMP"));
    assert!(excluded(&ex, r"C:\Data", "~$letter.docx"));
    assert!(excluded(&ex, r"C:\Data", "ntuser.dat.LOG1"));
    assert!(excluded(&ex, r"C:\Data", "NTUSER.DAT"));
    assert!(excluded(&ex, r"C:\Data", "abc"));
    // `?` is one character, even one that takes two UTF-16 units.
    assert!(excluded(&ex, r"C:\Data", "a😀c"));
    assert!(!excluded(&ex, r"C:\Data", "abbc"));
    assert!(!excluded(&ex, r"C:\Data", "report.tmp.txt"));
    assert!(!excluded(&ex, r"C:\Data", "letter~$.docx"));
}

#[test]
fn a_path_pattern_leaves_out_only_that_path() {
    let ex = exclusions(&[r"C:\Windows\Temp"]);
    assert!(excluded(&ex, r"C:\", r"Windows\Temp"));
    assert!(excluded(&ex, r"c:\WINDOWS", "temp"));
    assert!(!excluded(&ex, r"C:\", r"Data\Windows\Temp"));
    assert!(!excluded(&ex, r"C:\", r"Windows\Temp2"));
    assert!(!excluded(&ex, r"C:\", "Temp"));
    assert!(!excluded(&ex, r"D:\", r"Windows\Temp"));
}

#[test]
fn wildcards_in_a_path_pattern_match_within_one_part() {
    let ex = exclusions(&[r"C:\Packages\*\AC\Temp", r"C:\Windows\INF\setupapi.*.log"]);
    assert!(excluded(&ex, r"C:\", r"Packages\App.One\AC\Temp"));
    assert!(!excluded(&ex, r"C:\", r"Packages\App\x\AC\Temp"));
    assert!(excluded(&ex, r"C:\Windows", r"INF\setupapi.dev.log"));
    assert!(!excluded(&ex, r"C:\Windows", r"INF\setupapi.log"));
}

#[test]
fn a_pattern_may_start_with_an_environment_variable() {
    let env = FakeEnv::with(&[
        ("TEMP", r"C:\Users\R\AppData\Local\Temp"),
        ("SystemRoot", r"D:\WINDOWS"),
    ]);
    let ex = Exclusions::new(&["%TEMP%", r"%systemroot%\Logs", r"%Missing%\x"], &env);
    assert!(excluded(&ex, r"C:\Users\R", r"AppData\Local\Temp"));
    assert!(excluded(&ex, r"D:\", r"Windows\Logs"));
    assert!(!excluded(&ex, r"C:\", r"Windows\Logs"));
    // A variable this computer does not have leaves nothing out.
    assert!(!excluded(&ex, r"C:\", r"%Missing%\x"));
    assert!(!excluded(&ex, r"C:\", "%TEMP%"));
}

#[test]
fn the_existing_part_of_a_pattern_is_taken_at_its_real_location() {
    // Windows keeps TEMP in the 8.3 form, while the scanner sees long names.
    let short = r"C:\Users\RUSLAN~1\AppData\Local\Temp";
    let long = r"C:\Users\Руслан\AppData\Local\Temp";
    let env = FakeEnv::with(&[("TEMP", short)]).existing(short, long);
    let ex = Exclusions::new(&["%TEMP%", r"%TEMP%\gone\*.dmp", r"%TEMP%\*\x"], &env);
    assert!(excluded(&ex, r"C:\Users\Руслан", r"AppData\Local\Temp"));
    assert!(excluded(&ex, long, r"gone\a.dmp"));
    assert!(excluded(&ex, long, r"any\x"));
}

#[test]
fn the_exclusions_field_holds_one_pattern_per_line() {
    let text = "  *.tmp \r\n\r\n%TEMP%\n\tC:\\Temp\\  \n";
    assert_eq!(
        parse_lines(text),
        Ok(vec!["*.tmp".into(), "%TEMP%".into(), r"C:\Temp\".into()])
    );
}

#[test]
fn a_pattern_whose_meaning_is_unclear_is_refused_with_its_line() {
    assert_eq!(
        parse_lines("*.tmp\n\n foo\\bar"),
        Err(LineError {
            line: 3,
            pattern: r"foo\bar".into(),
            error: PatternError::NotFullPath,
        })
    );
    for pattern in [r"\Windows\Temp", r"\\server\share", r"1:\x", r"C:Temp\x"] {
        assert_eq!(check(pattern), Err(PatternError::NotFullPath), "{pattern}");
    }
    for (pattern, error) in [
        ("C:/Temp", PatternError::Slash),
        ("a|b", PatternError::BadChar('|')),
        ("x\"y", PatternError::BadChar('"')),
        ("<a>", PatternError::BadChar('<')),
        ("a:b", PatternError::Colon),
        (r"C:\a:b", PatternError::Colon),
        ("C:", PatternError::Colon),
        ("a\u{7}b", PatternError::Control('\u{7}')),
        ("a\u{7f}b", PatternError::Control('\u{7f}')),
        ("a\u{85}b", PatternError::Control('\u{85}')),
        (r"%TEMP%\a>b", PatternError::BadChar('>')),
        // The variable's name is part of the line too.
        (r"%A|B%\x", PatternError::BadChar('|')),
    ] {
        assert_eq!(check(pattern), Err(error), "{pattern}");
    }
    // Repeats and variables this computer does not have are fine.
    for pattern in [
        "Code Cache",
        r"C:\",
        r"c:\Windows\*\x?",
        "%TEMP%",
        r"%SystemRoot%\Logs",
        r"%NoSuchVariable%\x",
        "~$*",
        "%%x",
        "100%",
    ] {
        assert_eq!(check(pattern), Ok(()), "{pattern}");
    }
}

#[test]
fn the_default_list_is_the_one_from_task_7_word_for_word() {
    // The list in the description of TASK-7, in its order (2026-10-31).
    let expected = [
        "$Recycle.Bin",
        "System Volume Information",
        "Config.Msi",
        "pagefile.sys",
        "swapfile.sys",
        "hiberfil.sys",
        "$Windows.~BT",
        "$Windows.~WS",
        "%TEMP%",
        r"%LOCALAPPDATA%\Temp",
        r"%SystemRoot%\Temp",
        r"%SystemRoot%\SystemTemp",
        r"%LOCALAPPDATA%\Packages\*\AC\Temp",
        "*.tmp",
        "~$*",
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
        r"%SystemRoot%\Prefetch",
        r"%SystemRoot%\SoftwareDistribution",
        r"%SystemRoot%\WinSxS\ManifestCache",
        r"%ProgramData%\Microsoft\Network\Downloader",
        r"%ProgramData%\Microsoft\Search\Data",
        r"%ProgramData%\Microsoft\Windows Defender",
        r"%ProgramFiles%\Windows Defender",
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
        "NTUSER.DAT*",
        "UsrClass.dat*",
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
    assert_eq!(expected.len(), 63);
    assert_eq!(DEFAULT_EXCLUSIONS, expected);
    for pattern in DEFAULT_EXCLUSIONS {
        assert_eq!(check(pattern), Ok(()), "{pattern}");
    }
    let new = crate::store::ProfileSettings::new_profile();
    assert_eq!((new.name.as_str(), new.scope.folders.len()), ("", 0));
    assert_eq!(new.scope.exclusions, expected);
}

#[test]
fn this_computer_gives_its_variables_and_real_locations() {
    let env = SystemEnvironment;
    let windows = env.var("SystemRoot").expect("Windows sets SystemRoot");
    let real = env
        .real_path(Path::new(&windows))
        .expect("the Windows folder exists");
    assert!(
        real.to_string_lossy()
            .eq_ignore_ascii_case(&windows.to_string_lossy()),
        "{real:?}"
    );
    assert_eq!(env.real_path(Path::new(r"C:\revisor: no such path")), None);
}

#[test]
fn a_pattern_expands_to_how_it_applies_on_the_computer() {
    let short = r"C:\Users\RUSLAN~1\AppData\Local\Temp";
    let long = r"C:\Users\Руслан\AppData\Local\Temp";
    let env = FakeEnv::with(&[("TEMP", short)]).existing(short, long);
    assert_eq!(expand("%TEMP%", &env), Expanded::Path(long.into()));
    assert_eq!(
        expand(r"%TEMP%\gone\*.dmp", &env),
        Expanded::Path(format!(r"{long}\gone\*.dmp").into())
    );
    assert_eq!(expand(r"D:\Logs\", &env), Expanded::Path(r"D:\Logs".into()));
    assert_eq!(expand("*.tmp", &env), Expanded::Name("*.tmp".into()));
    assert_eq!(expand(r"%Missing%\x", &env), Expanded::Inactive);
    assert_eq!(Expanded::Inactive.applied(), None);
    assert_eq!(Expanded::Name("x".into()).applied(), Some("x".into()));
    // Expanded once, the patterns match as `new` would have them.
    let ex = Exclusions::from_expanded(&[
        expand("%TEMP%", &env),
        expand("*.tmp", &env),
        expand(r"%Missing%\x", &env),
    ]);
    assert!(excluded(&ex, r"C:\Users\Руслан", r"AppData\Local\Temp"));
    assert!(excluded(&ex, r"C:\Data", "x.tmp"));
    assert!(!excluded(&ex, r"C:\Data", "x"));
}

#[test]
fn recorded_patterns_apply_as_they_did_without_expanding_again() {
    let env = FakeEnv::with(&[("TEMP", r"C:\Users\R\AppData\Local\Temp")]);
    let recorded: Vec<String> = ["%TEMP%", "*.tmp", r"%Missing%\x"]
        .iter()
        .filter_map(|p| expand(p, &env).applied())
        .collect();
    assert_eq!(recorded, [r"C:\Users\R\AppData\Local\Temp", "*.tmp"]);
    // Later, whatever the variables are then, the recorded patterns mean what they did.
    let ex = Exclusions::new(&recorded, &RecordedEnvironment);
    assert!(excluded(&ex, r"C:\Users\R", r"AppData\Local\Temp"));
    assert!(excluded(&ex, r"C:\Data", "x.tmp"));
    assert!(!excluded(&ex, r"C:\Data", "x"));
    assert_eq!(RecordedEnvironment.var("TEMP"), None);
    assert_eq!(RecordedEnvironment.real_path(Path::new(r"C:\")), None);
}

#[test]
fn a_profile_folder_itself_is_never_excluded() {
    let ex = exclusions(&["Code Cache", r"C:\Windows\Temp", r"C:\Windows"]);
    assert!(!excluded(&ex, r"C:\Data\Code Cache", "x.bin"));
    assert!(!excluded(&ex, r"C:\Windows\Temp", "x.log"));
    assert!(!excluded(&ex, r"C:\Windows", "System32"));
}

/// A computer whose variable `ODD` holds `value`, and where `C:\Data` is really at
/// `real`.
struct OddEnv {
    value: OsString,
    real: PathBuf,
}

impl Environment for OddEnv {
    fn var(&self, name: &str) -> Option<OsString> {
        name.eq_ignore_ascii_case("ODD").then(|| self.value.clone())
    }

    fn real_path(&self, path: &Path) -> Option<PathBuf> {
        path.as_os_str()
            .eq_ignore_ascii_case(r"C:\Data")
            .then(|| self.real.clone())
    }
}

/// `before`, then the UTF-16 unit `unit`, then `after`: a name that need not be valid
/// Unicode.
fn with_unit(before: &str, unit: u16, after: &str) -> OsString {
    use std::os::windows::ffi::OsStringExt;
    let wide: Vec<u16> = before
        .encode_utf16()
        .chain([unit])
        .chain(after.encode_utf16())
        .collect();
    OsString::from_wide(&wide)
}

/// [`excluded`] for names that need not be valid Unicode.
fn excluded_os(ex: &Exclusions, root: &OsStr, rel: &[&OsStr]) -> bool {
    let mut branch = ex.branch(Path::new(root));
    for name in rel {
        match ex.child(&branch, name) {
            Some(next) => branch = next,
            None => return true,
        }
    }
    false
}

#[test]
fn a_path_pattern_keeps_a_name_that_is_not_valid_unicode() {
    // Before TASK-29 the expanded path went through `to_string_lossy`: the unpaired
    // surrogate became U+FFFD, and the folder it named was never left out.
    let odd = with_unit("T", 0xD800, "mp");
    let other = with_unit("T", 0xDC00, "mp");
    let mut value = OsString::from(r"C:\");
    value.push(&odd);
    let env = OddEnv {
        value,
        real: PathBuf::from(r"C:\Data"),
    };
    let ex = Exclusions::new(&[r"%ODD%\Logs"], &env);
    let c = OsStr::new(r"C:\");
    assert!(excluded_os(&ex, c, &[&odd, OsStr::new("logs")]));
    assert!(!excluded_os(&ex, c, &[&other, OsStr::new("Logs")]));
    assert!(!excluded_os(
        &ex,
        c,
        &[OsStr::new("T\u{FFFD}mp"), OsStr::new("Logs")]
    ));
    // The same from a real location, and from a profile folder with such a name.
    let mut real = OsString::from(r"C:\");
    real.push(&odd);
    let env = OddEnv {
        value: OsString::new(),
        real: PathBuf::from(&real),
    };
    let ex = Exclusions::new(&[r"C:\Data\Logs"], &env);
    assert!(excluded_os(&ex, &real, &[OsStr::new("Logs")]));
    let mut other_root = OsString::from(r"C:\");
    other_root.push(&other);
    assert!(!excluded_os(&ex, &other_root, &[OsStr::new("Logs")]));
    // The snapshot records the pattern as text.
    assert_eq!(
        expand(r"C:\Data\Logs", &env).applied(),
        Some("C:\\T\u{FFFD}mp\\Logs".to_string())
    );
}

#[test]
fn a_path_pattern_at_a_network_location_matches_with_or_without_the_prefix() {
    // Before TASK-29 `\\?\UNC\server\share` had the parts UNC, server, share, and
    // `\\server\share` only server, share.
    let env = OddEnv {
        value: OsString::new(),
        real: PathBuf::from(r"\\?\UNC\server\share\Data"),
    };
    let ex = Exclusions::new(&[r"C:\Data\Logs"], &env);
    assert!(excluded(&ex, r"\\server\share", r"Data\Logs"));
    assert!(excluded(&ex, r"\\?\unc\server\share\data", "LOGS"));
    assert!(!excluded(&ex, r"\\server\share", "Logs"));
}
