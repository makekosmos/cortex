# Spec: ark-p2p-rust-backend

**Task ID**: `ark-p2p-rust-backend`
**Frozen**: 2026-04-09
**Status**: FROZEN (do not edit after implementation begins)

---

## Original task statement

Финальный переход на единый Rust-бекенд для всей P2P-сети Kepler. Rust crate `ark-core` становится источником истины для всего сетевого стека: LAN + Relay транспорты, event log, HLC/LWW, beacon discovery, storage, protocol messages. Делается Rust-relay сервер (`packages/ark-relay-server`) как отдельный binary под будущий деплой на VPS. UniFFI-биндинги для Kotlin (Android runtime должен реально работать — фиксится silent startSync баг) и Swift (scaffold для будущего macOS). Electron использует ark-core-rpc sidecar. Ergonomic `@arksync/node` TypeScript-обёртка спавнит sidecar и прячет IPC. Все legacy файлы удаляются. Smoke test: Delphi Electron (MacBook) ↔ Delphi Android (Nothing A063) реально синкаются end-to-end через единый Rust backend.

---

## Codebase baseline (as of freeze)

### What already exists and works

- `packages/ark-core/rust/src/` — full Rust crate with: `ffi.rs` (UniFFI ArkCore facade), `main.rs` (ark-core-rpc JSON-RPC binary), `sync_server.rs`, `sync_client.rs`, `beacon.rs` (UDP discovery), `db.rs`, `hlc.rs`, `protocol.rs`, `net.rs`, `host.rs`, `space.rs`, `types.rs`
- `ArkCore::new()` already creates a `new_multi_thread()` tokio runtime. The runtime itself is correct. The bug is that `start_sync()` calls `runtime.block_on(...)` which blocks the JNI calling thread on Android.
- `ArkCore::start_sync()` already runs the full LAN sync setup inside `block_on` — all server/client/beacon wiring is there.
- `apps/delphi/ts/electron/sidecar.ts` — full sidecar client with sync wrappers (`syncStart`, `syncStop`, etc.)
- `apps/delphi/ts/electron/main.ts` — Electron main imports from `sidecar.ts` for sync ops but also imports `PeerManager` from `./peer-manager` and types from `@arksync/core`
- `apps/delphi/kotlin/.../data/sync/PeerManager.kt` — already rewritten to use ArkCore UniFFI; calls `arkCore.startSync(FfiSyncConfig(...), listener)` directly
- Legacy TS sync files still present: `peer-manager.ts`, `peer-server.ts`, `peer-discovery.ts`, `broadcast-discovery.ts`
- `packages/arksync/` still present; `main.ts` still imports `PeerManager` from `./peer-manager` and `@arksync/core`

### Key wire format constraints

- `LanSyncMessage` JSON format in `packages/ark-core/rust/src/protocol.rs` is the authoritative wire format
- Field names and message type strings must NOT change — must remain binary-compatible with existing TS arksync code
- Sync port: 21531 (TCP WebSocket), Beacon port: 21532 (UDP broadcast)

---

## Acceptance Criteria

### Phase A — Rust backend completion

**AC1 — Android startSync non-blocking fix**
`ArkCore::start_sync()` in `packages/ark-core/rust/src/ffi.rs` must NOT call `runtime.block_on()` on the JNI calling thread. Fix: spawn the async setup body onto a dedicated OS thread (e.g. `std::thread::spawn` + `runtime.block_on` inside that thread) and synchronize back to the caller via a `std::sync::mpsc::channel` or `std::sync::OnceLock`. After the fix, calling `arkCore.startSync(...)` from Kotlin's `Dispatchers.IO` or main thread must not produce a 37-frame jank; ports 21531/21532 must bind within 3 seconds of the call. The public signature `start_sync(config: FfiSyncConfig, listener: Box<dyn ArkEventListener>) -> Result<bool, ArkCoreError>` remains unchanged.

