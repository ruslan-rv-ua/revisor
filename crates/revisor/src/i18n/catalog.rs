//! Every interface string in both languages as plain text, for proofreading with a screen
//! reader (TASK-17, ADR-0009): `cargo run -p revisor --example strings` writes it to a file.
//!
//! Each string has an id — the method of [`Strings`] and, for a string chosen by a value,
//! that value — then its English and Ukrainian text. Parameters show as `{name}`, and a
//! string with a count comes once per count that picks a plural form: 1, 3 and 5 give
//! "one", "few" and "many" in Ukrainian and "one" and "other" in English.
//!
//! Nothing can be left out unnoticed (TASK-25). A test builds the catalog with recorders of
//! the calls and checks that it calls every method of [`Strings`]. The values of every enum
//! a string is chosen by come from `all!` or are checked by `each_variant_in!`: a new
//! variant does not compile until it is named here, and one named without a value fails
//! the tests.

use std::path::PathBuf;

use revisor_core::compare::{Side, Warning};
use revisor_core::exclude::PatternError;
use revisor_core::scan::EntryKind;
use revisor_core::settings::{
    AttributeFormat, ChangeType, DifferenceView, FocusAfterComparison, FocusAfterSnapshot,
    Language, SizeFormat, Theme, TreeNodeText,
};
use revisor_core::store::Unavailable;
use revisor_core::typical::TypicalFolder;

use super::{En, Failure, Strings, Uk};
use crate::access::plain;
use crate::commands::{COMMANDS, CommandId, MenuId, Pane, Reason, SubmenuId};
use crate::details::{Attribute, Inaccessibility, Property, Unreadable};
use crate::difference::{
    Column, Counted, Field, InaccessibleIn, NodeText, PairText, RowChange, SizeChange,
    SnapshotText, SummaryText,
};
use crate::menus;

/// Counts that pick every plural form of both languages.
const COUNTS: [u64; 3] = [1, 3, 5];

/// The values of an enum whose variants hold no data. The match makes a new variant fail to
/// compile until it is listed here too, so the catalog cannot miss it.
macro_rules! all {
    ($ty:ident { $($v:ident),* $(,)? }) => {{
        let _every_variant_is_listed = |x: $ty| match x {
            $($ty::$v)|* => (),
        };
        [$($ty::$v),*]
    }};
}

/// Checks that `values`, a collection of an enum whose variants may hold data, have a value
/// of every variant. The match makes a new variant fail to compile until it is named here
/// too, and the assertion fails — in the tests, which build the catalog — when a named
/// variant has no value.
macro_rules! each_variant_in {
    ($values:expr, $ty:ident { $($v:ident),* $(,)? }) => {{
        let _every_variant_is_named = |x: &$ty| match x {
            $($ty::$v { .. } => ()),*
        };
        $(assert!(
            $values.iter().any(|value| matches!(value, $ty::$v { .. })),
            concat!("the catalog has no ", stringify!($ty), "::", stringify!($v)),
        );)*
    }};
}

/// One string: its id and its text in English and in Ukrainian.
pub struct Entry {
    pub id: String,
    pub en: String,
    pub uk: String,
}

/// The strings gathered so far, each as the two languages give it.
struct Catalog<'a> {
    en: &'a dyn Strings,
    uk: &'a dyn Strings,
    entries: Vec<Entry>,
}

impl Catalog<'_> {
    fn add(&mut self, id: impl Into<String>, text: impl Fn(&dyn Strings) -> String) {
        self.entries.push(Entry {
            id: id.into(),
            en: text(self.en),
            uk: text(self.uk),
        });
    }

    /// Adds a string with a count once per count of [`COUNTS`].
    fn add_per_count(&mut self, id: &str, text: impl Fn(&dyn Strings, u64) -> String) {
        for n in COUNTS {
            self.add(format!("{id}({n})"), |s| text(s, n));
        }
    }
}

/// A count with its text, as a summary shows it.
fn counted(n: u64) -> Counted {
    Counted {
        n,
        text: n.to_string(),
    }
}

/// Every string of revisor, in the order of [`Strings`].
pub fn entries() -> Vec<Entry> {
    entries_in(&En, &Uk)
}

