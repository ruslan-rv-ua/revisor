use super::*;
use crate::testing::TempDir;
use std::fs;
use std::path::PathBuf;

use crate::store::{ProfileSettings, Store};

/// The settings file in `tmp`, in its subfolder `data`, which does not exist at first.
fn db(tmp: &TempDir) -> PathBuf {
    tmp.path().join("data").join("settings.db")
}

#[test]
fn a_new_settings_file_is_made_with_the_defaults() {
    let tmp = TempDir::new("settings");
    let settings = Settings::open(&db(&tmp)).unwrap();
    assert!(db(&tmp).is_file(), "the file and its folder are created");
    assert_eq!(settings.last_profile().unwrap(), None);
    assert_eq!(
        settings.focus_after_snapshot().unwrap(),
        FocusAfterSnapshot::IfInList
    );
    assert!(settings.confirm_snapshot_deletion().unwrap());
    assert_eq!(settings.language().unwrap(), Language::SameAsWindows);
    // The difference list (TASK-13 #17, #28): sizes as in Explorer, every type, by path.
    assert_eq!(settings.size_format().unwrap(), SizeFormat::Explorer);
    assert_eq!(settings.difference_types().unwrap(), [true; 5]);
    assert_eq!(
        settings.difference_sort().unwrap(),
        DifferenceSort {
            column: SortColumn::Path,
            descending: false
        }
    );
    // The details of a change (TASK-14 #7, #13): attributes in words, and no warning.
    assert_eq!(settings.attribute_format().unwrap(), AttributeFormat::Words);
    assert!(!settings.warn_missing_entry().unwrap());
    // «Шлях скопійовано.» without the path (TASK-41 #8).
    assert!(!settings.show_copied_path().unwrap());
    // Typical folders of the current user only (TASK-43 #5).
    assert!(!settings.other_users_folders().unwrap());
    assert!(!settings.open_after_export().unwrap());
    assert_eq!(settings.export_folder().unwrap(), None);
    // The text report first, and the list separator of Windows in CSV (TASK-40 #1, #5).
    assert_eq!(settings.export_format().unwrap(), ExportFormat::Text);
    assert_eq!(settings.csv_delimiter().unwrap(), CsvDelimiter::Windows);
    // The difference as a list, and the long text of a tree's nodes (TASK-33).
    assert_eq!(settings.difference_view().unwrap(), DifferenceView::List);
    assert_eq!(settings.tree_node_text().unwrap(), TreeNodeText::Long);
    // The theme as Windows has it (TASK-34 #1).
    assert_eq!(settings.theme().unwrap(), Theme::SameAsWindows);
    let conn = rusqlite::Connection::open(db(&tmp)).unwrap();
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, SCHEMA_VERSION);
}

#[test]
fn settings_survive_a_restart() {
    let tmp = TempDir::new("settings");
    {
        let settings = Settings::open(&db(&tmp)).unwrap();
        settings.set_last_profile(Some("Дім.db")).unwrap();
        settings
            .set_focus_after_snapshot(FocusAfterSnapshot::Never)
            .unwrap();
        settings.set_confirm_snapshot_deletion(false).unwrap();
        settings.set_language(Language::Ukrainian).unwrap();
    }
    let settings = Settings::open(&db(&tmp)).unwrap();
    assert_eq!(settings.last_profile().unwrap().as_deref(), Some("Дім.db"));
    assert_eq!(settings.language().unwrap(), Language::Ukrainian);
    assert_eq!(
        settings.focus_after_snapshot().unwrap(),
        FocusAfterSnapshot::Never
    );
    assert!(!settings.confirm_snapshot_deletion().unwrap());

    // Each value can change again, and the last profile can be forgotten.
    settings
        .set_focus_after_snapshot(FocusAfterSnapshot::Always)
        .unwrap();
    settings.set_confirm_snapshot_deletion(true).unwrap();
    settings.set_language(Language::English).unwrap();
    settings.set_last_profile(None).unwrap();
    assert_eq!(settings.language().unwrap(), Language::English);
    assert_eq!(
        settings.focus_after_snapshot().unwrap(),
        FocusAfterSnapshot::Always
    );
    assert!(settings.confirm_snapshot_deletion().unwrap());
    assert_eq!(settings.last_profile().unwrap(), None);
}

