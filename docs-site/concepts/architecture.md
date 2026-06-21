# Архитектура

::: info Почему именно Electron, а не Tauri / Wails / etc.
Решение опирается на эксперимент с замерами:
[Tauri vs Electron — 2026-05-19](/experiments/tauri-vs-electron). Короткий вывод:
на Windows WebView2 = тот же Chromium, экономия RAM всего **24%** при цене 3-6
недель переписывания; на Linux WebKitGTK ломает TipTap в Eden.
:::

## Brand'ы

- **Kosmos** — внешний продукт и название экосистемы. Пользователь запускает `Kosmos.exe`; внутри живут Notes/Eden, Tasks/Delphi, Games/Arrancador, Focus/Activity и shared-пакеты (`@kosmos/ark`, `@kosmos/visuals`).
- **Kepler** — legacy/internal имя desktop host слоя: `platform/desktop/`, IPC namespace `kepler:*`, `kepler-backend`, `kepler.lock.json`. В UX и installed artifact'ах с 2026-05-26 используется **Kosmos**.
- **Kosmos Runtime.exe** — packaged display name для `platform/runtime`: Rust host, который держит `ark-core-rpc` child, gateway-WS для апок, command bus и LAN/relay sync. Слияние с `ark-core-rpc` в один процесс — отдельная архитектурная задача, не часть rename migration.

Раньше "Kepler" обозначал отдельный Rust+gpui фоновый sync host, затем Electron launcher. После product rename 2026-05-26 Kepler остаётся только кодовым namespace'ом; user-facing app называется Kosmos. Phase B-E (2026-05-14) свели весь stack в плоский top-level layout: `platform/desktop/`, `extensions/`, `crates/`, `mobile/`, `services/`.

## Высокоуровневая картина

```text
┌──────────────────────────────────────────────────────────────┐
│  Kosmos.exe  (Electron host, platform/desktop/)                         │
│    ├─ Launcher BrowserWindow (Ctrl+Shift+K, FTS5 search)     │
│    ├─ Tray icon                                              │
│    ├─ Settings window                                        │
│    ├─ Extension loader (extensions/<id>/)                    │
│    └─ spawn: Kosmos Runtime.exe                              │
└──────────────────────────────────────────────────────────────┘
                            │
                            │ child process
                            ▼
┌──────────────────────────────────────────────────────────────┐
│  Kosmos Runtime.exe  (Rust, platform/runtime)         │
│    ├─ spawn: Kosmos Data Engine.exe / ark-core-rpc.exe       │
│    ├─ spawn: Kosmos Local STT.exe                             │
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
└─────┘ └──────────┘ └──────────┘ └──────────┘ └────────────┘
 extensions/<id>/ (Vue-extensions внутри platform/desktop/)         shell view

Dashboard — встроенный shell view (platform/desktop/src/views/Dashboard*.vue), не extension.
Все общаются с kepler-backend через @kosmos/ark.
```

- `platform/desktop/` (Electron, npm `kepler-shell`) — оркестратор: launcher окно, tray, settings, extension loader, focus mode подсистема (см. `platform/desktop/electron/focus-*.ts`), спавн `kepler-backend`.
- `kepler-backend` (Rust) — shared runtime: supervisor для `ark-core-rpc`, WS gateway, command bus, sync, встроенный usage tracker, pomodoro host (`pomodoro_host.rs`).
- `ark-core-rpc` (Rust, `core/ark/crates/ark-core`) — канонический ARK runtime: SQLite, миграции, sync protocol.
- Extensions (`extensions/<id>/`) — Vue-bundles внутри Kepler shell. Общаются с `kepler-backend` через `@kosmos/ark`.

## Роли

### Kepler launcher (kepler-shell)

`platform/desktop/electron/main.ts` — Electron host:

- Спавнит `kepler-backend.exe` как child (`spawnBackend`).
- Подключается к нему через `@kosmos/ark` в kepler-mode (`ensureKeplerRunning`).
- Показывает launcher окно (frameless, acrylic background, alwaysOnTop) по `Alt+Space` (prod) / `Alt+\`` (dev) — см. `platform/desktop/electron/instance.ts → resolveInstance().hotkey`и`platform/desktop/electron/settings-window.ts → DEFAULT_HOTKEY_PROD`.
- Tray-иконка с menu (Открыть / Настройки / Выход).
- Список команд в launcher = **static open-commands** (`COMMANDS` из `electron/commands.ts`) + **dynamic action-commands** (через `arkClient.commands.list()` — приходят от running апок).
- При invoke: static исполняются локально (`spawn(exe)`), dynamic уходят в `kepler-backend` через `arkClient.commands.invoke(id)` — backend broadcast'ит `command_invoked`, owning апка handle'ит.

