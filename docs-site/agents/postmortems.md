# Постмортемы багов

::: tip Зачем эта страница
Журнал реальных багов, которые уже починили. Каждая запись = **что сломалось**, **почему**, **как починили**, **как не допустить повторения**. Цель — чтобы будущий ты (или следующий агент) не повторял одни и те же ошибки.

Workflow ведения постмортемов — `bug-postmortem` skill (`.claude/skills/bug-postmortem/SKILL.md`).
:::

## Шаблон записи

```
## YYYY-MM-DD — короткое название

**Симптомы** — что наблюдал пользователь (1-2 предложения).
**Где жило** — `file:line` ссылки.
**Root cause** — почему именно это произошло, на уровне «не угадаешь без stack trace».
**Fix** — что изменили + commit hash (когда появится).
**Регрешн-защита** — какой тест ловит повторение.
**Prevention** — общий вывод: на что обращать внимание в похожих ситуациях.
```

---

## 2026-05-23 — Kepler: file search показывает только файлы из %APPDATA% (НЕ ИСПРАВЛЕНО)

**Симптомы.** File search в launcher находит только файлы под `%APPDATA%\Roaming\...`, не находит проекты на `C:\` / `D:\`.

**Где жило.** Гипотеза по `services/kepler-backend/src/file_index/scanner/ntfs.rs:13-29` (`scan_drive_root`) → service pipe fail → `Volume::new(\\.\C:)` fail (admin required) → `services/kepler-backend/src/file_index/scanner.rs:70` (`scan_walk_root`) fallback. Default roots в `scanner::default_roots()` **корректны** — `GetLogicalDrives` + `GetDriveTypeW`, lock_dir НЕ передаётся как root.

**Root cause.** Архитектурная хрупкость — два exclusive fast-path (focus-svc named pipe / direct NTFS USN) **оба требуют privilege** (installed Windows service / admin token). Fallback на user-space `WalkDir` от C:\ настолько медленный что initial rescan не выходит за пределы системных каталогов за обозримое время; пользователь видит первые indexed файлы (которые случайно в `C:\Users\<u>\AppData\` потому что walk идёт alphabetically/depth-first из C:\) и считает что indexer работает только в AppData.

**Fix.** Только regression-тесты в `services/kepler-backend/src/file_index/scanner.rs::tests`: `env_override_takes_precedence`, `windows_default_returns_real_drive_roots` (`cfg(windows)`). Проверяют что default roots — настоящие drive letters формата `<L>:\`, не path в AppData. Защищают от регрессии «случайно поставить lock_dir как root».

**Реальный fix** требует substantial-задачи (отдельный proof loop в `.agent/tasks/`):

1. **Авто-инсталл `kepler-focus-svc` через UAC prompt** при первом file search → доступ к named pipe `\\.\pipe\kepler-focus-svc` → быстрый NTFS scan через installed service running as SYSTEM, либо
2. **Партиальные результаты с прогресс-баром UX** — показывать «индексация идёт, найдено N файлов» вместо тишины, либо
3. **Opt-in elevated initial scan** — однократный UAC prompt → direct `Volume::new(\\.\C:)` от admin → последующие scan'ы инкрементальные через watcher.

**Регрешн-защита.** 2 unit-теста выше (от другого класса регрессии — «lock_dir as root»). Реальная проблема fallback-UX не покрыта тестом — она требует functional testing, который пока не setup'нут.

**Prevention.** **Когда fast-path требует elevated privilege, fallback должен быть либо honestly-degraded (показывать пользователю «file search недоступен, установите helper»), либо реально работающим. Silent slow-walk fallback — анти-паттерн**: пользователь видит UI без feedback и считает фичу сломанной. Применимо ко всем фичам Kepler, требующим admin (focus mode hosts write, USN scanning, в будущем — keyboard global shortcuts).

---

## 2026-05-23 — Kepler: toggle автозапуска фейлится с «не удалось применить настройку»

**Симптомы.** Settings → переключатель «Автозапуск с Windows». Клик — UI показывает «Не удалось применить настройку», хотя запись физически появляется в `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.

**Где жило.** `shell/electron/settings-window.ts:153` (`isAutostartEnabled`) + `:177` (verify-readback в `setAutostartEnabled`).