Verification: `cargo build --release` in `packages/ark-core/rust/` passes; `./gradlew compileDebugKotlin` passes; on real device `adb shell cat /proc/net/tcp` shows port `5403` within 3 seconds of app start.

**AC2 — Relay transport module**
File `packages/ark-core/rust/src/relay_transport.rs` exists and `lib.rs` includes `pub mod relay_transport`. It implements:
- Outbound WebSocket client connecting to `wss://<host>/ws?space_id=X&device_id=Y&api_key=Z`
- Uses `LanSyncMessage` JSON wire format (same as LAN transport)
- Offline outbox: `VecDeque<LanSyncMessage>` drained on reconnect
- Exponential backoff reconnect: 1s → 2s → 4s → ... capped at 60s
- On relay peer connected/disconnected: fires the same `ArkEventListener` callbacks (`on_peer_connected`, `on_peer_disconnected`) as LAN transport

Verification: `cargo build --release` in `packages/ark-core/rust/` succeeds with no errors.

**AC3 — Mesh coordinator module**
File `packages/ark-core/rust/src/mesh.rs` exists and compiles. It contains:
- `MeshConfig { lan_enabled: bool, relay_url: Option<String>, relay_api_key: Option<String> }`
- A coordinator managing both LAN (sync_server + sync_client + beacon) and relay transport simultaneously
- Deduplication of incoming changes by `(device_id, entity_id, hlc)` so a change arriving from both transports is processed only once
- `FfiSyncConfig` in `ffi.rs` gains two new optional fields: `relay_url: Option<String>` and `relay_api_key: Option<String>` (both default to `None`; existing Kotlin callers need no changes)
- `StartSync` request enum in `main.rs` gains the same optional fields

Verification: `cargo build --release` succeeds; existing `FfiSyncConfig` construction in `PeerManager.kt` compiles without modification.

**AC4 — ark-relay-server binary**
Directory `packages/ark-relay-server/` exists with:
- `Cargo.toml` declaring a `[[bin]]` target named `ark-relay-server`; dependencies include `tokio`, one of `axum`/`warp`/`tokio-tungstenite`, `rusqlite`, `serde_json`
- `src/main.rs` implementing a WebSocket server with endpoint `GET /ws?space_id=X&device_id=Y&api_key=Z`
- Routes `LanSyncMessage` JSON frames between all devices sharing the same `space_id`
- Persists event log to SQLite with columns `(space_id, device_id, entity_json, hlc, stored_at)`
- New devices connecting receive a catch-up replay of all stored entities for their space_id
- API key validation: only clients with matching `api_key` query param are accepted (configured via env var or CLI argument)

Verification: `cargo build --release` in `packages/ark-relay-server/` succeeds; `cargo test` passes.

**AC5 — Swift UniFFI scaffold**
Directory `packages/ark-core/swift/` exists containing:
- Generated Swift bindings from `packages/ark-core/rust/src/ark_core.udl` (files produced by `uniffi-bindgen generate --language swift`)
- File `packages/ark-core/swift/ArkCoreExample.swift` — a minimal Swift snippet that creates `ArkCore()`, calls `startSync(config:listener:)`, and prints events
- No Xcode project file is present

Verification: Files exist at the stated path; no `.xcodeproj` or `.xcworkspace` present.

**AC6 — Integration test: relay round-trip**
File `packages/ark-core/rust/tests/relay_round_trip.rs` exists and the test passes. The test:
- Starts a local relay server (in-process tokio task or subprocess)
- Creates two ArkCore instances with `relay_url` pointing to the local server and the same `space_id`
- Instance A calls `broadcast_change_json` with a test entity
- Asserts that Instance B's `ArkEventListener::on_entity_changed` fires with the matching entity within 5 seconds

Verification: `cargo test --test relay_round_trip` in `packages/ark-core/rust/` passes.

---

### Phase B — @arksync/node TypeScript wrapper

**AC7 — @arksync/node package exists**
Directory `packages/arksync-node/` exists with `package.json` declaring `"name": "@arksync/node"` and a build script.

