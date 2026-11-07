---
id: doc-14
title: >-
  ADR-0010: Випуск — GitHub Actions за тегом публікує exe з .sha256, а scoop
  оновлює Excavator
type: other
created_date: '2026-11-01 19:42'
updated_date: '2026-11-06 23:34'
tags:
  - adr
---
# ADR-0010: Випуск — GitHub Actions за тегом публікує exe з .sha256, а scoop оновлює Excavator

- **Статус:** прийнято
- **Дата:** 2026-11-01

## Контекст

revisor встановлюється через scoop із власного bucket ruslan-rv-ua/scoop-bucket (бриф, розділ 5), а гілки ведемо за git flow: робота йде в develop, а `git flow release finish` зливає реліз у main і ставить тег vX.Y.Z. Вимоги до випуску:
- реліз з'являється в scoop без ручної правки маніфесту;
- випускається саме той код, що пройшов перевірки, а помилки ловляться до релізу;
- revisor — один exe без DLL, до 10 МіБ, із CRT, лінкованим статично (бриф, розділ 5; документ «Результати прототипу»);
- wxWidgets збирається із сирців під час `cargo build` (968 кроків Ninja), тож збірка в CI довга й залежить від інструментів раннера;
- автор користується скрінрідером, а частина користувачів завантажуватиме файл напряму, без scoop;
- репозиторій ruslan-rv-ua/revisor поки приватний, а scoop і Excavator завантажують файли без входу.

Bucket уже оновлює маніфести інших застосунків автора через Excavator, і токенів між репозиторіями там свідомо немає. Найближчий зразок — pathmaster автора: той самий стек (Rust, wxdragon, `+crt-static`), CI, релізний воркфлоу з перевірками артефакту й маніфест `bucket/pathmaster.json`.

## Рішення

**Публікація й CI.** Репозиторій стає публічним на початку MVP, ще до CI, з LICENSE (MIT) і README англійською й українською; типова гілка — develop. GitHub Actions перевіряють кожен push і PR у develop: `cargo fmt --check`, `cargo test --workspace --locked` і `cargo clippy --workspace --all-targets --locked -- -D warnings`. Збірку закріплено: точна версія Rust у `rust-toolchain.toml`, раннер `windows-2025`, дії — повним SHA, `Cargo.lock` з `--locked`, мінімальні `permissions`. Версії оновлюємо руками, без Dependabot.

**Реліз.** Тег vX.Y.Z із `git flow release finish` запускає релізний воркфлоу. Він бере текст релізу з розділу версії в CHANGELOG.md (Keep a Changelog), ще раз проганяє на тегу ті самі перевірки й звіряє версію тегу з Cargo.toml і VERSIONINFO. Потім збирає exe й перевіряє сам артефакт: таблицю імпорту (без DLL рантайму C), розмір до 10 МіБ і VERSIONINFO, прочитану з exe. Лише після цього він публікує в GitHub Releases окремий exe `revisor-vX.Y.Z-x64.exe` і `.sha256` до нього у форматі sha256sum. Тег із дефісом (`v0.1.0-rc.1`) стає pre-release. Суфікс після дефіса має лише тег: Cargo.toml, VERSIONINFO і CHANGELOG.md несуть 0.1.0, і з ними воркфлоу звіряє версію тегу без суфікса, бо числова версія VERSIONINFO має чотири числа й місця для rc у ній немає (вирішено 2026-11-07, TASK-19). Exe не підписано: його ідентичність — VERSIONINFO та іконка, а ліцензії сторонніх компонентів (файл cargo-about) exe показує у вікні «Про revisor». Порядок випуску описує документ «Випуск релізу».

**scoop.** Маніфест `bucket/revisor.json` — за зразком `bucket/pathmaster.json`: адреса exe з `#/revisor.exe`, `persist: data`, `checkver: github` і `autoupdate` з хешем із `$url.sha256`. Перший маніфест засіваємо вручну після 0.1.0, бо Excavator лише оновлює наявні. Далі після кожного релізу Excavator запускаємо в bucket вручну, а щодобовий запуск його страхує. Репозиторій revisor не має токена до bucket.

## Розглянуті альтернативи

- **Zip `revisor-X.Y.Z-windows-x64.zip`, як у marka (так записали в брифі 2026-10-30).** Поруч з exe лягали б LICENSE і README. Не обрано: revisor — один exe, а користувачеві скрінрідера не треба розпаковувати архів заради одного файлу; так уже роблять pathmaster і tapir. Ліцензії лежать у самому exe.
- **CI в приватному репозиторії до першого релізу.** Код довше лишався б закритим. Не обрано: на GitHub Free приватний репозиторій має $12 обчислень на місяць (≈ 1 200 хвилин Windows) і Windows-раннер на 2 vCPU замість 4, а після ліміту запуски блокуються. Реліз однаково вимагає публічного репозиторію.
- **Автоматичні нотатки GitHub.** Не обрано: вони перелічують злиті PR, а в git flow PR немає, тож сторінка релізу лишилася б голим посиланням «Full Changelog», як у pathmaster 0.1.0.
- **Підпис exe.** Azure Artifact Signing приватним особам доступний лише у США й Канаді. SignPath Foundation підписує безплатно, але від свого імені, лише вже випущені проєкти й зі схваленням кожного релізу вручну. Certum Open Source коштує €49–69 і потребує поновлення. Не обрано для 0.1: через scoop файл ставиться без мітки Mark-of-the-Web, а для прямого завантаження README пояснює попередження SmartScreen і перевірку хешу. До підпису повертаємося, коли він знадобиться.
- **Оновлення маніфесту з релізного воркфлоу revisor (repository_dispatch чи PR у bucket).** Не обрано: тоді репозиторій revisor тримав би токен із правом запису в bucket, а такі токени автор свідомо прибрав з усіх застосунків, коли bucket перейшов на Excavator.
- **Dependabot для дій і крейтів.** Не обрано: його PR зливаються на GitHub повз `git flow feature finish`, а кожне оновлення wxdragon однаково треба перевіряти з NVDA. Версії оновлюємо руками у feature-гілках.
- **Без закріплення (`windows-latest`, стабільний Rust, дії за тегами), як у marka.** Простіше. Не обрано: збірка змінювалася б сама собою, а пересунутий тег дії дав би сторонньому коду доступ до воркфлоу, який публікує релізи.

