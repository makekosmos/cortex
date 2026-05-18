# Delphi — задачи

::: tip Источник правды
`extensions/delphi/` (Vue-extension) + `docs/DELPHI-LEGACY-DB-DECISION.md`
:::

Delphi — приложение для управления задачами в Kosmos. Десктопный UI **мигрирован в Vue-extension** внутри Kepler shell (`extensions/delphi/`). Android-часть живёт отдельно в `mobile/delphi/`.

## ARK Runtime

- Канонический desktop sidecar — `ark-core-rpc` из `crates/ark-core/rust`.
- **Старый Delphi-specific Rust DB sidecar удалён.** Не пересобирать, не восстанавливать, не упаковывать.
- Задачи Delphi хранятся как обобщённые ARK-объекты с `type_id = task_obj`.
- Extension обращается к ARK через `@kosmos/ark` (через `window.kepler.ark.request(...)` из Kepler shell preload).
- На входе при смене shared-space **legacy todos мигрируются в `task_obj`**. После миграции object-данные — источник правды.

## Текущая модель

- `objects` с `type_id = task_obj` — сами задачи.
- `object_types` — typed/schema metadata.
- `object_links` — связи.
- App access — через `@kosmos/ark` SDK.

См. `docs/DELPHI-LEGACY-DB-DECISION.md`.

## Команды

Сборка происходит через Kepler shell:

```powershell
bun run --cwd shell build:extensions    # билдит все extensions включая Delphi
bun run --cwd shell build:js            # tsc + vite + extensions
bun run --cwd shell dev                 # dev: backend + extensions + Kepler shell renderer
```

В dev mode (HMR) extension поднимается через `bun run --cwd shell dev:extensions` (см. [Extension dev mode](/concepts/extension-dev-mode)).

## Boundaries

- Renderer **только** через `@kosmos/ark` SDK (либо через `window.electronAPI` shim — см. ниже).
- Никаких прямых SQL writes (`@kosmos/ark` → kepler-backend → ark-core-rpc).
- Новые task writes идут в ARK `task_obj`, **не** в legacy todo таблицы.
- Тесты используют изолированные тестовые БД, **никогда** — main user ARK DB.

## Mobile / Native

- `mobile/delphi/` — Android-часть (отдельные правила, см. её `AGENTS.md`).
- Desktop ARK-решения отсюда **не** применяются к Kotlin, если задача явно не говорит обратное.
- `mobile/ark-service/` — Android Room ContentProvider для `mobile/delphi`.

## Запрещено

- ❌ Восстанавливать или паковать legacy Delphi DB sidecar.
- ❌ Использовать legacy todo таблицы как long-term fallback после миграции.
- ❌ Прямой SQL write в `objects` из app services (см. [Граница записи](/concepts/write-boundary)).
- ❌ Дефолт пути к user DB в тестах.

## UX модель задач

Текущая модель форм / списков:

- **Sidebar:** только «Входящие» и «Сегодня» как top-level nav.
- **QuickEntry** (⌘N / Ctrl+N) — модалка. Поля: title, notes, дата (через `<DateChip>` — попап с `<Calendar>` из `@kosmos/visuals`), проект (dropdown + «Входящие»), `billable` toggle + опциональный `price`.
- **TodoRow** — клик разворачивает inline-форму, правый клик открывает `<ContextMenu>` с пунктом «Удалить».
- **ProjectPage** — список задач, бейдж «оплачиваемый + бюджет», суммарное оплачиваемое время по задачам проекта, расчётный `$/час`.

## Биллинг

Реализован минимальный flow «оплачиваемая задача + опциональная цена» + наследование от проекта.

### `task_obj.propsJson`

```ts
{
  billable: boolean,     // источник правды — стор задачи
  price: number | null,  // опциональная сумма за задачу (фикс-цена)
  // ... остальные поля task (description, priority, dates, и т.д.)
}
```

Старые задачи без полей читаются как `billable=false, price=null` (back-compat).

### `Project` (Delphi)

```ts
type Project = {
  // ...
  billable: boolean,     // помечает весь проект как оплачиваемый
  price?: number | null, // общий бюджет проекта (опц.)
}
```

### Наследование

При создании задачи в проекте `billable` авто-подставляется из `project.billable`. Пользователь может переопределить toggle'ом до сохранения. После сохранения значения независимы.

