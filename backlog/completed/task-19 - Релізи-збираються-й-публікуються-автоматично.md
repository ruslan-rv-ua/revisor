---
id: TASK-19
title: Релізи збираються й публікуються автоматично
status: Done
assignee:
  - '@claude'
created_date: '2026-10-30 17:04'
updated_date: '2026-11-07 01:34'
labels: []
milestone: m-1
dependencies:
  - TASK-7
  - TASK-8
  - TASK-9
  - TASK-10
  - TASK-11
  - TASK-12
  - TASK-13
  - TASK-14
  - TASK-15
  - TASK-16
  - TASK-17
  - TASK-18
  - TASK-25
  - TASK-26
references:
  - >-
    https://github.com/ruslan-rv-ua/pathmaster/blob/develop/.github/workflows/release.yml
  - >-
    https://github.com/ruslan-rv-ua/pathmaster/blob/develop/docs/release-checklist.md
  - 'https://github.com/ruslan-rv-ua/pathmaster/blob/develop/CHANGELOG.md'
  - >-
    https://github.com/ruslan-rv-ua/tapir/blob/develop/.github/workflows/release.yml
  - >-
    https://github.com/ruslan-rv-ua/scoop-bucket/blob/main/bucket/pathmaster.json
  - >-
    https://github.com/ruslan-rv-ua/scoop-bucket/blob/main/.github/workflows/excavator.yml
  - 'https://docs.github.com/en/rest/releases/releases#get-the-latest-release'
  - 'https://docs.github.com/en/actions/concepts/security/github_token'
  - 'https://github.com/ScoopInstaller/Scoop/wiki/App-Manifests'
  - 'https://github.com/ScoopInstaller/Scoop/wiki/App-Manifest-Autoupdate'
  - 'https://github.com/ScoopInstaller/Scoop/blob/master/bin/checkver.ps1'
  - 'https://github.com/ScoopInstaller/Scoop/blob/master/lib/autoupdate.ps1'
  - 'https://github.com/ScoopInstaller/Scoop/blob/master/lib/install.ps1'
  - 'https://github.com/ScoopInstaller/Scoop/wiki/Persistent-data'
  - 'https://github.com/ScoopInstaller/GithubActions'
  - 'https://github.com/ScoopInstaller/.github/blob/main/.github/CONTRIBUTING.md'
  - 'https://keepachangelog.com/en/1.1.0/'
  - 'https://semver.org/spec/v2.0.0.html'
  - >-
    https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation
  - >-
    https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options
  - >-
    https://learn.microsoft.com/en-us/windows/apps/develop/smart-app-control/overview
  - 'https://learn.microsoft.com/en-us/azure/artifact-signing/quickstart'
  - 'https://signpath.org/terms'
  - 'https://shop.certum.eu/open-source-code-signing-on-simplysign.html'
documentation:
  - backlog/docs/doc-2 - Бриф-продукту.md
  - >-
    backlog/docs/adr/doc-14 -
    ADR-0010-Випуск-—-GitHub-Actions-за-тегом-публікує-exe-з-.sha256-а-scoop-оновлює-Excavator.md
  - backlog/docs/doc-11 - Клавіатура-й-меню.md
type: chore
ordinal: 15000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
revisor встановлюється через scoop із bucket ruslan-rv-ua/scoop-bucket, тож кожен реліз має з'являтися там без ручної правки маніфесту, а випускатися має саме той код, що пройшов перевірки. Ця задача робить релізний воркфлоу, маніфест scoop і документ «Випуск релізу», а тоді випускає 0.1.0. Її виконують останньою в MVP, коли готові всі інші задачі. Публікацію репозиторію, CI, VERSIONINFO, іконку й файл ліцензій дає TASK-18 «Репозиторій публічний, CI перевіряє кожну зміну в develop, а exe має версію, іконку й ліцензії». Рішення ухвалено 2026-11-01 в обговоренні DRAFT-13 (ADR-0010). Зразок — pathmaster автора: його `release.yml`, `bucket/pathmaster.json` і чек-лист випуску.

