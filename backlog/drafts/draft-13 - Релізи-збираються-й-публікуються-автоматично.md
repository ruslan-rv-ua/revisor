---
id: DRAFT-13
title: Релізи збираються й публікуються автоматично
status: Draft
assignee: []
created_date: '2026-10-30 17:04'
labels: []
milestone: m-1
dependencies: []
references:
  - 'https://github.com/ruslan-rv-ua/scoop-bucket/blob/main/bucket/marka.json'
documentation:
  - backlog/docs/doc-2 - Бриф-продукту.md
type: chore
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
revisor встановлюється через scoop з bucket ruslan-rv-ua/scoop-bucket, тож кожен реліз має з'являтися там без ручної роботи. Беремо той самий зразок, що й в інших програмах цього bucket (наприклад, bucket/marka.json): архів із exe в GitHub Releases, файл .sha256 поруч, а маніфест із checkver і autoupdate оновлює Excavator. Перевірки на кожен push і PR ловлять помилки раніше, ніж до релізу.

Передумова: публічний репозиторій revisor на GitHub (наприклад, ruslan-rv-ua/revisor) ще треба створити.

Чернетка: вимоги ще обговорюються (див. «Відкриті питання» в брифі продукту).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 На кожен push і PR GitHub Actions збирає проєкт, запускає тести й clippy.
- [ ] #2 Тег vX.Y.Z запускає релізну збірку й публікує в GitHub Releases архів revisor-X.Y.Z-windows-x64.zip та файл .sha256 з його хешем.
- [ ] #3 Маніфест revisor у bucket має checkver і autoupdate, тож Excavator підхоплює нову версію без ручної роботи.
- [ ] #4 Маніфест позначає теку data як persist, і після оновлення через scoop профілі й знімки лишаються на місці.
<!-- AC:END -->
