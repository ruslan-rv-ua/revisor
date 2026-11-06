//! Keys and shortcuts: what the key handler matches on and how a key is named in the
//! interface (document «Клавіатура й меню», section «Назви клавіш»).

use wxdragon::event::KeyboardEvent;
use wxdragon::keycode::*;

use crate::i18n::Strings;

/// A key by its position, not by the character the layout gives it: letters and digits
/// carry the code Windows gives them in every layout, so Ctrl+N works in Ukrainian too.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    /// A–Z, as the uppercase ASCII letter.
    Letter(u8),
    /// 0–9 on the main keyboard.
    Digit(u8),
    /// F1–F24.
    F(u8),
    Enter,
    Delete,
    Esc,
    Space,
    Tab,
    Backspace,
    Home,
    End,
    PageUp,
    PageDown,
    Up,
    Down,
    Left,
    Right,
    /// The context menu (Applications) key.
    ContextMenu,
    /// Any other key; never part of a command's shortcut.
    Other(i32),
}

/// A key with the modifiers held down.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Shortcut {
    pub key: Key,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Shortcut {
    pub const fn new(key: Key) -> Self {
        Self {
            key,
            ctrl: false,
            shift: false,
            alt: false,
        }
    }

    pub const fn ctrl(self) -> Self {
        Self { ctrl: true, ..self }
    }

    pub const fn shift(self) -> Self {
        Self {
            shift: true,
            ..self
        }
    }

    pub const fn alt(self) -> Self {
        Self { alt: true, ..self }
    }

    /// The shortcut of a key event of a window's char hook.
    pub fn from_event(key: &KeyboardEvent) -> Self {
        let code = key.get_key_code().unwrap_or_default();
        Self::from_wx(code, key.control_down(), key.shift_down(), key.alt_down())
    }

    /// The access key this is: the letter pressed with Alt alone.
    pub fn access_letter(&self) -> Option<u8> {
        match self.key {
            Key::Letter(letter) if self.alt && !self.ctrl && !self.shift => Some(letter),
            _ => None,
        }
    }

    /// The shortcut a wx key event describes.
    pub fn from_wx(code: i32, ctrl: bool, shift: bool, alt: bool) -> Self {
        let key = match code {
            c @ 0x41..=0x5A => Key::Letter(c as u8),
            c @ 0x61..=0x7A => Key::Letter(c as u8 - 0x20),
            c @ 0x30..=0x39 => Key::Digit(c as u8 - 0x30),
            c @ WXK_F1..=WXK_F24 => Key::F((c - WXK_F1 + 1) as u8),
            WXK_RETURN | WXK_NUMPAD_ENTER => Key::Enter,
            WXK_DELETE => Key::Delete,
            WXK_ESCAPE => Key::Esc,
            WXK_SPACE => Key::Space,
            WXK_TAB => Key::Tab,
            WXK_BACK => Key::Backspace,
            WXK_HOME => Key::Home,
            WXK_END => Key::End,
            WXK_PAGEUP => Key::PageUp,
            WXK_PAGEDOWN => Key::PageDown,
            WXK_UP => Key::Up,
            WXK_DOWN => Key::Down,
            WXK_LEFT => Key::Left,
            WXK_RIGHT => Key::Right,
            WXK_WINDOWS_MENU => Key::ContextMenu,
            c => Key::Other(c),
        };
        Self {
            key,
            ctrl,
            shift,
            alt,
        }
    }

    /// Whether a command may take this shortcut under the rules of the document
    /// «Клавіатура й меню»; `Err` says which rule it breaks. The test of the command
    /// table runs it.
    #[cfg_attr(not(test), expect(dead_code))]
    pub fn check_rules(&self) -> Result<(), &'static str> {
        let Self {
            key,
            ctrl,
            shift,
            alt,
        } = *self;
        match (ctrl, shift, alt, key) {
            // Keys Windows and the controls take for themselves.
            (_, _, _, Key::F(10)) => Err("F10 opens the menu bar"),
            (_, _, _, Key::F(6)) => Err("F6 moves between the parts of the window"),
            (false, false, false, Key::F(4)) | (true, false, false, Key::F(4)) => {
                Err("F4 opens a combo box, Ctrl+F4 closes a document")
            }
            (true, false, false, Key::Letter(b'A' | b'C' | b'V' | b'X' | b'Z')) => {
                Err("Ctrl+A/C/V/X/Z belong to the edit controls")
            }
            // Standard Windows meanings the document keeps.
            (false, false, false, Key::Delete)
            | (false, false, true, Key::Enter)
            | (true, false, false, Key::Enter)
            | (false, false, true, Key::F(4)) => Ok(()),
            (true, _, true, _) => Err("Ctrl+Alt is AltGr in the Ukrainian layout"),
            (_, _, true, _) => Err("Alt with a key is an access key or a system key"),
            (false, false, false, Key::F(1..=12)) => Ok(()),
            (true, false, false, Key::F(1..=12)) | (false, true, false, Key::F(1..=12)) => Ok(()),
            (true, false, false, Key::Letter(_) | Key::Digit(_)) => Ok(()),
            (true, true, false, Key::Letter(_)) => Ok(()),
            _ => Err("not one of the allowed kinds of shortcut"),
        }
    }

    /// The shortcut as the interface writes it: "Ctrl+Shift+F5", with no spaces.
    pub fn name(&self, s: &dyn Strings) -> String {
        let mut name = String::new();
        for (held, modifier) in [
            (self.ctrl, "Ctrl+"),
            (self.shift, "Shift+"),
            (self.alt, "Alt+"),
        ] {
            if held {
                name.push_str(modifier);
            }
        }
        match self.key {
            Key::Letter(c) => name.push(char::from(c)),
            Key::Digit(d) => name.push(char::from(b'0' + d)),
            Key::F(n) => name.push_str(&format!("F{n}")),
            Key::Enter => name.push_str("Enter"),
            Key::Delete => name.push_str("Delete"),
            Key::Esc => name.push_str("Esc"),
            Key::Tab => name.push_str("Tab"),
            Key::Backspace => name.push_str("Backspace"),
            Key::Home => name.push_str("Home"),
            Key::End => name.push_str("End"),
            Key::PageUp => name.push_str("Page Up"),
            Key::PageDown => name.push_str("Page Down"),
            Key::Space => name.push_str(s.key_space()),
            Key::Up => name.push_str(s.key_up()),
            Key::Down => name.push_str(s.key_down()),
            Key::Left => name.push_str(s.key_left()),
            Key::Right => name.push_str(s.key_right()),
            Key::ContextMenu => name.push_str(s.key_context_menu()),
            Key::Other(code) => name.push_str(&format!("#{code}")),
        }
        name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{En, Uk};

    #[test]
    fn letters_and_digits_come_from_the_key_code() {
        // wx reports the Windows virtual-key code, which is 'N' for the key that types
        // "т" in the Ukrainian layout.
        assert_eq!(
            Shortcut::from_wx('N' as i32, true, false, false),
            Shortcut::new(Key::Letter(b'N')).ctrl()
        );
        assert_eq!(
            Shortcut::from_wx('1' as i32, true, false, false),
            Shortcut::new(Key::Digit(1)).ctrl()
        );
        assert_eq!(
            Shortcut::from_wx(WXK_F5, false, true, false),
            Shortcut::new(Key::F(5)).shift()
        );
    }

    #[test]
    fn numpad_enter_is_enter_but_other_numpad_keys_are_not() {
        assert_eq!(
            Shortcut::from_wx(WXK_NUMPAD_ENTER, false, false, false),
            Shortcut::new(Key::Enter)
        );
        // NVDA takes the numpad, Delete included, for its review keys.
        assert_eq!(
            Shortcut::from_wx(WXK_NUMPAD_DELETE, false, false, false),
            Shortcut::new(Key::Other(WXK_NUMPAD_DELETE))
        );
    }

    #[test]
    fn combinations_are_joined_with_plus_and_no_spaces() {
        let cases = [
            (Shortcut::new(Key::F(5)).ctrl(), "Ctrl+F5"),
            (Shortcut::new(Key::F(10)).shift(), "Shift+F10"),
            (Shortcut::new(Key::Enter).alt(), "Alt+Enter"),
            (Shortcut::new(Key::Letter(b'N')).ctrl(), "Ctrl+N"),
            (Shortcut::new(Key::Digit(3)).ctrl(), "Ctrl+3"),
            (
                Shortcut::new(Key::Letter(b'S')).ctrl().shift(),
                "Ctrl+Shift+S",
            ),
            (Shortcut::new(Key::F(4)).alt(), "Alt+F4"),
            (Shortcut::new(Key::Delete), "Delete"),
            (Shortcut::new(Key::Esc), "Esc"),
            (Shortcut::new(Key::PageDown).ctrl(), "Ctrl+Page Down"),
            (Shortcut::new(Key::Backspace), "Backspace"),
        ];
        for (shortcut, expected) in cases {
            assert_eq!(shortcut.name(&En), expected);
            assert_eq!(
                shortcut.name(&Uk),
                expected,
                "keys labelled on the keyboard stay English"
            );
        }
    }

    #[test]
    fn allowed_shortcut_classes_pass_the_rules() {
        let allowed = [
            Shortcut::new(Key::F(5)),
            Shortcut::new(Key::F(1)),
            Shortcut::new(Key::F(2)),
            Shortcut::new(Key::F(5)).ctrl(),
            Shortcut::new(Key::F(7)).shift(),
            Shortcut::new(Key::Letter(b'N')).ctrl(),
            Shortcut::new(Key::Letter(b'F')).ctrl(),
            Shortcut::new(Key::Digit(4)).ctrl(),
            Shortcut::new(Key::Letter(b'E')).ctrl().shift(),
            // Standard Windows meanings the document keeps.
            Shortcut::new(Key::Delete),
            Shortcut::new(Key::Enter).alt(),
            Shortcut::new(Key::Enter).ctrl(),
            Shortcut::new(Key::F(4)).alt(),
        ];
        for shortcut in allowed {
            assert_eq!(shortcut.check_rules(), Ok(()), "{shortcut:?}");
        }
    }

    #[test]
    fn forbidden_shortcuts_break_the_rules() {
        let forbidden = [
            Shortcut::new(Key::Letter(b'F')).alt(),
            Shortcut::new(Key::Digit(1)).alt(),
            Shortcut::new(Key::Letter(b'N')).ctrl().alt(),
            Shortcut::new(Key::F(5)).ctrl().shift().alt(),
            Shortcut::new(Key::Letter(b'N')),
            Shortcut::new(Key::Digit(1)),
            Shortcut::new(Key::Letter(b'S')).shift(),
            Shortcut::new(Key::F(10)),
            Shortcut::new(Key::F(10)).shift(),
            Shortcut::new(Key::F(4)),
            Shortcut::new(Key::F(4)).ctrl(),
            Shortcut::new(Key::F(6)),
            Shortcut::new(Key::F(6)).shift(),
            Shortcut::new(Key::Space).alt(),
            Shortcut::new(Key::Space).ctrl(),
            Shortcut::new(Key::Esc).ctrl(),
            Shortcut::new(Key::Tab).ctrl(),
            Shortcut::new(Key::Esc),
            Shortcut::new(Key::Enter),
            Shortcut::new(Key::Down).alt(),
            Shortcut::new(Key::ContextMenu),
            Shortcut::new(Key::Other(WXK_INSERT)),
            Shortcut::new(Key::Other(WXK_NUMPAD5)).ctrl(),
            Shortcut::new(Key::Other(',' as i32)).ctrl(),
            Shortcut::new(Key::Home),
            Shortcut::new(Key::End).ctrl(),
        ];
        for letter in *b"ACVXZ" {
            assert!(
                Shortcut::new(Key::Letter(letter))
                    .ctrl()
                    .check_rules()
                    .is_err()
            );
        }
        for shortcut in forbidden {
            assert!(shortcut.check_rules().is_err(), "{shortcut:?}");
        }
    }

    #[test]
    fn ukrainian_translates_unlabelled_keys() {
        assert_eq!(Shortcut::new(Key::Space).ctrl().name(&Uk), "Ctrl+ПРОБІЛ");
        assert_eq!(Shortcut::new(Key::Down).name(&Uk), "Стрілка вниз");
        assert_eq!(
            Shortcut::new(Key::ContextMenu).name(&Uk),
            "Клавіша контекстного меню"
        );
        assert_eq!(Shortcut::new(Key::Space).ctrl().name(&En), "Ctrl+Spacebar");
        assert_eq!(Shortcut::new(Key::ContextMenu).name(&En), "Menu key");
    }
}
