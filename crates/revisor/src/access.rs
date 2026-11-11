//! Access keys (document «Клавіші доступу», ADR-0008, ADR-0009, TASK-30): where the
//! letter of a label lives, how a label is written in each language, and the one
//! dispatcher of Alt with a letter for every window.
//!
//! The letter is marked with "&" in the English name, in the English module of strings
//! ("&Add folder"); the Ukrainian module gives the words only, and the label gets the same
//! Latin letter in parentheses ("Додати теку (&A)"). The key works by its position, so the
//! letter is the same in both languages and in any layout.
//!
//! The dispatcher moves the focus at once, but presses a button only once Alt is up:
//! closing or opening a window on the key press would hand the release of Alt to the
//! window below, which takes it for Alt alone and goes into its window menu, where the
//! next keys are lost.

use std::cell::Cell;
use std::rc::Rc;

use wxdragon::prelude::*;

use crate::i18n::{En, Lang, Strings};
use crate::keys::Shortcut;
use crate::win32;

/// A name of a label as each language gives it: marked in English, words in Ukrainian.
pub type Name = fn(&dyn Strings) -> &'static str;

/// The access key marked with "&" in `name`, as an uppercase Latin letter; "&&" is a
/// literal ampersand.
pub fn letter(name: &str) -> Option<u8> {
    marks(name)
        .next()
        .filter(char::is_ascii_alphabetic)
        .map(|c| c.to_ascii_uppercase() as u8)
}

/// The characters marked with "&" in `name`, in order.
fn marks(name: &str) -> impl Iterator<Item = char> + '_ {
    let mut chars = name.chars();
    std::iter::from_fn(move || {
        while let Some(c) = chars.next() {
            if c == '&' {
                match chars.next() {
                    Some('&') => continue,
                    marked => return marked,
                }
            }
        }
        None
    })
}

/// `name` as text, say in a window title or a message: without its access key mark.
pub fn plain(name: &str) -> String {
    let mut text = String::with_capacity(name.len());
    let mut chars = name.chars();
    while let Some(c) = chars.next() {
        if c == '&' {
            if let Some(next) = chars.next() {
                text.push(next);
            }
        } else {
            text.push(c);
        }
    }
    text
}

/// The label of a name whose English is `english` and whose words in the language of `s`
/// are `words`: the English as it is, marked; the Ukrainian words with the letter of the
/// English in parentheses. A parameter in the name must hold no "&".
pub fn compose(s: &dyn Strings, english: &str, words: &str) -> String {
    match s.lang() {
        Lang::En => words.to_string(),
        Lang::Uk => {
            let words = words.replace('&', "&&");
            match letter(english) {
                Some(letter) => format!("{words} (&{})", char::from(letter)),
                None => words,
            }
        }
    }
}

/// A button or check box: "&Add folder", "Додати теку (&A)".
pub fn label(s: &dyn Strings, name: Name) -> String {
    compose(s, name(&En), name(s))
}

/// The label of a field: "&Name:", "Назва (&N):".
pub fn field_label(s: &dyn Strings, name: Name) -> String {
    format!("{}:", label(s, name))
}

/// What Alt with a target's letter does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    /// Puts the focus on a control: at once.
    Focus,
    /// Presses a button or a check box: once Alt is up.
    Press,
}

/// A labelled control of a window, named by `id`, with the English of its label.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target<T> {
    pub english: String,
    pub act: Act,
    pub id: T,
}

impl<T> Target<T> {
    pub fn new(english: impl Into<String>, act: Act, id: T) -> Self {
        Self {
            english: english.into(),
            act,
            id,
        }
    }

    /// The target of the label `name`.
    pub fn of(name: Name, act: Act, id: T) -> Self {
        Self::new(name(&En), act, id)
    }

    /// The access key.
    pub fn letter(&self) -> Option<u8> {
        letter(&self.english)
    }
}

/// The target whose access key `key` is, if any of `targets`.
pub fn find<T: Copy>(key: Shortcut, targets: &[Target<T>]) -> Option<(Act, T)> {
    let pressed = key.access_letter()?;
    targets
        .iter()
        .find(|t| t.letter() == Some(pressed))
        .map(|t| (t.act, t.id))
}

/// Keeps a press until Alt is up.
pub struct Dispatcher<T> {
    pending: Cell<Option<T>>,
}