#[test]
fn the_view_of_the_difference_list_survives_a_restart() {
    let tmp = TempDir::new("settings");
    let sort = DifferenceSort {
        column: SortColumn::WhatChanged,
        descending: true,
    };
    {
        let settings = Settings::open(&db(&tmp)).unwrap();
        settings.set_size_format(SizeFormat::Bytes).unwrap();
        settings
            .set_difference_types([false, true, false, true, false])
            .unwrap();
        settings.set_difference_sort(sort).unwrap();
    }
    let settings = Settings::open(&db(&tmp)).unwrap();
    assert_eq!(settings.size_format().unwrap(), SizeFormat::Bytes);
    assert_eq!(
        settings.difference_types().unwrap(),
        [false, true, false, true, false]
    );
    assert_eq!(settings.difference_sort().unwrap(), sort);
    // Every type hidden is kept as such, not taken for nothing stored; and so is every
    // type shown again.
    settings.set_difference_types([false; 5]).unwrap();
    assert_eq!(settings.difference_types().unwrap(), [false; 5]);
    settings.set_difference_types([true; 5]).unwrap();
    assert_eq!(settings.difference_types().unwrap(), [true; 5]);
    settings.set_size_format(SizeFormat::Decimal).unwrap();
    assert_eq!(settings.size_format().unwrap(), SizeFormat::Decimal);
    for column in SortColumn::ALL {
        let sort = DifferenceSort {
            column,
            descending: false,
        };
        settings.set_difference_sort(sort).unwrap();
        assert_eq!(settings.difference_sort().unwrap(), sort);
    }
}

#[test]
fn a_type_added_since_the_filter_was_stored_is_shown() {
    // TASK-42 #11. Until 0.3 the setting named the four types shown: a user who hid the
    // deleted and the inaccessible rows then has renamed and moved entries shown in 0.4.
    let tmp = TempDir::new("settings");
    let settings = Settings::open(&db(&tmp)).unwrap();
    let shown = |added, deleted, modified, moved, inaccessible| {
        [added, deleted, modified, moved, inaccessible]
    };
    settings
        .set(KEY_DIFFERENCE_TYPES, "added,modified")
        .unwrap();
    assert_eq!(
        settings.difference_types().unwrap(),
        shown(true, false, true, true, false)
    );
    // Every type hidden then, as an empty text.
    settings.set(KEY_DIFFERENCE_TYPES, "").unwrap();
    assert_eq!(
        settings.difference_types().unwrap(),
        shown(false, false, false, true, false)
    );
    // What 0.4 stores wins from then on: the types hidden, the new one among them.
    settings
        .set_difference_types(shown(true, true, true, false, true))
        .unwrap();
    assert_eq!(
        settings.difference_types().unwrap(),
        shown(true, true, true, false, true)
    );
    // A type a later revisor hides, which this one does not know, hides nothing here.
    settings
        .set(KEY_DIFFERENCE_HIDDEN_TYPES, "deleted,hashed")
        .unwrap();
    assert_eq!(
        settings.difference_types().unwrap(),
        shown(true, false, true, true, true)
    );
}

