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

fn ours(folder: TypicalFolder, path: &Path) -> Typical {
    Typical::of_this_user(folder, path.to_path_buf())
}

fn record(sid: &str, path: &Path) -> ProfileRecord {
    ProfileRecord {
        sid: sid.to_owned(),
        path: path.to_path_buf(),
    }
}

#[test]
fn every_typical_folder_has_a_plain_path_on_this_computer() {
    let found = on_this_computer();
    assert_eq!(found.len(), TypicalFolder::ALL.len(), "{found:?}");
    for typical in &found {
        assert!(typical.path.is_absolute(), "{typical:?}");
        assert!(!typical.path.to_string_lossy().contains('%'), "{typical:?}");
        assert_eq!(typical.user, None);
    }
    let app_data = TypicalFolder::AppData.path().unwrap();
    assert!(app_data.ends_with("AppData"), "{}", app_data.display());
    let profile = TypicalFolder::Profile.path().unwrap();
    assert!(app_data.starts_with(&profile), "{}", profile.display());
}

#[test]
fn of_the_typical_folders_of_this_computer_only_the_user_folder_holds_others() {
    // TASK-43 #1: the user folder is offered with AppData and the desktop inside it.
    let offered = offer(&[], &on_this_computer());
    // Program Files (x86) is missing only on a 32-bit Windows, where it is Program Files.
    assert!(offered.len() >= TypicalFolder::ALL.len() - 1, "{offered:?}");
    let user = offered
        .iter()
        .find(|o| o.folder == TypicalFolder::Profile)
        .expect("the user folder is offered");
    assert!(
        offered.iter().any(|o| o.folder == TypicalFolder::AppData),
        "{offered:?}"
    );
    for (i, a) in offered.iter().enumerate() {
        for (j, b) in offered.iter().enumerate() {
            let holds = i != j && b.location.parts.starts_with(&a.location.parts);
            assert!(
                !holds || a.folder == TypicalFolder::Profile,
                "{a:?} holds {b:?}"
            );
        }
    }
    assert!(
        user.path
            .ends_with(TypicalFolder::Profile.path().unwrap().file_name().unwrap()),
        "{user:?}"
    );
}

#[test]
fn folders_inside_another_typical_folder_are_offered_too() {
    // TASK-43 #1.
    let tmp = TempDir::new("typical-nested");
    let user = tmp.folder(r"Users\me");
    let app_data = tmp.folder(r"Users\me\AppData");
    let desktop = tmp.folder(r"Users\me\Desktop");
    let offered = offer(
        &[],
        &[
            ours(TypicalFolder::Profile, &user),
            ours(TypicalFolder::AppData, &app_data),
            ours(TypicalFolder::Desktop, &desktop),
        ],
    );
    assert_eq!(offered_paths(&offered), [user, app_data, desktop]);
}

#[test]
fn the_user_folder_windows_and_other_users_are_unchecked_at_first() {
    // TASK-43 #2.
    let tmp = TempDir::new("typical-checked");
    let mut candidates: Vec<Typical> = TypicalFolder::ALL
        .into_iter()
        .map(|folder| ours(folder, &tmp.folder(&format!("{folder:?}"))))
        .collect();
    let other = tmp.folder("other");
    candidates.push(Typical {
        folder: TypicalFolder::AppData,
        user: Some("other".to_owned()),
        path: other,
    });
    let checked: Vec<(TypicalFolder, bool)> = offer(&[], &candidates)
        .iter()
        .map(|o| (o.folder, o.checked_at_first()))
        .collect();
    assert_eq!(
        checked,
        [
            (TypicalFolder::ProgramFiles, true),
            (TypicalFolder::ProgramFilesX86, true),
            (TypicalFolder::ProgramData, true),
            (TypicalFolder::Windows, false),
            (TypicalFolder::PublicDesktop, true),
            (TypicalFolder::Profile, false),
            (TypicalFolder::AppData, true),
            (TypicalFolder::Desktop, true),
            (TypicalFolder::AppData, false),
        ]
    );
}

