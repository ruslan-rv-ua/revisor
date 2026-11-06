//! What the menus show, built from the command table: item labels with their access keys
//! and shortcuts, the menu bar layout, the context menus and the «Keyboard shortcuts» text.

use crate::commands::{
    COMMANDS, Command, CommandId, MenuId, Pane, SubmenuId, command, context_items,
};
use crate::i18n::{Lang, Strings};
use crate::keys::{Key, Shortcut};
use crate::state::AppState;

/// A label with its access key: "&File" in English, where the letter is underlined, and
/// "Файл (&F)" in Ukrainian, where the Latin letter follows in parentheses.
fn with_access(s: &dyn Strings, name: &str, access: u8, ellipsis: bool) -> String {
    let name = name.replace('&', "&&");
    let mut label = match s.lang() {
        Lang::En => {
            let letter = char::from(access).to_ascii_lowercase();
            let matches = |(i, c): &(usize, char)| {
                c.to_ascii_lowercase() == letter && (*i == 0 || name[..*i].ends_with(' '))
            };
            let at = name
                .char_indices()
                .find(matches)
                .or_else(|| {
                    name.char_indices()
                        .find(|(_, c)| c.to_ascii_lowercase() == letter)
                })
                .map_or(0, |(i, _)| i);
            format!("{}&{}", &name[..at], &name[at..])
        }
        Lang::Uk => format!("{name} (&{})", char::from(access)),
    };
    if ellipsis {
        label.push('…');
    }
    label
}

/// A label of the main window or a dialog: "P&rofile:" or "Профіль (&R):".
pub fn field_label(s: &dyn Strings, name: &str, access: u8) -> String {
    format!("{}:", with_access(s, name, access, false))
}

/// A button of a dialog: "&Add folder…" or "Додати теку (&A)…".
pub fn button_label(s: &dyn Strings, name: &str, access: u8, ellipsis: bool) -> String {
    with_access(s, name, access, ellipsis)
}

/// A menu bar item: name, access key, ellipsis and, after a tab, the shortcut.
pub fn menu_label(s: &dyn Strings, c: &Command) -> String {
    let label = context_label(s, c);
    match c.shortcut {
        Some(shortcut) => format!("{label}\t{}", shortcut.name(s)),
        None => label,
    }
}

/// A context menu item: like the menu bar's, but without the shortcut.
pub fn context_label(s: &dyn Strings, c: &Command) -> String {
    with_access(s, c.name(s), c.access, c.ellipsis)
}

pub fn top_menu_label(s: &dyn Strings, menu: MenuId) -> String {
    with_access(s, s.menu(menu), menu.access(), false)
}

pub fn submenu_label(s: &dyn Strings, submenu: SubmenuId) -> String {
    with_access(s, s.submenu(submenu), submenu.access(), false)
}

/// The position of the menu item whose access key is `letter`, in item texts as Win32
/// gives them ("&File", "Файл (&F)"). WM_MENUCHAR uses it to find an item by the key's
/// position when the layout gives a Cyrillic character.
pub fn find_access_key<S: AsRef<str>>(items: &[S], letter: u8) -> Option<usize> {
    let wanted = char::from(letter).to_ascii_uppercase();
    items.iter().position(|item| {
        let text = item.as_ref().split('\t').next().unwrap_or_default();
        let mut chars = text.chars();
        while let Some(c) = chars.next() {
            if c == '&' {
                match chars.next() {
                    Some('&') => continue,
                    Some(key) => return key.to_ascii_uppercase() == wanted,
                    None => return false,
                }
            }
        }
        false
    })
}

/// A list's context menu as it should open now: groups split by separators, without the
/// commands that cannot run, and the list's Enter action, which is shown bold, if present.
#[derive(Debug, PartialEq, Eq)]
pub struct ContextMenu {
    pub groups: Vec<Vec<CommandId>>,
    pub default: Option<CommandId>,
}

