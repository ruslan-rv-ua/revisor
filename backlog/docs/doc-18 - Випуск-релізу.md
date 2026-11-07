---
id: doc-18
title: Випуск релізу
type: guide
created_date: '2026-11-06 23:28'
updated_date: '2026-11-07 01:35'
---
# Випуск релізу

> Як версія revisor потрапляє до людей: кроки випуску по порядку, що кожен має дати, і пастки. Механізм — `.github/workflows/release.yml`, маніфест scoop — `packaging/scoop/revisor.json`, рішення й причини — ADR-0010 і задача TASK-19. Зразок — чек-лист випуску pathmaster (кроки F1–F10) і `docs/release.md` tapir.

**Як ним користуватися.** Перед кожним релізом документ проходять, заповнюючи **копію**: біля кожного кроку — «пройдено», «не пройдено» чи «пропущено» з причиною, а вгорі — шапка нижче. **Непройдений крок зупиняє реліз.** Заповнену копію не публікуємо ні на сторінці релізу, ні в репозиторії: вона лишається в записах автора. Агент виконує кроки за словом автора, і кожен крок назовні — push, публікацію, `git flow release finish`, зміну bucket, запуск Excavator — автор дозволяє окремо.

```
Реліз:           v_._._
Дата:            ____-__-__
Exe (SHA-256):   ________________________________________________________________
NVDA:            ______
Windows:         ______ (збірка)
Пробний реліз:   v_._._-rc._ / не робили (причина)
```

## Передумови

Перевірити перед першим кроком:

- Репозиторій ruslan-rv-ua/revisor публічний. З приватного реліз не працює: scoop і Excavator завантажують файли без входу.
- `git config --get gitflow.branch.release.tagprefix` і `git config --get gitflow.branch.hotfix.tagprefix` кажуть `v`, а `gitflow.feature.finish.no-ff` і `gitflow.bugfix.finish.no-ff` — `true`. Ці ключі лежать лише в `.git/config` і не комітяться, тож у свіжому клоні чи на іншому комп'ютері їх ставлять знову (пастка 1).
- `gh auth status` показує вхід у GitHub.
- Локальний develop збігається з origin, а CI на його вершині зелений: `gh run list --branch develop --limit 1`.
- Для проходу з NVDA: NVDA з `loggingLevel = IO` і логом `C:\Temp\nvda.log` і Python 3.12+ (`tools/nvda/README.md`).

## Кроки

Кроки йдуть по порядку: жоден не можна зробити раніше за попередній. Р4, Р9 і засів bucket у Р10 стосуються упаковки. Як E3, F6 і F9 у pathmaster, вони обов'язкові для 0.1.0, а далі — лише коли змінилися релізний воркфлоу, маніфест scoop чи збірка exe (`[profile.release]`, `.cargo/config.toml`, `build.rs`, `app.rc`). Решту кроків проходять щоразу.

- [ ] **Р1. Гілка релізу.** `git switch develop`, `git pull`, тоді `git flow release start X.Y.Z`.
  *Очікується:* гілка `release/X.Y.Z` від вершини develop, яку бачив CI.
- [ ] **Р2. Версія й CHANGELOG.** На гілці релізу змінити три файли:
  - `version` у `[workspace.package]` у `Cargo.toml`;
  - `FILEVERSION`, `PRODUCTVERSION` і рядки `FileVersion`, `ProductVersion` у `crates/revisor/resources/app.rc`;
  - `CHANGELOG.md`: розділ `[Unreleased]` стає `## [X.Y.Z] - YYYY-MM-DD` із датою випуску, а над ним відкривається новий порожній `## [Unreleased]`. Внизу `[Unreleased]` тепер порівнює з `vX.Y.Z`, а новий рядок `[X.Y.Z]` — `vX.Y.Z` з попереднім тегом.

  Тоді виконати `cargo update --workspace`, `cargo fmt --all -- --check`, `cargo test --workspace --locked` і `cargo clippy --workspace --all-targets --locked -- -D warnings` і закомітити на гілці релізу. Для 0.1.0 версія вже стоїть, тож крок лише ставить дату розділу [0.1.0].
  *Очікується:* тести `the_versioninfo_carries_the_crate_version`, `the_newest_released_section_carries_the_crate_version`, `the_newest_released_section_has_text_to_publish` і `every_version_heading_carries_its_link_reference` зелені. Розділ версії людською мовою каже, що змінилося: саме його опублікує сторінка релізу. `Cargo.lock` руками не правлять: `cargo update --workspace` міняє в ньому лише версії крейтів revisor, а всі перевірки йдуть із `--locked` і на застарілому файлі падають.
