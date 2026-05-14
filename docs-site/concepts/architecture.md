# Архитектура

## Brand'ы

- **Kosmos** — название экосистемы. Под Kosmos живут продуктовые приложения (Eden, Delphi, Arrancador, Horologion, Dashboard) и shared-пакеты (`@kosmos/ark`, `kosmos-visuals`).
- **Kepler** — Electron-launcher + shared Rust runtime (`kepler-backend`), который держит один `ark-core-rpc` child на машину, gateway-WS для апок, command bus и LAN/relay sync.

Раньше "Kepler" обозначал отдельный Rust+gpui фоновый sync host. Сейчас это имя перешло на Electron launcher (`apps/kepler-shell`) + его Rust backend (`services/kepler-backend`). Шилд старой архитектуры — снят.

## Высокоуровневая картина

```text
┌──────────────────────────────────────────────────────────────┐
│  kepler.exe  (Electron host, apps/kepler-shell)              │
│    ├─ Launcher BrowserWindow (Ctrl+Shift+K, FTS5 search)     │
│    ├─ Tray icon                                              │
│    ├─ Settings window                                        │
│    ├─ Extension loader (apps/kepler-shell/extensions/<id>/)  │
│    └─ spawn: kepler-backend.exe                              │
└──────────────────────────────────────────────────────────────┘
                            │
                            │ child process
                            ▼
┌──────────────────────────────────────────────────────────────┐
│  kepler-backend.exe  (Rust, services/kepler-backend)         │
│    ├─ spawn: ark-core-rpc.exe (один на машину)               │
│    ├─ WS server 127.0.0.1:<random_port>                      │
│    ├─ Command bus (registry + invoke broadcast)              │
│    ├─ Auth (bearer token, PID-binding, file ACL)             │
│    ├─ Singleton (rusqlite WAL BEGIN IMMEDIATE)               │
│    ├─ Lock-file: %APPDATA%\Kosmos\kepler.lock.json           │
│    └─ start_sync (LAN + relay) при старте                    │
└──────────────────────────────────────────────────────────────┘
              │ WS ws://127.0.0.1:<port>
   ┌──────────┼────────────┬────────────┬────────────┐
┌──┴──┐ ┌─────┴────┐ ┌─────┴────┐ ┌─────┴───┐ ┌──────┴─────┐
│Eden │ │ Delphi   │ │Arrancador│ │Horologion│ │ Dashboard  │
└─────┘ └──────────┘ └──────────┘ └──────────┘ └────────────┘

каждая апка — standalone .exe Electron, коннектится к kepler-backend
по WS через @kosmos/ark
```

- `kepler-shell` (Electron) — оркестратор: launcher окно, tray, settings, extension loader, спавн `kepler-backend`.
- `kepler-backend` (Rust) — shared runtime: supervisor для `ark-core-rpc`, WS gateway, command bus, sync.
- `ark-core-rpc` (Rust) — канонический ARK runtime: SQLite, миграции, sync protocol.
- Apps — standalone Electron-апки. Renderer общается с app main по preload IPC, app main — с `kepler-backend` по WS через `@kosmos/ark`.

## Роли

### Kepler launcher (kepler-shell)

`apps/kepler-shell/electron/main.ts` — Electron host:

- Спавнит `kepler-backend.exe` как child (`spawnBackend`).
- Подключается к нему через `@kosmos/ark` в kepler-mode (`ensureKeplerRunning`).
- Показывает launcher окно (frameless, acrylic background, alwaysOnTop) по `Ctrl+Shift+K`.
- Tray-иконка с menu (Открыть / Настройки / Выход).
- Список команд в launcher = **static open-commands** (`COMMANDS` из `electron/commands.ts`) + **dynamic action-commands** (через `arkClient.commands.list()` — приходят от running апок).
- При invoke: static исполняются локально (`spawn(exe)`), dynamic уходят в `kepler-backend` через `arkClient.commands.invoke(id)` — backend broadcast'ит `command_invoked`, owning апка handle'ит.
- Extension loader (`electron/extension-host.ts`) — open Vue extension bundles в отдельные BrowserWindow (Phase 4 foundation, PoC — Dashboard).

