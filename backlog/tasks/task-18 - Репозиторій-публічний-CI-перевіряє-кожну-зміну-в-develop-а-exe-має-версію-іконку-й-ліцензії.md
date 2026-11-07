---
id: TASK-18
title: >-
  Репозиторій публічний, CI перевіряє кожну зміну в develop, а exe має версію,
  іконку й ліцензії
status: Done
assignee:
  - '@claude'
created_date: '2026-11-01 19:44'
updated_date: '2026-11-01 21:08'
labels: []
milestone: m-1
dependencies: []
references:
  - >-
    https://github.com/ruslan-rv-ua/pathmaster/blob/develop/.github/workflows/ci.yml
  - 'https://github.com/ruslan-rv-ua/pathmaster/blob/develop/rust-toolchain.toml'
  - 'https://docs.github.com/en/billing/concepts/product-billing/github-actions'
  - 'https://docs.github.com/en/billing/reference/actions-runner-pricing'
  - 'https://docs.github.com/en/actions/reference/runners/github-hosted-runners'
  - >-
    https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/managing-repository-settings/setting-repository-visibility
  - >-
    https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/working-with-forks/what-happens-to-forks-when-a-repository-is-deleted-or-changes-visibility
  - >-
    https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/proposing-changes-to-your-work-with-pull-requests/about-branches
  - >-
    https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows
  - 'https://docs.github.com/en/actions/reference/security/secure-use'
  - >-
    https://github.com/actions/runner-images/blob/main/images/windows/Windows2025-VS2026-Readme.md
  - 'https://opensource.org/license/mit'
  - 'https://www.apache.org/licenses/LICENSE-2.0.txt'
  - 'https://www.wxwidgets.org/about/licence/'
  - 'https://wiki.wxwidgets.org/License_Gotchas'
  - 'https://sqlite.org/copyright.html'
  - 'https://github.com/EmbarkStudios/cargo-about'
  - 'https://learn.microsoft.com/en-us/windows/win32/menurc/versioninfo-resource'
  - >-
    https://learn.microsoft.com/en-us/cpp/error-messages/tool-errors/cvtres-fatal-error-cvt1100
  - 'https://docs.rs/embed-manifest/latest/embed_manifest/'
documentation:
  - backlog/docs/doc-2 - Бриф-продукту.md
  - >-
    backlog/docs/adr/doc-14 -
    ADR-0010-Випуск-—-GitHub-Actions-за-тегом-публікує-exe-з-.sha256-а-scoop-оновлює-Excavator.md
  - backlog/docs/doc-8 - Результати-прототипу.md
type: chore
ordinal: 6250
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
revisor розробляємо відкрито, а помилки має ловити CI ще до релізу. Ця задача — основа випуску, і її виконують першою в MVP, до TASK-15: вона публікує репозиторій, запускає перевірки кожної зміни й дає exe ідентичність і ліцензії сторонніх компонентів. Решту випуску — релізний воркфлоу, маніфест scoop і сам 0.1.0 — робить TASK-19 «Релізи збираються й публікуються автоматично» наприкінці MVP. Рішення ухвалено 2026-11-01 в обговоренні DRAFT-13 (ADR-0010). Зразок — pathmaster автора, застосунок на тому самому стеку (Rust, wxdragon, `+crt-static`).

**Публікація.** Репозиторій ruslan-rv-ua/revisor стає публічним ще до CI. У публічному репозиторії стандартні раннери безплатні й без ліміту, а Windows-раннер має 4 vCPU і 16 ГБ. У приватному на GitHub Free — $12 на місяць (≈ 1 200 хвилин Windows) на 2 vCPU, а після ліміту запуски блокуються. Реліз із приватного репозиторію scoop однаково не завантажить. Перед публікацією додаємо:
- LICENSE — звичайна MIT, «Copyright (c) 2026 Ruslan Iskov», як у pathmaster і tapir;
- README.md англійською й README.uk.md українською з посиланнями одне на одне. Поки програма не готова, вони короткі: що таке revisor, позначка «у розробці, релізу ще немає», як зібрати із сирців, ліцензія. Повні README пише TASK-19.

Типова гілка на GitHub — develop, назавжди: main порожня до першого релізу, а кнопка «Run workflow» бачить лише воркфлоу з типової гілки. Email у комітах лишаємо як є: адреса вже публічна в історії marka. Аудит історії 2026-11-01 не знайшов ключів, токенів, паролів чи бінарних файлів. Публікацію практично не скасувати: якщо знову зробити репозиторій приватним, публічні форки лишаться, а логи Actions уже були публічними.