#[test]
fn folders_not_on_the_disk_are_not_offered() {
    let tmp = TempDir::new("typical-missing");
    let here = tmp.folder("Program Files");
    let missing = tmp.path().join("ProgramData");
    let offered = offer(
        &[],
        &[
            ours(TypicalFolder::ProgramFiles, &here),
            ours(TypicalFolder::ProgramData, &missing),
        ],
    );
    assert_eq!(offered_paths(&offered), [here]);
}

#[test]
fn folders_the_profile_has_or_covers_are_not_offered() {
    // TASK-36; TASK-43 #4: the user folder in the profile hides AppData and the desktop.
    let tmp = TempDir::new("typical-covered");
    let program_files = tmp.folder("Program Files");
    let windows = tmp.folder("Windows");
    let user = tmp.folder(r"Users\me");
    let app_data = tmp.folder(r"Users\me\AppData");
    let desktop = tmp.folder(r"Users\me\Desktop");
    let users = tmp.path().join("Users");
    let candidates = [
        ours(TypicalFolder::ProgramFiles, &program_files),
        ours(TypicalFolder::Windows, &windows),
        ours(TypicalFolder::Profile, &user),
        ours(TypicalFolder::AppData, &app_data),
        ours(TypicalFolder::Desktop, &desktop),
    ];
    // Program Files is in the profile, written in another case; Users covers the user's.
    let upper = PathBuf::from(program_files.to_string_lossy().to_uppercase());
    let offered = offer(&profile(&[&upper, &users]), &candidates);
    assert_eq!(offered_paths(&offered), std::slice::from_ref(&windows));
    let offered = offer(&profile(&[&user]), &candidates);
    assert_eq!(offered_paths(&offered), [program_files, windows]);
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
            ours(TypicalFolder::ProgramFiles, &program_files),
            ours(TypicalFolder::ProgramFilesX86, &program_files),
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
        &[ours(TypicalFolder::ProgramFiles, &program_files)],
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
            ours(TypicalFolder::ProgramFiles, &program_files),
            ours(TypicalFolder::Windows, &windows),
        ],
    );
    assert_eq!(offered.len(), 2);
    let first = add(&mut entries, &offered);
    assert_eq!(paths(&entries), [data, program_files, windows]);
    assert_eq!(first, Some(1));
    assert!(entries.iter().all(|f| f.location.is_some()));
}

#[test]
fn the_user_folder_chosen_with_folders_inside_it_is_added_alone() {
    // TASK-43 #3, #4: in the order of the list and in the reverse one.
    let tmp = TempDir::new("typical-user");
    let user = tmp.folder(r"Users\me");
    let app_data = tmp.folder(r"Users\me\AppData");
    let desktop = tmp.folder(r"Users\me\Desktop");
    let documents = tmp.folder(r"Users\me\Documents");
    let data = tmp.folder("Data");
    let candidates = [
        ours(TypicalFolder::Profile, &user),
        ours(TypicalFolder::AppData, &app_data),
        ours(TypicalFolder::Desktop, &desktop),
    ];
    let mut entries = profile(&[&documents, &data]);
    let offered = offer(&entries, &candidates);
    assert_eq!(add(&mut entries, &offered), Some(1));
    assert_eq!(paths(&entries), [data.clone(), user.clone()]);

    let mut entries = profile(&[&data]);
    let mut reversed = offer(&entries, &candidates);
    reversed.reverse();
    assert_eq!(add(&mut entries, &reversed), Some(1));
    assert_eq!(paths(&entries), [data, user]);
}

#[test]
fn nothing_chosen_adds_nothing() {
    let tmp = TempDir::new("typical-none");
    let data = tmp.folder("Data");
    let mut entries = profile(&[&data]);
    assert_eq!(add(&mut entries, &[]), None);
    assert_eq!(paths(&entries), [data]);
}

