---
id: TASK-26
title: 'Код HRESULT у тексті помилки записано як 0x80004005, а не від''ємним числом'
status: In Progress
assignee:
  - '@claude'
created_date: '2026-11-06 21:14'
updated_date: '2026-11-06 21:16'
labels: []
milestone: m-1
dependencies:
  - TASK-14
  - TASK-25
references:
  - 'https://learn.microsoft.com/en-us/windows/win32/uxguide/mess-error'
  - >-
    https://learn.microsoft.com/en-us/windows/win32/seccrypto/common-hresult-values
  - >-
    https://learn.microsoft.com/en-us/windows/win32/debug/system-error-codes--0-499-
  - 'https://www.sqlite.org/rescode.html'
documentation:
  - backlog/docs/doc-2 - Бриф-продукту.md
  - backlog/docs/doc-17 - Стиль-рядків-інтерфейсу.md
type: bug
ordinal: 14875
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Коли оболонка Windows не відкриває Провідник для команди «Показати в Провіднику» (TASK-14) і повертає HRESULT, що не несе коду Win32 (наприклад, E_FAIL), revisor показує цей код як знакове 32-бітне число: «Провідник не відкрито: невизначена помилка (код -2147467259).» За кодом користувач шукає допомогу (бриф, розділ «Причини помилок»), а такий запис знайти важко. Настанови Microsoft до повідомлень про помилки (розділ «Error codes») велять писати 32-бітні коди шістнадцятковими, з «0x» і великими літерами: правильними вони називають «1234» і «0xC0001234», а неправильними — «-1» і «-67113524». Так HRESULT пишуть і довідники Microsoft (E_FAIL — 0x80004005). Розбіжність помітила сесія TASK-25 (розділ «Можливе далі» її підсумку).

Рішення користувача 2026-11-06: окрема задача перед випуском 0.1.0 (TASK-19). Код зі старшим бітом — HRESULT чи NTSTATUS, який у десятковому записі від'ємний, — показуємо вісьмома шістнадцятковими цифрами з «0x» і великими літерами: «код 0x80004005» / «code 0x80004005». Коди Win32 і SQLite лишаються десятковими («код 3», «код 577»): так їх пишуть довідник System Error Codes («3 (0x3)») і документація SQLite («SQLITE_IOERR_READ (266)»), і так їх шукають.

Залежності: TASK-14 дає команду «Показати в Провіднику» і перетворення HRESULT на помилку, а TASK-25 — помилку з текстом і кодом Windows чи SQLite та документ «Стиль рядків інтерфейсу» (doc-17), де записано правила рядків.

Джерела — у полі references.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Помилка з 32-бітним кодом зі старшим бітом (HRESULT чи NTSTATUS) показує код вісьмома шістнадцятковими цифрами з «0x» і великими літерами обома мовами: «Провідник не відкрито: невизначена помилка (код 0x80004005).» / «File Explorer did not open: unspecified error (code 0x80004005).»
- [ ] #2 Коди Win32 і SQLite показано десятковими, як досі: «(код 3)», «(code 266)».
- [ ] #3 HRESULT, що несе код Win32 (0x8007xxxx), показано як цей код Win32: відома причина — своїми словами («немає доступу» для 0x80070005), решта — десятковим кодом («код 3» для 0x80070003).
- [ ] #4 Правило закріплено тестами обома мовами інтерфейсу: шістнадцятковий код HRESULT і NTSTATUS, десяткові коди Win32 і SQLite, HRESULT із кодом Win32.
- [ ] #5 Правило записано в документі «Стиль рядків інтерфейсу» (doc-17), у розділі «Причини помилок», з датою, задачею й джерелами.
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

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Гілка `bugfix/task-26-hresult-codes` від develop 3c95364.

Дослідження 2026-11-06: HRESULT без коду Win32 доходить до тексту одним шляхом — `win32::select_in_explorer` (SHParseDisplayName, SHOpenFolderAndSelectItems) → `hresult_error` → `io::Error::from_raw_os_error(hr)` → `failure::of_io` → `Failure::Other { text, code }` з від'ємним `code` → `failure()` в `i18n/uk.rs` і `i18n/en.rs` пише «(код {code})» / «(code {code})». Решта кодів додатні: коди Win32 з `raw_os_error` (GetLastError) і розширені коди SQLite.

1. Рішення користувача (по одному питанню): чи бути задачі й коли; формат коду; зразок у каталозі рядків.
2. Код:
   - `i18n/mod.rs`: одна функція запису коду для обох мов: код, від'ємний як i32 (старший біт — HRESULT чи NTSTATUS), пишемо як `0x` і 8 шістнадцяткових цифр великими літерами, решту — десятковим числом; `uk.rs` і `en.rs` пишуть код через неї;
   - `catalog.rs`: другий зразок `failure.Other.hresult` (E_FAIL) поруч із `failure.Other`;
   - `hresult_error` не змінюється: HRESULT із FACILITY_WIN32 і далі стає кодом Win32.
3. Тести:
   - обома мовами: E_FAIL (0x80004005) у «Провідник не відкрито: …», E_UNEXPECTED (0x8000FFFF, великі літери) і NTSTATUS (0xC0000005) — шістнадцятковими, Win32 (3) і SQLite (266) — десятковими;
   - межі функції: 0, i32::MAX, i32::MIN, -1;
   - у `win32.rs`: `hresult_error` для E_FAIL дає `Failure::Other` з кодом 0x80004005, для 0x80070005 — `AccessDenied`, для 0x80070003 — код 3.
4. doc-17 (MCP): правило в розділі «Причини помилок» з датою, задачею й джерелами (довідник System Error Codes, Common HRESULT Values, коди SQLite).
5. Перевірка: `cargo test --workspace --locked`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo fmt --check`; `cargo run -p revisor --example strings` до й після — diff каталогу лише в новому рядку. NVDA наживо — лише з дозволу користувача на клавіатуру: справжній HRESULT без коду Win32 від оболонки Windows викликати не вдається, тож почути його можна лише з тимчасовою підміною в локальній збірці, яку не комітимо.
6. Нотатки, підсумок, Done; `git flow bugfix finish` і push — лише зі слова користувача.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Рішення користувача 2026-11-06 (по одному питанню):
1. Окрема задача перед випуском 0.1.0: TASK-26, bug, m-1, ordinal 14875 між TASK-25 і TASK-19. Вона залежить від TASK-14 і TASK-25, а TASK-19 тепер залежить від неї.
2. Формат коду — шістнадцятковий лише для кодів зі старшим бітом (HRESULT, NTSTATUS): «код 0x80004005»; коди Win32 і SQLite лишаються десятковими («код 3»). Не обрано: усі коди шістнадцяткові (довідники пишуть коди Win32 і SQLite десятковими, і так їх шукають) і HRESULT обома записами (NVDA читає вдвічі довше, а від'ємний запис настанови Microsoft називають неправильним).
3. Каталог рядків має другий зразок помилки з кодом — `failure.Other.hresult`, тож наступна вичитка побачить обидва записи (389 → 390 рядків).
<!-- SECTION:NOTES:END -->
