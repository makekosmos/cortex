# Delphi

GTD-менеджер задач — часть экосистемы Kosmos. Три реализации: macOS (SwiftUI), Web/Desktop (Electron + Vue), Android (Kotlin + Compose).

## Платформы

| Платформа | Путь | Стек |
|-----------|------|------|
| **macOS** | `swift/` | SwiftUI + SwiftData, macOS 15+ |
| **Web/Desktop** | `ts/` | Electron + Vue 3 + Vite + Pinia + reka-ui + Tailwind CSS 4 |
| **Android** | `kotlin/` | Kotlin + Jetpack Compose + Material 3 + Room + Hilt, minSdk 28 |

Синхронизация: **P2P mesh (equal peers)** — все платформы запускают WS-сервер И клиент. Нет хаба.

## P2P Sync — равноправная синхронизация

### Архитектура

Все платформы (Electron, Android, macOS) — **равноправные пиры**. Каждый запускает WebSocket-сервер И подключается как клиент к другим пирам. Нет выделенного хаба.

| Платформа | WS-сервер | WS-клиент | Порт |
|-----------|-----------|-----------|------|
| **Electron** | `ws` library | `ws` library | 21531 |
| **Android** | Ktor CIO embedded | OkHttp | 21531 (fallback 21531-21541) |
| **macOS** | NWListener | URLSession | 21531 |

### Space Code

12-символьный Base32-Crockford код (формат `XXXX-XXXX-XXXX`), генерируемый случайно. Не кодирует IP.

- **Любая платформа** может создать пространство → генерирует случайный код
- **HMAC secret** = `normalizeCode(code)` (uppercase, без тире) — для аутентификации пиров
- **Space ID** = `SHA-256(normalized_code)[:16 hex]` = per-space DB identifier
- **Один активный Space** на устройство
- **Без пространства** → показывается экран настройки (SpaceSetupScreen/SpaceSetupView)

### QR-payload

```
ark://join?code=XXXX-XXXX-XXXX&addrs=192.168.1.70:21531,10.0.0.5:21531
```

Содержит Space code + все известные адреса создающего пира (LAN, WAN, IPv6).

### Multi-address peer records

Каждый пир хранит список адресов (LAN, WAN, IPv6) для каждого известного пира. При подключении пробует все адреса **параллельно**, берёт первый успешный.

### Peer list exchange (mesh discovery)

После `hello` пиры обмениваются списками известных пиров с их адресами. Это позволяет обнаруживать пиры транзитивно без mDNS.

### Протокол синхронизации

1. **hello** — клиент отправляет при подключении, сервер отвечает `hello_ack`
2. **peer_list** — обмен известными пирами и их адресами для mesh discovery
3. **version_vector** — обмен version vectors, вычисление diff
4. **batch sync** — пачки до 100 изменений, каждая с ACK (`batch_ack`)
5. **live mode** — после завершения sync, мутации идут как `live_change` с `live_ack`

- **Конфликт-резолюция**: HLC-based Last-Writer-Wins (Hybrid Logical Clock)
- **Version vector**: персистится в `sync_kv` (Electron/sidecar), DataStore (Android), UserDefaults (macOS)
- **Сущности**: todo, project, area, tag, heading

### Ключевые файлы

| Файл | Роль |
|------|------|
| `ts/electron/sync-server.ts` | Electron WS-сервер |
| `ts/electron/sync-client.ts` | Electron WS-клиент |
| `ts/src/services/sync/lan-protocol.ts` | Общие типы, HLC, diff, batch splitting |
| `ts/src/store/todos.ts` | `broadcastToLanSync()` на каждой мутации |
| `kotlin/.../data/sync/SyncServer.kt` | Android Ktor WS-сервер |
| `kotlin/.../data/sync/LanSyncClient.kt` | Android WS-клиент |
| `kotlin/.../data/sync/PeerManager.kt` | Android координатор пиров |
| `swift/Delphi/Sync/SyncServer.swift` | macOS NWListener WS-сервер |
| `swift/Delphi/Sync/SyncClient.swift` | macOS URLSession WS-клиент |
| `swift/Delphi/Sync/PeerManager.swift` | macOS координатор пиров |

### Важные правила реализации

- Vue 3 reactive proxies **MUST** быть deep-cloned через `JSON.parse(JSON.stringify())` перед Electron IPC (structured clone не может сериализовать Proxy-объекты)
- Android version vector **MUST** персиститься в DataStore, **НЕ** регенерироваться с `Instant.now()` при reconnect
- OkHttp WebSocket: без `pingInterval` (сервер шлёт WS-level pings), `readTimeout=0`
- **Нет кнопки "Очистить данные"** — данные удаляются только через системные настройки (Settings → Apps)

### Поведение синхронизации

- **Initial sync**: version vectors обмениваются, diff вычисляется, батчи отправляются с ACK
- **Live mode**: мутации транслируются как `live_change` с `live_ack`
- **Reconnection**: автоматический с exponential backoff
- **Persistence**: данные сохраняются между reconnect — space хранит всех пиров и задачи