#[test]
fn other_users_are_real_users_with_a_folder_on_the_disk_but_the_current_one() {
    // TASK-43 #7, #8.
    let tmp = TempDir::new("typical-users");
    let me = tmp.folder(r"Users\me");
    let olena = tmp.folder(r"Users\olena");
    let entra = tmp.folder(r"Users\AzureAD-Petro");
    let old = tmp.folder(r"Users\old.DOMAIN");
    let gone = tmp.path().join(r"Users\gone");
    let system = tmp.folder(r"Windows\system32\config\systemprofile");
    let records = [
        record("S-1-5-18", &system),
        record("S-1-5-19", &system),
        record("S-1-5-20", &system),
        record("S-1-5-21-1-2-3-1001", &olena),
        // The current user, written in another case.
        record(
            "S-1-5-21-1-2-3-1000",
            &PathBuf::from(me.to_string_lossy().to_uppercase()),
        ),
        record("S-1-12-1-4-5-6-7", &entra),
        record("S-1-5-21-1-2-3-1002", &gone),
        // A domain account Windows no longer knows still has its folder.
        record("S-1-5-21-9-8-7-1103", &old),
        // A copy Windows keeps of a broken profile, of a folder listed already.
        record("S-1-5-21-1-2-3-1001.bak", &olena),
        record("S-1-5-80-1-2", &olena),
    ];
    let found = other_users(&records, &me);
    let rows: Vec<(TypicalFolder, Option<&str>, PathBuf)> = found
        .iter()
        .map(|t| (t.folder, t.user.as_deref(), t.path.clone()))
        .collect();
    let three = |name, path: &Path| {
        [
            (TypicalFolder::Profile, Some(name), path.to_path_buf()),
            (TypicalFolder::AppData, Some(name), path.join("AppData")),
            (TypicalFolder::Desktop, Some(name), path.join("Desktop")),
        ]
    };
    let expected: Vec<_> = [
        three("AzureAD-Petro", &entra),
        three("old.DOMAIN", &old),
        three("olena", &olena),
    ]
    .concat();
    assert_eq!(rows, expected);
}

#[test]
fn only_a_folder_revisor_may_not_open_is_out_of_reach() {
    // TASK-43: a missing folder is not, nor a folder of the current user.
    let tmp = TempDir::new("typical-reach");
    let here = tmp.folder("olena");
    let other = |path: PathBuf| Typical {
        folder: TypicalFolder::Profile,
        user: Some("olena".to_owned()),
        path,
    };
    assert!(!others_out_of_reach(&[other(here.clone())]));
    assert!(!others_out_of_reach(&[other(tmp.path().join("gone"))]));
    assert!(!others_out_of_reach(&[ours(TypicalFolder::Profile, &here)]));
}

#[test]
fn a_user_sid_is_a_local_domain_or_entra_one() {
    assert!(is_user_sid("S-1-5-21-1004336348-1177238915-682003330-512"));
    assert!(is_user_sid("S-1-12-1-1-2-3-4"));
    assert!(!is_user_sid("S-1-5-18"));
    assert!(!is_user_sid("S-1-5-21-"));
    assert!(!is_user_sid("S-1-5-21-1-2-3-1001.bak"));
    assert!(!is_user_sid("S-1-5-21-1--2"));
}

#[test]
fn the_profile_list_of_this_computer_has_the_current_user() {
    let current = TypicalFolder::Profile.path().unwrap();
    let records = profile_list();
    assert!(
        records.iter().any(
            |r| path::eq_ignore_case_os(r.path.as_os_str(), current.as_os_str())
                && is_user_sid(&r.sid)
        ),
        "{records:?}"
    );
    assert!(
        records
            .iter()
            .all(|r| !r.path.to_string_lossy().contains('%'))
    );
}
