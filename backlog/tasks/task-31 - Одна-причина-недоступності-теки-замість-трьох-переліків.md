---
id: TASK-31
title: Одна причина недоступності теки замість трьох переліків
status: Done
assignee:
  - '@claude'
created_date: '2026-11-06 14:43'
updated_date: '2026-11-07 18:14'
labels: []
milestone: m-2
dependencies:
  - TASK-7
  - TASK-8
  - TASK-25
  - TASK-29
modified_files:
  - crates/revisor-core/src/folders.rs
  - crates/revisor-core/src/folders/tests.rs
  - crates/revisor-core/src/scan.rs
  - crates/revisor-core/src/snapshot.rs
  - crates/revisor-core/src/snapshot/tests.rs
  - crates/revisor/src/failure.rs
  - crates/revisor/src/profile_window.rs
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
- [x] #1 `LocateError` має вигляд «причина `Unavailable` або помилка `Io`»: варіанти NotNtfs і Network існують в ядрі лише в `Unavailable`.
- [x] #2 Ручне зіставлення `LocateError` → `Unavailable` у `snapshot.rs` замінено передачею причини як є.
- [x] #3 «Доступ заборонено» перевіряє одна функція ядра; власних констант `ERROR_ACCESS_DENIED` у `snapshot.rs` і `failure.rs` немає.
- [x] #4 Коди `STATE_*`, формат бази й тексти, які чує користувач (вікно профілю, подробиці різниці), не змінилися; наявні тести `folders`, `snapshot`, `store`, `difference` і `profile_window` проходять із тими самими висновками.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 Тести (cargo test) проходять
- [x] #2 cargo clippy і cargo fmt --check без зауважень
- [x] #3 Зміни інтерфейсу перевірено вручну з NVDA в українському інтерфейсі (якщо задача їх містить)
- [x] #4 Нотатки виконання й підсумок задачі заповнено
- [x] #5 Бриф і ADR чи README оновлено (якщо задача змінила поведінку чи рішення)
- [x] #6 Нові вікна й контроли мають клавіші доступу за документом «Клавіатура й меню» (якщо задача їх додає)
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. `folders.rs`: `LocateError` стає `Unavailable(store::Unavailable)` | `Io(io::Error)`. `locate` і `volume_error` повертають `Unavailable::Network` / `Unavailable::NotNtfs`. Display лишається тим самим за змістом. Кола залежностей немає: `store` не використовує `folders`.
2. `snapshot.rs`: замість двох гілок NotNtfs/Network — `Err(LocateError::Unavailable(reason)) => FolderState::Unavailable(reason)`; власну константу `ERROR_ACCESS_DENIED` прибрати, у `not_opened` викликати `scan::is_access_denied`.
3. Одна функція «доступ заборонено»: `scan/sys.rs::is_access_denied` лишається, де є, і стає публічною через `pub use` у `scan.rs` (`revisor_core::scan::is_access_denied`). Нового модуля немає.
4. `failure.rs`: прибрати `ERROR_ACCESS_DENIED`; у `of_io` спершу `is_access_denied(e)` → `Failure::AccessDenied`, далі решта кодів, як було.
5. `profile_window.rs`: `LocateError::Unavailable(Unavailable::NotNtfs(fs))` / `(Unavailable::Network)` → ті самі рядки. Інші причини `locate` не повертає, тож для них — `unreachable!` або той самий текст, що й для Io? Вирішимо при читанні коду, не змінюючи текстів.
6. Тести `folders/tests.rs` підлаштувати під нові варіанти. Далі fmt, clippy і `cargo test --workspace`.

Уточнення до кроку 5: у `profile_window.rs` для інших причин (`NoDrive`, `Error`, `NotAFolder`) стоїть гілка `unreachable!`. `locate` їх не дає, і це записано в документації `LocateError::Unavailable`. Нових текстів немає. `volume_error` тепер повертає `Option<Unavailable>`.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- `LocateError` = `Unavailable(store::Unavailable)` | `Io(io::Error)`. `folders` імпортує `store::Unavailable`, а `store` від `folders` не залежить, тож кола немає. `volume_error` повертає `Option<Unavailable>`. `Display` дає ті самі тексти для NotNtfs/Network, а для інших причин — запасний (`{reason:?}`), якого `locate` ніколи не дає.
- `snapshot.rs`: дві гілки NotNtfs/Network замінено однією — `Err(LocateError::Unavailable(reason)) => FolderState::Unavailable(reason)`. Константу `ERROR_ACCESS_DENIED` прибрано, лічильник `denied` бере `scan::is_access_denied`. Тест `absent` у `snapshot/tests.rs` бере код із `Foundation` локально.
- Одна функція: `scan/sys.rs::is_access_denied` лишилася на місці й відкрита як `revisor_core::scan::is_access_denied` (`pub use` у `scan.rs`). Нового модуля немає.
- `failure.rs`: власної константи немає. `of_io` спершу питає `is_access_denied`, решта кодів — як і була.
- `profile_window.rs`: ті самі рядки `folder_not_ntfs`/`folder_on_network`/`folder_not_examined`. Для недосяжних причин — `unreachable!`.
- Коди `STATE_*`, `store.rs`, i18n і `difference.rs` не змінено.
- Перевірки: `cargo fmt --check` ок, `cargo clippy --workspace --all-targets -- -D warnings` ок, `cargo test --workspace`: 165 + 187 + 6 + 3 passed, 0 failed (по одному ignored, як і до задачі).
- Коміт 45130cd на `feature/task-31-unavailable`.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`LocateError` тепер «причина `Unavailable` або помилка `Io`»: NotNtfs і Network є лише в `store::Unavailable`. Знімок передає причину від `locate` як є, без ручного зіставлення. «Доступ заборонено» перевіряє одна функція ядра `scan::is_access_denied`. Її беруть `snapshot.rs` і застосунок (`failure.rs`), власних констант `ERROR_ACCESS_DENIED` там більше немає. Вікно профілю показує ті самі тексти. Коди `STATE_*`, формат бази й тексти, які чує користувач, не змінилися. Видимого інтерфейсу задача не стосується, тож NVDA не знадобився. Бриф і ADR не змінено (поведінка та сама). fmt, clippy і всі тести робочого простору проходять.
<!-- SECTION:FINAL_SUMMARY:END -->
