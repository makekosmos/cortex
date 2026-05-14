# Запреты и гварды

::: danger Никогда
Этот список — нерушимое. Не «лучше не делать», а **запрещено**. Если ты как агент думаешь нарушить что-то отсюда — остановись и спроси человека.
:::

## ARK writes

- ❌ **Прямой SQL `INSERT` / `UPDATE` / `DELETE`** в `objects`, `object_types`, `object_links`, `tracked_apps`, `usage_sessions`, `usage_events`, `sync_kv` из **app TS services**.
- ❌ **Открытие ARK SQLite на запись** в app services через `better-sqlite3`, `sqlite3`, `node:sqlite` и т.п.
- ❌ Renderer открывает SQLite (любой) напрямую.

## Sync

- ❌ Direct Rust writer пишет в ARK без вызова `ark_core::db::bump_sync_version_vector`.
- ❌ Ослабление self-peer filtering при изменениях в sync startup.
- ❌ Ослабление routable-address filtering при изменениях в peer persistence.
- ❌ Изменение sync wire-протокола из `snake_case` в что-то другое.
- ❌ Destructive schema migration (`DROP TABLE`, `ALTER COLUMN` несовместимо). Только `CREATE TABLE IF NOT EXISTS` и additive.

## Тесты

- ❌ Дефолт пути к user ARK DB (`%APPDATA%\Kosmos\ark.db`) в тестах.
- ❌ Захардкоженный путь к real user dir (типа `C:\Users\me\AppData\...`).
- ❌ Запуск миграции/backfill против реальной ARK DB «чтобы проверить».
- ❌ Запуск Playwright против user vault Eden.

## Proof loop

- ❌ Объявить задачу завершённой, пока **каждый** AC не `PASS`.
- ❌ Редактировать `spec.md` после старта реализации.
- ❌ Fixer делает «попутный рефактор» вместе с fix'ом.

## Per-app запреты

### Eden

- ❌ Возврат к ripgrep как поисковому движку.
- ❌ Упрощение hardening для `save` / `move` / `delete` в `main/store.ts`.
- ❌ Возврат ручных `--titlebar-height` / `--titlebar-left-safe-area` костылей.
- ❌ Использование `vue-router` для titlebar history controls (нужна локальная история Eden).
- ❌ Deep import shared компонентов вместо public API `@kosmos/visuals`.
- ❌ Возврат `vite-plugin-electron` (миграция на `electron-vite` сделана).

### Delphi

- ❌ Восстановление, упаковка или использование legacy Delphi DB sidecar.
- ❌ Использование old todo таблиц как long-term fallback после миграции в `task_obj`.

### Arrancador

- ❌ In-process activity tracker / window polling loop внутри Arrancador.
- ❌ Arrancador-owned usage SQLite.
- ❌ Tauri зависимости / Tauri runtime пути.
- ❌ React зависимости / React runtime пути.

### Dashboard

- ❌ SQLite open в renderer.
- ❌ ARK queries вне `electron/services/analytics.ts`.
- ❌ Любые **writes** в ARK таблицы.
- ❌ Копирование shared sidebar / токенов внутрь `apps/dashboard`.

### Kepler Shell (launcher)

