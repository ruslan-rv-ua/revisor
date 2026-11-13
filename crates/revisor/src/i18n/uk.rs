//! Ukrainian interface strings. Key names follow the Microsoft Ukrainian Style Guide
//! (document «Клавіатура й меню», section «Назви клавіш»).

use revisor_core::compare::{Side, Warning};
use revisor_core::exclude::PatternError;
use revisor_core::scan::EntryKind;
use revisor_core::settings::{
    AttributeFormat, ChangeType, DifferenceView, FocusAfterComparison, FocusAfterSnapshot,
    Language, SizeFormat, Theme, TreeNodeText,
};
use revisor_core::typical::TypicalFolder;

use super::plural::{Category, category};
use super::{Failure, Lang, Strings, error_code, in_sentence, lower_first};
use crate::commands::{CommandId, MenuId, Pane, Reason, SubmenuId};
use crate::details::{Attribute, Inaccessibility, Property, Unreadable};
use crate::difference::{
    Column, Counted, Field, InaccessibleIn, NodeText, PairText, RowChange, SizeChange,
    SnapshotText, SummaryText,
};

pub struct Uk;

impl Uk {
    /// "1 запис", "3 записи", "12 345 записів".
    fn entries(&self, entries: u64, entries_text: &str) -> String {
        match category(Lang::Uk, entries.into()) {
            Category::One => format!("{entries_text} запис"),
            Category::Few => format!("{entries_text} записи"),
            _ => format!("{entries_text} записів"),
        }
    }

    /// "1 знімок", "3 знімки", "5 знімків".
    fn snapshots(&self, count: usize) -> String {
        match category(Lang::Uk, (count as u64).into()) {
            Category::One => format!("{count} знімок"),
            Category::Few => format!("{count} знімки"),
            _ => format!("{count} знімків"),
        }
    }

    /// A count with the noun in its form: `one` for 1 and 21, `few` for 2–4 and 22–24,
    /// `many` for the rest: "1 зміна", "3 зміни", "20 змін".
    fn counted(&self, count: &Counted, one: &str, few: &str, many: &str) -> String {
        let noun = match category(Lang::Uk, count.n.into()) {
            Category::One => one,
            Category::Few => few,
            _ => many,
        };
        format!("{} {noun}", count.text)
    }

    /// The snapshot of `side`, in the locative: «у ранішому знімку».
    fn side(&self, side: Side) -> &'static str {
        match side {
            Side::Earlier => "ранішому",
            Side::Later => "пізнішому",
        }
    }
}

impl Strings for Uk {
    fn lang(&self) -> Lang {
        Lang::Uk
    }