/// Every string of revisor as `en` and `uk` give it.
fn entries_in(en: &dyn Strings, uk: &dyn Strings) -> Vec<Entry> {
    let mut c = Catalog {
        en,
        uk,
        entries: Vec::new(),
    };
    let panes = all!(Pane {
        Snapshots,
        Difference
    });

    let commands: Vec<CommandId> = COMMANDS.iter().map(|command| command.id).collect();
    each_variant_in!(
        commands,
        CommandId {
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
            KeyboardShortcuts,
            About,
        }
    );
    for command in COMMANDS {
        c.add(format!("command.{:?}", command.id), |s| {
            plain(s.command(command.id))
        });
    }
    for command in COMMANDS {
        c.add(format!("command.{:?} (menu item)", command.id), |s| {
            menus::menu_label(s, command).replace('\t', "  [Tab]  ")
        });
    }
    let menu_ids = all!(MenuId {
        File,
        Snapshot,
        Compare,
        Difference,
        Help
    });
    for menu in menu_ids {
        c.add(format!("menu.{menu:?}"), |s| plain(s.menu(menu)));
        c.add(format!("menu.{menu:?} (menu bar)"), |s| {
            menus::top_menu_label(s, menu)
        });
    }
    for submenu in all!(SubmenuId { SortBy }) {
        c.add(format!("submenu.{submenu:?}"), |s| {
            plain(s.submenu(submenu))
        });
        c.add(format!("submenu.{submenu:?} (menu item)"), |s| {
            menus::submenu_label(s, submenu)
        });
    }
    let reasons = all!(Reason {
        NoProfiles,
        NoSnapshots,
        NoDifference,
        SnapshotRunning,
        SnapshotOfThisProfile,
        SnapshotNotRunning,
        ComparisonNotRunning,
        CompactionNotRunning,
        CommentWaiting,
        CommentNotWaiting,
        CompactionRunning,
        CompactionOfThisProfile,
        NoSelection,
        NoPreviousSnapshot,
        NoOtherSnapshot,
        AlreadyAdmin,
        NoChangeShown,
        TreeSortedByName,
    });
    for reason in reasons {
        c.add(format!("reason.{reason:?}"), |s| s.reason(reason).into());
    }
    c.add("unavailable", |s| s.unavailable("{command}", "{reason}"));
    for pane in panes {
        c.add(format!("key_only_in.{pane:?}"), |s| {
            s.key_only_in("Delete", pane)
        });
        c.add(format!("only_in.{pane:?}"), |s| s.only_in(pane));
        c.add(format!("in_list.{pane:?}"), |s| s.in_list("Enter", pane));
    }
    c.add("either", |s| s.either("{first}", "{Second}"));
    c.add("navigation", |s| s.navigation().into());
    c.add("switch_panes", |s| s.switch_panes().into());
    c.add("context_menu", |s| s.context_menu().into());
    c.add("shortcuts_text (the window «Keyboard shortcuts»)", |s| {
        menus::shortcuts_text(s)
    });

    for pane in panes {
        c.add(format!("pane.{pane:?}"), |s| s.pane(pane).into());
    }
    c.add("profile", |s| plain(s.profile()));
    c.add("no_actions", |s| s.no_actions().into());
    c.add("close", |s| plain(s.close()));
    c.add("close_window", |s| s.close_window().into());
    c.add("about", |s| s.about("{version}"));

    c.add("profile_name", |s| plain(s.profile_name()));
    c.add("profile_folders", |s| plain(s.profile_folders()));
    c.add("add_folder", |s| plain(s.add_folder()));
    c.add("remove_folder", |s| plain(s.remove_folder()));
    c.add("add_typical_folders", |s| plain(s.add_typical_folders()));
    let typical = all!(TypicalFolder {
        ProgramFiles,
        ProgramFilesX86,
        ProgramData,
        AppData,
        Windows,
        Desktop,
        PublicDesktop
    });
    for folder in typical {
        c.add(format!("typical_folder.{folder:?}"), |s| {
            s.typical_folder(folder).into()
        });
    }
    c.add("add", |s| s.add().into());
    c.add("typical_folders_in_profile", |s| {
        s.typical_folders_in_profile().into()
    });
    c.add("profile_exclusions", |s| plain(s.profile_exclusions()));
    c.add("create", |s| s.create().into());
    c.add("ok", |s| s.ok().into());
    c.add("cancel", |s| s.cancel().into());
    c.add("cancel_job", |s| plain(s.cancel_job()));
    let types = all!(ChangeType {
        Added,
        Deleted,
        Modified,
        Inaccessible
    });
    for kind in types {
        c.add(format!("type_box.{kind:?}"), |s| plain(s.type_box(kind)));
    }
    c.add("choose_folder", |s| s.choose_folder().into());
    c.add("name_missing", |s| s.name_missing().into());
    c.add("name_taken", |s| s.name_taken("{name}"));
    c.add("no_folders", |s| s.no_folders().into());
    let pattern_errors = [
        PatternError::NotFullPath,
        PatternError::Slash,
        PatternError::Colon,
        PatternError::Control('\u{7}'),
        PatternError::BadChar('|'),
    ];
    each_variant_in!(
        pattern_errors,
        PatternError {
            NotFullPath,
            Slash,
            Colon,
            Control,
            BadChar
        }
    );
    for error in pattern_errors {
        c.add(format!("bad_pattern.{error:?}"), |s| {
            s.bad_pattern(3, "{pattern}", error)
        });
    }
    c.add("folder_covered", |s| s.folder_covered("{folder}", "{by}"));
    c.add("folder_not_ntfs", |s| {
        s.folder_not_ntfs("{folder}", "FAT32")
    });
    c.add("folder_on_network", |s| s.folder_on_network("{folder}"));
    c.add("folder_not_examined", |s| {
        s.folder_not_examined("{folder}", "{reason}")
    });
    c.add("replace_folders", |s| {
        s.replace_folders("{folder}", &["{inside 1}".into(), "{inside 2}".into()])
    });
    c.add("replace", |s| plain(s.replace()));

    c.add("delete_profile_question(0)", |s| {
        s.delete_profile_question("{name}", 0)
    });
    c.add_per_count("delete_profile_question", |s, n| {
        s.delete_profile_question("{name}", n as usize)
    });
    c.add("delete", |s| plain(s.delete()));
    c.add("profile_created", |s| s.profile_created("{name}"));
    c.add("profile_changed", |s| s.profile_changed("{name}"));
    c.add("profile_deleted", |s| s.profile_deleted("{name}"));
    c.add("profile_not_saved", |s| s.profile_not_saved("{reason}"));
    c.add("profile_not_deleted", |s| {
        s.profile_not_deleted("{name}", "{reason}")
    });
    c.add("profile_not_opened", |s| {
        s.profile_not_opened("{file}", "{reason}")
    });
    c.add("profiles_not_listed", |s| {
        s.profiles_not_listed("{folder}", "{reason}")
    });
    let failures = [
        Failure::AccessDenied,
        Failure::InUse,
        Failure::DiskFull,
        Failure::NewerVersion,
        Failure::NotAProfile,
        Failure::NotSettings,
        Failure::Cancelled,
        Failure::SnapshotDeleted,
        Failure::Other {
            text: "{text of Windows or SQLite}".into(),
            code: 5,
        },
        // An HRESULT, E_FAIL, comes negative and shows in hexadecimal (TASK-26).
        Failure::Other {
            text: "{text of Windows}".into(),
            code: i64::from(0x8000_4005_u32.cast_signed()),
        },
        Failure::Unexpected("{text without a code}".into()),
    ];
    each_variant_in!(
        failures,
        Failure {
            AccessDenied,
            InUse,
            DiskFull,
            NewerVersion,
            NotAProfile,
            NotSettings,
            Cancelled,
            SnapshotDeleted,
            Other,
            Unexpected,
        }
    );
    for failure in &failures {
        let id = match failure {
            Failure::AccessDenied => "AccessDenied",
            Failure::InUse => "InUse",
            Failure::DiskFull => "DiskFull",
            Failure::NewerVersion => "NewerVersion",
            Failure::NotAProfile => "NotAProfile",
            Failure::NotSettings => "NotSettings",
            Failure::Cancelled => "Cancelled",
            Failure::SnapshotDeleted => "SnapshotDeleted",
            Failure::Other { code, .. } if *code < 0 => "Other.hresult",
            Failure::Other { .. } => "Other",
            Failure::Unexpected(_) => "Unexpected",
        };
        c.add(format!("failure.{id}"), |s| s.failure(failure));
    }

    c.add("progress", |s| plain(s.progress()));
    c.add_per_count("snapshot_progress", |s, n| {
        s.snapshot_progress(n, &n.to_string(), "{folder}")
    });
    c.add("snapshot_progress(5, no folder)", |s| {
        s.snapshot_progress(5, "5", "")
    });
    c.add_per_count("snapshot_saving", |s, n| {
        s.snapshot_saving(n, &n.to_string())
    });
    c.add("snapshot_started", |s| s.snapshot_started().into());
    c.add_per_count("snapshot_saved", |s, n| s.snapshot_saved(n, &n.to_string()));
    c.add("entries_inaccessible", |s| {
        s.entries_inaccessible("{count}")
    });
    c.add("folders_not_found", |s| {
        s.folders_not_found(&[
            ("{folder 1}".into(), s.folder_missing().into()),
            ("{folder 2}".into(), s.folder_volume_network().into()),
        ])
    });
    c.add("folder_missing", |s| s.folder_missing().into());
    c.add("folder_volume_not_ntfs", |s| {
        s.folder_volume_not_ntfs("FAT32")
    });
    c.add("folder_volume_network", |s| {
        s.folder_volume_network().into()
    });
    c.add("folder_drive_not_connected", |s| {
        s.folder_drive_not_connected().into()
    });
    c.add("folder_is_a_file", |s| s.folder_is_a_file().into());
    c.add("folder_unreachable", |s| s.folder_unreachable("{reason}"));
    c.add("snapshot_cancelled", |s| s.snapshot_cancelled().into());
    c.add("snapshot_failed", |s| s.snapshot_failed("{reason}"));
    c.add("snapshot_stopped", |s| s.snapshot_stopped().into());
    c.add("restart_question", |s| s.restart_question("{count}"));
    c.add("restart", |s| plain(s.restart()));
    c.add("close_during_snapshot", |s| {
        s.close_during_snapshot().into()
    });
    c.add("restart_during_snapshot", |s| {
        s.restart_during_snapshot().into()
    });
    c.add("restart_failed", |s| s.restart_failed("{reason}"));

    c.add("column_date_time", |s| s.column_date_time().into());
    c.add("column_comment", |s| s.column_comment().into());
    c.add("no_snapshots_yet", |s| s.no_snapshots_yet("F5"));
    c.add("no_profiles_yet", |s| s.no_profiles_yet("Ctrl+N"));
    c.add("no_profiles_item", |s| s.no_profiles_item().into());
    c.add("comment_of", |s| plain(&s.comment_of("{date and time}")));
    c.add("comment_saved", |s| s.comment_saved().into());
    c.add("comment_removed", |s| s.comment_removed().into());
    c.add("comment_cancelled", |s| s.comment_cancelled().into());
    c.add("comment_not_saved", |s| s.comment_not_saved("{reason}"));
    c.add("delete_snapshot_question (with a comment)", |s| {
        s.delete_snapshot_question("{date and time}", Some("{comment}"))
    });
    c.add("delete_snapshot_question (without a comment)", |s| {
        s.delete_snapshot_question("{date and time}", None)
    });
    c.add_per_count("delete_snapshots_question", |s, n| {
        s.delete_snapshots_question(n as usize)
    });
    c.add_per_count("snapshots_deleted", |s, n| {
        s.snapshots_deleted(n as usize, "{profile}")
    });
    c.add("snapshots_not_deleted", |s| {
        s.snapshots_not_deleted("{reason}")
    });
    c.add("compacting", |s| s.compacting("{profile}"));
    c.add("compaction_done", |s| {
        s.compaction_done("{profile}", "{size}")
    });
    c.add("compaction_cancelled", |s| {
        s.compaction_cancelled("{profile}")
    });
    c.add("compaction_failed", |s| {
        s.compaction_failed("{profile}", "{reason}")
    });
    c.add("compaction_stopped", |s| s.compaction_stopped().into());

    c.add("focus_after_snapshot", |s| plain(s.focus_after_snapshot()));
    let focus_options = all!(FocusAfterSnapshot {
        IfInList,
        Always,
        Never
    });
    for value in focus_options {
        c.add(format!("focus_option.{value:?}"), |s| {
            s.focus_option(value).into()
        });
    }
    c.add("confirm_snapshot_deletion", |s| {
        plain(s.confirm_snapshot_deletion())
    });
    c.add("settings_saved", |s| s.settings_saved().into());
    c.add("settings_not_saved", |s| s.settings_not_saved("{reason}"));
    c.add("settings_not_opened", |s| {
        s.settings_not_opened("{file}", "{reason}")
    });

    c.add("language", |s| plain(s.language()));
    let languages = all!(Language {
        SameAsWindows,
        English,
        Ukrainian
    });
    for value in languages {
        c.add(format!("language_option.{value:?}"), |s| {
            s.language_option(value).into()
        });
    }
    c.add("language_changes_after_restart", |s| {
        s.language_changes_after_restart().into()
    });
    c.add("restart_now", |s| plain(s.restart_now()));
    c.add("later", |s| plain(s.later()));

    c.add("theme", |s| plain(s.theme()));
    for value in all!(Theme {
        SameAsWindows,
        Light,
        Dark
    }) {
        c.add(format!("theme_option.{value:?}"), |s| {
            s.theme_option(value).into()
        });
    }

    c.add("comparing", |s| s.comparing().into());
    c.add("comparing_percent", |s| s.comparing_percent(45));
    c.add("waiting_for_database", |s| s.waiting_for_database().into());
    c.add("comparison_finished", |s| s.comparison_finished().into());
    c.add("comparison_cancelled", |s| s.comparison_cancelled().into());
    c.add("comparison_failed", |s| s.comparison_failed("{reason}"));
    c.add("comparison_stopped", |s| s.comparison_stopped().into());
    c.add("comment_stopped", |s| s.comment_stopped().into());
    c.add("compare_with_label", |s| {
        plain(&s.compare_with_label("{date and time}"))
    });
    c.add("compare", |s| s.compare().into());
    c.add("focus_after_comparison", |s| {
        plain(s.focus_after_comparison())
    });
    let comparison_focus_options = all!(FocusAfterComparison {
        IfNotMoved,
        Always,
        Never
    });
    for value in comparison_focus_options {
        c.add(format!("comparison_focus_option.{value:?}"), |s| {
            s.comparison_focus_option(value).into()
        });
    }

    c.add("snapshot_saved_comparison_cancelled", |s| {
        s.snapshot_saved_comparison_cancelled().into()
    });
    c.add("nothing_to_compare", |s| s.nothing_to_compare().into());

    let columns = all!(Column {
        Name,
        Change,
        WhatChanged,
        Size,
        DateModified,
        Folder
    });
    for column in columns {
        c.add(format!("column.{column:?}"), |s| s.column(column).into());
    }
    let changes = all!(RowChange {
        Added,
        Deleted,
        Modified,
        Replaced,
        Inaccessible,
    });
    let kinds = all!(EntryKind { File, Dir, Link });
    for kind in kinds {
        for change in changes {
            c.add(format!("change.{change:?}.{kind:?}"), |s| {
                s.change(change, kind).into()
            });
        }
    }
    let fields = all!(Field {
        FileVersion,
        ProductVersion,
        LinkTarget,
        Size,
        DateModified,
        DateCreated,
        Attributes,
        NameCase,
    });
    for field in fields {
        c.add(format!("field.{field:?}"), |s| s.field(field).into());
    }
    let inaccessible_in = all!(InaccessibleIn {
        Earlier,
        Later,
        Both
    });
    for which in inaccessible_in {
        c.add(format!("inaccessible_in.{which:?}"), |s| {
            s.inaccessible_in(which).into()
        });
    }
    c.add_per_count("bytes", |s, n| s.bytes(n, &n.to_string()));
    for power in 1..=6 {
        c.add(format!("size_unit({power})"), |s| s.size_unit(power).into());
    }
    let snapshot = |name: &str, comment: Option<&str>, deleted| SnapshotText {
        name: name.into(),
        comment: comment.map(Into::into),
        deleted,
    };
    let pairs = [
        (
            "pair (with comments)",
            PairText {
                profile: "{profile}".into(),
                profile_deleted: false,
                earlier: snapshot("{earlier}", Some("{comment 1}"), false),
                later: snapshot("{later}", Some("{comment 2}"), false),
            },
        ),
        (
            "pair (without comments)",
            PairText {
                profile: "{profile}".into(),
                profile_deleted: false,
                earlier: snapshot("{earlier}", None, false),
                later: snapshot("{later}", None, false),
            },
        ),
        (
            "pair (all deleted)",
            PairText {
                profile: "{profile}".into(),
                profile_deleted: true,
                earlier: snapshot("{earlier}", Some("{comment 1}"), true),
                later: snapshot("{later}", None, true),
            },
        ),
    ];
    for (id, pair) in &pairs {
        c.add(*id, |s| s.pair(pair));
    }
    let summary = |n: u64, size: SizeChange| SummaryText {
        changes: counted(n * 4),
        added: counted(n),
        deleted: counted(n),
        modified: counted(n * 2),
        replaced: counted(n),
        inaccessible: counted(n),
        size,
        warnings: counted(n),
    };
    c.add("summary (no changes)", |s| {
        s.summary(&summary(0, SizeChange::Unchanged))
    });
    let sizes = [
        SizeChange::Increased("{size}".into()),
        SizeChange::Decreased("{size}".into()),
        SizeChange::Unchanged,
    ];
    each_variant_in!(
        sizes,
        SizeChange {
            Increased,
            Decreased,
            Unchanged
        }
    );
    for size in &sizes {
        for n in [1, 5] {
            c.add(format!("summary({}, {size:?})", n * 4), |s| {
                s.summary(&summary(n, size.clone()))
            });
        }
    }
    // A count of 1 picks the form "one" for all the changes, and 3 "few" for the warnings.
    c.add("summary(1 change, 3 warnings)", |s| {
        s.summary(&SummaryText {
            changes: counted(1),
            added: counted(1),
            deleted: counted(0),
            modified: counted(0),
            replaced: counted(0),
            inaccessible: counted(0),
            size: SizeChange::Increased("{size}".into()),
            warnings: counted(3),
        })
    });
    c.add("showing", |s| s.showing("{shown}", "{total}"));
    let path = || PathBuf::from("{folder}");
    let sides = all!(Side { Earlier, Later });
    let mut warnings = Vec::new();
    for side in sides {
        for warning in [
            Warning::FolderOnlyIn { path: path(), side },
            Warning::FolderNarrowed {
                wider: PathBuf::from("{wider}"),
                side,
                compared: PathBuf::from("{compared}"),
            },
            Warning::FolderMissing { path: path(), side },
            Warning::FolderUnavailable {
                path: path(),
                side,
                reason: Unavailable::Network,
            },
            Warning::ExclusionOnlyIn {
                pattern: "{pattern}".into(),
                side,
            },
        ] {
            warnings.push((
                format!("warning.{}.{side:?}", warning_name(&warning)),
                warning,
            ));
        }
    }
    for warning in [
        Warning::DifferentVolumes {
            path: path(),
            earlier: 1,
            later: 2,
        },
        Warning::ExclusionAppliedDifferently {
            pattern: "{pattern}".into(),
            earlier: Some("{expanded 1}".into()),
            later: Some("{expanded 2}".into()),
        },
        Warning::ExclusionAppliedDifferently {
            pattern: "{pattern}".into(),
            earlier: None,
            later: Some("{expanded}".into()),
        },
    ] {
        warnings.push((format!("warning.{}", warning_name(&warning)), warning));
    }
    let every_warning: Vec<&Warning> = warnings.iter().map(|(_, warning)| warning).collect();
    each_variant_in!(
        every_warning,
        Warning {
            FolderOnlyIn,
            FolderNarrowed,
            FolderMissing,
            FolderUnavailable,
            DifferentVolumes,
            ExclusionOnlyIn,
            ExclusionAppliedDifferently,
        }
    );
    for (id, warning) in &warnings {
        c.add(id.as_str(), |s| s.warning(warning, "{reason}"));
    }
    c.add("no_difference_yet", |s| s.no_difference_yet().into());
    c.add("no_match", |s| s.no_match().into());
    c.add("nothing_compared_yet", |s| {
        s.nothing_compared_yet("F7", "Ctrl+F7")
    });
    c.add("comparison_field", |s| plain(s.comparison_field()));
    c.add("filter_field", |s| plain(s.filter_field()));
    c.add("size_format", |s| plain(s.size_format()));
    let size_options = all!(SizeFormat {
        Explorer,
        Decimal,
        Bytes
    });
    for value in size_options {
        c.add(format!("size_option.{value:?}"), |s| {
            s.size_option(value).into()
        });
    }

    // The difference tree (TASK-33): a file, a folder that is a change, a folder that is
    // none, in the long and in the short text.
    let node = |name: &str, change: Option<(&str, &str)>, inside: Option<(u64, u64)>| NodeText {
        name: name.into(),
        change: change.map(|(change, what)| (change.into(), what.into())),
        inside: inside.map(|(changes, inaccessible)| (counted(changes), counted(inaccessible))),
    };
    let modified = |s: &dyn Strings| s.change(RowChange::Modified, EntryKind::File).to_string();
    let folder_added = |s: &dyn Strings| s.change(RowChange::Added, EntryKind::Dir).to_string();
    let folder_modified =
        |s: &dyn Strings| s.change(RowChange::Modified, EntryKind::Dir).to_string();
    let fields =
        |s: &dyn Strings| format!("{}, {}", s.field(Field::Size), s.field(Field::DateModified));
    c.add("tree_node (a file, long)", |s| {
        s.tree_node(&node("{name}", Some((&modified(s), &fields(s))), None))
    });
    c.add("tree_node (a file, short)", |s| {
        s.tree_node(&node("{name}", Some((&modified(s), "")), None))
    });
    for n in COUNTS {
        c.add(format!("tree_node (a folder added, long)({n})"), |s| {
            s.tree_node(&node("{name}", Some((&folder_added(s), "")), Some((n, 0))))
        });
        c.add(format!("tree_node (a folder, long)({n})"), |s| {
            s.tree_node(&node("{name}", None, Some((n, n))))
        });
    }
    c.add("tree_node (a folder modified, long)", |s| {
        s.tree_node(&node(
            "{name}",
            Some((&folder_modified(s), s.field(Field::Attributes))),
            Some((5, 0)),
        ))
    });
    c.add(
        "tree_node (a folder with inaccessible entries only, long)",
        |s| s.tree_node(&node("{name}", None, Some((0, 3)))),
    );
    c.add("tree_node (a folder added, short)", |s| {
        s.tree_node(&node("{name}", Some((&folder_added(s), "")), None))
    });
    c.add("tree_node (a folder, short)", |s| {
        s.tree_node(&node("{name}", None, None))
    });
    for layout in all!(DifferenceView { List, Tree }) {
        c.add(format!("layout_shown.{layout:?}"), |s| {
            s.layout_shown(layout).into()
        });
    }
    c.add("tree_nodes", |s| plain(s.tree_nodes()));
    for value in all!(TreeNodeText { Long, Short }) {
        c.add(format!("tree_node_option.{value:?}"), |s| {
            s.tree_node_option(value).into()
        });
    }

    let properties = all!(Property {
        Path,
        Name,
        Change,
        Kind,
        Size,
        DateCreated,
        DateModified,
        Attributes,
        FileVersion,
        ProductVersion,
        Target,
        Inaccessible,
        Inside,
    });
    for property in properties {
        c.add(format!("property.{property:?}"), |s| {
            s.property(property).into()
        });
    }
    c.add("before_after", |s| s.before_after("{before}", "{after}"));
    c.add("unchanged", |s| s.unchanged("{value}"));
    c.add("not_counted", |s| s.not_counted("{text}"));
    for change in changes {
        c.add(format!("change_type.{change:?}"), |s| {
            s.change_type(change).into()
        });
    }
    c.add("what_changed", |s| s.what_changed("{fields}"));
    for kind in kinds {
        c.add(format!("kind.{kind:?}"), |s| s.kind(kind).into());
    }
    let attributes = all!(Attribute {
        ReadOnly,
        Hidden,
        System,
        Archive,
        Temporary,
        SparseFile,
        ReparsePoint,
        Compressed,
        Offline,
        NotContentIndexed,
        Encrypted,
        IntegrityStream,
        NoScrubData,
        RecallOnOpen,
        Pinned,
        Unpinned,
        RecallOnDataAccess,
        StrictlySequential,
    });
    for attribute in attributes {
        c.add(format!("attribute.{attribute:?}"), |s| {
            s.attribute(attribute).into()
        });
    }
    c.add("no_attributes", |s| s.no_attributes().into());
    c.add("version_not_read", |s| s.version_not_read().into());
    c.add("no_version", |s| s.no_version().into());
    c.add("target_unknown", |s| s.target_unknown().into());
    for why in all!(Unreadable {
        Folder,
        LinkTarget,
        Unknown
    }) {
        for which in inaccessible_in {
            let inaccessible = Inaccessibility {
                which,
                why,
                absent_in: None,
            };
            c.add(format!("inaccessible_details.{why:?}.{which:?}"), |s| {
                s.inaccessible_details(&inaccessible)
            });
        }
    }
    // An entry inaccessible in one snapshot and absent in the other, for either side.
    for absent_in in sides {
        let (which, absent_side) = match absent_in {
            Side::Earlier => (InaccessibleIn::Later, "earlier"),
            Side::Later => (InaccessibleIn::Earlier, "later"),
        };
        let absent = Inaccessibility {
            which,
            why: Unreadable::Folder,
            absent_in: Some(absent_in),
        };
        c.add(
            format!("inaccessible_details.Folder.{which:?} (absent in the {absent_side})"),
            |s| s.inaccessible_details(&absent),
        );
    }
    c.add("attribute_format", |s| plain(s.attribute_format()));
    let attribute_options = all!(AttributeFormat {
        Words,
        Letters,
        LettersWithCommas
    });
    for value in attribute_options {
        c.add(format!("attribute_option.{value:?}"), |s| {
            s.attribute_option(value).into()
        });
    }
    c.add("warn_missing_entry", |s| plain(s.warn_missing_entry()));
    c.add("entry_gone_question", |s| s.entry_gone_question().into());
    c.add("open", |s| plain(s.open()));
    c.add("no_folder_on_disk", |s| s.no_folder_on_disk("{path}"));
    c.add("explorer_failed", |s| s.explorer_failed("{reason}"));

    c.add("key_space", |s| s.key_space().into());
    c.add("key_up", |s| s.key_up().into());
    c.add("key_down", |s| s.key_down().into());
    c.add("key_left", |s| s.key_left().into());
    c.add("key_right", |s| s.key_right().into());
    c.add("key_context_menu", |s| s.key_context_menu().into());
    c.entries
}

