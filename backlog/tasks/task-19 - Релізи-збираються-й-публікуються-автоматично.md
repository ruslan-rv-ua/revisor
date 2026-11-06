---
id: TASK-19
title: Релізи збираються й публікуються автоматично
status: To Do
assignee: []
created_date: '2026-10-30 17:04'
updated_date: '2026-11-06 21:14'
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
- [ ] #1 Тег vX.Y.Z запускає релізний воркфлоу на windows-2025. Він бере текст релізу з розділу версії в CHANGELOG.md, проганяє на тегу форматування, тести й clippy і звіряє версію тегу з Cargo.toml і VERSIONINFO. Без розділу, з порожнім розділом чи з розбіжністю версій реліз не публікується.
- [ ] #2 Воркфлоу перевіряє готовий exe: у таблиці імпорту немає VCRUNTIME, MSVCP і api-ms-win-crt, розмір — до 10 МіБ, а VERSIONINFO, прочитана з exe, має версію тегу, CompanyName «Ruslan Iskov» і ProductName «revisor».
- [ ] #3 Лише після всіх перевірок воркфлоу публікує в GitHub Releases рівно два файли: revisor-vX.Y.Z-x64.exe і revisor-vX.Y.Z-x64.exe.sha256 у форматі «<hex64> *<ім'я>»; PDB лишається артефактом запуску на 90 днів.
- [ ] #4 Тег із дефісом (v0.1.0-rc.1) публікується як pre-release, і checkver scoop його не бачить.
- [ ] #5 CHANGELOG.md англійською відповідає Keep a Changelog 1.1.0, розділ 0.1.0 описує можливості MVP, а тест звіряє найновіший розділ із версією крейта.
- [ ] #6 README.md і README.uk.md повні й кажуть те саме: можливості, встановлення через scoop і прямим завантаженням (Get-FileHash і попередження SmartScreen для exe без підпису), клавіатура, де лежать дані, чого revisor свідомо не робить, ліцензія.
- [ ] #7 packaging/scoop/revisor.json має коментарі «##», адресу exe з #/revisor.exe, bin, ярлик «revisor» у меню «Пуск», persist data, checkver github і autoupdate з hash.url $url.sha256.
- [ ] #8 Пробний реліз v0.1.0-rc.1 пройшов увесь шлях: сторінка релізу, Get-FileHash завантажених файлів, попередження SmartScreen на чистій машині, встановлення через scoop із локального маніфесту. scoop update з rc.1 на 0.1.0 лишив профілі й знімки на місці.
- [ ] #9 Документ Backlog «Випуск релізу» українською описує кроки випуску та їхні пастки за зразком чек-листа pathmaster і прохід головного сценарію з NVDA обома мовами; 0.1.0 випущено за цим документом.
- [ ] #10 revisor 0.1.0 опубліковано, маніфест засіяно в bucket, і CI bucket його прийняв. scoop install revisor ставить 0.1.0 з ярликом у меню «Пуск»; перевстановлення без --purge лишає дані; Excavator бачить revisor, а checkver.ps1 revisor -f відтворює той самий маніфест.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 Тести (cargo test) проходять
- [ ] #2 cargo clippy і cargo fmt --check без зауважень
- [ ] #3 Зміни інтерфейсу перевірено вручну з NVDA в українському інтерфейсі (якщо задача їх містить)
- [ ] #4 Нотатки виконання й підсумок задачі заповнено
- [ ] #5 Бриф і ADR чи README оновлено (якщо задача змінила поведінку чи рішення)
- [ ] #6 Нові вікна й контроли мають клавіші доступу за документом «Клавіатура й меню» (якщо задача їх додає)
<!-- DOD:END -->