**Реліз.** Тег vX.Y.Z ставить у main `git flow release finish`. Ключ `gitflow.branch.release.tagprefix = v` лежить лише в `.git/config`, тож у свіжому клоні тег вийде без `v`, і воркфлоу не запуститься. Тег запускає релізний воркфлоу на `windows-2025`. Кроки в ньому впорядковано так, щоб дешеві перевірки падали першими, а файли публікувалися лише після всіх перевірок:
- текст релізу береться з розділу версії в CHANGELOG.md; якщо розділу немає чи він порожній, реліз падає: порожньої сторінки релізу ніхто не помітить, а червоний запуск помітять;
- форматування, тести й clippy ще раз, уже на тегу: коміт у main жоден запуск CI з TASK-18 не бачив;
- версія тегу має збігатися з Cargo.toml і VERSIONINFO;
- перевірки готового exe: таблиця імпорту без DLL рантайму C (загублений `+crt-static` інакше лишає збірку зеленою), розмір до 10 МіБ (бриф) і VERSIONINFO, прочитана з самого exe;
- `.sha256` у форматі sha256sum «<hex64> *<ім'я>»: його читають `Get-FileHash`, `sha256sum -c` і autoupdate scoop.

На сторінці релізу рівно два файли: `revisor-vX.Y.Z-x64.exe` і `revisor-vX.Y.Z-x64.exe.sha256`. PDB лишається артефактом запуску на 90 днів: лише з ним можна розібрати збій exe в користувача. Окремий exe замість zip: користувачеві скрінрідера не треба розпаковувати архів заради одного файлу, а revisor — один exe без DLL. Exe не підписано: через scoop файл ставиться без мітки Mark-of-the-Web, а для прямого завантаження README пояснює попередження SmartScreen і перевірку хешу. Тег із дефісом (`v0.1.0-rc.1`) воркфлоу публікує як pre-release. Такий реліз пропускають і API «latest release», і checkver scoop, а непозначений `-rc.1` scoop прочитав би як «0.1.0-».

**Історія змін.** CHANGELOG.md пишемо англійською за Keep a Changelog 1.1.0: `[Unreleased]` угорі, розділи «## [x.y.z] - YYYY-MM-DD», посилання на порівняння версій унизу. Розділ 0.1.0 описує можливості MVP, а далі кожна помітна зміна дописує свій рядок у `[Unreleased]` разом із самою зміною. Автоматичні нотатки GitHub не годяться: вони перелічують злиті PR, а в git flow PR немає.

**README.** Перед 0.1.0 README.md і README.uk.md стають повними, за розділами pathmaster: можливості; встановлення через scoop і прямим завантаженням; клавіатура; де лежать дані (тека `data`, persist у scoop); чого revisor свідомо не робить; ліцензія.

**scoop.** Маніфест робимо за зразком `bucket/pathmaster.json`, без `notes`, а його джерело лежить у `packaging/scoop/revisor.json`. Адреса exe має фрагмент `#/revisor.exe`: scoop перейменовує завантажений файл, тож `bin` і ярлик не залежать від версії, а `.sha256` scoop шукає поруч зі справжнім файлом. Excavator лише оновлює наявні маніфести, тож у bucket маніфест засіваємо вручну після 0.1.0. Далі після кожного релізу Excavator запускаємо в bucket вручну, а щодобовий запуск його страхує. Токенів між репозиторіями немає. `persist` робить на `data` junction до `~\scoop\persist\revisor\data`, тож і оновлення, і `scoop uninstall` без `--purge` дані лишають.

**Пробний реліз.** Перед 0.1.0 на гілці release/0.1.0 вручну ставимо тег `v0.1.0-rc.1` (версія 0.1.0-rc.1) і проходимо з ним увесь шлях. Persist перевіряємо справжнім оновленням ще до першого релізу. Ставимо rc.1 з локального маніфесту, створюємо профіль і знімок, міняємо в маніфесті адресу на 0.1.0 і запускаємо `scoop update`.

