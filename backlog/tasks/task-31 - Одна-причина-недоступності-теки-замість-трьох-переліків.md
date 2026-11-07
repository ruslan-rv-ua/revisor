---
id: TASK-31
title: Одна причина недоступності теки замість трьох переліків
status: To Do
assignee: []
created_date: '2026-11-06 14:43'
updated_date: '2026-11-07 18:02'
labels: []
milestone: m-2
dependencies:
  - TASK-7
  - TASK-8
  - TASK-25
  - TASK-29
type: chore
ordinal: 20000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Кандидат 7 з огляду архітектури 2026-11-05 («гіпотеза»). Мала версія за рішенням автора; йде після TASK-30. Помилок немає, виграш — у читанні коду.

**Проблема (звірено з кодом 2026-11-07).** `LocateError` у `folders.rs` (NotNtfs, Network, Io) і `Unavailable` у `store.rs` (NotNtfs, Network, NoDrive, Error(код), NotAFolder) мають два спільні варіанти, і `snapshot.rs` (рядки ~265–324) зіставляє їх руками. Перевірку «доступ заборонено» (`ERROR_ACCESS_DENIED`) написано тричі: `scan/sys.rs` (`is_access_denied`), `snapshot.rs` і застосунок (`failure.rs`).

**Рішення автора (2026-11-07): мала версія.**
- `LocateError` стає «причина `Unavailable` або помилка `Io`»: NotNtfs і Network живуть лише в `Unavailable`, ручне зіставлення в `snapshot.rs` скорочується.
- «Доступ заборонено» перевіряє одна функція ядра; її беруть `snapshot.rs` і застосунок.
- Не змінюються: коди `STATE_*` і формат бази (це запис для зберігання), тексти, які чує користувач (у вікні профілю й у подробицях різниці вони різні навмисно — різний контекст).
- Без ADR.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `LocateError` має вигляд «причина `Unavailable` або помилка `Io`»: варіанти NotNtfs і Network існують в ядрі лише в `Unavailable`.
- [ ] #2 Ручне зіставлення `LocateError` → `Unavailable` у `snapshot.rs` замінено передачею причини як є.
- [ ] #3 «Доступ заборонено» перевіряє одна функція ядра; власних констант `ERROR_ACCESS_DENIED` у `snapshot.rs` і `failure.rs` немає.
- [ ] #4 Коди `STATE_*`, формат бази й тексти, які чує користувач (вікно профілю, подробиці різниці), не змінилися; наявні тести `folders`, `snapshot`, `store`, `difference` і `profile_window` проходять із тими самими висновками.
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
