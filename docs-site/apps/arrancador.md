# Arrancador — игры, playtime, бэкапы

::: tip Источник правды
`apps/arrancador/AGENTS.md`, `apps/arrancador/README.md`, `apps/arrancador/DESIGN.md`, `apps/arrancador/TEST_STRATEGY.md`
:::

Arrancador — desktop-лаунчер для локальной игровой библиотеки. Управляет играми, отслеживает playtime (читая ARK usage data, **не** запуская собственный tracker), делает бэкапы сэйвов и держит системные инструменты.

## Стек

- Frontend: Vue 3 + TypeScript + Vite (renderer в `src-vue/`).
- Desktop shell: Electron.
- Локальная persistence: SQLite через `better-sqlite3` в Electron main.
- Native sidecars: Rust-sidecars допустимы для ограниченных нативных задач, когда Electron не хватает.

## Структура

```
apps/arrancador/
├─ src-vue/                          # активный renderer UI
├─ src/                              # shared renderer contracts и API helpers
├─ electron/
│  ├─ main.ts                        # Electron bootstrap
│  ├─ preload.ts                     # secure bridge
│  └─ main/
│     ├─ db/                         # local database adapters
│     └─ services/
│        ├─ games.ts                 # game CRUD / launch orchestration
│        ├─ ark-usage.ts             # ARK usage reader (prefer @kepler/ark)
│        ├─ ark-usage-backfill.ts    # legacy usage import через ARK usage/KV
│        ├─ ark-usage-bindings.ts    # биндинги game ↔ exe для usage
│        ├─ ark-game-objects.ts      # game_obj hydration и writes через @kepler/ark
│        ├─ ark-game-migration.ts    # миграция в game_obj
│        ├─ playtime-stats.ts        # делегирует aggregation в ARK runtime
│        └─ usage-process-search.ts  # поиск процессов через ARK
└─ scripts/                          # dev / build helpers
```

## Модель данных

- **Игры** — ARK-объекты `game_obj`.
- **Playtime / usage** — отдельный usage-слой ARK (`tracked_apps`, `usage_sessions`, `usage_events`).
- **Бэкапы и process search** — через `@kepler/ark`.
- Read-only SQLite — только fallback, когда `ark-core-rpc` недоступен. Запись запрещена.

## Команды

```powershell
cd apps/arrancador
bun run typecheck
bun run build:renderer
bun run build:main
bun run build:preload
bun run test               # тесты renderer-логики
bun run smoke:packaged     # smoke на packaged Electron сборке
```

## Правила

- Renderer вызывает preload, preload — explicit main-process handlers. Узкая граница.
- **ARK reads** через `@kepler/ark` сначала. Direct ARK SQLite reads — только inspector/fallback, **только** main-process сервисы, **только** read-only.
- **ARK writes** через `@kepler/ark`. Не писать напрямую в `objects`, `object_types`, `object_links`, `tracked_apps`, `usage_sessions`, `usage_events`, `sync_kv` из app services.
- Игры мапятся на process bindings внутри Arrancador, дальше ARK runtime агрегирует usage.
- **Не возвращай** в Arrancador собственный in-process tracker, window polling loop или Arrancador-owned usage SQLite. Usage capture теперь живёт в `services/usage-tracker`.
- Native работа — сначала Electron main-process services. Rust только как sidecar с явным Electron integration path.
- React и Tauri **не** активные runtime пути для Arrancador.

## Тесты

- Vitest-наборы рядом с каждым сервисом (`*.test.ts`).
- Все на изолированных DB или замоканных ARK API.
- `bun run smoke:packaged` собирает unpacked Windows-бандл, запускает `release/win-unpacked/arrancador.exe` с временными `APPDATA`, `LOCALAPPDATA`, `ARK_DB_PATH` под `apps/arrancador/.e2e/packaged-smoke`.

## ARK runtime endpoints

Arrancador использует:

- `objects.listByType('game_obj')` — игры
- `objects.getMany([...])` — bulk-hydration
- `usage.processes.recent(n)` — последние процессы
- `usage.processes.search(query, limit)` — поиск
- `usage.gamePlaytime.summary({ bindings, rangeStart, rangeEnd })` — агрегация playtime

Прямой SQL остаётся как read-only fallback **и** для inspector-режима, не для основного flow.

## Связанные документы

- [Граница записи в ARK](/concepts/write-boundary)
- [Read-only SQL boundary](/concepts/readonly-sql)
- [Модель данных ARK](/concepts/ark-objects)
- [usage-tracker](/services/usage-tracker)
