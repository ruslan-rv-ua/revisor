//! Interface strings (ADR-0009): one module per language behind the `Strings` trait.
//!
//! Every string is a method, and strings chosen by an id are an exhaustive `match`, so a
//! language that lacks a string, or takes other parameters for it, does not compile.

pub mod catalog;
mod en;
pub mod plural;
mod uk;

pub use en::En;
pub use uk::Uk;

use revisor_core::compare::Warning;
use revisor_core::exclude::PatternError;
use revisor_core::scan::EntryKind;
use revisor_core::settings::{
    AttributeFormat, ChangeType, CsvDelimiter, DifferenceView, FocusAfterComparison,
    FocusAfterSnapshot, Language, SizeFormat, Theme, TreeNodeText,
};
use revisor_core::typical::TypicalFolder;

use crate::commands::{CommandId, MenuId, Pane, Reason, SubmenuId};
use crate::details::{Attribute, Inaccessibility, Property};
use crate::difference::{
    Column, Field, InaccessibleIn, NodeText, PairText, RowChange, SummaryText,
};

/// An interface language.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    Uk,
}

/// Why an action on a file failed, in the terms revisor explains in its own words (brief,
/// section 5); anything else keeps the text and code of Windows or SQLite.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    AccessDenied,
    /// Another program, maybe another copy of revisor, has the file open.
    InUse,
    DiskFull,
    /// A newer version of revisor made the profile.
    NewerVersion,
    NotAProfile,
    /// The file in the place of the settings database is something else.
    NotSettings,
    /// The user cancelled the action, say at the administrator prompt.
    Cancelled,
    /// The snapshot was deleted while the action needed it (TASK-11).
    SnapshotDeleted,
    /// Windows or SQLite gave this text and code: revisor shows both, as help is searched
    /// for by the code. The code is as they give it, so an HRESULT or an NTSTATUS comes as a
    /// negative number, which `error_code` writes in hexadecimal (TASK-26).
    Other {
        text: String,
        code: i64,
    },
    /// An error with no code of Windows or SQLite, such as one of rusqlite itself: revisor
    /// says it did not expect it, and the text follows as it is, for a report of the defect
    /// (TASK-25).
    Unexpected(String),
}

/// The text of Windows or SQLite as the second half of a sentence (TASK-17, TASK-25): its
/// first letter small, unless its first word is a name — the word "Windows", or a word
/// with a capital letter after its first letter, as "SQLite", "OneDrive" or "NTFS".
fn in_sentence(text: &str) -> String {
    let word = text.split_whitespace().next().unwrap_or_default();
    let is_name = word.trim_end_matches(|c: char| !c.is_alphanumeric()) == "Windows"
        || word.chars().skip(1).any(char::is_uppercase);
    let mut chars = text.chars();
    match chars.next() {
        Some(first) if !is_name => first.to_lowercase().chain(chars).collect(),
        _ => text.to_string(),
    }
}

