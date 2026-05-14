# Kosmos — sync host + global launcher (opt-in)

::: tip Источник правды
`apps/kosmos/`, `.agent/tasks/2026-05-13-kosmos-phase-*` (proof loops)
:::

**Kosmos** — фоновый Rust-процесс с трей-иконкой, который активирует **синхронизацию между устройствами** + global launcher по хоткею. Это **opt-in feature** для Kepler — если sync не нужен, ставить Kosmos не обязательно.

## Архитектурная модель

::: info Sync = opt-in feature
Kepler-приложения (Eden, Delphi, Arrancador, Horologion, Dashboard) **по умолчанию работают standalone**: каждая апка спавнит свой `ark-core-rpc` child-процесс, читает/пишет общую `%APPDATA%\Kepler\ark.db`. Multi-process write через SQLite WAL. **Без LAN/relay sync.**

**Установи Kosmos если нужно:**
- Sync между устройствами (LAN или через relay-server)
- Единый sidecar на машине вместо N (экономия ~150 MB RAM при 4+ запущенных апках)
- Global launcher Ctrl+Shift+K (поиск ARK FTS5 + quick-create задач/заметок)
- Tray-иконка со статусом

Без Kosmos — apps просто работают локально, ARK FTS5 поиск работает, всё в одной DB shared между апками на машине.
:::

## Detection — как apps решают использовать Kosmos

При запуске каждой апки `@kepler/ark` `ensureKosmosRunning()` ищет `%APPDATA%\Kepler\kosmos.lock.json`. Если файл есть и PID жив:
- App коннектится к Kosmos через WS (cosmos mode) — получает sync + общий sidecar.

Если файла нет (Kosmos не установлен / не запущен):
- App спавнит свой `ark-core-rpc` child (standalone mode), работает локально.
- Sync **не запускается** (никто не вызывает `start_sync`).
- Если установишь Kosmos позже — следующий запуск апки автоматически переключится.

`KEPLER_REQUIRE_KOSMOS=1` env флаг — для production deployments, где запуск без Kosmos недопустим. Default — flag не set, апки fallback на standalone.

## Архитектура (target после Phase 6)

```text
                  ┌──────────────────────────────────────────┐
                  │  Kosmos (Rust + gpui tray, single inst.) │
                  │   ├─ ark-core-rpc child (stdio supervis.)│
                  │   ├─ WS server 127.0.0.1:<random_port>   │
                  │   ├─ kosmos.lock.json (strict ACL)       │
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
| 2 | `@kepler/ark` cosmos-mode + ARK hold-and-replay + Eden cutover | ✅ Code done, manual smoke pending |
| 1.5 | installer + kosmos-watcher | ✅ done |
| 1.6 | tray-icon + tao event loop | ✅ Tray иконка с menu "Выход", graceful shutdown через SHUTDOWN_REQUESTED |
| 3 | Delphi/Arrancador/Horologion cutover | ✅ Все три апки переписаны, typecheck зелёный |
| 4 | usage-tracker → WS RPC | ✅ Wired: persist_* через CosmosClient, spool fallback, periodic flush. `USAGE_TRACKER_DIRECT=1` для direct write mode |
| 5 | LAN sync централизация | ✅ Kosmos host auto-`start_sync` при старте; apps в cosmos mode не делают своего |
| 5.5 | Soak 2-4 недели | ❌ blocked on user dogfooding |
| 6 | Launcher UI + cleanup + auto-launch | ✅ Substantial: eframe launcher, Ctrl+Shift+K, FTS5 search, quick-create (task/note). Settings window + remove KEPLER_KOSMOS_OPTIONAL — следующие итерации/user-decision |

См. [Kosmos Roadmap](./kosmos-roadmap.md) для деталей.

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

## Discovery — `kosmos.lock.json`

Kosmos при старте атомарно пишет lock-файл с PID, port, token, protocol version:

```text
%APPDATA%\Kepler\kosmos.lock.json   (Windows)
$XDG_CONFIG_HOME/Kepler/kosmos.lock.json   (Linux)
~/Library/Application Support/Kepler/kosmos.lock.json   (macOS)
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

# 2. Запустить Kosmos в режиме разработки
$env:ARK_CORE_RPC_PATH = "$PWD\packages\ark-core\rust\target\release\ark-core-rpc.exe"
cargo run --manifest-path apps\kosmos\Cargo.toml --bin kosmos
```

В stderr увидишь:

```text
Kosmos v0.1.0 starting (protocol 1.0.0)
[kosmos] singleton acquired: ...\Kepler\kosmos-singleton.lock.db
[kosmos] ark-core-rpc binary: ...
[kosmos] db: ...\Kepler\ark.db
[kosmos] ark-core-rpc spawned and initialized
[kosmos] WS listening on 127.0.0.1:52436
[kosmos] lock-file: ...\Kepler\kosmos.lock.json
[kosmos] ready. Ctrl+C to stop.
```

Ctrl+C — graceful shutdown, lock-файл удаляется.

## Команды

| Команда | Что |
|---|---|
| `cargo run --manifest-path apps\kosmos\Cargo.toml --bin kosmos` | Запуск dev |
| `cargo build --release --manifest-path apps\kosmos\Cargo.toml --bin kosmos` | Релизный бинарь |
| `cargo test --manifest-path apps\kosmos\Cargo.toml --lib` | Unit-тесты (41) |
| `cargo test --manifest-path apps\kosmos\Cargo.toml --test handshake -- --test-threads=1` | E2E integration (4) |
| `cargo bench --manifest-path apps\kosmos\Cargo.toml --bench rpc_latency` | AC6 latency benchmark |

## Env vars (клиент-side)

| Var | Где |
|---|---|
| `KEPLER_REQUIRE_KOSMOS=1` | Eden/Delphi/etc — без Kosmos апка fail'ится. Для production deployments. Default — flag не set, апки работают standalone когда Kosmos недоступен. |
| `KEPLER_KOSMOS_OPTIONAL=1` | **Deprecated** — был для opt-in fallback'а в старой модели. Сейчас fallback дефолтный, флаг no-op (оставлен для backward compat). |
| `ARK_CORE_RPC_PATH` | Override path к `ark-core-rpc` бинарю (Kosmos resolve этот в spawn child). |
| `KEPLER_DB_PATH` | Override path к ARK DB (Kosmos сам). |
| `KOSMOS_SKIP_SYNC=1` | Не запускать LAN sync в Kosmos host (для тестов / dev). |
| `KEPLER_SPACE_ID` / `KEPLER_DEVICE_ID` / `KEPLER_DEVICE_NAME` | Kosmos sync identity. Авто-derived если не set. |
| `KEPLER_RELAY_URL` / `KEPLER_RELAY_API_KEY` / `KEPLER_AUTH_SECRET` | Relay sync config. |

## См. также

- [Kosmos Roadmap](./kosmos-roadmap.md)
- [@kepler/ark](../packages/kepler-ark.md) — TypeScript SDK с cosmos-mode
- [Sync](../concepts/sync.md) — sync layer, hold-and-replay для schema drift
- [Architecture](../concepts/architecture.md)
