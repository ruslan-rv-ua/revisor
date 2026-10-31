---
id: TASK-7
title: Профіль задає теки й винятки для знімків
status: To Do
assignee: []
created_date: '2026-10-30 15:23'
updated_date: '2026-10-31 22:06'
labels: []
milestone: m-1
dependencies: []
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
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 Тести (cargo test) проходять
- [ ] #2 cargo clippy і cargo fmt --check без зауважень
- [ ] #3 Зміни інтерфейсу перевірено вручну з NVDA (якщо задача їх містить)
- [ ] #4 Нотатки виконання й підсумок задачі заповнено
- [ ] #5 Бриф і ADR чи README оновлено (якщо задача змінила поведінку чи рішення)
<!-- DOD:END -->