impl<T: Copy> Default for Dispatcher<T> {
    fn default() -> Self {
        Self {
            pending: Cell::new(None),
        }
    }
}

impl<T: Copy> Dispatcher<T> {
    /// What to do now on Alt with the letter of target `id`: a focus moves at once, so
    /// `id` comes back; a press waits for [`Dispatcher::ready`].
    pub fn dispatch(&self, act: Act, id: T) -> Option<T> {
        match act {
            Act::Focus => Some(id),
            Act::Press => {
                self.pending.set(Some(id));
                None
            }
        }
    }

    /// The press waiting, once Alt is up; it is given once.
    pub fn ready(&self, alt_down: bool) -> Option<T> {
        if alt_down { None } else { self.pending.take() }
    }
}

/// Gives `dialog` its access keys: Alt with the letter of one of `targets` makes `act`
/// act on it, a focus at once, a press once Alt is up. The dialog's char hook and idle
/// handler are taken for it.
pub fn bind<T: Copy + 'static>(
    dialog: &Dialog,
    targets: Vec<Target<T>>,
    act: impl Fn(T) + 'static,
) {
    let dispatcher = Rc::new(Dispatcher::default());
    let act = Rc::new(act);
    let (waiting, on_key) = (Rc::clone(&dispatcher), Rc::clone(&act));
    dialog.on_char_hook(move |data| {
        if let WindowEventData::Keyboard(key) = data {
            let hit = find(Shortcut::from_event(&key), &targets);
            key.event.skip(hit.is_none());
            if let Some(id) = hit.and_then(|(how, id)| waiting.dispatch(how, id)) {
                on_key(id);
            }
        }
    });
    dialog.on_idle(move |data| {
        if let Some(id) = dispatcher.ready(win32::focus::alt_down()) {
            act(id);
        }
        if let WindowEventData::Idle(event) = data {
            event.event.skip(true);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{COMMANDS, MenuId, SubmenuId};
    use crate::i18n::Uk;
    use crate::keys::Key;

    #[test]
    fn the_letter_is_the_one_marked_in_the_english_name() {
        assert_eq!(letter("&Add folder"), Some(b'A'));
        assert_eq!(letter("Ca&ncel"), Some(b'N'));
        assert_eq!(letter("Save && &close"), Some(b'C'));
        assert_eq!(letter("Add folder"), None);
        assert_eq!(letter("Sizes &1"), None);
        assert_eq!(plain("Ca&ncel"), "Cancel");
        assert_eq!(plain("Save && &close"), "Save & close");
        assert_eq!(plain("Скасувати"), "Скасувати");
    }

    #[test]
    fn ukrainian_labels_take_the_english_letter_in_parentheses() {
        assert_eq!(label(&En, |s| s.add_folder()), "&Add folder");
        assert_eq!(label(&Uk, |s| s.add_folder()), "Додати теку (&A)");
        assert_eq!(field_label(&Uk, |s| s.profile()), "Профіль (&R):");
        assert_eq!(compose(&Uk, "A &b", "Слова & ще"), "Слова && ще (&B)");
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Id {
        Field,
        Button,
    }

    #[test]
    fn alt_with_a_letter_picks_its_target_and_nothing_else_does() {
        let targets = [
            Target::new("&Field", Act::Focus, Id::Field),
            Target::new("&Button", Act::Press, Id::Button),
        ];
        let alt = |letter| Shortcut::new(Key::Letter(letter)).alt();
        assert_eq!(find(alt(b'F'), &targets), Some((Act::Focus, Id::Field)));
        assert_eq!(find(alt(b'B'), &targets), Some((Act::Press, Id::Button)));
        assert_eq!(find(alt(b'Q'), &targets), None);
        // Alt with Ctrl or Shift, or a letter alone, is no access key.
        assert_eq!(find(alt(b'F').ctrl(), &targets), None);
        assert_eq!(find(alt(b'F').shift(), &targets), None);
        assert_eq!(find(Shortcut::new(Key::Letter(b'F')), &targets), None);
    }

    #[test]
    fn a_focus_moves_at_once_and_a_press_waits_for_alt_to_be_up() {
        let dispatcher = Dispatcher::default();
        assert_eq!(dispatcher.dispatch(Act::Focus, Id::Field), Some(Id::Field));
        assert_eq!(dispatcher.ready(false), None);
        assert_eq!(dispatcher.dispatch(Act::Press, Id::Button), None);
        assert_eq!(dispatcher.ready(true), None);
        assert_eq!(dispatcher.ready(true), None);
        assert_eq!(dispatcher.ready(false), Some(Id::Button));
        // Given once.
        assert_eq!(dispatcher.ready(false), None);
    }

    /// The problems with the labels of one window or menu, by their English names: a name
    /// without exactly one marked Latin letter, a letter used twice or among `taken`. The
    /// Ukrainian words need no check: a "&" in them is shown as it is.
    fn problems(place: &str, names: &[String], taken: &[u8]) -> Vec<String> {
        let mut problems = Vec::new();
        let mut letters: Vec<u8> = taken.to_vec();
        for name in names {
            match (marks(name).count(), letter(name)) {
                (1, Some(l)) if letters.contains(&l) => problems.push(format!(
                    "{place}: the letter {} of {name:?} is used twice",
                    char::from(l)
                )),
                (1, Some(l)) => letters.push(l),
                _ => problems.push(format!(
                    "{place}: {name:?} has no single marked Latin letter"
                )),
            }
        }
        problems
    }

    fn english<T>(targets: Vec<Target<T>>) -> Vec<String> {
        targets.into_iter().map(|t| t.english).collect()
    }

    #[test]
    fn the_letters_differ_within_each_window_and_menu() {
        let mut found = Vec::new();
        // The menu bar, each menu with its submenu, and the submenu.
        let bar: Vec<String> = MenuId::ALL.iter().map(|&m| En.menu(m).into()).collect();
        found.extend(problems("menu bar", &bar, &[]));
        for menu in MenuId::ALL {
            let mut items: Vec<String> = COMMANDS
                .iter()
                .filter(|c| c.place.menu == menu && c.place.submenu.is_none())
                .map(|c| En.command(c.id).into())
                .collect();
            if menu == MenuId::Difference {
                items.push(En.submenu(SubmenuId::SortBy).into());
            }
            found.extend(problems(&format!("menu {menu:?}"), &items, &[]));
        }
        let sort: Vec<String> = COMMANDS
            .iter()
            .filter(|c| c.place.submenu.is_some())
            .map(|c| En.command(c.id).into())
            .collect();
        found.extend(problems("submenu SortBy", &sort, &[]));
        // The main window: its labels take no letter of the menu bar, or the menu would
        // no longer open while the focus is in the panel.
        let bar_letters: Vec<u8> = bar.iter().filter_map(|name| letter(name)).collect();
        let main = english(crate::main_window::targets(None));
        found.extend(problems("main window", &main, &bar_letters));
        use crate::jobs::Kind;
        for kind in [
            Kind::Snapshot,
            Kind::Comparison,
            Kind::Comment,
            Kind::Compaction,
        ] {
            let progress = english(crate::main_window::targets(Some(kind)));
            found.extend(problems(
                &format!("main window, {kind:?}"),
                &progress,
                &bar_letters,
            ));
        }
        // The dialogs.
        let profile = english(crate::profile_window::targets());
        found.extend(problems("profile window", &profile, &[]));
        let settings = english(crate::settings_window::targets());
        found.extend(problems("settings window", &settings, &[]));
        let snapshot = "07.11.2026 10:00";
        found.extend(problems(
            "«Порівняти з…»",
            &[En.compare_with_label(snapshot)],
            &[],
        ));
        found.extend(problems("«Коментар»", &[En.comment_of(snapshot)], &[]));
        // The questions: each has its action and Cancel, which has no letter, or, after
        // the language changed, two buttons of their own (TASK-16).
        let questions: [&[Name]; 6] = [
            &[|s| s.delete()],
            &[|s| s.restart()],
            &[|s| s.close()],
            &[|s| s.open()],
            &[|s| s.replace()],
            &[|s| s.restart_now(), |s| s.later()],
        ];
        for buttons in questions {
            let names: Vec<String> = buttons.iter().map(|&n| n(&En).into()).collect();
            found.extend(problems("a question", &names, &[]));
        }
        assert!(found.is_empty(), "{found:#?}");
    }

    #[test]
    fn the_check_finds_a_letter_used_twice_or_missing() {
        let names = ["&Open", "&Other", "Plain", "&1 one", "&Two &marks"].map(String::from);
        let found = problems("test", &names, &[]);
        assert_eq!(found.len(), 4, "{found:#?}");
        assert!(problems("test", &names[..1], b"O")[0].contains("used twice"));
    }
}
