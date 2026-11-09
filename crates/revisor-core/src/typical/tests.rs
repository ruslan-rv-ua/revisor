use super::*;
use crate::testing::TempDir;

fn profile(folders: &[&Path]) -> Vec<ProfileFolder> {
    folders
        .iter()
        .map(|p| ProfileFolder::new(p.to_path_buf()))
        .collect()
}

fn offered_paths(offered: &[Offered]) -> Vec<PathBuf> {
    offered.iter().map(|o| o.path.clone()).collect()
}

fn paths(entries: &[ProfileFolder]) -> Vec<PathBuf> {
    entries.iter().map(|f| f.path.clone()).collect()
}

#[test]
fn every_typical_folder_has_a_plain_path_on_this_computer() {
    let found = on_this_computer();
    assert_eq!(found.len(), TypicalFolder::ALL.len(), "{found:?}");
    for (folder, path) in &found {
        assert!(path.is_absolute(), "{folder:?}: {}", path.display());
        assert!(!path.to_string_lossy().contains('%'), "{}", path.display());
    }
    let app_data = TypicalFolder::AppData.path().unwrap();
    assert!(app_data.ends_with("AppData"), "{}", app_data.display());
}

#[test]
fn the_typical_folders_of_this_computer_hold_one_another_nowhere() {
    let offered = offer(&[], &on_this_computer());
    // Program Files (x86) is missing only on a 32-bit Windows, where it is Program Files.
    assert!(offered.len() >= TypicalFolder::ALL.len() - 1, "{offered:?}");
    for (i, a) in offered.iter().enumerate() {
        let others: Vec<ProfileFolder> = offered
            .iter()
            .enumerate()
            .filter(|&(j, _)| j != i)
            .map(|(_, o)| o.as_profile_folder())
            .collect();
        assert_eq!(check_addition(&others, &a.location), Addition::New, "{a:?}");
    }
}

#[test]
fn folders_not_on_the_disk_are_not_offered() {
    let tmp = TempDir::new("typical-missing");
    let here = tmp.folder("Program Files");
    let missing = tmp.path().join("ProgramData");
    let offered = offer(
        &[],
        &[
            (TypicalFolder::ProgramFiles, here.clone()),
            (TypicalFolder::ProgramData, missing),
        ],
    );
    assert_eq!(offered_paths(&offered), [here]);
}

#[test]
fn folders_the_profile_has_or_covers_are_not_offered() {
    let tmp = TempDir::new("typical-covered");
    let program_files = tmp.folder("Program Files");
    let windows = tmp.folder("Windows");
    let desktop = tmp.folder(r"Users\me\Desktop");
    let users = tmp.path().join("Users");
    let candidates = [
        (TypicalFolder::ProgramFiles, program_files.clone()),
        (TypicalFolder::Windows, windows.clone()),
        (TypicalFolder::Desktop, desktop),
    ];
    // Program Files is in the profile, written in another case; Users covers the desktop.
    let upper = PathBuf::from(program_files.to_string_lossy().to_uppercase());
    let offered = offer(&profile(&[&upper, &users]), &candidates);
    assert_eq!(offered_paths(&offered), [windows]);
    // The whole drive covers them all.
    assert!(offer(&profile(&[tmp.path()]), &candidates).is_empty());
}

#[test]
fn a_place_is_offered_once() {
    let tmp = TempDir::new("typical-twice");
    let program_files = tmp.folder("Program Files");
    let offered = offer(
        &[],
        &[
            (TypicalFolder::ProgramFiles, program_files.clone()),
            (TypicalFolder::ProgramFilesX86, program_files.clone()),
        ],
    );
    assert_eq!(offered.len(), 1);
    assert_eq!(offered[0].folder, TypicalFolder::ProgramFiles);
}

#[test]
fn a_folder_inside_a_typical_one_does_not_hide_it() {
    let tmp = TempDir::new("typical-inside");
    let program_files = tmp.folder("Program Files");
    let foo = tmp.folder(r"Program Files\Foo");
    let offered = offer(
        &profile(&[&foo]),
        &[(TypicalFolder::ProgramFiles, program_files.clone())],
    );
    assert_eq!(offered_paths(&offered), [program_files]);
}

#[test]
fn chosen_folders_are_added_as_they_are_and_replace_the_folders_inside_them() {
    let tmp = TempDir::new("typical-add");
    let program_files = tmp.folder("Program Files");
    let windows = tmp.folder("Windows");
    let data = tmp.folder("Data");
    let foo = tmp.folder(r"Program Files\Foo");
    let bar = tmp.folder(r"Program Files\Bar");
    let mut entries = profile(&[&foo, &data, &bar]);
    let offered = offer(
        &entries,
        &[
            (TypicalFolder::ProgramFiles, program_files.clone()),
            (TypicalFolder::Windows, windows.clone()),
        ],
    );
    assert_eq!(offered.len(), 2);
    let first = add(&mut entries, &offered);
    assert_eq!(paths(&entries), [data, program_files, windows]);
    assert_eq!(first, Some(1));
    assert!(entries.iter().all(|f| f.location.is_some()));
}

#[test]
fn nothing_chosen_adds_nothing() {
    let tmp = TempDir::new("typical-none");
    let data = tmp.folder("Data");
    let mut entries = profile(&[&data]);
    assert_eq!(add(&mut entries, &[]), None);
    assert_eq!(paths(&entries), [data]);
}