- ❌ Возврат к ARK FTS5 search внутри лаунчера вместо command bus (был pivot — отброшен).
- ❌ Per-frame window resize animation: Win32 не успевает, окно дёргается. Размер окна — fixed 720×460.
- ❌ Hardcoded action commands в `apps/kepler-shell/electron/commands.ts`. Action-команды приходят dynamic от приложений через command bus, в `commands.ts` хардкодятся только `open`-команды (запуск приложения по имени).
- ❌ Использование `win.webContents.id` внутри `closed` event handler. После `closed` webContents уже destroyed — capture id в локальную `const wcId` **до** `win.on("closed", ...)`. См. [Extension host → Crash safety](/concepts/extension-host#crash-safety).
- ❌ Удаление `electron-api-shim.ts` в Delphi extension. Это compat-слой эмулирующий `window.electronAPI` поверх kepler ark bridge — без него сломаются ~30 call sites Delphi CRUD без переписывания. Миграция UI на нативный API — отдельная Phase 9.
- ❌ Загрузка extension renderer с `file://path/to/dist` когда хочешь HMR. В dev mode (`KEPLER_DEV=1` или Settings → Developer Mode) используй `loadURL('http://localhost:<devPort>/')` с поднятым Vite dev server'ом. См. [Extension dev mode](/concepts/extension-dev-mode).

### Command bus

- ❌ Nested wire format событий `{kind: "event", type: "...", payload: {...}}`. Только flat: `{event: "...", ...fields}` — это согласовано с peer/sync events.
- ❌ Регистрация commands вне `kepler-mode`. Self-managed / standalone-запуск приложения **не** должен падать из-за отсутствия commands API — оборачивай в `try/catch`.
- ❌ Прямой WS-доступ к backend из renderer'а приложений в обход `@kosmos/ark` SDK.

### Brand consistency

- ❌ «Kosmos launcher» / «Kosmos shell» в коде или документации. Лаунчер — **Kepler**. Экосистема — **Kosmos**.
- ❌ Возврат `apps/kosmos-shell/` или `services/kosmos-backend/`. После swap 2026-05-14 они называются `kepler-shell` и `kepler-backend`.

### usage-tracker

- ❌ Превращение в Windows Service.
- ❌ Добавление UI / tray icon / окон.
- ❌ Прямой SQL write без `ark_core::db` хелперов и без обновления `version_vector`.

## Файловые операции на Windows

::: danger Junction'ы bun workspaces
В этом репо `bun install` создаёт junction'ы (Windows-симлинки) в `apps/<name>/node_modules/@kosmos/<pkg>` → `packages/<pkg>`. PowerShell `Move-Item -Force` (и многие GUI-операции) **разрешают** junction'ы и удаляют **таргет** вместе с источником — а Корзину минуют. Так уже было потеряно несколько часов untracked-работы в `packages/kosmos-visuals/`. Восстановление возможно только если файлы успели попасть в asar предыдущего билда.
:::

- ❌ `Move-Item -Force` или `Remove-Item -Recurse -Force` на `apps/<name>/` целиком, пока внутри есть `node_modules/`. Сначала **удали** `apps/<name>/node_modules/` (`Remove-Item -Recurse -Force apps\<name>\node_modules`), и **только потом** перемещай или удаляй директорию.
- ❌ Переименование/перемещение `apps/<name>/` без предварительной коммитной зачистки untracked-файлов в `packages/*`. Если что-то ценное лежит как `??` в `git status` — закоммить или временно сохрани вне репо, иначе `Move-Item` уничтожит таргет junction'а навсегда.
- ❌ Удаление любых директорий внутри `apps/` или `packages/` через GUI-проводник Windows. Используй `git rm`, `Remove-Item` после удаления `node_modules`, или CLI с явным контролем.
- ❌ `rm -rf packages/...` или эквиваленты, если результат можно достичь через `git restore` / переключение веток.

## Git / tooling

- ❌ `--no-verify` при коммите.
- ❌ `git push --force` в `main` / `master`.
- ❌ `git reset --hard` или `git checkout .` для уничтожения чужих изменений.
- ❌ Амендить уже опубликованные коммиты.
- ❌ Коммит файлов с секретами (`.env`, `credentials.json`).
- ❌ Коммит `dist/`, `build/`, `coverage/`, `.tmp/`, `.e2e/`, `node_modules/`.
- ❌ Создание новых правил в `MEMORY.md` или AGENTS.md без согласования с человеком.

## UI

- ❌ Английский язык в UI приложений (placeholder'ы, лейблы, кнопки, эмпти-стейты, заголовки). User-facing — только русский. Английский OK для technical id'ов (`task_obj`, `time_entry_obj`).
- ❌ Hardcoded `#hex`, `rgb()`, кастомные шрифты в renderer-коде. Все цвета / радиусы / шрифты — через `var(--*)` из `@kosmos/visuals`.
- ❌ Свой titlebar / safe-area код. Всегда через `<DesktopChrome>` + `<DesktopContentSurface>`.

## Общая дисциплина

- ❌ «Попутно отрефакторил» вместе с задачей. Один логический change — один коммит.
- ❌ Добавление защитного кода для невозможных случаев.
- ❌ Создание новых документов (`.md` файлов) без явного запроса.
- ❌ Изменение `package.json` без видимой причины (особенно версий зависимостей).
- ❌ Игнорирование AC из `spec.md`. Если AC кажется неверным — это новая задача / обсуждение.
- ❌ Объявление «всё работает» без прогона гвардов и smoke.

## Гварды-команды

Прогнать **обязательно** в указанных случаях:

```powershell
# Перед PR в data services (apps/*/electron/main/services/, services/usage-tracker)
bun run ark:guard:writes

# Перед PR в любую substantial-задачу
bun run ark:smoke
```

См. [Smoke-матрица](/reference/smoke-matrix).
