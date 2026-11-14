---
id: TASK-47
title: Знімки й порівняння доступні з командного рядка
status: In Progress
assignee:
  - '@claude'
created_date: 2026-10-30 15:57
updated_date: '2026-11-14T20:34:00Z'
labels: []
milestone: m-6
dependencies:
  - TASK-7
  - TASK-8
  - TASK-10
  - TASK-11
  - TASK-13
  - TASK-16
  - TASK-19
  - TASK-23
  - TASK-24
  - TASK-39
  - TASK-40
references:
  - https://learn.microsoft.com/en-us/windows/console/console-allocation-policy
  - https://devblogs.microsoft.com/oldnewthing/20090101-00/?p=19643
  - https://github.com/ip7z/7zip/blob/main/DOC/readme.txt
  - https://www.voidtools.com/support/everything/command_line_interface/
  - https://github.com/ScoopInstaller/Scoop/wiki/App-Manifests
  - https://clig.dev/
  - >-
    https://learn.microsoft.com/en-us/dotnet/standard/commandline/design-guidance
  - https://pandoc.org/MANUAL.html
  - >-
    https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_character_encoding
  - https://github.com/git/git/blob/master/po/README.md
  - >-
    https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.utility/import-csv
  - >-
    https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_preference_variables
  - https://www.gnu.org/software/diffutils/manual/html_node/Invoking-diff.html
  - https://git-scm.com/docs/git-diff
  - https://doi.org/10.1145/3411764.3445544
  - >-
    https://github.blog/engineering/user-experience/building-a-more-accessible-github-cli/
  - https://man7.org/linux/man-pages/man1/flock.1.html
  - https://www.sqlite.org/c3ref/busy_timeout.html
  - >-
    https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.utility/export-csv
documentation:
  - backlog/docs/doc-2 - Бриф-продукту.md
type: feature
ordinal: 35000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Головний сценарій можна автоматизувати скриптом: знімок, тихе встановлення програми, а потім порівняння зі збереженим звітом. Для цього revisor потрібен командний рядок.

Етап: 0.5 (m-6), до 1.0. Команди й параметри командного рядка — частина «публічного API» для semver: до 1.0 їх ще можна змінювати, а з 1.0 несумісна зміна означатиме нову мажорну версію; нову команду чи параметр можна додати й пізніше, сумісно. Звіт має той самий формат, що й експорт: текстовий звіт — TASK-39, CSV — TASK-40. Формат CSV — так само частина «публічного API» (бриф, розділ 5).