**AC8 — ArkClient class is complete and type-correct**
`packages/arksync-node/` exports class `ArkClient` with:
- Constructor: `new ArkClient({ spaceId, deviceId, deviceName?, port?, relayUrl?, relayApiKey?, sidecarPath })`
- `start(): Promise<void>` — spawns `ark-core-rpc` at `sidecarPath`, sends `start_sync` RPC, subscribes to the event stream
- `stop(): Promise<void>` — sends `stop_sync`, kills the sidecar process
- `broadcastChange(entityType: string, entityId: string, data: Record<string, unknown>, deleted?: boolean): Promise<void>`
- `onPeerConnected(cb: (deviceId: string, deviceName: string) => void): () => void` — returns unsubscribe fn
- `onPeerDisconnected(cb: (deviceId: string, remaining: number) => void): () => void`
- `onEntityChanged(cb: (entityJson: string) => void): () => void`
- `getConnectedPeers(): Promise<Array<{ device_id: string; device_name: string }>>`

Verification: `bun run build` (or `tsc --noEmit`) in `packages/arksync-node/` produces 0 TypeScript errors.

**AC9 — Delphi Electron uses @arksync/node**
- `apps/delphi/ts/package.json` includes `"@arksync/node": "workspace:*"` in dependencies
- `apps/delphi/ts/electron/main.ts` imports sync lifecycle (`start`, `stop`, `broadcastChange`, event callbacks) from `@arksync/node`
- `main.ts` does NOT import from `./peer-manager`, `./sync-server`, `./sync-client`, `./peer-discovery`, `./broadcast-discovery`, or `@arksync/core`
- DB operations (`dbLoadAll`, `dbUpsertTodo`, etc.) remain in `sidecar.ts` — that file is NOT deleted

Verification: `bun run build` in `apps/delphi/ts/` exits 0 with no TypeScript errors.

---

### Phase C — Delete all legacy sync code

**AC10 — Legacy Electron TS sync files deleted**
The following files do NOT exist in the repository:
- `apps/delphi/ts/electron/peer-manager.ts`
- `apps/delphi/ts/electron/peer-server.ts`
- `apps/delphi/ts/electron/peer-discovery.ts`
- `apps/delphi/ts/electron/broadcast-discovery.ts`
- Any file named `ark-client.ts` under `apps/delphi/ts/`

And no remaining file in `apps/delphi/ts/` imports from these paths.

Verification: `ls apps/delphi/ts/electron/peer-manager.ts` returns "No such file"; `grep -r "from './peer-manager'" apps/delphi/ts/` returns empty.

**AC11 — Legacy Kotlin sync files deleted**
The following files do NOT exist:
- `apps/delphi/kotlin/.../data/sync/ArkSyncClient.kt`
- `apps/delphi/kotlin/.../data/sync/ArkPeerManager.kt`
- `apps/delphi/kotlin/.../data/sync/ArkPeerProtocol.kt`
- `apps/delphi/kotlin/.../data/sync/ArkEventMapper.kt`

Verification: `./gradlew compileDebugKotlin` exits BUILD SUCCESS; `grep -r "ArkSyncClient\|ArkPeerManager\|ArkPeerProtocol\|ArkEventMapper" apps/delphi/kotlin/` returns empty.

**AC12 — packages/arksync deleted; zero @arksync/core imports remain**
- `packages/arksync/` directory does not exist
- `packages/arksync` is removed from the root `package.json` workspaces array
- `grep -r "@arksync/core" apps/ packages/` returns empty output

Verification: The grep command exits 1 (no matches).

---

### Phase D — End-to-end smoke test

**AC13 — Electron starts without errors**
`bun run dev` in `apps/delphi/ts/` starts the Electron window without crashing; `ps aux | grep ark-core-rpc` shows the sidecar process; no import errors in the Electron console.

