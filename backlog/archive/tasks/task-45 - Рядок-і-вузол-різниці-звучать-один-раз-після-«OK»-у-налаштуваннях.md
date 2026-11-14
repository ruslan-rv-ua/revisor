---
id: TASK-45
title: Рядок і вузол різниці звучать один раз після «OK» у налаштуваннях
status: Done
assignee:
  - '@claude'
created_date: '2026-11-14T12:16:00Z'
updated_date: '2026-11-14T13:10:00Z'
labels: []
milestone: m-5
dependencies:
  - TASK-13
  - TASK-33
  - TASK-38
references:
  - crates/revisor/src/main_window.rs
  - crates/revisor/src/difference_tree.rs
  - tools/nvda/task41_copy_path.py
modified_files:
  - crates/revisor/src/main_window.rs
  - crates/revisor/src/difference_tree.rs
  - crates/revisor/src/difference_tree/tests.rs
  - tools/nvda/task45_settings_ok_once.py
  - tools/nvda/README.md
  - CHANGELOG.md
type: bug
ordinal: 33000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Після «OK» у вікні «Налаштування» NVDA двічі поспіль читає рядок у фокусі списку різниці: назву вікна, підсумок «список», рядок, ще раз рядок і лише тоді «Налаштування збережено.». Помічено під час живої перевірки TASK-41 (`tools/nvda/task41_copy_path.py uk copy` і `en copy`, крок «Enter: saved», 2026-11-14), обома мовами. Після «Скасувати» рядок звучить раз.

Причину підтверджено зондом подій WinEvents без клавіатури (2026-11-14, NVDA 2026.2, release-збірка `develop` fbd8b86). `MainWindow::settings` після збереження кличе `refresh_difference(current)`, а в поданні списком воно робить `set_item_count` (wx `SetItemCount` → `LVM_SETITEMCOUNT`). comctl32 на це, навіть за тієї самої кількості рядків, перебудовує доступні об'єкти списку (`EVENT_OBJECT_DESTROY` і `EVENT_OBJECT_CREATE`) і ще раз шле `EVENT_OBJECT_FOCUS` на рядок у фокусі — за ~47 мс після того, як wx повернув на нього фокус, коли діалог закрився. Кожен виклик окремо, ззовні: лише `LVM_SETITEMCOUNT` із тією самою кількістю — NVDA перечитує рядок; повторний вибір того самого рядка (`select_only`) — лише події вибору, NVDA мовчить; перемальовування — нічого. Так само на v0.3.0 (копія з scoop): вада давня, не з TASK-41.

У поданні деревом після «OK» NVDA каже вузол раз, а тоді «розгорнуто»: `TreeView::refresh` → `reveal` розгортає вже розгорнуту кореневу теку (wx `Expand` скидає `TVIS_EXPANDEDONCE` через `TVM_SETITEM`), і та шле `EVENT_OBJECT_STATECHANGE`.

«Розмір», «Атрибути» й «Вузли дерева» модель лише запам'ятовує: кількість рядків, їхній порядок і рядок у фокусі після «OK» ті самі.

Рішення автора (2026-11-14): після «OK» список лише перемальовується й оновлює підсумок — без `set_item_count` і повторного вибору рядка; інших викликів `refresh_difference` (сортування, типи змін, фільтр) не зачіпати. Відкинуто: `set_item_count` лише за зміни кількості в самому `refresh_difference` (після сортування, де запис лишився на місці, NVDA мовчав би, а зараз перечитує рядок) і оновлення лише за зміни формату (тоді за зміни формату рядок звучав би двічі й далі). Ваду дерева виправити в цій самій задачі: дерево не розгортає вдруге вже розгорнутих тек.

