//! Ukrainian interface strings. Key names follow the Microsoft Ukrainian Style Guide
//! (document «Клавіатура й меню», section «Назви клавіш»).

use super::{Lang, Strings};
use crate::commands::{CommandId, MenuId, Pane, Reason, SubmenuId};

pub struct Uk;

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
            Reason::NoDifference => "різниці ще немає, спершу порівняйте знімки",
            Reason::SnapshotNotRunning => "знімок зараз не триває",
            Reason::ComparisonNotRunning => "порівняння зараз не триває",
            Reason::CompactionNotRunning => "стискання зараз не триває",
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
