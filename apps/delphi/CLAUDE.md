# Delphi

GTD-менеджер задач — часть экосистемы Kosmos. Четыре реализации: macOS (SwiftUI), Web (Electron + Vue), Android (Kotlin + Compose) и Mobile (Expo + React Native).

## Платформы

| Платформа | Путь | Стек |
|-----------|------|------|
| **macOS** | `swift/` | SwiftUI + SwiftData, macOS 14+ |
| **Web/Desktop** | `ts/` | Electron 41 + Vue 3 (Composition API, `<script setup>`) + Vite 8 + Pinia + reka-ui + Tailwind CSS 4 |
| **Android** | `kotlin/` | Kotlin + Jetpack Compose + Material 3 + Room + Hilt, minSdk 28 |
| **Mobile** | `mobile/` | Expo + React Native, TypeScript |

Все версии синхронизируют данные с Ark через WebSocket (`/ws/sync`).

## Навигация по умолчанию

- **Мобильные устройства** (iOS/Android): главный экран при открытии — **Сегодня**
- **Десктоп** (macOS/Electron): главный экран при открытии — **Входящие** (Inbox)

## Модели данных

### TodoItem (задача)
```
id            UUID
title         String
notes         String?
priority      none | low | medium | high
scheduledDate Date?          — когда запланирована
deadline      Date?          — дедлайн
reminderDate  Date?          — напоминание
isToday       Bool           — помечена как "сегодня"
isEvening     Bool           — помечена как "вечер"
isSomeday     Bool           — отложена на "потом"
isCompleted   Bool
completedAt   Date?
isCancelled   Bool
cancelledAt   Date?
isTrashed     Bool
sortOrder     Int            — порядок в списке
headingID     UUID?          — заголовок внутри проекта
createdAt     Date

Связи:
  → Project?                 — проект (опционально)
  → Area?                    — область (опционально)
  → [Tag]                    — теги (many-to-many)
  → [ChecklistItem]          — чеклист (cascade delete)
  → RecurrenceData?          — повторение
```

### Project (проект)
```
id            UUID
title         String
notes         String?
status        active | someday | completed
scheduledDate Date?
deadline      Date?
sortOrder     Int
colorTag      String?
createdAt     Date

Связи:
  → [TodoItem]               — задачи
  → [Heading]                — заголовки-секции (cascade delete)
  → Area?                    — область
```

### Area (область)
```
id            UUID
title         String
sortOrder     Int
createdAt     Date

Связи:
  → [Project]                — проекты
  → [TodoItem]               — задачи вне проектов
```

### Tag
```
id            UUID
title         String
color         String?
createdAt     Date

Связи:
  → [TodoItem]               — many-to-many
```

### Heading (заголовок внутри проекта)
```
id            UUID
title         String
sortOrder     Int

Связи:
  → Project                  — родительский проект
```

### ChecklistItem (элемент чеклиста)
```
id            UUID
title         String
isCompleted   Bool
sortOrder     Int

Связи:
  → TodoItem                 — родительская задача
```

### RecurrenceData (повторение)
```
frequency       daily | weekly | monthly | yearly
interval        Int (каждые N единиц)
recurrenceType  fixed | afterCompletion
daysOfWeek      [Int]?       — для weekly
endDate         Date?
```

## Smart Lists (умные списки)

| Список | Фильтр |
|--------|--------|
| **Входящие** | Нет проекта, не today/evening/someday, не завершена, не в корзине |
| **Сегодня** | isToday=true ИЛИ scheduledDate=сегодня, не завершена |
| **Планы** | scheduledDate в будущем, не today/someday, не завершена |
| **Когда угодно** | Не someday, не завершена, не в корзине, нет scheduledDate в будущем |
| **Потом** | isSomeday=true, не завершена |
| **Журнал** | isCompleted=true ИЛИ isCancelled=true |
| **Корзина** | isTrashed=true |