#[test]
fn the_settings_of_the_details_survive_a_restart() {
    let tmp = TempDir::new("settings");
    {
        let settings = Settings::open(&db(&tmp)).unwrap();
        settings
            .set_attribute_format(AttributeFormat::Letters)
            .unwrap();
        settings.set_warn_missing_entry(true).unwrap();
    }
    let settings = Settings::open(&db(&tmp)).unwrap();
    assert_eq!(
        settings.attribute_format().unwrap(),
        AttributeFormat::Letters
    );
    assert!(settings.warn_missing_entry().unwrap());
    settings
        .set_attribute_format(AttributeFormat::LettersWithCommas)
        .unwrap();
    assert_eq!(
        settings.attribute_format().unwrap(),
        AttributeFormat::LettersWithCommas
    );
    settings
        .set_attribute_format(AttributeFormat::Words)
        .unwrap();
    settings.set_warn_missing_entry(false).unwrap();
    assert_eq!(settings.attribute_format().unwrap(), AttributeFormat::Words);
    assert!(!settings.warn_missing_entry().unwrap());
}

#[test]
fn showing_other_users_folders_survives_a_restart() {
    // TASK-43 #5.
    let tmp = TempDir::new("settings");
    Settings::open(&db(&tmp))
        .unwrap()
        .set_other_users_folders(true)
        .unwrap();
    let settings = Settings::open(&db(&tmp)).unwrap();
    assert!(settings.other_users_folders().unwrap());
    settings.set_other_users_folders(false).unwrap();
    assert!(
        !Settings::open(&db(&tmp))
            .unwrap()
            .other_users_folders()
            .unwrap()
    );
}

#[test]
fn showing_the_copied_path_survives_a_restart() {
    // TASK-41 #8.
    let tmp = TempDir::new("settings");
    Settings::open(&db(&tmp))
        .unwrap()
        .set_show_copied_path(true)
        .unwrap();
    let settings = Settings::open(&db(&tmp)).unwrap();
    assert!(settings.show_copied_path().unwrap());
    settings.set_show_copied_path(false).unwrap();
    assert!(
        !Settings::open(&db(&tmp))
            .unwrap()
            .show_copied_path()
            .unwrap()
    );
}

#[test]
fn the_export_settings_survive_a_restart() {
    // TASK-39 #2, #9.
    let tmp = TempDir::new("settings");
    let folder = std::path::Path::new(r"D:\Звіти");
    {
        let settings = Settings::open(&db(&tmp)).unwrap();
        settings.set_open_after_export(true).unwrap();
        settings.set_export_folder(folder).unwrap();
    }
    let settings = Settings::open(&db(&tmp)).unwrap();
    assert!(settings.open_after_export().unwrap());
    assert_eq!(settings.export_folder().unwrap().as_deref(), Some(folder));
}

#[test]
fn the_export_format_and_the_csv_delimiter_survive_a_restart() {
    // TASK-40 #1, #5.
    for delimiter in CsvDelimiter::ALL {
        let tmp = TempDir::new("settings");
        {
            let settings = Settings::open(&db(&tmp)).unwrap();
            settings.set_export_format(ExportFormat::Csv).unwrap();
            settings.set_csv_delimiter(delimiter).unwrap();
        }
        let settings = Settings::open(&db(&tmp)).unwrap();
        assert_eq!(settings.export_format().unwrap(), ExportFormat::Csv);
        assert_eq!(settings.csv_delimiter().unwrap(), delimiter);
    }
    let codes: Vec<&str> = CsvDelimiter::ALL.map(CsvDelimiter::as_str).to_vec();
    assert_eq!(codes, ["windows", "comma", "semicolon", "tab"]);
    let chars = CsvDelimiter::ALL.map(CsvDelimiter::char);
    assert_eq!(chars, [None, Some(','), Some(';'), Some('\t')]);
    assert_eq!(ExportFormat::parse("csv"), Some(ExportFormat::Csv));
    assert_eq!(ExportFormat::parse("xlsx"), None);
}

