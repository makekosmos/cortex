# Task Spec: ark-rust-runtime

## Metadata
- Task ID: ark-rust-runtime
- Created: 2026-04-09T09:49:19+00:00
- Frozen: 2026-04-08
- Repo root: /Users/kirill/Documents/projects/kepler
- Working directory at init: /Users/kirill/Documents/projects/kepler

## Guidance sources
- `/CLAUDE.md` (repo task proof loop)
- `/apps/delphi/CLAUDE.md` (P2P sync protocol, entity models, sidecar architecture, beacon rules, single-session invariant)
- `/apps/delphi/kotlin/CLAUDE.md` (Android sync files, beacon dedup, single-session-per-device, edge-to-edge)
- `/packages/ark-core/CLAUDE.md` (Rust crate layout, current runtime status, sync invariants, TODO list for runtime migration)
- Prior task artifacts:
  - `.agent/tasks/ark-core-rust/spec.md`
  - `.agent/tasks/ark-core-rust/evidence.md`
  - `.agent/tasks/ark-core-rust/verdict.json`

## Original task statement

Полный переход Electron и Android на единый Rust-crate `ark-core` для P2P-синхронизации.
Убрать TS `@arksync/core` runtime из Electron, Kotlin `SyncServer`/`LanSyncClient`/`BroadcastDiscovery`
из Android. Всё работает через `ark-core-rpc` (stdin/stdout JSON-RPC + async events) для Electron
и через UniFFI bindings для Android. Сохранить все недавние фиксы: self-reject, stale session
eviction, beacon dedup, routable address filtering, host name как device name.

### Concrete goal

After this task lands:

- Electron Delphi (MacBook) no longer imports from `@arksync/core` at runtime — sync + beacon run inside the `ark-core-rpc` sidecar process.
- Android Delphi no longer uses `SyncServer.kt`, `LanSyncClient.kt`, `BroadcastDiscovery.kt` — sync runs through UniFFI bindings into the same Rust crate.
- The Rust crate `packages/ark-core/rust/` is the single source of truth for LAN sync on all active platforms.
- All recent TS/Kotlin fixes are preserved by design:
  - self-connect rejection in `handle_hello` (client + server)
  - stale session eviction by `device_id` on the server
  - `purge_self_peers` on start
  - beacon dedup by `device_id` with 30s TTL
  - routable address filtering (link-local / unique-local / virtual interfaces)
  - host name (OS-level) used as `device_name`, not process name

### Starting state (inspected 2026-04-08)

