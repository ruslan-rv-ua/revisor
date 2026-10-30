---
id: doc-1
title: 'ADR-0001: Rust і wxDragon для GUI'
type: other
created_date: '2026-10-30 15:04'
tags:
  - adr
---
# ADR-0001: Rust і wxDragon для GUI

- **Статус:** прийнято
- **Дата:** 2026-10-30

## Контекст

GUI revisor має бути на wxWidgets — це вимога брифу. Важливі також невеликий розмір exe, доступність для скрінрідерів і швидка робота з великими деревами файлів. Вибирали між Rust (менший exe, але повільна компіляція) і Go (швидка компіляція).

## Рішення

revisor пишемо на Rust, а GUI будуємо на wxWidgets через біндинг wxDragon (крейт `wxdragon`).

## Розглянуті альтернативи

- **Go + wxGo.** Відкинуто: автор позначив проєкт як «Not actively maintained». Біндинг обгортає wxWidgets 3.1.0 (2016), збирається лише через MinGW і під час компіляції потребує понад 5 ГБ пам'яті.
- **Go + windigo** (нативний Win32). Відкинуто: це відмова від wxWidgets, лише Windows і низькорівневий API.

## Наслідки

Позитивні:
- wxDragon активно розвивається: версія 0.9.24 від 26.10.2026 на wxWidgets 3.3.4.
- wxWidgets лінкується статично, тож виходить один exe без DLL wxWidgets. Це зручно для портабельності й scoop.
- Потрібні нативні контроли є: ListCtrl з віртуальним режимом, TreeCtrl, меню з акселераторами, іконка в треї, переклади, діалоги вибору файлів і тек.
- Ліцензії сумісні з MIT: wxDragon має ліцензію MIT/Apache-2.0, а ліцензія wxWidgets (wxWindows Library Licence) дозволяє статичне лінкування.

Негативні наслідки й ризики:
- wxDragon молодий (0.x), тож API може змінюватися між версіями.
- Деяких обгорток немає, наприклад для глобальних гарячих клавіш (RegisterHotKey). Такі речі доведеться робити напряму через Win32 API.
- Перша збірка довга: до ~3 хв із готовими бібліотеками wxWidgets або понад 20 хв, якщо збирати їх із сирців. Далі збірка кешується.
- Розмір exe визначає переважно wxWidgets (кілька мегабайт). Точну цифру покаже прототип.

## Джерела

- [wxDragon на GitHub](https://github.com/AllenDang/wxDragon)
- [wxdragon-sys на docs.rs](https://docs.rs/crate/wxdragon-sys/latest)
- [ListCtrl у wxdragon](https://docs.rs/wxdragon/latest/wxdragon/widgets/list_ctrl/struct.ListCtrl.html)
- [wxGo](https://github.com/dontpanic92/wxGo)
- [windigo](https://github.com/rodrigocfd/windigo)
