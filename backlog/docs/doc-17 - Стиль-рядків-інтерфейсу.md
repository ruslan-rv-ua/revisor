---
id: doc-17
title: Стиль рядків інтерфейсу
type: guide
created_date: '2026-11-06 20:03'
---
# Стиль рядків інтерфейсу

> Правила, за якими написано рядки інтерфейсу revisor обома мовами, і рішення, з яких вони взялися. Вичитка (усі рядки виводить `cargo run -p revisor --example strings <файл>`, TASK-17) звіряє рядки з цим документом, із глосарієм (терміни), документом «Клавіатура й меню» (назви клавіш, меню, клавіші доступу, три крапки) і посібниками Microsoft для української й англійської. Рішення, яке змінює чи доповнює правило, дописуємо сюди з датою й задачею. Рішення TASK-17 і TASK-25 ухвалено 2026-11-06.

## Посібники

- Українська — за Microsoft Ukrainian Style Guide, англійська — за Microsoft Writing Style Guide, з американським правописом: canceled, License, licenses (TASK-17).
- Від посібника відходимо лише рішенням, записаним тут чи в «Клавіатура й меню», з причиною.

## Речення й розділові знаки

- Результат дії: що сталося, двокрапка, чому. «Знімок не порівняно: іншого знімка в профілі немає.» / «The snapshot was not compared: the profile has no other snapshot.» (TASK-17).
- Стан у полі прогресу називаємо дією, а не від першої особи: «Очікування бази профілю…» / «Waiting for the profile database…», а не «Чекаю, доки звільниться база профілю…» (TASK-17).
- Порада відомої причини помилки стоїть після тире, а не після другої двокрапки: «Профіль не збережено: немає доступу — перевірте права на цей файл чи теку.» (TASK-17).
- В англійських реченнях тире пишемо без пробілів, як велить Microsoft Writing Style Guide: «access is denied—check the permissions of this file or folder», «in the earlier snapshot—the contents of the folder could not be read». В українських — із пробілами, як вимагає правопис (TASK-25).
- Тире-роздільник між назвою й клавішею чи програмою лишається з пробілами обома мовами: рядки вікна «Гарячі клавіші» («Порівняти з… — Ctrl+F7» / «Compare with… — Ctrl+F7») і заголовок головного вікна («Дім — revisor» / «Home — revisor»). Там тире не розриває речення, а відділяє назву, тож правило посібника про тире в реченні його не стосується (рішення TASK-25).
- Попередження порівняння вже каже, що теку не вдалося прочитати, тож причина в дужках іде без повтору «теку не прочитати:» (TASK-17): «Теку «D:\Ігри» не вдалося прочитати в ранішому знімку (диск теки не підключено), тож її вміст не порівнювали.» У «Тек не знайдено» після знімка помилка Windows стоїть після «теку не прочитати:».
- Назва команди з трьома крапками має їх і поза меню — у вікні «Гарячі клавіші» й у «чому ні» в рядку стану, а заголовки вікон — без трьох крапок (TASK-17; докладно — у «Клавіатура й меню»).

## Підказки з клавішами

- В українських підказках перед назвою клавіші стоїть «клавішу» чи «клавіші»: «Натисніть клавішу F5, щоб зробити перший знімок.» («Клавіатура й меню»).
- В англійських підказках — «Press»: «Press F5 to take the first snapshot.», «Press Ctrl+N to create the first profile.». Microsoft Writing Style Guide радить select, а для F-клавіш і стрілок — use, але revisor пишемо для користувачів скрінрідерів: посібник Microsoft до Екранного диктора й посібник NVDA кажуть «press», «select» поруч із переліком знімків легко сплутати з виділенням рядка, а англійська підказка збігається з українським «Натисніть» (рішення TASK-25).
- Назви клавіш обома мовами — за «Клавіатура й меню», розділ «Назви клавіш»: англійські клавіші з написом — як підписано, без напису — Spacebar, Up arrow, Down arrow, Left arrow, Right arrow, Application key (TASK-25).

## Причини помилок

- Відома причина має власний текст обома мовами: що сталося, чому й що робити (бриф, розділ 5).
- Для решти до свого речення revisor додає текст Windows чи SQLite і код: «Знімок не збережено: системі не вдається знайти вказаний шлях (код 3).» / «… (code 3).»
- Текст Windows чи SQLite після двокрапки пишемо з малої літери, бо він продовжує речення revisor (рішення 9 TASK-17). Власна назва на його початку лишається, як є: слово Windows і слово з великою літерою не на початку, як SQLite, OneDrive, NTFS чи BitLocker. «revisor не перезапущено: Windows не вдається перевірити цифровий підпис цього файлу (код 577).», «…: the system cannot find the path specified (code 3).» Windows — єдина власна назва без такої літери, якою починаються тексти Windows (рішення TASK-25).
- Помилка без коду Windows чи SQLite — власна помилка бібліотеки rusqlite чи помилка вводу-виводу без коду — каже, що її не чекали, а її текст іде в дужках як є, для повідомлення про ваду: «Знімок не збережено: сталася непередбачена помилка (Query returned no rows).» / «The snapshot was not saved: an unexpected error occurred (Query returned no rows).» Так радять настанови Microsoft до повідомлень про помилки: про невідому причину кажуть чесно, а не вигадують її (рішення TASK-25).
- Причини, чому знімок не прочитав теку профілю: «теки немає», «тека на томі FAT32, не NTFS», «тека на мережевому ресурсі», «за цим шляхом файл, а не тека» (TASK-8, TASK-17) і «диск теки не підключено» / «the folder's drive is not connected» — коли диска теки немає в системі чи в пристрої немає носія (рішення TASK-25).

## Слова

- Терміни — за глосарієм. Англійський термін гарячої клавіші — keyboard shortcut, як в інтерфейсі й у Microsoft Writing Style Guide (TASK-25).
- Англійські атрибути файлів — словами Провідника, як українські: data not on the device, content not indexed, with integrity checking, no background integrity scan, downloaded when opened, always on this device, online-only (TASK-17).
- Дрібні рішення TASK-17: «за цим шляхом файл, а не тека»; «Теку … не можна додати, бо revisor не може її перевірити: …» / «… cannot be added because revisor cannot examine it: …»; англійською «the profile database», а не «the profile's database».

## Джерела

- [Microsoft Ukrainian Style Guide](https://download.microsoft.com/download/6/4/a/64a8b98e-e69f-47c9-96dd-f8bc55c176c6/ukr-ukr-StyleGuide.pdf)
- [Microsoft Writing Style Guide](https://learn.microsoft.com/en-us/style-guide/welcome/): [тире](https://learn.microsoft.com/en-us/style-guide/punctuation/dashes-hyphens/), [клавіші й гарячі клавіші](https://learn.microsoft.com/en-us/style-guide/a-z-word-list-term-collections/term-collections/keys-keyboard-shortcuts)
- [Настанови Microsoft щодо повідомлень про помилки](https://learn.microsoft.com/en-us/windows/win32/uxguide/mess-error) (Handling unknown errors, Error codes)
- [Посібник Microsoft до Екранного диктора, розділ 1](https://support.microsoft.com/en-us/windows/chapter-1-introducing-narrator-7fe8fd72-541f-4536-7658-bfc37ddaf9c6)
- Рішення й причини — у нотатках задач TASK-17 і TASK-25
