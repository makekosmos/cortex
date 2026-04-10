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

### UDP Beacon discovery (Syncthing-style)

Primary discovery — UDP broadcast на порт `LAN_SYNC_PORT + 1` (21532). Каждый пир каждые 5 сек шлёт beacon `{t, s=space_id, d=device_id, n=device_name, p=ws_port, a=[routable_addresses]}` во все широковещательные адреса IPv4-подсетей. mDNS не используется — блокируется AP isolation на многих роутерах.

**Дедупликация входящих beacon'ов — обязательна.** Receiver держит `Map<device_id, SeenPeer>` и вызывает `onPeerDiscovered` только когда (а) device_id новый, или (б) список адресов изменился. TTL 30 с (2× beacon interval) — stale-записи вычищаются, чтобы peer мог переанонсироваться. **Без дедупа** каждый beacon (каждые 5 с) триггерил reconnect → бесконечный спам `[SyncClient] All addresses failed`.

### Фильтрация адресов (Syncthing-style)

Beacon'ы и `ownAddresses` (в `hello`/`peer_list`) **MUST** содержать только маршрутизируемые адреса. Фильтры:

- loopback (`internal=true`, `127.0.0.0/8`, `::1`)
- IPv4 link-local `169.254.0.0/16`
- IPv6 link-local `fe80::/10` и unique-local `fc00::/7`
- виртуальные интерфейсы по префиксу имени: `utun*`, `awdl*`, `llw*`, `bridge*`, `anpi*`, `docker*`, `br-*`, `veth*`, `virbr*`, `vboxnet*`, `vmnet*`, `tun*`, `tap*`, `wg*`, `tailscale*`, `vEthernet*`, `VMware*`, `VirtualBox*`, `rmnet*`, `dummy*`

Реализация: `packages/ark-core/rust/src/beacon.rs` (Rust beacon), `kotlin/.../BroadcastDiscovery.kt` (`sendBeacon`). **Если добавляешь новый способ анонсирования адресов — фильтруй там же.**

### Device name = host name, не process name

Все платформы анонсируют **реальное имя устройства ОС**, а не имя приложения:

| Платформа | Источник | Пример |
|-----------|----------|--------|
| **Electron** | `os.hostname()` с trim `.local` | `Kirill-MacBook-Pro-437` |
| **Android** | `${Build.MANUFACTURER} ${Build.MODEL}` | `Nothing A063` |
| **macOS** | `Host.current().localizedName` | `Kirill's MacBook Pro` |

Хелпер в Electron: `getHostDeviceName()` в `ts/electron/main.ts`. Renderer-процесс передаёт пустую строку в `lan-sync:start` / `peer:setMeshCredentials`, main-процесс всегда подставляет host name. **Никогда** не захардкоживай `"Delphi Electron"` или имя процесса.

### Single-session-per-device на SyncServer

`SyncServer.peers` внутри хранит сессии ключом по WS-соединению, но **внешне видимо** должно быть **одно устройство = одна запись**:

1. В `handleHello`: после аутентификации новой сессии — закрыть все прочие authenticated-сессии с тем же `device_id` (`CloseReason.NORMAL`, reason `superseded`). Предварительно снять флаг `authenticated` на stale-сессиях, чтобы их `close` handler не дёрнул лишний `onPeerDisconnected`.
2. `getConnectedPeers()` **MUST** дедупить по `device_id` (`LinkedHashMap<device_id, name>`) — защита на случай гонки между handshake и eviction.
3. В координаторе (`PeerManager.updatePeerCounts`): мерж inbound-сессий (`SyncServer`) и outbound-клиента (`LanSyncClient`) дедупится по `device_id`. Для этого `LanSyncClient.ServerInfo` хранит `deviceId` сервера. Физическое устройство, подключённое в обе стороны, = одна запись.

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
| `packages/ark-core/rust/src/sync_server.rs` | Rust WS-сервер (все платформы через UniFFI / sidecar) |
| `packages/ark-core/rust/src/sync_client.rs` | Rust WS-клиент с address racing |
| `packages/ark-core/rust/src/beacon.rs` | UDP Beacon discovery (порт 21532) |
| `packages/ark-core/rust/src/relay_transport.rs` | Outbound relay WebSocket клиент (backoff, offline outbox) |
| `packages/ark-core/rust/src/mesh.rs` | MeshCoordinator: LAN + relay, дедупликация изменений |
| `packages/ark-core/rust/src/ffi.rs` | UniFFI facade (ArkCore, FfiSyncConfig, ArkEventListener) |
| `packages/arksync-node/src/ark-client.ts` | `@arksync/node` ArkClient — TypeScript обёртка над sidecar IPC |
| `ts/electron/main.ts` | Electron main: ArkClient из @arksync/node, stale-sync reset, macOS application menu |
| `ts/electron/sidecar.ts` | SidecarClient: только DB ops |
| `ts/src/services/sync/lan-protocol.ts` | Общие типы, HLC, diff, batch splitting (standalone) |
| `ts/src/store/todos.ts` | CRUD + `broadcastToLanSync()` на каждой мутации |
| `kotlin/.../data/sync/PeerManager.kt` | Android координатор: UniFFI `ArkCore.startSync()` |