Вирішено (2026-11-14, розбір DRAFT-9 з користувачем; джерела — у посиланнях):
- **Окремий exe.** Консольний `revisor-cli.exe` поруч із `revisor.exe`. На консольний exe cmd, .bat і PowerShell чекають і отримують його код виходу, а на exe з вікнами — ні (Microsoft, «Console Allocation Policy»), тож скрипт не почне встановлення, поки знімок не готовий; вікно revisor лишається без консолі. Так роблять 7-Zip (7z.exe і 7zG.exe) і Everything (es.exe). Ім'я однакове і в scoop (друга команда в `bin`), і за прямого завантаження; реліз дає два exe, кожен зі своїм .sha256. Відкинуто: `revisor.com`, як devenv чи WinSCP (у scoop обидва файли дістають shim з тим самим ім'ям), той самий exe з AttachConsole (оболонки не чекають, а через shim scoop виводу не видно) і консольний exe з політикою `detached` (лише Windows 11 24H2+).
- **Команди й параметри.** Дві команди: `revisor-cli snapshot` і `revisor-cli compare`; «знімок + порівняння» — два виклики поспіль, переліків профілів і знімків немає. Назви англійські, як колонки CSV. У кожного параметра довга назва, однолітерні — лише `-p` (`--profile`), `-o` (`--output`), `-q` (`--quiet`), `-h` (`--help`); `--version` без `-v`, бо `-v` неоднозначний (clig.dev; Microsoft, System.CommandLine).
- **Профіль.** Обов'язковий `--profile <назва>`: назва, як у вікні, без огляду на регістр. Без нього команда не виконується й каже чому, а за невідомої назви називає наявні профілі, тож скрипт не залежить від того, що відкривали у вікні. Тека `data` — та сама, що й у вікна, поруч з exe.
- **Знімок.** `snapshot` робить знімок профілю, а `--comment <текст>` дає йому коментар, як у вікні.
- **Які знімки порівнюються.** `compare` порівнює останній знімок із попереднім, як «Швидке порівняння», а `--with <номер>` — останній із вибраним. Номер — номер знімка в базі, якого база ніколи не видає вдруге; номер нового знімка друкує `snapshot`, тож скрипт запам'ятовує знімок «до» (`$before = revisor-cli snapshot …`). Різниця завжди йде від ранішого знімка до пізнішого.
- **Звіт.** Обов'язковий `--output <файл>`. Формат вирішує розширення: `.csv` (без огляду на регістр) — CSV, інше — текстовий звіт, а `--format text|csv` вибирає формат явно (так pandoc вгадує формат за розширенням). Звіт містить усю різницю: запам'ятовані у вікні фільтри не діють, а заголовок текстового звіту — без рядка фільтра. Файл пишеться поруч і заступає наявний лише цілим; наявний файл перезаписується без питань, як у Export-Csv. Звіту в stdout немає: PowerShell розшифровує вивід програм кодовою сторінкою OEM, а Windows PowerShell 5.1 при `>` пише UTF-16, тож кирилиця спотворилася б.
- **Мова.** Повідомлення, `--help` і текстовий звіт — типово англійською на будь-якому комп'ютері, а `--language uk` дає українську (вирішено з користувачем). Формати текстового звіту — як у вікні: регіональні формати чисел і дат, параметри «Розмір» і «Атрибути». CSV однаковий за будь-якої мови.
- **Роздільник CSV.** З налаштування «Роздільник у CSV», тож CSV такий самий, як експортований із вікна, і Excel на тому самому комп'ютері відкриває його в колонки; `--delimiter comma|semicolon|tab|windows` задає його явно для скрипта, що сам читатиме файл. Відкинуто «завжди кома»: CSV відрізнявся б від експортованого з вікна.
- **Коди виходу.** 0 — знімок збережено (навіть із недоступними записами) чи звіт записано, хоч би скільки змін; 1 — не вдалося або скасовано Ctrl+C; 2 — неправильний виклик. Не 0 завжди означає збій, тож `$?` і `$PSNativeCommandUseErrorActionPreference` у PowerShell працюють без винятків; код «є зміни», як у diff і fc, PowerShell вважав би помилкою (Microsoft описує цю пастку для robocopy). Такий код можна додати пізніше окремим параметром, як `git diff --exit-code`.
- **Вивід.** stdout — лише дані: номер нового знімка від `snapshot`; `compare` туди нічого не пише. Повідомлення йдуть у stderr: рядок про початок і підсумок наприкінці, як у рядку стану вікна (записи й недоступні — з порадою запустити від імені адміністратора; файл звіту й підсумок змін). `--progress` додає рядок прогресу раз на 10 с (типово його немає, бо NVDA читає кожен рядок), а `--quiet` прибирає все, крім помилок. Анімацій і рядків, що перемальовуються, немає: спінер скрінрідер читає символами (CHI 2021; GitHub CLI).
- **Зайнятий профіль.** Коли базу зайняла інша дія revisor, revisor-cli чекає за правилом ядра, як вікно (ADR-0012), один раз каже про це в stderr, а Ctrl+C скасовує. `--timeout <секунди>` обмежує чекання (`0` — одразу збій, як `flock -w 0`), а далі — збій із кодом 1; без нього чекання не обмежене, як у вікні.
- **Звук і відкриття файлу.** Немає, хоч би що казали налаштування вікна (звук наприкінці з TASK-46, «Відкривати файл після експорту»): чим закінчити, вирішує скрипт.
- **Документи.** Рішення про окремий exe записує нова ADR-0015, а ADR-0005 (крейт командного рядка) і ADR-0010 (реліз — один exe) отримують доповнення з посиланням на неї. Як саме збирати revisor-cli (окремий крейт чи другий exe крейта застосунку), вирішує план задачі.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Поруч із revisor.exe є консольний revisor-cli.exe: cmd, .bat і PowerShell чекають на нього й отримують його код виходу, а вікно revisor, як і раніше, відкривається без консолі.
- [ ] #2 revisor-cli snapshot -p <профіль> робить знімок профілю, як вікно, із коментарем з --comment, і пише в stdout лише номер нового знімка.
- [ ] #3 revisor-cli compare -p <профіль> -o <файл> порівнює останній знімок із попереднім, а з --with <номер> — із вибраним, і записує всю різницю без фільтрів вікна; файл заступає наявний лише цілим, без питань.
- [ ] #4 .csv у --output дає CSV, інше розширення — текстовий звіт, а --format text|csv вибирає формат явно; формат той самий, що й експорту з вікна (TASK-39, TASK-40).
- [ ] #5 Повідомлення, --help і текстовий звіт типово англійською, а --language uk — українською; формати текстового звіту — як у вікні.
- [ ] #6 Роздільник CSV — з налаштування «Роздільник у CSV», а --delimiter comma|semicolon|tab|windows задає його явно.
- [ ] #7 --profile обов'язковий і шукає профіль за назвою без огляду на регістр; невідома назва, профіль без двох знімків чи невідомий номер --with дають помилку з причиною (для назви — з наявними профілями).
- [ ] #8 Коди виходу: 0 — готово (і зі знімком, де є недоступні записи), 1 — збій чи скасування Ctrl+C, 2 — неправильний виклик.
- [ ] #9 У stderr — рядок про початок і підсумок наприкінці: для знімка — записи, недоступні з порадою запустити від імені адміністратора й теки, яких немає; для порівняння — файл і підсумок змін. --progress додає рядок прогресу раз на 10 с, --quiet лишає лише помилки; анімацій і рядків, що перемальовуються, немає.
- [ ] #10 Коли профіль зайнятий іншою дією revisor, revisor-cli чекає й один раз каже про це, Ctrl+C скасовує, а --timeout <секунди> обмежує чекання (0 — одразу збій).
- [ ] #11 Однолітерні лише -p, -o, -q, -h; --help і <команда> --help описують команди й параметри без колонок, вирівняних пробілами, а --version називає версію.
- [ ] #12 revisor-cli не грає звуків і не відкриває звіту.
- [ ] #13 Тести покривають розбір параметрів і неправильні виклики, вибір формату, вибір знімків, коди виходу й запис звіту.
- [ ] #14 Реліз публікує обидва exe з .sha256 і перевіряє кожен (розмір, без DLL C runtime, VERSIONINFO); маніфест scoop у packaging дає обидві команди.
- [ ] #15 ADR-0015 і доповнення ADR-0005 та ADR-0010; бриф, «Випуск релізу», README обома мовами й CHANGELOG описують командний рядок.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 Тести (cargo test) проходять
- [ ] #2 cargo clippy і cargo fmt --check без зауважень
- [ ] #3 Зміни інтерфейсу перевірено вручну з NVDA в українському інтерфейсі (якщо задача їх містить)
- [ ] #4 Нотатки виконання й підсумок задачі заповнено
- [ ] #5 Бриф і ADR чи README оновлено (якщо задача змінила поведінку чи рішення)
- [ ] #6 Нові вікна й контроли мають клавіші доступу за документом «Клавіші доступу» (якщо задача їх додає)
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Рішення плану (2026-11-14, з автором). Спершу revisor-cli було зібрано другим [[bin]] крейта застосунку, але так wxWidgets потрапляє в revisor-cli.exe (5,3 МБ) через #[no_mangle]-функції wxdragon, які rustc лишає в кожному exe з бібліотекою застосунку. Автор вирішив розплутати зараз. Остаточна будова — три бібліотечні шари й два exe:
- revisor-core — як був, плюс snapshot::take_when_free: чекання зайнятої бази за правилом ADR-0012;
- revisor-ui (новий, без wx; назву обрав автор) — усе спільне для вікна й командного рядка: i18n (рядки обома мовами, каталог вичитки), commands (лише ідентифікатори CommandId, MenuId, SubmenuId, Pane, Reason — рядки їх називають), difference (модель різниці, звіт, CSV, дерево), details, export (запис через тимчасовий файл), failure, format (колишній win32/format), snapshot_text, data (тека data, файл налаштувань, тека профілів), cli (типи Command і UsageError, які називають рядки);
- revisor (застосунок) — вікна на wxDragon; таблиця команд, меню й клавіші лишаються тут; Arrival/arrival/arrival_after_snapshot з difference.rs переїжджають у застосунок (вони про фокус вікна); каталог застосунку додає до каталогу revisor-ui пункти меню з клавішами;
- revisor-cli (новий крейт, консольний exe) — main.rs, args.rs (lexopt), interrupt.rs (Ctrl+C), власні build.rs і resources/cli.rc (іконка з crates/revisor/resources), тести модуля й тест зібраного exe.
Довідку, помилки й коди виходу пишемо самі; типово англійською, --language uk — українською. Самого релізу 0.5.0 задача не робить.

