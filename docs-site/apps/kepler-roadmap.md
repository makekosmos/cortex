# Kepler — Roadmap

Полный план — `C:\Users\Kazui\.claude\plans\sharded-waddling-dove.md`.

## Ближайшее

### Phase 2 — Eden cutover (WIP — manual smoke pending)

**Что готово:**
- `apps/eden/ts/main/ark.ts` полностью переписан на `@kosmos/ark` kepler-mode.
- `KOSMOS_KEPLER_OPTIONAL=1` env-флаг для fallback на self-managed sidecar (transitional).
- ARK hold-and-replay для schema drift (`sync_pending_objects` таблица + `sync_error` / `sync_replay` events).
- `@kosmos/ark` имеет `ensureKeplerRunning()` helper + `keplerLock` mode в `ArkClient`.

**Что осталось:**
- Manual smoke: запустить Kepler + Eden, verify ровно 1 `ark-core-rpc` процесс в Task Manager, CRUD заметок работает.
- Manual smoke: kill Kepler в runtime, verify Eden показывает ошибку, restart Kepler, verify Eden reconnect.
- Playbook: `.agent/tasks/2026-05-13-kepler-phase-2-eden-cutover/manual_smoke.md`.

### Phase 1.5 — installer + kepler-watcher ✅ done

- ✅ `apps/kepler/installer/install.ps1` — HKCU Run autostart, копирование `kepler.exe` + `ark-core-rpc.exe` + `kepler-watcher.exe` (если есть) + `kepler.png`. Поддерживает `-NoStartup` / `-NoLaunch` / кастомный `-InstallDir`. Если watcher есть — autostart запускает его (он управляет lifecycle kepler.exe); иначе kepler.exe напрямую.
- ✅ `apps/kepler/installer/uninstall.ps1` — снятие HKCU Run, остановка процессов, удаление lock-файла + singleton-lock + файлов установки. Опции `-KeepFiles` / `-KeepDb`.
- ✅ `services/kepler-watcher/` — ~80 строк Rust бинарь. Держит kepler.exe child, при exit'е респавнит с exp backoff (1s → 30s cap). Если kepler прожил больше 60 сек — backoff сбрасывается. Watcher завершается только когда сам получает SIGTERM/Ctrl+C от OS.

### Phase 3 cutover — все Electron-апки ✅ done (typecheck зелёный)

Все три апки переписаны на `@kosmos/ark` kepler-aware resolution + `KOSMOS_KEPLER_OPTIONAL=1` fallback (аналог Eden из Phase 2):

- ✅ **Horologion** — `apps/horologion/electron/main.ts` `getArk()` функция расширена с `ensureKeplerRunning` switch.
- ✅ **Arrancador** — `apps/arrancador/electron/main/services/ark-game-objects.ts` — `getArkObjects` / `getArkObjectTypes` стали async и идут через `resolveArkClient` с kepler путём. 3 callsites обновлены (`await getArkObjects()`).
- ✅ **Delphi** — `apps/delphi/ts/electron/sidecar.ts` lines 1-310 (transport) полностью переписаны: `SidecarClient` теперь thin wrapper над `@kosmos/ark` `ArkClient`. Public API (`request` / `onEvent` / `currentDbPath` / `reinit` / `releaseAndReset` / `shutdown`) сохранён, только последние три стали async. 343 строки domain types + 40 wrapper functions (lines 311-653) — без изменений. `dbSwitchSpace` обновлён с `await sidecar.reinit(...)`.

Dashboard (`apps/dashboard/`) **не переключается** на kepler mode — он read-only через `better-sqlite3`, WAL разрешает multi-reader. Это сознательное решение из плана.

### Phase 1.6 — tray-icon + tao event loop ✅

**Реализовано:**
- `apps/kepler/src/tray.rs` (~190 строк): tray иконка на отдельной std-thread с `tao` event loop, чтобы не конфликтовать с tokio runtime в main thread.
- Tray menu: «Kosmos Kepler» (disabled label) + «Выход».
- Иконка резолвится через `KEPLER_ICON_PATH` env / рядом с exe / dev fallback `apps/kepler/icons/master.png`.
- Tooltip показывает PID + WS port.
- Graceful shutdown: клик «Выход» → `SHUTDOWN_REQUESTED.store(true)` → main loop в tokio видит через poll каждые 250ms → cleanup + exit.
- `cargo build` зелёный с `tray-icon = "0.20"` + `tao = "0.31"` + `global-hotkey = "0.7"`.

**TODO для Phase 6:** окно «О Kepler» с PID/port/uptime/connected clients — будет частью gpui launcher work.

## Потом

### Phase 3 — Delphi + Arrancador + Horologion cutover

