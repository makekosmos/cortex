# Phase 1: Kepler host scaffold (dark-launch)

**Дата начала**: 2026-05-13
**Дата завершения**: TBD
**Связанный план**: `C:\Users\Kazui\.claude\plans\sharded-waddling-dove.md`
**Memory**: [[project-kepler-planning]], [[feedback-kosmos-naming-translit]]

## Цель

Создать новый Rust бинарь `apps/kepler/`, который:
1. Запускается в системном трее (`tray-icon` крейт).
2. Спавнит ровно один `ark-core-rpc` child-процесс через stdio, супервизирует его.
3. Выставляет JSON-RPC API через WebSocket-сервер на `127.0.0.1:<random_port>` (auto-pick свободного порта).
4. Атомарно пишет `%APPDATA%\Kosmos\kepler.lock.json` с PID, port, authToken, protocolVersion и OS-strict permissions (Win ACL только owner / 0600 elsewhere).
5. Singleton-enforced — вторая копия отказывается стартовать (паттерн из `services/usage-tracker/src/singleton.rs`).
6. Hello-handshake протокола 1.0.0 c PID-binding auth: client передаёт свой PID, Kepler проверяет что процесс существует и принадлежит тому же user.
7. **Dark-launch**: никаких изменений в Electron-апках. Апки всё ещё self-spawn собственный `ark-core-rpc`. Kepler просто живёт параллельно.

## Acceptance criteria

### AC1 — функциональный (все ~60 ARK операций через WS)
**Утверждение**: запустить Kepler, через `wscat -c ws://127.0.0.1:<port>?token=<token>` сделать handshake и затем последовательно вызвать представительный набор операций ARK (минимум: `create_object`, `get_object`, `update_object`, `delete_object`, `list_objects_by_type`, `upsert_object_type`, `search_objects`, `get_lan_sync_status`). Все ops отвечают валидным JSON-RPC ответом.
**Проверка**: `raw/ac1-functional.md` — список ops и ответов.

### AC2 — singleton enforcement
**Утверждение**: при работающем первом Kepler-инстансе попытка запустить вторую копию падает с понятным сообщением (`"Another Kepler instance is already running"`) и exit code != 0. Lock-DB в `%APPDATA%\Kosmos\kepler-singleton.lock.db`, паттерн из `services/usage-tracker/src/singleton.rs:7-27`.
**Проверка**: `raw/ac2-singleton.md` — два exit-code'а и stderr.

### AC3 — lock-file OS permissions
**Утверждение**: `%APPDATA%\Kosmos\kepler.lock.json` имеет permissions, при которых другой user account на той же машине **не может** прочитать файл.
**Проверка**:
- Win: `Get-Acl $env:APPDATA\Kosmos\kepler.lock.json` показывает только текущий user SID в ACL (никаких `BUILTIN\Users`, `Authenticated Users`).
- Unix (фаза 6+): `stat -c %a` → `600`.
**Raw**: `raw/ac3-permissions.md`.

### AC4 — version handshake (отказ при отсутствии protocolVersion)
**Утверждение**: client, отправивший hello-message без поля `protocolVersion`, получает ошибку с `code: "missing_protocol_version"` и Kepler закрывает соединение. Аналогично — client с `protocolVersion: "2.0.0"` (MAJOR mismatch) получает `code: "incompatible_protocol_version"` + закрытие.
**Проверка**: `tests/ws_protocol.rs::test_version_handshake_*` + ручной test через wscat.

### AC5 — PID-binding auth
**Утверждение**: client, отправивший в hello-message `pid: 99999999` (несуществующий PID), получает `code: "invalid_pid"` + закрытие. Client с PID реально другого user'а — `code: "foreign_user_pid"` + закрытие. Client с собственным PID и validным token — ok.
**Проверка**: `tests/auth.rs::test_pid_binding_*`.

### AC6 — latency benchmark (gate для решения о транспорте)
**Утверждение**: Criterion benchmark `benches/rpc_latency.rs` показывает:
- P50 RTT ≤ 5ms для каждого из {`get_object`, `search_objects`, `upsert_object`}.
- P95 RTT ≤ 15ms для каждого из этих RPC.
- При 100 concurrent requests P95 ≤ 30ms.