### kepler-backend (Rust)

`services/kepler-backend/`:

| Файл | Что делает |
|---|---|
| `main.rs` | bootstrap: singleton, lock-file, ark-core-rpc supervisor, WS server, start_sync |
| `ark_host.rs` | spawn + watchdog `ark-core-rpc`, proxy stdio JSON ↔ WS |
| `ws_server.rs` | WS accept loop, auth handshake, dispatch operations (intercept `commands.*`) |
| `command_bus.rs` | registry per WS-connection, broadcast invoke/changed events |
| `sync.rs` | вызов `start_sync` на ark-core-rpc после health |
| `lock_file.rs` | `%APPDATA%\Kosmos\kepler.lock.json` (pid, ws_port, bearer token) |
| `singleton.rs` | rusqlite WAL BEGIN IMMEDIATE — одна копия на машину |
| `auth.rs` | bearer token, PID-binding, file ACL |
| `protocol_version.rs` | hello-handshake version match |

### Apps (consumers + producers)

Каждая апка — standalone Electron .exe. Связи:

- **Renderer**: Vue 3 Vapor через `kosmos-visuals` (`Sidebar`, `Titlebar`, `DesktopChrome`, `CommandPalette` и т.п.). Никакого SQLite, всё через preload IPC.
- **Electron main**: коннектится к `kepler-backend` через `@kosmos/ark` (kepler-mode). Спавн собственного sidecar — **не делает**. Sync — **не запускает** (backend сам делает).
- **Consumers**: читают/пишут ARK objects через `arkClient.objects.*` / `arkClient.links.*`. Подписываются на entity events.
- **Producers**: регистрируют action-commands через `arkClient.commands.register(...)` и слушают `arkClient.commands.onInvoked(...)`. Пример — Horologion регистрирует `horologion:pomodoro:25` и при invoke стартует таймер.

См. [Command bus](/concepts/command-bus).

## Слои

### Renderer

Vue 3.6 Vapor в Eden, Dashboard, Arrancador, Horologion. Никогда не пишет напрямую в SQLite. Общается с Electron main через preload API.

Использует общие UI-примитивы из `kosmos-visuals`: `Sidebar`, `Titlebar`, `TitlebarHistoryControls`, `DesktopChrome`, `DesktopContentSurface`, `CommandPalette`, `StatusDot`, `TodoRow`, `GamePosterCard`, `QuickEntryPanel`, `CustomCaret`.

### Electron main (per-app)

Оркестрирует приложение, держит IPC, держит `ArkClient` в kepler-mode. **Единственный** слой, который вызывает `@kosmos/ark`. Renderer его не видит — он работает через `window.<appName>Api` (`window.dashboardApi`, `window.arrancador`, и т.п.).

### @kosmos/ark

Канонический TS-клиент. Два режима:

- **kepler-mode (default сейчас)** — `ArkClient` коннектится к `kepler-backend` через WS, используя `kepler.lock.json` (bearer token, port). Шейринг одного backend'а с другими апками.
- **self-managed sidecar (legacy)** — `ArkClient` сам спавнит и владеет процессом `ark-core-rpc.exe`. Использовалось до Kepler как brand swap; сейчас live только для тестов и fallback.

См. [@kosmos/ark](/packages/kosmos-ark).

### ark-core-rpc (Rust)

Канонический бинарь рантайма. Принимает newline-delimited JSON со stdin, пишет ответы и события в stdout. Внутри:

| Файл | Что делает |
|---|---|
| `main.rs` | stdin/stdout цикл, диспетчер операций |
| `db.rs` | SQLite CRUD, миграции, sync storage adapter |
| `schema.rs` | DDL — `CREATE TABLE IF NOT EXISTS` и индексы |
| `types.rs` | shared entities и sync payloads |
| `protocol.rs` | wire-протокол sync (фреймы) |
| `hlc.rs` | Hybrid Logical Clock |
| `sync_server.rs`, `sync_client.rs` | WebSocket sync |
| `beacon.rs` | UDP discovery в LAN |
| `relay_transport.rs`, `relay_sync.rs` | relay-bridge поверх sync |
| `mesh.rs` | координация LAN + relay |
| `host.rs`, `net.rs` | фильтрация hostname и routable addresses |
| `ffi.rs` | UniFFI facade для Android / Swift |

