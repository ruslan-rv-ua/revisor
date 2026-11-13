//! The command table (ADR-0008): every command revisor has, with its place in the menus,
//! its keys and whether it can run now. The menu bar, the context menus, the key handler
//! and the «Keyboard shortcuts» window are all built from `COMMANDS`; the keys follow the
//! document «Клавіатура й меню».

use crate::access::plain;
use crate::i18n::Strings;
use crate::keys::{Key, Shortcut};
use crate::state::{AppState, ChangeType, DifferenceView, SortColumn};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CommandId {
    NewProfile,
    EditProfile,
    DeleteProfile,
    Settings,
    RestartAsAdmin,
    Exit,
    TakeSnapshot,
    SnapshotAndCompare,
    Comment,
    DeleteSnapshots,
    CancelSnapshot,
    CancelCompaction,
    CancelComment,
    CompareWithPrevious,
    CompareWith,
    CancelComparison,
    Details,
    ShowInExplorer,
    FilterByText,
    ShowAdded,
    ShowDeleted,
    ShowModified,
    ShowMoved,
    ShowInaccessible,
    SortByPath,
    SortByName,
    SortByChange,
    SortByWhatChanged,
    SortBySize,
    SortByDateModified,
    SortByFolder,
    SortAscending,
    SortDescending,
    ViewAsList,
    ViewAsTree,
    Export,
    CancelExport,
    KeyboardShortcuts,
    About,
}

/// A menu of the menu bar, in menu bar order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MenuId {
    File,
    Snapshot,
    Compare,
    Difference,
    Help,
}

impl MenuId {
    pub const ALL: [MenuId; 5] = [
        Self::File,
        Self::Snapshot,
        Self::Compare,
        Self::Difference,
        Self::Help,
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SubmenuId {
    SortBy,
}

/// One of the two parts of the main window, named after its main control: the snapshot
/// list and the difference list. F6 moves between them, and a key can be limited to one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pane {
    Snapshots,
    Difference,
}

/// Why a command cannot run now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    NoProfiles,
    NoSnapshots,
    NoDifference,
    /// A snapshot is being taken, and only one goes at a time.
    SnapshotRunning,
    /// A snapshot of the open profile is being taken, so the profile cannot change.
    SnapshotOfThisProfile,
    SnapshotNotRunning,
    ComparisonNotRunning,
    CompactionNotRunning,
    /// A comment waits for the database of its profile: only one goes at a time, and the
    /// profile cannot change meanwhile (TASK-24).
    CommentWaiting,
    CommentNotWaiting,
    /// A profile's database is being compacted, and only one compaction goes at a time
    /// (TASK-9).
    CompactionRunning,
    /// The open profile's database is being compacted, which closes it to everything
    /// (TASK-9).
    CompactionOfThisProfile,
    /// No snapshot is selected in the snapshot list, so there is nothing to delete.
    NoSelection,
    /// The current snapshot is the oldest: there is none before it to compare it with
    /// (TASK-11).
    NoPreviousSnapshot,
    /// The profile has one snapshot only: there is no other one to compare it with
    /// (TASK-11).
    NoOtherSnapshot,
    /// revisor already runs as administrator.
    AlreadyAdmin,
    /// The difference list shows no row, so there is no change to act on (TASK-14).
    NoChangeShown,
    /// The difference shows as a tree, whose nodes are always in the order of their names
    /// (TASK-33 #5).
    TreeSortedByName,
    /// The focus of the difference tree is on a node «Ще…», which stands for the nodes of
    /// a page not shown and is no file or folder (TASK-37 #7).
    MoreNode,
    /// The difference is being exported, and one export goes at a time (TASK-39).
    ExportRunning,
    ExportNotRunning,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandKind {
    Normal,
    /// A menu item with a check mark.
    Check,
    /// One of a group of items of which one is chosen; a separator ends the group.
    Radio,
}

/// Where a command stands in the menu bar. Items keep table order; a change of `group`
/// puts a separator between them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place {
    pub menu: MenuId,
    pub group: u8,
    /// The submenu the item is in, and its group there.
    pub submenu: Option<(SubmenuId, u8)>,
}

/// Where a command stands in a list's context menu; groups are ordered by number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContextPlace {
    pub pane: Pane,
    pub group: u8,
}

pub struct Command {
    pub id: CommandId,
    pub place: Place,
    /// Whether the label ends in "…": the command asks for more before it acts.
    pub ellipsis: bool,
    pub kind: CommandKind,
    pub shortcut: Option<Shortcut>,
    /// The pane the shortcut is limited to. Elsewhere the key goes to the focused control,
    /// and when that control has no use for it, revisor says where the key works.
    pub scope: Option<Pane>,
    /// A key that runs the command only in that pane's list and belongs to the control
    /// everywhere else (Enter). It is not shown in the menu.
    pub list_key: Option<(Pane, Shortcut)>,
    pub context: Option<ContextPlace>,
    pub available: fn(&AppState) -> Result<(), Reason>,
    /// For check and radio items: whether the item is marked now.
    pub checked: Option<fn(&AppState) -> bool>,
}

impl Command {
    /// The command's name as text, without access key or ellipsis.
    pub fn name(&self, s: &dyn Strings) -> String {
        plain(s.command(self.id))
    }

    /// The name as messages and the «Keyboard shortcuts» window say it: with the menu's
    /// ellipsis, so that «Порівняти з…» does not end on its preposition (TASK-17).
    pub fn spoken_name(&self, s: &dyn Strings) -> String {
        let mut name = self.name(s);
        if self.ellipsis {
            name.push('…');
        }
        name
    }
}

pub fn command(id: CommandId) -> &'static Command {
    COMMANDS
        .iter()
        .find(|c| c.id == id)
        .expect("every command id is in the table")
}

// Availability conditions.

fn always(_: &AppState) -> Result<(), Reason> {
    Ok(())
}

fn require(condition: bool, reason: Reason) -> Result<(), Reason> {
    if condition { Ok(()) } else { Err(reason) }
}

fn has_profile(state: &AppState) -> Result<(), Reason> {
    state.profile.as_ref().map(|_| ()).ok_or(Reason::NoProfiles)
}

