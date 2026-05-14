# Arrancador — игры, playtime, бэкапы

::: tip Источник правды
`extensions/arrancador/`
:::

Arrancador — лаунчер для локальной игровой библиотеки. Управляет играми, отслеживает playtime (читая ARK usage data, **не** запуская собственный tracker), делает бэкапы сэйвов и держит системные инструменты. После Phase B-D — Vue-extension в Kepler shell.

## Стек

- Frontend: Vue 3 + TypeScript + Vite (renderer внутри extension window).
- Runtime: `extensions/arrancador/` — Vue-bundle, открывается через Kepler shell `extension-host.ts`.
- Persistence: ARK objects + `@kepler/ark` SDK (никаких локальных SQLite в extension).

## Структура

```
extensions/arrancador/
├─ manifest.json
├─ index.html
├─ vite.config.mjs
└─ src/
   ├─ main.ts                    # entry
   ├─ App.vue                    # root
   ├─ pages/                     # Library, Catalogue, Scan, Statistics, Settings, GameDetail, Sqoba
   ├─ components/
   ├─ composables/
   │  ├─ useGames.ts             # shared store, реагирует на entity_changed
   │  └─ useSearchQuery.ts
   └─ lib/
      └─ arkGames.ts             # ARK bridge — list_objects_by_type / projections
```

## Модель данных

- **Игры** — ARK-объекты `game_obj`.
- **Playtime / usage** — usage-слой ARK (`tracked_apps`, `usage_sessions`, `usage_events`), captured модулем `services/kepler-backend/src/usage_tracker/` (Phase E).
- **Бэкапы и process search** — через `@kepler/ark`.

## Команды

Сборка проходит через Kepler shell:

```powershell
bun run --cwd shell build:extensions
bun run --cwd shell build:js
bun run --cwd shell dev
```

## Правила

- ARK reads/writes — **только** через `@kepler/ark`. Никаких прямых SQL writes (см. [Граница записи](/concepts/write-boundary)).
- Игры мапятся на process bindings внутри Arrancador, дальше ARK runtime агрегирует usage.
- **Не возвращай** в Arrancador собственный in-process tracker, window polling loop или Arrancador-owned usage SQLite. Usage capture теперь живёт в `services/kepler-backend/src/usage_tracker/`.
- React и Tauri **не** активные runtime пути для Arrancador.

## ARK runtime endpoints

Arrancador использует:

- `objects.listByType('game_obj')` — игры
- `objects.getMany([...])` — bulk-hydration
- `usage.processes.recent(n)` — последние процессы
- `usage.processes.search(query, limit)` — поиск
- `usage.gamePlaytime.summary({ bindings, rangeStart, rangeEnd })` — агрегация playtime

## Страницы и их состояние

| Страница | Файл | Состояние |
|---|---|---|
| Library | `src/pages/LibraryPage.vue` | ✅ list из `game_obj` через `useGames` |
| Catalogue | `src/pages/CataloguePage.vue` | ⏳ stub (RAWG metadata не подключен) |
| Scan | `src/pages/ScanPage.vue` | ✅ read-only (native scanner spawn — stub) |
| Sqoba | `src/pages/SqobaPage.vue` | ⏳ stub |
| Statistics | `src/pages/StatisticsPage.vue` | ✅ JS-агрегация по списку игр (heatmap по `usage_sessions` — stub) |
| Settings | `src/pages/SettingsPage.vue` | ✅ localStorage |
| GameDetail | `src/pages/GameDetailPage.vue` | ✅ read-only (game launch — stub) |

Routing — Vue Router с `createMemoryHistory` (нет file-system URLs внутри extension'а).

### Архитектура данных

- **`src/lib/arkGames.ts`** — ARK bridge: `arkBridge()` достаёт `window.kepler.ark`, `loadGames()` дергает `list_objects_by_type(game_obj)` и projection'ит `ArkObjectRecord` → `ArrancadorGame`. **Read-only** в extension'е; write paths появятся когда Kepler host расширит preload канал.
- **`src/composables/useGames.ts`** — shared store на vue refs (без Pinia). Singleton с ref-count, подписывается на `entity_changed` через `kepler.ark.subscribe` и автоматически refresh'ит список.
- **`src/composables/useSearchQuery.ts`** — query state для Library / Catalogue фильтрации.

### Что НЕ мигрировано (stubs)

Эти функции в extension'е отсутствуют, ожидают либо расширения preload API, либо отдельной Phase 5+ работы:

- **RAWG metadata** — fetch'а нет, Catalogue показывает заглушку.
- **Native scanner spawn** — sidecar для сканирования диска (Steam/Epic/GOG) недоступен из extension renderer'а напрямую. План — мигрировать в `services/kepler-backend` Rust либо в shell sidecar (см. [Decisions → 2026-05-14 Arrancador native scanner](/reference/decisions#2026-05-14-arrancador-native-scanner-остался-в-legacy)).
- **Game launch** — `ShellExecute(exePath)` требует Electron main / Rust shell — в extension renderer'е нет такого канала.
- **Heatmap по `usage_sessions`** — Statistics показывает суммарный playtime, но визуальный heatmap по дням / часам не подключен.

## Command bus integration

Arrancador сейчас интегрирован в [Kepler launcher](/apps/kepler) **как static "open" команда** — `arrancador:open` открывает extension window. Команда живёт в `shell/electron/commands.ts`.

Dynamic action commands (`arrancador:game:launch:<id>`, `arrancador:backup:run`) пока не реализованы.

## Связанные документы

- [Kepler](/apps/kepler) — host, который запускает Arrancador.
- [Command bus](/concepts/command-bus)
- [Граница записи в ARK](/concepts/write-boundary)
- [Read-only SQL boundary](/concepts/readonly-sql)
- [Модель данных ARK](/concepts/ark-objects)
- [usage-tracker](/services/usage-tracker)
