# Dashboard — встроенный ARK browser

::: tip Источник правды
`shell/src/views/DashboardRoot.vue`, `shell/src/views/DashboardView.vue`,
`shell/src/dashboard/`, `shell/electron/dashboard-window.ts`
:::

Dashboard — встроенная часть Kepler shell'а (не extension). Read-only
ARK browser: sidebar с object_types + таблица объектов.

::: info Pivot 2026-05-14 → 2026-05-15
До 2026-05-14 Dashboard жил как Vue extension и показывал usage analytics
(foreground sessions, app ranking и т.п.). По решению пользователя
переосмыслен как ARK browser и встроен в shell. Старый extension
заморожен в `legacy/dashboard-extension/`.

2026-05-15 — концепция spaces (welcome screen → space picker → space view)
убрана. Dashboard сразу открывается на единый список объектов поверх
одной DB на юзера (`%APPDATA%\Kosmos\ark.db`). См.
[Архитектура — SQLite](/concepts/architecture#sqlite).
:::

## Стек

- Frontend: Vue 3 + Composition API + `<script setup lang="ts">`, тот же
  renderer-bundle, что launcher и settings.
- Runtime: отдельный `BrowserWindow` внутри Kepler main process
  (`shell/electron/dashboard-window.ts`).
- Routing: hash-based, окно грузится с `#/dashboard` — `DashboardRoot.vue`
  безусловно рендерит `<DashboardView />`.
- Data access: `window.kepler.ark.request(...)` через main process IPC.
- Shared UI: `@kepler/visuals` (`DesktopChrome`, `DesktopContentSurface`,
  CSS tokens).

## Структура

```
shell/
├─ electron/
│  ├─ dashboard-window.ts        # openDashboardWindow() + window state
│  ├─ main.ts                    # kepler:ark:request handler
│  ├─ preload.ts                 # window.kepler.ark bridge
│  └─ commands.ts                # static `dashboard:open` команда
└─ src/
   ├─ main.ts                    # hash → root view dispatch
   ├─ views/
   │  ├─ DashboardRoot.vue       # wrapper, рендерит DashboardView
   │  └─ DashboardView.vue       # sidebar + main pane
   └─ dashboard/
      ├─ KosmosLogo.vue          # SVG sphere icon
      ├─ SidebarItem.vue         # row для sidebar
      ├─ ObjectTable.vue         # таблица объектов
      ├─ store.ts                # ref'ы + loaders
      └─ types.ts                # DashboardObjectRow, DashboardObjectType
```

## Содержимое

- Sidebar (240px) внутри `<DesktopChrome>` `#sidebar` slot:
  - «Всё» — load all objects (`list_objects`).
  - «Настройки» — placeholder stub.
  - Группа «Типы» — items per `list_object_types`, dot цвет hashed по type id.
- Main pane (`<DesktopContentSurface>` с `border-left`):
  - Header с current selection label (имя типа / «Всё» / «Настройки»).
  - `ObjectTable`: sticky-header table с тремя columns —
    Значение / Тип / Добавлено.
  - Row click → `console.log` (заглушка под будущий object inspector).

## Жёсткие правила

::: danger
- Renderer **никогда** не открывает SQLite напрямую. Все DB reads — через
  `window.kepler.ark.request(...)`.
- Dashboard — **read-only**. Никаких writes в ARK таблицы.
- Когда возможно — `@kepler/ark` operations (`list_object_types`,
  `list_objects_by_type`, `list_objects`). Raw SQL — запрещено.
- Hardcoded `#hex` цвета только для SidebarItem dot'ов (преднамеренно
  избегаем `var(--accent)` чтобы цвета отличались между типами); остальные
  цвета — `var(--*)` из `@kepler/visuals`.
- Окно использует `<DesktopChrome>` + `<DesktopContentSurface>` из
  `@kepler/visuals` — не дублируй own chrome.
- Закрытие dashboard окна **не** закрывает Kepler shell.
:::

## Команды

```powershell
bun run --cwd shell build:js               # tsc + vite renderer + extensions
bun run --cwd shell typecheck              # tsc --noEmit
bun run --cwd shell dev                    # backend + extensions + Kepler renderer
```

В dev mode dashboard грузится с `${VITE_DEV_SERVER_URL}#/dashboard`
(тот же renderer-bundle, что launcher).

## IPC API

| Channel | Direction | Описание |
|---|---|---|
| `kepler:ark:request` | renderer → main | Generic ARK RPC bridge — `arkClient.invokeOperation({ operation, ...params })`. |

Открытие окна:
- Через tray menu → «Dashboard».
- Через static launcher command `dashboard:open` (в `shell/electron/commands.ts`).

## ARK operations, которые Dashboard использует

- `list_object_types` — sidebar «Типы».
- `list_objects` — main pane «Всё».
- `list_objects_by_type` — main pane после клика по типу в sidebar.

Если нужен новый endpoint — добавь в `crates/ark-core/rust` и `packages/ark`,
не пиши raw SQL в shell.

## Связанные документы

- [Kepler](/apps/kepler) — host, который держит Dashboard window.
- [Граница записи в ARK](/concepts/write-boundary)
- [@kepler/visuals](/packages/visuals)
- `docs/ARK-READONLY-SQL-BOUNDARY.md`
