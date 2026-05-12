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

- ❌ Дефолт пути к user ARK DB (`%APPDATA%\Kepler\ark.db`) в тестах.
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
- ❌ Deep import shared компонентов вместо public API `@kepler/visuals`.
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

### usage-tracker

- ❌ Превращение в Windows Service.
- ❌ Добавление UI / tray icon / окон.
- ❌ Прямой SQL write без `ark_core::db` хелперов и без обновления `version_vector`.

## Git / tooling

- ❌ `--no-verify` при коммите.
- ❌ `git push --force` в `main` / `master`.
- ❌ `git reset --hard` или `git checkout .` для уничтожения чужих изменений.
- ❌ Амендить уже опубликованные коммиты.
- ❌ Коммит файлов с секретами (`.env`, `credentials.json`).
- ❌ Коммит `dist/`, `build/`, `coverage/`, `.tmp/`, `.e2e/`, `node_modules/`.
- ❌ Создание новых правил в `MEMORY.md` или AGENTS.md без согласования с человеком.

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
