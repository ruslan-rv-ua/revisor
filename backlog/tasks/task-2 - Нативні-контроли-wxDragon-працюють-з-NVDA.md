---
id: TASK-2
title: Нативні контроли wxDragon працюють з NVDA
status: In Progress
assignee:
  - '@claude'
created_date: '2026-10-30 17:37'
updated_date: '2026-10-30 18:32'
labels: []
milestone: m-0
dependencies:
  - TASK-1
references:
  - >-
    https://docs.rs/wxdragon/latest/wxdragon/widgets/list_ctrl/struct.ListCtrl.html
documentation:
  - backlog/docs/doc-2 - Бриф-продукту.md
  - backlog/docs/adr/doc-5 - ADR-0004-Лише-нативні-контроли-Windows.md
  - backlog/docs/adr/doc-7 - ADR-0005-Ядро-без-GUI-і-застосунок-на-wxDragon.md
type: spike
ordinal: 2000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Інтерфейс revisor будуємо лише з нативних контролів Windows (ADR-0004). Ще до MVP, поки змінити підхід дешево, треба переконатися, що wxDragon дає все потрібне для доступного інтерфейсу: великі списки, дерево, меню з гарячими клавішами, фонову роботу з прогресом.

Код перевірки — окремий приклад (example) у workspace; у застосунок його не переносимо. Перевіряє автор вручну з NVDA.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Віртуальний ListCtrl на 1 млн рядків із кількома колонками прокручується з клавіатури без помітних затримок.
- [ ] #2 NVDA озвучує рядки віртуального ListCtrl і позицію в списку.
- [ ] #3 TreeCtrl, що заповнюється під час розгортання вузлів, NVDA озвучує з рівнем і станом «згорнуто/розгорнуто».
- [ ] #4 NVDA озвучує пункти меню разом із гарячими клавішами, а самі клавіші спрацьовують.
- [ ] #5 Фонова робота оновлює прогрес в інтерфейсі без зависання, її можна скасувати, а NVDA може прочитати стан прогресу.
- [ ] #6 Знайдені проблеми доступності й обхідні шляхи записано в нотатках задачі.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 Тести (cargo test) проходять
- [ ] #2 cargo clippy і cargo fmt --check без зауважень
- [ ] #3 Зміни інтерфейсу перевірено вручну з NVDA (якщо задача їх містить)
- [ ] #4 Нотатки виконання й підсумок задачі заповнено
- [ ] #5 Бриф і ADR чи README оновлено (якщо задача змінила поведінку чи рішення)
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Узгоджено з користувачем 2026-10-30 (по одному питанню).

1. Гілка `feature/task-2-nvda-controls` від `develop`.
2. Приклад `crates/revisor/examples/nvda_controls.rs` (запуск `cargo run --example nvda_controls`, release — `cargo run --release --example nvda_controls`). У застосунок код не переносимо. Написи англійською.
3. Одне вікно з меню; порядок Tab: підпис + віртуальний ListCtrl → підпис + TreeCtrl → кнопка Start → смуга прогресу (Gauge) і текст стану → кнопка Cancel. Кожен контрол має підпис (StaticText перед ним), щоб NVDA читав назву.
4. Віртуальний ListCtrl (`ListCtrlStyle::Report | Virtual | SingleSel`): 1 000 000 синтетичних рядків, колонки Path, Change, Size, Modified; текст генерується з номера рядка в колбеку `set_virtual_text_callback`.
5. TreeCtrl із синтетичними теками: вузли до 5 рівнів, діти додаються в `ItemExpanding` (до розгортання вузол має «плюс» через `set_item_has_children`).
6. Меню з гарячими клавішами (через `\t` у назві пункту): File → Exit (Ctrl+Q); View → Go to list (Ctrl+1), Go to tree (Ctrl+2), Go to row… (Ctrl+G, діалог номера рядка); Work → Start (F5), Cancel (Shift+F5); Help → About (F1).
7. Фонова робота: потік ~20 с, прогрес в `Arc<AtomicU32>`, скасування через `Arc<AtomicBool>`; UI опитує стан таймером (~100 мс) і оновлює Gauge та текст стану. Список і дерево під час роботи доступні.
8. Перевірки: `cargo build --example nvda_controls`, clippy (`--all-targets`), fmt, тести workspace.
9. Покрокові інструкції для користувача (NVDA) по кожному критерію; його відгуки, проблеми й обхідні шляхи — у нотатки задачі. Критерії NVDA відмічаю лише після його підтвердження.
10. Рев'ю, підсумок, коміти англійською; злиття в `develop` і пуш — лише за згодою користувача.

Уточнення під час реалізації: смуга прогресу (Gauge) не є зупинкою Tab, тому стан роботи дублюється в полі «Job status» (TextCtrl лише для читання) — до нього можна дійти Tab і прочитати. Коли кнопку Start чи Cancel вимикаємо, а фокус був на ній, фокус переходить на другу кнопку, щоб не загубитися.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Приклад `crates/revisor/examples/nvda_controls.rs` зібрано без додаткових фіч wxdragon (`default-features = false` вистачає для ListCtrl, TreeCtrl, Gauge, меню, діалогів, Timer). Release-exe прикладу — 4,3 МБ. Фонову роботу UI опитує таймером (100 мс): `wxdragon::call_after` вимагає `Send`, а віджети не `Send`, тож атомарні змінні + таймер простіші.
<!-- SECTION:NOTES:END -->
