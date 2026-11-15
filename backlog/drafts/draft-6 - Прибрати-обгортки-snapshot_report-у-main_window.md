---
id: DRAFT-6
title: Прибрати обгортки snapshot_report у main_window
status: Draft
assignee: []
created_date: '2026-11-15T14:58:00Z'
labels:
  - review cleanup
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Огляд коду (2026-11-15). Після перенесення тексту знімка в revisor-ui (snapshot_text, TASK-47) методи MainWindow::snapshot_report і snapshot_report_after у crates/revisor/src/main_window.rs стали однорядковими обгортками над snapshot_text::report / report_after. Викликати спільні функції напряму й прибрати обгортки.
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