**Root cause.** Асимметричный Electron Windows API: `app.setLoginItemSettings({ path, args, openAtLogin })` пишет в HKCU значение `"<exe>" --autostart`. `app.getLoginItemSettings()` **без аргументов** читает HKCU и сравнивает с дефолтным `process.execPath` (без args) — mismatch → возвращает `openAtLogin: false`, хотя запись физически есть. Это документированное поведение Electron, но non-obvious: написал с `{path, args}` → должен и читать с `{path, args}`. Симптом «set успешен, get возвращает false → verify фейлится → UI показывает error».

**Fix.** Константа `AUTOSTART_ARGS = ["--autostart"]` единственный источник. И `isAutostartEnabled()`, и verify-readback внутри `setAutostartEnabled` передают `{ path: process.execPath, args: AUTOSTART_ARGS }` в `getLoginItemSettings`. Симметрично с `set`.

**Регрешн-защита.** Defense-in-depth: одна константа гарантирует что изменение `args` потребует менять оба места одновременно. E2e на HKCU непрактично (Playwright бежит в test slot где autorunEnabled=false) — manual repro: prod-сборка → Settings → toggle → перезагрузка → проверить состояние тоггла и HKCU.

**Prevention.** **Когда Electron-API имеет парный `set`/`get`, проверяй симметричность сигнатур сразу.** Любая запись в реестр с non-default arguments должна верифицироваться чтением **с теми же** arguments. Применимо везде где есть `app.getXxx()` / `app.setXxx()` с optional parameters — `setUserTasks`, `setJumpList`, `setLoginItemSettings`.

---

## 2026-05-23 — Kepler: launcher window не показывается при manual launch

**Симптомы.** При запуске `Kepler.exe` (через ярлык, exe, после установки NSIS, после autoupdater quit-and-install) launcher window не появляется — пользователь видит только tray icon, не понимает запустилось ли приложение вообще. (Пользователь жаловался на противоположный симптом — окно показывается при autostart — но фактический баг был симметричен: «всегда скрыто», т.е. оба пути сломаны.)

**Где жило.** `shell/electron/main.ts:1308-1310` в `app.whenReady().then(...)` — `createLauncher()` создавал окно с `show: false` и `launcherHidden = true`, но **ни одной ветки не было** которая бы дёргала `showLauncher()` при manual launch. Маркер `--autostart` уже выставлялся в `setAutostartEnabled` через args, но в `whenReady` им только логировали диагностику — поведение не зависело.

**Root cause.** Эволюционный артефакт. Изначально launcher был hidden-by-default + hotkey-only (стиль PowerToys Run / Spotlight). Но Kepler **также** имеет tray + меню запуска + ярлык — для пользователя, кликнувшего exe вручную, hidden-by-default UX-сломан (нет обратной связи). Маркер `--autostart` был добавлен для будущей дифференциации, но саму дифференциацию никто не дописал. Комментарий в `main.ts:1299` гласил «launcher по умолчанию hidden — это by design», что **закрепляло баг как намерение**.

**Fix.** Чистая функция `shouldShowLauncherOnStartup(argv): boolean` в `main.ts` — возвращает `!argv.includes("--autostart")`. После `createTray()` в `whenReady` вызываем `showLauncher()` если функция вернула true. Согласовано с `AUTOSTART_ARGS` в settings-window.ts (Bug «toggle автозапуска фейлится» fix зависит от того же маркера).

**Регрешн-защита.** `shouldShowLauncherOnStartup` — чистая функция от argv, экспортирована, тестируется без Electron mock'ов. Manual repro: `Kepler.exe` → окно показывается; `Kepler.exe --autostart` → tray-only.

**Prevention.** **Любая стартовая UX-логика разветвляющаяся по argv/env должна жить в маленькой чистой функции с явным test-surface** — иначе превращается в куски кода с шестью `if (process.env...)` ветками без проверок. Комментарии вида «by design» **без spec-ссылки** — красный флаг: если правило настоящее — оно в `docs-site/`, если нет — фоссилизированный артефакт, который маскирует bug как намерение.

---

## 2026-05-23 — Horologion: focus widget пропадает между фазами pomodoro

**Симптомы.** Pomodoro запущен, focus widget (docked корнер) показывается нормально, через ~25 минут (длительность work-фазы) **пропадает**. Pomodoro session при этом не stop'нута — `completed_pomodoros` уже инкрементнут, phase = ShortBreak (или LongBreak) с `isRunning=false`, ждёт ручного Skip/Start. Пользователь думает что pomodoro «всё ещё идёт», индикатора нет.

