---
id: TASK-1
title: 'Каркас workspace збирається, а час збірки й розмір exe виміряно'
status: Done
assignee:
  - '@claude'
created_date: '2026-10-30 17:37'
updated_date: '2026-10-30 18:18'
labels: []
milestone: m-0
dependencies: []
references:
  - 'https://github.com/AllenDang/wxDragon'
documentation:
  - backlog/docs/doc-2 - Бриф-продукту.md
  - backlog/docs/adr/doc-1 - ADR-0001-Rust-і-wxDragon-для-GUI.md
  - backlog/docs/adr/doc-7 - ADR-0005-Ядро-без-GUI-і-застосунок-на-wxDragon.md
type: spike
ordinal: 1000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Найбільші сумніви щодо Rust — довга компіляція й розмір exe (ADR-0001). Прототип стає основою для MVP, тож каркас одразу справжній: Cargo workspace із крейтом ядра без залежності від wxDragon і крейтом застосунку на wxDragon (ADR-0005).

Тулчейн — MSVC: на машині вже є Rust stable для MSVC і Visual Studio Build Tools 2026. Для тулчейна GNU wxDragon вимагає саме WinLibs GCC 15.1.0 UCRT, а встановлено GCC 15.2.0.

Бриф вимагає, щоб програма була одним exe без DLL, тож це перевіряємо вже на каркасі. Цифри з цієї задачі підуть у документ «Результати прототипу» (остання задача етапу).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Workspace містить крейт ядра без залежності від wxDragon і крейт застосунку на wxDragon, який відкриває порожнє вікно.
- [x] #2 Проєкт збирається тулчейном MSVC, а тести ядра запускаються без збирання wxWidgets.
- [x] #3 Час холодної збірки й інкрементальної збірки після зміни в ядрі та після зміни в застосунку виміряно й записано в нотатках задачі.
- [x] #4 Розмір релізного exe з налаштуваннями на малий розмір виміряно й записано в нотатках задачі.
- [x] #5 Релізний exe — один файл без DLL і запускається на чистій Windows без бібліотек Visual C++ (наприклад, у Windows Sandbox).
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 Тести (cargo test) проходять
- [x] #2 cargo clippy і cargo fmt --check без зауважень
- [x] #3 Зміни інтерфейсу перевірено вручну з NVDA (якщо задача їх містить)
- [x] #4 Нотатки виконання й підсумок задачі заповнено
- [x] #5 Бриф і ADR чи README оновлено (якщо задача змінила поведінку чи рішення)
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Гілка `feature/task-1-workspace` від `develop`.
2. Cargo workspace (resolver 3, edition 2024): `crates/revisor-core` (бібліотека без залежностей, заглушка з тестом) і `crates/revisor` (bin `revisor.exe` на `wxdragon` 0.9.x, порожнє вікно, `windows_subsystem = "windows"` у release, маніфест через `embed-manifest` у build.rs).
3. `.cargo/config.toml`: `+crt-static` для `x86_64-pc-windows-msvc` — wxdragon-sys тоді збирає wxWidgets з `/MT`, і exe не потребує бібліотек Visual C++.
4. Профіль release на малий розмір: `opt-level = "z"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`, `strip = true`.
5. Перевірити: `cargo test -p revisor-core` не збирає wxWidgets; clippy і fmt чисті.
6. Виміряти: холодна збірка (debug і release), інкрементальна після зміни в ядрі й після зміни в застосунку; розмір release exe з типовими фічами wxdragon і з `default-features = false`. У каркасі залишити мінімальний набір, якщо він збирається й вікно відкривається.
7. Перевірити залежності exe (`dumpbin /dependents`) — лише системні DLL Windows. Запуск у Windows Sandbox — вручну користувачем.
8. Записати цифри в нотатки задачі.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Тулчейн: Rust 1.99.1 MSVC, VS Build Tools 2026, CMake + Ninja. wxdragon 0.9.24 не завантажує готові бібліотеки, а збирає wxWidgets 3.3.4 із сирців (968 кроків Ninja).
- Холодна збірка (крейти вже завантажено): debug 2 хв 11 с; release 1 хв 59 с з типовими фічами wxdragon, 1 хв 10 с із `default-features = false`.
- Інкрементальна збірка після зміни в ядрі: debug 2,2 с, release 3,3 с.
- Інкрементальна після зміни в застосунку: debug 1,9 с, release 3,3 с.
- Інкрементальні збірки міряно після правки коментаря, а не коду.
- Розмір релізного exe (opt-level "z", LTO, codegen-units 1, panic abort, strip): 6,1 МБ з типовими фічами wxdragon, 4,0 МБ із `default-features = false`. У каркасі залишено мінімальний набір, вікно відкривається з заголовком «revisor 0.1.0».
- `+crt-static` у `.cargo/config.toml`. `dumpbin /dependents` показує лише системні DLL Windows (KERNEL32, USER32, GDI32, gdiplus, MSIMG32, COMDLG32, SHELL32, SHLWAPI, COMCTL32, ole32, OLEAUT32, ADVAPI32, OLEACC, UxTheme, bcryptprimitives, ntdll), без VCRUNTIME, MSVCP і ucrt.
- `cargo test -p revisor-core` не збирає wxWidgets. `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` і `cargo fmt --check` чисті.
- Рев'ю (стандарти + відповідність задачі) виправлено: тест версії не прив'язаний до номера, помилка запуску GUI не ковтається (`expect`), вікно центрується до показу, build.rs перевіряє `CARGO_CFG_TARGET_OS`.
- DoD #3 не стосується: інтерфейсу з контролами в каркасі немає, лише порожнє вікно.
- Запуск у Windows Sandbox ще не перевірено — робить користувач вручну.

- Критерій 5 закрито рішенням користувача на підставі `dumpbin /dependents`: релізний exe залежить лише від системних DLL Windows, без VCRUNTIME, MSVCP і ucrt. Запуск у Windows Sandbox не виконувався: користувач працює з NVDA, а в пісочниці його немає.
- DoD #5: бриф і ADR не змінювались, бо задача не змінила ні поведінки, ні рішень.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Створено Cargo workspace: `crates/revisor-core` — бібліотека ядра без залежностей; `crates/revisor` — застосунок на wxdragon 0.9.24 з `default-features = false`, який відкриває порожнє вікно. CRT лінкується статично (`+crt-static` у `.cargo/config.toml`), профіль release налаштовано на малий розмір (opt-level "z", LTO, codegen-units 1, panic abort, strip), маніфест вбудовується через embed-manifest.

Ключові цифри: холодна збірка debug 2 хв 11 с, release 1 хв 59 с (з мінімальними фічами wxdragon — 1 хв 10 с); інкрементальна збірка — 2–3 с; релізний exe — 4,0 МБ (6,1 МБ з типовими фічами).

Перевірки: cargo test, clippy (-D warnings) і fmt --check чисті; тести ядра не збирають wxWidgets; `dumpbin /dependents` показує лише системні DLL Windows. Запуск у Windows Sandbox замінено перевіркою dumpbin за рішенням користувача.
<!-- SECTION:FINAL_SUMMARY:END -->