/// The open profile's database is open to reading and writing: it is not being compacted
/// (TASK-9).
fn not_compacting(state: &AppState) -> Result<(), Reason> {
    require(
        !state.compacting_open_profile,
        Reason::CompactionOfThisProfile,
    )
}

/// No comment waits for the database of the open profile (TASK-24).
fn not_commenting(state: &AppState) -> Result<(), Reason> {
    require(!state.commenting_open_profile, Reason::CommentWaiting)
}

/// The open profile can be changed or deleted unless it is being snapshotted (TASK-8),
/// its database is being compacted (TASK-9) or a comment waits for it (TASK-24).
fn profile_free(state: &AppState) -> Result<(), Reason> {
    has_profile(state)?;
    require(
        !state.snapshotting_open_profile,
        Reason::SnapshotOfThisProfile,
    )?;
    not_compacting(state)?;
    not_commenting(state)
}

/// Snapshots of the open profile can be deleted: there are selected ones, the profile is
/// not being snapshotted, no comment waits for it (TASK-24), and no database is being
/// compacted, since the deletion starts a compaction and only one goes at a time
/// (TASK-9).
fn snapshots_deletable(state: &AppState) -> Result<(), Reason> {
    has_snapshots(state)?;
    require(
        !state.snapshotting_open_profile,
        Reason::SnapshotOfThisProfile,
    )?;
    not_compacting(state)?;
    not_commenting(state)?;
    require(!state.compaction_running, Reason::CompactionRunning)?;
    require(state.selected_snapshots > 0, Reason::NoSelection)
}

/// The current snapshot's comment can change: even while the profile is being
/// snapshotted, but not while its database is being compacted (TASK-9), nor while
/// another comment waits for a database, as one goes at a time (TASK-24).
fn comment_possible(state: &AppState) -> Result<(), Reason> {
    has_snapshots(state)?;
    not_compacting(state)?;
    require(!state.comment_waiting, Reason::CommentWaiting)
}

fn no_snapshot_running(state: &AppState) -> Result<(), Reason> {
    require(!state.snapshot_running, Reason::SnapshotRunning)
}

/// A snapshot of the open profile can be taken: there is a profile, its database is
/// open, and no snapshot is being taken, as only one goes at a time (TASK-8, TASK-9). So
/// can «Знімок + порівняння», also on a profile without snapshots (TASK-12 #3, #4).
fn snapshot_possible(state: &AppState) -> Result<(), Reason> {
    has_profile(state)?;
    no_snapshot_running(state)?;
    not_compacting(state)
}

fn not_elevated(state: &AppState) -> Result<(), Reason> {
    require(!state.elevated, Reason::AlreadyAdmin)
}

fn has_snapshots(state: &AppState) -> Result<(), Reason> {
    has_profile(state)?;
    require(state.snapshots > 0, Reason::NoSnapshots)
}

/// The difference list shows a difference, of whichever profile (TASK-13 #6): its filter
/// and its order can change.
fn difference_shown(state: &AppState) -> Result<(), Reason> {
    require(state.has_difference, Reason::NoDifference)
}

/// The difference list shows a change to act on (TASK-14): a row of a difference of
/// whichever profile, the one with the focus. The difference is in memory, so no profile
/// or snapshot is needed (TASK-13 #6). The tree shows the same rows (TASK-33).
fn change_shown(state: &AppState) -> Result<(), Reason> {
    difference_shown(state)?;
    require(state.difference_rows > 0, Reason::NoChangeShown)
}

/// The change to act on is an entry or a folder, which File Explorer can show: not a node
/// «Ще…» of the tree (TASK-37 #7).
fn entry_shown(state: &AppState) -> Result<(), Reason> {
    change_shown(state)?;
    require(!state.more_node, Reason::MoreNode)
}

/// The difference shows as a list, whose order can change: the tree's nodes are always
/// by name (TASK-33 #5).
fn list_sortable(state: &AppState) -> Result<(), Reason> {
    difference_shown(state)?;
    require(
        state.layout == DifferenceView::List,
        Reason::TreeSortedByName,
    )
}

/// The current snapshot can be compared (TASK-11): the profile has snapshots and its
/// database is open; a snapshot being taken is no obstacle (#16).
fn comparison_possible(state: &AppState) -> Result<(), Reason> {
    has_snapshots(state)?;
    not_compacting(state)
}

/// Quick compare: the current snapshot has one before it.
fn previous_comparable(state: &AppState) -> Result<(), Reason> {
    comparison_possible(state)?;
    require(
        state.current_snapshot.is_some_and(|row| row > 0),
        Reason::NoPreviousSnapshot,
    )
}

/// Compare with: the profile has another snapshot to choose.
fn other_comparable(state: &AppState) -> Result<(), Reason> {
    comparison_possible(state)?;
    require(state.snapshots > 1, Reason::NoOtherSnapshot)
}

fn snapshot_running(state: &AppState) -> Result<(), Reason> {
    require(state.snapshot_running, Reason::SnapshotNotRunning)
}

fn comparison_running(state: &AppState) -> Result<(), Reason> {
    require(state.comparison_running, Reason::ComparisonNotRunning)
}

fn compaction_running(state: &AppState) -> Result<(), Reason> {
    require(state.compaction_running, Reason::CompactionNotRunning)
}

fn comment_waiting(state: &AppState) -> Result<(), Reason> {
    require(state.comment_waiting, Reason::CommentNotWaiting)
}

/// The difference shown can be exported (TASK-39 #1): one export at a time.
fn exportable(state: &AppState) -> Result<(), Reason> {
    difference_shown(state)?;
    require(!state.export_running, Reason::ExportRunning)
}

fn export_running(state: &AppState) -> Result<(), Reason> {
    require(state.export_running, Reason::ExportNotRunning)
}

const fn place(menu: MenuId, group: u8) -> Place {
    Place {
        menu,
        group,
        submenu: None,
    }
}

const fn sort_place(group: u8) -> Place {
    Place {
        menu: MenuId::Difference,
        group: 3,
        submenu: Some((SubmenuId::SortBy, group)),
    }
}

const fn key(key: Key) -> Shortcut {
    Shortcut::new(key)
}