fn warning_name(warning: &Warning) -> &'static str {
    match warning {
        Warning::FolderOnlyIn { .. } => "FolderOnlyIn",
        Warning::FolderNarrowed { .. } => "FolderNarrowed",
        Warning::FolderMissing { .. } => "FolderMissing",
        Warning::FolderUnavailable { .. } => "FolderUnavailable",
        Warning::DifferentVolumes { .. } => "DifferentVolumes",
        Warning::ExclusionOnlyIn { .. } => "ExclusionOnlyIn",
        Warning::ExclusionAppliedDifferently { earlier: None, .. } => {
            "ExclusionAppliedDifferently (did not apply in the earlier)"
        }
        Warning::ExclusionAppliedDifferently { .. } => "ExclusionAppliedDifferently",
    }
}

/// The catalog as text: per string a line with its id, then "en:" and "uk:" lines; the
/// lines of a text with several lines follow, indented.
pub fn catalog() -> String {
    let mut text = String::from(
        "revisor: interface strings in English (en) and Ukrainian (uk).\n\
         {name} is a parameter; a count in parentheses after an id picks the plural form.\n",
    );
    let entries = entries();
    text.push_str(&format!("Strings: {}.\n", entries.len()));
    for entry in entries {
        text.push('\n');
        text.push_str(&entry.id);
        text.push('\n');
        for (lang, value) in [("en", &entry.en), ("uk", &entry.uk)] {
            let mut lines = value.trim_end_matches('\n').split('\n');
            text.push_str(&format!("  {lang}: {}\n", lines.next().unwrap_or_default()));
            for line in lines {
                text.push_str(&format!("      {line}\n"));
            }
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{METHODS, Recorder};

    #[test]
    fn the_catalog_calls_every_method_of_the_trait_in_both_languages() {
        let (en, uk) = (Recorder::new(&En), Recorder::new(&Uk));
        entries_in(&en, &uk);
        assert!(
            METHODS.len() > 100,
            "the trait has its methods: {METHODS:?}"
        );
        for called in [en.called(), uk.called()] {
            // The language of the strings is not a string.
            let missing: Vec<&str> = METHODS
                .iter()
                .copied()
                .filter(|&method| method != "lang" && !called.contains(method))
                .collect();
            assert!(missing.is_empty(), "not in the catalog: {missing:?}");
        }
    }

    #[test]
    fn ids_are_unique_and_plural_forms_show() {
        let entries = entries();
        let mut ids: Vec<&str> = entries.iter().map(|e| e.id.as_str()).collect();
        ids.sort_unstable();
        let count = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), count, "an id is used twice");
        let uk: Vec<&str> = entries
            .iter()
            .filter(|e| e.id.starts_with("snapshot_saved("))
            .map(|e| e.uk.as_str())
            .collect();
        assert_eq!(
            uk,
            [
                "Знімок збережено: 1 запис.",
                "Знімок збережено: 3 записи.",
                "Знімок збережено: 5 записів."
            ]
        );
        let text = catalog();
        assert!(text.contains("\ncommand.NewProfile\n  en: New profile\n  uk: Новий профіль\n"));
    }
}
