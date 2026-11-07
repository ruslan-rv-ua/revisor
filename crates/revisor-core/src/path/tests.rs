use super::*;

/// UTF-16 text with `unit` between `before` and `after`, as a name that need not be valid
/// Unicode.
fn with_unit(before: &str, unit: u16, after: &str) -> OsString {
    let wide: Vec<u16> = before
        .encode_utf16()
        .chain([unit])
        .chain(after.encode_utf16())
        .collect();
    OsString::from_wide(&wide)
}

fn folded(text: &str) -> Folded {
    fold(OsStr::new(text))
}

#[test]
fn verbatim_paths() {
    assert_eq!(verbatim(Path::new(r"C:\")), Path::new(r"\\?\C:\"));
    assert_eq!(verbatim(Path::new(r"C:\a\b")), Path::new(r"\\?\C:\a\b"));
    assert_eq!(
        verbatim(Path::new(r"\\server\share\x")),
        Path::new(r"\\?\UNC\server\share\x")
    );
    // Windows takes a `\\?\` path as it is, so `..` is resolved before, by the caller.
    let absolute = std::path::absolute(r"C:\a\..\b").unwrap();
    assert_eq!(verbatim(&absolute), Path::new(r"\\?\C:\b"));
}

#[test]
fn verbatim_and_device_paths_stay_as_they_are() {
    for path in [
        r"\\?\C:\x",
        r"\\?\UNC\server\share\x",
        r"\\?\Volume{0b5c1d2e-0000-0000-0000-100000000000}\x",
        r"\\.\C:\x",
        r"\\.\PhysicalDrive0",
    ] {
        assert_eq!(verbatim(Path::new(path)), Path::new(path), "{path}");
    }
}

#[test]
fn plain_paths_drop_the_verbatim_prefix() {
    assert_eq!(plain(Path::new(r"\\?\C:\a\b")), Path::new(r"C:\a\b"));
    assert_eq!(
        plain(Path::new(r"\\?\UNC\server\share\x")),
        Path::new(r"\\server\share\x")
    );
    assert_eq!(plain(Path::new(r"C:\a")), Path::new(r"C:\a"));
    assert_eq!(
        plain(Path::new(r"\\server\share\x")),
        Path::new(r"\\server\share\x")
    );
    // A device path is not a verbatim one.
    assert_eq!(plain(Path::new(r"\\.\C:\x")), Path::new(r"\\.\C:\x"));
}

#[test]
fn the_prefix_is_taken_in_any_case() {
    // `scan::plain` and `exclude::split_path` took `UNC` only in upper case.
    assert_eq!(
        plain(Path::new(r"\\?\unc\server\share")),
        Path::new(r"\\server\share")
    );
    assert_eq!(
        plain(Path::new(r"\\?\Unc\server\share")),
        Path::new(r"\\server\share")
    );
    assert_eq!(
        verbatim(Path::new(r"\\?\unc\server\share")),
        Path::new(r"\\?\unc\server\share")
    );
    assert_eq!(
        parts(OsStr::new(r"\\?\unc\server\share")),
        parts(OsStr::new(r"\\server\share"))
    );
}

#[test]
fn a_network_path_has_the_same_parts_with_and_without_the_prefix() {
    // `folders::parts` and `exclude::split_path` took `UNC` for a part.
    let expected = vec![folded("SERVER"), folded("SHARE"), folded("X")];
    assert_eq!(parts(OsStr::new(r"\\?\UNC\server\share\x")), expected);
    assert_eq!(parts(OsStr::new(r"\\server\share\x")), expected);
    assert_eq!(
        split(OsStr::new(r"\\?\UNC\server\share\x")),
        ["server", "share", "x"]
    );
}

#[test]
fn parts_leave_out_the_prefix_and_empty_parts() {
    let expected = vec![folded("C:"), folded("DATA"), folded("X")];
    assert_eq!(parts(OsStr::new(r"\\?\C:\Data\x")), expected);
    assert_eq!(parts(OsStr::new(r"c:\data\X\")), expected);
    assert_eq!(parts(OsStr::new(r"C:\\Data\\x")), expected);
    assert_eq!(split(OsStr::new(r"C:\Data\x\")), ["C:", "Data", "x"]);
    assert_eq!(parts(OsStr::new(r"C:\")), vec![folded("C:")]);
}

#[test]
fn a_name_that_is_not_valid_unicode_is_never_replaced() {
    // `exclude::split_path` turned an unpaired surrogate into U+FFFD, so two different
    // names became one.
    let a = with_unit("Da", 0xD800, "ta");
    let b = with_unit("Da", 0xDC00, "ta");
    assert_eq!(
        fold(&a).chars(),
        ['D' as u32, 'A' as u32, 0xD800, 'T' as u32, 'A' as u32]
    );
    assert_ne!(fold(&a), fold(&b));
    assert_ne!(fold(&a), folded("Da\u{FFFD}ta"));
    let mut path = OsString::from(r"C:\");
    path.push(&a);
    path.push(r"\x");
    assert_eq!(split(&path), [OsString::from("C:"), a.clone(), "x".into()]);
    assert_eq!(parts(&path)[1], fold(&a));
    assert!(eq_ignore_case_os(&a, &with_unit("dA", 0xD800, "TA")));
    assert!(!eq_ignore_case_os(&a, &b));
}

#[test]
fn a_character_outside_the_bmp_folds_to_one_item() {
    // So `?` in a pattern takes it whole.
    assert_eq!(folded("a😀c").chars(), ['A' as u32, 0x1F600, 'C' as u32]);
}

#[test]
fn names_fold_the_way_ntfs_compares_them() {
    assert_eq!(folded("Руслан"), folded("РУСЛАН"));
    assert_eq!(folded("straße"), folded("STRAßE"));
    // `ß` has no single upper-case character, so it stays as it is.
    assert_ne!(folded("straße"), folded("STRASSE"));
    assert!(eq_ignore_case("Профіль", "профІЛЬ"));
    assert!(!eq_ignore_case("a", "ab"));
    assert_eq!(upcase(u16::from(b'a')), u16::from(b'A'));
    assert_eq!(upcase(0x0434), 0x0414); // д → Д
    assert_eq!(upcase(0xD800), 0xD800);
}