#[test]
fn the_view_and_the_tree_node_text_survive_a_restart() {
    let tmp = TempDir::new("settings");
    {
        let settings = Settings::open(&db(&tmp)).unwrap();
        settings.set_difference_view(DifferenceView::Tree).unwrap();
        settings.set_tree_node_text(TreeNodeText::Short).unwrap();
    }
    let settings = Settings::open(&db(&tmp)).unwrap();
    assert_eq!(settings.difference_view().unwrap(), DifferenceView::Tree);
    assert_eq!(settings.tree_node_text().unwrap(), TreeNodeText::Short);
    settings.set_difference_view(DifferenceView::List).unwrap();
    settings.set_tree_node_text(TreeNodeText::Long).unwrap();
    assert_eq!(settings.difference_view().unwrap(), DifferenceView::List);
    assert_eq!(settings.tree_node_text().unwrap(), TreeNodeText::Long);
    // A value no revisor wrote falls back to the default.
    settings.set(KEY_DIFFERENCE_VIEW, "grid").unwrap();
    settings.set(KEY_TREE_NODE_TEXT, 3).unwrap();
    assert_eq!(settings.difference_view().unwrap(), DifferenceView::List);
    assert_eq!(settings.tree_node_text().unwrap(), TreeNodeText::Long);
    for value in DifferenceView::ALL {
        assert_eq!(DifferenceView::parse(value.as_str()), Some(value));
    }
    for value in TreeNodeText::ALL {
        assert_eq!(TreeNodeText::parse(value.as_str()), Some(value));
    }
    assert_eq!(DifferenceView::ALL[0], DifferenceView::default());
    assert_eq!(TreeNodeText::ALL[0], TreeNodeText::default());
}

#[test]
fn the_theme_survives_a_restart() {
    // TASK-34 #1: revisor starts with the theme chosen.
    let tmp = TempDir::new("settings");
    {
        let settings = Settings::open(&db(&tmp)).unwrap();
        settings.set_theme(Theme::Dark).unwrap();
    }
    let settings = Settings::open(&db(&tmp)).unwrap();
    assert_eq!(settings.theme().unwrap(), Theme::Dark);
    settings.set_theme(Theme::Light).unwrap();
    assert_eq!(settings.theme().unwrap(), Theme::Light);
    // A value no revisor wrote falls back to the default.
    settings.set(KEY_THEME, "sepia").unwrap();
    assert_eq!(settings.theme().unwrap(), Theme::SameAsWindows);
    settings.set(KEY_THEME, 1).unwrap();
    assert_eq!(settings.theme().unwrap(), Theme::SameAsWindows);
    for value in Theme::ALL {
        assert_eq!(Theme::parse(value.as_str()), Some(value));
    }
    assert_eq!(Theme::ALL[0], Theme::default());
}

#[test]
fn values_are_stored_by_name() {
    for value in SizeFormat::ALL {
        assert_eq!(SizeFormat::parse(value.as_str()), Some(value));
    }
    for value in AttributeFormat::ALL {
        assert_eq!(AttributeFormat::parse(value.as_str()), Some(value));
    }
    assert_eq!(AttributeFormat::ALL[0], AttributeFormat::default());
    assert_eq!(AttributeFormat::parse("hex"), None);
    for value in ChangeType::ALL {
        assert_eq!(ChangeType::parse(value.as_str()), Some(value));
    }
    for value in SortColumn::ALL {
        assert_eq!(SortColumn::parse(value.as_str()), Some(value));
    }
    assert_eq!(SortColumn::parse("colour"), None);
    // The check boxes index the shown types by their order.
    for (i, kind) in ChangeType::ALL.into_iter().enumerate() {
        assert_eq!(kind as usize, i);
    }
}

#[test]
fn two_processes_share_one_settings_file() {
    let tmp = TempDir::new("settings");
    let first = Settings::open(&db(&tmp)).unwrap();
    let second = Settings::open(&db(&tmp)).unwrap();
    first.set_last_profile(Some("Робота.db")).unwrap();
    assert_eq!(second.last_profile().unwrap().as_deref(), Some("Робота.db"));
    second.set_confirm_snapshot_deletion(false).unwrap();
    assert!(!first.confirm_snapshot_deletion().unwrap());
}