## Клавиши (macOS)

| Действие | Сочетание |
|----------|-----------|
| Новая задача | Cmd+N |
| Новый проект | Cmd+Option+N |
| Новый заголовок | Cmd+Shift+N |
| Завершить | Cmd+K |
| Отменить | Cmd+Option+K |
| Дублировать | Cmd+D |
| Переместить | Cmd+Shift+M |
| Сегодня | Cmd+T |
| Вечер | Cmd+E |
| Когда угодно | Cmd+R |
| Потом | Cmd+O |
| Дедлайн | Cmd+Shift+D |
| Теги | Cmd+Shift+T |
| Поиск | Cmd+F |
| Сайдбар | Cmd+/ |
| Smart List 1-6 | Cmd+1..6 |

## Архитектура TS-версии (`ts/`)

### Структура

```
ts/
├── sidecar/               — Rust sidecar (delphi-db)
│   ├── Cargo.toml         — rusqlite (bundled), serde_json
│   └── src/main.rs        — stdin/stdout JSON RPC + SQLite (WAL)
├── electron/              — Electron main process
│   ├── main.ts            — точка входа, IPC-хендлеры (db:*, fs:*, peer:*)
│   ├── sidecar.ts         — SidecarClient: spawn delphi-db, JSON queue, dbLoadAll/upsertTodo/…
│   ├── peer-discovery.ts  — mDNS (bonjour-service), _ark-peer._tcp
│   ├── peer-manager.ts    — P2P mesh: outbound WS, HMAC auth, broadcast
│   ├── peer-server.ts     — inbound WS для входящих peer-подключений
│   └── peer-protocol.ts   — типы и протокол P2P-сообщений
├── src/
│   ├── App.vue            — корневой layout, connection bootstrap, P2P bridge
│   ├── main.ts            — createApp, router, Pinia
│   ├── components/        — UI-компоненты (SideBar, QuickEntry, QuickOpen, TodoRow, …)
│   ├── pages/             — route views (TodayPage, AllTaskPage, ProjectPage, …)
│   ├── composables/       — useSmartList, useQuickEntry, useTheme
│   ├── store/
│   │   ├── todos.ts       — Pinia store: задачи, проекты, CRUD → localDb + arkSync
│   │   └── tasks.ts       — вспомогательные утилиты для задач
│   ├── services/
│   │   ├── sync/          — ark-client (WS), hlc (Hybrid Logical Clock), pairing, peer-bridge
│   │   ├── api/           — HTTP helpers
│   │   ├── filters/       — smart list фильтры
│   │   ├── gemini/        — голосовой ввод (Gemini Live API)
│   │   ├── recurrence/    — повторяющиеся задачи
│   │   ├── runtime/       — runtime utilities
│   │   └── storage/       — local-db.ts (sidecar bridge), localStorage wrappers
│   ├── router/            — vue-router конфиг
│   └── types/             — TypeScript типы (Task, Project, Priority, …)
└── vite.config.ts
```

### Rust Sidecar (локальная БД)

`sidecar/` — бинарник `delphi-db` на Rust (паттерн из Eden):
- **Протокол**: stdin/stdout, одна строка = один JSON-запрос/ответ
- **БД**: `<userData>/delphi.db` (SQLite WAL, rusqlite bundled)
- **Таблицы**: `todos`, `projects`, `areas`, `tags`, `headings`, `sync_kv`
- **IPC**: Electron main → `electron/sidecar.ts` → `SidecarClient` → spawn процесс

**Операции**: `init` (открыть БД), `load_all`, `upsert_todo`, `delete_todo`, `batch_upsert_todos`, `upsert_project`, `delete_project`, `upsert_area`, `upsert_tag`, `upsert_heading`, `delete_heading`, `get_sync_kv`, `set_sync_kv`, `clear_all`

