---
id: TASK-7
title: Профіль задає теки й винятки для знімків
status: In Progress
assignee:
  - '@claude'
created_date: '2026-10-30 15:23'
updated_date: '2026-11-02 02:03'
labels: []
milestone: m-1
dependencies:
  - TASK-15
references:
  - >-
    https://learn.microsoft.com/en-us/windows/msix/packaging-tool/generate-template-file
  - >-
    https://learn.microsoft.com/en-us/windows/win32/backup/registry-keys-for-backup-and-restore
  - >-
    https://learn.microsoft.com/en-us/windows/deployment/update/windows-update-logs
  - 'https://learn.microsoft.com/en-us/windows/deployment/upgrade/log-files'
  - >-
    https://learn.microsoft.com/en-us/windows-hardware/drivers/install/setupapi-text-logs
  - 'https://learn.microsoft.com/en-us/windows/win32/eventlog/eventlog-key'
  - 'https://technet.microsoft.com/en-us/library/cc947815.aspx'
  - 'https://learn.microsoft.com/en-us/windows/win32/wer/wer-settings'
  - >-
    https://learn.microsoft.com/en-us/windows/win32/wer/collecting-user-mode-dumps
  - >-
    https://learn.microsoft.com/en-us/troubleshoot/windows-server/performance/memory-dump-file-options
  - >-
    https://learn.microsoft.com/en-us/windows-server/remote/remote-desktop-services/rds-vdi-recommendations-2004
  - >-
    https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-gettemppath2w
  - >-
    https://learn.microsoft.com/en-us/windows/win32/secauthz/implementing-an-appcontainer
  - 'https://support.microsoft.com/en-us/kb/313472'
  - >-
    https://learn.microsoft.com/en-us/archive/blogs/chadduffey/what-is-application-launch-prefetching
  - >-
    https://learn.microsoft.com/en-us/troubleshoot/windows-client/installing-updates-features-roles/additional-resources-for-windows-update
  - >-
    https://learn.microsoft.com/en-us/defender-endpoint/microsoft-defender-antivirus-updates
  - >-
    https://devblogs.microsoft.com/windows-search-platform/configuration-and-settings
  - >-
    https://learn.microsoft.com/en-us/previous-versions/troubleshoot/browsers/security-privacy/apps-access-admin-web-cache
  - >-
    https://learn.microsoft.com/en-us/windows/win32/api/shlobj_core/nf-shlobj_core-shaddtorecentdocs
  - >-
    https://learn.microsoft.com/en-us/troubleshoot/windows-server/certificates-and-public-key-infrastructure-pki/event-id-4107-or-event-id-11-is-logged
  - 'https://microsoft.github.io/DirectX-Specs/d3d/ShaderCache.html'
  - >-
    https://learn.microsoft.com/en-us/windows/client-management/mdm/policy-csp-admx-thumbnails
  - >-
    https://learn.microsoft.com/en-us/troubleshoot/windows-client/shell-experience/recycle-bin-corrupted
  - 'https://devblogs.microsoft.com/oldnewthing/20031120-00/?p=41753'
  - >-
    https://learn.microsoft.com/en-us/previous-versions/windows/it-pro/windows-server-2003/cc736395(v=ws.10)
  - >-
    https://learn.microsoft.com/en-us/windows/win32/cimwin32prov/win32-pagefilesetting
  - >-
    https://learn.microsoft.com/en-us/troubleshoot/windows-client/setup-upgrade-and-drivers/disable-and-re-enable-hibernation
  - 'https://learn.microsoft.com/en-us/windows/win32/sysinfo/registry-hives'
  - >-
    https://chromium.googlesource.com/chromium/src/+/main/content/browser/network_service_instance_impl.h
  - >-
    https://chromium.googlesource.com/chromium/src/+/main/content/browser/storage_partition_impl.cc
  - >-
    https://chromium.googlesource.com/chromium/src/+/main/gpu/ipc/common/gpu_disk_cache_type.cc
  - >-
    https://chromium.googlesource.com/chromium/src/+/refs/tags/120.0.6099.71/gpu/ipc/common/gpu_disk_cache_type.cc
  - >-
    https://chromium.googlesource.com/chromium/src/+/main/chrome/browser/chrome_content_browser_client.cc
  - >-
    https://chromium.googlesource.com/chromium/src/+/main/components/services/storage/public/cpp/constants.cc
  - 'https://www.electronjs.org/docs/latest/api/app'
  - 'https://firefox-source-docs.mozilla.org/toolkit/profile/index.html'
  - >-
    https://searchfox.org/mozilla-central/source/netwerk/cache2/CacheFileIOManager.cpp
  - 'https://searchfox.org/mozilla-central/source/startupcache/StartupCache.cpp'
  - 'https://peps.python.org/pep-3147/'
  - >-
    https://github.com/duplicati/duplicati/blob/master/Duplicati/Library/Utility/FilterGroups.cs
  - 'https://github.com/CrashPlan-Labs/Tools/blob/main/exclusions.txt'
  - 'https://www.backblaze.com/computer-backup/docs/configure-exclusions-windows'
  - 'https://help.backblaze.com/hc/en-us/articles/217665528'
  - 'https://kbx.macrium.com/macrium-reflect-x/backup-defaults'
  - 'https://www.acronis.com/en-us/support/documentation/ATI2025/13778.html'
  - >-
    https://helpcenter.veeam.com/docs/agentforwindows/userguide/backup_job_folders.html