- `packages/ark-core/rust/src/` contains: `db.rs`, `hlc.rs`, `lib.rs`, `main.rs`, `net.rs`, `protocol.rs`, `schema.rs`, `space.rs`, `sync_client.rs`, `sync_server.rs`, `types.rs`. No `beacon.rs`. No host-name helper. `StorageBackend` trait exists in `sync_server.rs`, no `SqliteStorageBackend` implementation.
- `packages/ark-core/rust/Cargo.toml` already has `uniffi = "0.28"`, `crate-type = ["cdylib", "lib"]`, and `[[bin]] ark-core-rpc`. `lib.rs` has `uniffi::setup_scaffolding!()`. No `#[uniffi::export]` is applied anywhere yet, and the binary `main.rs` only dispatches DB operations (`init`, `load_all`, `upsert_*`, `delete_*`, `batch_upsert_todos`, `get_sync_kv`, `set_sync_kv`, `clear_all`, `delete_trashed`).
- `apps/delphi/ts/electron/sidecar.ts` spawns `ark-core-rpc`, runs a single-request/single-response JSON queue with `activeRequest`/`requestQueue`, treats every stdout line as a response, and has **no event stream handling**.
- `apps/delphi/ts/electron/main.ts` imports `SyncServer`, `SyncClient`, `LAN_SYNC_PORT`, `mergePeerRecords`, `PeerRecord`, `SyncEntity`, `PeerChange` from `@arksync/core`, and `getOwnAddresses` from `@arksync/node`. It instantiates `SyncServer` / `SyncClient` in the main process and wires `BroadcastDiscovery` from `./broadcast-discovery`.
- `apps/delphi/ts/electron/broadcast-discovery.ts` implements the Syncthing-style UDP beacon on port 21532 with a `Map<device_id, SeenPeer>` dedup, 30s TTL, and routable-address filtering.
- `apps/delphi/kotlin/app/src/main/java/com/kazui/delphi/data/sync/` contains: `SyncServer.kt` (907 LOC), `LanSyncClient.kt` (797 LOC), `BroadcastDiscovery.kt` (314 LOC), `PeerManager.kt` (409 LOC), plus helper files (`HLC.kt`, `Pairing.kt`, `ArkDiscovery.kt`, `ArkEventMapper.kt`, `ArkPeerProtocol.kt`, `PeerRecord.kt`, `ArkPeerManager.kt`, `SyncEntityParser.kt`, `ArkSyncClient.kt`). The Kotlin package root is `com.kazui.delphi` (not `com.kosmos.delphi` — the task brief's path is a typo; see Assumption A1).
- `apps/delphi/kotlin/app/build.gradle.kts` has no `jniLibs` wiring, no Rust cross-compile plugin, no UniFFI Kotlin source set.
- `packages/arksync/` (TS) still exists and is still consumed by `ts/electron/main.ts`. It becomes legacy after this task but is **not** deleted here.

---

## Acceptance criteria

All ACs describe the same final runtime state. They are grouped into phases for
build/verify ergonomics, but the verifier must judge the full set at the end —
partial completion of later phases without earlier phases is a FAIL.

### Phase 1 — Complete the Rust library

**AC1 — `beacon.rs` module exists and implements Syncthing-style UDP discovery.**
- A new module `packages/ark-core/rust/src/beacon.rs` is added and `pub mod beacon;` is declared in `lib.rs`.
- The module exposes a `BroadcastDiscovery` type (or equivalent) that:
  - Binds a UDP socket on port `LAN_SYNC_PORT + 1` = `21532`.
  - Periodically (every ~5 s) sends a JSON beacon payload with exactly these keys and types, matching the existing TS/Kotlin beacon on the wire:
    - `t`: string, beacon type, value `"delphi"`
    - `s`: string, `space_id`
    - `d`: string, `device_id`
    - `n`: string, `device_name`
    - `p`: number, WebSocket port (typically `21531`)
    - `a`: array of strings, routable addresses formatted as `ip:port` (IPv4) and `[ipv6]:port` (IPv6)
  - Broadcasts to `255.255.255.255` and to the calculated broadcast address of every enumerated IPv4 interface whose name is not rejected by `net::is_virtual_interface` and whose address passes `net::is_routable_v4`.
  - Filters outbound `a` addresses through `net::filter_routable_addresses` (or equivalent per-address check).
  - Ignores inbound beacons where `t != "delphi"`, `s != own space_id`, or `d == own device_id` (self-reject).
  - Deduplicates inbound beacons via a `HashMap<String, SeenPeer>` (or equivalent concurrent structure) keyed by `device_id`. An `on_peer_discovered` callback fires only when the `device_id` is new or its `device_name` / sorted address list changed since the last fire. Plain duplicate beacons only refresh the TTL and do not invoke the callback.
  - Evicts seen peers whose last-seen timestamp is older than `PEER_TTL = 30s`.
- The module is **covered by unit tests** that demonstrate: (a) self-reject by `device_id`, (b) dedup of repeat beacons with unchanged addresses, (c) callback re-fires when address list changes, (d) stale eviction after TTL, (e) output `a` array never contains a non-routable address.

**AC2 — Host-level address enumeration in Rust.**
- Add `fn get_own_addresses(port: u16) -> Vec<String>` as a public function reachable from `beacon.rs` and/or a new helper (`net.rs` or a dedicated `host.rs`). It uses `if-addrs` (added to `Cargo.toml`) or equivalent to enumerate host network interfaces.
- The returned list skips:
  - loopback (`127.0.0.0/8`, `::1`)
  - IPv4 link-local (`169.254.0.0/16`)
  - IPv6 link-local (`fe80::/10`)
  - IPv6 unique-local (`fc00::/7`)
  - any interface whose lowercased name starts with a prefix in `net::VIRTUAL_IFACE_PREFIXES`. At minimum this list must cover: `utun`, `awdl`, `llw`, `anpi`, `bridge`, `docker`, `br-`, `veth`, `virbr`, `vboxnet`, `vmnet`, `tun`, `tap`, `wg`, `tailscale`, `vethernet`, `vmware`, `virtualbox`, `rmnet`, `dummy`, `ap` (macOS), and `anpi`. (`net::VIRTUAL_IFACE_PREFIXES` already covers most; the spec permits adding missing ones rather than hard-pinning the exact Vec contents.)
- IPv4 addresses are formatted `a.b.c.d:<port>`. IPv6 addresses are formatted `[addr]:<port>` with any `%zone` suffix stripped.
- Unit tests cover: filtering of loopback, link-local, unique-local, virtual interface names, and IPv6 zone-id stripping.

**AC3 — `SqliteStorageBackend` implementation.**
- Closes AC12 from the prior task (`ark-core-rust`), which was FAIL.
- A type `SqliteStorageBackend` is added (in `db.rs`, `sync_server.rs`, or a new `storage.rs`) that implements the existing `#[async_trait] StorageBackend` trait from `sync_server.rs`, wrapping the synchronous `rusqlite::Connection` owned by `db.rs`.
- All four trait methods (`load_entities`, `apply_entity`, `get_kv`, `set_kv`) are implemented. Synchronous rusqlite calls must be wrapped in `tokio::task::spawn_blocking` (or equivalent) so they do not block the async runtime.
- `load_entities(vector)` returns every entity whose HLC is **either missing** from `vector` or **newer** than the HLC stored in `vector[id]` — matching the TS `StorageBackend` contract used by `sync-server.ts` and `sync-client.ts`.
- `apply_entity(entity)` performs an upsert when `entity.deleted` is not set, and a hard delete when `entity.deleted == Some(true)`, matching Assumption 4 of the prior spec.
- `get_kv` / `set_kv` delegate to `db::get_sync_kv` / `db::set_sync_kv`.
- At least one unit test exercises the roundtrip: insert entities via `apply_entity`, query them via `load_entities(empty_vector)`, and verify the returned set matches.

**AC4 — Host device-name helper.**
- A public function `get_host_device_name() -> String` is added (in `net.rs`, `host.rs`, or a new module) that:
  - On macOS: returns `hostname()` with a trailing `.local` (case-insensitive) stripped, trimmed of whitespace. Falls back to `"Ark Device"` if the call fails or the result is empty. (The Electron TS code used `"Delphi Electron"` as a fallback — the Rust fallback is intentionally more generic because the crate is reused by non-Delphi embedders.)
  - On Linux / other Unix: returns `hostname()` with the same `.local` strip and fallback.
  - On Android: returns `"Android device"` by default — the Android FFI caller (`start_sync` below) always passes an explicit `device_name`, so this default is only used if the caller omits it. **The Rust crate does NOT attempt to read `Build.MANUFACTURER`/`Build.MODEL`**; that stays on the Kotlin side.
- The function is covered by at least a smoke test (`get_host_device_name()` returns a non-empty string on the test host).

### Phase 2 — Expand `ark-core-rpc` (Electron path)

**AC5 — New sync operations in the RPC binary.**
- `packages/ark-core/rust/src/main.rs` extends the `Request` enum with at least these new variants (snake_case via existing `#[serde(tag = "operation", rename_all = "snake_case")]`):
  - `start_sync { space_id: String, device_id: String, device_name: Option<String>, port: Option<u16> }`
  - `stop_sync`
  - `broadcast_change { entity: SyncEntity }`
  - `get_connected_peers` — returns `Vec<{ device_id: String, device_name: String }>`
  - `leave_space` (stops sync, drops in-memory server/client/beacon state, purges self-peers so the next space start is clean)
- Existing DB operations (`init`, `load_all`, `upsert_*`, `delete_*`, `batch_upsert_todos`, `get_sync_kv`, `set_sync_kv`, `clear_all`, `delete_trashed`) remain wire-compatible: no field renames, no removed operations. The new variants share the same request envelope (`{"operation": "<name>", ...}`) and the same response envelope (`{"ok": true, "data": ...}` / `{"ok": false, "error": "..."}`).
- `start_sync` is idempotent: calling it when sync is already running restarts the sync layer cleanly (stops the previous instance, then starts a new one with the new parameters). It does not leak threads/tasks, leak sockets, or produce duplicate beacon senders.
- `start_sync` spawns, inside the same process: the Rust sync server (port 21531 via `sync_server`), the beacon discovery module (port 21532 via `beacon`), and outbound `SyncClient` connections for every known peer in persisted `sync.peers` sync_kv. Storage is the `SqliteStorageBackend` from AC3.
- `leave_space` makes `get_connected_peers` return an empty list and stops emitting peer-connected events until the next `start_sync`.

**AC6 — Async event stream on stdout.**
- While sync is running, the sidecar pushes JSON lines to stdout that are **not** responses to prior requests. These event lines are distinguishable from response lines by the **presence of an `event` field and the absence of an `ok` field**. Response lines keep the existing `{"ok": true|false, ...}` shape.
- At minimum these events are emitted:
  - `{"event": "peer_connected", "device_id": "...", "device_name": "..."}`
  - `{"event": "peer_disconnected", "device_id": "...", "remaining": <u32>}`
  - `{"event": "entity_changed", "entity": {...}}` — on every inbound `live_change` and on every inbound `sync_changes` entity (post-apply)
  - `{"event": "peer_list_updated", "peers": [{device_id, device_name}, ...]}` (optional but recommended)
- Events are emitted one per line, on the same stdout stream as responses. The binary never produces partial lines or interleaves event text inside a response line.
- Response ordering with respect to events is **not** guaranteed (events may be interleaved between request submissions and responses), so the Electron client must demux — see AC7.

**AC7 — `sidecar.ts` demuxes events from responses.**
- `apps/delphi/ts/electron/sidecar.ts` parses each complete stdout line and dispatches it by type:
  - If the parsed JSON has `event` field (or lacks `ok`) → route to a subscription callback, **do not** consume the `activeRequest` slot.
  - If it has `ok` → treat as a response to the current `activeRequest`, keeping the existing queue semantics.
- The sidecar exposes an `onSidecarEvent(listener)` (or equivalent) API that `main.ts` can subscribe to. Multiple subscribers are allowed; removing a subscriber does not affect others.
- The existing `MAX_QUEUE_SIZE = 500` behaviour and error propagation (`failAll`, `resetChild`) continue to work with the new stream handling, including: if the child dies with pending requests AND pending event subscriptions, both are cleaned up.
- Existing DB wrapper functions (`dbLoadAll`, `dbUpsertTodo`, etc.) continue to work unchanged against the new binary — no regression in the DB JSON-RPC path.

### Phase 3 — Electron migration

**AC8 — `main.ts` stops importing the TS sync runtime.**
- `apps/delphi/ts/electron/main.ts` removes all **runtime** imports from `@arksync/core` and `@arksync/node`. Concretely: after this task, the file must not construct, instantiate, or invoke any of: `SyncServer`, `SyncClient`, `mergePeerRecords`, `BroadcastDiscovery` (the TS class from `./broadcast-discovery`), or `getOwnAddresses`.
- Type-only imports (`import type { SyncEntity, PeerRecord } from '@arksync/core'`) are permitted if and only if those types are still needed in the renderer IPC shape. If they are removed, equivalent types must be declared locally or imported from `src/services/sync/lan-protocol.ts`.
- The module-level variables `syncServer`, `syncClients`, `broadcastDiscovery` are removed. Their roles are taken over by sidecar RPC calls and event subscriptions.
- `./broadcast-discovery.ts` is either deleted or reduced to exported helper types only (no `BroadcastDiscovery` class is instantiated from `main.ts` or anywhere else in the Electron main process).

**AC9 — Sync lifecycle goes through the sidecar.**
- `startSync(spaceId, deviceId, deviceName, seedAddresses)` in `main.ts` is rewritten to call `sidecar.request({operation: 'start_sync', space_id, device_id, device_name, ...})`. Seed addresses, if provided, are either added to the sidecar's known-peer record via a new `add_seed_peer` operation or passed as part of the `start_sync` payload — the spec does not mandate which, but `bun run dev` initial-join from a QR code must still reach the beacon MacBook.
- `lan-sync:stop` IPC calls `sidecar.request({operation: 'stop_sync'})`.
- The connection-status dot logic (`lan-sync:getStatus`) now reads from `sidecar.request({operation: 'get_connected_peers'})` instead of the TS `syncServer.getConnectedPeerEntries()` / `syncClients` map.
- `leave_space` is called when the user leaves a space so the next `start_sync` does not inherit stale peer records.

**AC10 — Renderer mutations flow through the sidecar.**
- The IPC handler for `lan-sync:broadcastChange` forwards the entity to the sidecar via `sidecar.request({operation: 'broadcast_change', entity})`. It does not call any TS `SyncServer.broadcastLiveChange` / `SyncClient.broadcastLiveChange`.
- HLC assignment for outgoing entities is performed on the Rust side (inside the sidecar). The TS renderer is responsible only for constructing the entity payload.

**AC11 — Incoming changes arrive via the event stream.**
- The sidecar event `entity_changed` is forwarded to the renderer via the existing IPC channel `lan-sync:change`. The renderer handler continues to work unchanged — the entity shape on the IPC is byte-compatible with what the TS `SyncServer.onChange` used to deliver.
- The sidecar events `peer_connected` / `peer_disconnected` are forwarded to the renderer via existing IPC channels `lan-sync:peerConnected` / `lan-sync:peerDisconnected`, with the same payload shape (`deviceId: string` for connect; `deviceId: string, remaining: number` for disconnect) so the connection-indicator dot logic keeps working without changes to the renderer side.

**AC12 — End-to-end Electron dev run passes.**
- `bun run dev` from `apps/delphi/ts/` starts Electron against the `ark-core-rpc` binary built from `packages/ark-core/rust/`. With an Android peer on the same LAN running the new UniFFI-backed build (Phase 4), the following sync flows succeed end-to-end:
  - Initial sync (both directions — first-time connection with pre-existing data on one or both sides).
  - Live updates on both sides (create, update, complete, move).
  - Delete propagation (hard delete) on both sides.
- The connection indicator dot on the Electron UI reaches the green state and reflects disconnect + reconnect correctly (yellow → green).

**AC13 — TypeScript typecheck is clean.**
- `cd apps/delphi/ts && bunx tsc --noEmit` passes without introducing any new errors or new `@ts-expect-error` / `@ts-ignore` comments compared to the pre-task baseline.
- `grep "from ['\"]@arksync/(core|node)['\"]" apps/delphi/ts/electron/` returns zero runtime imports (type-only imports may appear, but each one must be explicitly marked `import type`).

### Phase 4 — Android UniFFI migration

**AC14 — UniFFI export surface on the Rust crate.**
- The crate exposes a UniFFI surface that Android can consume. At minimum the following are exported via `#[uniffi::export]` (or equivalent UDL / proc-macro annotations):
  - `open_db(path: String)` and all DB CRUD methods currently in `db.rs` (todo / project / area / tag / heading upsert+delete, `load_all`, `batch_upsert_todos`, `get_sync_kv`, `set_sync_kv`, `clear_all`, `delete_trashed`).
  - `start_sync(config: SyncConfig)` where `SyncConfig` carries `space_id, device_id, device_name, port?, db_path?` and any other parameters needed by the caller.
  - `stop_sync()`
  - `broadcast_change(entity: SyncEntity)`
  - `get_connected_peers() -> Vec<ConnectedPeer>` where `ConnectedPeer { device_id, device_name }`.
  - A **callback / listener interface** for event delivery (peer connected, peer disconnected, entity changed). The Android side passes a Kotlin implementation of this interface at `start_sync` time; the Rust side invokes it on every corresponding event.
- All exported types are `uniffi`-friendly (no borrowed references, no generics over non-primitive types). The `SyncEntity` export uses owned `String` + `JsonObject` / `HashMap<String, JsonValue>` representation consistent with the existing `types.rs` definition.
- `cargo build` on the workspace succeeds with the new `#[uniffi::export]` surface, both in `--lib` and in the existing binary target.

**AC15 — Kotlin bindings generate and compile.**
- Running `uniffi-bindgen generate` (or the equivalent build-script / gradle task) against the built cdylib produces a Kotlin source file for the bindings (name and location not strictly mandated, but it must land somewhere under `apps/delphi/kotlin/app/src/main/java/` or an equivalent source set referenced by `app/build.gradle.kts`).
- The Rust cdylib is cross-compiled for **at least** the `arm64-v8a` Android ABI. `armeabi-v7a` and `x86_64` are nice-to-have; missing them is not a FAIL by itself, but they must not be referenced in `jniLibs/` if they aren't actually built.
- `apps/delphi/kotlin/app/build.gradle.kts` (or a module-level script it includes) wires the Rust shared library into `jniLibs/<ABI>/libark_core.so` (or equivalent) and includes the generated Kotlin source in the build.
- The spec does not pin the exact toolchain approach — cargo-ndk, cross, a Gradle plugin (`mozilla/rust-android-gradle`), or a local shell script are all acceptable. It does not pin specific Gradle task names or Kotlin method names.

**AC16 — Android sync classes removed or reduced to facades.**
- `apps/delphi/kotlin/app/src/main/java/com/kazui/delphi/data/sync/SyncServer.kt`, `LanSyncClient.kt`, and `BroadcastDiscovery.kt` are either:
  - **Deleted entirely** (preferred), OR
  - Reduced to thin Kotlin facades that contain no protocol logic (no `hello`, no `version_vector`, no `batch_upsert`, no WebSocket lifecycle, no UDP beacon lifecycle) and delegate every operation to the UniFFI entry points from AC14.
- `PeerManager.kt` may stay as a Kotlin coroutine coordinator that bridges the UniFFI event callback into `StateFlow`s consumed by the UI, but it must not contain protocol logic either. Every version vector computation, batch split, HLC tick, beacon send, and peer dedup happens in Rust.
- After this change, `grep "class (SyncServer|LanSyncClient|BroadcastDiscovery)" apps/delphi/kotlin/app/src/main/java/` returns **zero class declarations**, or only delegating facades whose bodies contain only UniFFI calls + logging.

**AC17 — Android build succeeds.**
- `cd apps/delphi/kotlin && ./gradlew :app:compileDebugKotlin` passes without errors. A full `./gradlew :app:assembleDebug` succeeds end-to-end and produces an APK under `app/build/outputs/apk/debug/`.
- The produced APK bundles the Rust shared library for at least `arm64-v8a` and the generated UniFFI Kotlin source.

**AC18 — Android runtime parity on real device.**
- With Delphi installed on the Nothing A063 phone (or equivalent arm64 Android device), the app:
  - Discovers the Electron MacBook via UDP beacon (port 21532) on the same LAN.
  - Completes the `hello` handshake and enters sync mode.
  - Performs initial sync (both directions) and live sync (both directions).
  - Deduplicates by `device_id` in the UI peer list (inbound + outbound connections do not double-count).
  - Advertises only routable addresses in its beacons and `hello` messages.
  - Appears to the MacBook peer under the OS-level device name (`${Build.MANUFACTURER} ${Build.MODEL}` passed through by the Kotlin caller).
- If a real-device run is infeasible at verification time, the verifier may accept the combined proof of AC15 + AC17 + AC21 (test harness run) + a stub UniFFI callback unit test that demonstrates the event delivery path is hooked up. In that case the verdict for AC18 is `PASS (deferred-device-test)` with an explicit note.

### Phase 5 — Behavioural parity and invariants

**AC19 — Invariants enforced in Rust (and covered by tests).**
The Rust crate enforces, and unit or integration tests verify, every invariant below:

- (a) **Self-connect rejection in `handle_hello`** on both `sync_server.rs` and `sync_client.rs`: receiving a `hello` whose `device_id` matches our own `device_id` closes the connection immediately and does not emit a `peer_connected` event. Test must construct a hello with a matching device_id and assert rejection.
- (b) **Single-session-per-device_id on the server**: when a new `hello` authenticates with a `device_id` that already has another authenticated session, the older session is evicted (its `authenticated` flag cleared, then the WS closed with a `superseded`-equivalent reason) before the new session is accepted. A test must simulate the race and assert the server ends with exactly one session per device_id.
- (c) **`knownPeerRecords` purged of self-references on `start()`** and on every peer-list merge / `update_peer_record` call: any stored record whose `device_id == our own` or whose every address is in our own address set is dropped. Test for both paths.
- (d) **Beacon dedup by `device_id` with 30s TTL**: duplicate beacons with unchanged addresses do not fire `on_peer_discovered`. After 30s without a beacon, the seen record is evicted and the next beacon fires the callback again. Tests already listed in AC1.
- (e) **Routable addresses only**: every address placed into an outbound `hello`, `peer_list`, or beacon `a` array passes `net::is_address_routable`. Any address failing the check must be silently dropped at assembly time. Test via a fixture that includes at least one of each class (loopback, link-local, unique-local, virtual interface) and asserts they are absent from the outbound payload.
- (f) **Device name defaults to OS host name**, never a process name like `"Delphi Electron"`, `"ark-core-rpc"`, etc. Test: starting the sidecar without an explicit `device_name` results in a hello that carries the OS host name (strip `.local`).

**AC20 — Integration / end-to-end harness.**
- At least one automated integration test exists that exercises the Electron-to-Android sync path end-to-end, in one of these forms:
  - **Preferred**: a Rust integration test or binary harness that boots two `ark-core-rpc` processes (or two in-process sync layers), connects them over loopback using routable addresses, and verifies a full protocol round-trip: `hello` → `peer_list` → `version_vector` exchange → one `sync_changes` batch with ACK → one `live_change` → one `live_ack` → clean shutdown. Entities must be applied to the receiver side and verifiable via `load_entities`.
  - **Alternative**: if true dual-process is infeasible in CI, a single-process test that drives `SyncServer` + `SyncClient` with two distinct `device_id`s against an in-memory `SqliteStorageBackend` is acceptable, provided it covers the same message sequence.
- The test must explicitly assert the self-connect rejection path (start a client with `device_id == server_device_id` and assert the connection is closed without entering sync).

**AC21 — Cargo test + cargo build green with new modules.**
- `cd packages/ark-core/rust && cargo build` succeeds with zero warnings (excluding UniFFI-generated code warnings out of our control).
- `cd packages/ark-core/rust && cargo test` passes, including all new tests from AC1–AC4, AC19, AC20. The prior 59 tests continue to pass.
- `cd packages/ark-core/rust && cargo clippy -- -D warnings` passes (matching the prior task's lint bar).

---

## Constraints

1. Rust 2021 edition. No `unsafe` outside FFI boundaries (UniFFI-generated code is exempt).
2. Do **not** delete `packages/arksync/` files in this task. They are marked "legacy, not loaded at runtime". Removing them is a separate cleanup task.
3. Do **not** break the current Delphi UI, Pinia store, Room DAO, Hilt modules, or on-disk DB data. Storage format must stay identical — todos, projects, areas, tags, headings, sync_kv schema is untouched beyond what AC3 already requires (which is no schema change).
4. Do **not** change the wire protocol between peers. The protocol is cross-version: TS-built arksync and Rust-built ark-core must remain byte-compatible on every message type. All JSON field names stay snake_case, `SyncEntity.type` stays serialized as `"type"`, `deleted` stays `skip_serializing_if`, HLC format stays `<ISO8601>:<counter:06d>:<device_id>`.
5. All already-applied recent fixes must be preserved. A port that regresses any of the following is a FAIL:
   - self-connect rejection in `handle_hello`
   - single-session-per-device eviction
   - `purge_self_peers` on start
   - beacon dedup by device_id
   - routable address filtering in hello / peer_list / beacons
   - host name (OS-level) as `device_name`
6. `ark-core-rpc` JSON-RPC request/response envelope stays the same:
   - Request: `{"operation": "<snake_case>", ...params}`
   - Response: `{"ok": true, "data": ...}` or `{"ok": false, "error": "..."}`
   - New sync operations (AC5) join the same enum. No existing operation is renamed, removed, or has its field names changed.
7. The new event stream (AC6) shares the same stdout as responses but must never corrupt a response line or a prior event line. One JSON object per line is the only permitted framing.
8. Android package path is `com.kazui.delphi` (not `com.kosmos.delphi`). The implementer must target the real path; see Assumption A1.
9. Dependencies added to `Cargo.toml` must be justified:
   - `if-addrs` (or equivalent) for interface enumeration — required by AC2.
   - The UniFFI toolchain version already in `Cargo.toml` (`uniffi = "0.28"`) is the baseline. If a newer version is required for Kotlin export of trait objects / callback interfaces, it may be upgraded, but not downgraded.
   - No runtime dependency on a relay server, STUN/TURN, or any network service outside the LAN.
10. No HMAC auth layer on the wire. Authentication remains implicit `space_id` match, same as the current behaviour and Assumption 1 of the prior spec.

---

## Non-goals

1. **No macOS Swift migration.** `apps/delphi/swift-archive/` stays archived. The spec does not require a Swift UniFFI surface, though the Rust UniFFI export must not make such a surface impossible to add later.
2. **No Delphi UI refactor.** Vue components, Pinia store, Hilt modules, Compose screens, and renderer IPC channel names (`lan-sync:change`, `lan-sync:peerConnected`, etc.) stay the same beyond the minimum wiring changes required to swap the sync layer. No redesign of the connection indicator, no new settings screen.
3. **No HMAC auth layer.** `peer-protocol.ts` HMAC handshake stays defined-but-unused, as in the prior spec.
4. **No relay server. No WAN NAT traversal.** The sync path remains LAN-only, same as the current behaviour.
5. **No deletion of `packages/arksync/`.** Legacy but present.
6. **No changes to the JSON-RPC envelope format** for existing DB operations. The only wire-format change is additive: new operations + a new event kind of stdout line.
7. **No migration of the local SQLite database format.** `db.rs` schema is unchanged.
8. **No change to the UDP beacon wire format.** The JSON keys `{t, s, d, n, p, a}` stay; Rust-side beacons must interoperate with existing TS / Kotlin beacons from any peer that hasn't upgraded yet in a transition window.

---

## Assumptions

1. **A1 — Android package path.** The task brief references `apps/delphi/kotlin/app/src/main/java/com/kosmos/delphi/...`, but the real path is `com.kazui.delphi` (confirmed by listing `app/src/main/java/`). All Android ACs target the real path. If a future rename lands, ACs carry over unchanged.
2. **A2 — UniFFI version.** `uniffi = "0.28"` in `Cargo.toml` is assumed sufficient for Kotlin callback-interface export. If proc-macro export of trait-based listeners hits a known limitation, the implementer may bump to the next stable UniFFI release without re-opening the spec.
3. **A3 — Rust host device-name fallback.** On macOS the function strips `.local` and otherwise returns the raw hostname. On Android the Kotlin caller always provides an explicit `device_name`, so the Rust-side default (`"Android device"`) is a safety net, not the observed value in production.
4. **A4 — Event envelope.** Events are distinguished from responses by the **presence of an `event` field**, not by a separate channel. `main.ts` (via `sidecar.ts`) demuxes on this field. If a future task needs richer event metadata, it can extend the envelope without breaking the demux.
5. **A5 — Seed-address bootstrap.** QR-code join still works: either `start_sync` accepts an optional `seed_addresses` parameter, or a new `add_seed_peer` operation is added. Either is acceptable; the spec does not pin which.
6. **A6 — Integration test scope.** If the dual-sidecar integration test from AC20 cannot be run in CI (sandbox restrictions), the alternative single-process test variant is explicitly allowed and still earns PASS.
7. **A7 — No UI-visible behaviour change beyond sync being "the same but Rust-backed".** The connection indicator, peer list, settings screen, and quick-add bar behave identically from the user's perspective.
8. **A8 — Test bar.** "All tests pass" = `cargo test` green, `bunx tsc --noEmit` clean, `./gradlew :app:compileDebugKotlin` green. No new clippy warnings.
9. **A9 — TS sync runtime is not imported elsewhere.** This task only requires `ts/electron/main.ts` to be free of runtime imports from `@arksync/core` / `@arksync/node`. Other files that still import runtime sync types (e.g. the renderer-side `src/services/sync/lan-protocol.ts` for local type definitions) are out of scope for deletion.

---

## Verification plan

A fresh verifier must run every step and judge against the current repository state, not against any narrative claims in this file or in `evidence.md`.

### Build checks

- `cd packages/ark-core/rust && cargo build` — zero errors, zero warnings (AC21).
- `cd packages/ark-core/rust && cargo build --release` — succeeds (sanity check for release-mode binary used by Electron dev).
- `cd packages/ark-core/rust && cargo clippy -- -D warnings` — passes.
- `cd apps/delphi/ts && bunx tsc --noEmit` — no new errors (AC13).
- `cd apps/delphi/kotlin && ./gradlew :app:compileDebugKotlin` — succeeds (AC17).
- `cd apps/delphi/kotlin && ./gradlew :app:assembleDebug` — produces a debug APK that contains `libark_core.so` for at least `arm64-v8a`.

### Unit tests

- `cd packages/ark-core/rust && cargo test` — all tests pass, including the new ones specified in AC1 (beacon), AC2 (address enumeration), AC3 (SqliteStorageBackend), AC4 (host name), AC19 (invariants), AC20 (integration).
- Test count is strictly greater than the prior task's 59 (the new ACs add tests; the implementer must not remove existing ones).

### Integration tests

- The harness from AC20 runs and passes. It must print a final success line that the verifier can grep for (e.g. `INTEGRATION OK`), and the test must cover at least: hello + version_vector exchange, one sync_changes batch + ACK, one live_change + live_ack, self-connect rejection.

### Lint / grep guards

- `rg "from ['\"]@arksync/core['\"]" apps/delphi/ts/electron/main.ts` — zero **runtime** imports. (Type-only `import type` lines are allowed.)
- `rg "from ['\"]@arksync/node['\"]" apps/delphi/ts/electron/main.ts` — zero.
- `rg "new (SyncServer|SyncClient|BroadcastDiscovery)\(" apps/delphi/ts/electron/` — zero instantiations in the Electron main process.
- `rg -n "class (SyncServer|LanSyncClient|BroadcastDiscovery)" apps/delphi/kotlin/app/src/main/java/com/kazui/delphi/data/sync/` — zero class declarations OR only delegating facades whose method bodies contain only UniFFI calls + logging.
- `rg -n "'Delphi Electron'|\"Delphi Electron\"|'ark-core-rpc'" apps/delphi/ts/electron/main.ts packages/ark-core/rust/src/` — zero process-name literals used as device name (the `getHostDeviceName` fallback string is permitted only inside the fallback branch).

### Manual / runtime checks

- `bun run dev` from `apps/delphi/ts/` boots Electron, and the `ark-core-rpc` child process appears in the Electron process tree. Killing it forces Electron to recover (this is implicit in the existing `failAll` path; no regression allowed).
- Running Delphi on the Nothing A063 phone and Electron on the MacBook on the same Wi-Fi: both devices discover each other via beacon, the connection indicator reaches the green state on both sides, and sync flows from AC12 + AC18 succeed.
- The MacBook's peer list shows the Android device under its OS-level name (e.g. `Nothing A063`), and vice versa shows the MacBook under its OS hostname with `.local` stripped.
- Creating a todo on the Android side appears on the Electron side within ~1 second and vice versa. Deleting a todo on either side propagates.
- Killing sync on one side and restarting it does not leave ghost peers in the other side's list (single-session-per-device eviction).
- Unplugging one side from the network for > 30 s and reconnecting causes the peer to be rediscovered via beacon and the connection indicator returns to green without a manual restart.

### Regression guards

- Prior DB JSON-RPC operations (`init`, `load_all`, `upsert_todo`, `delete_todo`, `batch_upsert_todos`, `upsert_project`, `delete_project`, `upsert_area`, `upsert_tag`, `upsert_heading`, `delete_heading`, `get_sync_kv`, `set_sync_kv`, `clear_all`, `delete_trashed`) continue to work unchanged from Electron. Verified by a smoke test that loads all entities via `dbLoadAll()`, inserts one todo via `dbUpsertTodo`, and re-loads.
- Space switching via `dbSwitchSpace(spaceId)` continues to work — after a switch, `start_sync` on the new DB does not bleed peers from the old DB.
- `sync.peers` sync_kv entry still persists known peers across sidecar restarts (Rust side now owns the load/save).

---

## Out of scope for this task (reminder)

- Deleting `packages/arksync/`.
- Deleting unused `ts/src/services/sync/` files that are no longer referenced.
- Rewriting the Electron connection indicator, the Pinia store, or any Compose screen.
- Rewriting peer-manager.ts (unrelated Nostr / pairing flow — out of scope).
- macOS Swift build.
- HMAC auth on the wire.
- WAN / relay sync.

If a verifier finds residual references to the legacy TS / Kotlin sync code in files outside the ones listed in AC8 and AC16, that is a future-cleanup note, not a FAIL.