- [ ] **Р3. README.** Якщо реліз змінив те, що описують README.md і README.uk.md (можливості, встановлення, клавіші, дані, чого revisor не робить), оновити обидва.
  *Очікується:* обидва README кажуть те саме, або реліз їх не стосується.
- [ ] **Р4. Пробний реліз (rc).** Для 0.1.0 і коли змінилася упаковка — за розділом «Пробний реліз».
  *Очікується:* пробний реліз пройшов увесь шлях.
- [ ] **Р5. Прохід головного сценарію з NVDA обома мовами** — щоразу, за розділом «Прохід з NVDA».
  *Очікується:* скрипт закінчився рядком `all heard` для `uk` і для `en`, або ручний прохід дав те, що каже таблиця. SHA-256 exe — у шапці копії.
- [ ] **Р6. Завершення гілки релізу.** `git flow release finish X.Y.Z --no-ff --no-push -m "revisor vX.Y.Z"` (з `GIT_MERGE_AUTOEDIT=no`; `-M` і `--update-message` задають повідомлення двох злиттів, `--no-ff` дає окремий коміт злиття в main). Тоді **до push** прочитати ім'я тегу: `git tag --points-at main`. Лише коли це `vX.Y.Z`: `git push origin main develop vX.Y.Z`.
  *Очікується:* main має коміт злиття з тегом `vX.Y.Z`, develop — злиття гілки релізу, гілку релізу видалено. Тег без «v» нічого не запустить (пастка 1), а тег не на main лишив би hotfix без бази (пастка 2).
- [ ] **Р7. Релізний воркфлоу.** `gh run watch` на запуску «Release» цього тегу, тоді `gh release view vX.Y.Z`.
  *Очікується:* зелені всі кроки: тег і версії, розділ CHANGELOG, fmt, тести, clippy, файл ліцензій, таблиця імпорту без VCRUNTIME, MSVCP і api-ms-win-crt, розмір до 10 МіБ і VERSIONINFO з exe. На сторінці рівно два файли, `revisor-vX.Y.Z-x64.exe` і `revisor-vX.Y.Z-x64.exe.sha256`, текст сторінки — розділ версії з CHANGELOG.md, а реліз — «Latest», не pre-release. PDB лежить в артефактах запуску, а не на сторінці (пастка 8).
- [ ] **Р8. Хеш завантажених файлів** — щоразу. Завантажити обидва файли наново й виконати перевірку з README (розділ «Direct download»).
  *Очікується:* `MATCHES the published fingerprint`. Цей крок доводить, що інструкція для людей працює, а не лише що хеш записано.
- [ ] **Р9. Чиста машина.** На комп'ютері без Visual C++ Redistributable запустити завантажений exe.
  *Очікується:* SmartScreen показує попередження, яке описує README, «Докладніше» → «Виконати в будь-якому разі» його минає, і revisor запускається. Записати збірку Windows.