documentation:
  - backlog/docs/doc-2 - Бриф-продукту.md
  - >-
    backlog/docs/adr/doc-9 -
    ADR-0006-Налаштування-профілю-—-у-його-базі-SQLite.md
  - backlog/docs/doc-11 - Клавіатура-й-меню.md
  - >-
    backlog/docs/adr/doc-12 -
    ADR-0008-Команди-—-в-одній-таблиці-а-клавіші-revisor-обробляє-сам.md
modified_files:
  - crates/revisor-core/examples/diff_bench.rs
  - crates/revisor-core/examples/store_bench.rs
  - crates/revisor-core/src/diff.rs
  - crates/revisor-core/src/diff/tests.rs
  - crates/revisor-core/src/exclude.rs
  - crates/revisor-core/src/exclude/tests.rs
  - crates/revisor-core/src/folders.rs
  - crates/revisor-core/src/folders/tests.rs
  - crates/revisor-core/src/lib.rs
  - crates/revisor-core/src/names.rs
  - crates/revisor-core/src/profiles.rs
  - crates/revisor-core/src/profiles/tests.rs
  - crates/revisor-core/src/scan.rs
  - crates/revisor-core/src/scan/tests.rs
  - crates/revisor-core/src/store.rs
  - crates/revisor-core/src/store/tests.rs
  - crates/revisor/src/commands.rs
  - crates/revisor/src/failure.rs
  - crates/revisor/src/i18n/en.rs
  - crates/revisor/src/i18n/mod.rs
  - crates/revisor/src/i18n/plural.rs
  - crates/revisor/src/i18n/uk.rs
  - crates/revisor/src/main.rs
  - crates/revisor/src/main_window.rs
  - crates/revisor/src/menus.rs
  - crates/revisor/src/message_box.rs
  - crates/revisor/src/profile_window.rs
  - crates/revisor/src/state.rs
  - crates/revisor/src/win32.rs
  - backlog/docs/doc-11 - Клавіатура-й-меню.md
  - backlog/docs/doc-2 - Бриф-продукту.md
type: feature
ordinal: 7000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Для різних задач користувач відстежує різні частини файлової системи: наприклад, теки програм або власні робочі теки. Профіль — це іменований набір тек із винятками й налаштуваннями, у якого є власний перелік знімків. Узгоджений термін: «Профіль» / «Profile». Назви профілів не повторюються, щоб профілі можна було розрізнити в переліку.

