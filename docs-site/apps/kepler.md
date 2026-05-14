# Kepler — sync host + global launcher (opt-in)

::: tip Источник правды
`apps/kepler/`, `.agent/tasks/2026-05-13-kepler-phase-*` (proof loops)
:::

**Kepler** — фоновый Rust-процесс с трей-иконкой, который активирует **синхронизацию между устройствами** + global launcher по хоткею. Это **opt-in feature** для Kosmos — если sync не нужен, ставить Kepler не обязательно.

## Архитектурная модель

::: info Sync = opt-in feature
Kosmos-приложения (Eden, Delphi, Arrancador, Horologion, Dashboard) **по умолчанию работают standalone**: каждая апка спавнит свой `ark-core-rpc` child-процесс, читает/пишет общую `%APPDATA%\Kosmos\ark.db`. Multi-process write через SQLite WAL. **Без LAN/relay sync.**

**Установи Kepler если нужно:**
- Sync между устройствами (LAN или через relay-server)
- Единый sidecar на машине вместо N (экономия ~150 MB RAM при 4+ запущенных апках)
- Global launcher Ctrl+Shift+K (поиск ARK FTS5 + quick-create задач/заметок)
- Tray-иконка со статусом

Без Kepler — apps просто работают локально, ARK FTS5 поиск работает, всё в одной DB shared между апками на машине.
:::

## Detection — как apps решают использовать Kepler

При запуске каждой апки `@kosmos/ark` `ensureKeplerRunning()` ищет `%APPDATA%\Kosmos\kepler.lock.json`. Если файл есть и PID жив:
- App коннектится к Kepler через WS (kepler mode) — получает sync + общий sidecar.

Если файла нет (Kepler не установлен / не запущен):
- App спавнит свой `ark-core-rpc` child (standalone mode), работает локально.
- Sync **не запускается** (никто не вызывает `start_sync`).
- Если установишь Kepler позже — следующий запуск апки автоматически переключится.

`KOSMOS_REQUIRE_KEPLER=1` env флаг — для production deployments, где запуск без Kepler недопустим. Default — flag не set, апки fallback на standalone.

## Архитектура (target после Phase 6)

```text
                  ┌──────────────────────────────────────────┐
                  │  Kepler (Rust + gpui tray, single inst.) │
                  │   ├─ ark-core-rpc child (stdio supervis.)│
                  │   ├─ WS server 127.0.0.1:<random_port>   │
                  │   ├─ kepler.lock.json (strict ACL)       │
                  │   ├─ Singleton lock (SQLite WAL)         │
                  │   └─ Launcher window (Ctrl+Shift+K, Phase 6)│
                  └────────────────┬─────────────────────────┘
                                   │ ws://127.0.0.1:<port>
       ┌──────────┬────────────────┼────────────────┬──────────┐
   ┌───┴───┐ ┌────┴────┐    ┌──────┴─────┐ ┌────────┴──┐ ┌─────┴──────┐
   │ Eden  │ │ Delphi  │    │ Arrancador │ │ Dashboard │ │usage-tracker│
   └───────┘ └─────────┘    └────────────┘ └───────────┘ └─────────────┘
```

## Текущий статус

| Фаза | Что | Статус |
|------|-----|--------|
| 1 | Host scaffold (singleton, lock-file, WS server, ark-core-rpc supervisor, version handshake, PID-binding) | ✅ AC1-AC7 PASS, 41 unit + 4 integration |
| 2 | `@kosmos/ark` kepler-mode + ARK hold-and-replay + Eden cutover | ✅ Code done, manual smoke pending |
| 1.5 | installer + kepler-watcher | ✅ done |
| 1.6 | tray-icon + tao event loop | ✅ Tray иконка с menu "Выход", graceful shutdown через SHUTDOWN_REQUESTED |
| 3 | Delphi/Arrancador/Horologion cutover | ✅ Все три апки переписаны, typecheck зелёный |
| 4 | usage-tracker → WS RPC | ✅ Wired: persist_* через KeplerClient, spool fallback, periodic flush. `USAGE_TRACKER_DIRECT=1` для direct write mode |
| 5 | LAN sync централизация | ✅ Kepler host auto-`start_sync` при старте; apps в kepler mode не делают своего |
| 5.5 | Soak 2-4 недели | ❌ blocked on user dogfooding |
| 6 | Launcher UI + cleanup + auto-launch | ✅ Substantial: eframe launcher, Ctrl+Shift+K, FTS5 search, quick-create (task/note). Settings window + remove KOSMOS_KEPLER_OPTIONAL — следующие итерации/user-decision |

См. [Kepler Roadmap](./kepler-roadmap.md) для деталей.

## Стек

| Слой | Технология |
|---|---|
| Runtime | Rust 2021, tokio (multi-thread) |
| GUI (Phase 6) | gpui 0.2.2 + gpui-component 0.5.1 |
| Tray (Phase 1.6) | `tray-icon` крейт |
| Hotkey (Phase 6) | `global-hotkey` крейт |
| Transport | `tokio-tungstenite` (WS 127.0.0.1, latency P95 = 258µs на dev машине) |
| Auth | bearer token (256-bit hex) + OS file permissions + PID-binding |
| Singleton | `rusqlite` WAL `BEGIN IMMEDIATE` (паттерн из `services/usage-tracker`) |

## Discovery — `kepler.lock.json`

Kepler при старте атомарно пишет lock-файл с PID, port, token, protocol version:

```text
%APPDATA%\Kosmos\kepler.lock.json   (Windows)
$XDG_CONFIG_HOME/Kosmos/kepler.lock.json   (Linux)
~/Library/Application Support/Kosmos/kepler.lock.json   (macOS)
```

Структура:

```json
{
  "format_version": 1,
  "protocol_version": { "major": 1, "minor": 0, "patch": 0 },
  "pid": 12345,
  "ws_port": 52436,
  "auth_token": "deadbeef...",
  "started_at": "2026-05-13T15:00:00Z",
  "db_path": "..."
}
```

::: warning Permissions критичны
Файл создаётся с **strict OS permissions** — на Windows ACL разрешает чтение только текущему user SID (через `icacls /inheritance:r /grant:r %USERNAME%:F`); на Unix — `chmod 0600`. Любой другой user account на той же машине **не должен** прочитать `auth_token`. Это AC3 spec фазы 1.
:::

## Protocol — hello-handshake

Перед любыми RPC client отправляет hello:

```json
{
  "kind": "hello",
  "protocolVersion": "1.0.0",
  "token": "<auth_token из lock-file>",
  "pid": 12345,
  "clientId": "eden"
}
```

Server отвечает `hello_ok` или `hello_error`:

```json
{ "kind": "hello_ok", "protocolVersion": "1.0.0", "compatibility": "exact" }
```

```json
{ "kind": "hello_error", "code": "incompatible_protocol_version", "message": "..." }
```

### Reject коды

| Код | Когда |
|---|---|
| `missing_protocol_version` | client не прислал `protocolVersion` |
| `malformed_protocol_version` | не парсится как semver |
| `incompatible_protocol_version` | MAJOR mismatch (`2.x.x` vs server `1.x.x`) |
| `missing_token` | client не прислал `token` |
| `invalid_token` | token не совпадает с lock-file |
| `missing_pid` | client не прислал `pid` |
| `invalid_pid` | процесс с указанным PID не существует |
| `foreign_user_pid` | процесс принадлежит другому user account |
| `malformed_hello` | hello — не валидный JSON |

После hello_ok — обычный JSON-RPC (`{operation, _req_id, ...}` → `{ok, data, error, _req_id}`).

## Запуск (dev)

```powershell
# 1. Собрать ark-core-rpc release
cargo build --release --manifest-path packages\ark-core\rust\Cargo.toml --bin ark-core-rpc

# 2. Запустить Kepler в режиме разработки
$env:ARK_CORE_RPC_PATH = "$PWD\packages\ark-core\rust\target\release\ark-core-rpc.exe"
cargo run --manifest-path apps\kepler\Cargo.toml --bin kepler
```

В stderr увидишь:

```text
Kepler v0.1.0 starting (protocol 1.0.0)
[kepler] singleton acquired: ...\Kosmos\kepler-singleton.lock.db
[kepler] ark-core-rpc binary: ...
[kepler] db: ...\Kosmos\ark.db
[kepler] ark-core-rpc spawned and initialized
[kepler] WS listening on 127.0.0.1:52436
[kepler] lock-file: ...\Kosmos\kepler.lock.json
[kepler] ready. Ctrl+C to stop.
```

Ctrl+C — graceful shutdown, lock-файл удаляется.

## Команды

| Команда | Что |
|---|---|
| `cargo run --manifest-path apps\kepler\Cargo.toml --bin kepler` | Запуск dev |
| `cargo build --release --manifest-path apps\kepler\Cargo.toml --bin kepler` | Релизный бинарь |
| `cargo test --manifest-path apps\kepler\Cargo.toml --lib` | Unit-тесты (41) |
| `cargo test --manifest-path apps\kepler\Cargo.toml --test handshake -- --test-threads=1` | E2E integration (4) |
| `cargo bench --manifest-path apps\kepler\Cargo.toml --bench rpc_latency` | AC6 latency benchmark |

## Env vars (клиент-side)

| Var | Где |
|---|---|
| `KOSMOS_REQUIRE_KEPLER=1` | Eden/Delphi/etc — без Kepler апка fail'ится. Для production deployments. Default — flag не set, апки работают standalone когда Kepler недоступен. |
| `KOSMOS_KEPLER_OPTIONAL=1` | **Deprecated** — был для opt-in fallback'а в старой модели. Сейчас fallback дефолтный, флаг no-op (оставлен для backward compat). |
| `ARK_CORE_RPC_PATH` | Override path к `ark-core-rpc` бинарю (Kepler resolve этот в spawn child). |
| `KOSMOS_DB_PATH` | Override path к ARK DB (Kepler сам). |
| `KEPLER_SKIP_SYNC=1` | Не запускать LAN sync в Kepler host (для тестов / dev). |
| `KOSMOS_SPACE_ID` / `KOSMOS_DEVICE_ID` / `KOSMOS_DEVICE_NAME` | Kepler sync identity. Авто-derived если не set. |
| `KOSMOS_RELAY_URL` / `KOSMOS_RELAY_API_KEY` / `KOSMOS_AUTH_SECRET` | Relay sync config. |

## См. также

- [Kepler Roadmap](./kepler-roadmap.md)
- [@kosmos/ark](../packages/kosmos-ark.md) — TypeScript SDK с kepler-mode
- [Sync](../concepts/sync.md) — sync layer, hold-and-replay для schema drift
- [Architecture](../concepts/architecture.md)