**CI.** Воркфлоу запускається на кожен push і PR у develop, як у pathmaster. Перевірки ті самі, що в DoD: форматування (першим, бо воно не потребує збірки й падає за секунди), тести й clippy з `-D warnings`, усе з `--locked`. Коміт, який `git flow release finish` створює в main, цей CI не бачить, тож TASK-19 проганяє ті самі перевірки на тегу. Збірку закріплено, щоб вона не змінювалася сама собою:
- точна версія Rust у `rust-toolchain.toml` разом із clippy, rustfmt і ціллю x86_64-pc-windows-msvc;
- раннер `windows-2025` (з 2026-07-06 це образ із Visual Studio 2026), а не `windows-latest`;
- дії — повним SHA з тегом у коментарі, бо пересунутий тег дав би сторонньому коду доступ до воркфлоу;
- мінімальні `permissions`.

Dependabot не вмикаємо: його PR зливалися б на GitHub повз `git flow feature finish`. Версії оновлюємо руками у feature-гілках.

wxWidgets збирається із сирців під час `cargo build`, і pathmaster натрапив тут на три пастки раннера. wxdragon-sys вимагає генератора Ninja для x64 MSVC; bindgen у wxdragon-sys не знаходить libclang.dll без `LIBCLANG_PATH`; глибокий шлях до теки збірки ламає збірку wxWidgets через MAX_PATH (там узяли `CARGO_TARGET_DIR` = `C:\t`). Час холодної збірки визначають статичні бібліотеки wxWidgets, тож кеш тримає все дерево збірки з ключем від `Cargo.lock` і `rust-toolchain.toml`. Прототип міряв збірку лише локально, тож час запусків CI записуємо в нотатках.

**VERSIONINFO та іконка.** Exe не підписано (ADR-0010), тож ресурс версії — єдине, що exe каже про себе; Провідник показує його у властивостях файлу. Знімки revisor записують версії exe, тож оновлення самого revisor теж видно в них як зміну версії. Версія ресурсу береться з Cargo.toml, CompanyName — «Ruslan Iskov», ProductName — «revisor». Маніфест Windows лишається в embed-manifest (`build.rs`): другий маніфест у `.rc` дав би помилку дубліката ресурсу. Іконка проста, на 16–256 px, її вигляд погоджує автор. Головне вікно TASK-15 бере її звідси.

**Ліцензії сторонніх компонентів.** Exe вкомпільовує понад сотню крейтів (MIT, Apache-2.0 тощо), wxWidgets і SQLite. MIT вимагає включати текст ліцензії в усі копії, а Apache-2.0 — давати копію ліцензії й NOTICE і в бінарній формі. Реліз — окремий exe без архіву, тож ці тексти мають бути в самому exe. cargo-about генерує файл ліцензій усіх крейтів; файл лежить у репозиторії, а CI перевіряє, що він актуальний. Частин на C і C++ cargo-about не бачить, тож wxWidgets і бібліотеки, які він вкомпільовує, дописуємо вручну. Перед цим перевіряємо, які з них потрапляють у збірку з `default-features = false`. Виняток wxWindows Licence дозволяє поширювати бінарні версії wxWidgets на власних умовах. SQLite — суспільне надбання, але rusqlite має ліцензію MIT. Вікно «Про revisor» (TASK-15) показує цей файл полем лише для читання. cargo-deny не беремо.

Джерела — у полі references.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Перед публікацією в репозиторії є LICENSE (MIT, «Copyright (c) 2026 Ruslan Iskov») і README.md англійською й README.uk.md українською з посиланнями одне на одне: що таке revisor, позначка «у розробці, релізу ще немає», як зібрати із сирців, ліцензія.
- [x] #2 Репозиторій ruslan-rv-ua/revisor публічний, а його типова гілка на GitHub — develop.
- [x] #3 На кожен push і PR у develop GitHub Actions на раннері windows-2025 перевіряють форматування (cargo fmt --check, ще до збірки), а тоді запускають cargo test --workspace --locked і cargo clippy --workspace --all-targets --locked -- -D warnings; невдала перевірка робить запуск червоним.
- [x] #4 Збірку закріплено: точна версія Rust у rust-toolchain.toml (clippy, rustfmt, ціль x86_64-pc-windows-msvc), дії — повним SHA з тегом у коментарі, мінімальні permissions; Dependabot не ввімкнено.
- [x] #5 wxWidgets збирається на раннері, а кеш дерева збірки з ключем від Cargo.lock і rust-toolchain.toml скорочує повторний запуск; час холодного й повторного запуску записано в нотатках задачі.
- [x] #6 Exe має VERSIONINFO з версією з Cargo.toml, CompanyName «Ruslan Iskov» і ProductName «revisor», і Провідник показує їх у властивостях файлу; тест перевіряє, що версія ресурсу збігається з версією крейта. Маніфест Windows в exe лишається один.
- [x] #7 Exe має іконку на 16–256 px, вигляд якої погодив автор; її показують Провідник і головне вікно.
- [x] #8 Файл ліцензій сторонніх компонентів генерує cargo-about, і він лежить у репозиторії; CI падає, якщо файл не відповідає Cargo.lock. wxWidgets і вкомпільовані в нього бібліотеки дописано вручну.
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
Порядок продиктовано вимогою «публічний ще до CI»: воркфлоу не має потрапити в origin, поки репозиторій приватний.

