use super::*;
use crate::testing::{TempDir, junction};
use std::process::Command;

/// A drive letter `subst` gives to a folder for the length of a test.
struct SubstDrive(char);

impl SubstDrive {
    fn new(target: &Path) -> Self {
        // SAFETY: no arguments.
        let used = unsafe { windows_sys::Win32::Storage::FileSystem::GetLogicalDrives() };
        let letter = (b'J'..=b'Z')
            .rev()
            .find(|l| used & (1 << (l - b'A')) == 0)
            .expect("a free drive letter") as char;
        let status = Command::new("subst")
            .arg(format!("{letter}:"))
            .arg(target)
            .status()
            .unwrap();
        assert!(status.success());
        Self(letter)
    }

    fn root(&self) -> PathBuf {
        PathBuf::from(format!(r"{}:\", self.0))
    }
}

impl Drop for SubstDrive {
    fn drop(&mut self) {
        let _ = Command::new("subst")
            .arg(format!("{}:", self.0))
            .arg("/D")
            .status();
    }
}

fn located(path: &Path) -> Location {
    locate(path).unwrap_or_else(|e| panic!("{}: {e:?}", path.display()))
}

/// What adding `new` to a profile with the folders `existing` would do.
fn addition(existing: &[&Path], new: &Path) -> Addition {
    let existing: Vec<ProfileFolder> = existing
        .iter()
        .map(|p| ProfileFolder::new(p.to_path_buf()))
        .collect();
    assert!(existing.iter().all(|f| f.location.is_some()));
    check_addition(&existing, &located(new))
}

#[test]
fn a_folder_already_in_the_profile_is_covered_however_it_is_written() {
    let tmp = TempDir::new("folders");
    let data = tmp.folder("Data");
    let upper = PathBuf::from(data.to_string_lossy().to_uppercase());
    let slash = PathBuf::from(format!("{}\\", data.display()));
    assert_eq!(addition(&[&data], &upper), Addition::Covered(0));
    assert_eq!(addition(&[&data], &slash), Addition::Covered(0));
}

#[test]
fn a_folder_inside_a_profile_folder_is_covered() {
    let tmp = TempDir::new("folders");
    let other = tmp.folder("Other");
    let data = tmp.folder("Data");
    let inner = tmp.folder(r"Data\a\b");
    assert_eq!(addition(&[&other, &data], &inner), Addition::Covered(1));
}

#[test]
fn a_folder_whose_name_only_starts_the_same_is_not_inside() {
    let tmp = TempDir::new("folders");
    let data = tmp.folder("Data");
    let data2 = tmp.folder("Data2");
    assert_eq!(addition(&[&data], &data2), Addition::New);
    assert_eq!(addition(&[&data2], &data), Addition::New);
}

#[test]
fn a_folder_that_holds_profile_folders_would_replace_them() {
    let tmp = TempDir::new("folders");
    let x = tmp.folder(r"Data\x");
    let other = tmp.folder("Other");
    let z = tmp.folder(r"Data\y\z");
    let data = tmp.path().join("Data");
    assert_eq!(
        addition(&[&x, &other, &z], &data),
        Addition::Replaces(vec![0, 2])
    );
}

#[test]
fn a_folder_reached_through_a_junction_is_where_its_target_is() {
    let tmp = TempDir::new("folders");
    let data = tmp.folder("Data");
    let target = tmp.folder(r"Data\inner");
    let link = tmp.path().join("link");
    junction(&link, &target);
    assert_eq!(addition(&[&data], &link), Addition::Covered(0));
    assert_eq!(addition(&[&link], &target), Addition::Covered(0));
}

#[test]
fn a_folder_reached_through_a_subst_drive_is_where_its_target_is() {
    let tmp = TempDir::new("folders");
    let data = tmp.folder(r"Data\a");
    let drive = SubstDrive::new(tmp.path());
    assert_eq!(
        addition(&[&data], &drive.root().join(r"Data\a")),
        Addition::Covered(0)
    );
    assert_eq!(
        addition(&[&data], &drive.root()),
        Addition::Replaces(vec![0])
    );
}

#[test]
fn a_profile_folder_that_is_gone_from_the_disk_is_compared_by_its_path() {
    let tmp = TempDir::new("folders");
    let data = tmp.folder("Data");
    let existing = [ProfileFolder::new(data.join("gone"))];
    assert!(existing[0].location.is_none());
    assert_eq!(
        check_addition(&existing, &located(&data)),
        Addition::Replaces(vec![0])
    );
    assert_eq!(
        check_addition(&existing, &located(&tmp.folder("Other"))),
        Addition::New
    );
}

#[test]
fn a_folder_that_is_missing_cannot_be_located() {
    let tmp = TempDir::new("folders");
    assert!(matches!(
        locate(&tmp.path().join("missing")),
        Err(LocateError::Io(_))
    ));
}

#[test]
fn only_local_ntfs_volumes_are_supported() {
    assert!(volume_error(r"\\?\C:\Data", "NTFS").is_none());
    for fs in ["FAT32", "exFAT", "ReFS"] {
        assert!(
            matches!(volume_error(r"\\?\E:\", fs), Some(Unavailable::NotNtfs(name)) if name == fs),
            "{fs}"
        );
    }
    // A network share reports the file system of the server's disk.
    for path in [r"\\?\UNC\server\share\x", r"\Device\Mup\server\share"] {
        assert!(
            matches!(volume_error(path, "NTFS"), Some(Unavailable::Network)),
            "{path}"
        );
    }
    // The temp folder of this computer is on NTFS.
    located(&std::env::temp_dir());
}

#[test]
fn the_real_path_of_a_junction_is_its_target() {
    let tmp = TempDir::new("folders");
    let target = tmp.folder("target");
    let link = tmp.path().join("link");
    junction(&link, &target);
    let real = real_path(&link.join("..").join("link")).unwrap();
    assert_eq!(
        real.to_string_lossy().to_uppercase(),
        real_path(&target).unwrap().to_string_lossy().to_uppercase()
    );
    assert!(!real.to_string_lossy().starts_with(r"\\?\"));
    assert!(real_path(&tmp.path().join("missing")).is_err());
}