/// A plain menu item that can run when `available` says so; each entry overrides the
/// defaults it needs. Every command names its condition: none runs by default.
const fn item(
    id: CommandId,
    place: Place,
    available: fn(&AppState) -> Result<(), Reason>,
) -> Command {
    Command {
        id,
        place,
        ellipsis: false,
        kind: CommandKind::Normal,
        shortcut: None,
        scope: None,
        list_key: None,
        context: None,
        available,
        checked: None,
    }
}

const fn shown_check(id: CommandId, digit: u8) -> Command {
    Command {
        kind: CommandKind::Check,
        shortcut: Some(key(Key::Digit(digit)).ctrl()),
        ..item(id, place(MenuId::Difference, 1), difference_shown)
    }
}

const fn sort_radio(id: CommandId, group: u8, checked: fn(&AppState) -> bool) -> Command {
    Command {
        kind: CommandKind::Radio,
        checked: Some(checked),
        ..item(id, sort_place(group), list_sortable)
    }
}

fn sorted_by(state: &AppState, column: SortColumn) -> bool {
    state.sort.column == column
}

/// «Списком» or «Деревом» (TASK-33 #7): both items have Ctrl+T, which runs the one not
/// marked, so it switches the view.
const fn view_radio(id: CommandId, checked: fn(&AppState) -> bool) -> Command {
    Command {
        kind: CommandKind::Radio,
        shortcut: Some(key(Key::Letter(b'T')).ctrl()),
        checked: Some(checked),
        ..item(id, place(MenuId::Difference, 2), difference_shown)
    }
}

pub static COMMANDS: &[Command] = &[
    // File
    Command {
        ellipsis: true,
        shortcut: Some(key(Key::Letter(b'N')).ctrl()),
        ..item(CommandId::NewProfile, place(MenuId::File, 0), always)
    },
    Command {
        ellipsis: true,
        ..item(CommandId::EditProfile, place(MenuId::File, 0), profile_free)
    },
    Command {
        ellipsis: true,
        ..item(
            CommandId::DeleteProfile,
            place(MenuId::File, 0),
            profile_free,
        )
    },
    Command {
        ..item(CommandId::Settings, place(MenuId::File, 1), always)
    },
    Command {
        ..item(
            CommandId::RestartAsAdmin,
            place(MenuId::File, 1),
            not_elevated,
        )
    },
    Command {
        shortcut: Some(key(Key::F(4)).alt()),
        ..item(CommandId::Exit, place(MenuId::File, 2), always)
    },
    // Snapshot
    Command {
        shortcut: Some(key(Key::F(5))),
        ..item(
            CommandId::TakeSnapshot,
            place(MenuId::Snapshot, 0),
            snapshot_possible,
        )
    },
    Command {
        shortcut: Some(key(Key::F(5)).ctrl()),
        ..item(
            CommandId::SnapshotAndCompare,
            place(MenuId::Snapshot, 0),
            snapshot_possible,
        )
    },
    Command {
        ellipsis: true,
        shortcut: Some(key(Key::F(2))),
        context: Some(ContextPlace {
            pane: Pane::Snapshots,
            group: 1,
        }),
        ..item(
            CommandId::Comment,
            place(MenuId::Snapshot, 1),
            comment_possible,
        )
    },
    Command {
        shortcut: Some(key(Key::Delete)),
        scope: Some(Pane::Snapshots),
        context: Some(ContextPlace {
            pane: Pane::Snapshots,
            group: 2,
        }),
        ..item(
            CommandId::DeleteSnapshots,
            place(MenuId::Snapshot, 1),
            snapshots_deletable,
        )
    },
    Command {
        shortcut: Some(key(Key::F(5)).shift()),
        ..item(
            CommandId::CancelSnapshot,
            place(MenuId::Snapshot, 2),
            snapshot_running,
        )
    },
    Command {
        ..item(
            CommandId::CancelCompaction,
            place(MenuId::Snapshot, 2),
            compaction_running,
        )
    },
    Command {
        shortcut: Some(key(Key::F(2)).shift()),
        ..item(
            CommandId::CancelComment,
            place(MenuId::Snapshot, 2),
            comment_waiting,
        )
    },
    // Compare
    Command {
        shortcut: Some(key(Key::F(7))),
        list_key: Some((Pane::Snapshots, key(Key::Enter))),
        context: Some(ContextPlace {
            pane: Pane::Snapshots,
            group: 0,
        }),
        ..item(
            CommandId::CompareWithPrevious,
            place(MenuId::Compare, 0),
            previous_comparable,
        )
    },
    Command {
        ellipsis: true,
        shortcut: Some(key(Key::F(7)).ctrl()),
        context: Some(ContextPlace {
            pane: Pane::Snapshots,
            group: 0,
        }),
        ..item(
            CommandId::CompareWith,
            place(MenuId::Compare, 0),
            other_comparable,
        )
    },
    Command {
        shortcut: Some(key(Key::F(7)).shift()),
        ..item(
            CommandId::CancelComparison,
            place(MenuId::Compare, 1),
            comparison_running,
        )
    },
    // Difference
    Command {
        shortcut: Some(key(Key::Enter).alt()),
        list_key: Some((Pane::Difference, key(Key::Enter))),
        context: Some(ContextPlace {
            pane: Pane::Difference,
            group: 0,
        }),
        ..item(
            CommandId::Details,
            place(MenuId::Difference, 0),
            change_shown,
        )
    },
    Command {
        shortcut: Some(key(Key::Enter).ctrl()),
        context: Some(ContextPlace {
            pane: Pane::Difference,
            group: 0,
        }),
        ..item(
            CommandId::ShowInExplorer,
            place(MenuId::Difference, 0),
            entry_shown,
        )
    },
    Command {
        shortcut: Some(key(Key::Letter(b'F')).ctrl()),
        ..item(
            CommandId::FilterByText,
            place(MenuId::Difference, 1),
            difference_shown,
        )
    },
    Command {
        checked: Some(|s| s.shows(ChangeType::Added)),
        ..shown_check(CommandId::ShowAdded, 1)
    },
    Command {
        checked: Some(|s| s.shows(ChangeType::Deleted)),
        ..shown_check(CommandId::ShowDeleted, 2)
    },
    Command {
        checked: Some(|s| s.shows(ChangeType::Modified)),
        ..shown_check(CommandId::ShowModified, 3)
    },
    Command {
        checked: Some(|s| s.shows(ChangeType::Moved)),
        ..shown_check(CommandId::ShowMoved, 4)
    },
    Command {
        checked: Some(|s| s.shows(ChangeType::Inaccessible)),
        ..shown_check(CommandId::ShowInaccessible, 5)
    },
    view_radio(CommandId::ViewAsList, |s| s.layout == DifferenceView::List),
    view_radio(CommandId::ViewAsTree, |s| s.layout == DifferenceView::Tree),
    sort_radio(CommandId::SortByPath, 0, |s| sorted_by(s, SortColumn::Path)),
    sort_radio(CommandId::SortByName, 0, |s| sorted_by(s, SortColumn::Name)),
    sort_radio(CommandId::SortByChange, 0, |s| {
        sorted_by(s, SortColumn::Change)
    }),
    sort_radio(CommandId::SortByWhatChanged, 0, |s| {
        sorted_by(s, SortColumn::WhatChanged)
    }),
    sort_radio(CommandId::SortBySize, 0, |s| sorted_by(s, SortColumn::Size)),
    sort_radio(CommandId::SortByDateModified, 0, |s| {
        sorted_by(s, SortColumn::DateModified)
    }),
    sort_radio(CommandId::SortByFolder, 0, |s| {
        sorted_by(s, SortColumn::Folder)
    }),
    sort_radio(CommandId::SortAscending, 1, |s| !s.sort.descending),
    sort_radio(CommandId::SortDescending, 1, |s| s.sort.descending),
    // A rare command, in a group of its own at the bottom, with no shortcut (TASK-39 #1).
    Command {
        ellipsis: true,
        ..item(CommandId::Export, place(MenuId::Difference, 4), exportable)
    },
    Command {
        ..item(
            CommandId::CancelExport,
            place(MenuId::Difference, 4),
            export_running,
        )
    },
    // Help
    Command {
        shortcut: Some(key(Key::F(1))),
        ..item(CommandId::KeyboardShortcuts, place(MenuId::Help, 0), always)
    },
    Command {
        ..item(CommandId::About, place(MenuId::Help, 1), always)
    },
];