    fn command(&self, id: CommandId) -> &'static str {
        match id {
            CommandId::NewProfile => "Новий профіль",
            CommandId::EditProfile => "Редагувати профіль",
            CommandId::DeleteProfile => "Видалити профіль",
            CommandId::Settings => "Налаштування",
            CommandId::RestartAsAdmin => "Перезапустити від імені адміністратора",
            CommandId::Exit => "Вихід",
            CommandId::TakeSnapshot => "Зробити знімок",
            CommandId::SnapshotAndCompare => "Знімок + порівняння",
            CommandId::Comment => "Коментар",
            CommandId::DeleteSnapshots => "Видалити знімки",
            CommandId::CancelSnapshot => "Скасувати знімок",
            CommandId::CancelCompaction => "Скасувати стискання",
            CommandId::CompareWithPrevious => "Порівняти з попереднім",
            CommandId::CompareWith => "Порівняти з",
            CommandId::CancelComparison => "Скасувати порівняння",
            CommandId::CancelComment => "Скасувати коментар",
            CommandId::Details => "Подробиці",
            CommandId::ShowInExplorer => "Показати в Провіднику",
            CommandId::Export => "Експортувати",
            CommandId::CancelExport => "Скасувати експорт",
            CommandId::FilterByText => "Фільтрувати за текстом",
            CommandId::ShowAdded => "Додані",
            CommandId::ShowDeleted => "Видалені",
            CommandId::ShowModified => "Змінені",
            CommandId::ShowMoved => "Перейменовані чи переміщені",
            CommandId::ShowInaccessible => "Недоступні",
            CommandId::SortByPath => "Шлях",
            CommandId::SortByName => "Ім'я",
            CommandId::SortByChange => "Зміна",
            CommandId::SortByWhatChanged => "Що змінилося",
            CommandId::SortBySize => "Розмір",
            CommandId::SortByDateModified => "Дата зміни",
            CommandId::SortByFolder => "Тека",
            CommandId::SortAscending => "За зростанням",
            CommandId::SortDescending => "За спаданням",
            CommandId::ViewAsList => "Списком",
            CommandId::ViewAsTree => "Деревом",
            CommandId::KeyboardShortcuts => "Гарячі клавіші",
            CommandId::About => "Про revisor",
        }
    }

    fn menu(&self, id: MenuId) -> &'static str {
        match id {
            MenuId::File => "Файл",
            MenuId::Snapshot => "Знімок",
            MenuId::Compare => "Порівняння",
            MenuId::Difference => "Різниця",
            MenuId::Help => "Довідка",
        }
    }

    fn submenu(&self, id: SubmenuId) -> &'static str {
        match id {
            SubmenuId::SortBy => "Сортувати за",
        }
    }

    fn reason(&self, reason: Reason) -> &'static str {
        match reason {
            Reason::NoProfiles => "профілів ще немає",
            Reason::NoSnapshots => "знімків ще немає",
            Reason::NoDifference => "різниці ще немає",
            Reason::SnapshotRunning => "триває знімок",
            Reason::SnapshotOfThisProfile => "триває знімок цього профілю",
            Reason::SnapshotNotRunning => "знімок зараз не триває",
            Reason::ComparisonNotRunning => "порівняння зараз не триває",
            Reason::CompactionNotRunning => "стискання зараз не триває",
            Reason::CommentWaiting => "коментар ще чекає на базу профілю",
            Reason::CommentNotWaiting => "жоден коментар не чекає на базу профілю",
            Reason::CompactionRunning => "триває стискання бази, а водночас іде лише одне",
            Reason::CompactionOfThisProfile => "триває стискання бази цього профілю",
            Reason::NoSelection => "жодного знімка не виділено",
            Reason::NoPreviousSnapshot => "попереднього знімка немає",
            Reason::NoOtherSnapshot => "іншого знімка в профілі немає",
            Reason::AlreadyAdmin => "revisor уже працює від імені адміністратора",
            Reason::NoChangeShown => "у списку різниці немає змін",
            Reason::TreeSortedByName => "у дереві вузли завжди впорядковано за ім'ям",
            Reason::MoreNode => "вузол «Ще…» — не файл і не тека",
            Reason::ExportRunning => "триває експорт, а водночас іде лише один",
            Reason::ExportNotRunning => "експорт зараз не триває",
        }
    }

    fn unavailable(&self, command: &str, reason: &str) -> String {
        format!("{command}: {reason}.")
    }

    fn key_only_in(&self, key: &str, pane: Pane) -> String {
        let pane = match pane {
            Pane::Snapshots => "переліку знімків",
            Pane::Difference => "списку різниці",
        };
        format!("Клавіша {key} діє лише в {pane}.")
    }

    fn only_in(&self, pane: Pane) -> String {
        match pane {
            Pane::Snapshots => "(лише в переліку знімків)".into(),
            Pane::Difference => "(лише в списку різниці)".into(),
        }
    }

    fn in_list(&self, key: &str, pane: Pane) -> String {
        match pane {
            Pane::Snapshots => format!("{key} у переліку знімків"),
            Pane::Difference => format!("{key} у списку чи дереві різниці"),
        }
    }

    fn either(&self, first: &str, second: &str) -> String {
        format!("{first} чи {}", lower_first(second))
    }

    fn navigation(&self) -> &'static str {
        "Переходи"
    }

    fn switch_panes(&self) -> &'static str {
        "Перейти між переліком знімків і списком різниці"
    }

    fn context_menu(&self) -> &'static str {
        "Контекстне меню"
    }

    fn pane(&self, pane: Pane) -> &'static str {
        match pane {
            Pane::Snapshots => "Знімки",
            Pane::Difference => "Різниця",
        }
    }

    fn profile(&self) -> &'static str {
        "Профіль"
    }

    fn no_actions(&self) -> &'static str {
        "Тут зараз немає доступних дій."
    }

    fn close(&self) -> &'static str {
        "Закрити"
    }

    fn close_window(&self) -> &'static str {
        "Закрити"
    }

    fn about(&self, version: &str) -> String {
        format!(
            "revisor {version}\nЛіцензія: MIT\nhttps://github.com/ruslan-rv-ua/revisor\n\n\
             Сторонні компоненти та їхні ліцензії:\n"
        )
    }

    fn profile_name(&self) -> &'static str {
        "Назва"
    }

    fn profile_folders(&self) -> &'static str {
        "Теки"
    }

    fn add_folder(&self) -> &'static str {
        "Додати теку"
    }

    fn remove_folder(&self) -> &'static str {
        "Прибрати теку"
    }

    fn add_typical_folders(&self) -> &'static str {
        "Додати типові теки"
    }

    fn typical_folder(&self, folder: TypicalFolder) -> &'static str {
        match folder {
            TypicalFolder::ProgramFiles => "Program Files",
            TypicalFolder::ProgramFilesX86 => "Program Files (x86)",
            TypicalFolder::ProgramData => "ProgramData",
            TypicalFolder::Windows => "Windows",
            TypicalFolder::PublicDesktop => "Загальний робочий стіл",
            TypicalFolder::Profile => "Тека користувача",
            TypicalFolder::AppData => "AppData користувача",
            TypicalFolder::Desktop => "Робочий стіл користувача",
        }
    }

    fn other_user_folder(&self, folder: TypicalFolder, user: &str) -> String {
        format!("{} {user}", self.typical_folder(folder))
    }

    fn add(&self) -> &'static str {
        "Додати"
    }

    fn typical_folders_in_profile(&self) -> &'static str {
        "Усі типові теки вже є в профілі."
    }

    fn other_users_need_admin(&self) -> &'static str {
        "Теки інших користувачів видно лише від імені адміністратора."
    }

    fn profile_exclusions(&self) -> &'static str {
        "Винятки"
    }

    fn create(&self) -> &'static str {
        "Створити"
    }

    fn ok(&self) -> &'static str {
        "OK"
    }

    fn cancel(&self) -> &'static str {
        "Скасувати"
    }

    fn cancel_job(&self) -> &'static str {
        "Скасувати"
    }

    fn type_box(&self, kind: ChangeType) -> &'static str {
        match kind {
            ChangeType::Added => "Додані",
            ChangeType::Deleted => "Видалені",
            ChangeType::Modified => "Змінені",
            ChangeType::Moved => "Перейменовані чи переміщені",
            ChangeType::Inaccessible => "Недоступні",
        }
    }

    fn choose_folder(&self) -> &'static str {
        "Виберіть теку для профілю"
    }

    fn name_missing(&self) -> &'static str {
        "Введіть назву профілю."
    }

    fn name_taken(&self, name: &str) -> String {
        format!("Профіль «{name}» уже є. Виберіть іншу назву.")
    }

    fn no_folders(&self) -> &'static str {
        "Додайте до профілю хоча б одну теку."
    }

    fn bad_pattern(&self, line: usize, pattern: &str, error: PatternError) -> String {
        let why = match error {
            PatternError::NotFullPath => "шаблон із \\ — це повний шлях, тож він починається \
                з диска, як-от C:\\, чи зі змінної середовища, як-от %TEMP%"
                .to_string(),
            PatternError::Slash => "у шляхах пишуть \\, а не /".to_string(),
            PatternError::Colon => {
                "двокрапка буває лише після літери диска на початку шляху, як у C:\\".to_string()
            }
            PatternError::Control(c) => {
                format!("у ньому є керівний символ (U+{:04X})", u32::from(c))
            }
            PatternError::BadChar(c) => format!("в іменах файлів і тек не буває символу {c}"),
        };
        format!("Рядок {line} у винятках, «{pattern}»: {why}.")
    }

    fn folder_covered(&self, folder: &str, by: &str) -> String {
        format!("Теку «{folder}» не додано: її файли вже охоплює тека профілю «{by}».")
    }

    fn folder_not_ntfs(&self, folder: &str, file_system: &str) -> String {
        format!(
            "Теку «{folder}» не можна додати: revisor підтримує лише томи NTFS, а ця тека \
             на томі {file_system}."
        )
    }

    fn folder_on_network(&self, folder: &str) -> String {
        format!(
            "Теку «{folder}» не можна додати: revisor підтримує лише томи NTFS цього \
             комп'ютера, а це мережева тека."
        )
    }

    fn folder_not_examined(&self, folder: &str, reason: &str) -> String {
        format!("Теку «{folder}» не можна додати, бо revisor не може її перевірити: {reason}.")
    }

    fn replace_folders(&self, folder: &str, inside: &[String]) -> String {
        let inside: Vec<String> = inside.iter().map(|f| format!("«{f}»")).collect();
        format!(
            "Тека «{folder}» містить теки профілю: {}. Замінити їх нею?",
            inside.join(", ")
        )
    }

    fn replace(&self) -> &'static str {
        "Замінити"
    }

    fn delete_profile_question(&self, name: &str, snapshots: usize) -> String {
        if snapshots == 0 {
            return format!(
                "Видалити профіль «{name}»? Знімків у ньому немає. Цю дію не можна скасувати."
            );
        }
        format!(
            "Видалити профіль «{name}» і {} у ньому? Цю дію не можна скасувати.",
            self.snapshots(snapshots)
        )
    }

    fn delete(&self) -> &'static str {
        "Видалити"
    }

    fn profile_created(&self, name: &str) -> String {
        format!("Профіль «{name}» створено.")
    }

    fn profile_changed(&self, name: &str) -> String {
        format!("Профіль «{name}» змінено.")
    }

    fn profile_deleted(&self, name: &str) -> String {
        format!("Профіль «{name}» видалено.")
    }

    fn profile_not_saved(&self, reason: &str) -> String {
        format!("Профіль не збережено: {reason}.")
    }

    fn profile_not_deleted(&self, name: &str, reason: &str) -> String {
        format!("Профіль «{name}» не видалено: {reason}.")
    }

    fn profile_not_opened(&self, file: &str, reason: &str) -> String {
        format!("Файл профілю «{file}» не вдалося відкрити: {reason}.")
    }

    fn profiles_not_listed(&self, folder: &str, reason: &str) -> String {
        format!("Теку профілів «{folder}» не вдалося прочитати: {reason}.")
    }

    fn failure(&self, failure: &Failure) -> String {
        match failure {
            Failure::AccessDenied => "немає доступу — перевірте права на цей файл чи теку".into(),
            Failure::InUse => "файл відкрито в іншій програмі, можливо, в іншій копії \
                revisor — закрийте її й спробуйте ще раз"
                .into(),
            Failure::DiskFull => "на диску немає місця — звільніть місце й спробуйте ще раз".into(),
            Failure::NewerVersion => {
                "профіль створила новіша версія revisor — оновіть revisor".into()
            }
            Failure::NotAProfile => {
                "це не файл профілю revisor — перенесіть його з теки профілів".into()
            }
            Failure::NotSettings => {
                "це не файл налаштувань revisor — перенесіть його з теки data".into()
            }
            Failure::Cancelled => "дію скасовано".into(),
            Failure::SnapshotDeleted => "знімок тим часом видалено".into(),
            Failure::Other { text, code } => {
                format!("{} (код {})", in_sentence(text), error_code(*code))
            }
            Failure::Unexpected(text) => format!("сталася непередбачена помилка ({text})"),
        }
    }

    fn progress(&self) -> &'static str {
        "Прогрес"
    }

    fn snapshot_progress(&self, entries: u64, entries_text: &str, folder: &str) -> String {
        let entries = self.entries(entries, entries_text);
        if folder.is_empty() {
            entries
        } else {
            format!("{entries}, читається {folder}")
        }
    }

    fn snapshot_saving(&self, entries: u64, entries_text: &str) -> String {
        format!(
            "Збереження знімка: {}…",
            self.entries(entries, entries_text)
        )
    }

    fn snapshot_started(&self) -> &'static str {
        "Знімок розпочато."
    }

    fn snapshot_saved(&self, entries: u64, entries_text: &str) -> String {
        format!("Знімок збережено: {}.", self.entries(entries, entries_text))
    }

    fn entries_inaccessible(&self, entries_text: &str) -> String {
        format!("Недоступних записів: {entries_text}.")
    }

    fn folders_not_found(&self, folders: &[(String, String)]) -> String {
        let named: Vec<String> = folders
            .iter()
            .map(|(path, why)| format!("«{path}» ({why})"))
            .collect();
        format!("Тек не знайдено: {}.", named.join("; "))
    }

    fn folder_missing(&self) -> &'static str {
        "теки немає"
    }

    fn folder_volume_not_ntfs(&self, file_system: &str) -> String {
        format!("тека на томі {file_system}, не NTFS")
    }

    fn folder_volume_network(&self) -> &'static str {
        "тека на мережевому ресурсі"
    }

    fn folder_drive_not_connected(&self) -> &'static str {
        "диск теки не підключено"
    }

    fn folder_is_a_file(&self) -> &'static str {
        "за цим шляхом файл, а не тека"
    }

    fn folder_unreachable(&self, reason: &str) -> String {
        format!("теку не прочитати: {reason}")
    }

    fn snapshot_cancelled(&self) -> &'static str {
        "Знімок скасовано."
    }

    fn snapshot_failed(&self, reason: &str) -> String {
        format!("Знімок не збережено: {reason}.")
    }

    fn snapshot_stopped(&self) -> &'static str {
        "знімок несподівано зупинився"
    }

    fn restart_question(&self, denied_text: &str) -> String {
        format!(
            "Записів, недоступних через права: {denied_text}. Перезапустити revisor від імені \
             адміністратора? Він знову відкриє цей профіль; тоді зробіть новий знімок, а цей \
             лишиться в переліку."
        )
    }

    fn restart(&self) -> &'static str {
        "Перезапустити"
    }

    fn close_during_snapshot(&self) -> &'static str {
        "Триває знімок, і закриття revisor його скасує. Закрити revisor?"
    }

    fn restart_during_snapshot(&self) -> &'static str {
        "Триває знімок, і перезапуск revisor його скасує. Перезапустити від імені \
         адміністратора?"
    }

    fn restart_failed(&self, reason: &str) -> String {
        format!("revisor не перезапущено: {reason}.")
    }

    fn column_date_time(&self) -> &'static str {
        "Дата й час"
    }

    fn column_comment(&self) -> &'static str {
        "Коментар"
    }

    fn no_snapshots_yet(&self, key: &str) -> String {
        format!("Знімків ще немає. Натисніть клавішу {key}, щоб зробити перший знімок.")
    }

    fn no_profiles_yet(&self, key: &str) -> String {
        format!("Профілів ще немає. Натисніть клавіші {key}, щоб створити перший профіль.")
    }

    fn no_profiles_item(&self) -> &'static str {
        "Профілів ще немає"
    }

    fn comment_of(&self, snapshot: &str) -> String {
        format!("Коментар до знімка {snapshot}")
    }

    fn comment_saved(&self) -> &'static str {
        "Коментар збережено."
    }

    fn comment_removed(&self) -> &'static str {
        "Коментар прибрано."
    }

    fn comment_cancelled(&self) -> &'static str {
        "Коментар скасовано."
    }

    fn comment_not_saved(&self, reason: &str) -> String {
        format!("Коментар не збережено: {reason}.")
    }

    fn delete_snapshot_question(&self, name: &str, comment: Option<&str>) -> String {
        match comment {
            Some(comment) => {
                format!("Видалити знімок {name} «{comment}»? Цю дію не можна скасувати.")
            }
            None => format!("Видалити знімок {name}? Цю дію не можна скасувати."),
        }
    }

    fn delete_snapshots_question(&self, count: usize) -> String {
        format!(
            "Видалити {}? Цю дію не можна скасувати.",
            self.snapshots(count)
        )
    }

    fn snapshots_deleted(&self, count: usize, profile: &str) -> String {
        format!(
            "Видалено {}; триває стискання бази профілю «{profile}».",
            self.snapshots(count)
        )
    }

    fn snapshots_not_deleted(&self, reason: &str) -> String {
        format!("Знімки не видалено: {reason}.")
    }

    fn compacting(&self, profile: &str) -> String {
        format!("Стискання бази профілю «{profile}»…")
    }

    fn compaction_done(&self, profile: &str, freed: &str) -> String {
        format!("Базу профілю «{profile}» стиснуто: звільнено {freed}.")
    }

    fn compaction_cancelled(&self, profile: &str) -> String {
        format!(
            "Стискання бази профілю «{profile}» скасовано. Знімки видалено, а їхнє місце \
             займуть наступні знімки."
        )
    }

    fn compaction_failed(&self, profile: &str, reason: &str) -> String {
        format!(
            "Базу профілю «{profile}» не стиснуто: {reason}. Знімки видалено, а їхнє місце \
             займуть наступні знімки."
        )
    }

    fn compaction_stopped(&self) -> &'static str {
        "стискання несподівано зупинилося"
    }

    fn focus_after_snapshot(&self) -> &'static str {
        "Фокус після знімка"
    }

    fn focus_option(&self, value: FocusAfterSnapshot) -> &'static str {
        match value {
            FocusAfterSnapshot::IfInList => "Якщо фокус у переліку",
            FocusAfterSnapshot::Always => "Завжди",
            FocusAfterSnapshot::Never => "Ніколи",
        }
    }

    fn confirm_snapshot_deletion(&self) -> &'static str {
        "Підтверджувати видалення знімків"
    }

    fn settings_saved(&self) -> &'static str {
        "Налаштування збережено."
    }

    fn settings_not_saved(&self, reason: &str) -> String {
        format!("Налаштування не збережено: {reason}.")
    }

    fn settings_not_opened(&self, file: &str, reason: &str) -> String {
        format!(
            "Файл налаштувань «{file}» не вдалося відкрити: {reason}. Використано типові \
             налаштування, а їхні зміни не зберігаються."
        )
    }

    fn language(&self) -> &'static str {
        "Мова"
    }

    fn language_option(&self, value: Language) -> &'static str {
        match value {
            Language::SameAsWindows => "Як у Windows",
            Language::English => "English",
            Language::Ukrainian => "Українська",
        }
    }

    fn language_changes_after_restart(&self) -> &'static str {
        "Мова інтерфейсу зміниться після перезапуску revisor. Перезапустити зараз?"
    }

    fn restart_now(&self) -> &'static str {
        "Перезапустити зараз"
    }

    fn later(&self) -> &'static str {
        "Пізніше"
    }

    fn theme(&self) -> &'static str {
        "Тема"
    }

    fn theme_option(&self, value: Theme) -> &'static str {
        match value {
            Theme::SameAsWindows => "Як у Windows",
            Theme::Light => "Світла",
            Theme::Dark => "Темна",
        }
    }

    fn comparing(&self) -> &'static str {
        "Порівняння знімків…"
    }

    fn comparing_percent(&self, percent: u8) -> String {
        format!("Порівняння знімків: {percent} %…")
    }

    fn waiting_for_database(&self) -> &'static str {
        "Очікування бази профілю…"
    }

    fn comparison_finished(&self) -> &'static str {
        "Порівняння завершено."
    }

    fn comparison_cancelled(&self) -> &'static str {
        "Порівняння скасовано."
    }

    fn comparison_failed(&self, reason: &str) -> String {
        format!("Знімки не порівняно: {reason}.")
    }

    fn comparison_stopped(&self) -> &'static str {
        "порівняння несподівано зупинилося"
    }

    fn comment_stopped(&self) -> &'static str {
        "запис коментаря несподівано зупинився"
    }

    fn compare_with_label(&self, snapshot: &str) -> String {
        format!("Порівняти знімок {snapshot} зі знімком")
    }

    fn compare(&self) -> &'static str {
        "Порівняти"
    }

    fn focus_after_comparison(&self) -> &'static str {
        "Фокус після порівняння"
    }

    fn comparison_focus_option(&self, value: FocusAfterComparison) -> &'static str {
        match value {
            FocusAfterComparison::IfNotMoved => "Якщо фокус не рухали",
            FocusAfterComparison::Always => "Завжди",
            FocusAfterComparison::Never => "Ніколи",
        }
    }

    fn snapshot_saved_comparison_cancelled(&self) -> &'static str {
        "Знімок збережено, порівняння скасовано."
    }

    fn nothing_to_compare(&self) -> &'static str {
        "Знімок не порівняно: іншого знімка в профілі немає."
    }

    fn column(&self, column: Column) -> &'static str {
        match column {
            Column::Name => "Ім'я",
            Column::Change => "Зміна",
            Column::WhatChanged => "Що змінилося",
            Column::Size => "Розмір",
            Column::DateModified => "Дата зміни",
            Column::Folder => "Тека",
        }
    }

    fn change(&self, change: RowChange, kind: EntryKind) -> &'static str {
        match (kind, change) {
            (EntryKind::File, RowChange::Added) => "Додано",
            (EntryKind::File, RowChange::Deleted) => "Видалено",
            (EntryKind::File, RowChange::Modified) => "Змінено",
            (EntryKind::File, RowChange::Replaced) => "Замінено",
            (EntryKind::File, RowChange::Renamed) => "Перейменовано",
            (EntryKind::File, RowChange::Moved) => "Переміщено",
            (EntryKind::File, RowChange::MovedAndRenamed) => "Переміщено й перейменовано",
            (EntryKind::File, RowChange::Inaccessible) => "Недоступно",
            (EntryKind::Dir, RowChange::Added) => "Додано теку",
            (EntryKind::Dir, RowChange::Deleted) => "Видалено теку",
            (EntryKind::Dir, RowChange::Modified) => "Змінено теку",
            (EntryKind::Dir, RowChange::Replaced) => "Замінено теку",
            (EntryKind::Dir, RowChange::Renamed) => "Перейменовано теку",
            (EntryKind::Dir, RowChange::Moved) => "Переміщено теку",
            (EntryKind::Dir, RowChange::MovedAndRenamed) => "Переміщено й перейменовано теку",
            (EntryKind::Dir, RowChange::Inaccessible) => "Недоступна тека",
            (EntryKind::Link, RowChange::Added) => "Додано посилання",
            (EntryKind::Link, RowChange::Deleted) => "Видалено посилання",
            (EntryKind::Link, RowChange::Modified) => "Змінено посилання",
            (EntryKind::Link, RowChange::Replaced) => "Замінено посилання",
            (EntryKind::Link, RowChange::Renamed) => "Перейменовано посилання",
            (EntryKind::Link, RowChange::Moved) => "Переміщено посилання",
            (EntryKind::Link, RowChange::MovedAndRenamed) => "Переміщено й перейменовано посилання",
            (EntryKind::Link, RowChange::Inaccessible) => "Недоступне посилання",
        }
    }

    fn field(&self, field: Field) -> &'static str {
        match field {
            Field::FileVersion => "версія файлу",
            Field::ProductVersion => "версія продукту",
            Field::LinkTarget => "ціль посилання",
            Field::Size => "розмір",
            Field::DateModified => "дата зміни",
            Field::DateCreated => "дата створення",
            Field::Attributes => "атрибути",
            Field::NameCase => "регістр імені",
        }
    }

    fn inaccessible_in(&self, which: InaccessibleIn) -> &'static str {
        match which {
            InaccessibleIn::Earlier => "у ранішому знімку",
            InaccessibleIn::Later => "у пізнішому знімку",
            InaccessibleIn::Both => "в обох знімках",
        }
    }

    fn moved_from(&self, former: &str) -> String {
        format!("з {former}")
    }

    fn bytes(&self, bytes: u64, text: &str) -> String {
        match category(Lang::Uk, bytes.into()) {
            Category::One => format!("{text} байт"),
            Category::Few => format!("{text} байти"),
            _ => format!("{text} байтів"),
        }
    }

    fn size_unit(&self, power: u32) -> &'static str {
        match power {
            1 => "КБ",
            2 => "МБ",
            3 => "ГБ",
            4 => "ТБ",
            5 => "ПБ",
            _ => "ЕБ",
        }
    }

    fn pair(&self, pair: &PairText) -> String {
        let snapshot = |snapshot: &SnapshotText, last: bool| {
            let mut text = snapshot.name.clone();
            if snapshot.deleted {
                text.push_str(" (знімок видалено)");
            }
            if let Some(comment) = &snapshot.comment {
                text.push_str(&format!(", «{comment}»"));
                if !last {
                    text.push(',');
                }
            }
            text
        };
        let mut profile = pair.profile.clone();
        if pair.profile_deleted {
            profile.push_str(" (профіль видалено)");
        }
        format!(
            "{profile}: з {} до {}",
            snapshot(&pair.earlier, false),
            snapshot(&pair.later, true)
        )
    }

    fn summary(&self, summary: &SummaryText) -> String {
        let t = summary;
        let mut text = if t.changes.n == 0 {
            "Змін немає.".to_string()
        } else {
            let mut parts = Vec::new();
            if t.added.n > 0 {
                parts.push(format!("додано {}", t.added.text));
            }
            if t.deleted.n > 0 {
                parts.push(format!("видалено {}", t.deleted.text));
            }
            if t.modified.n > 0 {
                let mut modified = format!("змінено {}", t.modified.text);
                if t.replaced.n > 0 {
                    modified.push_str(&format!(", з них замінено {}", t.replaced.text));
                }
                parts.push(modified);
            }
            if t.moved.n > 0 {
                parts.push(format!("перейменовано чи переміщено {}", t.moved.text));
            }
            format!(
                "{}: {}.",
                self.counted(&t.changes, "зміна", "зміни", "змін"),
                parts.join(", ")
            )
        };
        if t.inaccessible.n > 0 {
            text.push_str(&format!(" Недоступно {}.", t.inaccessible.text));
        }
        if t.changes.n > 0 {
            text.push_str(&match &t.size {
                SizeChange::Increased(size) => format!(" Розмір збільшився на {size}."),
                SizeChange::Decreased(size) => format!(" Розмір зменшився на {size}."),
                SizeChange::Unchanged => " Розмір не змінився.".to_string(),
            });
        }
        if t.warnings.n > 0 {
            text.push_str(&format!(
                " {}.",
                self.counted(&t.warnings, "попередження", "попередження", "попереджень")
            ));
        }
        text
    }

    fn showing(&self, shown_text: &str, total_text: &str) -> String {
        format!("Показано {shown_text} із {total_text}.")
    }

    fn warning(&self, warning: &Warning, reason: &str) -> String {
        match warning {
            Warning::FolderOnlyIn { path, side } => format!(
                "Тека «{}» є лише в {} знімку, тож її не порівнювали.",
                path.display(),
                self.side(*side)
            ),
            Warning::FolderNarrowed {
                wider,
                side,
                compared,
            } => format!(
                "У {} знімку тека «{}» ширша, тож порівнювали лише її частину «{}», яка є \
                 в обох знімках.",
                self.side(*side),
                wider.display(),
                compared.display()
            ),
            Warning::FolderMissing {
                path,
                side: Side::Earlier,
            } => format!(
                "Теки «{}» не було в ранішому знімку, тож її вміст показано доданим.",
                path.display()
            ),
            Warning::FolderMissing {
                path,
                side: Side::Later,
            } => format!(
                "Теки «{}» немає в пізнішому знімку, тож її вміст показано видаленим.",
                path.display()
            ),
            Warning::FolderUnavailable { path, side, .. } => format!(
                "Теку «{}» не вдалося прочитати в {} знімку ({reason}), тож її вміст не \
                 порівнювали.",
                path.display(),
                self.side(*side)
            ),
            Warning::DifferentVolumes { path, .. } => format!(
                "Тека «{}» у знімках лежить на різних томах, тож змін у ній може бути багато.",
                path.display()
            ),
            Warning::ExclusionOnlyIn { pattern, side } => format!(
                "Виняток «{pattern}» є лише в {} знімку, і його застосовано до обох.",
                self.side(*side)
            ),
            Warning::ExclusionAppliedDifferently {
                pattern,
                earlier,
                later,
            } => {
                let applied = |applied: &Option<String>| match applied {
                    Some(applied) => format!("означав «{applied}»"),
                    None => "не діяв".to_string(),
                };
                format!(
                    "Виняток «{pattern}» у ранішому знімку {}, а в пізнішому {}; застосовано \
                     обидва.",
                    applied(earlier),
                    applied(later)
                )
            }
        }
    }

    fn no_difference_yet(&self) -> &'static str {
        "Різниці ще немає"
    }

    fn no_match(&self) -> &'static str {
        "Жодна зміна не відповідає фільтру"
    }

    fn nothing_compared_yet(&self, quick: &str, choose: &str) -> String {
        format!(
            "Ще нічого не порівнювали. Натисніть клавішу {quick}, щоб порівняти поточний знімок \
             із попереднім, або клавіші {choose}, щоб вибрати, з яким знімком його порівняти."
        )
    }

    fn comparison_field(&self) -> &'static str {
        "Порівняння"
    }

    fn filter_field(&self) -> &'static str {
        "Фільтр"
    }

    fn size_format(&self) -> &'static str {
        "Розмір"
    }

    fn size_option(&self, value: SizeFormat) -> &'static str {
        match value {
            SizeFormat::Explorer => "Як у Провіднику",
            SizeFormat::Decimal => "По 1000",
            SizeFormat::Bytes => "У байтах",
        }
    }

    fn tree_node(&self, node: &NodeText) -> String {
        let change = node.change.as_ref().map(|(change, what)| {
            let mut text = lower_first(change);
            if !what.is_empty() {
                text.push_str(": ");
                text.push_str(what);
            }
            (text, !what.is_empty())
        });
        let inside = node.inside.as_ref().map(|(changes, inaccessible)| {
            let mut parts = Vec::new();
            if changes.n > 0 {
                parts.push(self.counted(changes, "зміна", "зміни", "змін"));
            }
            if inaccessible.n > 0 {
                parts.push(format!("недоступно {}", inaccessible.text));
            }
            parts.join(", ")
        });
        let inside = inside.filter(|inside| !inside.is_empty());
        let name = &node.name;
        match (change, inside) {
            (None, None) => name.clone(),
            (Some((change, _)), None) => format!("{name} — {change}"),
            (None, Some(inside)) => format!("{name} — {inside}"),
            // After what changed, the count follows a semicolon, not to read as a field.
            (Some((change, true)), Some(inside)) => format!("{name} — {change}; {inside}"),
            (Some((change, false)), Some(inside)) => format!("{name} — {change}, {inside}"),
        }
    }

    fn layout_shown(&self, layout: DifferenceView) -> &'static str {
        match layout {
            DifferenceView::List => "Різницю показано списком.",
            DifferenceView::Tree => "Різницю показано деревом.",
        }
    }

    fn tree_more(&self, count: &str, above: bool) -> String {
        if above {
            format!("Ще {count} вище — клавіша Enter покаже попередні 1000")
        } else {
            format!("Ще {count} — клавіша Enter покаже наступні 1000")
        }
    }

    fn tree_nodes(&self) -> &'static str {
        "Вузли дерева"
    }

    fn tree_node_option(&self, value: TreeNodeText) -> &'static str {
        match value {
            TreeNodeText::Long => "Ім'я, зміна й що змінилося",
            TreeNodeText::Short => "Лише ім'я й зміна",
        }
    }

    fn property(&self, property: Property) -> &'static str {
        match property {
            Property::Path => "Шлях",
            Property::Name => "Ім'я",
            Property::Change => "Зміна",
            Property::Kind => "Вид",
            Property::Size => "Розмір",
            Property::DateCreated => "Дата створення",
            Property::DateModified => "Дата зміни",
            Property::Attributes => "Атрибути",
            Property::FileVersion => "Версія файлу",
            Property::ProductVersion => "Версія продукту",
            Property::Target => "Ціль",
            Property::Inaccessible => "Недоступно",
            Property::Inside => "Усередині",
        }
    }

    fn before_after(&self, before: &str, after: &str) -> String {
        format!("було {before}, стало {after}")
    }

    fn unchanged(&self, value: &str) -> String {
        format!("{value} (без змін)")
    }

    fn not_counted(&self, text: &str) -> String {
        format!("{text} (не враховано)")
    }

    fn change_type(&self, change: RowChange) -> &'static str {
        match change {
            RowChange::Added => "додано",
            RowChange::Deleted => "видалено",
            RowChange::Modified => "змінено",
            RowChange::Replaced => "замінено",
            RowChange::Renamed => "перейменовано",
            RowChange::Moved => "переміщено",
            RowChange::MovedAndRenamed => "переміщено й перейменовано",
            RowChange::Inaccessible => "недоступно",
        }
    }

    fn what_changed(&self, fields: &str) -> String {
        format!("змінилося: {fields}")
    }

    fn kind(&self, kind: EntryKind) -> &'static str {
        match kind {
            EntryKind::File => "файл",
            EntryKind::Dir => "тека",
            EntryKind::Link => "посилання",
        }
    }

    fn attribute(&self, attribute: Attribute) -> &'static str {
        match attribute {
            Attribute::ReadOnly => "лише для читання",
            Attribute::Hidden => "прихований",
            Attribute::System => "системний",
            Attribute::Archive => "архівний",
            Attribute::Temporary => "тимчасовий",
            Attribute::SparseFile => "розріджений",
            Attribute::ReparsePoint => "точка повторного аналізу",
            Attribute::Compressed => "стиснутий",
            Attribute::Offline => "дані не на пристрої",
            Attribute::NotContentIndexed => "вміст не індексується",
            Attribute::Encrypted => "зашифрований",
            Attribute::IntegrityStream => "з контролем цілісності",
            Attribute::NoScrubData => "без фонової перевірки цілісності",
            Attribute::RecallOnOpen => "завантажується під час відкриття",
            Attribute::Pinned => "завжди на цьому пристрої",
            Attribute::Unpinned => "відкріплений",
            Attribute::RecallOnDataAccess => "лише в Інтернеті",
            Attribute::StrictlySequential => "суворо послідовний",
        }
    }

    fn no_attributes(&self) -> &'static str {
        "без атрибутів"
    }

    fn version_not_read(&self) -> &'static str {
        "версію не прочитано"
    }

    fn no_version(&self) -> &'static str {
        "немає"
    }

    fn target_unknown(&self) -> &'static str {
        "невідома"
    }

    fn inaccessible_details(&self, inaccessible: &Inaccessibility) -> String {
        let why = match inaccessible.why {
            Unreadable::Folder => "вміст теки не вдалося прочитати",
            Unreadable::LinkTarget => "ціль посилання не вдалося прочитати",
            Unreadable::Unknown => {
                "невідомо, чи він там був: теку на шляху до нього не вдалося прочитати або \
                 вона є посиланням"
            }
        };
        let mut text = format!("{} — {why}", self.inaccessible_in(inaccessible.which));
        if let Some(side) = inaccessible.absent_in {
            text.push_str(&format!("; у {} знімку його немає", self.side(side)));
        }
        text.push('.');
        text
    }

    fn attribute_format(&self) -> &'static str {
        "Атрибути"
    }

    fn attribute_option(&self, value: AttributeFormat) -> &'static str {
        match value {
            AttributeFormat::Words => "Словами",
            AttributeFormat::Letters => "Літерами, як у Провіднику",
            AttributeFormat::LettersWithCommas => "Літерами через кому",
        }
    }

    fn warn_missing_entry(&self) -> &'static str {
        "Попереджати, коли запису вже немає на диску"
    }

    fn other_users_folders(&self) -> &'static str {
        "Показувати теки інших користувачів у переліку типових тек"
    }

    fn settings_general(&self) -> &'static str {
        "Загальні"
    }

    fn settings_snapshots(&self) -> &'static str {
        "Знімки"
    }

    fn settings_difference(&self) -> &'static str {
        "Різниця"
    }

    fn entry_gone_question(&self) -> &'static str {
        "Цього запису вже немає на диску. Відкрити теку, де він був?"
    }

    fn open(&self) -> &'static str {
        "Відкрити"
    }

    fn no_folder_on_disk(&self, path: &str) -> String {
        format!("на диску немає жодної теки шляху «{path}»")
    }

    fn export_text_type(&self) -> &'static str {
        "Текстовий звіт (*.txt)"
    }

    fn exporting(&self) -> &'static str {
        "Експорт різниці…"
    }

    fn exported(&self, file: &str) -> String {
        format!("Різницю експортовано: {file}.")
    }

    fn export_cancelled(&self) -> &'static str {
        "Експорт скасовано."
    }

    fn export_failed(&self, reason: &str) -> String {
        format!("Різницю не експортовано: {reason}.")
    }

    fn export_stopped(&self) -> &'static str {
        "експорт несподівано зупинився"
    }

    fn open_failed(&self, reason: &str) -> String {
        format!("Файл не відкрито: {reason}.")
    }

    fn open_after_export(&self) -> &'static str {
        "Відкривати файл після експорту"
    }

    fn settings_export(&self) -> &'static str {
        "Експорт"
    }

    fn report_title(&self, version: &str) -> String {
        format!("Звіт про різницю, revisor {version}")
    }

    fn report_filter(&self, types: Option<&[ChangeType]>, text: Option<&str>) -> String {
        let mut parts = Vec::new();
        if let Some(types) = types {
            parts.push(if types.is_empty() {
                "жодного типу".to_string()
            } else {
                let names: Vec<&str> = types
                    .iter()
                    .map(|kind| match kind {
                        ChangeType::Added => "додані",
                        ChangeType::Deleted => "видалені",
                        ChangeType::Modified => "змінені",
                        ChangeType::Moved => "перейменовані чи переміщені",
                        ChangeType::Inaccessible => "недоступні",
                    })
                    .collect();
                names.join(", ")
            });
        }
        if let Some(text) = text {
            parts.push(format!("текст «{text}»"));
        }
        format!("Фільтр: {}.", parts.join("; "))
    }

    fn report_folder(&self, path: &str) -> String {
        format!("Тека: {path}")
    }

    fn report_group(&self, kind: ChangeType, count: &str) -> String {
        let name = match kind {
            ChangeType::Added => "Додано",
            ChangeType::Deleted => "Видалено",
            ChangeType::Modified => "Змінено",
            ChangeType::Moved => "Перейменовано чи переміщено",
            ChangeType::Inaccessible => "Недоступно",
        };
        format!("{name}: {count}")
    }

    fn report_replaced(&self) -> &'static str {
        "замінено"
    }

    fn report_inaccessible(&self, inaccessible: &Inaccessibility) -> String {
        let which = match inaccessible.which {
            InaccessibleIn::Earlier => "в ранішому знімку",
            InaccessibleIn::Later => "в пізнішому знімку",
            InaccessibleIn::Both => "в обох знімках",
        };
        let why = match inaccessible.why {
            Unreadable::Folder => "вміст теки не вдалося прочитати",
            Unreadable::LinkTarget => "ціль посилання не вдалося прочитати",
            Unreadable::Unknown => {
                "невідомо, чи він там був: теку на шляху до нього не вдалося прочитати або \
                 вона є посиланням"
            }
        };
        let mut text = format!("недоступно {which} — {why}");
        if let Some(side) = inaccessible.absent_in {
            text.push_str(&format!("; у {} знімку його немає", self.side(side)));
        }
        text
    }

    fn explorer_failed(&self, reason: &str) -> String {
        format!("Провідник не відкрито: {reason}.")
    }

    fn key_space(&self) -> &'static str {
        "ПРОБІЛ"
    }
    fn key_up(&self) -> &'static str {
        "Стрілка вгору"
    }
    fn key_down(&self) -> &'static str {
        "Стрілка вниз"
    }
    fn key_left(&self) -> &'static str {
        "Стрілка вліво"
    }
    fn key_right(&self) -> &'static str {
        "Стрілка вправо"
    }
    fn key_context_menu(&self) -> &'static str {
        "Клавіша контекстного меню"
    }
}
