//! Interface strings (ADR-0009): one module per language behind the `Strings` trait.
//!
//! Every string is a method, and strings chosen by an id are an exhaustive `match`, so a
//! language that lacks a string, or takes other parameters for it, does not compile.

mod en;
pub mod plural;
mod uk;

pub use en::En;
pub use uk::Uk;

use crate::commands::{CommandId, MenuId, Pane, Reason, SubmenuId};

/// An interface language.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    Uk,
}

/// All interface strings of one language.
pub trait Strings {
    fn lang(&self) -> Lang;

    /// A command's name as the menu shows it, without access key or ellipsis.
    fn command(&self, id: CommandId) -> &'static str;
    fn menu(&self, id: MenuId) -> &'static str;
    fn submenu(&self, id: SubmenuId) -> &'static str;
    /// Why a command cannot run now, as the second half of `unavailable`.
    fn reason(&self, reason: Reason) -> &'static str;
    /// The status bar message for a command that cannot run now.
    fn unavailable(&self, command: &str, reason: &str) -> String;
    /// The status bar message for a key used outside the only pane it works in.
    fn key_only_in(&self, key: &str, pane: Pane) -> String;

    // The «Keyboard shortcuts» window.
    /// After a shortcut limited to one pane: "(only in the snapshot list)".
    fn only_in(&self, pane: Pane) -> String;
    /// A key that works in one list: "Enter in the snapshot list".
    fn in_list(&self, key: &str, pane: Pane) -> String;
    fn navigation(&self) -> &'static str;
    fn switch_panes(&self) -> &'static str;
    fn context_menu(&self) -> &'static str;

    // Key names that are not printed on most keyboards.
    fn key_space(&self) -> &'static str;
    fn key_up(&self) -> &'static str;
    fn key_down(&self) -> &'static str;
    fn key_left(&self) -> &'static str;
    fn key_right(&self) -> &'static str;
    fn key_context_menu(&self) -> &'static str;
}
