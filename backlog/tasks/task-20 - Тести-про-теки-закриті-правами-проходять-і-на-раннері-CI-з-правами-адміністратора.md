---
id: TASK-20
title: >-
  Тести про теки, закриті правами, проходять і на раннері CI з правами
  адміністратора
status: To Do
assignee: []
created_date: '2026-11-04 18:49'
labels: []
milestone: m-1
dependencies:
  - TASK-8
  - TASK-18
references:
  - 'https://github.com/ruslan-rv-ua/revisor/actions/runs/41485939201'
  - 'https://github.com/ruslan-rv-ua/revisor/actions/runs/41889894105'
  - crates/revisor-core/src/elevation.rs
  - crates/revisor-core/src/scan/tests.rs
  - crates/revisor-core/src/snapshot/tests.rs
  - .github/workflows/ci.yml
type: bug
ordinal: 9250
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
CI на develop червоний від злиття TASK-8 (2026-11-02): запуски 41485939201 (після злиття TASK-8) і 41889894105 (після злиття TASK-9, 2026-11-04) падають на кроці Test, а кроки Clippy та «Licences are up to date» після нього не виконуються. Поки так, перевірка кожної зміни в develop (TASK-18) не виконує свого призначення, а зелений CI потрібен і для автоматичних релізів (TASK-19).

Падають три тести ядра, які закривають теку через `icacls /deny` і чекають, що сканер позначить її недоступною: `scan::tests::a_root_closed_by_rights_is_its_own_inaccessible_entry` (бачить 2 записи замість 1), `scan::tests::marks_unreadable_folder_and_goes_on` (тека не недоступна) і `snapshot::tests::a_folder_closed_by_rights_counts_as_denied` (0 закритих правами замість 1). Решта тестів проходить: 51 тест застосунку й 112 тестів ядра.

Причина, з'ясована 2026-11-04: тест `an_administrator_gets_the_backup_privilege_and_nobody_fails` у `crates/revisor-core/src/elevation.rs` викликає `enable_backup_privilege`, яка вмикає SeBackupPrivilege для всього процесу тестів. На машині автора (звичайний користувач) цього привілею немає, тож виклик нічого не змінює, і всі тести проходять. Раннер GitHub (windows-2025) працює від імені адміністратора: привілей вмикається, і теки, закриті правами, читаються, бо сканер відкриває їх із `FILE_FLAG_BACKUP_SEMANTICS` (TASK-8 #19, резервне читання закритих тек від імені адміністратора). Тести йдуть паралельно в одному процесі, тому порядок їх запуску не рятує. Виявлено під час завершення TASK-9; сама поведінка revisor правильна, хибні лише умови тестів на раннері.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `cargo test --workspace --locked` проходить на раннері GitHub (windows-2025, процес від імені адміністратора): запуск CI на develop зелений на всіх кроках, зокрема Clippy і «Licences are up to date».
- [ ] #2 Ті самі тести проходять на машині звичайного користувача без SeBackupPrivilege, як і досі, і в консолі, запущеній від імені адміністратора.
- [ ] #3 Тести про теки, закриті правами, і далі доводять, що без привілею резервного копіювання сканер позначає таку теку недоступною, а знімок рахує її серед закритих правами; їх не пропущено й не послаблено до перевірки, яка проходить завжди.
- [ ] #4 Тест привілею резервного копіювання не змінює стану, від якого залежать інші тести, у якому б порядку чи потоці вони не йшли.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 Тести (cargo test) проходять
- [ ] #2 cargo clippy і cargo fmt --check без зауважень
- [ ] #3 Зміни інтерфейсу перевірено вручну з NVDA в українському інтерфейсі (якщо задача їх містить)
- [ ] #4 Нотатки виконання й підсумок задачі заповнено
- [ ] #5 Бриф і ADR чи README оновлено (якщо задача змінила поведінку чи рішення)
- [ ] #6 Нові вікна й контроли мають клавіші доступу за документом «Клавіатура й меню» (якщо задача їх додає)
<!-- DOD:END -->