### Файлы по платформам (Space UI)

| Платформа | SpaceManager | SpaceSetupUI |
|-----------|-------------|--------------|
| **TS/Electron** | `src/services/space/space-manager.ts` | `src/components/SpaceSetup.vue` |
| **Swift** | `Delphi/Space/SpaceManager.swift` | `Delphi/Space/SpaceSetupView.swift` |
| **Kotlin** | `data/space/SpaceManager.kt` | `ui/screens/space/SpaceSetupScreen.kt` |

> Подробная документация по каждой платформе:
> - `swift/CLAUDE.md` — macOS SwiftUI
> - `kotlin/CLAUDE.md` — Android Kotlin
> Архитектура TS/Electron описана ниже.

## Навигация по умолчанию

- **Android**: главный экран — **Сегодня**
- **macOS/Electron**: главный экран — **Входящие** (Inbox)

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
│   ├── main.ts            — точка входа, IPC-хендлеры (db:*, fs:*, lan-sync:*)
│   ├── sidecar.ts         — SidecarClient: spawn delphi-db, JSON queue, dbLoadAll/upsertTodo/…
│   ├── sync-server.ts     — WS-сервер (порт 21531), sync protocol
│   ├── sync-client.ts     — WS-клиент, подключение к другим пирам
│   ├── peer-manager.ts    — координатор пиров, mesh discovery
│   ├── peer-discovery.ts  — mDNS (bonjour-service), _ark-peer._tcp (legacy)
│   └── peer-protocol.ts   — типы протокола P2P-сообщений
├── src/
│   ├── App.vue            — корневой layout, connection bootstrap, P2P sync bridge
│   ├── main.ts            — createApp, router, Pinia
│   ├── components/        — UI-компоненты (SideBar, QuickEntry, QuickOpen, TodoRow, …)
│   ├── pages/             — route views (TodayPage, AllTaskPage, ProjectPage, …)
│   ├── composables/       — useSmartList, useQuickEntry, useTheme
│   ├── store/
│   │   ├── todos.ts       — Pinia store: задачи, проекты, CRUD → localDb + lanSync
│   │   └── tasks.ts       — вспомогательные утилиты для задач
│   ├── services/
│   │   ├── sync/          — lan-protocol, hlc, ark-client (legacy), peer-bridge
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
                     → broadcastToLanSync() (live_change to connected peers)
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
- **Зелёный** (`online`) — LAN sync активен, live mode работает
- **Жёлтый пульсирующий** (`syncing`) — устанавливается соединение / batch sync
- **Красный** (`offline`) — нет подключённых клиентов или пространство не создано

При наведении — тултип (reka-ui Tooltip) с описанием текущего состояния.

### P2P Sync (Electron main process)

`sync-server.ts` — WebSocket-сервер на порту 21531:
- Принимает подключения от Android/macOS пиров
- Протокол: hello → peer_list → version_vector → batch sync (max 100/batch) → ACK → live mode
- HLC-based LWW конфликт-резолюция
- Version vector персистится в sidecar (`sync_kv`)
- IPC bridge `lan-sync:change` → renderer обрабатывает входящие изменения

`sync-client.ts` — WebSocket-клиент:
- Подключается к другим пирам по известным адресам
- Пробует все адреса пира параллельно (LAN, WAN, IPv6)

### Legacy: ArkSyncClient

Код остаётся, но не используется:
- `ark-client.ts` — WS relay через Ark-сервер

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

## Синхронизация

### P2P Sync (основной режим)

Описан выше. При первом запуске без пространства → показать SpaceSetupScreen/View.

### Где живёт код (TS/Electron)

| Файл | Роль |
|------|------|
| `electron/sync-server.ts` | WS-сервер (порт 21531), sync protocol |
| `electron/sync-client.ts` | WS-клиент, подключение к другим пирам |
| `electron/peer-manager.ts` | Координатор пиров, mesh discovery |
| `src/services/sync/lan-protocol.ts` | Типы, HLC, diff, batch splitting |
| `store/todos.ts` | CRUD + `broadcastToLanSync()` на каждой мутации |
| `App.vue` | Bootstrap sync, обработка входящих изменений |

### Исходящие изменения (store/todos.ts)

Каждая мутация вызывает `broadcastToLanSync()` — отправляет `live_change` всем подключённым пирам.

### UUID normalization

Все UUID MUST be lowercase на всех платформах. Mac UUID по умолчанию uppercase.
- **Swift**: `uuidString.lowercased()`
- **Kotlin**: `.lowercase()`
- **TS/Electron**: `.toLowerCase()`

### Sidecar: clear_all

Операция `clear_all` в Rust sidecar delphi-db — удаляет все строки из таблиц `todos`, `projects`, `areas`, `tags`, `headings`, `sync_kv`.

### Ark WebSocket relay (legacy/отключён)

Старая синхронизация через Ark-сервер по WebSocket (`ws://ark-server/ws/sync?key=API_KEY`). Код остаётся в `services/sync/ark-client.ts`, но не используется при активном LAN sync.

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
