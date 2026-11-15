//! revisor-ui: what the window and the command line share (ADR-0005, ADR-0015) — the
//! interface without the windows. The strings in both languages ([`i18n`]), the model of
//! the difference with its report and CSV ([`difference`]), the details of a change
//! ([`details`]), the export to a file ([`export`]), the words of a failure ([`failure`]),
//! the regional formats of Windows ([`format`]), what a saved snapshot says
//! ([`snapshot_text`]), the identities of the window's commands the strings name
//! ([`commands`]), the parts of a call of revisor-cli the strings name ([`cli`]) and where
//! the data lives ([`data`]). Nothing here knows wxDragon: the application `revisor` puts
//! windows over it, and `revisor-cli` a console.

pub mod cli;
pub mod commands;
pub mod data;
pub mod details;
pub mod difference;
pub mod export;
pub mod failure;
pub mod format;
pub mod i18n;
pub mod snapshot_text;