Зонд і його вивід — у теці нотаток сесій TASK-41 (`settings-double-row`: `settings_focus_probe.py`, `isolate_probe.py`), у репозиторії їх немає; живу перевірку варто перенести в `tools/nvda`.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Після «OK» у налаштуваннях із фокусом на рядку списку різниці revisor не шле другої події фокуса на рядок, і NVDA каже назву вікна, підсумок «список» і рядок у фокусі по разу, а тоді «Налаштування збережено.».
- [x] #2 У поданні деревом після «OK» NVDA каже підсумок «дерево» й вузол у фокусі по разу, без «розгорнуто», а тоді «Налаштування збережено.».
- [x] #3 Новий «Розмір», «Атрибути» чи «Вузли дерева» видно одразу після «OK»: рядки, підсумок і вузли пишуться новим форматом, а фокус лишається на тому самому записі.
- [x] #4 Ctrl+T, нова різниця, фільтр і типи змін у дереві звучать і розгортають теки, як досі.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 Тести (cargo test) проходять
- [x] #2 cargo clippy і cargo fmt --check без зауважень
- [x] #3 Зміни інтерфейсу перевірено вручну з NVDA в українському інтерфейсі (якщо задача їх містить)
- [x] #4 Нотатки виконання й підсумок задачі заповнено
- [x] #5 Бриф і ADR чи README оновлено (якщо задача змінила поведінку чи рішення)
- [x] #6 Нові вікна й контроли мають клавіші доступу за документом «Клавіші доступу» (якщо задача їх додає)
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Гілка `bugfix/task-45-settings-ok-once` від develop.
2. `crates/revisor/src/main_window.rs`: `settings()` після «OK» замість `refresh_difference(current)` кличе новий `restyle_difference()`: у списку лише перемальовує рядки (`refresh_items`), у дереві оновлює тексти вузлів (`TreeView::refresh`), далі `describe_difference()` (підсумок, назва списку). Без `set_item_count` і `select_only`: рядки, їхній порядок і рядок у фокусі ті самі. Інші виклики `refresh_difference` (сортування, типи змін, фільтр) — як були.
3. `crates/revisor/src/difference_tree.rs`: `reveal` розгортає на шляху до вузла лише згорнуті теки (`Control::is_expanded`): wx `Expand` на розгорнутій теці переписує її стан, і comctl32 шле подію зміни стану («розгорнуто»). Тест на моку `Control`: `refresh` не розгортає вдруге відкритих тек, а згорнуту на шляху розгортає.
4. `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`.
5. Жива перевірка з NVDA — після згоди автора: новий скрипт `tools/nvda/task45_settings_ok_once.py` без клавіатури (WM_COMMAND, LVM_SETITEMSTATE, кнопки діалогу через WM_COMMAND/BN_CLICKED; «Розмір» — CB_SETCURSEL): список і дерево, «Скасувати» й «OK», зміна формату (#1–#3); `task38_ctrl_t_once.py uk` (#4). Опис у `tools/nvda/README.md`.
6. CHANGELOG `[Unreleased]`, нотатки й підсумок задачі, коміт; `git flow bugfix finish` з no-ff — після слова автора; push — лише з дозволу.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-14, виконання (гілка `bugfix/task-45-settings-ok-once`, коміти 98dda1f і c7395e3).

- `main_window.rs`: `settings()` після «OK» кличе новий `restyle_difference()`: у списку лише `refresh_items` усіх рядків, у дереві `TreeView::refresh` (нові тексти вузлів), далі `describe_difference()`. `set_item_count` і `select_only` у цьому шляху більше не кличуться; інші виклики `refresh_difference` (сортування, типи змін, фільтр) — як були.
- `difference_tree.rs`: `reveal` розгортає на шляху до вузла лише згорнуті теки (`Control::is_expanded`). wx `DoExpand` завжди скидає `TVIS_EXPANDEDONCE` через `TVM_SETITEM` (`src/msw/treectrl.cpp`), і comctl32 шле `EVENT_OBJECT_STATECHANGE`. Це стосується й `fill` та `fill_open` (TASK-38): теки, щойно розгорнуті там, `reveal` уже не розгортає вдруге.
- Тест на моку `Control` `a_refresh_leaves_the_open_folders_on_the_way_as_they_are`: після `fill` розгорнуто дві теки, `refresh` із новим текстом вузла нічого не розгортає, а згорнута на шляху тека розгортається знову, сама. Зі старим `reveal` тест падає.

Перевірка:
- `cargo test --workspace` — 254 + 231 + 6 + 3 пройшли; `cargo clippy --workspace --all-targets -- -D warnings` і `cargo fmt --check` — чисто.
- NVDA 2026.2, release-збірка, без клавіатури: `tools/nvda/task45_settings_ok_once.py uk` і `en` — `all right`. «Скасувати», «OK» без змін і «OK» із новим форматом: одна подія фокуса на рядку чи вузлі, список не перебудовується, у дереві немає зміни стану елемента; NVDA каже назву вікна, підсумок, рядок чи вузол раз, без «розгорнуто», тоді «Налаштування збережено.» (#1, #2). «Розмір» у байтах одразу видно в рядку й підсумку, короткий текст вузла — одразу (#3). «Видалені» туди й назад і фільтр «b» у дереві: тека розгорнута, фокус на тому самому вузлі, NVDA каже лише «Показано N із 3.» (#4).
- Той самий скрипт на збірці `develop` до виправлення: у списку дві події фокуса на рядку, список перебудовано, рядок двічі; у дереві зміна стану й «розгорнуто»; частина `types` звучить так само, як після виправлення.
- `task38_ctrl_t_once.py uk` — `all right`: Ctrl+T на той самий, сусідній, останній і перший запис, нова різниця в дереві (#4).
- Зі справжньою клавіатурою (згода автора), зонд: налаштування через `WM_COMMAND` і Enter чи Esc; через Alt+F, S і Enter; Ctrl+Tab двічі, Alt+H, Enter; і все разом — щоразу рядок раз.
- `task41_copy_path.py uk copy` (клавіатура, згода автора), двічі: на кроці «Enter: saved» NVDA ще каже рядок двічі. Запис WinEvents поруч показав, що revisor там шле лише звичайне повернення фокуса — FOREGROUND, FOCUS на списку, FOCUS на рядку, тоді назву рядка стану, — ті самі події, що й у прогонах з одним читанням. Без клавіатури шлях task41 (копіювання, контекстне меню, прапорець) дав друге читання раз із ~37 закриттів налаштувань. Автор вирішив це ігнорувати (2026-11-14); #1 уточнено: revisor не шле другої події фокуса на рядок.

Зонди діагностики — у теці нотаток сесій TASK-41 (`settings-double-row`); у репозиторії лише `task45_settings_ok_once.py`. Нових вікон і контролів задача не додає (DoD #6 не стосується); бриф і ADR не змінено — поведінка лише стала такою, як задумано, а `tools/nvda/README.md` і CHANGELOG оновлено.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Після «OK» у налаштуваннях revisor більше не змушує список різниці казати рядок у фокусі вдруге, а дерево — додавати «розгорнуто» після вузла.

**Причина** (підтверджено зондом WinEvents без клавіатури, і на v0.3.0 так само). `settings()` після збереження кликав `refresh_difference`, а той — `set_item_count` (wx `SetItemCount` → `LVM_SETITEMCOUNT`). comctl32 на це, навіть за тієї самої кількості рядків, перебудовує доступні об'єкти списку й ще раз шле подію фокуса на рядок — за ~47 мс після того, як фокус повернувся з діалогу, тож NVDA читав рядок двічі. У дереві `reveal` розгортав і вже відкриті теки; wx тоді переписує стан елемента, і NVDA казав «розгорнуто».

**Зміна.** Після «OK» список лише перемальовує рядки, дерево оновлює тексти вузлів, а підсумок іде за новим форматом (`restyle_difference` у `main_window.rs`); `reveal` у `difference_tree.rs` розгортає лише згорнуті теки. Сортування, типи змін і фільтр працюють, як були.

**Перевірка.** `cargo test --workspace` (новий тест на моку `Control`), clippy з `-D warnings`, `fmt --check` — чисто. Новий скрипт без клавіатури `tools/nvda/task45_settings_ok_once.py` українською й англійською — `all right`, а на збірці до виправлення — рядок двічі й «розгорнуто». `task38_ctrl_t_once.py uk` — `all right`. Зонд зі справжніми Enter, Esc, Alt+F, S, Ctrl+Tab і Alt+H — рядок раз.

**Відомі межі.** У повному сценарії `task41_copy_path.py uk copy` NVDA після Enter у налаштуваннях ще каже рядок двічі, хоча revisor шле там лише звичайні події повернення фокуса; автор вирішив це ігнорувати.
<!-- SECTION:FINAL_SUMMARY:END -->