1. feature/license-readme: LICENSE (MIT, Copyright (c) 2026 Ruslan Iskov), README.md (англ.) і README.uk.md (укр.) з посиланнями одне на одне: що таке revisor, «у розробці, релізу ще немає», збірка із сирців (VS Build Tools, CMake, Ninja, LLVM + LIBCLANG_PATH), ліцензія. Finish → push develop (CI ще немає).
2. Після підтвердження автора: `gh repo edit --visibility public`, типова гілка — develop.
3. feature/ci-identity-licenses:
   - rust-toolchain.toml: channel 1.99.1 (поточний локальний, на ньому міряв прототип), clippy, rustfmt, ціль x86_64-pc-windows-msvc.
   - .github/workflows/ci.yml за зразком pathmaster: push/PR у develop, windows-2025, `permissions: contents: read`, дії повним SHA з тегом у коментарі, Ninja, LIBCLANG_PATH, CARGO_TARGET_DIR=C:\t, fmt → кеш C:\t (ключ — Cargo.lock + rust-toolchain.toml) → test → clippy → перевірка файлу ліцензій.
   - crates/revisor/resources/app.rc (ICON + VERSIONINFO, без маніфесту), icon.svg + app.ico (16/24/32/48/256) і tools/make-icon.ps1; build.rs компілює .rc через llvm-rc (поруч із libclang) і передає .res лінкеру; маніфест лишається в embed-manifest.
   - Головне вікно ставить іконку з того самого SVG (wxMSW не бере іконку exe для фрейму).
   - Тест у crates/revisor-core/tests/versioninfo.rs: FILEVERSION/PRODUCTVERSION і рядки = версія крейта, CompanyName, ProductName (без лінкування wxWidgets).
   - cargo-about: about.toml + шаблон, згенерований файл ліцензій у репозиторії; ручний розділ для wxWidgets і вкомпільованих бібліотек (у збірці з default-features = false: zlib, libpng, expat, nanosvg; jpeg/tiff/regex/webp/lunasvg вимкнено).
4. Finish → push → заміряти холодний і повторний запуск CI (gh run rerun), записати в нотатки.
5. Оновити бриф/ADR, якщо щось змінилося; DoD.