**Где жило.** `shell/electron/focus-widget.ts:446` (`deriveFocusStateFromBackend`, `widgetActive = isRunning && phase !== "idle"`) + `extensions/horologion/src/lib/usePomodoroSession.ts:147` (`pushFocusWidgetState`, `active = isRunning.value && !isPaused.value && phase.value !== "idle"`).

**Root cause.** Два разных контракта столкнулись. Backend session (`crates/ark-core/rust/src/pomodoro/session.rs:399-438`, `finish_phase`) при `auto_start_break=false` (default config) после work делает `is_running=false`, эмитит `Finished`, затем переключает phase на ShortBreak/LongBreak с `is_running=false` и эмитит `PhaseChanged`. Backend трактует «не idle» = «session жива» (включая межфазный простой); widget derive трактовал «visible» как «активно тикает». При корректном backend-state `phase=ShortBreak, isRunning=false` widget уходил в hide, потеряв связь с реально живущей session. Единственный валидный индикатор «session закончилась» — `phase === "idle"`, потому что **только** `Session::stop()` возвращает phase в Idle.

**Fix.** В обоих местах деривации (main process + renderer push) `widgetActive` теперь `phase !== "idle"`. `phaseEndsAtMs` остаётся `null` пока `isRunning && !isPaused` ложен — автономный tick widget'а не запустится при межфазном простое, виджет покажет статичный MM:SS = `remainingSec`.

**Регрешн-защита.** `tests/e2e/horologion-focus-widget-between-phases.spec.ts` (headless Playwright): start pomodoro через ARK op → assert widget active → skip (work→ShortBreak с isRunning=false) → assert widget **всё ещё** active и BrowserWindow жив → stop → assert widget inactive. Без зависимости от реального 25-минутного wait.

**Prevention.** **Источник правды о видимости UI-элемента должен быть один и совпадать с lifecycle сущности, а не с её sub-state'ом.** «Сессия жива» (session.is_alive() = phase !== Idle) ≠ «активно тикает» (is_running). Если деривация одного UI-state'а живёт в двух местах (main derive + renderer push) — оба должны использовать **одну** чистую функцию, а не дублировать формулу. Кандидат: вынести `deriveWidgetActive(phase, isRunning, isPaused)` в `@kosmos/ark` или общий util и импортировать в обоих сайтах. Применимо ко всем UI-элементам которые tied к long-running backend state'у — focus widget, sync indicator, dashboard, любой tray badge.

---

## 2026-05-23 — Eden: drag-select прыгает по viewport

**Симптомы.** Mouse drag-select в Eden — viewport резко прокручивается вверх/вниз даже когда курсор далеко от края окна. Обычный in-view drag-select становится практически невозможным.

**Где жило.** `extensions/eden/src/Editor.css:28-29` + дубль `extensions/eden/src/App.css:2283-2284` — `scroll-padding-top: 30vh; scroll-padding-bottom: 30vh` на `.editor-wrapper`.

**Root cause.** Chromium во время mouse-drag для text-selection периодически вызывает native `scrollIntoView` для selection endpoint'а. `scroll-padding` участвует в расчёте видимости — что в padding-зоне считается «невидимым». При `padding: 30vh` любая selection в верхних или нижних 30% viewport'а триггерила scroll-into-view → viewport «догонял» selection прыжками. Не воспроизводится в jsdom (нет реализации selection scroll), не отлавливается через JS stack trace (вызов — из native Chromium).

**Fix.** Удалить `scroll-padding-top/bottom` из обоих селекторов `.editor-wrapper`. Breathing room «снизу при письме» сохранён через существующий `padding-bottom: 50vh` на `.ProseMirror` — это реальный layout, не виртуальный через scroll-padding. Autoscroll при выходе курсора за пределы окна (native Chromium + `useBlockSelection` для rubber-band) продолжает работать.

**Регрешн-защита.** Чистый CSS-фикс — JS-тест не нужен. В `Editor.css` оставлен комментарий объясняющий почему НЕ возвращать `scroll-padding`. Manual repro: открыть длинную заметку, поставить курсор в центр, drag-select мышью в пределах viewport — viewport должен оставаться стабильным.