### kepler-backend (Rust)

`platform/runtime/`:

| Файл                  | Что делает                                                                                         |
| --------------------- | -------------------------------------------------------------------------------------------------- |
| `main.rs`             | bootstrap: singleton, lock-file, ark-core-rpc supervisor, WS server, dictation runtime, start_sync |
| `ark_host.rs`         | spawn + watchdog `ark-core-rpc`, proxy stdio JSON ↔ WS                                             |
| `ws_server.rs`        | WS accept loop, auth handshake, dispatch operations (intercept `commands.*`)                       |
| `command_bus.rs`      | registry per WS-connection, broadcast invoke/changed events                                        |
| `sync.rs`             | вызов `start_sync` на ark-core-rpc после health                                                    |
| `lock_file.rs`        | `%APPDATA%\Kosmos\kepler.lock.json` (pid, ws_port, bearer token)                                   |
| `singleton.rs`        | rusqlite WAL BEGIN IMMEDIATE — одна копия на машину                                                |
| `auth.rs`             | bearer token, PID-binding, file ACL                                                                |
| `protocol_version.rs` | hello-handshake version match                                                                      |

### Apps (consumers + producers)

Eden, Delphi, Arrancador, Akasha — Vue-extensions в `extensions/<id>/`, открываются внутри Kepler shell. Dashboard и Focus Session — встроенные shell views. Связи:

- **Renderer**: Vue 3 Vapor через `@kosmos/visuals` (`Sidebar`, `Titlebar`, `DesktopChrome`, `CommandPalette` и т.п.). Никакого SQLite, всё через preload IPC.
- **Extension host** (Kepler shell main): коннектится к `kepler-backend` через `@kosmos/ark` (kepler-mode). Спавн собственного sidecar — **не делает**. Sync — **не запускает** (backend сам делает).
- **Consumers**: читают/пишут ARK objects через `arkClient.objects.*` / `arkClient.links.*`. Подписываются на entity events.
- **Producers**: регистрируют action-commands через `arkClient.commands.register(...)` и слушают `arkClient.commands.onInvoked(...)`. Пример — Delphi регистрирует `delphi:task:create` и при invoke создаёт задачу.

См. [Command bus](/concepts/command-bus).

## Слои

### Renderer

Vue 3.6 Vapor в Eden, Dashboard, Arrancador, Akasha и shell views. Никогда не пишет напрямую в SQLite. Общается с Electron main через preload API.

Использует общие UI-примитивы из `@kosmos/visuals`: `Sidebar`, `Titlebar`, `TitlebarHistoryControls`, `DesktopChrome`, `DesktopContentSurface`, `CommandPalette`, `StatusDot`, `TodoRow`, `GamePosterCard`, `QuickEntryPanel`, `CustomCaret`.

### Electron main (per-app)

Оркестрирует приложение, держит IPC, держит `ArkClient` в kepler-mode. **Единственный** слой, который вызывает `@kosmos/ark`. Renderer его не видит — он работает через `window.<appName>Api` (`window.dashboardApi`, `window.arrancador`, и т.п.).

### @kosmos/ark

См. [@kosmos/ark](/packages/ark).

### ark-core-rpc (Rust)

Канонический бинарь рантайма. Принимает newline-delimited JSON со stdin, пишет ответы и события в stdout. Все исходники — в `core/ark/crates/ark-core/rust/src/`:

| Файл                                  | Что делает                                   |
| ------------------------------------- | -------------------------------------------- |
| `main.rs`                             | stdin/stdout цикл, диспетчер операций        |
| `db.rs`                               | SQLite CRUD, миграции, sync storage adapter  |
| `schema.rs`                           | DDL — `CREATE TABLE IF NOT EXISTS` и индексы |
| `types.rs`                            | shared entities и sync payloads              |
| `protocol.rs`                         | wire-протокол sync (фреймы)                  |
| `hlc.rs`                              | Hybrid Logical Clock                         |
| `sync_server.rs`, `sync_client.rs`    | WebSocket sync                               |
| `beacon.rs`                           | UDP discovery в LAN                          |
| `relay_transport.rs`, `relay_sync.rs` | relay-bridge поверх sync                     |
| `mesh.rs`                             | координация LAN + relay                      |
| `host.rs`, `net.rs`                   | фильтрация hostname и routable addresses     |
| `ffi.rs`                              | UniFFI facade для Android / Swift            |

### SQLite

Схема — additive: `init_schema` мигрирует существующие БД на месте через `CREATE TABLE IF NOT EXISTS` без перезаписи файла.

LAN sync namespace = глобальный `kepler-default` (фиксированный `spaceId` в ArkClient / backend `resolve_space_id`). Сменить через env `KOSMOS_SPACE_ID` если требуется изоляция мешей (advanced).

