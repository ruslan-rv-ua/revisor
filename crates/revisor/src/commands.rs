//! The command table (ADR-0008): every command revisor has, with its place in the menus,
//! its keys and whether it can run now. The menu bar, the context menus, the key handler
//! and the «Keyboard shortcuts» window are all built from `COMMANDS`; the keys follow the
//! document «Клавіатура й меню».

use crate::i18n::Strings;
use crate::keys::{Key, Shortcut};
use crate::state::{AppState, ChangeType, SortColumn};

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
    CompareWithPrevious,
    CompareWith,
    CancelComparison,
    Details,
    ShowInExplorer,
    FilterByText,
    ShowAdded,
    ShowDeleted,
    ShowModified,
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

    /// The access key: the Latin letter, also in the Ukrainian interface.
    pub const fn access(self) -> u8 {
        match self {
            Self::File => b'F',
            Self::Snapshot => b'S',
            Self::Compare => b'C',
            Self::Difference => b'D',
            Self::Help => b'H',
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SubmenuId {
    SortBy,
}

impl SubmenuId {
    pub const fn access(self) -> u8 {
        match self {
            Self::SortBy => b'S',
        }
    }
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
    SnapshotNotRunning,
    ComparisonNotRunning,
    CompactionNotRunning,
    /// The command's own task has not been done yet.
    NotYet,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemKind {
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
    /// The access key: a Latin letter of the English name, also in Ukrainian.
    pub access: u8,
    /// Whether the label ends in "…": the command asks for more before it acts.
    pub ellipsis: bool,
    pub kind: ItemKind,
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
    /// The command's name, without access key or ellipsis.
    pub fn name(&self, s: &dyn Strings) -> &'static str {
        s.command(self.id)
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

/// For commands whose task has not been done: unavailable, except in the demo state.
fn not_yet(state: &AppState) -> Result<(), Reason> {
    if state.demo {
        Ok(())
    } else {
        Err(Reason::NotYet)
    }
}

fn has_profile(state: &AppState) -> Result<(), Reason> {
    state.profile.as_ref().map(|_| ()).ok_or(Reason::NoProfiles)
}

fn has_snapshots(state: &AppState) -> Result<(), Reason> {
    has_profile(state)?;
    if state.snapshots > 0 {
        Ok(())
    } else {
        Err(Reason::NoSnapshots)
    }
}

fn has_difference(state: &AppState) -> Result<(), Reason> {
    has_snapshots(state)?;
    if state.has_difference {
        Ok(())
    } else {
        Err(Reason::NoDifference)
    }
}

fn snapshot_running(state: &AppState) -> Result<(), Reason> {
    if state.snapshot_running {
        Ok(())
    } else {
        Err(Reason::SnapshotNotRunning)
    }
}

fn comparison_running(state: &AppState) -> Result<(), Reason> {
    if state.comparison_running {
        Ok(())
    } else {
        Err(Reason::ComparisonNotRunning)
    }
}

fn compaction_running(state: &AppState) -> Result<(), Reason> {
    if state.compaction_running {
        Ok(())
    } else {
        Err(Reason::CompactionNotRunning)
    }
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

/// The defaults of a plain menu item; each entry overrides what it needs.
const fn item(id: CommandId, place: Place, access: u8) -> Command {
    Command {
        id,
        place,
        access,
        ellipsis: false,
        kind: ItemKind::Normal,
        shortcut: None,
        scope: None,
        list_key: None,
        context: None,
        available: not_yet,
        checked: None,
    }
}

const fn shown_check(id: CommandId, access: u8, digit: u8) -> Command {
    Command {
        kind: ItemKind::Check,
        shortcut: Some(key(Key::Digit(digit)).ctrl()),
        available: has_difference,
        ..item(id, place(MenuId::Difference, 2), access)
    }
}

const fn sort_radio(
    id: CommandId,
    group: u8,
    access: u8,
    checked: fn(&AppState) -> bool,
) -> Command {
    Command {
        kind: ItemKind::Radio,
        available: has_difference,
        checked: Some(checked),
        ..item(id, sort_place(group), access)
    }
}

fn sorted_by(state: &AppState, column: SortColumn) -> bool {
    state.sort == column
}

pub static COMMANDS: &[Command] = &[
    // File
    Command {
        ellipsis: true,
        shortcut: Some(key(Key::Letter(b'N')).ctrl()),
        ..item(CommandId::NewProfile, place(MenuId::File, 0), b'N')
    },
    Command {
        ellipsis: true,
        available: has_profile,
        ..item(CommandId::EditProfile, place(MenuId::File, 0), b'E')
    },
    Command {
        ellipsis: true,
        available: has_profile,
        ..item(CommandId::DeleteProfile, place(MenuId::File, 0), b'D')
    },
    item(CommandId::Settings, place(MenuId::File, 1), b'S'),
    item(CommandId::RestartAsAdmin, place(MenuId::File, 1), b'R'),
    Command {
        shortcut: Some(key(Key::F(4)).alt()),
        available: always,
        ..item(CommandId::Exit, place(MenuId::File, 2), b'X')
    },
    // Snapshot
    Command {
        shortcut: Some(key(Key::F(5))),
        available: has_profile,
        ..item(CommandId::TakeSnapshot, place(MenuId::Snapshot, 0), b'T')
    },
    Command {
        shortcut: Some(key(Key::F(5)).ctrl()),
        available: has_snapshots,
        ..item(
            CommandId::SnapshotAndCompare,
            place(MenuId::Snapshot, 0),
            b'A',
        )
    },
    Command {
        ellipsis: true,
        shortcut: Some(key(Key::F(2))),
        context: Some(ContextPlace {
            pane: Pane::Snapshots,
            group: 1,
        }),
        available: has_snapshots,
        ..item(CommandId::Comment, place(MenuId::Snapshot, 1), b'C')
    },
    Command {
        shortcut: Some(key(Key::Delete)),
        scope: Some(Pane::Snapshots),
        context: Some(ContextPlace {
            pane: Pane::Snapshots,
            group: 2,
        }),
        available: has_snapshots,
        ..item(CommandId::DeleteSnapshots, place(MenuId::Snapshot, 1), b'D')
    },
    Command {
        shortcut: Some(key(Key::F(5)).shift()),
        available: snapshot_running,
        ..item(CommandId::CancelSnapshot, place(MenuId::Snapshot, 2), b'N')
    },
    Command {
        available: compaction_running,
        ..item(
            CommandId::CancelCompaction,
            place(MenuId::Snapshot, 2),
            b'O',
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
        available: has_snapshots,
        ..item(
            CommandId::CompareWithPrevious,
            place(MenuId::Compare, 0),
            b'C',
        )
    },
    Command {
        ellipsis: true,
        shortcut: Some(key(Key::F(7)).ctrl()),
        context: Some(ContextPlace {
            pane: Pane::Snapshots,
            group: 0,
        }),
        available: has_snapshots,
        ..item(CommandId::CompareWith, place(MenuId::Compare, 0), b'W')
    },
    Command {
        shortcut: Some(key(Key::F(7)).shift()),
        available: comparison_running,
        ..item(CommandId::CancelComparison, place(MenuId::Compare, 1), b'N')
    },
    // Difference
    Command {
        shortcut: Some(key(Key::Enter).alt()),
        list_key: Some((Pane::Difference, key(Key::Enter))),
        context: Some(ContextPlace {
            pane: Pane::Difference,
            group: 0,
        }),
        available: has_difference,
        ..item(CommandId::Details, place(MenuId::Difference, 0), b'D')
    },
    Command {
        shortcut: Some(key(Key::Enter).ctrl()),
        context: Some(ContextPlace {
            pane: Pane::Difference,
            group: 0,
        }),
        available: has_difference,
        ..item(
            CommandId::ShowInExplorer,
            place(MenuId::Difference, 0),
            b'E',
        )
    },
    Command {
        shortcut: Some(key(Key::Letter(b'F')).ctrl()),
        available: has_difference,
        ..item(CommandId::FilterByText, place(MenuId::Difference, 1), b'F')
    },
    Command {
        checked: Some(|s| s.shows(ChangeType::Added)),
        ..shown_check(CommandId::ShowAdded, b'A', 1)
    },
    Command {
        checked: Some(|s| s.shows(ChangeType::Deleted)),
        ..shown_check(CommandId::ShowDeleted, b'T', 2)
    },
    Command {
        checked: Some(|s| s.shows(ChangeType::Modified)),
        ..shown_check(CommandId::ShowModified, b'M', 3)
    },
    Command {
        checked: Some(|s| s.shows(ChangeType::Inaccessible)),
        ..shown_check(CommandId::ShowInaccessible, b'N', 4)
    },
    sort_radio(CommandId::SortByPath, 0, b'T', |s| {
        sorted_by(s, SortColumn::Path)
    }),
    sort_radio(CommandId::SortByName, 0, b'N', |s| {
        sorted_by(s, SortColumn::Name)
    }),
    sort_radio(CommandId::SortByChange, 0, b'C', |s| {
        sorted_by(s, SortColumn::Change)
    }),
    sort_radio(CommandId::SortByWhatChanged, 0, b'W', |s| {
        sorted_by(s, SortColumn::WhatChanged)
    }),
    sort_radio(CommandId::SortBySize, 0, b'S', |s| {
        sorted_by(s, SortColumn::Size)
    }),
    sort_radio(CommandId::SortByDateModified, 0, b'D', |s| {
        sorted_by(s, SortColumn::DateModified)
    }),
    sort_radio(CommandId::SortByFolder, 0, b'F', |s| {
        sorted_by(s, SortColumn::Folder)
    }),
    sort_radio(CommandId::SortAscending, 1, b'A', |s| !s.descending),
    sort_radio(CommandId::SortDescending, 1, b'E', |s| s.descending),
    // Help
    Command {
        shortcut: Some(key(Key::F(1))),
        available: always,
        ..item(CommandId::KeyboardShortcuts, place(MenuId::Help, 0), b'K')
    },
    Command {
        available: always,
        ..item(CommandId::About, place(MenuId::Help, 1), b'A')
    },
];

/// Checks a command table against the rules of the document «Клавіатура й меню»; each
/// problem found is one line of the result.
pub fn validate(table: &[Command], en: &dyn Strings) -> Vec<String> {
    let mut problems = Vec::new();

    // Shortcuts: allowed by the rules, and one meaning each in the whole main window.
    for (i, c) in table.iter().enumerate() {
        let Some(shortcut) = c.shortcut else { continue };
        let key = shortcut.name(en);
        if let Err(rule) = shortcut.check_rules() {
            problems.push(format!("{}: {key}: {rule}", c.name(en)));
        }
        for other in &table[i + 1..] {
            let clash = other.shortcut == Some(shortcut)
                || other.list_key.is_some_and(|(_, k)| k == shortcut);
            if clash {
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

    // Access keys: a Latin letter of the English name, unique on its menu level.
    let mut levels: Vec<(String, Vec<(u8, &str)>)> = Vec::new();
    let mut add = |level: String, access: u8, name: &'static str| match levels
        .iter_mut()
        .find(|(l, _)| *l == level)
    {
        Some((_, keys)) => keys.push((access, name)),
        None => levels.push((level, vec![(access, name)])),
    };
    for menu in MenuId::ALL {
        add("menu bar".into(), menu.access(), en.menu(menu));
    }
    let mut submenus = Vec::new();
    for c in table {
        let parent = en.menu(c.place.menu).to_string();
        match c.place.submenu {
            Some((sub, _)) => {
                if !submenus.contains(&sub) {
                    submenus.push(sub);
                    add(parent, sub.access(), en.submenu(sub));
                }
                add(en.submenu(sub).into(), c.access, c.name(en));
            }
            None => add(parent, c.access, c.name(en)),
        }
    }
    for (level, keys) in &levels {
        for (i, &(access, name)) in keys.iter().enumerate() {
            if !access.is_ascii_uppercase() {
                problems.push(format!(
                    "{level}: the access key of {name} is not a Latin letter"
                ));
                continue;
            }
            let letter = char::from(access);
            if !name.to_ascii_uppercase().contains(letter) {
                problems.push(format!("{level}: access key {letter} is not in {name}"));
            }
            if keys[..i].iter().any(|&(a, _)| a == access) {
                problems.push(format!("{level}: access key {letter} is used twice"));
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
    use crate::i18n::En;

    #[test]
    fn the_command_table_follows_the_rules() {
        assert_eq!(validate(COMMANDS, &En), Vec::<String>::new());
    }

    #[test]
    fn validation_catches_broken_tables() {
        let f5 = Some(key(Key::F(5)));
        let broken = [
            // Same shortcut twice.
            Command {
                shortcut: f5,
                ..item(CommandId::TakeSnapshot, place(MenuId::Snapshot, 0), b'T')
            },
            Command {
                shortcut: f5,
                ..item(CommandId::CancelSnapshot, place(MenuId::Snapshot, 0), b'N')
            },
            // A forbidden shortcut.
            Command {
                shortcut: Some(key(Key::Letter(b'X')).alt()),
                ..item(CommandId::Exit, place(MenuId::File, 0), b'X')
            },
            // An access key used twice in one menu, and one missing from the name.
            item(CommandId::Settings, place(MenuId::File, 0), b'X'),
            item(CommandId::About, place(MenuId::Help, 0), b'Q'),
            // A Cyrillic access key.
            item(CommandId::KeyboardShortcuts, place(MenuId::Help, 0), 0xD0),
            // A context menu whose Enter action is not first.
            Command {
                context: Some(ContextPlace {
                    pane: Pane::Difference,
                    group: 0,
                }),
                ..item(
                    CommandId::ShowInExplorer,
                    place(MenuId::Difference, 0),
                    b'E',
                )
            },
            Command {
                list_key: Some((Pane::Difference, key(Key::Enter))),
                context: Some(ContextPlace {
                    pane: Pane::Difference,
                    group: 0,
                }),
                ..item(CommandId::Details, place(MenuId::Difference, 0), b'D')
            },
        ];
        let problems = validate(&broken, &En);
        let mentions = |needle: &str| problems.iter().any(|p| p.contains(needle));
        assert!(mentions("F5"), "{problems:#?}");
        assert!(mentions("Alt+X"), "{problems:#?}");
        assert!(mentions("access key X"), "{problems:#?}");
        assert!(mentions("access key Q"), "{problems:#?}");
        assert!(mentions("not a Latin letter"), "{problems:#?}");
        assert!(mentions("Enter action"), "{problems:#?}");
        assert!(mentions("not in Settings"), "{problems:#?}");
        assert_eq!(problems.len(), 7, "{problems:#?}");
    }
}
