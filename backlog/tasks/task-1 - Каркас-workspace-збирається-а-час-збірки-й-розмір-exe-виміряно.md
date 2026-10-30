---
id: TASK-1
title: 'Каркас workspace збирається, а час збірки й розмір exe виміряно'
status: To Do
assignee: []
created_date: '2026-10-30 17:37'
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
- [ ] #1 Workspace містить крейт ядра без залежності від wxDragon і крейт застосунку на wxDragon, який відкриває порожнє вікно.
- [ ] #2 Проєкт збирається тулчейном MSVC, а тести ядра запускаються без збирання wxWidgets.
- [ ] #3 Час холодної збірки й інкрементальної збірки після зміни в ядрі та після зміни в застосунку виміряно й записано в нотатках задачі.
- [ ] #4 Розмір релізного exe з налаштуваннями на малий розмір виміряно й записано в нотатках задачі.
- [ ] #5 Релізний exe — один файл без DLL і запускається на чистій Windows без бібліотек Visual C++ (наприклад, у Windows Sandbox).
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 Тести (cargo test) проходять
- [ ] #2 cargo clippy і cargo fmt --check без зауважень
- [ ] #3 Зміни інтерфейсу перевірено вручну з NVDA (якщо задача їх містить)
- [ ] #4 Нотатки виконання й підсумок задачі заповнено
- [ ] #5 Бриф і ADR чи README оновлено (якщо задача змінила поведінку чи рішення)
<!-- DOD:END -->
