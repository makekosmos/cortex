# Архитектура

::: info Почему именно Electron, а не Tauri / Wails / etc.
Решение опирается на эксперимент с замерами:
[Tauri vs Electron — 2026-05-19](/experiments/tauri-vs-electron). Короткий вывод:
на Windows WebView2 = тот же Chromium, экономия RAM всего **24%** при цене 3-6
недель переписывания; на Linux WebKitGTK ломает TipTap в Eden.
:::

## Brand'ы

- **Kosmos** — название экосистемы. Под Kosmos живут продуктовые приложения (Eden, Delphi, Arrancador, Horologion, Dashboard) и shared-пакеты (`@kosmos/ark`, `@kosmos/visuals`).
- **Kepler** — Electron-launcher + shared Rust runtime (`kepler-backend`), который держит один `ark-core-rpc` child на машину, gateway-WS для апок, command bus и LAN/relay sync.

Раньше "Kepler" обозначал отдельный Rust+gpui фоновый sync host. Сейчас это имя перешло на Electron launcher (`shell/`, npm package `kepler-shell`) + его Rust backend (`services/kepler-backend`). Шилд старой архитектуры — снят. Phase B-E (2026-05-14) свели весь stack в плоский top-level layout: `shell/`, `extensions/`, `crates/`, `mobile/`, `services/`.

## Высокоуровневая картина

```text
┌──────────────────────────────────────────────────────────────┐
│  kepler.exe  (Electron host, shell/)                         │
│    ├─ Launcher BrowserWindow (Ctrl+Shift+K, FTS5 search)     │
│    ├─ Tray icon                                              │
│    ├─ Settings window                                        │
│    ├─ Extension loader (extensions/<id>/)                    │
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
│    ├─ usage_tracker module (Phase E2 — встроен в backend)    │
│    └─ start_sync (LAN + relay) при старте                    │
└──────────────────────────────────────────────────────────────┘
              │ WS ws://127.0.0.1:<port>
   ┌──────────┼────────────┬────────────┬────────────┐
┌──┴──┐ ┌─────┴────┐ ┌─────┴────┐ ┌─────┴───┐ ┌──────┴─────┐
│Eden │ │ Delphi   │ │Arrancador│ │Horologion│ │ Dashboard  │
└─────┘ └──────────┘ └──────────┘ └──────────┘ └────────────┘
 standalone   extensions/<id>/ (Vue-extensions внутри shell/)

Eden — standalone Electron .exe (apps/eden). Остальные — Vue-extensions,
открываются внутри Kepler shell, общаются с kepler-backend через @kosmos/ark.
```

- `shell/` (Electron, npm `kepler-shell`) — оркестратор: launcher окно, tray, settings, extension loader, спавн `kepler-backend`.
- `kepler-backend` (Rust) — shared runtime: supervisor для `ark-core-rpc`, WS gateway, command bus, sync, встроенный usage tracker.
- `ark-core-rpc` (Rust, `crates/ark-core`) — канонический ARK runtime: SQLite, миграции, sync protocol.
- Extensions (`extensions/<id>/`) — Vue-bundles внутри Kepler shell. Общаются с `kepler-backend` через `@kosmos/ark`.
- Eden — пока standalone (`apps/eden/ts`), миграция в extensions — Phase 6.

## Роли

### Kepler launcher (kepler-shell)

`shell/electron/main.ts` — Electron host:

- Спавнит `kepler-backend.exe` как child (`spawnBackend`).
- Подключается к нему через `@kosmos/ark` в kepler-mode (`ensureKeplerRunning`).
- Показывает launcher окно (frameless, acrylic background, alwaysOnTop) по `Ctrl+Shift+K`.
- Tray-иконка с menu (Открыть / Настройки / Выход).
- Список команд в launcher = **static open-commands** (`COMMANDS` из `electron/commands.ts`) + **dynamic action-commands** (через `arkClient.commands.list()` — приходят от running апок).
- При invoke: static исполняются локально (`spawn(exe)`), dynamic уходят в `kepler-backend` через `arkClient.commands.invoke(id)` — backend broadcast'ит `command_invoked`, owning апка handle'ит.
- Extension loader (`shell/electron/extension-host.ts`) — open Vue extension bundles в отдельные BrowserWindow (Phase 4: Dashboard / Horologion / Delphi / Arrancador мигрированы как extensions).

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

Eden (`apps/eden/ts`) — standalone Electron .exe. Остальные (Delphi, Arrancador, Dashboard, Horologion) — Vue-extensions в `extensions/<id>/`, открываются внутри Kepler shell. Связи:

- **Renderer**: Vue 3 Vapor через `@kosmos/visuals` (`Sidebar`, `Titlebar`, `DesktopChrome`, `CommandPalette` и т.п.). Никакого SQLite, всё через preload IPC.
- **Electron main** (Eden) / **extension host** (Kepler shell main): коннектится к `kepler-backend` через `@kosmos/ark` (kepler-mode). Спавн собственного sidecar — **не делает**. Sync — **не запускает** (backend сам делает).
- **Consumers**: читают/пишут ARK objects через `arkClient.objects.*` / `arkClient.links.*`. Подписываются на entity events.
- **Producers**: регистрируют action-commands через `arkClient.commands.register(...)` и слушают `arkClient.commands.onInvoked(...)`. Пример — Horologion регистрирует `horologion:pomodoro:25` и при invoke стартует таймер.

См. [Command bus](/concepts/command-bus).

## Слои

### Renderer

Vue 3.6 Vapor в Eden, Dashboard, Arrancador, Horologion. Никогда не пишет напрямую в SQLite. Общается с Electron main через preload API.

Использует общие UI-примитивы из `@kosmos/visuals`: `Sidebar`, `Titlebar`, `TitlebarHistoryControls`, `DesktopChrome`, `DesktopContentSurface`, `CommandPalette`, `StatusDot`, `TodoRow`, `GamePosterCard`, `QuickEntryPanel`, `CustomCaret`.

### Electron main (per-app)

Оркестрирует приложение, держит IPC, держит `ArkClient` в kepler-mode. **Единственный** слой, который вызывает `@kosmos/ark`. Renderer его не видит — он работает через `window.<appName>Api` (`window.dashboardApi`, `window.arrancador`, и т.п.).

### @kosmos/ark

Канонический TS-клиент. Два режима:

- **kepler-mode (default сейчас)** — `ArkClient` коннектится к `kepler-backend` через WS, используя `kepler.lock.json` (bearer token, port). Шейринг одного backend'а с другими апками.
- **self-managed sidecar (legacy)** — `ArkClient` сам спавнит и владеет процессом `ark-core-rpc.exe`. Использовалось до Kepler как brand swap; сейчас live только для тестов и fallback.

См. [@kosmos/ark](/packages/ark).

### ark-core-rpc (Rust)

Канонический бинарь рантайма. Принимает newline-delimited JSON со stdin, пишет ответы и события в stdout. Все исходники — в `crates/ark-core/rust/src/`:

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

Одна база на юзера — `%APPDATA%\Kosmos\ark.db`. Концепция multi-space убрана 2026-05-15: больше нет welcome screen / space picker, больше нет `%APPDATA%\Kosmos\spaces\<spaceId>\ark.db`, больше нет `KOSMOS_DB_PATH` / `selected-space.json`. Все apps (Eden, Delphi, Horologion, Arrancador, usage-tracker, Dashboard) работают на одной DB.

Схема — additive: `init_schema` мигрирует существующие БД на месте через `CREATE TABLE IF NOT EXISTS` без перезаписи файла.

LAN sync namespace = глобальный `kepler-default` (фиксированный `spaceId` в ArkClient / backend `resolve_space_id`). Сменить через env `KOSMOS_SPACE_ID` если требуется изоляция мешей (advanced).

См. [Модель данных ARK](/concepts/ark-objects).

Apps (`@kosmos/ark` в kepler-mode) не открывают SQLite напрямую — они коннектятся к `kepler-backend` по WS из `kepler.lock.json`, а backend владеет путём к SQLite.

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
| ark-core | `crates/ark-core/rust` | Rust crate + бинарь | runtime данных |
| @kosmos/ark | `packages/ark` | TS SDK | клиент к sidecar / kepler-backend |
| kepler-shell | `shell/` (npm `kepler-shell`) | TS + Electron | launcher host, tray, settings, extension loader |
| kepler-backend | `services/kepler-backend` | Rust binary | shared runtime supervisor + WS gateway + command bus + sync + usage_tracker модуль |
| ark-relay-server | `services/ark-relay-server` | Rust server | WebSocket relay (NAT-обход p2p sync) |
| kepler-watcher | `services/kepler-watcher` | Rust | watcher-демон над ark-core |
| @kosmos/visuals | `packages/visuals` | TS + Vue | дизайн-система |
| Eden (standalone) | `apps/eden/ts` | TS + Electron | заметки, пока вне shell (Phase 6 — миграция) |
| Vue-extensions | `extensions/{delphi,arrancador,dashboard,horologion}` | TS + Vue | продуктовые оболочки внутри Kepler shell |
| ark-service (Android) | `mobile/ark-service` | Kotlin + Room | Android ContentProvider, держит данные Android Delphi (`mobile/delphi`). Изолирован от desktop ARK, ждёт миграции на UniFFI |
| usage-tracker module | `services/kepler-backend/src/usage_tracker/` | Rust | захват usage data → ARK; standalone-бинарь заморожен в `legacy/usage-tracker/` после Phase E3 |