/// `text` with its first letter small, as words of a column read after a dash (TASK-33):
/// "Додано теку" in «docs — додано теку».
fn lower_first(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_lowercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// A code of Windows or SQLite as help is searched for by it (TASK-26). A 32-bit code with
/// its high bit set — an HRESULT or an NTSTATUS, negative as a number — is written in
/// hexadecimal with "0x" and eight capital digits, as Microsoft's guidelines for error
/// messages have it: "0x80004005". Any other code, as those of Win32 and SQLite, is
/// written in decimal, as the System Error Codes reference and SQLite's documentation
/// write them: "3".
fn error_code(code: i64) -> String {
    match i32::try_from(code) {
        Ok(negative) if negative < 0 => format!("0x{:08X}", negative.cast_unsigned()),
        _ => code.to_string(),
    }
}

/// Declares the trait [`Strings`] from its methods and, for the tests, `METHODS`, their
/// names, and `Recorder`, a `Strings` that notes every method called on it. The test of
/// the catalog builds the catalog with recorders, so it checks that the catalog calls
/// every method without a list of them kept apart from the trait (TASK-25).
macro_rules! strings {
    (
        $(#[$trait_attr:meta])*
        pub trait Strings {
            $(
                $(#[$attr:meta])*
                fn $name:ident(&self $(, $arg:ident: $ty:ty)*) -> $ret:ty;
            )*
        }
    ) => {
        $(#[$trait_attr])*
        pub trait Strings {
            $(
                $(#[$attr])*
                fn $name(&self $(, $arg: $ty)*) -> $ret;
            )*
        }

        /// The names of the methods of [`Strings`].
        #[cfg(test)]
        pub const METHODS: &[&str] = &[$(stringify!($name)),*];

        /// A [`Strings`] that answers as `inner` does and notes the name of every method
        /// called on it.
        #[cfg(test)]
        pub struct Recorder<'a> {
            inner: &'a dyn Strings,
            called: std::cell::RefCell<std::collections::BTreeSet<&'static str>>,
        }

        #[cfg(test)]
        impl<'a> Recorder<'a> {
            pub fn new(inner: &'a dyn Strings) -> Self {
                Self {
                    inner,
                    called: Default::default(),
                }
            }

            /// The names of the methods called so far.
            pub fn called(&self) -> std::collections::BTreeSet<&'static str> {
                self.called.borrow().clone()
            }
        }

        #[cfg(test)]
        impl Strings for Recorder<'_> {
            $(
                fn $name(&self $(, $arg: $ty)*) -> $ret {
                    self.called.borrow_mut().insert(stringify!($name));
                    self.inner.$name($($arg),*)
                }
            )*
        }
    };
}

strings! {
/// All interface strings of one language. A name with an access key is marked with "&"
/// before its letter in English, and has the words only in Ukrainian (TASK-30): the
/// `access` module writes the label, and `access::plain` gives the name as text.
pub trait Strings {
    fn lang(&self) -> Lang;

    /// A command's name as the menu shows it, without ellipsis.
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
    /// Two commands that share a shortcut, which switches between them (TASK-33): "List or
    /// tree".
    fn either(&self, first: &str, second: &str) -> String;
    fn navigation(&self) -> &'static str;
    fn switch_panes(&self) -> &'static str;
    fn context_menu(&self) -> &'static str;

    // The main window.
    /// A pane's main list, as screen readers name it.
    fn pane(&self, pane: Pane) -> &'static str;
    /// The label of the profile box, without colon.
    fn profile(&self) -> &'static str;
    /// Said when a list's context menu would have no items.
    fn no_actions(&self) -> &'static str;
    /// The action button of a question that closes revisor.
    fn close(&self) -> &'static str;
    /// Closes the «Help» windows, with no access key; Esc does the same.
    fn close_window(&self) -> &'static str;
    /// The «About revisor» text above the third-party licences.
    fn about(&self, version: &str) -> String;

    // The profile window (TASK-7); labels come without colon.
    fn profile_name(&self) -> &'static str;
    fn profile_folders(&self) -> &'static str;
    fn add_folder(&self) -> &'static str;
    fn remove_folder(&self) -> &'static str;
    /// The button that opens the list of typical folders, and the title of its window
    /// (TASK-36).
    fn add_typical_folders(&self) -> &'static str;
    /// A typical folder in its list, before its path.
    fn typical_folder(&self, folder: TypicalFolder) -> &'static str;
    /// A folder of another user of this computer in the list of typical folders, before
    /// its path: the user folder, AppData or the desktop of `user` (TASK-43).
    fn other_user_folder(&self, folder: TypicalFolder, user: &str) -> String;
    /// The button of the list of typical folders that adds the checked ones.
    fn add(&self) -> &'static str;
    /// Said instead of the list when no typical folder is left to add.
    fn typical_folders_in_profile(&self) -> &'static str;
    /// Said above the list of typical folders, and after the message that all of them are
    /// in the profile, when other users' folders are left out for want of rights (TASK-43).
    fn other_users_need_admin(&self) -> &'static str;
    fn profile_exclusions(&self) -> &'static str;
    /// The button that makes a new profile.
    fn create(&self) -> &'static str;
    fn ok(&self) -> &'static str;
    /// Cancel of a dialog, with no access key: Esc presses it.
    fn cancel(&self) -> &'static str;
    /// Cancel of the progress row of the main window while it shows a snapshot, a
    /// comparison or a comment waiting for the database (TASK-24).
    fn cancel_job(&self) -> &'static str;
    /// A check box under the difference list that shows or hides the rows of a type
    /// (TASK-13 #22).
    fn type_box(&self, kind: ChangeType) -> &'static str;
    /// The prompt of the folder picker.
    fn choose_folder(&self) -> &'static str;
    fn name_missing(&self) -> &'static str;
    fn name_taken(&self, name: &str) -> String;
    fn no_folders(&self) -> &'static str;
    fn bad_pattern(&self, line: usize, pattern: &str, error: PatternError) -> String;
    /// A folder whose files a folder of the profile covers already.
    fn folder_covered(&self, folder: &str, by: &str) -> String;
    fn folder_not_ntfs(&self, folder: &str, file_system: &str) -> String;
    fn folder_on_network(&self, folder: &str) -> String;
    fn folder_not_examined(&self, folder: &str, reason: &str) -> String;
    /// The question whether a new folder replaces the profile's folders inside it.
    fn replace_folders(&self, folder: &str, inside: &[String]) -> String;
    fn replace(&self) -> &'static str;

    // Profiles in the main window (TASK-7).
    fn delete_profile_question(&self, name: &str, snapshots: usize) -> String;
    fn delete(&self) -> &'static str;
    fn profile_created(&self, name: &str) -> String;
    fn profile_changed(&self, name: &str) -> String;
    fn profile_deleted(&self, name: &str) -> String;
    fn profile_not_saved(&self, reason: &str) -> String;
    fn profile_not_deleted(&self, name: &str, reason: &str) -> String;
    /// A file in the folder of profiles that revisor cannot open as a profile.
    fn profile_not_opened(&self, file: &str, reason: &str) -> String;
    fn profiles_not_listed(&self, folder: &str, reason: &str) -> String;
    /// Why an action on a file failed, as the second half of a message.
    fn failure(&self, failure: &Failure) -> String;

    // The snapshot in progress and its outcome (TASK-8). Counts come with their text in
    // the user's regional format; the number itself picks the plural form.
    /// The label of the progress field, without access key or colon.
    fn progress(&self) -> &'static str;
    /// The progress field while folders are read; without a folder, the count alone.
    fn snapshot_progress(&self, entries: u64, entries_text: &str, folder: &str) -> String;
    /// The progress field while the snapshot is written to the database.
    fn snapshot_saving(&self, entries: u64, entries_text: &str) -> String;
    fn snapshot_started(&self) -> &'static str;
    fn snapshot_saved(&self, entries: u64, entries_text: &str) -> String;
    fn entries_inaccessible(&self, entries_text: &str) -> String;
    /// The folders of the profile the snapshot did not find: each path with the reason.
    fn folders_not_found(&self, folders: &[(String, String)]) -> String;
    fn folder_missing(&self) -> &'static str;
    fn folder_volume_not_ntfs(&self, file_system: &str) -> String;
    fn folder_volume_network(&self) -> &'static str;
    /// The folder's drive is not on the system, or its device holds no medium (TASK-25).
    fn folder_drive_not_connected(&self) -> &'static str;
    fn folder_is_a_file(&self) -> &'static str;
    fn folder_unreachable(&self, reason: &str) -> String;
    fn snapshot_cancelled(&self) -> &'static str;
    fn snapshot_failed(&self, reason: &str) -> String;
    /// The reason when the snapshot's thread ended without a result.
    fn snapshot_stopped(&self) -> &'static str;
    /// The offer to restart as administrator after a snapshot with entries closed by rights.
    fn restart_question(&self, denied_text: &str) -> String;
    fn restart(&self) -> &'static str;
    /// The questions asked while a snapshot is being taken.
    fn close_during_snapshot(&self) -> &'static str;
    fn restart_during_snapshot(&self) -> &'static str;
    fn restart_failed(&self, reason: &str) -> String;

    // The snapshot list (TASK-9).
    fn column_date_time(&self) -> &'static str;
    fn column_comment(&self) -> &'static str;
    /// Shown in the empty list; `key` is the shortcut that takes a snapshot.
    fn no_snapshots_yet(&self, key: &str) -> String;
    /// Shown in the empty list while there are no profiles; `key` makes a profile.
    fn no_profiles_yet(&self, key: &str) -> String;
    /// The only item of the profile box while there are no profiles.
    fn no_profiles_item(&self) -> &'static str;
    /// The label of the comment field, without access key or colon.
    fn comment_of(&self, snapshot: &str) -> String;
    fn comment_saved(&self) -> &'static str;
    fn comment_removed(&self) -> &'static str;
    /// A comment whose wait for the database was cancelled (TASK-24).
    fn comment_cancelled(&self) -> &'static str;
    fn comment_not_saved(&self, reason: &str) -> String;
    /// The question before one snapshot, named with its comment, is deleted.
    fn delete_snapshot_question(&self, name: &str, comment: Option<&str>) -> String;
    /// The question before several snapshots are deleted.
    fn delete_snapshots_question(&self, count: usize) -> String;
    /// The status bar message once snapshots are deleted and the compaction started.
    fn snapshots_deleted(&self, count: usize, profile: &str) -> String;
    fn snapshots_not_deleted(&self, reason: &str) -> String;
    /// The progress field while a profile's database is compacted.
    fn compacting(&self, profile: &str) -> String;
    /// `freed` is the space returned, in the user's format.
    fn compaction_done(&self, profile: &str, freed: &str) -> String;
    fn compaction_cancelled(&self, profile: &str) -> String;
    fn compaction_failed(&self, profile: &str, reason: &str) -> String;
    /// The reason when the compaction's thread ended without a result.
    fn compaction_stopped(&self) -> &'static str;

    // The settings window (TASK-9); labels come without access key or colon.
    fn focus_after_snapshot(&self) -> &'static str;
    fn focus_option(&self, value: FocusAfterSnapshot) -> &'static str;
    fn confirm_snapshot_deletion(&self) -> &'static str;
    fn settings_saved(&self) -> &'static str;
    fn settings_not_saved(&self, reason: &str) -> String;
    /// The settings file could not be used, so the defaults are.
    fn settings_not_opened(&self, file: &str, reason: &str) -> String;

    // The interface language (TASK-16).
    /// The label of the language box, without access key or colon.
    fn language(&self) -> &'static str;
    /// A value of the language setting; each language is named in itself.
    fn language_option(&self, value: Language) -> &'static str;
    /// Said once the setting makes the next start speak another language.
    fn language_changes_after_restart(&self) -> &'static str;
    fn restart_now(&self) -> &'static str;
    fn later(&self) -> &'static str;

    // The theme of the interface (TASK-34).
    /// The label of the theme box, without access key or colon.
    fn theme(&self) -> &'static str;
    fn theme_option(&self, value: Theme) -> &'static str;

    // Comparisons (TASK-11).
    /// The progress field while the snapshots are compared.
    fn comparing(&self) -> &'static str;
    /// The progress field once the bar shows: with the percent done.
    fn comparing_percent(&self, percent: u8) -> String;
    /// The progress field while the comparison waits for the profile's database.
    fn waiting_for_database(&self) -> &'static str;
    fn comparison_finished(&self) -> &'static str;
    fn comparison_cancelled(&self) -> &'static str;
    fn comparison_failed(&self, reason: &str) -> String;
    /// The reason when the comparison's thread ended without a result.
    fn comparison_stopped(&self) -> &'static str;
    /// The reason when the thread of a comment waiting for the database ended without a
    /// result (TASK-24).
    fn comment_stopped(&self) -> &'static str;
    /// The label of the list of the «Порівняти з…» window, without access key or colon;
    /// `snapshot` is the current snapshot's name.
    fn compare_with_label(&self, snapshot: &str) -> String;
    /// The button of the «Порівняти з…» window that starts the comparison.
    fn compare(&self) -> &'static str;
    /// The label of the setting «Фокус після порівняння», without access key or colon.
    fn focus_after_comparison(&self) -> &'static str;
    fn comparison_focus_option(&self, value: FocusAfterComparison) -> &'static str;

    // «Знімок + порівняння» (TASK-12).
    /// Said when the comparison of «Знімок + порівняння» is cancelled: its snapshot stays.
    fn snapshot_saved_comparison_cancelled(&self) -> &'static str;
    /// Said after the report of the snapshot of «Знімок + порівняння» when the profile
    /// had no snapshot to compare it with.
    fn nothing_to_compare(&self) -> &'static str;

    // The difference list (TASK-11, TASK-13).
    /// The heading of a column of the difference list.
    fn column(&self, column: Column) -> &'static str;
    /// What the column «Зміна» says: what the row is and, for a folder or a link, which.
    fn change(&self, change: RowChange, kind: EntryKind) -> &'static str;
    /// A field that differs, as the column «Що змінилося» names it.
    fn field(&self, field: Field) -> &'static str;
    /// In which snapshot an inaccessible entry is so, as the column «Що змінилося» says.
    fn inaccessible_in(&self, which: InaccessibleIn) -> &'static str;
    /// Where a renamed or moved entry was, as the column «Що змінилося» starts (TASK-42):
    /// "з a.txt"; `former` is its name, its folder or its path then.
    fn moved_from(&self, former: &str) -> String;
    /// A size in bytes; `text` is the count in the user's format: "5 байтів".
    fn bytes(&self, bytes: u64, text: &str) -> String;
    /// The unit of 1024 (or 1000) bytes to the power `power`, from 1 to 6: KB to EB.
    fn size_unit(&self, power: u32) -> &'static str;
    /// The line of the pair of snapshots in the field «Порівняння».
    fn pair(&self, pair: &PairText) -> String;
    /// The summary of a difference.
    fn summary(&self, summary: &SummaryText) -> String;
    /// How many rows the list shows of all the difference has: "Показано 12 із 22.".
    fn showing(&self, shown_text: &str, total_text: &str) -> String;
    /// A warning of the comparison, a line of the field «Порівняння»; `reason` says why
    /// a folder could not be read, for the warning that says so.
    fn warning(&self, warning: &Warning, reason: &str) -> String;
    /// What the difference list is named before the first comparison.
    fn no_difference_yet(&self) -> &'static str;
    /// What the difference list is named while the filter hides every row.
    fn no_match(&self) -> &'static str;
    /// The field «Порівняння» before the first comparison; `quick` and `choose` are the
    /// shortcuts of «Порівняти з попереднім» and «Порівняти з…».
    fn nothing_compared_yet(&self, quick: &str, choose: &str) -> String;
    /// The labels of the field «Порівняння» and of the field «Фільтр», without access key
    /// or colon.
    fn comparison_field(&self) -> &'static str;
    fn filter_field(&self) -> &'static str;
    /// The label of the setting «Розмір» (TASK-13), without access key or colon.
    fn size_format(&self) -> &'static str;
    fn size_option(&self, value: SizeFormat) -> &'static str;

    // The difference tree (TASK-33).
    /// What a node of the tree says: "a.txt — змінено: розмір", "docs — 12 змін".
    fn tree_node(&self, node: &NodeText) -> String;
    /// What the node «Ще…» of a folder shown by pages says (TASK-37): how many nodes are
    /// not shown after the pages shown, or before them when `above`, and that Enter shows
    /// the next page of them, of `difference_tree::PAGE` nodes.
    fn tree_more(&self, count: &str, above: bool) -> String;
    /// Said when Ctrl+T switches the view while the focus is not on the difference.
    fn layout_shown(&self, layout: DifferenceView) -> &'static str;
    /// The label of the setting «Вузли дерева», without access key or colon.
    fn tree_nodes(&self) -> &'static str;
    fn tree_node_option(&self, value: TreeNodeText) -> &'static str;

    // The details of a change (TASK-14).
    /// The name of a property, at the start of its line: "Розмір".
    fn property(&self, property: Property) -> &'static str;
    /// A property that differs: "було 5 байтів, стало 7 байтів".
    fn before_after(&self, before: &str, after: &str) -> String;
    /// A property that has not changed: "5 байтів (без змін)".
    fn unchanged(&self, value: &str) -> String;
    /// A difference the comparison does not count: "… (не враховано)".
    fn not_counted(&self, text: &str) -> String;
    /// The type of a change at the start of the line «Зміна»: "змінено".
    fn change_type(&self, change: RowChange) -> &'static str;
    /// What changed, after the type in the line «Зміна»: "змінилося: розмір".
    fn what_changed(&self, fields: &str) -> String;
    /// The kind of an entry: "тека".
    fn kind(&self, kind: EntryKind) -> &'static str;
    /// An attribute in words: "прихований".
    fn attribute(&self, attribute: Attribute) -> &'static str;
    fn no_attributes(&self) -> &'static str;
    fn version_not_read(&self) -> &'static str;
    /// In place of the version of a file without one.
    fn no_version(&self) -> &'static str;
    /// In place of the target of a link that could not be read.
    fn target_unknown(&self) -> &'static str;
    /// The value of the line «Недоступно»: in which snapshot, and why.
    fn inaccessible_details(&self, inaccessible: &Inaccessibility) -> String;
    /// The label of the setting «Атрибути», without access key or colon.
    fn attribute_format(&self) -> &'static str;
    fn attribute_option(&self, value: AttributeFormat) -> &'static str;
    /// The check box of the setting «Попереджати, коли запису вже немає на диску».
    fn warn_missing_entry(&self) -> &'static str;
    /// The check box of the setting «Показувати скопійований шлях у рядку стану»
    /// (TASK-41).
    fn show_copied_path(&self) -> &'static str;
    /// The check box of the setting «Показувати теки інших користувачів у переліку
    /// типових тек» (TASK-43).
    fn other_users_folders(&self) -> &'static str;
    /// The tabs of the settings window (TASK-44): «Загальні», «Знімки», «Різниця».
    fn settings_general(&self) -> &'static str;
    fn settings_snapshots(&self) -> &'static str;
    fn settings_difference(&self) -> &'static str;
    /// The question before File Explorer opens the folder of an entry no longer on disk.
    fn entry_gone_question(&self) -> &'static str;
    /// The button of that question that opens the folder.
    fn open(&self) -> &'static str;
    /// Why File Explorer was not opened: no folder of `path` is on disk.
    fn no_folder_on_disk(&self, path: &str) -> String;
    fn explorer_failed(&self, reason: &str) -> String;

    // Copying the path of the entry in focus (TASK-41).
    /// The status bar once the path is on the clipboard: «Шлях скопійовано.», or with the
    /// path, when the setting asks for it: «Шлях скопійовано: C:\Data\app.exe.».
    fn path_copied(&self, path: Option<&str>) -> String;
    /// The status bar when the path could not be put on the clipboard.
    fn path_not_copied(&self) -> &'static str;

    // The export of the difference (TASK-39).
    /// The type of file of the save dialog: «Текстовий звіт (*.txt)».
    fn export_text_type(&self) -> &'static str;
    /// The second type of file of the save dialog: «CSV (*.csv)» (TASK-40 #1).
    fn export_csv_type(&self) -> &'static str;
    /// The progress row's text while the difference is exported.
    fn exporting(&self) -> &'static str;
    /// The status bar after the export: «Різницю експортовано: <file>.».
    fn exported(&self, file: &str) -> String;
    fn export_cancelled(&self) -> &'static str;
    fn export_failed(&self, reason: &str) -> String;
    /// Why an export failed when its thread ended without a result.
    fn export_stopped(&self) -> &'static str;
    /// Why the exported file did not open in its program.
    fn open_failed(&self, reason: &str) -> String;
    /// The setting «Відкривати файл після експорту», and the settings window's tab of it.
    fn open_after_export(&self) -> &'static str;
    fn settings_export(&self) -> &'static str;
    /// The setting «Роздільник у CSV» (TASK-40 #5), without access key or colon, and its
    /// values.
    fn csv_delimiter(&self) -> &'static str;
    fn csv_delimiter_option(&self, value: CsvDelimiter) -> &'static str;
    /// The first line of the report: «Звіт про різницю, revisor 0.5.0».
    fn report_title(&self, version: &str) -> String;
    /// The line of the filter of the report: the types shown, when not all are, and the
    /// text, when there is one.
    fn report_filter(&self, types: Option<&[ChangeType]>, text: Option<&str>) -> String;
    /// A line of the report's header per folder compared.
    fn report_folder(&self, path: &str) -> String;
    /// The name of a group of the report, with its count: «Додано: 3».
    fn report_group(&self, kind: ChangeType, count: &str) -> String;
    /// A replaced entry, in its line of the report.
    fn report_replaced(&self) -> &'static str;
    /// In which snapshot an entry is inaccessible and why, in its line of the report.
    fn report_inaccessible(&self, inaccessible: &Inaccessibility) -> String;

    // Key names that are not printed on most keyboards.
    fn key_space(&self) -> &'static str;
    fn key_up(&self) -> &'static str;
    fn key_down(&self) -> &'static str;
    fn key_left(&self) -> &'static str;
    fn key_right(&self) -> &'static str;
    fn key_context_menu(&self) -> &'static str;
}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_delete_question_counts_the_snapshots_in_each_language() {
        let uk = |n| Uk.delete_profile_question("Дім", n);
        assert_eq!(
            uk(0),
            "Видалити профіль «Дім»? Знімків у ньому немає. Цю дію не можна скасувати."
        );
        for (n, snapshots) in [
            (1, "1 знімок"),
            (3, "3 знімки"),
            (5, "5 знімків"),
            (11, "11 знімків"),
            (21, "21 знімок"),
        ] {
            assert_eq!(
                uk(n),
                format!("Видалити профіль «Дім» і {snapshots} у ньому? Цю дію не можна скасувати.")
            );
        }
        let en = |n| En.delete_profile_question("Home", n);
        assert_eq!(
            en(0),
            "Delete the profile “Home”? It has no snapshots. This cannot be undone."
        );
        assert_eq!(
            en(1),
            "Delete the profile “Home” and its 1 snapshot? This cannot be undone."
        );
        assert_eq!(
            en(2),
            "Delete the profile “Home” and its 2 snapshots? This cannot be undone."
        );
    }

    #[test]
    fn snapshot_counts_take_their_plural_forms() {
        assert_eq!(
            Uk.snapshot_progress(1, "1", r"C:\x"),
            r"1 запис, читається C:\x"
        );
        assert_eq!(Uk.snapshot_progress(3, "3", ""), "3 записи");
        assert_eq!(Uk.snapshot_saved(5, "5"), "Знімок збережено: 5 записів.");
        assert_eq!(Uk.snapshot_saved(21, "21"), "Знімок збережено: 21 запис.");
        assert_eq!(
            Uk.snapshot_saving(12345, "12 345"),
            "Збереження знімка: 12 345 записів…"
        );
        assert_eq!(
            En.snapshot_progress(1, "1", r"C:\x"),
            r"1 entry, reading C:\x"
        );
        assert_eq!(En.snapshot_progress(0, "0", ""), "0 entries");
        assert_eq!(
            En.snapshot_saved(12345, "12,345"),
            "Snapshot saved: 12,345 entries."
        );
    }

    #[test]
    fn the_delete_snapshot_questions_name_the_snapshot_or_count_them() {
        assert_eq!(
            Uk.delete_snapshot_question("31.10.2026 14:05:12", Some("перед встановленням X")),
            "Видалити знімок 31.10.2026 14:05:12 «перед встановленням X»? Цю дію не можна \
             скасувати."
        );
        assert_eq!(
            Uk.delete_snapshot_question("31.10.2026 14:05:12", None),
            "Видалити знімок 31.10.2026 14:05:12? Цю дію не можна скасувати."
        );
        for (n, snapshots) in [(2, "2 знімки"), (5, "5 знімків"), (21, "21 знімок")]
        {
            assert_eq!(
                Uk.delete_snapshots_question(n),
                format!("Видалити {snapshots}? Цю дію не можна скасувати.")
            );
        }
        assert_eq!(
            Uk.snapshots_deleted(3, "Дім"),
            "Видалено 3 знімки; триває стискання бази профілю «Дім»."
        );
        assert_eq!(
            En.delete_snapshot_question("10/31/2026 2:05:12 PM", Some("before X")),
            "Delete the snapshot 10/31/2026 2:05:12 PM “before X”? This cannot be undone."
        );
        assert_eq!(
            En.delete_snapshots_question(2),
            "Delete 2 snapshots? This cannot be undone."
        );
        assert_eq!(
            En.snapshots_deleted(1, "Home"),
            "Deleted 1 snapshot; the database of the profile “Home” is being compacted."
        );
    }

    #[test]
    fn the_empty_list_hints_name_the_keys() {
        // In a sentence the word «клавіша» comes before the key's name (document
        // «Клавіатура й меню»).
        assert_eq!(
            Uk.no_snapshots_yet("F5"),
            "Знімків ще немає. Натисніть клавішу F5, щоб зробити перший знімок."
        );
        assert_eq!(
            Uk.no_profiles_yet("Ctrl+N"),
            "Профілів ще немає. Натисніть клавіші Ctrl+N, щоб створити перший профіль."
        );
        assert_eq!(
            En.no_snapshots_yet("F5"),
            "No snapshots yet. Press F5 to take the first snapshot."
        );
        assert_eq!(
            En.no_profiles_yet("Ctrl+N"),
            "No profiles yet. Press Ctrl+N to create the first profile."
        );
    }

    #[test]
    fn the_comparison_strings_come_in_both_languages() {
        assert_eq!(Uk.comparing_percent(45), "Порівняння знімків: 45 %…");
        assert_eq!(En.comparing_percent(45), "Comparing snapshots: 45%…");
        assert_eq!(
            Uk.compare_with_label("05.11.2026 12:00:00"),
            "Порівняти знімок 05.11.2026 12:00:00 зі знімком"
        );
        assert_eq!(
            En.compare_with_label("11/5/2026 12:00:00 PM"),
            "Compare the snapshot 11/5/2026 12:00:00 PM &with"
        );
        assert_eq!(
            Uk.comparison_failed("знімок тим часом видалено"),
            "Знімки не порівняно: знімок тим часом видалено."
        );
        assert_eq!(
            En.comparison_failed("the snapshot has been deleted meanwhile"),
            "The snapshots were not compared: the snapshot has been deleted meanwhile."
        );
        let uk: Vec<&str> = FocusAfterComparison::ALL
            .iter()
            .map(|&v| Uk.comparison_focus_option(v))
            .collect();
        assert_eq!(uk, ["Якщо фокус не рухали", "Завжди", "Ніколи"]);
        let en: Vec<&str> = FocusAfterComparison::ALL
            .iter()
            .map(|&v| En.comparison_focus_option(v))
            .collect();
        assert_eq!(en, ["If focus has not moved", "Always", "Never"]);
    }

    #[test]
    fn the_difference_list_strings_come_in_both_languages() {
        use crate::difference::{Column, RowChange};
        use revisor_core::compare::Side;
        let uk: Vec<&str> = Column::ALL.iter().map(|&c| Uk.column(c)).collect();
        assert_eq!(
            uk,
            [
                "Ім'я",
                "Зміна",
                "Що змінилося",
                "Розмір",
                "Дата зміни",
                "Тека"
            ]
        );
        let en: Vec<&str> = Column::ALL.iter().map(|&c| En.column(c)).collect();
        assert_eq!(
            en,
            [
                "Name",
                "Change",
                "What changed",
                "Size",
                "Date modified",
                "Folder"
            ]
        );
        // The examples of the task (#14, #15).
        assert_eq!(Uk.change(RowChange::Added, EntryKind::Dir), "Додано теку");
        assert_eq!(
            Uk.change(RowChange::Deleted, EntryKind::Link),
            "Видалено посилання"
        );
        assert_eq!(
            Uk.change(RowChange::Inaccessible, EntryKind::Dir),
            "Недоступна тека"
        );
        assert_eq!(
            Uk.change(RowChange::Replaced, EntryKind::Link),
            "Замінено посилання"
        );
        assert_eq!(Uk.change(RowChange::Added, EntryKind::File), "Додано");
        assert_eq!(En.change(RowChange::Added, EntryKind::Dir), "Folder added");
        assert_eq!(
            En.change(RowChange::Deleted, EntryKind::Link),
            "Link deleted"
        );
        assert_eq!(
            En.change(RowChange::Inaccessible, EntryKind::Dir),
            "Inaccessible folder"
        );
        assert_eq!(En.change(RowChange::Replaced, EntryKind::File), "Replaced");
        // Renamed and moved entries (TASK-42 #6): what happened, with the kind of a folder
        // and of a link, and where the entry was.
        assert_eq!(
            Uk.change(RowChange::Renamed, EntryKind::File),
            "Перейменовано"
        );
        assert_eq!(
            Uk.change(RowChange::Moved, EntryKind::Dir),
            "Переміщено теку"
        );
        assert_eq!(
            Uk.change(RowChange::MovedAndRenamed, EntryKind::Link),
            "Переміщено й перейменовано посилання"
        );
        assert_eq!(
            En.change(RowChange::Renamed, EntryKind::Dir),
            "Folder renamed"
        );
        assert_eq!(En.change(RowChange::Moved, EntryKind::Link), "Link moved");
        assert_eq!(
            En.change(RowChange::MovedAndRenamed, EntryKind::File),
            "Moved and renamed"
        );
        assert_eq!(Uk.moved_from(r"C:\Old\a.txt"), r"з C:\Old\a.txt");
        assert_eq!(En.moved_from("a.txt"), "from a.txt");
        assert_eq!(
            Uk.type_box(ChangeType::Moved),
            "Перейменовані чи переміщені"
        );
        assert_eq!(En.type_box(ChangeType::Moved), "Renamed or mo&ved");
        assert_eq!(
            Uk.nothing_compared_yet("F7", "Ctrl+F7"),
            "Ще нічого не порівнювали. Натисніть клавішу F7, щоб порівняти поточний знімок із \
             попереднім, або клавіші Ctrl+F7, щоб вибрати, з яким знімком його порівняти."
        );
        assert_eq!(
            En.nothing_compared_yet("F7", "Ctrl+F7"),
            "Nothing compared yet. Press F7 to compare the current snapshot with the previous \
             one, or Ctrl+F7 to choose the snapshot to compare it with."
        );
        assert_eq!(Uk.no_difference_yet(), "Різниці ще немає");
        assert_eq!(En.no_difference_yet(), "No difference yet");
        assert_eq!(Uk.no_match(), "Жодна зміна не відповідає фільтру");
        assert_eq!(En.no_match(), "No changes match the filter");
        assert_eq!(Uk.showing("12", "22"), "Показано 12 із 22.");
        assert_eq!(En.showing("12", "22"), "Showing 12 of 22.");
        let missing = Warning::FolderMissing {
            path: std::path::PathBuf::from(r"D:\Games"),
            side: Side::Earlier,
        };
        assert_eq!(
            En.warning(&missing, ""),
            "The folder \u{201c}D:\\Games\u{201d} was missing in the earlier snapshot, so its \
             contents show as added."
        );
        assert_eq!(
            Uk.warning(&missing, ""),
            "Теки «D:\\Games» не було в ранішому знімку, тож її вміст показано доданим."
        );
        for (power, unit) in [
            (1, "КБ"),
            (2, "МБ"),
            (3, "ГБ"),
            (4, "ТБ"),
            (5, "ПБ"),
            (6, "ЕБ"),
        ] {
            assert_eq!(Uk.size_unit(power), unit);
        }
        assert_eq!(En.size_unit(3), "GB");
        assert_eq!(Uk.comparison_field(), "Порівняння");
        assert_eq!(En.comparison_field(), "Co&mparison");
        assert_eq!(Uk.filter_field(), "Фільтр");
        assert_eq!(En.filter_field(), "Fi&lter");
        assert_eq!(Uk.size_format(), "Розмір");
        assert_eq!(En.size_format(), "&Size");
    }

    #[test]
    fn the_snapshot_and_compare_strings_come_in_both_languages() {
        assert_eq!(
            Uk.snapshot_saved_comparison_cancelled(),
            "Знімок збережено, порівняння скасовано."
        );
        assert_eq!(
            En.snapshot_saved_comparison_cancelled(),
            "Snapshot saved, comparison canceled."
        );
        assert_eq!(
            Uk.nothing_to_compare(),
            "Знімок не порівняно: іншого знімка в профілі немає."
        );
        assert_eq!(
            En.nothing_to_compare(),
            "The snapshot was not compared: the profile has no other snapshot."
        );
    }

    #[test]
    fn a_known_failure_gives_its_advice_after_a_dash() {
        // The failure comes after a colon, so its advice takes a dash (TASK-17), with no
        // spaces around it in English, as Microsoft Writing Style Guide has it (TASK-25).
        assert_eq!(
            Uk.profile_not_saved(&Uk.failure(&Failure::AccessDenied)),
            "Профіль не збережено: немає доступу — перевірте права на цей файл чи теку."
        );
        assert_eq!(
            En.profile_not_saved(&En.failure(&Failure::DiskFull)),
            "The profile was not saved: the disk is full—free some space and try again."
        );
        let other = Failure::Other {
            text: "Не вдається знайти вказаний шлях".into(),
            code: 3,
        };
        assert_eq!(
            Uk.snapshot_failed(&Uk.failure(&other)),
            "Знімок не збережено: не вдається знайти вказаний шлях (код 3)."
        );
    }

    #[test]
    fn the_text_of_windows_starts_small_unless_its_first_word_is_a_name() {
        // An ordinary first word, in either language of Windows.
        assert_eq!(
            in_sentence("The system cannot find the path specified"),
            "the system cannot find the path specified"
        );
        assert_eq!(
            in_sentence("Системі не вдається знайти вказаний шлях"),
            "системі не вдається знайти вказаний шлях"
        );
        // A name: the word Windows, or a word with a capital letter after its first one.
        for name in [
            "Windows cannot verify the digital signature for this file",
            "Windows не вдається перевірити цифровий підпис цього файлу",
            "Windows, as it says",
            "OneDrive cannot sync this file",
            "SQLite failed",
            "NTFS is needed",
            "I/O error",
        ] {
            assert_eq!(in_sentence(name), name);
        }
        // Only the first word counts, and a word only starting as a name is none.
        assert_eq!(
            in_sentence("Access to OneDrive is denied"),
            "access to OneDrive is denied"
        );
        assert_eq!(in_sentence("Windowsless"), "windowsless");
        assert_eq!(in_sentence("A"), "a");
        assert_eq!(in_sentence(""), "");
    }

    #[test]
    fn a_failure_keeps_a_name_after_the_colon_in_both_languages() {
        let failed = |text: &str, code| Failure::Other {
            text: text.into(),
            code,
        };
        assert_eq!(
            En.restart_failed(&En.failure(&failed(
                "Windows cannot verify the digital signature for this file",
                577
            ))),
            "revisor was not restarted: Windows cannot verify the digital signature for this \
             file (code 577)."
        );
        assert_eq!(
            Uk.restart_failed(&Uk.failure(&failed(
                "Windows не вдається перевірити цифровий підпис цього файлу",
                577
            ))),
            "revisor не перезапущено: Windows не вдається перевірити цифровий підпис цього \
             файлу (код 577)."
        );
        assert_eq!(
            Uk.snapshot_failed(&Uk.failure(&failed("OneDrive не може синхронізувати", 362))),
            "Знімок не збережено: OneDrive не може синхронізувати (код 362)."
        );
        assert_eq!(
            En.snapshot_failed(&En.failure(&failed("Access is denied", 5))),
            "The snapshot was not saved: access is denied (code 5)."
        );
    }

    #[test]
    fn an_error_without_a_code_is_unexpected_in_both_languages() {
        let unexpected = Failure::Unexpected("Query returned no rows".into());
        assert_eq!(
            Uk.snapshot_failed(&Uk.failure(&unexpected)),
            "Знімок не збережено: сталася непередбачена помилка (Query returned no rows)."
        );
        assert_eq!(
            En.snapshot_failed(&En.failure(&unexpected)),
            "The snapshot was not saved: an unexpected error occurred (Query returned no rows)."
        );
    }

    /// A code as `io::Error::raw_os_error` gives it: an HRESULT or an NTSTATUS negative.
    fn raw(code: u32) -> i64 {
        i64::from(code.cast_signed())
    }

    #[test]
    fn an_hresult_is_hexadecimal_and_a_code_of_win32_or_sqlite_decimal_in_both_languages() {
        let failed = |text: &str, code| Failure::Other {
            text: text.into(),
            code,
        };
        // E_FAIL, which carries no Win32 code, from File Explorer (TASK-26).
        assert_eq!(
            Uk.explorer_failed(&Uk.failure(&failed("Невизначена помилка", raw(0x8000_4005)))),
            "Провідник не відкрито: невизначена помилка (код 0x80004005)."
        );
        assert_eq!(
            En.explorer_failed(&En.failure(&failed("Unspecified error", raw(0x8000_4005)))),
            "File Explorer did not open: unspecified error (code 0x80004005)."
        );
        // Capital letters, in an HRESULT (E_UNEXPECTED) and an NTSTATUS alike.
        assert_eq!(
            En.failure(&failed("Catastrophic failure", raw(0x8000_FFFF))),
            "catastrophic failure (code 0x8000FFFF)"
        );
        assert_eq!(
            Uk.failure(&failed("Порушення доступу", raw(0xC000_0005))),
            "порушення доступу (код 0xC0000005)"
        );
        // The codes of Win32 and SQLite stay decimal.
        assert_eq!(
            Uk.failure(&failed("Системі не вдається знайти вказаний шлях", 3)),
            "системі не вдається знайти вказаний шлях (код 3)"
        );
        assert_eq!(
            En.failure(&failed("disk I/O error", 266)),
            "disk I/O error (code 266)"
        );
    }

    #[test]
    fn only_a_32_bit_code_with_the_high_bit_set_is_hexadecimal() {
        assert_eq!(error_code(0), "0");
        assert_eq!(error_code(1223), "1223");
        assert_eq!(error_code(i64::from(i32::MAX)), "2147483647");
        // The high bit set: eight hexadecimal digits with capital letters.
        assert_eq!(error_code(i64::from(i32::MIN)), "0x80000000");
        assert_eq!(error_code(-1), "0xFFFFFFFF");
        assert_eq!(error_code(raw(0x8007_000E)), "0x8007000E");
        // A number beyond 32 bits is no such code: it is written as it is.
        assert_eq!(error_code(i64::from(i32::MIN) - 1), "-2147483649");
    }

    #[test]
    fn folders_not_found_are_named_with_their_reasons() {
        let folders = [
            (r"D:\Робота".to_string(), Uk.folder_missing().to_string()),
            (r"E:\Архів".to_string(), Uk.folder_volume_not_ntfs("FAT32")),
        ];
        assert_eq!(
            Uk.folders_not_found(&folders),
            r"Тек не знайдено: «D:\Робота» (теки немає); «E:\Архів» (тека на томі FAT32, не NTFS)."
        );
        let folders = [(r"D:\Work".to_string(), En.folder_missing().to_string())];
        assert_eq!(
            En.folders_not_found(&folders),
            r"Folders not found: “D:\Work” (the folder is missing)."
        );
        // A drive that is not connected says so in revisor's words (TASK-25).
        let folders = [(
            r"F:\Фото".to_string(),
            Uk.folder_drive_not_connected().into(),
        )];
        assert_eq!(
            Uk.folders_not_found(&folders),
            r"Тек не знайдено: «F:\Фото» (диск теки не підключено)."
        );
        let folders = [(
            r"F:\Photos".to_string(),
            En.folder_drive_not_connected().into(),
        )];
        assert_eq!(
            En.folders_not_found(&folders),
            r"Folders not found: “F:\Photos” (the folder's drive is not connected)."
        );
    }
}