#[test]
fn a_value_no_revisor_wrote_falls_back_to_the_default() {
    let tmp = TempDir::new("settings");
    drop(Settings::open(&db(&tmp)).unwrap());
    let conn = rusqlite::Connection::open(db(&tmp)).unwrap();
    conn.execute(
        "INSERT OR REPLACE INTO setting VALUES ('focus_after_snapshot', 'sometimes')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT OR REPLACE INTO setting VALUES ('confirm_snapshot_deletion', 'yes')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT OR REPLACE INTO setting VALUES ('language', 'fr')",
        [],
    )
    .unwrap();
    drop(conn);
    let settings = Settings::open(&db(&tmp)).unwrap();
    assert_eq!(
        settings.focus_after_snapshot().unwrap(),
        FocusAfterSnapshot::IfInList
    );
    assert!(settings.confirm_snapshot_deletion().unwrap());
    assert_eq!(settings.language().unwrap(), Language::SameAsWindows);
}

#[test]
fn files_that_are_not_revisor_settings_are_refused() {
    let tmp = TempDir::new("settings");
    fs::create_dir_all(db(&tmp).parent().unwrap()).unwrap();

    // A text file is not a database at all.
    fs::write(db(&tmp), b"some text, not a database").unwrap();
    assert!(matches!(
        Settings::open(&db(&tmp)),
        Err(SettingsError::NotSettings)
    ));
    fs::remove_file(db(&tmp)).unwrap();

    // Another program's database has tables but not revisor's mark.
    let other = rusqlite::Connection::open(db(&tmp)).unwrap();
    other.execute_batch("CREATE TABLE t (x)").unwrap();
    drop(other);
    assert!(matches!(
        Settings::open(&db(&tmp)),
        Err(SettingsError::NotSettings)
    ));
    fs::remove_file(db(&tmp)).unwrap();

    // A profile database is not the settings database either.
    drop(Store::create(&db(&tmp), &ProfileSettings::new_profile()).unwrap());
    assert!(matches!(
        Settings::open(&db(&tmp)),
        Err(SettingsError::NotSettings)
    ));
}

#[test]
fn settings_from_a_newer_revisor_are_refused() {
    let tmp = TempDir::new("settings");
    drop(Settings::open(&db(&tmp)).unwrap());
    let conn = rusqlite::Connection::open(db(&tmp)).unwrap();
    conn.pragma_update(None, "user_version", SCHEMA_VERSION + 1)
        .unwrap();
    drop(conn);
    assert!(matches!(
        Settings::open(&db(&tmp)),
        Err(SettingsError::NewerSchema(v)) if v == SCHEMA_VERSION + 1
    ));
}

#[test]
fn the_focus_setting_names_round_trip() {
    for value in FocusAfterSnapshot::ALL {
        assert_eq!(FocusAfterSnapshot::parse(value.as_str()), Some(value));
    }
    assert_eq!(FocusAfterSnapshot::parse("sometimes"), None);
    assert_eq!(FocusAfterSnapshot::default(), FocusAfterSnapshot::IfInList);
}

#[test]
fn the_comparison_focus_setting_is_kept_and_named() {
    let tmp = TempDir::new("settings");
    let settings = Settings::open(&db(&tmp)).unwrap();
    assert_eq!(
        settings.focus_after_comparison().unwrap(),
        FocusAfterComparison::IfNotMoved
    );
    settings
        .set_focus_after_comparison(FocusAfterComparison::Never)
        .unwrap();
    drop(settings);
    let settings = Settings::open(&db(&tmp)).unwrap();
    assert_eq!(
        settings.focus_after_comparison().unwrap(),
        FocusAfterComparison::Never
    );
    for value in FocusAfterComparison::ALL {
        assert_eq!(FocusAfterComparison::parse(value.as_str()), Some(value));
    }
    assert_eq!(FocusAfterComparison::parse("sometimes"), None);
    assert_eq!(
        FocusAfterComparison::ALL[0],
        FocusAfterComparison::default()
    );
}
