//! Ukrainian interface strings. Key names follow the Microsoft Ukrainian Style Guide
//! (document «Клавіатура й меню», section «Назви клавіш»).

use revisor_core::exclude::PatternError;
use revisor_core::settings::FocusAfterSnapshot;

use super::plural::{Category, category};
use super::{Failure, Lang, Strings};
use crate::commands::{CommandId, MenuId, Pane, Reason, SubmenuId};

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
}

impl Strings for Uk {
    fn lang(&self) -> Lang {
        Lang::Uk
    }

    fn command(&self, id: CommandId) -> &'static str {
        match id {
            CommandId::NewProfile => "Новий профіль",
            CommandId::EditProfile => "Змінити профіль",
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
            CommandId::Details => "Подробиці",
            CommandId::ShowInExplorer => "Показати в Провіднику",
            CommandId::FilterByText => "Фільтрувати за текстом",
            CommandId::ShowAdded => "Додані",
            CommandId::ShowDeleted => "Видалені",
            CommandId::ShowModified => "Змінені",
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
            Reason::CompactionRunning => "триває стискання бази, а водночас іде лише одне",
            Reason::CompactionOfThisProfile => "триває стискання бази цього профілю",
            Reason::NoSelection => "жодного знімка не виділено",
            Reason::AlreadyAdmin => "revisor уже працює від імені адміністратора",
            Reason::NotYet => "цієї команди ще немає в цій версії revisor",
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
            Pane::Difference => format!("{key} у списку різниці"),
        }
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
        format!("Теку «{folder}» не можна додати: revisor не може її перевірити: {reason}.")
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
            Failure::AccessDenied => "немає доступу: перевірте права на цей файл чи теку".into(),
            Failure::InUse => "файл відкрито в іншій програмі, можливо, в іншій копії \
                revisor: закрийте її й спробуйте ще раз"
                .into(),
            Failure::DiskFull => "на диску немає місця: звільніть місце й спробуйте ще раз".into(),
            Failure::NewerVersion => {
                "профіль створила новіша версія revisor: оновіть revisor".into()
            }
            Failure::NotAProfile => {
                "це не файл профілю revisor: перенесіть його з теки профілів".into()
            }
            Failure::NotSettings => {
                "це не файл налаштувань revisor: перенесіть його з теки data".into()
            }
            Failure::Cancelled => "дію скасовано".into(),
            Failure::Other {
                text,
                code: Some(code),
            } => format!("{text} (код {code})"),
            Failure::Other { text, code: None } => text.clone(),
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

    fn folder_is_a_file(&self) -> &'static str {
        "за цією адресою файл, а не тека"
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

    fn demo_change(&self, n: usize) -> String {
        format!("Демо-зміна {n}")
    }

    fn demo_done(&self, command: &str) -> String {
        format!("Виконано: {command}.")
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
