# Dashboard — встроенный ARK browser

::: tip Источник правды
`shell/src/views/Dashboard*.vue`, `shell/src/dashboard/`,
`shell/electron/dashboard-window.ts`
:::

Dashboard — встроенная часть Kepler shell'а (не extension). Это entry point в
данные ARK: пользователь видит карточки spaces (welcome screen), кликает
карточку и попадает в space view — sidebar с object_types + таблица объектов.

::: info Pivot 2026-05-14
До 2026-05-14 Dashboard жил как Vue extension и показывал usage analytics
(foreground sessions, app ranking и т.п.). По решению пользователя Dashboard
был переосмыслен как ARK browser и встроен в shell. Старый extension
заморожен в `legacy/dashboard-extension/`.
:::

## Стек

- Frontend: Vue 3 + Composition API + `<script setup lang="ts">`, тот же
  renderer-bundle, что launcher и settings.
- Runtime: отдельный `BrowserWindow` внутри Kepler main process
  (`shell/electron/dashboard-window.ts`).
- Routing: hash-based, `#/dashboard/welcome` / `#/dashboard/space/<spaceId>`.
- Data access: `window.kepler.ark.request(...)` + `window.kepler.spaces.list()`
  через main process IPC.
- Shared UI: `@kepler/visuals` (CSS tokens).

## Структура

```
shell/
├─ electron/
│  ├─ dashboard-window.ts        # openDashboardWindow() + window state
│  ├─ main.ts                    # kepler:spaces:list / kepler:ark:request handlers
│  ├─ preload.ts                 # window.kepler.{spaces,ark} bridges
│  └─ commands.ts                # static `dashboard:open` команда
└─ src/
   ├─ main.ts                    # hash → root view dispatch
   ├─ views/
   │  ├─ DashboardRoot.vue       # hash router (welcome / space)
   │  ├─ DashboardWelcomeView.vue
   │  └─ DashboardSpaceView.vue
   └─ dashboard/
      ├─ KosmosLogo.vue          # SVG sphere icon
      ├─ SpaceCard.vue           # iCloud-style карточка space'а
      ├─ SidebarItem.vue         # row для sidebar
      ├─ ObjectTable.vue         # таблица объектов
      ├─ store.ts                # ref'ы + loaders
      └─ types.ts                # SpaceMeta, DashboardObjectRow, ...
```

## Welcome view

- Sphere logo + «Kosmos» текст по центру сверху.
- Space cards (480×240): header (имя space), counter, footer с relative time +
  green sync pill + orange label pill.
- Page footer: copyright + tagline.
- Click карточки → `window.location.hash = "#/dashboard/space/<id>"`.

## Space view

- Grid 240px sidebar + main pane.
- Sidebar:
  - «Всё» — load all objects (`list_objects`).
  - «Настройки» — placeholder stub.
  - Группа «Типы» — items per `list_object_types`, dot цвет hashed по type id.
- Main pane:
  - Header с current selection label.
  - `ObjectTable`: sticky-header table с пятью columns —
    Значение / Тип / Добавлено / Данные X / Данные Y.
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
- Hash routing внутри dashboard window — без `vue-router`. Один listener на
  `hashchange` в `DashboardRoot.vue`.
- Закрытие dashboard окна **не** закрывает Kepler shell.
:::

## Команды

```powershell
bun run --cwd shell build:js               # tsc + vite renderer + extensions
bun run --cwd shell typecheck              # tsc --noEmit
bun run --cwd shell dev                    # backend + extensions + Kepler renderer
```

В dev mode dashboard грузится с `${VITE_DEV_SERVER_URL}#/dashboard/welcome`
(тот же renderer-bundle, что launcher).

## IPC API

| Channel | Direction | Описание |
|---|---|---|
| `kepler:spaces:list` | renderer → main | Возвращает `SpaceMeta[]` (scan `%APPDATA%/Kosmos/spaces/<id>/`). |
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
