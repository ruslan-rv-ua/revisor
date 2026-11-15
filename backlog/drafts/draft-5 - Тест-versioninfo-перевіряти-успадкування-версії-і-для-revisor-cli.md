---
id: DRAFT-5
title: 'Тест versioninfo: перевіряти успадкування версії і для revisor-cli'
status: Draft
assignee: []
created_date: '2026-11-15T14:58:00Z'
labels:
  - review tests
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Огляд коду (2026-11-15). Тест both_crates_take_the_workspace_version у crates/revisor-core/tests/versioninfo.rs перевіряє version.workspace = true лише для revisor і revisor-core, хоча cli.rc уже звіряється з версією workspace. Явна версія в crates/revisor-cli/Cargo.toml пройшла б тести, а --version і VERSIONINFO розійшлися б. Додати revisor-cli/Cargo.toml до перевірки.
<!-- SECTION:DESCRIPTION:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 Тести (cargo test) проходять
- [ ] #2 cargo clippy і cargo fmt --check без зауважень
- [ ] #3 Зміни інтерфейсу перевірено вручну з NVDA в українському інтерфейсі (якщо задача їх містить)
- [ ] #4 Нотатки виконання й підсумок задачі заповнено
- [ ] #5 Бриф і ADR чи README оновлено (якщо задача змінила поведінку чи рішення)
- [ ] #6 Нові вікна й контроли мають клавіші доступу за документом «Клавіші доступу» (якщо задача їх додає)
<!-- DOD:END -->
