---
id: TASK-29
title: 'Шлях — один модуль ядра замість чотирьох, без кола залежностей'
status: Done
assignee:
  - '@claude'
created_date: '2026-11-06 14:43'
updated_date: '2026-11-07 17:11'
labels: []
milestone: m-2
dependencies:
  - TASK-7
  - TASK-8
  - TASK-10
  - TASK-11
modified_files:
  - CHANGELOG.md
  - crates/revisor-core/src/path.rs
  - crates/revisor-core/src/path/tests.rs
  - crates/revisor-core/src/names.rs
  - crates/revisor-core/src/lib.rs
  - crates/revisor-core/src/folders.rs
  - crates/revisor-core/src/exclude.rs
  - crates/revisor-core/src/exclude/tests.rs
  - crates/revisor-core/src/compare.rs
  - crates/revisor-core/src/scan.rs
  - crates/revisor-core/src/scan/tests.rs
  - crates/revisor-core/src/diff.rs
  - crates/revisor-core/src/profiles.rs
  - crates/revisor-core/src/snapshot.rs
  - crates/revisor-core/src/testing.rs
  - crates/revisor-core/examples/diff_bench.rs
  - crates/revisor/src/difference.rs
type: chore
ordinal: 18000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Кандидат 5 з огляду архітектури 2026-11-05 («варто дослідити»). Йде після TASK-27 і TASK-28; стосується лише ядра (`crates/revisor-core`), застосунок префіксів `\\?\` не чіпає.

**Проблема (звірено з кодом 2026-11-07).** Щоб відповісти «це той самий шлях?», треба відкрити п'ять файлів ядра:
- `scan.rs`: `verbatim`, `plain`;
- `folders.rs`: `real_path`, `parts`, `strip`;
- `exclude.rs`: `split_path`, `fold`;
- `compare.rs`: `relative_parts`;
- `names.rs`: `upcase`, `eq_ignore_case`, `eq_ignore_case_os` (беруть і `diff`, і `compare`).

«Зняти префікс `\\?\`» написано чотири рази й по-різному: `scan::plain` порівнює префікс із регістром і перетворює `\\?\UNC\…` на `\\…`; `folders::real_path` порівнює без регістру й теж обробляє UNC; `folders::parts` знімає лише `\\?\`, тож `\\?\UNC\srv\share` дає частини UNC, SRV, SHARE, а `\\srv\share` — SRV, SHARE; `exclude::split_path` порівнює з регістром і переводить шлях у `String` через `to_string_lossy`, тож назва, що не є правильним Unicode, спотворюється. Частини шляху мають три подання: `Vec<Vec<u16>>` (folders), `String` → `Vec<u32>` (exclude: шаблону `?` треба цілий символ) і `Vec<OsString>` (compare). `scan` і `folders` залежать одне від одного по колу (`scan` → `folders::real_path`, `folders` → `scan::verbatim`).

**Рішення автора (2026-11-07).**
1. **Одна правильна поведінка.** Модуль `path` має одну поведінку для всіх: префікс знімається без регістру, `\\?\UNC\x` і `\\x` дають ті самі частини, назви не спотворюються. Виправлення розбіжностей приходять разом із модулем, кожне з тестом, і їх записано в CHANGELOG `[Unreleased]`. Для користувача майже непомітні: мережевих тек revisor не підтримує.
2. **Один тип згорнутої назви — із символів.** Символи u32, згорнуті так, як NTFS порівнює назви; непарні сурогати лишаються як є. Підходить і для порівняння шляхів, і для шаблонів із `?`; його беруть `folders`, `exclude` і `compare`.
3. **`real_path` лишається в `folders`.** Він відкриває файл і питає Windows (`GetFinalPathNameByHandle`), тобто це введення-виведення; `path` працює лише з рядком шляху й тестується без файлів. `real_path` знімає префікс через `path::plain`. Коло розривається: `scan` → `folders` → `path`.
4. **`names` входить у `path`.** Згортання назви й частини шляху — одна дія; `eq_ignore_case` і `eq_ignore_case_os` стають надбудовою над нею, `names.rs` зникає.
5. **Без ADR.** Внутрішня будова ядра, як TASK-27; рішення записано тут і в документації модуля.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 У `revisor-core` є модуль `path`: verbatim, plain, частини шляху, згортання назви й порівняння назв без регістру; він не відкриває файлів і не кличе Win32, а його тести не чіпають диска.
- [x] #2 Згорнута назва має один тип із символів (u32, згортання як у NTFS, непарні сурогати лишаються як є), і його беруть `folders`, `exclude` і `compare`; `folders::parts`, `folders::strip`, `exclude::split_path`, `exclude::fold` і `compare::relative_parts` зникли або стали викликами `path`.
- [x] #3 `names.rs` зник: `upcase`, `eq_ignore_case` і `eq_ignore_case_os` живуть у `path` як надбудова над згортанням, і `diff` та `compare` беруть їх звідти.
- [x] #4 Кола немає: `folders` не імпортує `scan`, залежність йде `scan` → `folders` → `path`; `verbatim` і `plain` переїхали зі `scan` у `path`.
- [x] #5 `real_path` лишається в `folders` і знімає префікс через `path::plain`; зняття префікса `\\?\` написано в ядрі один раз.
- [x] #6 Тести `path` покривають кожну колишню розбіжність: префікс у будь-якому регістрі, `\\?\UNC\srv\share` і `\\srv\share` дають ті самі частини, `\\?\` і `\\.\` у verbatim, назва з непарним сурогатом у шляху винятку не спотворюється, а `?` у шаблоні бере символ поза BMP цілим.
- [x] #7 Наявні тести `scan`, `folders`, `exclude`, `compare` і `diff` проходять із тими самими висновками; тести, що перевіряли функції, які переїхали, перенесено в `path`.
- [x] #8 Виправлення розбіжностей, що їх може помітити користувач (шлях винятку з префіксом чи з неправильним Unicode), записано в CHANGELOG `[Unreleased]` у розділі Fixed.
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
Гілка `feature/task-29-path-module` від develop.

1. Новий `crates/revisor-core/src/path.rs` (`pub mod path`), лише робота з рядком шляху, без файлів і Win32:
   - `verbatim(abs)` — абсолютний шлях у форму `\\?\` (`\\?\…` і `\\.\…` лишаються як є, `\\srv\share` → `\\?\UNC\srv\share`), префікси порівнюються без регістру. `std::path::absolute` (це `GetFullPathNameW`) кличуть `scan` і `folders` перед ним, щоб `path` лишався чистим.
   - `plain(path)` — єдине в ядрі зняття префікса: `\\?\UNC\x` → `\\x`, `\\?\x` → `x`, без регістру.
   - `Folded` — згорнута назва, символи u32 (згортання як у NTFS, непарні сурогати як є); `fold(name)`.
   - `parts(path)` — частини `plain(path)` без порожніх, кожна `Folded`: `\\?\UNC\srv\share` і `\\srv\share` дають SRV, SHARE.
   - `upcase(u16)`, `eq_ignore_case`, `eq_ignore_case_os` — надбудова над згортанням (з `names.rs`, який зникає).
2. `folders`: `Location` на `Vec<Folded>`, `parts`/`strip` зникають; `real_path` знімає префікс через `path::plain`; `open` кличе `path::verbatim`; імпорту `scan` немає.
3. `exclude`: `split_path`/`fold` зникають, `Exclusions` тримає `Folded`, `branch` і `from_expanded` беруть `path::parts`; `glob` лишається на символах.
4. `compare`: `pair_folders`/`is_inside`/`relative_parts`/`locate` на `path::parts` і `Folded`; `eq_ignore_case` з `path`.
5. `scan`: `verbatim`/`plain` з `path`; `diff`, `profiles` і застосунок (`difference.rs`) імпортують з `path`.
6. Тести: `path/tests.rs` — кожна колишня розбіжність (префікс у будь-якому регістрі, UNC-частини, `\\?\` і `\\.\` у verbatim, непарний сурогат, `?` і символ поза BMP); тести функцій, що переїхали, — туди ж.
7. CHANGELOG `[Unreleased]` → Fixed; fmt, clippy, test.

Рішення автора 2026-11-07 (питання під час планування): `Expanded::Path` тримає `OsString`, тож скан застосовує виняток із непарним сурогатом точно; запис у знімок (`applied TEXT`) лишається текстом (lossy), схема БД без змін. `Expanded::applied` повертає `Option<String>`. Для цього `path` має й незгорнуті частини (`split`), з яких `parts` згортає.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Виконання (2026-11-07, гілка `feature/task-29-path-module`, коміт 840bc4c — після amend хеш інший, див. git log):
- `path.rs`: `verbatim`, `plain`, `split` (частини як написано), `parts` (згорнуті), `Folded` (символи u32) і `fold`, `upcase`, `eq_ignore_case`, `eq_ignore_case_os`. Єдиний `strip` префікса без регістру. Імпортує лише std; `std::path::absolute` (а це `GetFullPathNameW`) кличуть `scan::scan` і `folders::open` перед `path::verbatim`, тож `verbatim` бере вже абсолютний шлях і нічого не питає у Windows (`debug_assert!` на абсолютність).
- `folders`: `Location` на `Vec<Folded>`; `parts`/`strip` зникли; `real_path` = `path::plain(final_path(..))`; імпорт лише `crate::path`. `is_network` лишився: він класифікує шлях (`\\?\UNC\` чи `\Device\Mup\`), а не знімає префікс.
- `exclude`: `split_path`/`fold` зникли; `Exclusions` на `Folded`, `glob` на `Folded::chars`; `pattern_parts` працює з `OsString` через `path::split`; `Expanded::Path(OsString)`, `applied()` → `Option<String>` (lossy лише для запису в БД).
- `compare`: `pair_folders`/`is_inside` на `Folded`; `relative_parts` → `path::split` (частини як написано: `child` винятків і `locate` через `eq_ignore_case_os` порівнюють без регістру, а `real_path` порівняння тепер має справжній регістр, не верхній — для винятків це байдуже).
- `diff`, `profiles`, застосунок `difference.rs` — імпорт `upcase`/`eq_ignore_case` з `path`; `testing.rs`, `snapshot.rs`, приклад `diff_bench` — під нові сигнатури.
- Тести: `path/tests.rs` (9 тестів: verbatim, `\\?\` і `\\.\` як є, plain, префікс у будь-якому регістрі, UNC-частини, частини без порожніх, непарний сурогат, символ поза BMP, згортання як у NTFS); тести `verbatim_paths` і `plain_paths_drop_the_verbatim_prefix` перенесено зі `scan/tests.rs` у `path`. У `exclude/tests.rs` два нові: шлях винятку зі змінної чи справжнього розташування з непарним сурогатом виключає саме ту теку (а не іншу з U+FFFD), і справжнє розташування `\\?\UNC\…` збігається з `\\server\share`. Тест `?` поза BMP у винятку вже був (`a😀c`).
- Перевірки: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (ядро 187 passed, 1 ignored; застосунок 161 passed) — усе чисто.
- Лишається як було (поза межами задачі): `expand` для справжнього розташування на мережі записує шлях без `\\` на початку (`server\share\…`), бо частини з'єднуються без префікса; на збіг це не впливає (ті самі частини), лише на текст у знімку.
- DoD #3, #5, #6 не стосуються задачі: інтерфейсу вона не змінює, бриф і ADR не зачіпає (рішення №5 — без ADR), нових вікон немає.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Шлях у ядрі тепер обробляє один модуль `revisor-core::path`. Раніше це робили чотири місця (`scan`, `folders`, `exclude`, `compare`) і `names.rs`, і кожне по-своєму.

**Що змінилось**
- `path` працює лише з рядком шляху. У ньому `verbatim`, `plain`, `split`/`parts`, згорнута назва `Folded` (символи u32, згортання як у NTFS, непарні сурогати лишаються як є), а також `upcase`, `eq_ignore_case` й `eq_ignore_case_os` як надбудова над згортанням. Префікс `\\?\` знімається в одному місці й без регістру.
- `folders::real_path` знімає префікс через `path::plain`. `folders` імпортує лише `path`, тож коло `scan` ↔ `folders` розірвано: тепер `scan` → `folders` → `path`.
- `exclude` і `compare` беруть `Folded` і `path::parts`. Функції `folders::parts`/`strip`, `exclude::split_path`/`fold`, `scan::verbatim`/`plain` і `names.rs` зникли, а `compare::relative_parts` став викликом `path::split`.
- `Expanded::Path` тримає `OsString`, як вирішив автор: скан застосовує виняток точно, а запис у знімок лишається текстом, і схема БД не змінилась.

**Виправлено (CHANGELOG `[Unreleased]` → Fixed)**
- Шлях винятку, у змінній чи справжньому розташуванні якого є назва з непарним сурогатом, тепер виключає цю теку. Раніше назва псувалась через `to_string_lossy`, і виняток нічого не виключав.
- Справжнє розташування `\\?\UNC\server\share` тепер має ті самі частини, що й `\\server\share`.

**Тести.** Додано 9 тестів у `path/tests.rs`: кожна колишня розбіжність, а також перенесені тести `verbatim` і `plain`. У `exclude/tests.rs` додано 2 тести: непарний сурогат і UNC. Решта тестів `scan`, `folders`, `exclude`, `compare` і `diff` проходять із тими самими висновками. `cargo fmt --check`, `clippy -D warnings` і `cargo test --workspace` проходять без зауважень.
<!-- SECTION:FINAL_SUMMARY:END -->
