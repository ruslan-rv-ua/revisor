//! The difference shown in the main window, apart from the window (TASK-11): the rows of
//! the difference list — one per change and per inaccessible entry, with its kind and its
//! path taken from the trees of the difference on demand — and where the focus goes when
//! a new difference arrives, also that of «Знімок + порівняння» (TASK-12).

use std::path::PathBuf;

use revisor_core::compare::Difference;
use revisor_core::diff::{self, ChangeKind};
use revisor_core::settings::FocusAfterComparison;

/// What a row of the difference list is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Added,
    Deleted,
    Modified,
    /// An entry inaccessible in at least one snapshot, which is not a change (TASK-10).
    Inaccessible,
}

/// A row of the difference list: the folder of the difference it is in, and the entry's
/// indices in that folder's trees.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Row {
    folder: usize,
    kind: Kind,
    old: Option<u32>,
    new: Option<u32>,
}

/// The rows of the difference list, over the difference shown; empty until the first
/// comparison. The rows follow the folders of the difference, and in each the changes in
/// tree order, then the inaccessible entries.
#[derive(Debug, Default)]
pub struct Rows {
    difference: Option<Difference>,
    rows: Vec<Row>,
}

impl Rows {
    /// Shows `difference` in place of the one shown.
    pub fn replace(&mut self, difference: Difference) {
        let mut rows = Vec::new();
        for (folder, compared) in difference.folders.iter().enumerate() {
            rows.extend(compared.diff.changes.iter().map(|c| Row {
                folder,
                kind: match c.kind {
                    ChangeKind::Added => Kind::Added,
                    ChangeKind::Deleted => Kind::Deleted,
                    ChangeKind::Modified => Kind::Modified,
                },
                old: c.old,
                new: c.new,
            }));
            rows.extend(compared.diff.inaccessible.iter().map(|i| Row {
                folder,
                kind: Kind::Inaccessible,
                old: i.old,
                new: i.new,
            }));
        }
        self.rows = rows;
        self.difference = Some(difference);
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// What the row at `row` is.
    pub fn kind(&self, row: usize) -> Option<Kind> {
        self.rows.get(row).map(|r| r.kind)
    }

    /// The path of the entry of the row at `row`: in the earlier tree where it is there,
    /// else in the later one.
    pub fn path(&self, row: usize) -> Option<PathBuf> {
        let row = self.rows.get(row)?;
        let folder = &self.difference.as_ref()?.folders[row.folder];
        match (row.old, row.new) {
            (Some(a), _) => Some(diff::path(&folder.old, a)),
            (None, Some(b)) => Some(diff::path(&folder.new, b)),
            (None, None) => None,
        }
    }
}

/// Where the keyboard focus is when a comparison ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusPlace {
    /// Where it was when the comparison started.
    Unmoved,
    /// On the comparison's progress field or its Cancel button, which go away with it.
    Progress,
    /// In the difference list.
    DifferenceList,
    Elsewhere,
}

/// What the window does with a difference that has just arrived.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arrival {
    /// The difference takes the place of the one shown; the focus stays where it is, and
    /// the end is said without it.
    Replace,
    /// The difference takes the place of the one shown, and the keyboard focus moves to
    /// its first row: into a new list, which is announced as on coming to it from
    /// elsewhere, also when the focus was in the old one (#21).
    Focus,
    /// The profile of the snapshot of «Знімок + порівняння» is opened in place of another
    /// one, and the difference arrives as with `Focus` (TASK-12 #7).
    OpenProfileAndFocus,
}

/// What happens when a comparison ends under the setting «Фокус після порівняння», with
/// the focus at `focus` (TASK-11 #12, #13, #21).
pub fn arrival(setting: FocusAfterComparison, focus: FocusPlace) -> Arrival {
    match (setting, focus) {
        (_, FocusPlace::DifferenceList) => Arrival::Focus,
        (FocusAfterComparison::Always, _) => Arrival::Focus,
        (FocusAfterComparison::Never, _) => Arrival::Replace,
        (FocusAfterComparison::IfNotMoved, FocusPlace::Unmoved | FocusPlace::Progress) => {
            Arrival::Focus
        }
        (FocusAfterComparison::IfNotMoved, FocusPlace::Elsewhere) => Arrival::Replace,
    }
}