**Если P95 > 30ms** — STOP, открываем follow-up task на переключение транспорта с WS на named-pipes (Win) / UDS (Mac/Linux). См. план, решение #2.

**Проверка**: `raw/ac6-bench.md` — output `cargo bench --bench rpc_latency`.

### AC7 — unit tests зелёные
**Утверждение**: `cargo test --manifest-path apps/kepler/Cargo.toml` зелёный. Минимум покрытые тесты:
- `tests/ws_protocol.rs` — framing, error responses, version handshake (mismatch → disconnect), big payloads (10MB), malformed JSON.
- `tests/lock_file.rs` — atomic write (temp+rename), stale PID detection (lock есть, PID мёртв → take over), permission check.
- `tests/dispatcher.rs` — client-scoped req_id routing (2 клиента, перекрывающиеся id), event subscription filter, broadcast.
- `tests/auth.rs` — token validation, PID-binding (несуществующий PID → reject, foreign-user PID → reject).

**Проверка**: `raw/ac7-tests.md` — `cargo test` output.

## Зависимости

### Внешние crates (запиненные версии)
```toml
gpui = "0.2.2"
gpui-component = "0.5.1"
tray-icon = "0.20"
global-hotkey = "0.7"          # для Phase 6, не Phase 1; добавим в Cargo.toml уже сейчас
tokio = { workspace = true, features = ["full"] }
tokio-tungstenite = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
rusqlite = { workspace = true }
ark-core = { path = "../../packages/ark-core/rust" }
windows = { version = "0.58", features = ["Win32_Security", "Win32_Security_Authorization"] }

[dev-dependencies]
criterion = "0.5"
tempfile = "3"
tokio-test = "0.4"
```

### Reused infrastructure
- `services/usage-tracker/src/singleton.rs:1-52` — копируется в `apps/kepler/src/singleton.rs`.
- `services/usage-tracker/installer/install.ps1:20-24` — паттерн HKCU Run autostart.
- `packages/ark-core/rust/src/main.rs:1-250` — JSON-RPC dispatch reuse как backend WS-сервера.
- `packages/ark-core/rust/src/sync_server.rs` — tokio-tungstenite паттерн.

### Assets
- `D:\Personal\Hobby\Coding\kosmos\kepler.png` (источник, master иконка от пользователя) → копируется в `apps/kepler/icons/master.png` при scaffold'е.
- Production иконки (16/32/64/128/256/512 px PNG + .ico) генерируются build.rs или bun-скриптом из master.

## Out of scope (НЕ делаем в Phase 1)
- Изменения в Electron-апках (Eden/Delphi/Arrancador/Horologion/Dashboard) — Phase 2-3.
- `@kosmos/ark` `KeplerTransport` mode — Phase 2.
- Launcher window, global hotkey, FTS5 search UI, quick-create — Phase 6.
- `sync_pending_objects` таблица и hold-and-replay — Phase 2.
- usage-tracker миграция на WS — Phase 4.
- LAN sync централизация (убираем `start_sync` из апок) — Phase 5.
- `ensureKeplerRunning()` helper в `@kosmos/ark` — Phase 2.
- Heart изменения — НИКОГДА (re-classified, см. план).

## Rollback

Phase 1 — dark-launch, никакого user-visible эффекта. Rollback тривиальный:
1. Если установлен HKCU Run entry — удалить через `Remove-ItemProperty`.
2. Удалить exe + lock-file + singleton DB.
3. Удалить `apps/kepler/` директорию (revert PR).

## Verification matrix

После завершения всех AC — собрать `evidence.md` со ссылками на `raw/ac1...ac7.md`. `evidence.json`:
```json
{
  "AC1": "PASS",
  "AC2": "PASS",
  "AC3": "PASS",
  "AC4": "PASS",
  "AC5": "PASS",
  "AC6": "PASS",
  "AC7": "PASS"
}
```

Если **любой** AC = FAIL — `problems.md` со списком issues, минимальный fix, reverify. План запрещает клеймить «готово» пока хоть один AC не PASS.

## Спецификация заморожена

После старта реализации этот файл **не редактируется**. Если в процессе работы окажется, что AC нужно расширить/уточнить — отдельная итерация плана (через `.agent/decisions/`).
