use super::*;
use crate::store::{Scope, Store};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// A fresh folder under the system temp folder, removed on drop. Profiles go into its
/// subfolder `profiles`, which does not exist at first.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("revisor-profiles-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn profiles(&self) -> PathBuf {
        self.0.join("profiles")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
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
    let tmp = TempDir::new();
    let created = create(&tmp.profiles(), &settings("Дім")).unwrap();
    assert_eq!(created.path, tmp.profiles().join("Дім.db"));
    assert_eq!(created.name, "Дім");
    assert_eq!(names(&tmp.profiles()), ["Дім"]);
    assert_eq!(
        Store::open(&created.path).unwrap().settings().unwrap(),
        settings("Дім")
    );
}

#[test]
fn a_name_a_file_cannot_have_is_made_to_fit() {
    let tmp = TempDir::new();
    let dir = tmp.profiles();
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
    let tmp = TempDir::new();
    let dir = tmp.profiles();
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
    let tmp = TempDir::new();
    let dir = tmp.profiles();
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
    let tmp = TempDir::new();
    let dir = tmp.profiles();
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
    let tmp = TempDir::new();
    let dir = tmp.profiles();
    let home = create(&dir, &settings("Дім")).unwrap();
    let work = create(&dir, &settings("Робота")).unwrap();
    let mut store = Store::open(&home.path).unwrap();
    store
        .save(
            &settings("Дім").scope,
            None,
            &crate::scan::synthetic(10),
            &AtomicBool::new(false),
        )
        .unwrap();
    drop(store);

    assert_eq!(snapshot_count(&home.path).unwrap(), 1);
    assert_eq!(snapshot_count(&work.path).unwrap(), 0);
    assert_eq!(settings_of(&work.path).unwrap(), settings("Робота"));
}

#[test]
fn a_deleted_profile_leaves_nothing_behind() {
    let tmp = TempDir::new();
    let dir = tmp.profiles();
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
    let tmp = TempDir::new();
    let dir = tmp.profiles();
    create(&dir, &settings("Дім")).unwrap();
    fs::write(dir.join("broken.db"), b"not a database").unwrap();
    fs::write(dir.join("notes.txt"), b"not a profile").unwrap();

    let (profiles, unreadable) = list(&dir).unwrap();

    assert_eq!(profiles.len(), 1);
    assert_eq!(unreadable.len(), 1);
    assert_eq!(unreadable[0].path, dir.join("broken.db"));
}

#[test]
fn a_missing_folder_holds_no_profiles() {
    let tmp = TempDir::new();
    let (profiles, unreadable) = list(&tmp.profiles()).unwrap();
    assert!(profiles.is_empty() && unreadable.is_empty());
}