### Важные правила реализации

- Vue 3 reactive proxies **MUST** быть deep-cloned через `JSON.parse(JSON.stringify())` перед Electron IPC (structured clone не может сериализовать Proxy-объекты)
- Electron main **MUST** сбрасывать локальное sync-state (`syncActive`, peer cache, текущий runtime), если sidecar возвращает `Sync not running`; иначе UI продолжит слать `broadcast_change` в мёртвый runtime и спамить warnings
- macOS/Electron: **НЕ** ставить `Menu.setApplicationMenu(null)` в Delphi; используй нормальный application menu template, иначе возможен Cocoa warning `representedObject is not a WeakPtrToElectronMenuModelAsNSObject`
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
> - `swift/AGENTS.md` — macOS SwiftUI
> - `kotlin/AGENTS.md` — Android Kotlin
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
│   └── sidecar.ts         — SidecarClient: spawn delphi-db, JSON queue, dbLoadAll/upsertTodo/…
├── src/
│   ├── App.vue            — корневой layout, connection bootstrap, P2P sync bridge
│   ├── main.ts            — createApp, router, Pinia
│   ├── components/        — UI-компоненты (SideBar, QuickEntry, QuickOpen, TodoRow, …)
│   ├── pages/             — route views (TodayPage, AllTaskPage, WeekPage, ProjectPage, …)
│   ├── composables/       — useSmartList, useQuickEntry, useTheme, useSidebarState
│   ├── store/
│   │   ├── todos.ts       — Pinia store: задачи, проекты, CRUD → localDb + lanSync
│   │   └── tasks.ts       — вспомогательные утилиты для задач
│   ├── services/
│   │   ├── sync/          — lan-protocol, hlc, ark-types, peer-bridge
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

### Sidebar zen-mode width

`src/composables/useSidebarState.ts` хранит module-level `sidebarHidden` и отдаёт `wrapClass`/`wrapStyle` для страниц контента. **Smart-list страницы, ProjectPage и SettingsPage MUST использовать этот composable** для общего контейнера контента, чтобы при скрытии сайдбара layout плавно переключался между `100%` и `var(--bringhurst-wide)` вместо резкого reflow.

### Состояние подключения (App.vue)

Индикатор-точка в правом верхнем углу:
- **Зелёный** (`online`) — LAN sync активен, live mode работает
- **Жёлтый пульсирующий** (`syncing`) — устанавливается соединение / batch sync
- **Красный** (`offline`) — нет подключённых клиентов или пространство не создано

При наведении — тултип (reka-ui Tooltip) с описанием текущего состояния.

### P2P Sync (Electron main process)

Electron использует `@arksync/node` → `ArkClient` → IPC к Rust sidecar `ark-core-rpc`. TS-уровень не содержит WebSocket-кода — весь P2P в Rust:

- **`packages/arksync-node/src/ark-client.ts`** — `ArkClient`: `start()`, `stop()`, `broadcastChange()`, `onPeerConnected`, `onEntityChanged`
- **Sidecar IPC** через `electron/main.ts` → `lan-sync:start`, `lan-sync:change`, `lan-sync:broadcast`
- **Version vector**: персистируется в sidecar `sync_kv` (SQLite)
- **Incoming changes**: IPC `lan-sync:change` → renderer → Pinia store

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
| `packages/arksync-node/src/ark-client.ts` | `@arksync/node` ArkClient — TypeScript обёртка над sidecar IPC |
| `electron/main.ts` | ArkClient из @arksync/node, IPC-хендлеры |
| `electron/sidecar.ts` | SidecarClient: только DB ops |
| `src/services/sync/ark-types.ts` | Типы ArkChange, маппинг сущностей, settings helpers |
| `src/services/sync/lan-protocol.ts` | Общие типы, HLC, diff, batch splitting |
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

### Relay транспорт (@arksync/node / Rust)

Relay WebSocket транспорт реализован в `packages/ark-core/rust/src/relay_transport.rs` и координируется через `mesh.rs`. `@arksync/node` ArkClient принимает опциональные `relayUrl` и `relayApiKey` — без них работает только LAN.

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
- Линтер: oxlint, форматтер: oxfmt (с sortImports)
- Тесты: vitest (unit), playwright (e2e)