Теки одного профілю не вкладаються одна в одну, інакше їхні файли потрапили б у знімок двічі. Тому теку всередині теки профілю додати не можна, а ширшу теку revisor пропонує поставити замість вкладених у неї. Теки порівнюються за справжнім розташуванням на диску, тож ту саму теку не вийде додати й під іншим шляхом (через junction чи диск `subst`).

Винятки потрібні, бо інакше знімок засмічують кеші, тимчасові файли й журнали, і справжні зміни тонуть у шумі. Типові винятки новий профіль отримує як звичайні винятки, які можна редагувати. Так усе видно в одному місці й прихованих правил немає. Наслідок: зміна типового списку не зачіпає вже створені профілі.

Шаблон винятку без `\` — це ім'я файлу чи теки на будь-якій глибині (`*.tmp`, `Code Cache`): так один рядок виключає однакові теки кешу різних програм, де б вони не лежали. Шаблон із `\` — повний шлях. Він може починатися зі змінної середовища (`%TEMP%`, `%LOCALAPPDATA%`, `%SystemRoot%`, `%ProgramData%`), яку revisor розгортає на поточному комп'ютері. Так типовий список працює й тоді, коли TEMP перенесено в іншу теку чи Windows стоїть не на диску C:. `*` і `?` діють у межах однієї частини шляху, регістр літер не важить, а тека, що відповідає шаблону, виключається з усім вмістом.

Типовий список складено лише з документації та усталеної практики, без даних окремого комп'ютера (2026-10-31). До нього входить лише те, що (1) описано в документації Microsoft чи виробника або у вихідному коді чи є в типових списках щонайменше двох незалежних програм; (2) містить тимчасові, відтворювані чи службові дані, які Windows або програма змінює сама; (3) не ховає справжніх змін програми. Найближча практика — зразок винятків MSIX Packaging Tool, який теж знімає систему до й після встановлення. Джерела — у полі references. Список (63 шаблони):
- системні файли й теки томів: `$Recycle.Bin`, `System Volume Information`, `Config.Msi`, `pagefile.sys`, `swapfile.sys`, `hiberfil.sys`, `$Windows.~BT`, `$Windows.~WS`;
- тимчасові файли: `%TEMP%`, `%LOCALAPPDATA%\Temp`, `%SystemRoot%\Temp`, `%SystemRoot%\SystemTemp`, `%LOCALAPPDATA%\Packages\*\AC\Temp`, `*.tmp`, `~$*`;
- журнали й діагностика Windows: `%SystemRoot%\Logs`, `%SystemRoot%\debug`, `%SystemRoot%\Panther`, `%SystemRoot%\INF\setupapi.*.log`, `%SystemRoot%\System32\winevt\Logs`, `%SystemRoot%\System32\LogFiles`, `%SystemRoot%\System32\MSDtc\MSDTC.LOG`, `%SystemRoot%\System32\MSDtc\trace\dtctrace.log`, `%SystemRoot%\Minidump`, `%SystemRoot%\MEMORY.DMP`, `%SystemRoot%\LiveKernelReports`, `%ProgramData%\Microsoft\Windows\WER`, `%LOCALAPPDATA%\Microsoft\Windows\WER`, `%LOCALAPPDATA%\CrashDumps`, `%ProgramData%\USOShared\Logs`;
- служби Windows, що оновлюються самі: `%SystemRoot%\Prefetch`, `%SystemRoot%\SoftwareDistribution`, `%SystemRoot%\WinSxS\ManifestCache`, `%ProgramData%\Microsoft\Network\Downloader`, `%ProgramData%\Microsoft\Search\Data`, `%ProgramData%\Microsoft\Windows Defender`, `%ProgramFiles%\Windows Defender`;
- кеші Windows: `%SystemRoot%\CSC`, `%LOCALAPPDATA%\Microsoft\Windows\Explorer`, `%LOCALAPPDATA%\Microsoft\Windows\INetCache`, `%LOCALAPPDATA%\Microsoft\Windows\INetCookies`, `%LOCALAPPDATA%\Microsoft\Windows\WebCache`, `%LOCALAPPDATA%\Microsoft\Windows\History`, `%APPDATA%\Microsoft\Windows\Recent`, `CryptnetUrlCache`, `D3DSCache`, `Thumbs.db`;
- кущі реєстру користувачів: `NTUSER.DAT*`, `UsrClass.dat*` (revisor реєстр не знімає, а ці файли змінюються майже з кожним входом);
- кеші браузерів і програм на Chromium чи Electron, Firefox і Python: `Cache_Data`, `No_Vary_Search`, `Logical_Invalidation`, `Code Cache`, `GPUCache`, `ShaderCache`, `GrShaderCache`, `GraphiteDawnCache`, `DawnGraphiteCache`, `DawnWebGPUCache`, `DawnCache`, `cache2`, `startupCache`, `__pycache__`.

Навмисно не виключаємо: цілі Windows, Program Files, ProgramData й AppData, а також `%SystemRoot%\Installer`, `WinSxS` (крім `ManifestCache`), `Servicing`, `Packages`, `WindowsApps` і сховища ключів, бо туди пише й сама програма; правила на розширення (`*.log`, `*.etl`, `*.db`), бо це й сліди програми; загальні імена `Cache`, `Temp`, `tmp`; `CacheStorage` і `ScriptCache` Chromium, бо це дані сайтів, а не кеш; системні кущі в `%SystemRoot%\System32\config`, бо поруч лежить профіль SYSTEM; кеш Delivery Optimization, бо його теку не задокументовано. Назви кешів Chromium змінюються між версіями (`DawnCache` став `DawnWebGPUCache`), тож список варто переглядати.

Знімки профілю зберігаються в окремій базі SQLite (див. ADR-0002 у `docs/adr`). Налаштування профілю (назва, теки й винятки) лежать у тій самій базі (ADR-0006). Так профіль — це один файл у теці `data`: його можна скопіювати, перенести чи видалити цілком, і налаштування не розійдуться зі знімками. Налаштування мігрують разом зі схемою бази (ADR-0003) і змінюються транзакцією. Опція хешу вмісту з'явиться в 0.x (окрема чернетка «Хеш вмісту як опція профілю»).

Кожен знімок запам'ятовує теки й винятки, з якими його зроблено. Якщо їх змінили між двома знімками, порівняння охоплює лише те, що охоплюють обидва знімки, і попереджає, що лишилося поза порівнянням: інакше зміна налаштувань дала б у різниці фальшиві додані й видалені записи (задача TASK-10 «Порівняння двох знімків показує додані, видалені й змінені файли»).

Профіль видаляється разом зі своєю базою, тобто з усіма знімками: це одна дія з одним файлом, і місце на диску звільняється одразу. Щоб нічого не зникло без відома користувача, revisor спершу просить підтвердження й називає, скільки знімків буде видалено.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Можна мати кілька профілів, кожен зі своєю назвою.
- [ ] #2 Профіль можна створити, змінити й видалити.
- [ ] #3 У профілі вказують одну або кілька тек для відстеження.
- [ ] #4 У профілі можна задати винятки — шаблони шляхів і імен; файли й теки, що їм відповідають, не потрапляють у знімок.
- [ ] #5 Новий профіль отримує типовий список винятків із опису задачі (63 шаблони, саме в такому записі), і його можна редагувати як звичайні винятки профілю.
- [ ] #6 Кожен профіль має власний перелік знімків.
- [ ] #7 Теку на томі, крім NTFS (флешка FAT32 чи exFAT, Dev Drive на ReFS, мережева тека), не можна додати до профілю: revisor пояснює, що підтримуються лише томи NTFS.
- [ ] #8 Налаштування профілю (назва, теки й винятки) зберігаються в його базі разом зі знімками, тож профіль — це один файл бази в теці `data`.
- [ ] #9 Профіль видаляється разом із базою й усіма знімками лише після підтвердження: у ньому названо профіль і кількість його знімків і сказано, що дію не скасувати. Без підтвердження нічого не видаляється.
- [ ] #10 Шаблон винятку без `\` відповідає імені файлу чи теки на будь-якій глибині: наприклад, `Code Cache` виключає такі теки в усіх теках профілю.
- [ ] #11 Шаблон винятку з `\` відповідає повному шляху й може починатися зі змінної середовища (`%TEMP%`, `%LOCALAPPDATA%`, `%SystemRoot%`, `%ProgramData%` тощо): revisor розгортає її на поточному комп'ютері, коли робить знімок.
- [ ] #12 У шаблонах винятків `*` і `?` діють у межах однієї частини шляху, а регістр літер не враховується.
- [ ] #13 Тека, що відповідає винятку, не потрапляє в знімок разом з усім вмістом.
- [ ] #14 Кожен знімок зберігає теки й винятки профілю, з якими його зроблено; пізніші зміни профілю цих даних не змінюють.
- [ ] #15 Теку, яка вже є в профілі або лежить усередині його теки, додати не можна: revisor пояснює, що її файли вже охоплено. Теки порівнюються за справжнім розташуванням на диску, тож регістр літер, `\` у кінці й шлях через junction чи диск `subst` на перевірку не впливають.
- [ ] #16 Коли нова тека містить одну чи кілька тек профілю, revisor пропонує замінити їх нею: після згоди вкладені теки зникають із профілю, а нова тека додається; після відмови профіль не змінюється.
- [ ] #17 Назви профілів не повторюються: створити чи перейменувати профіль із назвою, яка вже є (без урахування регістру), не можна.
- [ ] #18 Команди «Новий профіль…» (Ctrl+N), «Змінити профіль…» і «Видалити профіль…» стоять у меню «Файл» і таблиці команд TASK-15 з назвами й клавішами з документа «Клавіатура й меню»; для кожної задано, коли вона доступна, і причину, яку revisor каже, коли ні, а рядки інтерфейсу задачі мають англійську й українську версії.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 Тести (cargo test) проходять
- [ ] #2 cargo clippy і cargo fmt --check без зауважень
- [ ] #3 Нотатки виконання й підсумок задачі заповнено
- [ ] #4 Бриф і ADR чи README оновлено (якщо задача змінила поведінку чи рішення)
- [ ] #5 Нові вікна й контроли мають клавіші доступу за документом «Клавіатура й меню» (якщо задача їх додає)
- [ ] #6 Зміни інтерфейсу перевірено вручну з NVDA в українському інтерфейсі (якщо задача їх містить)
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Рішення автора (2026-11-02, усі — рекомендовані варіанти)

1. TASK-7 заповнює випадний список «Профіль:». Профілі стоять за абеткою без урахування регістру, вибраний профіль — відкритий, і його назва в заголовку. Новий профіль стає відкритим; після видалення й під час запуску відкривається перший за абеткою. TASK-9 додає перелік знімків, запам'ятовування профілю й підказку «Профілів ще немає»; її критерії лишаються, а в нотатках TASK-9 буде записано, що вже зроблено.
2. Файл профілю — `data\profiles\<назва>.db`. Символи, заборонені в іменах файлів, замінено на «_», а якщо ім'я зайняте, додано номер. Перейменування профілю перейменовує й файл, а показано назву, записану в базі.
3. Винятки редагують у багаторядковому полі, по одному шаблону на рядок; порожні рядки й пробіли на краях відкидаються.
4. Винятки діють лише на вміст тек профілю, а не на самі теки.
5. Винятки підключено до сканера ядра вже в TASK-7; той самий зіставлювач потім візьме порівняння (TASK-10 #19).
6. До випуску 0.1.0 схема бази лишається версією 1 без міграцій; правило записати в бриф.
7. Рядок винятків revisor не приймає, лише коли його значення неясне: шаблон із `\` не починається з диска (`C:\`) чи змінної (`%TEMP%`), або в рядку є `/ < > " |`, керівний символ чи `:` не після літери диска. revisor називає номер рядка й причину, а вікно лишається відкритим. Повтори й невідомі змінні приймаються.

