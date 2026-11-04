---
id: TASK-20
title: >-
  Тести про теки, закриті правами, проходять і на раннері CI з правами
  адміністратора
status: Done
assignee: []
created_date: '2026-11-04 18:49'
updated_date: '2026-11-04 19:02'
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
  - 'https://github.com/ruslan-rv-ua/revisor/actions/runs/41892551712'
modified_files:
  - crates/revisor-core/src/elevation.rs
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
- [x] #1 `cargo test --workspace --locked` проходить на раннері GitHub (windows-2025, процес від імені адміністратора): запуск CI на develop зелений на всіх кроках, зокрема Clippy і «Licences are up to date».
- [x] #2 Ті самі тести проходять на машині звичайного користувача без SeBackupPrivilege, як і досі, і в консолі, запущеній від імені адміністратора.
- [x] #3 Тести про теки, закриті правами, і далі доводять, що без привілею резервного копіювання сканер позначає таку теку недоступною, а знімок рахує її серед закритих правами; їх не пропущено й не послаблено до перевірки, яка проходить завжди.
- [x] #4 Тест привілею резервного копіювання не змінює стану, від якого залежать інші тести, у якому б порядку чи потоці вони не йшли.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 Тести (cargo test) проходять
- [x] #2 cargo clippy і cargo fmt --check без зауважень
- [ ] #3 Зміни інтерфейсу перевірено вручну з NVDA в українському інтерфейсі (якщо задача їх містить)
- [x] #4 Нотатки виконання й підсумок задачі заповнено
- [ ] #5 Бриф і ADR чи README оновлено (якщо задача змінила поведінку чи рішення)
- [ ] #6 Нові вікна й контроли мають клавіші доступу за документом «Клавіатура й меню» (якщо задача їх додає)
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Гілка `bugfix/backup-privilege-test-thread-token` від `develop`.
2. У `crates/revisor-core/src/elevation.rs` винести вмикання привілею в `enable_backup_privilege_on(&Token)`; публічна `enable_backup_privilege` і далі працює з токеном процесу (поведінка revisor не змінюється).
3. Тест привілею перевести на копію токена потоку: `ImpersonateSelf` → `OpenThreadToken` → увімкнути привілей на ньому → `RevertToSelf` (на drop). Токен процесу лишається без привілею, тож тести про закриті теки не залежать від порядку й потоку.
4. Наприкінці того самого тесту перевірити через `GetTokenInformation(TokenPrivileges)`, що привілей процесу не ввімкнено.
5. Перевірити локально: звичайний користувач і консоль від імені адміністратора; `cargo fmt`, `cargo clippy`.
6. Нотатки, підсумок, завершення гілки через `git flow bugfix finish`, зелений CI на develop.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Що змінено

Лише `crates/revisor-core/src/elevation.rs`; поведінка revisor не змінилася.

- Вмикання привілею винесено в `enable_backup_privilege_on(&Token)`; публічна `enable_backup_privilege()` і далі відкриває токен процесу й викликає її. Пошук LUID привілею — окрема `backup_privilege()`.
- Тест `an_administrator_gets_the_backup_privilege_and_nobody_fails` більше не торкається токена процесу: потік тесту через `ImpersonateSelf(SecurityImpersonation)` отримує власну копію токена процесу, відкриває її `OpenThreadToken` і вмикає привілей на ній; `RevertToSelf` на drop. Інші тести, у якому б потоці чи порядку вони не йшли, бачать токен процесу без привілею, тож закрита `icacls /deny` тека для них недоступна і на машині адміністратора.
- Той самий тест наприкінці перевіряє через `GetTokenInformation(TokenPrivileges)`, що на токені потоку привілей увімкнено саме тоді, коли функція це повідомила, а на токені процесу після повернення він не ввімкнений.

## Перевірка (2026-11-04)

- Звичайний користувач: `cargo test -p revisor-core --locked` — 115 passed, 1 ignored; `cargo clippy --workspace --all-targets -- -D warnings` і `cargo fmt --check` без зауважень.
- Процес від імені адміністратора (бінарник тестів ядра запущено через `Start-Process -Verb RunAs`): 115 passed, три тести про закриті теки зелені; `whoami /priv` у тому самому підвищеному сеансі показує SeBackupPrivilege Disabled.
- AC #1 (зелений CI на develop) підтверджується після злиття гілки й пушу.

- CI на develop після злиття (запуск 41892551712, коміт 578071d, 2026-11-04): усі кроки зелені — Format, Test, Clippy, «Licences are up to date». AC #1 виконано.
- DoD #3, #5, #6 не стосуються задачі: інтерфейс, бриф, ADR і документ «Клавіатура й меню» не змінювалися.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
CI на develop знову зелений. Тест `an_administrator_gets_the_backup_privilege_and_nobody_fails` у `crates/revisor-core/src/elevation.rs` вмикав SeBackupPrivilege для всього процесу тестів, і на раннері GitHub (адміністратор) три тести про теки, закриті правами, читали ці теки через резервну семантику й падали. Тепер тест вмикає привілей на копії токена, яку через `ImpersonateSelf` має лише його потік, повертається через `RevertToSelf` і перевіряє, що токен процесу лишився без привілею. Вмикання привілею винесено у внутрішню функцію з параметром-токеном; публічна `enable_backup_privilege` і поведінка revisor не змінилися. Тести про закриті теки не ослаблено. Перевірено як звичайний користувач і в підвищеному процесі локально (115 тестів ядра зелені, SeBackupPrivilege процесу Disabled), а також на CI: запуск 41892551712 зелений на всіх кроках. Гілка `bugfix/backup-privilege-test-thread-token` злита в develop через git flow і запушена.
<!-- SECTION:FINAL_SUMMARY:END -->