pub fn context_menu(pane: Pane, state: &AppState) -> ContextMenu {
    let mut groups: Vec<Vec<CommandId>> = Vec::new();
    let mut last_group = None;
    for c in context_items(COMMANDS, pane).filter(|c| (c.available)(state).is_ok()) {
        let group = c.context.map(|ctx| ctx.group);
        if last_group != Some(group) {
            groups.push(Vec::new());
            last_group = Some(group);
        }
        groups
            .last_mut()
            .expect("a group was just pushed")
            .push(c.id);
    }
    let default = groups
        .first()
        .and_then(|g| g.first())
        .copied()
        .filter(|&id| command(id).list_key.is_some_and(|(p, _)| p == pane));
    ContextMenu { groups, default }
}

/// The main window title: "<profile> — revisor", or "revisor" while there is no profile.
pub fn window_title(profile: Option<&str>) -> String {
    match profile {
        Some(profile) => format!("{profile} — revisor"),
        None => "revisor".into(),
    }
}

/// The text of the «Keyboard shortcuts» window: every command with a shortcut, by menu.
pub fn shortcuts_text(s: &dyn Strings) -> String {
    let mut text = String::new();
    for menu in MenuId::ALL {
        let mut lines = COMMANDS
            .iter()
            .filter(|c| c.place.menu == menu)
            .filter_map(|c| {
                let shortcut = c.shortcut?;
                let mut keys = shortcut.name(s);
                if let Some(pane) = c.scope {
                    keys = format!("{keys} {}", s.only_in(pane));
                }
                if let Some((pane, key)) = c.list_key {
                    keys = format!("{keys}, {}", s.in_list(&key.name(s), pane));
                }
                Some(format!("{} — {keys}", c.spoken_name(s)))
            });
        if let Some(first) = lines.next() {
            text.push_str(s.menu(menu));
            text.push('\n');
            for line in std::iter::once(first).chain(lines) {
                text.push_str(&line);
                text.push('\n');
            }
            text.push('\n');
        }
    }
    let f6 = Shortcut::new(Key::F(6));
    let context_keys = [
        Shortcut::new(Key::F(10)).shift(),
        Shortcut::new(Key::ContextMenu),
    ];
    text.push_str(s.navigation());
    text.push('\n');
    text.push_str(&format!(
        "{} — {}, {}\n",
        s.switch_panes(),
        f6.name(s),
        f6.shift().name(s)
    ));
    let [shift_f10, application_key] = context_keys.map(|k| k.name(s));
    text.push_str(&format!(
        "{} — {shift_f10}, {application_key}\n",
        s.context_menu()
    ));
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{En, Uk};

    #[test]
    fn context_menus_hold_only_available_commands_with_the_enter_action_first() {
        // A profile with snapshots and a difference whose list shows rows.
        let ready = AppState {
            profile: Some("Дім".into()),
            snapshots: 3,
            selected_snapshots: 1,
            current_snapshot: Some(2),
            has_difference: true,
            difference_rows: 2,
            ..AppState::new()
        };
        assert_eq!(
            context_menu(Pane::Snapshots, &ready),
            ContextMenu {
                groups: vec![
                    vec![CommandId::CompareWithPrevious, CommandId::CompareWith],
                    vec![CommandId::Comment],
                    vec![CommandId::DeleteSnapshots],
                ],
                default: Some(CommandId::CompareWithPrevious),
            }
        );
        assert_eq!(
            context_menu(Pane::Difference, &ready),
            ContextMenu {
                groups: vec![vec![CommandId::Details, CommandId::ShowInExplorer]],
                default: Some(CommandId::Details),
            }
        );
        let empty = ContextMenu {
            groups: vec![],
            default: None,
        };
        assert_eq!(context_menu(Pane::Snapshots, &AppState::new()), empty);
        // A difference whose list shows nothing, and no difference at all.
        let no_rows = AppState {
            difference_rows: 0,
            ..ready.clone()
        };
        assert_eq!(context_menu(Pane::Difference, &no_rows), empty);
        let no_difference = AppState {
            has_difference: false,
            ..ready
        };
        assert_eq!(context_menu(Pane::Difference, &no_difference), empty);
    }

    #[test]
    fn the_title_names_the_open_profile() {
        assert_eq!(window_title(None), "revisor");
        assert_eq!(window_title(Some("Дім")), "Дім — revisor");
    }

    #[test]
    fn the_shortcuts_text_lists_commands_with_shortcuts_by_menu() {
        let uk = shortcuts_text(&Uk);
        let lines: Vec<&str> = uk.lines().collect();
        for expected in [
            "Файл",
            "Новий профіль… — Ctrl+N",
            "Коментар… — F2",
            "Порівняти з… — Ctrl+F7",
            "Вихід — Alt+F4",
            "Зробити знімок — F5",
            "Видалити знімки — Delete (лише в переліку знімків)",
            "Порівняти з попереднім — F7, Enter у переліку знімків",
            "Подробиці — Alt+Enter, Enter у списку різниці",
            "Додані — Ctrl+1",
            "Гарячі клавіші — F1",
            "Перейти між переліком знімків і списком різниці — F6, Shift+F6",
            "Контекстне меню — Shift+F10, Клавіша контекстного меню",
        ] {
            assert!(lines.contains(&expected), "{expected:?} not in\n{uk}");
        }
        assert!(
            !uk.contains("Налаштування"),
            "commands without a shortcut are not listed"
        );
        assert!(
            !uk.contains("Сортувати"),
            "a menu without shortcuts is not listed"
        );
        assert!(
            shortcuts_text(&En)
                .lines()
                .any(|l| l == "Take snapshot — F5")
        );
    }

    #[test]
    fn english_labels_underline_a_letter_of_the_name() {
        assert_eq!(
            menu_label(&En, command(CommandId::NewProfile)),
            "&New profile…\tCtrl+N"
        );
        assert_eq!(menu_label(&En, command(CommandId::Exit)), "E&xit\tAlt+F4");
        // A word-initial letter is preferred over an earlier one inside a word.
        assert_eq!(
            menu_label(&En, command(CommandId::SnapshotAndCompare)),
            "Snapshot &and compare\tCtrl+F5"
        );
        assert_eq!(
            menu_label(&En, command(CommandId::EditProfile)),
            "&Edit profile…"
        );
        assert_eq!(top_menu_label(&En, MenuId::File), "&File");
        assert_eq!(submenu_label(&En, SubmenuId::SortBy), "&Sort by");
    }

    #[test]
    fn ukrainian_labels_put_the_latin_letter_in_parentheses() {
        assert_eq!(
            menu_label(&Uk, command(CommandId::NewProfile)),
            "Новий профіль (&N)…\tCtrl+N"
        );
        assert_eq!(
            menu_label(&Uk, command(CommandId::Exit)),
            "Вихід (&X)\tAlt+F4"
        );
        assert_eq!(
            menu_label(&Uk, command(CommandId::DeleteSnapshots)),
            "Видалити знімки (&D)\tDelete"
        );
        assert_eq!(top_menu_label(&Uk, MenuId::File), "Файл (&F)");
        assert_eq!(submenu_label(&Uk, SubmenuId::SortBy), "Сортувати за (&S)");
    }

    #[test]
    fn field_labels_end_in_a_colon() {
        assert_eq!(field_label(&En, "Profile", b'R'), "P&rofile:");
        assert_eq!(field_label(&Uk, "Профіль", b'R'), "Профіль (&R):");
    }

    #[test]
    fn context_labels_have_no_shortcut() {
        assert_eq!(context_label(&En, command(CommandId::Comment)), "&Comment…");
        assert_eq!(
            context_label(&Uk, command(CommandId::Comment)),
            "Коментар (&C)…"
        );
    }

    #[test]
    fn access_keys_are_found_by_latin_letter() {
        let items = [
            "Файл (&F)",
            "Знімок (&S)",
            "Порівняння (&C)",
            "Різниця (&D)",
            "Довідка (&H)",
        ];
        assert_eq!(find_access_key(&items, b'C'), Some(2));
        assert_eq!(find_access_key(&items, b'h'), Some(4));
        assert_eq!(find_access_key(&items, b'Q'), None);
        // "&&" is a literal ampersand, not an access key; the shortcut after a tab is not.
        let items = ["Save && &close", "Tab\tCtrl+&Q"];
        assert_eq!(find_access_key(&items, b'C'), Some(0));
        assert_eq!(find_access_key(&items, b'Q'), None);
    }
}