1. [зроблено] Ядро: snapshot::take_when_free і SnapshotWork::when_free, Progress::is_waiting, тести на чекання й скасування.
2. [зроблено, проміжний стан, зафіксовано комітом] revisor-cli як другий exe крейта застосунку: модуль cli (виконання, args, interrupt), рядки в i18n обома мовами й у каталозі, тести розбору, вибору знімків, кодів виходу, запису звітів і зайнятого профілю; cli.rc і build.rs на два exe; тест versioninfo для обох .rc; тест зібраного exe (консольна підсистема, коди виходу через cmd і PowerShell).
3. Крейт revisor-ui: перенести i18n, difference (без Arrival), details, export (без open), failure, win32/format → format, snapshot_text; додати commands (ідентифікатори), cli (типи), data; plain з access → i18n. Каталог: entries() для всіх методів Strings, перелік CommandId через all!; тест «каталог викликає кожен метод» лишається з каталогом.
4. Застосунок: lib.rs бере модулі з revisor_ui через use, щоб шляхи crate::difference тощо не змінилися; commands.rs і access.rs реекспортують ідентифікатори й plain, win32/mod.rs — format; Arrival з тестами — в окремий модуль застосунку; export::open → win32::shell::open_file; каталог застосунку = каталог revisor-ui + пункти меню з клавішами; приклад strings без змін.
5. Крейт revisor-cli: cli.rs → main.rs, args.rs, interrupt.rs, тести; build.rs (маніфест + cli.rc); Cargo.toml (revisor-core, revisor-ui, lexopt, windows-sys Console; dev rusqlite). Прибрати [[bin]] і cli.rc з крейта застосунку, build.rs застосунку повернути до одного .rc. Перевірити розмір revisor-cli.exe у release і що wx у нього не потрапляє.
6. Ліцензії: THIRD-PARTY-LICENSES.txt генерувати для всього workspace (-m Cargo.toml), щоб lexopt був у файлі, який показує «Про revisor»; CI й release.yml беруть ту саму команду.
7. Воркфлоу релізу й пакування (критерій #14, без самого релізу): release.yml збирає й перевіряє обидва exe (розмір ≤ 10 МіБ, без DLL C runtime, VERSIONINFO), публікує чотири файли, обидва PDB — в артефакт; packaging/scoop/revisor.json: масиви url і hash, bin з обома exe, autoupdate для обох.
8. Документи: ADR-0015 переписати (окремий крейт над revisor-ui), доповнення ADR-0005 (три шари) і ADR-0010 (два exe); бриф (розділ 5, структура коду), «Випуск релізу», README.md і README.uk.md (розділ про командний рядок), CHANGELOG (Unreleased).
9. cargo fmt, cargo test --workspace, cargo clippy -D warnings; підсумок; Done; злиття в develop через git flow (--no-ff), без push.
<!-- SECTION:PLAN:END -->
