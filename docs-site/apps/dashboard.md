# Dashboard — read-only аналитика ARK

::: tip Источник правды
`extensions/dashboard/` + `docs/ARK-READONLY-SQL-BOUNDARY.md`
:::

Dashboard — read-only observability для ARK usage data: foreground sessions, recorded time, app ranking, recent session playback. После Phase B-D — Vue-extension внутри Kepler shell.

## Стек

- Frontend: Vue 3 + Composition API + `<script setup lang="ts">`.
- Runtime: Vue-extension в `extensions/dashboard/`, открывается через Kepler shell `extension-host.ts`.
- Data access: read-only ARK через `window.kepler.ark.request(...)` (kepler-backend → ark-core-rpc).
- Shared UI: `@kepler/visuals`.

## Структура

```
extensions/dashboard/
├─ manifest.json                # id, title, devPort, capabilities
├─ index.html
├─ vite.config.mjs              # @kepler/visuals + Tailwind v4 (если используется)
└─ src/
   ├─ main.ts                   # entry: createApp(...).mount('#app')
   ├─ App.vue                   # root composition surface
   ├─ pages/                    # Overview, Sessions, ...
   ├─ components/               # секции UI и чарты
   ├─ composables/
   │  └─ useDashboardData.ts    # ARK reads
   └─ utils/format.ts
```

## Жёсткие правила

::: danger
- Renderer **НИКОГДА** не открывает SQLite напрямую. Все DB reads — через `@kepler/ark` SDK (`window.kepler.ark.request`).
- Dashboard — **read-only**. Никаких writes в ARK таблицы.
- Когда возможно — `@kepler/ark` analytics endpoints (`get_usage_analytics`, `list_recent_usage_processes`, …). Raw SQL — запрещено.
- `@kepler/visuals` — только через import/alias. Не копируй shared sidebar или токены внутрь `extensions/dashboard`.
- Desktop chrome выровнен с shared visuals. Wire actions в dashboard, но layout primitives — в `@kepler/visuals`.
- `TitlebarHistoryControls` из `@kepler/visuals`, disabled-state — из реального состояния Vue Router history.
- Route components тонкие. Data fetching — в `useDashboardData`. Side effects — в composables.
- Hash-based router navigation (memory history внутри extension window).
:::

## Команды

Сборка происходит через Kepler shell:

```powershell
bun run --cwd shell build:extensions       # билдит Dashboard и другие extensions
bun run --cwd shell build:js               # tsc + vite + extensions
bun run --cwd shell dev                    # dev mode (backend + extensions + Kepler renderer)
```

В HMR-режиме см. [Extension dev mode](/concepts/extension-dev-mode) — extension Dashboard поднимается на одном из портов 5180-5183.

## ARK runtime endpoints, которые Dashboard предпочитает

Когда покрывают нужный view — использовать вместо raw SQL:

- `list_objects_by_type`
- `get_objects_by_ids`
- `list_recent_usage_processes`
- `search_usage_processes`
- `get_usage_game_playtime_summary`
- `get_usage_analytics`

Если endpoint'а нет — добавь его в `crates/ark-core/rust` и `packages/ark`. Не пиши свой raw-SQL слой в extension'е.

## Command bus integration

Dashboard сейчас интегрирован в [Kepler launcher](/apps/kepler) **только как static "open" команда** — `dashboard:open` открывает extension window через `extension-host.ts → openExtension('dashboard')`. Команда живёт в `shell/electron/commands.ts`.

Dynamic action commands (например `dashboard:filter:games-by-playtime`, `dashboard:report:weekly`) пока не реализованы — это работа Phase 4 roadmap'а Kepler.

## Связанные документы

- [Kepler](/apps/kepler) — host, который запускает Dashboard.
- [Command bus](/concepts/command-bus)
- [Read-only SQL boundary](/concepts/readonly-sql)
- [Граница записи в ARK](/concepts/write-boundary)
- [@kepler/visuals](/packages/visuals)
- `docs/ARK-READONLY-SQL-BOUNDARY.md`
