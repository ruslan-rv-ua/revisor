//! Every interface string of revisor in both languages, for proofreading with a screen
//! reader (TASK-17, ADR-0009): `cargo run -p revisor --example strings` writes it to a file.
//! The strings themselves, and the test that none of them is left out, are in revisor-ui
//! (`revisor_ui::i18n::catalog`, TASK-47); the window adds what only it makes of them: the
//! menu items with their keys, the menu bar, the submenus and the text of the window
//! «Keyboard shortcuts».

use revisor_ui::i18n::catalog::{self, Entry, entry};

use crate::commands::{COMMANDS, MenuId, SubmenuId};
use crate::menus;

/// The catalog as text: the strings of revisor-ui, then the labels of the window.
pub fn catalog() -> String {
    let mut entries = catalog::entries();
    entries.extend(window_entries());
    catalog::text(&entries)
}

/// The labels only the window makes of the strings.
fn window_entries() -> Vec<Entry> {
    let mut entries = Vec::new();
    for command in COMMANDS {
        entries.push(entry(
            format!("command.{:?} (menu item)", command.id),
            |s| menus::menu_label(s, command).replace('\t', "  [Tab]  "),
        ));
    }
    for menu in MenuId::ALL {
        entries.push(entry(format!("menu.{menu:?} (menu bar)"), |s| {
            menus::top_menu_label(s, menu)
        }));
    }
    for submenu in SubmenuId::ALL {
        entries.push(entry(format!("submenu.{submenu:?} (menu item)"), |s| {
            menus::submenu_label(s, submenu)
        }));
    }
    entries.push(entry(
        "shortcuts_text (the window «Keyboard shortcuts»)",
        |s| menus::shortcuts_text(s),
    ));
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_adds_its_labels_and_every_id_stays_unique() {
        let mut entries = catalog::entries();
        let strings = entries.len();
        entries.extend(window_entries());
        assert!(entries.len() > strings);
        let mut ids: Vec<&str> = entries.iter().map(|e| e.id.as_str()).collect();
        ids.sort_unstable();
        let count = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), count, "an id is used twice");

        let text = catalog();
        assert!(text.contains("\ncommand.NewProfile\n  en: New profile\n  uk: Новий профіль\n"));
        assert!(
            text.contains("\ncommand.NewProfile (menu item)\n  en: "),
            "{text:.300}"
        );
        assert!(text.contains("\nshortcuts_text (the window «Keyboard shortcuts»)\n"));
    }
}