Питання до автора: вигляд іконки; як CI отримує cargo-about; назва й місце файлу ліцензій.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Рішення автора 2026-11-01: іконка — варіант B «два знімки» (темно-бірюзовий заокруглений квадрат #0F4C45, задній квадрат контуром #5DCAA5, передній суцільний #E1F5EE); CI отримує cargo-about 0.9.3 готовим архівом релізу з перевіркою SHA-256, записаного у воркфлоу; файл ліцензій — один crates/revisor/THIRD-PARTY-LICENSES.txt, ручний розділ C/C++ — у шаблоні cargo-about поруч.

У збірці з default-features = false wxWidgets 3.3.4 вкомпільовує (CMakeCache wxdragon_sys_cmake_build): zlib, libpng, expat і nanosvg — builtin; libjpeg, libtiff, regex, libwebp, lunasvg, xpm вимкнено. Лінкуються wxpng, wxzlib, wxexpat (вивід build-скрипта wxdragon-sys).

2026-11-01: LICENSE (MIT, Copyright (c) 2026 Ruslan Iskov), README.md і README.uk.md влито в develop (feature/license-readme) і відправлено в origin, поки репозиторій був приватним і без воркфлоу. Після згоди автора `gh repo edit --visibility public` і `--default-branch develop`: `gh repo view` показує PUBLIC і develop; форків на момент публікації — 0.

2026-11-01: feature/ci-identity-licenses влито в develop (коміти 138b752, 2b018fe, злиття 03764ee) і відправлено в origin.

CI (`.github/workflows/ci.yml`), запуск 41348800352: холодний (спроба 1, без кешу) — 7 хв 58 с на job, з них крок Test (збірка wxWidgets + тести) — 6 хв 34 с, Clippy — 24 с, перевірка ліцензій — 11 с; повторний (спроба 2, `gh run rerun`, Cache hit) — 1 хв 56 с, з них відновлення кешу — 48 с, Test — 24 с. Обидва зелені. Червоний запуск від невдалої перевірки на практиці не демонстрували: кожен крок завершується з кодом команди, а багаторядкові кроки перевіряють `$LASTEXITCODE` явно.

Закріплення: Rust 1.99.1 у `rust-toolchain.toml` (clippy, rustfmt, x86_64-pc-windows-msvc); actions/checkout v7.0.1 = 3d3c42e5aac5ba805825da76410c181273ba90b1, actions/cache v6.1.0 = 55cc8345863c7cc4c66a329aec7e433d2d1c52a9 (звірено з тегами); `permissions: contents: read`; `persist-credentials: false`; Dependabot немає. Ninja ставиться `choco install ninja` без версії, як у pathmaster, — єдине незакріплене.

Кеш: увесь `C:\t` (CARGO_TARGET_DIR), ключ — `hashFiles('Cargo.lock', 'rust-toolchain.toml')`; зміни rustflags чи компіляторів образу перебудовують відбитки cargo.

VERSIONINFO і іконка: `crates/revisor/resources/app.rc` (ICON + VERSIONINFO 0.1.0, CompanyName «Ruslan Iskov», ProductName «revisor», без маніфесту) компілює `llvm-rc` з теки `LIBCLANG_PATH` у `build.rs`, `.res` іде лінкеру; маніфест лишився в embed-manifest. llvm-readobj показує один MANIFEST, VERSIONINFO, GROUP_ICON. `(Get-Item revisor.exe).VersionInfo` показує ці поля. Тест `crates/revisor-core/tests/versioninfo.rs` звіряє FILEVERSION/PRODUCTVERSION і рядки з версією крейта (перевірено, що падає при розбіжності) і що обидва крейти успадковують версію workspace. Іконка — варіант B, `icon.svg` → `app.ico` (16/24/32/48/256) скриптом `tools/make-icon.ps1`; Провідник показує її (ExtractAssociatedIcon), вікно — теж (WM_GETICON повертає іконку, головне вікно ставить її з того самого SVG через `BitmapBundle::from_svg_data`).

Ліцензії: `crates/revisor/THIRD-PARTY-LICENSES.txt` генерує cargo-about 0.9.3 з `about.toml` і шаблону `about.hbs`; 23 крейти; ручний розділ — wxWidgets 3.3.4, zlib 1.3.2, libpng 1.6.59, Expat 2.8.5, NanoSVG і примітка про SQLite. CI: `cargo fetch --locked`, `cargo about generate --frozen --fail`, `git diff --exit-code`. `--frozen` тримає вивід відтворюваним, а `cargo fetch` потрібен, бо `cargo metadata` вимагає крейти всіх цілей. Крейти revisor і revisor-core отримали `publish = false`, щоб `private.ignore` не вносив їх у файл. Вбудовування файлу в exe робить TASK-15 (#8, вікно «Про revisor»).

Рев'ю (стандарти й специфікація) жорстких порушень не знайшло; виправлено: перевірка коду `tar`, `persist-credentials: false`, точніші коментарі кешу й about.hbs, повідомлення про `LIBCLANG_PATH` у build.rs.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Репозиторій ruslan-rv-ua/revisor публічний (типова гілка — develop), має LICENSE (MIT) і короткі README.md / README.uk.md.

Що зроблено:
- CI `.github/workflows/ci.yml` на кожен push і PR у develop (windows-2025): cargo fmt --check до збірки, cargo test і cargo clippy -D warnings з --locked, перевірка актуальності файлу ліцензій. Холодний запуск — 7 хв 58 с (збірка wxWidgets + тести — 6 хв 34 с), повторний із кешем `C:\t` — 1 хв 56 с; обидва зелені.
- Закріплення: Rust 1.99.1 у rust-toolchain.toml, actions/checkout і actions/cache повним SHA з тегом у коментарі, `permissions: contents: read`, `persist-credentials: false`, без Dependabot.
- VERSIONINFO (версія з Cargo.toml, CompanyName «Ruslan Iskov», ProductName «revisor») і іконка з `crates/revisor/resources/app.rc`, яку build.rs компілює llvm-rc; маніфест лишився один (embed-manifest). Тест versioninfo.rs звіряє версію ресурсу з версією крейта.
- Іконка «два знімки» (16–256 px) з icon.svg через tools/make-icon.ps1; її показують Провідник і головне вікно.
- `crates/revisor/THIRD-PARTY-LICENSES.txt` генерує cargo-about 0.9.3 (23 крейти) плюс ручний розділ wxWidgets, zlib, libpng, Expat, NanoSVG і примітка про SQLite; CI падає, якщо файл застарів.

Свідомо лишилося:
- Ninja ставиться `choco install ninja` без версії (як у pathmaster) — єдине незакріплене.
- Червоний запуск від невдалої перевірки на практиці не демонстрували; кроки завершуються з кодом команди, багаторядкові перевіряють `$LASTEXITCODE`.
- Вбудовування файлу ліцензій в exe і вікно «Про revisor» — TASK-15.
<!-- SECTION:FINAL_SUMMARY:END -->