### SQLite

Одна база на пространство данных (`space`), путь типа `%APPDATA%\Kosmos\spaces\<spaceId>\ark.db` (или `%APPDATA%\Kosmos\ark.db` для usage-tracker и default space).

Схема — additive: `init_schema` мигрирует существующие БД на месте через `CREATE TABLE IF NOT EXISTS` без перезаписи файла.

См. [Модель данных ARK](/concepts/ark-objects).

## Communication primitives

| Primitive | Где определён | Когда |
|---|---|---|
| **ARK objects** | `objects` + `object_links` в SQLite | Долгоживущие данные, репликация sync'ом |
| **Entity events** | `onArkEvent` / `onEntityChanged` в `@kosmos/ark` | Подписка на изменения объектов |
| **Command bus** | `commands.*` в WS protocol | Императивные «ручки» апок (Pomodoro start, create note и т.п.). См. [Command bus](/concepts/command-bus) |
| **Extension host** | `kepler:extension:*` IPC в kepler-shell | Загрузка Vue extension bundles внутри launcher'а (Phase 4 foundation). См. [Extension host](/concepts/extension-host) |

## Принципы

### 1. Local-first

Каждое устройство имеет полную копию своих данных. Сеть нужна для синхронизации, не для чтения. Приложение работает offline.

### 2. Single shared runtime

`ark-core-rpc` владеет схемой, объектами, usage-данными, sync-протоколом. `kepler-backend` владеет только одной копией `ark-core-rpc` на машину + WS gateway. Приложения — тонкие оболочки. Если ты пишешь много логики работы с данными внутри приложения — ты, скорее всего, ошибаешься; этот код должен быть в ARK.

### 3. Narrow contract

Приложения говорят с ARK **только** через `@kosmos/ark` (TS) или `ark_core::db` (Rust-writers). Прямые SQL writes в app services запрещены. См. [Граница записи](/concepts/write-boundary).

### 4. Auditable changes

Substantial-правки проходят через proof loop с явными AC и evidence. См. [Proof loop](/concepts/proof-loop).

### 5. Sync-aware

Любая запись в синхронизируемую сущность обязана обновлять `lan_sync.version_vector` через runtime. Прямой SQL `INSERT` в обход — ломает CRDT-merge на пирах. Если по веской причине пишешь напрямую (Rust direct writer вроде `usage-tracker`) — используй `ark_core::db::bump_sync_version_vector`.

## Компонентная карта

| Кусок | Где | Тип | Роль |
|---|---|---|---|
| ark-core | `packages/ark-core/rust` | Rust crate + бинарь | runtime данных |
| @kosmos/ark | `packages/kosmos-ark` | TS SDK | клиент к sidecar / kepler-backend |
| kepler-shell | `apps/kepler-shell` | TS + Electron | launcher host, tray, settings, extension loader |
| kepler-backend | `services/kepler-backend` | Rust binary | shared runtime supervisor + WS gateway + command bus + sync |
| ark-relay-server | `services/ark-relay-server` | Rust server | WebSocket relay (NAT-обход p2p sync) |
| kosmos-visuals | `packages/kosmos-visuals` | TS + Vue | дизайн-система |
| Electron apps | `apps/{eden,delphi,arrancador,dashboard,horologion}` | TS + Electron | продуктовые оболочки |
| ark-service | `apps/ark-service` | Kotlin + Room | Android ContentProvider, держит данные Android Delphi (`apps/delphi/kotlin`). Изолирован от desktop ARK, ждёт миграции на UniFFI |
| usage-tracker | `services/usage-tracker` | Rust | захват usage data → ARK (через kepler-backend WS либо direct write) |
