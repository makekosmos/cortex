# Dashboard — read-only аналитика ARK

::: tip Источник правды
`apps/dashboard/AGENTS.md`, `apps/dashboard/README.md`, `docs/ARK-READONLY-SQL-BOUNDARY.md`
:::

Dashboard — desktop observability app для ARK usage data: foreground sessions, recorded time, app ranking, recent session playback. **Read-only inspector** для ARK SQLite-БД.

## Стек

- Frontend: Vue 3 + Composition API + `<script setup lang="ts">`.
- Runtime: Electron.
- Data access: read-only ARK inspection / analytics в Electron main.
- Shared UI: `@kosmos/visuals`.

## Структура

```
apps/dashboard/
├─ src/
│  ├─ App.vue                          # root composition surface
│  ├─ components/
│  │  └─ dashboard/                    # секции product UI и чарты
│  │     └─ DashboardShell.vue         # product shell, тянет shared chrome
│  ├─ pages/                           # route-level composition pages
│  ├─ composables/
│  │  └─ useDashboardData.ts           # shared state + renderer actions
│  └─ utils/format.ts
├─ shared/analytics.ts                 # IPC контракты, типы аналитики
├─ electron/
│  ├─ main.ts                          # BrowserWindow + IPC handlers
│  ├─ preload.ts                       # window.dashboardApi
│  ├─ services/analytics.ts            # boundary для ARK analytics
│  └─ db/sqlite.ts                     # node-side SQLite wrapper
└─ scripts/
   ├─ seedSmokeDb.ts                   # smoke fixture (TypeScript)
   ├─ seedSmokeDb.py                   # Python-вариант для Playwright (избегает ABI drift better-sqlite3)
   └─ smokeAnalytics.ts                # CLI smoke assertion
```

## Жёсткие правила

::: danger
- Renderer **НИКОГДА** не открывает SQLite напрямую. Все DB reads — через `window.dashboardApi`.
- Dashboard — **read-only**. Никаких writes в ARK таблицы.
- Когда возможно — `@kosmos/ark` analytics endpoints. Read-only SQLite fallback / inspector только в Electron main, не в renderer.
- `electron/services/analytics.ts` — **единственная** граница для dashboard analytics. Не дублируй ARK queries внутри Vue компонентов.
- `@kosmos/visuals` — только через import/alias. Не копируй shared sidebar или токены внутрь `apps/dashboard`.
- Desktop chrome выровнен с shared visuals. Wire actions в dashboard, но layout primitives — в `@kosmos/visuals`.
- `TitlebarHistoryControls` из `@kosmos/visuals`, disabled-state — из реального состояния Vue Router history.
- Route components тонкие. Data fetching — в `useDashboardData`. Side effects — в composables или Electron main.
- Hash-based router navigation для стабильного deep-linking и Electron e2e navigation.
:::

## Команды

```powershell
cd apps/dashboard
bun run build              # renderer + Electron bundles
bun run package:dir        # unpacked desktop bundle
bun run dist               # Windows NSIS через electron-builder
bun run smoke:seed         # сидинг smoke БД (.tmp, .e2e, .agent/tasks/<TASK>/...)
bun run smoke:analytics    # CLI assertion аналитики
bun run test:e2e           # Playwright (globalSetup сидит через seedSmokeDb.py)
bun run test:e2e:smoke     # прямой Playwright-library smoke
```

`smoke:seed` и `smoke:analytics` — verification helpers, **обязательно** указывают на `.tmp`, `.e2e`, `.agent/tasks/<TASK_ID>/` или другой изолированный путь. Smoke использует `apps/dashboard/.e2e/smoke-dashboard.db`, не user DB.

## ARK runtime endpoints, которые Dashboard предпочитает

Когда покрывают нужный view — использовать вместо raw SQL:

- `list_objects_by_type`
- `get_objects_by_ids`
- `list_recent_usage_processes`
- `search_usage_processes`
- `get_usage_game_playtime_summary`

Когда не покрывают — допустим read-only SQLite через `electron/services/analytics.ts` и `electron/db/sqlite.ts`.

## Зачем Python-сидинг

`seedSmokeDb.py` нужен потому, что Playwright `globalSetup` запускается в Node-окружении, а native-зависимость `better-sqlite3` имеет ABI, которая может разъезжаться между Node-версиями. Python-сидинг через стандартный `sqlite3` модуль избегает этой проблемы.

## Command bus integration

Dashboard сейчас интегрирован в [Kepler launcher](/apps/kepler) **только как static "open" команда** — `dashboard:open` спавнит `dashboard.exe` напрямую, без regаster в command bus от самой апки. Команда живёт в `apps/kepler-shell/electron/commands.ts` и матчится локально на стороне launcher'а.

Dynamic action commands (например `dashboard:filter:games-by-playtime`, `dashboard:report:weekly`) пока не реализованы — это работа Phase 4 roadmap'а Kepler.

### Extension в Kepler

Dashboard мигрирован в Kepler как полноценный **Vue extension** — `apps/kepler-shell/extensions/dashboard/`. Static `bundle.js` PoC заменён реальным Vue-приложением. Внутри extension'а — Overview и Sessions pages, данные тянутся через `window.kepler.ark.request("get_usage_analytics", { … })` (тот же ARK runtime endpoint, что использовал standalone `electron/services/analytics.ts`).

Standalone `apps/dashboard/` остаётся для legacy fallback и e2e smoke, но основной flow в Kosmos — открытие через `dashboard:open` из Kepler launcher'а, которое поднимает extension window. После Phase 8 standalone .exe ретайернется.

## Связанные документы

- [Kepler](/apps/kepler) — host, который запускает Dashboard.
- [Command bus](/concepts/command-bus)
- [Read-only SQL boundary](/concepts/readonly-sql)
- [Граница записи в ARK](/concepts/write-boundary)
- [kosmos-visuals](/packages/kosmos-visuals)
- `docs/ARK-READONLY-SQL-BOUNDARY.md`