**Поток данных при старте (Electron)**:
```
App.vue → isLocalDbAvailable()
  true  → loadAllFromLocalDb() → IPC db:loadAll → sidecar → SQLite  (мгновенно, offline)
  false → fetchTasksFromArk()  → HTTP /events                       (web режим, fallback)
```

**Каждая мутация (store/todos.ts)**:
```
addTodo/updateTodo/… → localDbUpsertTodo (fire & forget)
                     → arkSync.sendChange (WebSocket relay)
```

**Сборка**:
```bash
bun run build:sidecar:dev   # cargo build (debug)
bun run build:sidecar       # cargo build --release
bun run dev                 # build sidecar:dev + vite
```

### UI-библиотека

reka-ui (headless Vue 3 components): Tooltip, Dialog и т.д. Стили — Tailwind CSS 4 с CSS-переменными (`--background`, `--foreground`, `--border`, `--popover`, `--muted-foreground`).

### Состояние подключения (App.vue)

Индикатор-точка в правом верхнем углу:
- **Зелёный** (`online`) — WebSocket с Ark активен, realtime sync работает
- **Жёлтый пульсирующий** (`syncing`) — устанавливается соединение
- **Красный** (`offline`) — соединение потеряно или не настроено

При наведении — тултип (reka-ui Tooltip) с описанием текущего состояния.

### Синхронизация (renderer)

`ArkSyncClient` (`services/sync/ark-client.ts`):
- WebSocket с version vectors (localStorage)
- Outbox для offline-изменений
- Heartbeat ping/pong
- Reconnect с exponential backoff (max 30s)
- `onStatus(cb)` / `onChange(cb)` для подписки

### P2P mesh (Electron main process)

- mDNS discovery → peer-manager координирует outbound WS
- HMAC auth, hop_path для предотвращения петель
- IPC bridge `peer:change` → renderer обрабатывает как Ark changes

## Сервисы

### TodoFilterService
Централизованная фильтрация по smart lists. Кеширует счётчики.

### LemmaSearchService
Морфологический поиск через NLTagger. Русские склонения, английские формы.

### NaturalDateParser
Парсинг дат из текста:
- "сегодня", "завтра", "послезавтра"
- "3d", "2w", "3mo", "1y"
- "in 5 days", "next monday"
- Русские дни недели, относительные фразы

## Синхронизация с Ark

Все клиенты (Swift, Android Kotlin, Web/TS) синхронизируются с Ark **только через WebSocket**. REST `/tasks` не существует.

### Подключение

```
ws://ark-server/ws/sync?key=API_KEY
```

После открытия WebSocket клиент сразу шлёт `sync_start`:
```json
{
  "type": "sync_start",
  "device_id": "delphi-web-<uuid>",
  "device_name": "Delphi Web",
  "platform": "web",
  "vector": {"delphi-web-abc": 42, "delphi-android-xyz": 15}
}
```

Сервер отвечает пачкой пропущенных изменений:
```json
{"type": "sync_changes", "changes": [...]}
```

### Realtime изменения

Каждая мутация отправляется немедленно:
```json
{
  "type": "change",
  "event_id": "<todo-uuid>",
  "change_type": "create" | "update" | "delete",
  "data": {
    "event_type": "task",
    "category": "productivity",
    "source": "delphi-web",
    "source_id": "<todo-uuid>",
    "summary": "Заголовок задачи",
    "occurred_at": "2025-01-01T12:00:00Z",
    "data": { /* все поля TodoItem */ }
  }
}
```

Сервер бродкастит изменение всем другим подключённым клиентам.

### Маппинг event_type

| Модель | event_type | source |
|--------|-----------|--------|
| TodoItem | `"task"` | `"delphi-web"` / `"delphi"` / `"delphi-android"` |
| Project | `"project"` | `"delphi-web"` |
| Area | `"area"` | `"delphi-web"` |
| Tag | `"tag"` | `"delphi-web"` |

Дедупликация: `source_id` = UUID объекта (идемпотентный upsert по `source_id` alone, без `source`).

