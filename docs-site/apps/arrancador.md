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
| Library | `src/pages/LibraryPage.vue` | ✅ list + кнопка «Запустить» (Steam URL или прямой exe) |
| Catalogue | `src/pages/CataloguePage.vue` | ✅ RAWG search + apply-to-library через Modal+Dropdown |
| Scan | `src/pages/ScanPage.vue` | ✅ «Сканировать сейчас» + Steam/Epic, история last 10 |
| Sqoba | `src/pages/SqobaPage.vue` | ✅ per-game backup list + create + restore с confirmation |
| Statistics | `src/pages/StatisticsPage.vue` | ✅ JS-агрегация по списку игр (heatmap по `usage_sessions` — stub) |
| Settings | `src/pages/SettingsPage.vue` | ✅ RAWG API key (password input + eye-toggle + status) + localStorage |
| GameDetail | `src/pages/GameDetailPage.vue` | ✅ read-only (доп. actions — follow-up) |

Routing — Vue Router с `createMemoryHistory` (нет file-system URLs внутри extension'а).

### Архитектура данных

- **`src/lib/arkGames.ts`** — ARK bridge: `arkBridge()` достаёт `window.kepler.ark`, `loadGames()` дергает `list_objects_by_type(game_obj)` и projection'ит `ArkObjectRecord` → `ArrancadorGame`. **Read-only** в extension'е; write paths появятся когда Kepler host расширит preload канал.
- **`src/composables/useGames.ts`** — shared store на vue refs (без Pinia). Singleton с ref-count, подписывается на `entity_changed` через `kepler.ark.subscribe` и автоматически refresh'ит список.
- **`src/composables/useSearchQuery.ts`** — query state для Library / Catalogue фильтрации.

### Backend (kepler-backend Rust)

После Arrancador full completion (2026-05-18) в `services/kepler-backend/src/arrancador/`:

- **`scanner.rs`** — Steam libraryfolders.vdf + appmanifest.acf custom parser (без deps), Epic Games Launcher `Data/Manifests/*.item` JSON. GOG скипнут в MVP.
- **`launcher.rs`** — `steam://rungameid/<app_id>` через `cmd /c start` для Steam, прямой `Command::new(exe).spawn()` для Epic/manual. Fire-and-forget tracking (process polling в `usage_tracker` подхватывает по имени exe).
- **`rawg.rs`** — RAWG.io HTTP client через `reqwest`, search + get_details + apply (merge metadata в `game_obj.propsJson`). API key хранится в `arrancador-config.json`.
- **`sqoba.rs`** — discover save paths (Saved Games / My Games / LocalAppData / Roaming) + zip backup в `<data_dir>/sqoba/<game_id>/<timestamp>.zip` с `_sqoba_meta.json` внутри, restore с path traversal protection, rotation keep N=10.
- **`config.rs`** — typed `ArrancadorConfig { rawg_api_key, custom_scan_paths, sqoba_dest_dir, keep_backups }` в `%APPDATA%\Kosmos\arrancador-config.json`.

WS namespace `arrancador.*` (через preload — `window.kepler.arrancador.*`):

- `scan()` → `{added, updated, skipped, errors[]}`
- `launch({game_id})` → `{ok, pid, started_at, method}`
- `rawg.search({query})`, `rawg.apply({game_id, rawg_id})`
- `sqoba.backup({game_id})`, `sqoba.list({game_id})`, `sqoba.restore({backup_id})`
- `config.get()`, `config.set_rawg_key({key})`

### Out of scope (после Arrancador completion)

- **GOG scanner** — GOG Galaxy SQLite DB парс. Skip'нуто из MVP, добавится при необходимости.
- **3rd-party launcher'ы** (Battle.net, Riot, EA Origin) — не покрыты.
- **Heatmap по `usage_sessions`** — Statistics показывает суммарный playtime, визуальный heatmap по дням/часам не подключен.
- **Achievement tracking** / In-game overlay — нет.
- **Steam path discovery через registry** (HKCU `Software\Valve\Steam\SteamPath`) — сейчас hardcode `C:\Program Files (x86)\Steam`. Override через config `custom_scan_paths` или env для тестов.

## Command bus integration

Arrancador интегрирован в [Kepler launcher](/apps/kepler) **как static "open" команда** — `arrancador:open` открывает extension window. Команда живёт в `shell/electron/commands.ts`.

Dynamic action commands (`arrancador:game:launch:<id>`, `arrancador:backup:run`) — follow-up; сейчас вызывается всё через UI в самом extension'е через `window.kepler.arrancador.*` preload bridge.

## Связанные документы

- [Kepler](/apps/kepler) — host, который запускает Arrancador.
- [Command bus](/concepts/command-bus)
- [Граница записи в ARK](/concepts/write-boundary)
- [Read-only SQL boundary](/concepts/readonly-sql)
- [Модель данных ARK](/concepts/ark-objects)
- [usage-tracker](/services/usage-tracker)