Тот же паттерн что у Eden:
- `apps/delphi/ts/electron/sidecar.ts` — replace transport (lines 1-310), сохранить types/wrappers.
- `apps/arrancador/electron/main/ark-runtime.ts` — kepler-mode.
- `apps/horologion/electron/main.ts` — kepler-mode.
- Dashboard оставляем direct-read (`better-sqlite3` read-only).

### Phase 4 — usage-tracker через WS RPC ✅

**Реализовано:**
- `kepler_client` модуль — полноценный WS-клиент к Kepler host: hello-handshake, `_req_id` correlation, auto-reconnect при transport errors, typed errors (`KeplerError`).
- `spool` модуль — in-memory FIFO. MAX_ENTRIES=10000, drain/drain_n/push, overflow drops oldest.
- `main.rs` wiring: `try_init_kepler_writer(device_id)` при старте; persist_tracked_app / persist_usage_session / persist_usage_event теперь ходят через `try_kepler_write` сначала, при ошибке кладут в spool (а не пишут direct — избегаем double-write).
- Periodic `flush_spool_batch` каждые 30 сек в main loop — батч 50 ops, при ошибке прерывается, retry на следующем интервале.
- Env `USAGE_TRACKER_DIRECT=1` принудительно direct mode (skip Kepler).
- 12 unit-тестов passing (4 existing + 5 spool + 3 kepler_client), no regressions.

**Устраняет:** race на `lan_sync.version_vector` row (Kepler централизованно делает `bump_sync_version_vector` через `upsert_*` ops).

**Known risk** (из плана): hot event stream при high freq может перегрузить JSON serde. Mitigation — opt-in binary канал (postcard/bincode) поверх WS, только для emitted events. Активировать только если профилирование покажет проблему в production.

### Phase 5 — LAN sync централизация ✅

**Готово:**
- `@kosmos/ark` kepler mode пропускает `start_sync` (app не делает свой sync). 
- Kepler host автоматически вызывает `start_sync` после spawn ark-core-rpc — Kepler является **единственным** sync node на машине. Конфигурация через env: `KOSMOS_SPACE_ID`, `KOSMOS_DEVICE_ID`, `KOSMOS_DEVICE_NAME`, `KOSMOS_RELAY_URL`, `KOSMOS_RELAY_API_KEY`, `KOSMOS_AUTH_SECRET`. `KEPLER_SKIP_SYNC=1` отключает sync (для тестов).
- Persistent device_id хранится в `%APPDATA%\Kosmos\kepler-device-id.txt` (16 hex символов, генерируется при первом запуске).

Same-machine LAN overhead (4 sidecar'а через TCP loopback) **закрыт mechanically** — теперь на машине 1 sync node.

**Known risk:** hot event stream (10 events/sec может). Mitigation — opt-in binary канал (postcard/bincode) поверх WS, только для emitted events, если AC5 fail.

### Phase 5 — LAN sync централизация

Убрать `start_sync` op из Electron-апок (Kepler exclusive sync owner). Beacon на машине ровно один → одна из главных причин Kepler закрыта.

### Phase 5.5 — Soak (2-4 недели)

Active dogfooding. Memory leak detection через hourly RSS log → linear regression slope (порог 1 MB/day). 3 reset подряд без зелёного 14-дневного окна → architecture review.

### Phase 6 — Launcher UI + cleanup + auto-launch

- gpui-component launcher window: глобальный хоткей `Ctrl+Shift+K`, поиск через ARK FTS5, quick-create задач (через `QuickEntryPanel` концепцию).
- Settings window: подключённые клиенты, путь к DB, sync статус, autostart toggle.
- `ensureKeplerRunning()` auto-launch в апках (если Kepler installed но не запущен — апка делает `Start-Process` сама).
- Убрать `KOSMOS_KEPLER_OPTIONAL` env-флаг — Kepler обязателен (точка невозврата).
- Убрать `ark-core-rpc.exe` из Electron-апок bundle (теперь только в Kepler installer).

## Адъюнкт-блайнд-споты

Покрыты планом (см. [главный план](../../../../../C:\Users\Kazui\.claude\plans\sharded-waddling-dove.md)):

| # | Блайнд-спот | Статус |
|---|---|---|
| #1 | Schema drift между версиями апок | ✅ Решено Phase 2: `sync_pending_objects` hold-and-replay |
| #2 | `schemaJson` validation | ⏳ Отдельная задача после Kepler |
| #3 | Sidecar crash recovery | ✅ Phase 1+2: kepler-watcher + client reconnect |
| #4 | Heart vs FTS5 | ✅ Re-classified — Heart vault filesystem manager, не search engine; конфликта нет |
| #5 | Android UniFFI bindings | ⏳ Не блокирует Kepler |
| #6 | Relay fallback | ⏳ Отдельная задача |
| #7 | Same-machine LAN overhead | 🔄 Решится Phase 5 |

## Баги / замечания

(пусто)