## Ядро (revisor-core)

- `names`: регістр як у NTFS (`upcase` переходить сюди з diff) і порівняння без урахування регістру.
- `exclude`:
  - розбір і перевірка шаблонів, типовий список із 63 шаблонів саме в записі з опису задачі;
  - шаблон без `\` відповідає імені на будь-якій глибині; шаблон із `\` або з провідною `%ЗМІННОЮ%` — повному шляху;
  - змінна розгортається, коли збирається знімок; наявну частину шляху без `*`/`?` зведено до справжнього розташування (GetFinalPathNameByHandleW), тож 8.3-імена в %TEMP% (Windows скорочує TEMP, Raymond Chen) і junction не заважають;
  - `*` і `?` діють у межах однієї частини шляху, регістр не важить;
  - середовище (змінні, справжній шлях) — трейт, щоб тести його підміняли.
- `scan`: `ScanOptions.exclusions`. Запис, що відповідає винятку, не потрапляє в знімок, а в теку, що відповідає, сканер не заходить; корінь не перевіряється. Повні шляхи зіставляються від справжнього розташування кореня.
- `store`:
  - схема 1 доповнена таблицями `profile` (назва), `profile_folder`, `profile_exclusion`, `snapshot_folder` і `snapshot_exclusion` та `PRAGMA application_id`;
  - `Store::create(path, settings)`; `open` відкриває лише наявну базу профілю;
  - `settings` і `set_settings` (транзакцією);
  - `save(scope, …)` зберігає теки й винятки знімка в тій самій транзакції; `snapshot_scope(id)`, `snapshot_count()`.
- `profiles`: файли профілів у теці — перелік (назви читаються з баз), створення, зміна з перейменуванням файлу (якщо файл зайнятий, лишається старе ім'я), видалення разом із `-journal`, унікальність назв без урахування регістру, ім'я файлу з назви (заборонені символи, імена пристроїв, крапки й пробіли в кінці, довжина).
- `folders`:
  - `locate`: справжнє розташування теки (VOLUME_NAME_GUID, а без нього — DOS чи NT) і перевірка тому: лише NTFS, мережа (`\\?\UNC\`, `\Device\Mup\`) не підтримується;
  - `check_addition`: «уже охоплено» (та сама тека чи вкладена) або «замінить вкладені» (список).

## Застосунок (revisor)

- Команди: «Новий профіль…» (Ctrl+N) доступна завжди; «Змінити профіль…» і «Видалити профіль…» — коли є профіль, інакше «профілів ще немає».
- Вікно профілю (`profile_window.rs`):
  - заголовок «Новий профіль» / «Змінити профіль»;
  - «Назва (N):», «Теки (F):», «Додати теку (A)…» (стандартне вікно вибору теки), «Прибрати теку (R)», «Винятки (E):»;
  - кнопка «Створити» / «OK» (Enter) і «Скасувати» (Esc);
  - Alt+літера розпізнається за положенням клавіші в `on_char_hook` вікна, як у головному вікні.
- «Додати теку…»:
  - том не NTFS чи мережева тека → пояснення, що підтримуються лише томи NTFS;
  - тека вже охоплена → пояснення, яка тека профілю її охоплює;
  - нова тека містить теки профілю → питання з кнопками «Замінити» (типова) і «Скасувати»;
  - фокус переходить на додану теку в переліку, а після «Прибрати теку» — на наступну.
- «Створити» / «OK»: порожня назва, назва вже є, жодної теки чи неясний рядок винятків → повідомлення, і фокус повертається до поля (до того рядка).
- Видалення: «Видалити профіль «X» і N знімків у ньому? Цю дію не можна скасувати.» (без знімків — окреме речення); кнопки «Видалити» і «Скасувати», типова «Скасувати», як для знімків у TASK-9.
- Рядок стану: «Профіль «X» створено / змінено / видалено»; файли профілів, яких не вдалося відкрити, revisor називає з причиною.
- «Профіль:» сортується через StrCmpLogicalW, як у Провіднику, а вибір у ньому відкриває профіль.
- Рядки обома мовами, форма множини за CLDR; причини помилок — за правилом брифу.

## Документи

- «Клавіатура й меню»: таблиця клавіш доступу вікна профілю.
- Бриф: `data\profiles\<назва>.db` і правило версії схеми до 0.1.0.
- TASK-9: нотатка про вже зроблену частину «Профіль:».

## TDD-шви

- exclude: розбір, зіставлення, змінні.
- scan із винятками на тимчасових теках (#4, #10–#13).
- store: налаштування; знімок зберігає свої теки й винятки після зміни профілю (#14).
- profiles: створення, перейменування, видалення, імена файлів, унікальність.
- folders: регістр, `\` у кінці, junction, subst, вкладення.
- Команди й рядки застосунку.

## Перевірка

cargo test, clippy, fmt; перевірка з NVDA (сам, після дозволу автора); code review; коміт у `feature/profiles`.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Що зроблено

Гілка `feature/profiles`.

Ядро (`crates/revisor-core/src/`):
- `exclude.rs` — шаблони винятків:
  - перевірка рядків (`check`, `parse_lines`) і типовий список `DEFAULT_EXCLUSIONS` (63 шаблони, як в описі);
  - зіставлювач `Exclusions` із `Branch`: ім'я — на будь-якій глибині; повний шлях може починатися зі змінної, а його наявна частина зводиться до справжнього розташування; `*` і `?` діють у межах частини; регістр — як у NTFS;
  - середовище `Environment` / `SystemEnvironment`, яке тести підміняють.
- `scan.rs` — `ScanOptions.exclusions`. Виключений запис не потрапляє в знімок, у виключену теку сканер не заходить, а корінь не перевіряється. Повні шляхи зіставляються від справжнього розташування кореня.
- `store.rs`:
  - у схемі 1 — таблиці `profile`, `profile_folder`, `profile_exclusion`, `snapshot_folder`, `snapshot_exclusion` і `PRAGMA application_id` («RVPR»);
  - `Store::create` не займає наявного файлу, а `open` відкриває лише наявну базу профілю;
  - `settings` / `set_settings` (транзакцією), `save(scope, …)`, `snapshot_scope`.
- `profiles.rs` — профілі в теці: `list`, `name_taken`, `create`, `update` (перейменовує й файл), `delete` (разом із `-journal`), ім'я файлу з назви.
- `folders.rs`:
  - `locate`: справжнє розташування теки (GetFinalPathNameByHandleW, спершу GUID, далі DOS чи NT); лише NTFS (GetVolumeInformationByHandleW) і не мережа (`\\?\UNC\`, `\Device\Mup\`);
  - `check_addition` і `real_path`.
- `names.rs` — `upcase` (перенесено з diff) і `eq_ignore_case`.

Застосунок (`crates/revisor/src/`):
- `profile_window.rs` — вікно «Новий профіль» / «Змінити профіль»:
  - назва, теки з кнопками «Додати теку…» (стандартне вікно вибору теки) і «Прибрати теку», винятки в багаторядковому полі;
  - клавіші доступу за положенням клавіші (`on_char_hook`);
  - теку перевіряє одразу, решту — на «Створити» / «OK»; вікно не закривається, доки профіль не збережено.
- `main_window.rs` — «Профіль:» за абеткою (StrCmpLogicalW), заголовок, команди «Новий профіль…», «Змінити профіль…» і «Видалити профіль…» з підтвердженням, повідомлення в рядку стану.
- `message_box.rs` — класичні вікна повідомлень Windows (MessageBoxW) з кнопками revisor через хук CBT.
- `failure.rs` — причини помилок за правилом брифу:
  - своїми словами: немає доступу, файл відкрито в іншій програмі, немає місця, профіль новішої версії, не профіль;
  - решта — текст і код Windows чи SQLite.
- `i18n` — рядки TASK-7 обома мовами; кількість знімків — у формі множини за CLDR.
- `commands.rs` — «Новий профіль…» доступна завжди.
- `win32.rs` — `compare_names` (StrCmpLogicalW).

Документи:
- «Клавіатура й меню» — таблиця клавіш доступу вікна профілю й клавіші цього вікна;
- бриф — файл `data\profiles\<назва>.db` і версія схеми 1 до 0.1.0;
- TASK-9 — нотатка, що «Профіль:» уже заповнено.

## Що з'ясовано

- wxDragon кличе обробник з `Skip(true)`, тож обробник «OK» сам кличе `skip(false)`, інакше вікно закрилося б без перевірки.
- Windows тримає `%TEMP%` у формі 8.3 (Raymond Chen, «Why does the system convert TEMP to a short file name?»), тож наявну частину шаблону зведено до справжнього розташування.
- GetFinalPathNameByHandleW розгортає junction і диск `subst`; VOLUME_NAME_GUID для мережевих тек не працює.
- У NVDA автора вимкнено «Повідомляти опис об'єктів» (`reportObjectDescriptions = False`), а текст діалогу NVDA подає як опис. Тому NVDA сам не читає тексту жодного вікна повідомлень, ні TaskDialog wxMessageDialog, ні класичного MessageBox, а NVDA+B читає. Спершу я хибно шукав причину в TaskDialog і перейшов на MessageBoxW. Перехід лишив, бо NVDA+B читає класичне вікно повністю (перевірено).

## Перевірка з NVDA 2026.2, українська розкладка (2026-11-02)

Мовлення — з нового хвоста IO-журналу NVDA, клавіші — `keybd_event` і `SendInput`.
- Ctrl+N відкриває «Новий профіль діалог» із фокусом на «Назва (N): редактор Alt+N».
- Alt+N, Alt+F, Alt+A, Alt+R і Alt+E спрацьовують за української розкладки.
- Додана тека отримує фокус у переліку. На вкладену теку revisor відповідає повідомленням, на ширшу — питанням «Замінити»; після заміни в переліку лишається одна ширша тека. Коли тек не лишилося, «Прибрати теку» переносить фокус на «Додати теку (A)…».
- Порожня назва, профіль без тек чи назва, що вже є, дають повідомлення, текст якого читає NVDA+B. Після нього фокус повертається до поля, а назва виділена.
- Рядок стану: «Профіль «Тест» створено.», «Профіль «Третій» змінено.», «Профіль «Третій» видалено.».
- Заголовок: «Тест — revisor», а без профілів — «revisor». Після перейменування файл `Другий.db` став `Третій.db`.
- У підтвердженні видалення типова кнопка «Скасувати»: Enter і Esc нічого не видаляють, а Shift+Tab і «Видалити» видаляють.
- Без профілів «Змінити профіль (E)…» і «Видалити профіль (D)…» читаються як «недоступно».
- Esc закриває вікно профілю, нічого не створивши.
- З NVDA не перевірено: фокус і виділення рядка винятків, що не пройшов перевірки (скрипт збився на вікні вибору теки); логіку покриває тест `exclude`.
<!-- SECTION:NOTES:END -->
