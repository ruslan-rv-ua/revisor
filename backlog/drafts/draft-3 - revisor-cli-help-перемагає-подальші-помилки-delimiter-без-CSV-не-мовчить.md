---
id: DRAFT-3
title: 'revisor-cli: --help перемагає подальші помилки, --delimiter без CSV не мовчить'
status: Draft
assignee: []
created_date: '2026-11-15T14:58:00Z'
labels:
  - review cli
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Огляд коду (2026-11-15), revisor-cli. Дві дрібниці розбору аргументів у crates/revisor-cli/src/args.rs: (1) --help лише запам'ятовується, розбір триває, тож «snapshot --help --bogus» дає помилку з кодом 2 замість довідки; (2) --delimiter мовчки ігнорується, коли звіт текстовий (--format text або файл не .csv), хоча для «чужих» опцій команди є перевірка NotForCommand. Вирішити: довідка перемагає подальші помилки; --delimiter без CSV — помилка або попередження.
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