- [ ] **Р10. scoop.** Перший реліз — засів bucket за розділом «Засів bucket». Кожен наступний — у bucket: Actions → Excavator → Run workflow (або `gh workflow run excavator.yml -R ruslan-rv-ua/scoop-bucket`).
  *Очікується:* запуск Excavator зелений і комітить «revisor: Update to version X.Y.Z». CI bucket на цьому коміті не запускається (пастка 6). Червоний запуск — сигнал: помилка checkver означає, що змінилася сторінка релізу чи форма тегу, а помилка хешу — що `.sha256` не відповідає файлу. Тоді назад до Р8, а маніфест у bucket руками не правити. Пропущений крок затримує реліз не більше ніж на добу: щодобовий запуск о 04:20 UTC підхопить його сам.
- [ ] **Р11. Оновлення в автора.** `scoop update`, тоді `scoop update revisor` і запуск.
  *Очікується:* revisor X.Y.Z, профілі й знімки на місці (`~\scoop\persist\revisor\data`).

## Прохід з NVDA

```powershell
python -X utf8 tools\nvda\release_pass.py uk <exe>
python -X utf8 tools\nvda\release_pass.py en <exe>
```

`<exe>` — файл, який випускаємо: з пробного релізу (завантажений чи поставлений scoop, `~\scoop\apps\revisor\current\revisor.exe`). Без пробного релізу — `target\release\revisor.exe`, зібраний `cargo build --release --locked` з вершини гілки релізу. Кожен прогін триває близько двох хвилин, і клавіатуру й мишу тим часом не чіпають. Скрипт запускає копію exe з власною текою `data` і мовою, друкує SHA-256 exe, після кожного кроку — сказане NVDA і `OK` чи `MISSING` біля слів, які крок мусить мати, а наприкінці — `all heard` або перелік того, чого NVDA не сказав.

Вручну ті самі кроки проходять у копії exe з порожньою текою `data`, а мову міняють у налаштуваннях (діє після перезапуску).

| # | Дія | Що має сказати NVDA |
|---|---|---|
| Н1 | Запустити revisor без профілів | «Профілів ще немає» (No profiles yet) |
| Н2 | Ctrl+N | вікно «Новий профіль» (New profile) і поле «Назва» |
| Н3 | Набрати назву; Alt+A, у вікні вибору теки — шлях і Enter; Alt+N, Enter | «Профіль «<назва>» створено.» (Profile “<name>” created.) |
| Н4 | F5 | «Знімок розпочато.», «Знімок збережено: N записів.» (Snapshot saved: N entries.) |
| Н5 | У теці профілю змінити один файл, видалити другий, додати третій і теку з файлом | — |
| Н6 | Ctrl+F5 | «Знімок збережено: …», фокус на списку різниці: підсумок зі словами «додано», «видалено», «змінено» (added, deleted, modified) і перший рядок |
| Н7 | Стрілка вниз | наступна зміна: ім'я, зміна, що змінилося |
| Н8 | Alt+Enter, тоді Esc | вікно «Подробиці: <ім'я>» (Details: <name>) з рядком «Зміна: …»; Esc повертає фокус на ту саму зміну |
| Н9 | Ctrl+2 двічі | «Показано N із M.» (Showing N of M.) |
| Н10 | Ctrl+F, набрати «new», Esc | після секунди тиші «Показано N із M.»; Esc очищає поле |
| Н11 | F6, тоді Shift+F6 | перелік знімків, тоді список різниці |
| Н12 | F1, тоді Esc | вікно «Гарячі клавіші» з рядком «Знімок + порівняння — Ctrl+F5» (Snapshot and compare — Ctrl+F5) |

## Пробний реліз

Для 0.1.0 і коли змінилася упаковка (розділ «Кроки»). Тег із дефісом проходить той самий воркфлоу, але реліз стає pre-release, тож ні API «latest release», ні Excavator його не бачать (пастка 5).