**Документ «Випуск релізу».** Порядок випуску описує документ Backlog українською поруч із брифом. У ньому кроки за зразком чек-листа pathmaster (F1–F10) разом із пастками, а ще прохід головного сценарію з NVDA обома мовами перед кожним релізом. Непройдений крок зупиняє реліз; заповнену копію не публікуємо.

Джерела — у полі references.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Тег vX.Y.Z запускає релізний воркфлоу на windows-2025. Він бере текст релізу з розділу версії в CHANGELOG.md, проганяє на тегу ті самі перевірки, що й CI в develop (форматування, тести, clippy і актуальність файлу ліцензій), і звіряє версію тегу з Cargo.toml і VERSIONINFO. Без розділу, з порожнім розділом чи з розбіжністю версій реліз не публікується.
- [x] #2 Воркфлоу перевіряє готовий exe: у таблиці імпорту немає VCRUNTIME, MSVCP і api-ms-win-crt, розмір — до 10 МіБ, а VERSIONINFO, прочитана з exe, має версію тегу, CompanyName «Ruslan Iskov» і ProductName «revisor».
- [x] #3 Лише після всіх перевірок воркфлоу публікує в GitHub Releases рівно два файли: revisor-vX.Y.Z-x64.exe і revisor-vX.Y.Z-x64.exe.sha256 у форматі «<hex64> *<ім'я>»; PDB лишається артефактом запуску на 90 днів.
- [x] #4 Тег із дефісом (v0.1.0-rc.1) публікується як pre-release, і checkver scoop його не бачить.
- [x] #5 CHANGELOG.md англійською відповідає Keep a Changelog 1.1.0, розділ 0.1.0 описує можливості MVP, а тест звіряє найновіший розділ із версією крейта.
- [x] #6 README.md і README.uk.md повні й кажуть те саме: можливості, встановлення через scoop і прямим завантаженням (Get-FileHash і попередження SmartScreen для exe без підпису), клавіатура, де лежать дані, чого revisor свідомо не робить, ліцензія.
- [x] #7 packaging/scoop/revisor.json має коментарі «##», адресу exe з #/revisor.exe, bin, ярлик «revisor» у меню «Пуск», persist data, checkver github і autoupdate з hash.url $url.sha256.
- [x] #8 Пробний реліз v0.1.0-rc.1 пройшов увесь шлях: сторінка релізу, Get-FileHash завантажених файлів, встановлення через scoop із локального маніфесту. scoop update з rc.1 на 0.1.0 лишив профілі й знімки на місці.
- [x] #9 Документ Backlog «Випуск релізу» українською описує кроки випуску та їхні пастки за зразком чек-листа pathmaster і прохід головного сценарію з NVDA обома мовами; 0.1.0 випущено за цим документом.
- [x] #10 revisor 0.1.0 опубліковано, маніфест засіяно в bucket, і CI bucket його прийняв. scoop install revisor ставить 0.1.0 з ярликом у меню «Пуск»; перевстановлення без --purge лишає дані; Excavator бачить revisor, а checkver.ps1 revisor -f відтворює той самий маніфест.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 Тести (cargo test) проходять
- [x] #2 cargo clippy і cargo fmt --check без зауважень
- [x] #3 Зміни інтерфейсу перевірено вручну з NVDA в українському інтерфейсі (якщо задача їх містить)
- [x] #4 Нотатки виконання й підсумок задачі заповнено
- [x] #5 Бриф і ADR чи README оновлено (якщо задача змінила поведінку чи рішення)
- [x] #6 Нові вікна й контроли мають клавіші доступу за документом «Клавіатура й меню» (якщо задача їх додає)
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Рішення автора (2026-11-07)