## Наслідки

Позитивні:
- Реліз потрапляє в scoop без ручної правки маніфесту, а між репозиторіями немає жодного токена.
- Випускається саме той коміт, що пройшов перевірки на тегу. Помилку в exe — загублений `+crt-static`, зайвий розмір, неправильну версію — воркфлоу ловить до публікації.
- Стандартні раннери публічного репозиторію безплатні й без ліміту хвилин.
- Пробний pre-release перевіряє весь шлях, а Excavator і checkver його не бачать.

Негативні наслідки й ризики:
- Публікацію практично не скасувати: якщо знову зробити репозиторій приватним, публічні форки лишаться, а історія й логи Actions уже були публічними.
- Exe без підпису: за прямого завантаження SmartScreen попереджає про нього, а Smart App Control, де його ввімкнено, може його заблокувати незалежно від мітки.
- Ключ `gitflow.branch.release.tagprefix = v` лежить лише в `.git/config`. У свіжому клоні `finish` поставить тег без `v`, і релізний воркфлоу мовчки не запуститься, тож ім'я тегу перевіряємо перед push (документ «Випуск релізу»).
- Засів першого маніфесту й запуск Excavator після релізу — ручні кроки. Якщо Excavator не запустити, реліз потрапить у scoop лише під час щодобового запуску.
- Образ `windows-2025` оновлюється, і версії VS, LLVM, CMake й Ninja в ньому змінюються. Від цього захищають перевірки артефакту, а не номери версій.
- Закріплені версії самі не оновлюються: Rust, крейти й дії оновлюємо руками.

## Джерела

- [GitHub: білінг Actions](https://docs.github.com/en/billing/concepts/product-billing/github-actions)
- [GitHub: ціни раннерів](https://docs.github.com/en/billing/reference/actions-runner-pricing)
- [GitHub: стандартні раннери](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)
- [GitHub: видимість репозиторію](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/managing-repository-settings/setting-repository-visibility)
- [GitHub: безпечне використання Actions](https://docs.github.com/en/actions/reference/security/secure-use)
- [GitHub REST: latest release](https://docs.github.com/en/rest/releases/releases#get-the-latest-release)
- [GitHub: GITHUB_TOKEN](https://docs.github.com/en/actions/concepts/security/github_token)
- [runner-images: Windows Server 2025 з VS 2026](https://github.com/actions/runner-images/blob/main/images/windows/Windows2025-VS2026-Readme.md)
- [Scoop: App Manifests](https://github.com/ScoopInstaller/Scoop/wiki/App-Manifests)
- [Scoop: App Manifest Autoupdate](https://github.com/ScoopInstaller/Scoop/wiki/App-Manifest-Autoupdate)
- [Scoop: checkver.ps1](https://github.com/ScoopInstaller/Scoop/blob/master/bin/checkver.ps1)
- [Scoop: autoupdate.ps1](https://github.com/ScoopInstaller/Scoop/blob/master/lib/autoupdate.ps1)
- [Scoop: Persistent data](https://github.com/ScoopInstaller/Scoop/wiki/Persistent-data)
- [ScoopInstaller/GithubActions (Excavator)](https://github.com/ScoopInstaller/GithubActions)
- [Microsoft: репутація SmartScreen](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation)
- [Microsoft: варіанти підпису коду](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options)
- [Microsoft: Smart App Control](https://learn.microsoft.com/en-us/windows/apps/develop/smart-app-control/overview)
- [Azure Artifact Signing: quickstart](https://learn.microsoft.com/en-us/azure/artifact-signing/quickstart)
- [SignPath Foundation: умови](https://signpath.org/terms)
- [Certum: Open Source Code Signing](https://shop.certum.eu/open-source-code-signing-on-simplysign.html)
- [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/)
- [SemVer 2.0.0](https://semver.org/spec/v2.0.0.html)
- [pathmaster: ci.yml](https://github.com/ruslan-rv-ua/pathmaster/blob/develop/.github/workflows/ci.yml)
- [pathmaster: release.yml](https://github.com/ruslan-rv-ua/pathmaster/blob/develop/.github/workflows/release.yml)
- [scoop-bucket: bucket/pathmaster.json](https://github.com/ruslan-rv-ua/scoop-bucket/blob/main/bucket/pathmaster.json)
- [scoop-bucket: excavator.yml](https://github.com/ruslan-rv-ua/scoop-bucket/blob/main/.github/workflows/excavator.yml)