См. [Модель данных ARK](/concepts/ark-objects).

Apps (`@kosmos/ark` в kepler-mode) не открывают SQLite напрямую — они коннектятся к `kepler-backend` по WS из `kepler.lock.json`, а backend владеет путём к SQLite.

## Communication primitives

| Primitive          | Где определён                                                    | Когда                                                                                                                 |
| ------------------ | ---------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| **ARK objects**    | `objects` + `object_links` в SQLite                              | Долгоживущие данные, репликация sync'ом                                                                               |
| **Entity events**  | `onArkEvent` / `onEntityChanged` в `@kosmos/ark`                 | Подписка на изменения объектов                                                                                        |
| **Command bus**    | `commands.*` в WS protocol                                       | Императивные «ручки» апок (Pomodoro start, create note и т.п.). См. [Command bus](/concepts/command-bus)              |
| **Local STT IPC**  | `dictation.*` local sidecar protocol                             | Dictation host owns state/retries/pending, sidecar owns native engine lifecycle and crash isolation                   |
| **Extension host** | `kepler:extension:*` IPC в kepler-shell                          | Загрузка Vue extension bundles внутри launcher'а (Phase 4 foundation). См. [Extension host](/concepts/extension-host) |
| **Focus mode**     | `platform/desktop/electron/focus-*.ts` + backend `pomodoro_host` | Floating widget, app/website blocker, pomodoro phase events. См. [Focus mode](/concepts/focus-mode)                   |

## Принципы

### 1. Local-first

Каждое устройство имеет полную копию своих данных. Сеть нужна для синхронизации, не для чтения. Приложение работает offline.

### 2. Single shared runtime

`ark-core-rpc` владеет схемой, объектами, usage-данными, sync-протоколом. `kepler-backend` владеет только одной копией `ark-core-rpc` на машину + WS gateway. Local STT остаётся отдельным sidecar boundary: `kepler-backend` хранит dictation state, а `Kosmos Local STT` владеет native engine lifecycle. Приложения — тонкие оболочки. Если ты пишешь много логики работы с данными внутри приложения — ты, скорее всего, ошибаешься; этот код должен быть в ARK.

### 3. Narrow contract

Приложения говорят с ARK **только** через `@kosmos/ark` (TS) или `ark_core::db` (Rust-writers). Прямые SQL writes в app services запрещены. См. [Граница записи](/concepts/write-boundary).

### 4. Auditable changes

Substantial-правки проходят через proof loop с явными AC и evidence. См. [Proof loop](/concepts/proof-loop).

### 5. Sync-aware

Любая запись в синхронизируемую сущность обязана обновлять `lan_sync.version_vector` через runtime. Прямой SQL `INSERT` в обход — ломает CRDT-merge на пирах. Если по веской причине пишешь напрямую (Rust direct writer вроде `usage-tracker`) — используй `ark_core::db::bump_sync_version_vector`.

## Компонентная карта

| Кусок                 | Где                                                                             | Тип                 | Роль                                                                                                                                  |
| --------------------- | ------------------------------------------------------------------------------- | ------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| ark-core              | `core/ark/crates/ark-core/rust`                                                 | Rust crate + бинарь | runtime данных                                                                                                                        |
| @kosmos/ark           | `core/ark/packages/ark`                                                         | TS SDK              | клиент к sidecar / kepler-backend                                                                                                     |
| kepler-shell          | `platform/desktop/` (npm `kepler-shell`)                                        | TS + Electron       | launcher host, tray, settings, extension loader                                                                                       |
| kepler-backend        | `platform/runtime`                                                              | Rust binary         | shared runtime supervisor + WS gateway + command bus + sync + usage_tracker модуль                                                    |
| ark-relay-server      | `services/relay-reference`                                                      | Rust server         | WebSocket relay (NAT-обход p2p sync)                                                                                                  |
| kepler-watcher        | `platform/native-services/kepler-watcher`                                       | Rust                | watcher-демон над ark-core                                                                                                            |
| @kosmos/visuals       | `packages/visuals`                                                              | TS + Vue            | дизайн-система                                                                                                                        |
| Dashboard             | `platform/desktop/src/views/Dashboard*.vue` + `platform/desktop/src/dashboard/` | TS + Vue            | встроенный shell view (read-only ARK browser), не extension                                                                           |
| ark-service (Android) | `incubator/mobile/ark-service`                                                  | Kotlin + Room       | Android ContentProvider, держит данные Android Delphi (`incubator/mobile/delphi`). Изолирован от desktop ARK, ждёт миграции на UniFFI |
| usage-tracker module  | `platform/runtime/src/usage_tracker/`                                           | Rust                | захват usage data → ARK; standalone-бинарь после Phase E3 больше не active-tree path                                                  |