1. На гілці `release/X.Y.Z` після Р2–Р3: `git tag -a vX.Y.Z-rc.N -m "revisor vX.Y.Z-rc.N"` (N від 1). Ім'я тегу перевірити (`git tag --points-at HEAD`) і запушити лише тег: `git push origin vX.Y.Z-rc.N`. Гілці релізу на origin бути не треба: тег несе свій коміт.
2. Воркфлоу відкидає суфікс: звіряє X.Y.Z з Cargo.toml і app.rc і бере текст із розділу [X.Y.Z]. Exe каже X.Y.Z, бо числова версія VERSIONINFO має чотири числа й місця для rc у ній немає. На сторінці pre-release — `revisor-vX.Y.Z-rc.N-x64.exe` і його `.sha256`.
3. Перевірити сторінку (pre-release, рівно два файли, текст розділу [X.Y.Z]) і що API, який читає checkver в Excavator, цього релізу не бачить: `gh api repos/ruslan-rv-ua/revisor/releases/latest` називає попередній реліз, а до першого відповідає 404. Локальний checkver без токена до першого релізу каже «X.Y.Z-» (пастка 5), а після нього — попередню версію.
4. Завантажити обидва файли й пройти перевірку хешу з README (як Р8).
5. Якщо Р9 цього релізу обов'язковий — пройти його на цьому файлі.
6. Поставити rc із локального маніфесту: скопіювати `packaging/scoop/revisor.json` у тимчасову теку як `revisor.json`, поставити `version` `X.Y.Z-rc.N`, `url` на файл rc із `#/revisor.exe` і `hash` — перші 64 символи його `.sha256`; тоді `scoop install <тека>\revisor.json`. *Очікується:* ярлик «revisor» у меню «Пуск», запуск без вікна консолі, тека `~\scoop\persist\revisor\data`. У ньому створити профіль і зробити знімок; Р5 можна пройти на цьому exe.
7. Коли X.Y.Z опубліковано (Р7): у тому самому локальному маніфесті поставити `version`, `url` і `hash` X.Y.Z і виконати `scoop update revisor`. *Очікується:* revisor X.Y.Z, профіль і знімки на місці. Тоді `scoop uninstall revisor` без `--purge`, щоб Р10 поставив revisor уже з bucket.

Пробний реліз лишається на сторінці релізів із позначкою pre-release.

## Засів bucket

Один раз, після першого справжнього релізу: Excavator лише оновлює маніфести, які вже є в bucket (пастка 6).

1. Клон ruslan-rv-ua/scoop-bucket, гілка main, оновлена.
2. `packaging/scoop/revisor.json` скопіювати як `bucket/revisor.json`. `hash` — перші 64 символи `revisor-vX.Y.Z-x64.exe.sha256`, `version` і `url` — X.Y.Z, а коментарі «##» — словами для живої копії, як у `bucket/pathmaster.json`.
3. `scoop install <bucket>\bucket\revisor.json`: ставиться, має ярлик, запускається; тоді `scoop uninstall revisor` без `--purge`.
4. Розділ revisor у README bucket, як у сусідніх застосунків.
5. Коміт «Add revisor X.Y.Z» і push у main. *Очікується:* CI bucket перевірив маніфест і не відкотив коміт.
6. `scoop update`, тоді `scoop install revisor`. *Очікується:* X.Y.Z з ярликом «revisor» у меню «Пуск», і дані з пробного релізу на місці. Перевстановлення без `--purge` (`scoop uninstall revisor`, `scoop install revisor`) їх лишає.
7. Excavator — Run workflow. *Очікується:* зелений запуск, у лозі є revisor, і він уже актуальний. `& "$(scoop prefix scoop)\bin\checkver.ps1" -App revisor -Dir <bucket>\bucket -ForceUpdate` переписує маніфест без жодної зміни: `git -C <bucket> diff --exit-code`.

## Пастки