/// Checks a command table against the rules of the document «Клавіатура й меню»; each
/// problem found is one line of the result. The test of the table runs it.
#[cfg_attr(not(test), expect(dead_code))]
pub fn validate(table: &[Command], en: &dyn Strings) -> Vec<String> {
    let mut problems = Vec::new();

    // Shortcuts: allowed by the rules, and one meaning each in the whole main window. The
    // two radio items of one group may share one, which switches between them (TASK-33).
    for (i, c) in table.iter().enumerate() {
        let Some(shortcut) = c.shortcut else { continue };
        let key = shortcut.name(en);
        if let Err(rule) = shortcut.check_rules() {
            problems.push(format!("{}: {key}: {rule}", c.name(en)));
        }
        let sharing = table
            .iter()
            .filter(|o| o.shortcut == Some(shortcut))
            .count();
        for (j, other) in table.iter().enumerate() {
            let switch = sharing == 2
                && c.kind == CommandKind::Radio
                && other.kind == CommandKind::Radio
                && c.place == other.place;
            let same_shortcut = j > i && other.shortcut == Some(shortcut) && !switch;
            let same_as_list_key = j != i && other.list_key.is_some_and(|(_, k)| k == shortcut);
            if same_shortcut || same_as_list_key {
                problems.push(format!(
                    "{key} is used by both {} and {}",
                    c.name(en),
                    other.name(en)
                ));
            }
        }
    }
    for (i, c) in table.iter().enumerate() {
        let Some((pane, key)) = c.list_key else {
            continue;
        };
        for other in &table[i + 1..] {
            if other.list_key == Some((pane, key)) {
                let key = key.name(en);
                problems.push(format!(
                    "{key} is used by both {} and {} in one list",
                    c.name(en),
                    other.name(en)
                ));
            }
        }
    }

    // Context menus: the list's Enter action comes first.
    for pane in [Pane::Snapshots, Pane::Difference] {
        let first = context_items(table, pane).next();
        let enter = table.iter().find(|c| {
            c.list_key.is_some_and(|(p, _)| p == pane)
                && c.context.is_some_and(|ctx| ctx.pane == pane)
        });
        if let (Some(first), Some(enter)) = (first, enter)
            && first.id != enter.id
        {
            problems.push(format!(
                "the Enter action {} is not first in its context menu",
                enter.name(en)
            ));
        }
    }

    problems
}

