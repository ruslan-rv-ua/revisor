use super::*;
use crate::job::Cancel;
use crate::store::{Scope, Snapshot, Store};
use crate::testing::TempDir;

/// The `profiles` subfolder of `tmp`, which does not exist at first.
fn profiles(tmp: &TempDir) -> PathBuf {
    tmp.path().join("profiles")
}

fn settings(name: &str) -> ProfileSettings {
    ProfileSettings {
        name: name.into(),
        scope: Scope {
            folders: vec![r"C:\Data".into()],
            exclusions: vec!["*.tmp".into()],
        },
    }
}

fn file_name(profile: &ProfileFile) -> String {
    profile
        .path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

/// The names of the profiles in `dir`, sorted.
fn names(dir: &Path) -> Vec<String> {
    let (profiles, unreadable) = list(dir).unwrap();
    assert!(unreadable.is_empty(), "{unreadable:?}");
    let mut names: Vec<String> = profiles.into_iter().map(|p| p.name).collect();
    names.sort();
    names
}

#[test]
fn a_new_profile_is_one_file_named_after_it() {
    let tmp = TempDir::new("profiles");
    let created = create(&profiles(&tmp), &settings("Дім")).unwrap();
    assert_eq!(created.path, profiles(&tmp).join("Дім.db"));
    assert_eq!(created.name, "Дім");
    assert_eq!(names(&profiles(&tmp)), ["Дім"]);
    assert_eq!(
        Store::open(&created.path).unwrap().settings().unwrap(),
        settings("Дім")
    );
}

#[test]
fn a_name_a_file_cannot_have_is_made_to_fit() {
    let tmp = TempDir::new("profiles");
    let dir = profiles(&tmp);
    let cases = [
        (r#"a/b:c*?"<>|\d"#, "a_b_c_______d.db"),
        ("con", "con_.db"),
        ("Lpt1.x", "Lpt1_.x.db"),
        ("Звіт. ", "Звіт.db"),
        ("...", "profile.db"),
    ];
    for (name, expected) in cases {
        let created = create(&dir, &settings(name)).unwrap();
        assert_eq!(file_name(&created), expected, "{name}");
        assert_eq!(created.name, name);
    }
    assert_eq!(names(&dir).len(), cases.len());
}

#[test]
fn a_profile_whose_file_name_is_taken_gets_a_number() {
    let tmp = TempDir::new("profiles");
    let dir = profiles(&tmp);
    assert_eq!(
        file_name(&create(&dir, &settings("a/b")).unwrap()),
        "a_b.db"
    );
    assert_eq!(
        file_name(&create(&dir, &settings("a:b")).unwrap()),
        "a_b (2).db"
    );
    assert_eq!(
        file_name(&create(&dir, &settings("a*b")).unwrap()),
        "a_b (3).db"
    );
}

#[test]
fn profile_names_do_not_repeat_regardless_of_case() {
    let tmp = TempDir::new("profiles");
    let dir = profiles(&tmp);
    let home = create(&dir, &settings("Дім")).unwrap();
    let work = create(&dir, &settings("Робота")).unwrap();

    assert!(matches!(
        create(&dir, &settings("дІМ")),
        Err(ProfileError::NameTaken)
    ));
    assert!(matches!(
        update(&work.path, &settings("ДІМ")),
        Err(ProfileError::NameTaken)
    ));
    // A profile may keep its own name, and change only its case.
    let renamed = update(&home.path, &settings("дім")).unwrap();
    assert_eq!(file_name(&renamed), "дім.db");
    assert_eq!(names(&dir), ["Робота", "дім"]);
}

#[test]
fn a_renamed_profile_moves_to_a_file_named_after_it() {
    let tmp = TempDir::new("profiles");
    let dir = profiles(&tmp);
    let home = create(&dir, &settings("Дім")).unwrap();
    let mut changed = settings("Робота");
    changed.scope.folders.push(r"D:\".into());

    let updated = update(&home.path, &changed).unwrap();

    assert_eq!(updated.path, dir.join("Робота.db"));
    assert!(!home.path.exists());
    assert_eq!(
        Store::open(&updated.path).unwrap().settings().unwrap(),
        changed
    );
    // Settings without a new name leave the file where it is.
    let same = update(&updated.path, &settings("Робота")).unwrap();
    assert_eq!(same.path, updated.path);
}

#[test]
fn each_profile_has_its_own_snapshots() {
    let tmp = TempDir::new("profiles");
    let dir = profiles(&tmp);
    let home = create(&dir, &settings("Дім")).unwrap();
    let work = create(&dir, &settings("Робота")).unwrap();
    let mut store = Store::open(&home.path).unwrap();
    store
        .save(
            &Snapshot::of_tree(crate::scan::synthetic(10)),
            &Cancel::new(),
        )
        .unwrap();
    drop(store);

    assert_eq!(snapshot_count(&home.path).unwrap(), 1);
    assert_eq!(snapshot_count(&work.path).unwrap(), 0);
    assert_eq!(settings_of(&work.path).unwrap(), settings("Робота"));
}

#[test]
fn a_deleted_profile_leaves_nothing_behind() {
    let tmp = TempDir::new("profiles");
    let dir = profiles(&tmp);
    let home = create(&dir, &settings("Дім")).unwrap();
    let journal = PathBuf::from(format!("{}-journal", home.path.display()));
    fs::write(&journal, b"left over").unwrap();

    delete(&home.path).unwrap();

    assert!(!home.path.exists());
    assert!(!journal.exists());
    assert!(names(&dir).is_empty());
}

#[test]
fn files_that_are_not_profiles_are_listed_apart() {
    let tmp = TempDir::new("profiles");
    let dir = profiles(&tmp);
    create(&dir, &settings("Дім")).unwrap();
    fs::write(dir.join("broken.db"), b"not a database").unwrap();
    fs::write(dir.join("notes.txt"), b"not a profile").unwrap();

    let (profiles, unreadable) = list(&dir).unwrap();

    assert_eq!(profiles.len(), 1);
    assert_eq!(unreadable.len(), 1);
    assert_eq!(unreadable[0].path, dir.join("broken.db"));
}

#[test]
fn a_profiles_snapshots_are_listed_oldest_first() {
    let tmp = TempDir::new("profiles");
    let home = create(&profiles(&tmp), &settings("Дім")).unwrap();
    assert!(snapshots_of(&home.path).unwrap().is_empty());
    let mut store = Store::open(&home.path).unwrap();
    let snapshot = Snapshot::of_tree(crate::scan::synthetic(10));
    let first = store.save(&snapshot, &Cancel::new()).unwrap();
    let second = store.save(&snapshot, &Cancel::new()).unwrap();
    drop(store);
    let ids: Vec<i64> = snapshots_of(&home.path)
        .unwrap()
        .iter()
        .map(|s| s.id)
        .collect();
    assert_eq!(ids, [first, second]);
}

#[test]
fn a_database_another_connection_holds_is_busy() {
    let tmp = TempDir::new("profiles");
    let home = create(&profiles(&tmp), &settings("Дім")).unwrap();
    let holder = rusqlite::Connection::open(&home.path).unwrap();
    holder.execute_batch("BEGIN EXCLUSIVE").unwrap();

    let error = Store::open_with_timeout(&home.path, std::time::Duration::from_millis(50))
        .and_then(|store| store.list())
        .unwrap_err();
    assert!(error.is_busy(), "{error:?}");
    assert!(!StoreError::NotAProfile.is_busy());
    assert!(!StoreError::Cancelled.is_busy());

    holder.execute_batch("COMMIT").unwrap();
    assert!(snapshots_of(&home.path).unwrap().is_empty());
}

#[test]
fn a_missing_folder_holds_no_profiles() {
    let tmp = TempDir::new("profiles");
    let (profiles, unreadable) = list(&profiles(&tmp)).unwrap();
    assert!(profiles.is_empty() && unreadable.is_empty());
}

#[test]
fn the_list_when_free_waits_for_a_profile_another_connection_holds() {
    let tmp = TempDir::new("profiles");
    let home = create(&profiles(&tmp), &settings("Дім")).unwrap();
    let holder = rusqlite::Connection::open(&home.path).unwrap();
    holder.execute_batch("BEGIN EXCLUSIVE").unwrap();
    let releasing = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(300));
        holder.execute_batch("COMMIT").unwrap();
    });
    let listing = std::time::Instant::now();
    let (found, unreadable) = list_when_free(&profiles(&tmp), &Cancel::new(), |_| false).unwrap();
    assert!(listing.elapsed() >= std::time::Duration::from_millis(200));
    assert_eq!(found, [home]);
    assert!(unreadable.is_empty());
    releasing.join().unwrap();
}

#[test]
fn the_list_when_free_does_not_wait_for_a_known_profile_nor_a_cancelled_one() {
    let tmp = TempDir::new("profiles");
    let home = create(&profiles(&tmp), &settings("Дім")).unwrap();
    let holder = rusqlite::Connection::open(&home.path).unwrap();
    holder.execute_batch("BEGIN EXCLUSIVE").unwrap();

    let (found, unreadable) =
        list_when_free(&profiles(&tmp), &Cancel::new(), |path| path == home.path).unwrap();
    assert!(found.is_empty());
    assert_eq!(unreadable.len(), 1);
    assert!(unreadable[0].error.is_busy(), "{:?}", unreadable[0].error);

    let cancel = Cancel::new();
    cancel.cancel();
    let (found, unreadable) = list_when_free(&profiles(&tmp), &cancel, |_| false).unwrap();
    assert!(found.is_empty());
    assert!(matches!(unreadable[0].error, StoreError::Cancelled));
    drop(holder);
}