1. **Префікс тегу лише в `.git/config`.** git-flow-next бере ім'я тегу з гілки, а префікс — з `gitflow.branch.release.tagprefix` (для hotfix — `gitflow.branch.hotfix.tagprefix`). Без ключа `finish` ставить голий `X.Y.Z`, а `release.yml` чекає `v*`: нічого не запускається, і червоного запуску теж немає — це видно, лише коли чекаєш. Тож ім'я тегу читаємо до push. Голий тег виправляють локально: поставити ключ, `git tag -d X.Y.Z`, `git tag -a vX.Y.Z -m "revisor vX.Y.Z" main`.
2. **Тег стоїть у main.** `finish` ставить його на коміт злиття в main, а від main починається hotfix. Тег на develop лишив би перше термінове виправлення без бази.
3. **Коміту в main CI develop не бачив.** Тому воркфлоу сам проганяє fmt, тести, clippy і перевірку файлу ліцензій, і саме на тегу. Push develop і зелений CI до Р1 лише дають відповідь раніше.
4. **Опублікований тег не переставляють.** Коли воркфлоу впав, релізу немає, а тег уже на origin. Збій раннера чи мережі — «Re-run all jobs» на тому самому тегу. Помилка в коді чи файлах — нова версія через `git flow hotfix start X.Y.Z+1` (як у tapir). Переставити чи видалити опублікований тег вирішує лише автор.
5. **Pre-release і checkver.** checkver `github` читає `releases/latest`, а API «latest release» пропускає pre-release. Тег із дефісом воркфлоу позначає pre-release сам. Непозначений `vX.Y.Z-rc.1` scoop прочитав би як «X.Y.Z-»: регулярний вираз checkver — `[\d.-]+`. Поки повного релізу ще немає, `github.com/…/releases/latest` переадресовує на перелік релізів, і checkver без токена, який читає сторінку, а не API, знаходить там тег rc і каже «X.Y.Z-» (перевірено 2026-11-07 на v0.1.0-rc.1). Excavator іде з токеном через API, а той відповідає 404, тож pre-release він не бачить; маніфесту в bucket до першого релізу однаково немає.
6. **Excavator лише оновлює.** Маніфесту, якого немає в bucket, він не створить, тож перший засіваємо руками. Excavator пушить із `github.token`, тож CI bucket на його коміт не запускається: маніфест перевіряє сам Excavator, і зеленим має бути саме його запуск.
7. **Exe без підпису.** SmartScreen попереджає лише про файл із позначкою інтернету (Mark-of-the-Web), тобто завантажений браузером, а не поставлений scoop. Smart App Control, де його ввімкнено, може заблокувати exe без підпису незалежно від позначки.
8. **PDB живе 90 днів.** Лише з PDB можна розібрати збій exe в користувача, а артефакт запуску GitHub зберігає 90 днів. Щоб мати його довше, завантажте його (`gh run download <запуск> -n revisor-vX.Y.Z-x64-pdb`) у записи релізу.
9. **`--purge` видаляє дані.** `scoop uninstall revisor --purge` прибирає й `~\scoop\persist\revisor`, а без `--purge` дані лишаються.
10. **NVDA і приватність.** Лог NVDA містить те, що автор читав до перевірки: скрипт читає лише те, що дописано після його старту, і перестає читати перед закриттям вікна. Під час ручного проходу так само — не копіювати в записи нічого зі старішої частини логу.

## Коли щось пішло не так

- **Тег запушено, а запуску «Release» немає.** Тег без «v» (пастка 1). Голий тег на origin видаляє лише автор, а правильний `vX.Y.Z` ставлять на той самий коміт у main.
- **Запуск «Release» червоний.** Лог називає перевірку, що не пройшла. Далі — за пасткою 4.
- **Реліз є, а bucket не оновився.** `gh run list -R ruslan-rv-ua/scoop-bucket --workflow excavator.yml`: лог Excavator каже, що він побачив. Найчастіше реліз позначено pre-release або на сторінці немає `.sha256`.
- **Маніфест у bucket треба повернути.** Воркфлоу bucket «Set a manifest by hand (override)»: app, version, hash і url попередньої версії.
- **Hotfix.** `git flow hotfix start X.Y.Z+1` від main, далі Р2–Р11: гілка hotfix замість гілки релізу, а `git flow hotfix finish` зливає її в main і develop і ставить тег `vX.Y.Z+1`.