### Распределение по time entries

[Horologion](/apps/horologion) пишет `time_entry_obj` с `propsJson.taskId` и `propsJson.billable`. Delphi читает их через ARK SDK в `ProjectPage`:

- Σ billable секунд по задаче → chip с часами.
- Σ billable секунд по всем задачам проекта → общий часовой счётчик.
- Если у проекта задан `price` — `$/час = price / Σ(billable_hours)`.

## electron-api shim в extension

Delphi портирован в `extensions/delphi/` **как есть** из standalone Electron-апки — все компоненты, сторы и helpers продолжают звать `window.electronAPI.*` (legacy main process IPC). В extension renderer'е этих каналов нет — есть только `window.kepler.ark.request(operation, params)`.

Чтобы не переписывать каждый call-site, существует **compatibility shim** `extensions/delphi/src/lib/electron-api-shim.ts`. Импортируется в `main.ts` как side-effect **до** `createApp(...).mount(...)` и устанавливает `window.electronAPI` поверх `kepler.ark.request`.

### Mapping legacy каналов → ARK operations

| Legacy channel | Ark operation |
|---|---|
| `db:loadAll`, `ark:listDelphiTasks` | `list_objects_by_type` (`task_obj`) → массив `TodoItem` |
| `db:upsertTodo`, `ark:upsertDelphiTask` | `upsert_object` (`task_obj`) |
| `db:deleteTodo`, `ark:deleteDelphiTask` | `delete_object` |
| `db:batchUpsertTodos` | цикл `upsert_object` |
| `ark:listTimeEntries` | `list_objects_by_type` (`time_entry_obj`) |

`TodoItem ↔ ArkObjectRecord` mapping инлайнен прямо в shim — поля `billable` / `price` / `description` / `dates` / `priority` сериализуются в `propsJson`, plain text — в `contentJson`.

### Graceful no-op'ы

Каналы, которых физически нет в extension'е (P2P sync, file system, space management), возвращают пустые значения, чтобы UI graceful показывал offline без crash'а.

- `lan-sync:start` → `false`, `lan-sync:getStatus` → `{ active: false, peers: 0, peerNames: [] }`.
- `sync:getOwnAddresses` → `[]`, `sync:getQrPayload` → `undefined`.
- `space:*` → `undefined` (концепция spaces удалена 2026-05-15 — single DB per user; Delphi shim просто отвечает no-op'ом legacy call-site'ам).
- `db:switchSpace`, `db:deleteSpace`, `db:getSyncKv`, `db:setSyncKv`, `db:clearAll` — `warnOnce()` + no-op.

### Tailwind

Delphi extension сохраняет Tailwind v4 (`@tailwindcss/vite` plugin в `extensions/delphi/vite.config.mjs` + `@import "tailwindcss"` в `src/global.css`). Оригинальный UI Delphi на Tailwind utility classes; переписывание на plain CSS — отдельная задача (см. [Kepler Roadmap → Phase 9](./kepler-roadmap.md#phase-9-delphi-ui-tailwind-plain-css-открытый-вопрос)).

## Command bus integration

Delphi регистрируется в [Kepler command bus](/concepts/command-bus) как provider действий. Юзер из launcher'а (`Ctrl+Shift+K`) может быстро создать задачу или прыгнуть в `Сегодня`.

### Зарегистрированные команды

| ID | Что делает |
|---|---|
| `delphi:task:create` | Открывает `QuickEntry` модалку |
| `delphi:task:today` | `router.push('/today')` — страница сегодняшних задач |

Регистрация — внутри extension'а через `ArkClient.commands.register([...])`:

```ts
await arkClient.commands.register([
  { id: 'delphi:task:create', title: 'Создать задачу', subtitle: 'Delphi', category: 'action' },
  { id: 'delphi:task:today',  title: 'Открыть сегодняшние задачи', subtitle: 'Delphi', category: 'action' },
]);
```

## Связанные документы

- `docs/DELPHI-LEGACY-DB-DECISION.md` — почему legacy sidecar удалён.
- [Command bus](/concepts/command-bus) — протокол dynamic commands.
- [Модель данных ARK](/concepts/ark-objects).
- [Horologion](/apps/horologion) — трекер времени, который привязывается к Delphi-задачам.
- [@kosmos/ark](/packages/ark).