/// What happens when the comparison of «Знімок + порівняння» ends (TASK-12 #5, #7): as
/// after any comparison while the snapshot's profile is open, `of_open_profile`, with the
/// focus counted from the command's start. Another profile opened meanwhile has moved the
/// focus, so it goes to the difference only with «Завжди», which opens the snapshot's
/// profile again, or when it is in the difference list already (TASK-11 #21).
pub fn arrival_after_snapshot(
    setting: FocusAfterComparison,
    focus: FocusPlace,
    of_open_profile: bool,
) -> Arrival {
    match (setting, focus) {
        _ if of_open_profile => arrival(setting, focus),
        (FocusAfterComparison::Always, _) => Arrival::OpenProfileAndFocus,
        (_, FocusPlace::DifferenceList) => Arrival::Focus,
        _ => Arrival::Replace,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use revisor_core::compare::FolderDifference;
    use revisor_core::diff::{Change, Diff, Fields, Inaccessible};
    use revisor_core::scan::synthetic;
    use revisor_core::store::SnapshotInfo;
    use std::path::Path;

    fn info(id: i64) -> SnapshotInfo {
        SnapshotInfo {
            id,
            taken_at: id * 1000,
            comment: None,
        }
    }

    /// A difference of one folder: the first entries of a generated tree, with an entry
    /// added, one deleted, one modified and one inaccessible.
    fn difference() -> Difference {
        let old = synthetic(6);
        let new = synthetic(6);
        let change = |kind, old, new| Change {
            kind,
            old,
            new,
            fields: Fields::default(),
        };
        Difference {
            earlier: info(1),
            later: info(2),
            folders: vec![FolderDifference {
                path: PathBuf::from(&*old[0].name),
                diff: Diff {
                    changes: vec![
                        change(ChangeKind::Modified, Some(1), Some(1)),
                        change(ChangeKind::Deleted, Some(2), None),
                        change(ChangeKind::Added, None, Some(3)),
                    ],
                    inaccessible: vec![Inaccessible {
                        old: None,
                        new: Some(4),
                    }],
                },
                old,
                new,
            }],
            warnings: Vec::new(),
        }
    }

    #[test]
    fn the_rows_follow_the_changes_then_the_inaccessible_entries_with_their_paths() {
        let mut rows = Rows::default();
        assert!(rows.is_empty());
        assert_eq!(rows.kind(0), None);
        assert_eq!(rows.path(0), None);
        let d = difference();
        let (old, new) = (d.folders[0].old.clone(), d.folders[0].new.clone());
        rows.replace(d);
        assert_eq!(rows.len(), 4);
        let kinds: Vec<Kind> = (0..4).map(|i| rows.kind(i).unwrap()).collect();
        assert_eq!(
            kinds,
            [
                Kind::Modified,
                Kind::Deleted,
                Kind::Added,
                Kind::Inaccessible
            ]
        );
        // The path comes from the earlier tree where the entry is in it.
        assert_eq!(rows.path(0), Some(diff::path(&old, 1)));
        assert_eq!(rows.path(1), Some(diff::path(&old, 2)));
        assert_eq!(rows.path(2), Some(diff::path(&new, 3)));
        assert_eq!(rows.path(3), Some(diff::path(&new, 4)));
        assert!(rows.path(0).unwrap().starts_with(Path::new(&*old[0].name)));
        assert_eq!(rows.kind(4), None);
    }

    #[test]
    fn the_focus_moves_to_the_difference_when_the_user_waited_for_it() {
        use FocusAfterComparison::*;
        use FocusPlace::*;
        assert_eq!(arrival(IfNotMoved, Unmoved), Arrival::Focus);
        assert_eq!(arrival(IfNotMoved, Progress), Arrival::Focus);
        assert_eq!(arrival(IfNotMoved, Elsewhere), Arrival::Replace);
        for focus in [Unmoved, Progress, Elsewhere] {
            assert_eq!(arrival(Always, focus), Arrival::Focus);
            assert_eq!(arrival(Never, focus), Arrival::Replace);
        }
        // In the list already: the first row of the new difference, whatever the setting.
        for setting in FocusAfterComparison::ALL {
            assert_eq!(arrival(setting, DifferenceList), Arrival::Focus);
        }
    }

    #[test]
    fn the_difference_of_snapshot_and_compare_opens_its_profile_only_with_always() {
        use FocusAfterComparison::*;
        use FocusPlace::*;
        let places = [Unmoved, Progress, DifferenceList, Elsewhere];
        // Its profile open: as after any comparison (TASK-12 #5).
        for setting in FocusAfterComparison::ALL {
            for focus in places {
                assert_eq!(
                    arrival_after_snapshot(setting, focus, true),
                    arrival(setting, focus)
                );
            }
        }
        // Another profile open: the focus has moved (#7).
        for focus in places {
            assert_eq!(
                arrival_after_snapshot(Always, focus, false),
                Arrival::OpenProfileAndFocus
            );
        }
        for setting in [IfNotMoved, Never] {
            for focus in [Unmoved, Progress, Elsewhere] {
                assert_eq!(
                    arrival_after_snapshot(setting, focus, false),
                    Arrival::Replace
                );
            }
            assert_eq!(
                arrival_after_snapshot(setting, DifferenceList, false),
                Arrival::Focus
            );
        }
    }
}