1. **Версія пробного релізу — без суфікса**, як у tapir. Cargo.toml, app.rc і CHANGELOG лишаються 0.1.0; воркфлоу відкидає «v» і суфікс тегу після дефіса (v0.1.0-rc.1 → 0.1.0) і звіряє з ними 0.1.0: числова версія VERSIONINFO має чотири числа, і місця для rc у ній немає. Ім'я файлу й сторінка мають повний тег (revisor-v0.1.0-rc.1-x64.exe), а тег із дефісом стає pre-release. Тож «(версія 0.1.0-rc.1)» в описі означає лише тег.
2. **CHANGELOG:** розділ «## [0.1.0] - дата» з можливостями MVP пишемо одразу в цій гілці, над ним — порожній [Unreleased]; справжню дату випуску ставимо на гілці release/0.1.0. Тести — як у pathmaster, без винятків.
3. **persist:** під час rc ставимо rc.1 з локального маніфесту й робимо профіль і знімок; коли 0.1.0 опубліковано, міняємо в локальному маніфесті адресу й хеш на 0.1.0 і запускаємо scoop update. Bucket засіваємо лише після цього.
4. **NVDA перед релізом:** новий скрипт tools/nvda/release_pass.py (мова uk чи en) проходить головний сценарій на exe, який випускаємо, а документ «Випуск релізу» має таблицю тих самих кроків, щоб пройти їх і вручну. Скрипт запускаю лише зі згоди автора.
5. **На тегу — усі чотири перевірки CI develop:** fmt, test, clippy і файл ліцензій (критерій #1 доповнено).
6. **Англійські README й CHANGELOG — американським правописом**, як інтерфейс (License).

## План

**A. Гілка feature/task-19-releases (від develop 2cef881)**
1. `.github/workflows/release.yml` за зразком pathmaster і tapir: `on: push: tags: ["v*"]`, `windows-2025`, `permissions: contents: write`, дії повним SHA (checkout із `persist-credentials: false`, upload-artifact), env `LIBCLANG_PATH` і `CARGO_TARGET_DIR=C:\t`, без кешу. Кроки від дешевих: версія тегу (без «v» і суфікса) = `[workspace.package] version` = FileVersion/ProductVersion з app.rc; PRERELEASE, якщо тег має дефіс; текст релізу з розділу CHANGELOG (немає чи порожній → червоний запуск); середовище (LIBCLANG_PATH, llvm-rc); Ninja 1.13.2 і Rust із rust-toolchain.toml, як у ci.yml; fmt; test і clippy з `--release --target x86_64-pc-windows-msvc` (wxWidgets збирається один раз); cargo-about 0.9.3 (SHA-256) і файл ліцензій; build `--release --locked --target`; staging `dist\revisor-<тег>-x64.exe`; dumpbin /DEPENDENTS (KERNEL32 є, VCRUNTIME, MSVCP, api-ms-win-crt немає); розмір ≤ 10 МіБ; VersionInfo з exe (CompanyName, ProductName, FileVersion, ProductVersion); `.sha256` «<hex64> *<ім'я>»; PDB — артефакт запуску на 90 днів; `gh release create --verify-tag --notes-file` (+ `--prerelease` для тегу з дефісом).
2. CHANGELOG.md: вступ, порожній [Unreleased], [0.1.0] з Added (можливості MVP), посилання внизу ([Unreleased] → compare/v0.1.0...HEAD, [0.1.0] → releases/tag/v0.1.0).
3. Тест `crates/revisor-core/tests/changelog.rs` за зразком pathmaster: граматика заголовків «## [», [Unreleased] зверху, найновіший розділ = версія крейта, кожен заголовок має посилання.
4. `packaging/scoop/revisor.json` за зразком pathmaster.json: «##», version 0.1.0, url …/v0.1.0/revisor-v0.1.0-x64.exe#/revisor.exe із хешем-заглушкою з нулів (засів підставляє справжній), bin, ярлик «revisor», persist data, checkver github, autoupdate з hash.url $url.sha256.
5. README.md і README.uk.md повні (розділи pathmaster): можливості; встановлення (scoop; пряме завантаження з Get-FileHash і SmartScreen); клавіатура (гарячі клавіші з doc-11); де лежать дані (data\settings.db, data\profiles, persist у scoop, uninstall без --purge і з ним); чого revisor свідомо не робить; збірка із сирців; ліцензія.
6. `tools/nvda/release_pass.py` (uk|en, exe) на revisor_nvda.py + рядок у tools/nvda/README.md.
7. Документ Backlog «Випуск релізу» (guide, поруч із брифом): передумови, кроки за F1–F10 pathmaster з пастками, таблиця проходу NVDA обома мовами, пробний реліз, засів bucket і Excavator, що робити, коли щось пішло не так. Непройдений крок зупиняє реліз; заповнену копію не публікуємо.
8. Бриф і ADR-0010 — правило версії rc і посилання на документ (за згодою автора).
9. Локально: fmt, test (і `--release --target`), clippy -D warnings; кроки воркфлоу на PowerShell — на локальній release-збірці.
10. За словом автора: `git flow feature finish` і push develop; CI на develop зелений.

**B. Пробний реліз**
11. `git flow release start 0.1.0`; дата в [0.1.0]; коміт.
12. Анотований тег v0.1.0-rc.1 на release/0.1.0; за словом автора push тегу → pre-release.
13. Перевірки: сторінка (два файли, текст розділу [0.1.0], pre-release), checkver його не бачить, завантаження (за словом) і Get-FileHash, SmartScreen на чистій машині (руками автора), scoop install з локального маніфесту, ярлик, профіль і знімок, прохід NVDA uk і en (зі згоди).

**C. Реліз 0.1.0**
14. За словом автора `git flow release finish 0.1.0` (тег v0.1.0 у main; ім'я тегу перевірити до push), за словом — push main, develop і тегу → реліз.
15. scoop update з rc.1 на 0.1.0 з локального маніфесту → профілі й знімки на місці.
16. Засів bucket (де клон — питання автору; push за словом) → CI bucket зелений; scoop install revisor → 0.1.0 з ярликом; перевстановлення без --purge лишає дані; Excavator (за словом) бачить revisor; checkver.ps1 revisor -f відтворює маніфест.
17. Нотатки, підсумок, Done; коміт Backlog — у короткій feature-гілці (finish і push за словом).
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-07, гілка feature/task-19-releases (від develop 2cef881): коміти b8b884c (план), ee626c2 (`.github/workflows/release.yml`), a3a7883 (CHANGELOG.md і `crates/revisor-core/tests/changelog.rs`), 78187cf (`packaging/scoop/revisor.json`), be73c82 (повні README.md і README.uk.md), b8e290c (`tools/nvda/release_pass.py` і рядок у tools/nvda/README.md), d0ece44 (документ «Випуск релізу», doc-18), 3d79dab (бриф і ADR-0010: правило версії rc, посилання на doc-18 — за згодою автора).

Воркфлоу: усі кроки на PowerShell перевірено локально (скрипт-обгортка в скретчпаді сесії читає `run:` з release.yml і запускає їх, як раннер: `$ErrorActionPreference = 'stop'`, файл GITHUB_ENV). На локальній release-збірці тег v0.1.0 і v0.1.0-rc.1 проходять усі ворота (rc: VERSION=0.1.0, PRERELEASE=True, файл revisor-v0.1.0-rc.1-x64.exe); `.sha256` — 88 байтів «<hex64> *<ім'я>» без нового рядка, `sha256sum -c` каже OK; текст релізу без BOM, з тире, без рядків-посилань із низу файлу. Відмовляють як слід: тег v0.1.1 (версія), v0.1 (форма тегу), порожній і відсутній розділ [0.1.0], не-PE файл (таблиця імпорту не прочитана), python.exe (VCRUNTIME140 → «+crt-static was lost»), проксі cargo.exe (14 МБ → розмір), sha256sum.exe з Git (CompanyName '' → VERSIONINFO).

Тести в release-профілі (`cargo test --workspace --locked --release`) проходять за 36 с і wxWidgets не перезбирають: тести й збірка ділять один вивід wxdragon-sys, як і міряв pathmaster. PDB: попри `strip = true` у `[profile.release]`, лінкер MSVC і далі пише revisor.pdb (41 МБ), тож артефакт PDB профілю змін не потребує. Дії: actions/checkout v7.0.1 = 3d3c42e5…, як у ci.yml; actions/upload-artifact v7.0.2 = cf430e030ddbb5b0abf93d22962f4752f3646cd9 (звірено з тегом через gh api; v7.0.2 від 2026-10-07). Регулярний вираз checkver для github-API — `(?:v|V)?([\d.-]+)` (локальний scoop 0.6.0): непозначений pre-release `v0.1.0-rc.1` він прочитав би як «0.1.0-», як і каже опис.

Тест changelog.rs (6 тестів): граматика заголовків «## [» і самоперевірка на зразку, заголовок без «]» панікує, [Unreleased] зверху й під ним версія крейта, розділ версії має текст (той самий витяг, що й воркфлоу: без рядків-посилань унизу), у кожного випущеного заголовка є дата YYYY-MM-DD, посилання внизу — як у Keep a Changelog (compare між версіями, найстаріша — releases/tag). Перевірено, що падає: версія розділу 0.1.1 → 3 тести червоні; порожній розділ 0.1.0 → 1. Локально: fmt, clippy -D warnings чисто; тести 147 (+1 ignored) застосунку, 178 (+1 ignored) ядра, 6 changelog, 3 versioninfo.

Пробний прогін `tools/nvda/release_pass.py` за згодою автора на локальній release-збірці (SHA-256 789ce4bf2da1fa1c94d690fc10bb02c459e35743b47f6f9c0d890879a1713f51), NVDA 2026.2: `uk` і `en` — `all heard`, код 0. Профіль через вікно й вибір теки (шлях і Enter: тека закривається першим Enter), підсумок «5 змін: додано 3, видалено 1, змінено 1. Розмір збільшився на 37 байтів.» / «5 changes: 3 added, 1 deleted, 1 modified. …», подробиці, «Показано 4 із 5.», «Показано 1 із 5.», F6, F1. Клавіатура вільна, тимчасову теку прибрано.

Відкладене дрібне: у doc-18 пастка 2 каже «перший термінове виправлення» — виправити на «перше термінове виправлення» з наступним оновленням документа.

2026-11-07, пробний реліз (за словом автора). `git flow feature finish` → develop 9ca2379 (merge --no-ff із трейлером; дерево = 09fe337), push origin/develop 2cef881..9ca2379; CI 42316183849 зелений. `git flow release start 0.1.0` → release/0.1.0 на 9ca2379; дата [0.1.0] у CHANGELOG уже 2026-11-07, тож коміту Р2 не було. Анотований тег v0.1.0-rc.1 на 9ca2379, push тегу після зеленого CI.

Запуск «Release» 42316561864 для v0.1.0-rc.1: зелений, 682 с (холодна збірка без кешу). Усі 18 кроків: «Releasing v0.1.0-rc.1 as version 0.1.0 (pre-release: True)», fmt, тести на тегу (147 + 178 + 6 + 3, `--release --target`), clippy, файл ліцензій, збірка; імпорт без VCRUNTIME, MSVCP і api-ms-win-crt (лише api-ms-win-core-synch); 6 090 240 байтів (5,81 МіБ); VERSIONINFO з exe — Ruslan Iskov, revisor, 0.1.0 / 0.1.0; `.sha256` «dc3c738572d22c3140c52abdc80259e746911172f22b7dd19b84c5691d65e089 *revisor-v0.1.0-rc.1-x64.exe» (93 байти). Сторінка: pre-release «revisor v0.1.0-rc.1», не чернетка, рівно два файли, текст — розділ [0.1.0] без рядків-посилань. PDB — артефакт revisor-v0.1.0-rc.1-x64-pdb (12,8 МБ у zip), зберігається до 2027-02-05.

checkver і rc (#4): API «latest release» (`gh api repos/ruslan-rv-ua/revisor/releases/latest`), який читає Excavator (він ставить SCOOP_GH_TOKEN = GITHUB_TOKEN, src/Action/Scheduled.psm1), відповідає 404 — pre-release не бачить. Але локальний checkver без токена читає HTML: поки повного релізу немає, `github.com/…/releases/latest` переадресовує (302) на `/releases`, і регулярний вираз знаходить там тег rc: «revisor: 0.1.0- (scoop version is 0.1.0) autoupdate available». На Excavator це не впливає (API, а маніфесту в bucket до 0.1.0 і так немає); після 0.1.0 `/releases/latest` веде на справжній реліз — перевірити ще раз після публікації 0.1.0. Записано в пастку 5 документа «Випуск релізу».

Завантаження (за словом автора): `gh release download v0.1.0-rc.1` у порожню теку скретчпаду; перевірка з README дослівно — «MATCHES the published fingerprint»; `sha256sum -c` — OK; VERSIONINFO завантаженого exe 0.1.0. SmartScreen на чистій машині: автор вирішив пропустити («пропустимо», 2026-11-07) — у #8 цієї частини не перевірено.

scoop install з локального маніфесту (за словом автора; version 0.1.0-rc.1, url файлу rc із #/revisor.exe, hash із .sha256): «Checking hash … OK», current → 0.1.0-rc.1, «Making C:\scoop\shims\revisor.exe a GUI binary», ярлик «Scoop Apps\revisor.lnk» у меню «Пуск» → C:\scoop\apps\revisor\current\revisor.exe, `data` — junction на C:\scoop\persist\revisor\data; хеш exe = опублікованому, потоку Zone.Identifier немає (лише :$DATA), тож SmartScreen для копії через scoop не спрацьовує. Перед встановленням scoop сам оновив себе й списки bucket-ів (звичайна поведінка `scoop install`). Підготовка persist (скрипт у скретчпаді, за згодою автора): профіль «Перевірка persist» на теці скретчпаду, F5 у поставленому revisor — 1 знімок, 3 записи; revisor сам створив settings.db у persist.

Прохід NVDA (Р5) на поставленому rc.1 (SHA-256 dc3c7385…e089), NVDA 2026.2, за згодою автора: `release_pass.py uk` і `en` — `all heard`, код 0 (ті самі фрази, що й на локальній збірці). Клавіатура вільна.

2026-11-07, реліз 0.1.0 (за словом автора). `git flow release finish 0.1.0 --no-ff --no-fetch --no-push -m "revisor v0.1.0" -M "Merge branch 'release/0.1.0'…" --update-message "Merge branch 'main' into develop…"` (GIT_MERGE_AUTOEDIT=no, трейлери Co-Authored-By) → main 2032c96 з тегом v0.1.0, develop 62e475a; ім'я тегу перевірено до push; push main ef98baa..2032c96, develop 9ca2379..62e475a, тег v0.1.0.

Запуск «Release» 42319427709: зелений, 508 с. Реліз «revisor v0.1.0» не pre-release, «latest» в API; два файли: revisor-v0.1.0-x64.exe (6 090 240 Б) і .sha256 (88 Б), SHA-256 a91a1b98344bd1460163c1cf0b339fc9378c76aca539aa5a9e58674cc1344825; VERSIONINFO Ruslan Iskov / revisor / 0.1.0; PDB-артефакт до 2027-02-05. CI develop 42319427543 зелений.

Після 0.1.0 `/releases/latest` → v0.1.0, checkver без токена каже «revisor: 0.1.0» — rc не бачить і в режимі HTML.

Р8 для 0.1.0: завантажено (за словом автора), «MATCHES the published fingerprint», `sha256sum -c` OK.

persist: локальний маніфест → version/url/hash 0.1.0, `scoop update revisor`: current → 0.1.0, data — той самий junction, профіль «Перевірка persist» з 1 знімком (3 записи) і settings.db на місці.

bucket: bucket/revisor.json (вступ «##» для живої копії + хеш), розділ README після Tapir (за згодою автора); перед push `scoop install` із файлу — хеш OK, GUI-shim, ярлик, persist; коміт 7a45f6c, push; CI bucket 42320890715 — validate success («Valid JSON», «Valid manifest structure», Version 0.1.0), відкат пропущено. `scoop update` + `scoop install revisor` із bucket ruslan-rv-ua → 0.1.0, ярлик «Scoop Apps\revisor.lnk», дані на місці; перевстановлення без --purge (кеш очищено, свіже завантаження з GitHub, хеш OK) — профіль і знімок на місці.

Excavator 42321464443 (workflow_dispatch, за словом автора): зелений, у лозі «revisor: 0.1.0», нового коміту немає. `checkver.ps1 -App revisor -Dir bucket -ForceUpdate` знайшов той самий хеш і переписав маніфест без змін змісту (`git diff` порожній; лише CRLF у робочій копії через core.autocrlf=true), файл повернуто з індексу.

SmartScreen на чистій машині пропущено за рішенням автора 2026-11-07; критерій #8 скорочено за його згодою. README описує попередження за документацією Microsoft, наживо не бачили; українські назви кнопок у README.uk («Докладніше», «Виконати в будь-якому разі») взято з README pathmaster і не звірено з Windows.

Лишилося в автора: revisor 0.1.0 поставлено через scoop; тестовий профіль «Перевірка persist» у C:\scoop\persist\revisor\data\profiles дивиться на теку скретчпаду сесії — його можна видалити. Pre-release v0.1.0-rc.1 лишається на сторінці релізів.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Релізи revisor збираються й публікуються автоматично, і 0.1.0 випущено.

**Що зроблено.** `.github/workflows/release.yml`: тег vX.Y.Z на windows-2025 бере текст релізу з розділу CHANGELOG, проганяє на тегу fmt, тести, clippy і файл ліцензій, звіряє версію з Cargo.toml і VERSIONINFO, перевіряє готовий exe (імпорт без рантайму C, до 10 МіБ, VERSIONINFO з exe) і лише тоді публікує два файли — exe і `.sha256`; PDB лишається артефактом на 90 днів, тег із дефісом стає pre-release. CHANGELOG.md за Keep a Changelog 1.1.0 і тест `crates/revisor-core/tests/changelog.rs`. Маніфест `packaging/scoop/revisor.json` за зразком pathmaster. Повні README.md і README.uk.md. Документ Backlog «Випуск релізу» (doc-18) з кроками, пастками й таблицею проходу NVDA; скрипт `tools/nvda/release_pass.py`. Бриф і ADR-0010 доповнено правилом версії rc і посиланням на doc-18.

**Як випущено.** Пробний реліз v0.1.0-rc.1 (pre-release, checkver його не бачить) пройшов сторінку релізу, Get-FileHash, scoop install із локального маніфесту, профіль і знімок, прохід NVDA uk і en. Потім `git flow release finish 0.1.0` → v0.1.0 опубліковано, `scoop update` з rc.1 на 0.1.0 лишив профіль і знімок. Маніфест засіяно в ruslan-rv-ua/scoop-bucket (7a45f6c), CI bucket зелений, `scoop install revisor` ставить 0.1.0 з ярликом, перевстановлення без --purge лишає дані, Excavator бачить revisor, checkver -f відтворює той самий маніфест.

**Рішення автора.** Шість рішень із плану (версія rc без суфікса, CHANGELOG одразу в гілці, persist через локальний маніфест, NVDA-скрипт перед релізом, усі чотири перевірки CI на тегу, американський правопис англійських текстів) і скорочений критерій #8 — без перевірки SmartScreen на чистій машині.

**Що свідомо лишилося.** Попередження SmartScreen наживо не бачили: README описує його за документацією Microsoft, українські назви кнопок не звірено з Windows. Тестовий профіль «Перевірка persist» у persist scoop можна видалити; pre-release v0.1.0-rc.1 лишається на сторінці релізів.
<!-- SECTION:FINAL_SUMMARY:END -->
