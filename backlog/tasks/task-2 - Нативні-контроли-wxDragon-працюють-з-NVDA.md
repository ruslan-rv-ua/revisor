---
id: TASK-2
title: Нативні контроли wxDragon працюють з NVDA
status: To Do
assignee: []
created_date: '2026-10-30 17:37'
labels: []
milestone: m-0
dependencies:
  - TASK-1
references:
  - >-
    https://docs.rs/wxdragon/latest/wxdragon/widgets/list_ctrl/struct.ListCtrl.html
documentation:
  - backlog/docs/doc-2 - Бриф-продукту.md
  - backlog/docs/adr/doc-5 - ADR-0004-Лише-нативні-контроли-Windows.md
  - backlog/docs/adr/doc-7 - ADR-0005-Ядро-без-GUI-і-застосунок-на-wxDragon.md
type: spike
ordinal: 2000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Інтерфейс revisor будуємо лише з нативних контролів Windows (ADR-0004). Ще до MVP, поки змінити підхід дешево, треба переконатися, що wxDragon дає все потрібне для доступного інтерфейсу: великі списки, дерево, меню з гарячими клавішами, фонову роботу з прогресом.

Код перевірки — окремий приклад (example) у workspace; у застосунок його не переносимо. Перевіряє автор вручну з NVDA.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Віртуальний ListCtrl на 1 млн рядків із кількома колонками прокручується з клавіатури без помітних затримок.
- [ ] #2 NVDA озвучує рядки віртуального ListCtrl і позицію в списку.
- [ ] #3 TreeCtrl, що заповнюється під час розгортання вузлів, NVDA озвучує з рівнем і станом «згорнуто/розгорнуто».
- [ ] #4 NVDA озвучує пункти меню разом із гарячими клавішами, а самі клавіші спрацьовують.
- [ ] #5 Фонова робота оновлює прогрес в інтерфейсі без зависання, її можна скасувати, а NVDA може прочитати стан прогресу.
- [ ] #6 Знайдені проблеми доступності й обхідні шляхи записано в нотатках задачі.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 Тести (cargo test) проходять
- [ ] #2 cargo clippy і cargo fmt --check без зауважень
- [ ] #3 Зміни інтерфейсу перевірено вручну з NVDA (якщо задача їх містить)
- [ ] #4 Нотатки виконання й підсумок задачі заповнено
- [ ] #5 Бриф і ADR чи README оновлено (якщо задача змінила поведінку чи рішення)
<!-- DOD:END -->