**Prevention.** `scroll-padding` >> 0 на скролл-контейнере, содержащем contentEditable / textarea — **анти-паттерн**. Любой UA-инициированный scrollIntoView (drag-select, IME composition, find-in-page, accessibility focus) будет двигать viewport относительно padding-зоны, не относительно реальных краёв. Если нужно breathing room — используй реальный `padding-bottom` / spacer элемент.

---

## 2026-05-23 — file_index UNIQUE constraint вешает backend

**Симптомы.** После часа работы Kepler внезапно подвисает: Eden показывает infinite loading, `commands.list` / `app_index.list_all` / `focus.*` все таймаутят через 30s. Backend.exe жив как процесс, supervisor не делает respawn, но WS-server не отвечает.

**Где жило.** `services/kepler-backend/src/file_index/store.rs::replace_all` (схема `files.path TEXT PRIMARY KEY` в `store.rs:14`). Вызов из `services/kepler-backend/src/file_index/mod.rs::rescan:102`, async-spawn из `services/kepler-backend/src/main.rs:299`.

**Root cause.** `replace_all` использовал простой `INSERT INTO files (path, ...) VALUES (?, ?, ?, ?)` без дедупа и без `OR REPLACE`. NTFS fast scan на C:\ + D:\ может вернуть один и тот же абсолютный path дважды:

- Junction points (`C:\Documents and Settings` → `C:\Users`)
- Symlinks (часто в `C:\ProgramData\Package Cache`)
- Mount points (D:\ как junction внутри C:\)
- WSL'овский `\\?\` namespace, видимый из двух root'ов

Первый дубликат → `UNIQUE constraint failed: files.path` → транзакция rollback → `Err` из `rescan` → лог пишет `WARN file_index initial rescan failed` (`main.rs:307`) и task завершается. **Сам fail не должен валить backend** — но в наблюдаемом инциденте после WARN backend-лог обрывается на час, потом начинают сыпаться 30s-таймауты на всех ark-операциях. Точная second-order причина (Windows hibernate vs. блокировка tokio worker через `std::sync::Mutex<Connection>` в долгой транзакции vs. шторм событий watcher'а от browser hang) не установлена однозначно из имеющихся логов — backend stderr capture не было.

**Fix.** [commit hash] — `replace_all` теперь дедупит paths через `HashSet<&str>` перед `INSERT`. Дедуп выбран вместо `INSERT OR REPLACE` потому что:

1. Дубликаты в input — **сигнал** что scanner возвращает мусор (полезно ловить в логах позже, если такой dedup-counter добавим).
2. `INSERT OR REPLACE` молча перезаписывал бы и при реальных багах upsert-логики.
3. FTS-индекс при дубликатах путей дал бы две записи и поиск возвращал бы файл дважды.

**Регрешн-защита.** `services/kepler-backend/src/file_index/store.rs::tests::replace_all_dedupes_duplicate_paths` — кидает в `replace_all` три записи с двумя одинаковыми path'ами, проверяет что `Ok` и в БД ровно 2 строки.

**Prevention.**

- **Любой массовый bulk-insert по сырым данным из внешнего источника (FS scanner, network discovery, third-party API) обязан дедупиться перед `INSERT`.** SQLite UNIQUE — это контракт, который ты обещаешь соблюсти, не реактивный валидатор.
- **`std::sync::Mutex<Connection>` в долгих транзакциях, вызываемых из async task'а — анти-паттерн.** При 700k вставках держит worker thread tokio. Лучше: `tokio::task::spawn_blocking` или отдельный thread с channel'ом. См. [`db-resilience.md`](/concepts/db-resilience).
- **Backend должен писать stderr в файл, не только stdout.** Если stdout пропадает (zombie pipe), мы теряем все следы. Сейчас `crash_reporter` ловит panic, но stuck-без-panic — нет. Возможный TODO: периодический heartbeat-лог.
- **Supervisor должен делать health-check WS, а не только pid-alive.** Сейчас pid жив → supervisor спит, даже если WS-server stuck.

**Связанные правила.** [forbidden.md § Rust](/agents/forbidden) — про Mutex poison recovery, `RUST_BACKTRACE=1`, и обязательный `db_backup::maybe_backup_on_startup`. Этот случай показывает что недостаточно — нужен ещё health-check.