### Offline

Изменения, сделанные без соединения, попадают в outbox (localStorage).
После reconnect — flushOutbox отправляет их в порядке очереди.

### UUID normalization

Все `source_id`/`event_id` MUST be lowercase на всех платформах. Mac UUID по умолчанию uppercase.
- **Swift**: `.lowercased()` при генерации/отправке
- **Kotlin**: `.lowercase()` при получении
- **TS/Electron**: `.toLowerCase()` при получении

### server_epoch

Сервер включает `server_epoch` (UUID) в каждый `sync_changes`. Клиенты хранят его локально. Когда epoch меняется → БД сервера была стёрта → сбросить version vector, запушить все локальные данные, НЕ удалять локальные данные.

### is_full_sync

Сервер включает `is_full_sync` boolean в `sync_changes`. `pushLocalTodos`/`sendMissingToServer` выполняется только когда `is_full_sync=true` ИЛИ `epochChanged=true` — НЕ при каждом reconnect.

### Zombie cleanup

Удаление локальных задач, отсутствующих на сервере (исключая outbox). Запускается ТОЛЬКО когда `is_full_sync=true AND epochChanged=false`.

### "Clear local data" (dev)

Временная кнопка в настройках на всех платформах. Очищает локальную БД + sync state (vector, epoch, outbox), затем переподключается для свежего full sync.
- **Swift**: `ArkSyncClient.clearLocalData()` — per-object deletion (не batch) из-за ограничений SwiftData relationships
- **Kotlin**: `ArkSyncClient.clearLocalData()` — использует `deleteAll()` DAO методы
- **TS/Electron**: `handleClearLocalData()` в App.vue — очищает localStorage + sidecar через `dbClearAll()`

### Sidecar: clear_all

Добавлена операция `clear_all` в Rust sidecar delphi-db — удаляет все строки из таблиц `todos`, `projects`, `areas`, `tags`, `headings`, `sync_kv`.

### Где живёт код

| Файл | Роль |
|------|------|
| `services/sync/ark-client.ts` | `ArkSyncClient` (WS), маппинг TodoItem↔ArkChange |
| `store/todos.ts` | CRUD + вызов `arkSync.sendChange()` на каждой мутации |
| `App.vue` | `arkSync.onChange()` → `store.upsertTodo()` / `store.upsertProject()` |

### Входящие изменения (App.vue)

```
arkSync.onChange(change) →
  arkChangeEventType(change) === "task"    → arkChangeToTodoItem → store.upsertTodo
  arkChangeEventType(change) === "project" → arkChangeToProject  → store.upsertProject
```

### Исходящие изменения (store/todos.ts)

```
addTodo()      → todoItemToArkChange(todo, "create")  → arkSync.sendChange()
updateTodo()   → todoItemToArkChange(todo, "update")  → arkSync.sendChange()
removeTodo()   → todoItemToArkChange(todo, "delete")  → arkSync.sendChange()
completeTodo() → todoItemToArkChange(todo, "update")  → arkSync.sendChange()
(и т.д. для cancel, trash, restore, duplicate)
```

## Голосовой ввод (Web)

Google Gemini 2.5 Live API:
- Микрофон → PCM 16kHz → Gemini
- Function calling: create_task, update_last_task, delete_last_task
- Язык: русский (приоритет)

## Conventions

- Язык UI: русский
- Package manager: bun (workspace)
- Шрифт: Zed Mono Extended (web), системный (macOS)
- Тема: тёмная по умолчанию, поддержка светлой
- Path alias: `@/` → `src/`
- Vue: Composition API + `<script setup lang="ts">`, без Options API
- State: Pinia stores (`defineStore`), `shallowRef` для примитивов
- UI-компоненты: reka-ui (headless) + Tailwind CSS 4
- Линтер: oxlint, форматтер: prettier
- Тесты: vitest (unit), playwright (e2e)