**AC14 — Android APK builds and binds ports**
`./gradlew assembleDebug` in `apps/delphi/kotlin/` exits BUILD SUCCESS. After APK install and Space activation on a real device:
- `adb logcat -s PeerManager` shows "Starting ark-core sync"
- `adb shell cat /proc/net/tcp` shows port `5403` (hex for 21531) within 5 seconds
- `adb shell cat /proc/net/udp` shows port `5404` (hex for 21532) within 5 seconds

**AC15 — Electron → Android sync**
With both running on the same LAN: adding a task in the Electron app causes it to appear on Android within 5 seconds (confirmed via `adb logcat` showing `onEntityChanged` with matching entity id).

**AC16 — Android → Electron sync**
Adding a task on Android causes it to appear in the Electron app within 5 seconds (confirmed via Electron DevTools console showing `entity_changed` event with matching entity id).

---

### Phase E — Documentation

**AC17 — packages/ark-core/CLAUDE.md updated**
The "Runtime status" section in `packages/ark-core/CLAUDE.md` reflects that:
- Sync layer is LIVE (not library-only)
- Beacon is in Rust (`beacon.rs`)
- Relay transport exists (`relay_transport.rs`, `mesh.rs`)
- The TODO block for runtime transition is removed or marked complete

**AC18 — Delphi CLAUDE.md files updated**
`apps/delphi/CLAUDE.md` and `apps/delphi/kotlin/CLAUDE.md` sync file tables no longer reference deleted legacy files; they reference `@arksync/node`, `relay_transport.rs`, and `mesh.rs` where applicable.

---

## Constraints

1. Do NOT change the `LanSyncMessage` wire format (field names, message type strings, JSON structure) — binary-compatible with existing TS arksync peers.
2. Do NOT break any existing DB operations in `ark-core-rpc` (`init`, `load_all`, `upsert_todo`, `delete_todo`, `batch_upsert_todos`, `upsert_project`, `delete_project`, `upsert_area`, `upsert_tag`, `upsert_heading`, `delete_heading`, `get_sync_kv`, `set_sync_kv`, `clear_all`, `delete_trashed`).
3. Do NOT change Android `applicationId` (`com.kosmos.ark.data` for ark-service; `com.kazui.delphi` for Delphi).
4. Keep UniFFI `ArkCore` API backward-compatible: existing Kotlin call sites (`arkCore.startSync(FfiSyncConfig(...), listener)`, `arkCore.stopSync()`, `arkCore.broadcastChangeJson(...)`, `arkCore.getConnectedPeers()`, `arkCore.addSeedPeer(...)`) must compile without modification.
5. Android minSdk 28, arm64-v8a only.
6. Relay server: no auth beyond api_key query param matching.
7. `sidecar.ts` in Delphi Electron is NOT deleted — it retains all DB operation wrappers. Only sync lifecycle moves to `@arksync/node`.
8. Do NOT move DB storage from `ark-core-rpc` sidecar to a separate process.

---

## Non-goals

- React Native / Olympia / Elysium / Eden migration
- macOS SwiftUI Xcode project (only Swift UniFFI scaffold files required)
- Full relay server authentication or end-to-end encryption
- Horizontal relay server scaling or multi-tenant features
- Moving DB storage out of `ark-core-rpc`
- Any UI changes beyond removing legacy sync import sites

---

## Assumptions (narrowly resolved)

**A1 — startSync fix approach**: Use `std::thread::spawn` containing `runtime.block_on(async_setup_future)` and communicate the result back via `std::sync::mpsc`. The public signature stays synchronous. The spawned OS thread is separate from the JNI caller thread, so the caller is unblocked immediately (or after a brief channel wait). Alternative: use a `tokio::runtime::Handle::spawn` + block on a oneshot inside the calling thread but outside tokio context — both are acceptable.

**A2 — relay_transport.rs surface**: Relay transport is not exposed via UniFFI directly. It is wired through `mesh.rs` and invoked by `ArkCore::start_sync()` when `relay_url` is `Some(...)`. `FfiSyncConfig` relay fields are optional with `None` defaults.

