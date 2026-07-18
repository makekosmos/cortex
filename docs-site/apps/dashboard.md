# Dashboard — встроенный обзор данных и интеграций

::: tip Источник правды
`platform/desktop/src/views/DashboardRoot.vue`, `platform/desktop/src/views/DashboardView.vue`,
`platform/desktop/src/dashboard/`, `platform/desktop/src/integrations/`,
`platform/desktop/src/body/`, `platform/desktop/electron/dashboard-window.ts`,
`platform/runtime/src/integrations.rs`
:::

Dashboard — встроенная часть Kosmos shell (не extension): ARK browser, статистика времени,
настройка импорта Hevy/Toggl Track/LeetCode/Codewars, статистика программирования и
визуализация тренировочных данных в разделе «Тело».

::: info Pivot 2026-05-14 → 2026-05-15
До 2026-05-14 Dashboard жил как Vue extension и показывал usage analytics
(foreground sessions, app ranking и т.п.). По решению пользователя
переосмыслен как ARK browser и встроен в shell. Старый extension больше
не active-tree path.

2026-05-15 — концепция spaces (welcome screen → space picker → space view)
убрана. Dashboard сразу открывается на единый список объектов поверх
одной DB на юзера (`%APPDATA%\Kosmos\ark.db`). См.
[Архитектура — SQLite](/concepts/architecture#sqlite).
:::

## Стек

- Frontend: Vue 3 + Composition API + `<script setup lang="ts">`, тот же
  renderer-bundle, что launcher и settings.
- Runtime: отдельный `BrowserWindow` внутри Kepler main process
  (`platform/desktop/electron/dashboard-window.ts`).
- Routing: hash-based, окно грузится с `#/dashboard` — `DashboardRoot.vue`
  безусловно рендерит `<DashboardView />`.
- Data access: `window.kepler.ark.request(...)` через main process IPC; операции
  `integrations.*` обслуживает `platform/runtime`.
- Shared UI: `@kosmos/visuals` (`DesktopChrome`, `DesktopContentSurface`,
  CSS tokens).

## Структура

```
platform/desktop/
├─ electron/
│  ├─ dashboard-window.ts        # openDashboardWindow() + window state
│  ├─ main.ts                    # kepler:ark:request handler
│  ├─ preload.ts                 # window.kepler.ark bridge
│  └─ commands.ts                # static `dashboard:open` команда (title «Открыть таблицу данных»)
└─ src/
   ├─ main.ts                    # hash → root view dispatch
   ├─ views/
   │  ├─ DashboardRoot.vue       # wrapper, рендерит DashboardView
   │  └─ DashboardView.vue       # общий sidebar + выбор раздела
   ├─ integrations/              # плитки и modal настроек внешних сервисов
   ├─ coder/                     # LeetCode/Codewars статистика из локальных ARK-объектов
   ├─ body/                      # развитие и нагрузка по мышцам
   └─ dashboard/
      ├─ KosmosLogo.vue          # SVG sphere icon
      ├─ SidebarItem.vue         # row для sidebar
      ├─ ObjectTable.vue         # таблица объектов
      ├─ store.ts                # ref'ы + loaders
      └─ types.ts                # DashboardObjectRow, DashboardObjectType
```

## Содержимое

- Sidebar использует общие `SettingsSidebar` + `SidebarButton` из `@kosmos/visuals` и ведёт
  в «Интеграции», «Тело», «Все объекты», «Затреканное время» и ARK-типы.
- «Интеграции» показывает Hevy, Toggl Track, LeetCode и Codewars плитками; настройка
  открывается в общем `Modal`. Ключи, сессия LeetCode и имя профиля Codewars проверяются
  перед сохранением и хранятся только в системном keyring.
- Первый импорт получает всю доступную историю провайдера. Последующие синхронизации читают
  изменения после последней успешной синхронизации и детерминированно обновляют ARK-объекты.
- «Тело» строит развитие мышц и тоннаж за неделю/месяц/год по импортированным Hevy workouts.
- «Кодер» переключает локальную статистику LeetCode/Codewars без повторного сетевого запроса.
  Codewars импортирует публичный профиль и завершённые kata; публичный API не отдаёт
  неудачные попытки и сложность в списке завершений, поэтому UI их не выдумывает.
- Таблицы объектов и usage остаются read-only представлениями ARK.

## Жёсткие правила

::: danger

- Renderer **никогда** не открывает SQLite напрямую. Все DB reads — через
  `window.kepler.ark.request(...)`.
- Renderer не пишет в ARK напрямую. Импорт и локальные настройки выполняет Rust runtime через
  `integrations.*`; ARK writes проходят через `ark_core::db` с sync metadata.
- Когда возможно — `@kosmos/ark` operations (`list_object_types`,
  `list_objects_by_type`, `list_objects`). Raw SQL — запрещено.
- Цвета shell, sidebar и контента — semantic `var(--*)` из `@kosmos/visuals`.
- Окно использует `<DesktopChrome>` + `<DesktopContentSurface>` из
  `@kosmos/visuals` — не дублируй own chrome.
- Закрытие dashboard окна **не** закрывает Kepler shell.
  :::

## Команды

```powershell
bun run --cwd platform/desktop build:js               # tsc + vite renderer + extensions
bun run --cwd platform/desktop typecheck              # tsc --noEmit
bun run --cwd platform/desktop dev                    # backend + extensions + Kepler renderer
```

В dev mode dashboard грузится с `${VITE_DEV_SERVER_URL}#/dashboard`
(тот же renderer-bundle, что launcher).

## IPC API

| Channel              | Direction       | Описание                                                                        |
| -------------------- | --------------- | ------------------------------------------------------------------------------- |
| `kepler:ark:request` | renderer → main | Generic ARK RPC bridge — `arkClient.invokeOperation({ operation, ...params })`. |

Runtime перехватывает namespace `integrations.*`: list/settings/credentials/sync и body snapshot.

Открытие окна:

- Через static launcher command `dashboard:open` («Открыть таблицу данных», `kind: "command"`, `appName: "Kepler"`) в `platform/desktop/electron/commands.ts`. Иконка в launcher — `BuiltInIcon` (teal `Database` glyph).
- Раздел «Тело» также открывается напрямую командой `kosmos:body`.
- Из tray menu Dashboard убран (2026-05-16) — теперь там только «Открыть», «Настройки», «Выход».

## ARK operations, которые Dashboard использует

- `list_object_types` — sidebar «Типы».
- `list_objects` — main pane «Всё».
- `list_objects_by_type` — main pane после клика по типу в sidebar.
- `integrations.list`, `integrations.update_settings` — состояние и расписание провайдеров.
- `integrations.set_credential`, `integrations.clear_credential` — проверка и keyring.
- `integrations.sync_now` — полный первый импорт или incremental sync.
- `integrations.body_snapshot`, `integrations.body_weight_set` — данные раздела «Тело».

Если нужен новый endpoint — добавь в `core/ark/crates/ark-core/rust` и `core/ark/packages/ark`,
не пиши raw SQL в shell.

## Связанные документы

- [Kepler](/apps/kepler) — host, который держит Dashboard window.
- [Граница записи в ARK](/concepts/write-boundary)
- [@kosmos/visuals](/packages/visuals)
- `docs/ARK-READONLY-SQL-BOUNDARY.md`
