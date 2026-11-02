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

    // The main window.
    /// A pane's main list, as screen readers name it.
    fn pane(&self, pane: Pane) -> &'static str;
    /// The label of the profile box, without access key or colon.
    fn profile(&self) -> &'static str;
    /// Said when a list's context menu would have no items.
    fn no_actions(&self) -> &'static str;
    /// Closes the «Help» windows; Esc does the same.
    fn close(&self) -> &'static str;
    /// The «About revisor» text above the third-party licences.
    fn about(&self, version: &str) -> String;

    // The debug-only demo state (REVISOR_DEMO=1).
    fn demo_snapshot(&self, n: usize) -> String;
    fn demo_change(&self, n: usize) -> String;
    fn demo_done(&self, command: &str) -> String;

    // Key names that are not printed on most keyboards.
    fn key_space(&self) -> &'static str;
    fn key_up(&self) -> &'static str;
    fn key_down(&self) -> &'static str;
    fn key_left(&self) -> &'static str;
    fn key_right(&self) -> &'static str;
    fn key_context_menu(&self) -> &'static str;
}