/// A pane's context menu items in menu order: by group, then by table order.
pub fn context_items(table: &[Command], pane: Pane) -> impl Iterator<Item = &Command> {
    let mut items: Vec<&Command> = table
        .iter()
        .filter(|c| c.context.is_some_and(|ctx| ctx.pane == pane))
        .collect();
    items.sort_by_key(|c| c.context.map(|ctx| ctx.group));
    items.into_iter()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::access::letter;
    use crate::i18n::En;

    #[test]
    fn the_command_table_follows_the_rules() {
        assert_eq!(validate(COMMANDS, &En), Vec::<String>::new());
    }

    /// The commands of the difference list (TASK-13, TASK-42): the filter — by text and by
    /// the five types — and the order.
    const DIFFERENCE_VIEW: [CommandId; 15] = [
        CommandId::FilterByText,
        CommandId::ShowAdded,
        CommandId::ShowDeleted,
        CommandId::ShowModified,
        CommandId::ShowMoved,
        CommandId::ShowInaccessible,
        CommandId::SortByPath,
        CommandId::SortByName,
        CommandId::SortByChange,
        CommandId::SortByWhatChanged,
        CommandId::SortBySize,
        CommandId::SortByDateModified,
        CommandId::SortByFolder,
        CommandId::SortAscending,
        CommandId::SortDescending,
    ];

    #[test]
    fn with_a_profile_at_rest_and_a_difference_every_command_runs_but_the_cancels() {
        // A profile with snapshots, one selected, and a difference of three rows, as a
        // standard user, with nothing running.
        let state = AppState {
            profile: Some("Дім".into()),
            snapshots: 2,
            selected_snapshots: 1,
            current_snapshot: Some(1),
            has_difference: true,
            difference_rows: 3,
            ..AppState::new()
        };
        for c in COMMANDS {
            let available = (c.available)(&state);
            match c.id {
                CommandId::CancelSnapshot => {
                    assert_eq!(available, Err(Reason::SnapshotNotRunning));
                }
                CommandId::CancelCompaction => {
                    assert_eq!(available, Err(Reason::CompactionNotRunning));
                }
                CommandId::CancelComparison => {
                    assert_eq!(available, Err(Reason::ComparisonNotRunning));
                }
                CommandId::CancelComment => {
                    assert_eq!(available, Err(Reason::CommentNotWaiting));
                }
                CommandId::CancelExport => {
                    assert_eq!(available, Err(Reason::ExportNotRunning));
                }
                _ => assert_eq!(available, Ok(()), "{:?}", c.id),
            }
        }
        let take = command(CommandId::TakeSnapshot).available;
        assert_eq!(take(&AppState::new()), Err(Reason::NoProfiles));
    }

    #[test]
    fn export_needs_a_difference_and_goes_one_at_a_time() {
        // TASK-39 #1.
        let export = command(CommandId::Export).available;
        let cancel = command(CommandId::CancelExport).available;
        assert_eq!(export(&AppState::new()), Err(Reason::NoDifference));
        let shown = AppState {
            has_difference: true,
            ..AppState::new()
        };
        assert_eq!(export(&shown), Ok(()));
        let exporting = AppState {
            export_running: true,
            ..shown
        };
        assert_eq!(export(&exporting), Err(Reason::ExportRunning));
        assert_eq!(cancel(&exporting), Ok(()));
        // A difference with no row shown can be exported: its report has the header.
        let c = command(CommandId::Export);
        assert!(c.ellipsis && c.shortcut.is_none());
        assert_eq!(c.place.menu, MenuId::Difference);
    }

    #[test]
    fn details_and_file_explorer_need_a_change_in_the_list_of_any_difference() {
        let available = |id, state: &AppState| (command(id).available)(state);
        // A difference shown while no profile is open, its profile deleted (TASK-13 #6):
        // the details are in memory, and File Explorer needs only the path.
        let rows = AppState {
            has_difference: true,
            difference_rows: 1,
            ..AppState::new()
        };
        // A difference whose list shows nothing: no changes, or the filter hid them all.
        let empty = AppState {
            difference_rows: 0,
            ..rows.clone()
        };
        for id in [CommandId::Details, CommandId::ShowInExplorer] {
            assert_eq!(available(id, &rows), Ok(()), "{id:?}");
            assert_eq!(available(id, &empty), Err(Reason::NoChangeShown), "{id:?}");
            assert_eq!(
                available(id, &AppState::new()),
                Err(Reason::NoDifference),
                "{id:?}"
            );
            let c = command(id);
            assert_eq!(c.place.menu, MenuId::Difference, "{id:?}");
            assert_eq!(
                c.context,
                Some(ContextPlace {
                    pane: Pane::Difference,
                    group: 0
                }),
                "{id:?}"
            );
            assert!(!c.ellipsis, "{id:?} shows a window without asking for more");
        }
        // The keys of the document «Клавіатура й меню» (#14).
        let details = command(CommandId::Details);
        assert_eq!(details.shortcut, Some(key(Key::Enter).alt()));
        assert_eq!(details.list_key, Some((Pane::Difference, key(Key::Enter))));
        assert_eq!(letter(En.command(details.id)), Some(b'D'));
        let explorer = command(CommandId::ShowInExplorer);
        assert_eq!(explorer.shortcut, Some(key(Key::Enter).ctrl()));
        assert_eq!(explorer.list_key, None);
        assert_eq!(letter(En.command(explorer.id)), Some(b'E'));
    }

    #[test]
    fn the_filter_and_the_order_need_a_difference_of_any_profile() {
        // A difference shown while no profile is open, its profile deleted (TASK-13 #6).
        let shown = AppState {
            has_difference: true,
            ..AppState::new()
        };
        for id in DIFFERENCE_VIEW {
            assert_eq!((command(id).available)(&shown), Ok(()), "{id:?}");
            assert_eq!(
                (command(id).available)(&AppState::new()),
                Err(Reason::NoDifference),
                "{id:?}"
            );
            assert_eq!(command(id).place.menu, MenuId::Difference, "{id:?}");
        }
        // The keys of the document «Клавіатура й меню» (#30): the types in the order of the
        // check boxes, renamed or moved entries fourth (TASK-42 #11).
        let keys = [
            (CommandId::FilterByText, key(Key::Letter(b'F')).ctrl(), b'F'),
            (CommandId::ShowAdded, key(Key::Digit(1)).ctrl(), b'A'),
            (CommandId::ShowDeleted, key(Key::Digit(2)).ctrl(), b'T'),
            (CommandId::ShowModified, key(Key::Digit(3)).ctrl(), b'M'),
            (CommandId::ShowMoved, key(Key::Digit(4)).ctrl(), b'V'),
            (CommandId::ShowInaccessible, key(Key::Digit(5)).ctrl(), b'N'),
        ];
        for (id, shortcut, access) in keys {
            assert_eq!(command(id).shortcut, Some(shortcut), "{id:?}");
            assert_eq!(letter(En.command(id)), Some(access), "{id:?}");
        }
        for id in &DIFFERENCE_VIEW[6..] {
            let c = command(*id);
            assert_eq!(c.place.submenu.map(|(sub, _)| sub), Some(SubmenuId::SortBy));
            assert_eq!(c.shortcut, None, "the submenu has no shortcuts");
        }
    }

    #[test]
    fn the_marks_follow_the_view() {
        let mut state = AppState {
            has_difference: true,
            ..AppState::new()
        };
        let checked = |id, state: &AppState| (command(id).checked.unwrap())(state);
        // Every type shown, by path, ascending (TASK-13 #28).
        for id in &DIFFERENCE_VIEW[1..6] {
            assert!(checked(*id, &state), "{id:?}");
        }
        assert!(checked(CommandId::SortByPath, &state));
        assert!(checked(CommandId::SortAscending, &state));
        assert!(!checked(CommandId::SortDescending, &state));
        state.shown_types[ChangeType::Deleted as usize] = false;
        state.sort.column = SortColumn::Size;
        state.sort.descending = true;
        assert!(!checked(CommandId::ShowDeleted, &state));
        assert!(checked(CommandId::ShowAdded, &state));
        assert!(checked(CommandId::SortBySize, &state));
        assert!(!checked(CommandId::SortByPath, &state));
        assert!(checked(CommandId::SortDescending, &state));
        assert!(!checked(CommandId::SortAscending, &state));
    }

    #[test]
    fn a_node_more_of_the_tree_turns_its_page_and_has_nothing_for_file_explorer() {
        // TASK-37 #5, #7: «Подробиці» turns the page; File Explorer says why it cannot.
        use crate::state::DifferenceView;
        let available = |id, state: &AppState| (command(id).available)(state);
        let more = AppState {
            has_difference: true,
            difference_rows: 5,
            layout: DifferenceView::Tree,
            more_node: true,
            ..AppState::new()
        };
        assert_eq!(available(CommandId::Details, &more), Ok(()));
        assert_eq!(
            available(CommandId::ShowInExplorer, &more),
            Err(Reason::MoreNode)
        );
        let node = AppState {
            more_node: false,
            ..more
        };
        assert_eq!(available(CommandId::ShowInExplorer, &node), Ok(()));
    }

    #[test]
    fn the_view_items_share_ctrl_t_and_the_tree_says_why_it_cannot_be_sorted() {
        // TASK-33 #5, #7, #10.
        let available = |id, state: &AppState| (command(id).available)(state);
        let checked = |id, state: &AppState| (command(id).checked.unwrap())(state);
        let list = AppState {
            has_difference: true,
            ..AppState::new()
        };
        let tree = AppState {
            layout: DifferenceView::Tree,
            ..list.clone()
        };
        for id in [CommandId::ViewAsList, CommandId::ViewAsTree] {
            let c = command(id);
            assert_eq!(c.kind, CommandKind::Radio, "{id:?}");
            assert_eq!(c.place, place(MenuId::Difference, 2), "{id:?}");
            assert_eq!(c.shortcut, Some(key(Key::Letter(b'T')).ctrl()), "{id:?}");
            assert_eq!(available(id, &list), Ok(()), "{id:?}");
            assert_eq!(available(id, &tree), Ok(()), "{id:?}");
            assert_eq!(
                available(id, &AppState::new()),
                Err(Reason::NoDifference),
                "{id:?}"
            );
        }
        assert_eq!(letter(En.command(CommandId::ViewAsList)), Some(b'L'));
        assert_eq!(letter(En.command(CommandId::ViewAsTree)), Some(b'R'));
        assert!(checked(CommandId::ViewAsList, &list));
        assert!(!checked(CommandId::ViewAsTree, &list));
        assert!(checked(CommandId::ViewAsTree, &tree));
        assert!(!checked(CommandId::ViewAsList, &tree));
        // The tree is always by name; the order of the list stays for it.
        for id in &DIFFERENCE_VIEW[6..] {
            assert_eq!(available(*id, &list), Ok(()), "{id:?}");
            assert_eq!(
                available(*id, &tree),
                Err(Reason::TreeSortedByName),
                "{id:?}"
            );
        }
        // The filter works in both.
        for id in &DIFFERENCE_VIEW[..6] {
            assert_eq!(available(*id, &tree), Ok(()), "{id:?}");
        }
        assert_eq!(
            available(CommandId::Details, &tree),
            Err(Reason::NoChangeShown)
        );
    }

    #[test]
    fn only_the_two_radio_items_of_one_group_may_share_a_shortcut() {
        let ctrl_t = Some(key(Key::Letter(b'T')).ctrl());
        let radio = |id| Command {
            kind: CommandKind::Radio,
            shortcut: ctrl_t,
            ..item(id, place(MenuId::Difference, 2), always)
        };
        assert_eq!(
            validate(
                &[radio(CommandId::ViewAsList), radio(CommandId::ViewAsTree)],
                &En
            ),
            Vec::<String>::new()
        );
        // A third, or one of another group or kind, is a clash.
        let three = [
            radio(CommandId::ViewAsList),
            radio(CommandId::ViewAsTree),
            radio(CommandId::SortByName),
        ];
        assert!(!validate(&three, &En).is_empty());
        let other_group = [
            radio(CommandId::ViewAsList),
            Command {
                place: place(MenuId::Difference, 3),
                ..radio(CommandId::ViewAsTree)
            },
        ];
        assert!(!validate(&other_group, &En).is_empty());
        let check = [
            radio(CommandId::ViewAsList),
            Command {
                kind: CommandKind::Check,
                ..radio(CommandId::ViewAsTree)
            },
        ];
        assert!(!validate(&check, &En).is_empty());
    }

    #[test]
    fn a_comment_waiting_for_the_database_holds_its_profile_as_a_snapshot_does() {
        let available = |id, state: &AppState| (command(id).available)(state);
        let at_rest = AppState {
            profile: Some("Дім".into()),
            snapshots: 2,
            selected_snapshots: 1,
            current_snapshot: Some(1),
            ..AppState::new()
        };
        // The comment waits for the open profile's database (TASK-24).
        let waiting_here = AppState {
            comment_waiting: true,
            commenting_open_profile: true,
            ..at_rest.clone()
        };
        for id in [
            CommandId::Comment,
            CommandId::DeleteSnapshots,
            CommandId::EditProfile,
            CommandId::DeleteProfile,
        ] {
            assert_eq!(
                available(id, &waiting_here),
                Err(Reason::CommentWaiting),
                "{id:?}"
            );
        }
        for id in [
            CommandId::CancelComment,
            CommandId::TakeSnapshot,
            CommandId::SnapshotAndCompare,
            CommandId::CompareWithPrevious,
            CommandId::CompareWith,
            CommandId::NewProfile,
        ] {
            assert_eq!(available(id, &waiting_here), Ok(()), "{id:?}");
        }
        // The comment waits for another profile's database: one comment at a time, the
        // open profile is free.
        let waiting_elsewhere = AppState {
            comment_waiting: true,
            ..at_rest.clone()
        };
        assert_eq!(
            available(CommandId::Comment, &waiting_elsewhere),
            Err(Reason::CommentWaiting)
        );
        for id in [
            CommandId::CancelComment,
            CommandId::DeleteSnapshots,
            CommandId::EditProfile,
            CommandId::DeleteProfile,
        ] {
            assert_eq!(available(id, &waiting_elsewhere), Ok(()), "{id:?}");
        }
        assert_eq!(
            available(CommandId::CancelComment, &at_rest),
            Err(Reason::CommentNotWaiting)
        );
    }

    #[test]
    fn cancel_comment_is_in_the_snapshot_menu_with_shift_f2() {
        let c = command(CommandId::CancelComment);
        assert_eq!(c.place.menu, MenuId::Snapshot);
        assert_eq!(letter(En.command(c.id)), Some(b'M'));
        assert_eq!(c.shortcut, Some(key(Key::F(2)).shift()));
    }

    #[test]
    fn deleting_snapshots_needs_a_selection_and_a_profile_at_rest() {
        let available = |id, state: &AppState| (command(id).available)(state);
        let selected = AppState {
            profile: Some("Дім".into()),
            snapshots: 2,
            selected_snapshots: 2,
            ..AppState::new()
        };
        assert_eq!(available(CommandId::DeleteSnapshots, &selected), Ok(()));
        let none_selected = AppState {
            selected_snapshots: 0,
            ..selected.clone()
        };
        assert_eq!(
            available(CommandId::DeleteSnapshots, &none_selected),
            Err(Reason::NoSelection)
        );
        // Comments change while the profile is being snapshotted; deletion waits.
        let snapshotting = AppState {
            snapshot_running: true,
            snapshotting_open_profile: true,
            ..selected.clone()
        };
        assert_eq!(available(CommandId::Comment, &snapshotting), Ok(()));
        assert_eq!(
            available(CommandId::DeleteSnapshots, &snapshotting),
            Err(Reason::SnapshotOfThisProfile)
        );
        let no_snapshots = AppState {
            snapshots: 0,
            selected_snapshots: 0,
            ..selected
        };
        for id in [CommandId::Comment, CommandId::DeleteSnapshots] {
            assert_eq!(
                available(id, &no_snapshots),
                Err(Reason::NoSnapshots),
                "{id:?}"
            );
        }
    }

    #[test]
    fn a_compaction_closes_the_open_profile_but_not_the_others() {
        let available = |id, state: &AppState| (command(id).available)(state);
        let compacting = AppState {
            profile: Some("Дім".into()),
            snapshots: 2,
            selected_snapshots: 1,
            compaction_running: true,
            compacting_open_profile: true,
            ..AppState::new()
        };
        assert_eq!(available(CommandId::CancelCompaction, &compacting), Ok(()));
        for id in [
            CommandId::TakeSnapshot,
            CommandId::Comment,
            CommandId::DeleteSnapshots,
            CommandId::EditProfile,
            CommandId::DeleteProfile,
            // The comparisons of TASK-11 inherit the check (#28).
            CommandId::CompareWithPrevious,
            CommandId::CompareWith,
            CommandId::SnapshotAndCompare,
        ] {
            assert_eq!(
                available(id, &compacting),
                Err(Reason::CompactionOfThisProfile),
                "{id:?}"
            );
        }
        // Another profile is open meanwhile: everything of it works, except a deletion,
        // which would start a second compaction.
        let other_open = AppState {
            compacting_open_profile: false,
            ..compacting
        };
        for id in [
            CommandId::TakeSnapshot,
            CommandId::SnapshotAndCompare,
            CommandId::Comment,
            CommandId::EditProfile,
            CommandId::DeleteProfile,
            CommandId::CancelCompaction,
        ] {
            assert_eq!(available(id, &other_open), Ok(()), "{id:?}");
        }
        assert_eq!(
            available(CommandId::DeleteSnapshots, &other_open),
            Err(Reason::CompactionRunning)
        );
    }

    #[test]
    fn one_snapshot_at_a_time_and_its_profile_stays_as_it_is() {
        let running = AppState {
            profile: Some("Дім".into()),
            snapshot_running: true,
            snapshotting_open_profile: true,
            ..AppState::new()
        };
        let available = |id, state: &AppState| (command(id).available)(state);
        for id in [CommandId::TakeSnapshot, CommandId::SnapshotAndCompare] {
            assert_eq!(
                available(id, &running),
                Err(Reason::SnapshotRunning),
                "{id:?}"
            );
        }
        assert_eq!(available(CommandId::CancelSnapshot, &running), Ok(()));
        for id in [CommandId::EditProfile, CommandId::DeleteProfile] {
            assert_eq!(
                available(id, &running),
                Err(Reason::SnapshotOfThisProfile),
                "{id:?}"
            );
        }
        // Another profile is open meanwhile: it can change, but no second snapshot starts.
        let other_open = AppState {
            snapshotting_open_profile: false,
            ..running.clone()
        };
        assert_eq!(available(CommandId::EditProfile, &other_open), Ok(()));
        for id in [CommandId::TakeSnapshot, CommandId::SnapshotAndCompare] {
            assert_eq!(
                available(id, &other_open),
                Err(Reason::SnapshotRunning),
                "{id:?}"
            );
        }
    }

    #[test]
    fn snapshot_and_compare_needs_what_a_snapshot_needs_and_no_snapshot_before() {
        let available =
            |state: &AppState| (command(CommandId::SnapshotAndCompare).available)(state);
        // In the Snapshot menu with its letter and Ctrl+F5 (TASK-12 #14).
        let c = command(CommandId::SnapshotAndCompare);
        assert_eq!(c.place.menu, MenuId::Snapshot);
        assert_eq!(letter(En.command(c.id)), Some(b'A'));
        assert_eq!(c.shortcut, Some(key(Key::F(5)).ctrl()));
        assert_eq!(available(&AppState::new()), Err(Reason::NoProfiles));
        // A profile without snapshots: the snapshot is taken all the same (#4).
        let empty = AppState {
            profile: Some("Дім".into()),
            ..AppState::new()
        };
        assert_eq!(available(&empty), Ok(()));
        let two = AppState {
            snapshots: 2,
            current_snapshot: Some(0),
            ..empty.clone()
        };
        assert_eq!(available(&two), Ok(()));
        // A comparison that runs gives way to it (TASK-11 #15).
        let comparing = AppState {
            comparison_running: true,
            ..two.clone()
        };
        assert_eq!(available(&comparing), Ok(()));
        // One snapshot at a time, and the database open (#3).
        let snapshotting = AppState {
            snapshot_running: true,
            ..two.clone()
        };
        assert_eq!(available(&snapshotting), Err(Reason::SnapshotRunning));
        let compacting = AppState {
            compaction_running: true,
            compacting_open_profile: true,
            ..two
        };
        assert_eq!(available(&compacting), Err(Reason::CompactionOfThisProfile));
    }

    #[test]
    fn comparisons_need_a_previous_or_another_snapshot_and_run_during_a_snapshot() {
        let available = |id, state: &AppState| (command(id).available)(state);
        let two = AppState {
            profile: Some("Дім".into()),
            snapshots: 2,
            current_snapshot: Some(1),
            ..AppState::new()
        };
        assert_eq!(available(CommandId::CompareWithPrevious, &two), Ok(()));
        assert_eq!(available(CommandId::CompareWith, &two), Ok(()));
        // On the oldest snapshot there is nothing before it, but still another one.
        let oldest = AppState {
            current_snapshot: Some(0),
            ..two.clone()
        };
        assert_eq!(
            available(CommandId::CompareWithPrevious, &oldest),
            Err(Reason::NoPreviousSnapshot)
        );
        assert_eq!(available(CommandId::CompareWith, &oldest), Ok(()));
        let one = AppState {
            snapshots: 1,
            current_snapshot: Some(0),
            ..two.clone()
        };
        assert_eq!(
            available(CommandId::CompareWithPrevious, &one),
            Err(Reason::NoPreviousSnapshot)
        );
        assert_eq!(
            available(CommandId::CompareWith, &one),
            Err(Reason::NoOtherSnapshot)
        );
        let none = AppState {
            snapshots: 0,
            current_snapshot: None,
            ..two.clone()
        };
        for id in [CommandId::CompareWithPrevious, CommandId::CompareWith] {
            assert_eq!(available(id, &none), Err(Reason::NoSnapshots), "{id:?}");
            assert_eq!(
                available(id, &AppState::new()),
                Err(Reason::NoProfiles),
                "{id:?}"
            );
        }
        // A snapshot being taken is no obstacle (#16).
        let snapshotting = AppState {
            snapshot_running: true,
            snapshotting_open_profile: true,
            ..two.clone()
        };
        assert_eq!(
            available(CommandId::CompareWithPrevious, &snapshotting),
            Ok(())
        );
        assert_eq!(available(CommandId::CompareWith, &snapshotting), Ok(()));
        assert_eq!(
            available(CommandId::CancelComparison, &two),
            Err(Reason::ComparisonNotRunning)
        );
        let comparing = AppState {
            comparison_running: true,
            ..two
        };
        assert_eq!(available(CommandId::CancelComparison, &comparing), Ok(()));
    }

    #[test]
    fn restarting_as_administrator_is_for_standard_users() {
        let restart = command(CommandId::RestartAsAdmin).available;
        assert_eq!(restart(&AppState::new()), Ok(()));
        let admin = AppState {
            elevated: true,
            ..AppState::new()
        };
        assert_eq!(restart(&admin), Err(Reason::AlreadyAdmin));
    }

    #[test]
    fn profile_commands_need_a_profile_except_new_profile() {
        let none = AppState::new();
        let one = AppState {
            profile: Some("Дім".into()),
            ..AppState::new()
        };
        assert_eq!((command(CommandId::NewProfile).available)(&none), Ok(()));
        for id in [CommandId::EditProfile, CommandId::DeleteProfile] {
            assert_eq!(
                (command(id).available)(&none),
                Err(Reason::NoProfiles),
                "{id:?}"
            );
            assert_eq!((command(id).available)(&one), Ok(()), "{id:?}");
        }
    }

    #[test]
    fn a_list_key_clashes_with_a_later_shortcut() {
        let enter = key(Key::Enter);
        let table = [
            Command {
                list_key: Some((Pane::Snapshots, enter.ctrl())),
                ..item(
                    CommandId::CompareWithPrevious,
                    place(MenuId::Compare, 0),
                    always,
                )
            },
            Command {
                shortcut: Some(enter.ctrl()),
                ..item(
                    CommandId::ShowInExplorer,
                    place(MenuId::Difference, 0),
                    always,
                )
            },
        ];
        let problems = validate(&table, &En);
        assert!(
            problems.iter().any(|p| p.contains("Ctrl+Enter")),
            "{problems:#?}"
        );
    }

    #[test]
    fn validation_catches_broken_tables() {
        let f5 = Some(key(Key::F(5)));
        let broken = [
            // Same shortcut twice.
            Command {
                shortcut: f5,
                ..item(CommandId::TakeSnapshot, place(MenuId::Snapshot, 0), always)
            },
            Command {
                shortcut: f5,
                ..item(
                    CommandId::CancelSnapshot,
                    place(MenuId::Snapshot, 0),
                    always,
                )
            },
            // A forbidden shortcut.
            Command {
                shortcut: Some(key(Key::Letter(b'X')).alt()),
                ..item(CommandId::Exit, place(MenuId::File, 0), always)
            },
            // A context menu whose Enter action is not first.
            Command {
                context: Some(ContextPlace {
                    pane: Pane::Difference,
                    group: 0,
                }),
                ..item(
                    CommandId::ShowInExplorer,
                    place(MenuId::Difference, 0),
                    always,
                )
            },
            Command {
                list_key: Some((Pane::Difference, key(Key::Enter))),
                context: Some(ContextPlace {
                    pane: Pane::Difference,
                    group: 0,
                }),
                ..item(CommandId::Details, place(MenuId::Difference, 0), always)
            },
        ];
        let problems = validate(&broken, &En);
        let mentions = |needle: &str| problems.iter().any(|p| p.contains(needle));
        assert!(mentions("F5"), "{problems:#?}");
        assert!(mentions("Alt+X"), "{problems:#?}");
        assert!(mentions("Enter action"), "{problems:#?}");
        assert_eq!(problems.len(), 3, "{problems:#?}");
    }
}