**A3 — relay round-trip test isolation**: The integration test may run the relay server as an in-process tokio task (not a subprocess) to avoid cross-binary dependencies at test time.

**A4 — @arksync/node sidecar path**: `ArkClient` accepts explicit `sidecarPath: string`. Delphi Electron `main.ts` computes the path using the same logic as `sidecar.ts::getSidecarBinaryPath()` and passes it in. `sidecar.ts` continues to use its own internal path resolution for DB operations.

**A5 — Swift bindings generation**: Generated Swift files are committed to the repo under `packages/ark-core/swift/` so no toolchain is needed at consumer build time.

**A6 — Deleted files**: Files are physically removed (not emptied). All import sites are updated before deletion.

---

## Verification plan (ordered)

```bash
REPO=/Users/kirill/Documents/projects/kepler

# 1. Rust crate (ark-core)
cd "$REPO/packages/ark-core/rust" && cargo build --release
cd "$REPO/packages/ark-core/rust" && cargo test

# 2. Relay round-trip integration test
cd "$REPO/packages/ark-core/rust" && cargo test --test relay_round_trip

# 3. Relay server
cd "$REPO/packages/ark-relay-server" && cargo build --release
cd "$REPO/packages/ark-relay-server" && cargo test

# 4. TypeScript wrapper
cd "$REPO/packages/arksync-node" && bun run build

# 5. Electron app
cd "$REPO/apps/delphi/ts" && bun run build

# 6. Android
cd "$REPO/apps/delphi/kotlin" && ./gradlew compileDebugKotlin

# 7. Zero legacy imports
grep -r "@arksync/core" "$REPO/apps/" "$REPO/packages/"
grep -r "ArkSyncClient\|ArkPeerManager\|ArkPeerProtocol\|ArkEventMapper" "$REPO/apps/delphi/kotlin/"
grep -rn "from './peer-manager'\|from './peer-server'\|from './peer-discovery'\|from './broadcast-discovery'" "$REPO/apps/delphi/ts/"

# 8. Swift scaffold existence
ls "$REPO/packages/ark-core/swift/ArkCoreExample.swift"

# 9. Smoke test (manual, real hardware)
# MacBook: bun run dev in apps/delphi/ts  →  Electron window opens, sidecar spawns
# Android: ./gradlew installDebug  →  adb logcat -s PeerManager shows sync start
#          adb shell cat /proc/net/tcp  →  port 5403 present
#          adb shell cat /proc/net/udp  →  port 5404 present
# Electron → Android: add task in Electron, confirm on Android within 5s  (AC15)
# Android → Electron: add task on Android, confirm in Electron within 5s  (AC16)
```

### AC summary table

| AC | Phase | How verified |
|----|-------|-------------|
| AC1 | A1 | cargo build + ./gradlew compileDebugKotlin + manual adb check |
| AC2 | A2 | cargo build --release (ark-core) |
| AC3 | A3 | cargo build --release + ./gradlew compileDebugKotlin |
| AC4 | A4 | cargo build --release (relay-server) + cargo test |
| AC5 | A5 | file existence: ls packages/ark-core/swift/ |
| AC6 | A6 | cargo test --test relay_round_trip |
| AC7 | B1 | file existence: ls packages/arksync-node/package.json |
| AC8 | B1 | bun run build (arksync-node) |
| AC9 | B2 | bun run build (delphi/ts) |
| AC10 | C1 | file absence + grep no import |
| AC11 | C2 | ./gradlew compileDebugKotlin + grep |
| AC12 | C3 | grep -r "@arksync/core" returns empty |
| AC13 | D1 | manual: bun run dev + ps |
| AC14 | D2 | ./gradlew assembleDebug + manual adb |
| AC15 | D1 | manual: real device sync Electron→Android |
| AC16 | D2 | manual: real device sync Android→Electron |
| AC17 | E1 | file content check |
| AC18 | E1 | file content check |
