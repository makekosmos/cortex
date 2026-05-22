# Core context для Claude Code

::: tip Что это
Этот файл — единственный источник для корневого `CLAUDE.md`. Он держится в пределах ~200 строк по [Anthropic best-practices](https://code.claude.com/docs/en/best-practices) (CLAUDE.md загружается каждую сессию каждый turn; чем больше — тем хуже Claude следует правилам).

Полный контекст репо — в остальных страницах `docs-site/`. Здесь только то, без чего Claude **сделает ошибку**. Всё ситуативное — за pointer'ами.
:::

## За 30 секунд

- **Kosmos** = монорепо личной экосистемы (Bun workspaces + Cargo workspace, Windows-only сейчас).
- **Kepler** = лаунчер (`shell/`, Electron 720×460) + `services/kepler-backend/` (Rust: command bus + WS).
- **ARK** = общий Rust+SQLite рантайм (`crates/ark-core`, бинарь `ark-core-rpc`).
- **Extensions** (`extensions/<id>/`) — Vue-приложения, грузятся в Kepler shell как отдельные окна.
- **Apps** говорят с ARK **только** через `@kosmos/ark` (TS) или `ark_core::db` (Rust). Прямые SQL writes в синхронизируемые таблицы — **запрещены**.
- **Substantial-правки** — через `.agent/tasks/<DATE>-<slug>/` proof loop.

## Карта (где что)

| Имя | Где |
| --- | --- |
| Eden (заметки, TipTap) | `extensions/eden/` |
| Delphi (задачи) | `extensions/delphi/` |
| Horologion (трекер/pomodoro) | `extensions/horologion/` |
| Arrancador (игровая библиотека) | `extensions/arrancador/` |
| Kepler shell (лаунчер) | `shell/` |
| Kepler backend (Rust) | `services/kepler-backend/` |
| ARK core (Rust runtime) | `crates/ark-core/` |
| `@kosmos/ark` (TS SDK) | `packages/ark/` |
| `@kosmos/visuals` (UI токены) | `packages/visuals/` |
| Dashboard (встроенный shell view) | `shell/src/views/Dashboard*.vue` |

Подробное описание каждого — `docs-site/agents/index.md` («Карта приложений»).

## Жёсткие запреты (универсальные)

::: danger Никогда
**Полный список** (включая per-app: Eden / Delphi / Horologion / Shell / Distribution / usage-tracker / Brand / Spaces) — `docs-site/agents/forbidden.md`. **Прочитай соответствующую секцию перед работой в области.**
:::

Здесь — только universal, действует везде:

### ARK / data

- ❌ Прямой SQL `INSERT` / `UPDATE` / `DELETE` в `objects`, `object_types`, `object_links`, `tracked_apps`, `usage_sessions`, `usage_events`, `sync_kv` из **app TS services**.
- ❌ Открытие ARK SQLite на запись через `better-sqlite3` / `sqlite3` / `node:sqlite` в app services.
- ❌ Renderer открывает SQLite (любой) напрямую.
- ❌ Direct Rust writer пишет в синхронизируемую таблицу без `ark_core::db::bump_sync_version_vector` (или `record_local_upsert` / `record_local_delete` для legacy handler'ов).
- ❌ Destructive schema migration (`DROP TABLE`, несовместимый `ALTER COLUMN`). Только `CREATE TABLE IF NOT EXISTS` и additive.

### Rust

- ❌ `Mutex::lock().unwrap()` в production code paths. Используй `lock().unwrap_or_else(|e| e.into_inner())` для poison recovery. См. `docs-site/concepts/db-resilience.md`.
- ❌ Spawn `kepler-backend` без `RUST_BACKTRACE=1`.
- ❌ Удалять `crash_reporter::install`, `db_backup::maybe_backup_on_startup`, `db::check_integrity` из `services/kepler-backend/src/main.rs::setup`.

### Тесты

- ❌ E2e без `KOSMOS_HEADLESS=1`. Любой новый `BrowserWindow` в `shell/electron/` должен respect'ить `process.env.KOSMOS_HEADLESS === "1"` (`show: !headless`, `skipTaskbar: headless`).
- ❌ `.show()` / `.showInactive()` / `.focus()` / `.setAlwaysOnTop(true)` без headless guard'а.
- ❌ Захардкоженный или дефолтный путь к user ARK DB (`%APPDATA%\Kosmos\ark.db`) в тестах. Используй `KOSMOS_DATA_DIR` override под `tests/.e2e/<spec>/`.
- ❌ Хардкодить `path.join(appData, "Kosmos"|"Kepler", ...)` — используй `resolveInstance()` / `keplerDataDir()` из `shell/electron/instance.ts`.

### Git / tooling

- ❌ `--no-verify` при коммите.
- ❌ `git push --force` в `main` / `master`.
- ❌ `git reset --hard` / `git checkout .` для уничтожения чужих изменений.
- ❌ Амендить уже опубликованные коммиты.
- ❌ Коммит секретов (`.env`, `credentials.json`, `GH_TOKEN`).
- ❌ Коммит `dist/`, `build/`, `coverage/`, `.tmp/`, `.e2e/`, `node_modules/`.
- ❌ `git add -A` на shared working tree — забирает user WIP. Точечно `git add <file>` после `git status --short`.
- ❌ `Move-Item -Force` / `Remove-Item -Recurse -Force` на `apps/<name>/` или `extensions/<id>/` пока внутри есть `node_modules/` (junction'ы → удаление таргета).

### Bump / release

- ❌ **Bump версии без явного запроса пользователя.** Никакого «попутно с фиксом», «логически завершить релизом». Bump = release = пуш-уведомление пользователю. Только по явной команде («бамп eden», «релизни»). Менять `manifest.json::version` / `package.json::version` или запускать `ext:publish` / `electron-builder --publish` — **только** по запросу. См. skill `bump`.

### UI

- ❌ Английский в UI приложений. User-facing — только русский. Английский OK для technical id'ов (`task_obj`).
- ❌ Hardcoded `#hex` / `rgb()` / кастомные шрифты. Всё через `var(--*)` из `@kosmos/visuals`.
- ❌ Свой titlebar / safe-area. Используй `<DesktopChrome>` + `<DesktopContentSurface>`.
- ❌ `e.key === "<латинская буква>"` для Ctrl/Cmd-shortcut'ов. Используй `e.code === "KeyA"` (физическая клавиша) — иначе на RU-раскладке shortcut не ловится. Non-letter keys (`Enter`, `Escape`, arrows, F1-F12) — `e.key` OK.
- ❌ Nested interactive elements (`role="button"` на `<span>` внутри `<button>`).
- ❌ `addEventListener` в `onMounted` без `removeEventListener` в `onBeforeUnmount`.

### Framework / architecture

- ❌ Предлагать миграцию с Electron на Tauri / Wails. Решение зафиксировано экспериментом 2026-05-19 (`docs-site/experiments/tauri-vs-electron.md`).
- ❌ Возврат `apps/kosmos-shell/` или `services/kosmos-backend/` (после brand swap 2026-05-14 они `shell/` и `services/kepler-backend/`).
- ❌ «Kosmos launcher» / «Kosmos shell» в коде/доках. Лаунчер — **Kepler**, экосистема — **Kosmos**.

### Дисциплина

- ❌ «Попутно отрефакторил» вместе с задачей. Один логический change — один коммит.
- ❌ Защитный код для невозможных случаев. Fallback'и «на всякий случай».
- ❌ Создание новых `.md` файлов без явного запроса.
- ❌ Создание правил в `MEMORY.md` / `AGENTS.md` без согласования.
- ❌ Объявление «готово» если AC не PASS или гварды не прогнаны.
- ❌ Игнорирование failing test'ов как «pre-existing». Failing тесты исправляются всегда.

## Принципы работы

1. **Не угадывай — читай источник.** Перед правкой в `extensions/<name>/` или `apps/<name>/` — соответствующий `AGENTS.md` / `CLAUDE.md` в этой папке (если есть). Перед правкой в data-слое — `docs-site/concepts/write-boundary.md`.
2. **Меньший defensible diff** — только то, что в задаче. Никакого попутного рефакторинга.
3. **Прогоняй гварды:**
   - После правок в data-слой (`shell/electron`, `extensions/<id>/src`, `services/kepler-backend/src/usage_tracker`): `bun run ark:guard:writes`.
   - После любой substantial-правки: `bun run ark:smoke`.
4. **Калибруй time-estimate** через skill `estimate-calibration` перед тем как назвать срок пользователю.
5. **Visual verify перед «готово»** для UI правок — build/typecheck недостаточно. Либо Playwright spec + screenshot, либо честно «не проверял».

## Что substantial → нужен proof loop

- Новая фича / новый ARK endpoint / изменение схемы / sync-протокола / write-boundary.
- Нетривиальный багфикс (несколько файлов).
- Архитектурное решение (требует ADR).

**Не substantial:** опечатки, переименование переменной, одна строка в UI, косметика, patch-bump зависимости. Для них proof loop не нужен — просто правь.

Полный proof loop — `docs-site/concepts/proof-loop.md`.

## Поддержка документации

Источник правды — `docs-site/`. После публичных правок:

1. Правишь страницы в `docs-site/` (**не** `AGENTS.md` / `CLAUDE.md` — auto-generated, затрутся).
2. `bun run docs:sync` — регенерация `AGENTS.md`, `CLAUDE.md`, per-area `AGENTS.md`, `llms.txt`.
3. `bun run docs:check` — проверка путей/команд/ссылок.

## Дальше (читай по необходимости)

::: tip Pointer'ы, а не инлайн
Эти страницы **не** загружаются автоматически в context. Открывай их по мере необходимости — когда задача коснулась соответствующей области.
:::

**Старт и общие правила:**
- `docs-site/agents/index.md` — полная карта приложений + принципы.
- `docs-site/agents/forbidden.md` — **полный** список «никогда», включая per-app (Eden / Delphi / Horologion / Shell / Command bus / Distribution / Spaces / usage-tracker).
- `docs-site/agents/checklists.md` — чек-листы по областям перед сдачей.
- `docs-site/reference/rules.md` — сжатый TL;DR.

**Концепты архитектуры:**
- `docs-site/concepts/architecture.md` — общая картина.
- `docs-site/concepts/ark-objects.md` — модель данных ARK.
- `docs-site/concepts/write-boundary.md` — граница записи.
- `docs-site/concepts/sync.md` — синхронизация.
- `docs-site/concepts/proof-loop.md` — substantial-задачи.
- `docs-site/concepts/test-isolation.md` — изоляция тестовых БД.
- `docs-site/concepts/command-bus.md` — command bus.
- `docs-site/concepts/extension-host.md` — как extension'ы живут в shell.
- `docs-site/concepts/instances.md` — slot-based изоляция (prod / dev / test).

**Operational:**
- `STATUS.md` (корень) — актуальный snapshot состояния проекта.
- `docs-site/agents/testing.md` — e2e правила (headless mode, universal contract).
- `docs-site/agents/spec-templates.md` — шаблоны `spec.md`.
- `docs-site/agents/estimation.md` — калибровка оценок времени.
- `docs-site/agents/manual-tests-pending.md` — TODO визуальных проверок.
- `docs-site/agents/docs-maintenance.md` — поддержка документации.
- `docs-site/reference/smoke-matrix.md` — что прогонять перед PR.

**Полный набор правил** также доступен в корневом `AGENTS.md` (auto-generated, ~92k байт — не для каждой сессии, читай при необходимости).
