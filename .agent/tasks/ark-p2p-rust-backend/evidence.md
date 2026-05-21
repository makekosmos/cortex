# Evidence: ark-p2p-rust-backend

**Date**: 2026-04-09  
**Overall**: PASS (AC1-AC12, AC17-AC18 verified; AC13-AC16 require hardware)

---

## AC1 — Android startSync non-blocking fix

**Status**: PASS

`ffi.rs` implements `start_sync()` using `std::thread::Builder::new().name("ark-sync-setup").spawn(...)` + `std::sync::mpsc::channel`. The JNI thread blocks only until ports are bound (result_rx.recv_timeout(10s)), not for the lifetime of the sync engine. Shutdown is via a `tokio::sync::oneshot::Sender` stored in `sync_shutdown`.

Proof:

- File: `packages/ark-core/rust/src/ffi.rs` lines 302-524
- `cargo build --release` exits 0 (raw/build.txt: "0 errors, 1 warning")
- `./gradlew compileDebugKotlin` exits BUILD SUCCESS (raw/kotlin-build.txt)

---

## AC2 — Relay transport module

**Status**: PASS

File `packages/ark-core/rust/src/relay_transport.rs` exists with:

- URL pattern `{url}/ws?space_id=X&device_id=Y&api_key=Z`
- `LanSyncMessage` JSON wire format via `serialize_message`/`deserialize_message`
- Offline outbox (`Arc<Mutex<VecDeque<LanSyncMessage>>>`)
- Exponential backoff: `backoff_secs = (backoff_secs * 2).min(60)`, starting at 1s
- `RelayEvent::Connected/Disconnected/MessageReceived` events

`lib.rs` line 9: `pub mod relay_transport;`

Proof: `cargo build --release` exit 0 (raw/build.txt)

---

## AC3 — Mesh coordinator module

**Status**: PASS

- `packages/ark-core/rust/src/mesh.rs`: `MeshConfig { lan_enabled, relay_url, relay_api_key }`, `MeshCoordinator::should_process(device_id, entity_id, hlc)` uses `HashSet<DedupKey>`
- `ffi.rs` `FfiSyncConfig`: `relay_url: Option<String>` and `relay_api_key: Option<String>` both with `#[uniffi(default = None)]`
- `main.rs` `StartSync`: both fields present with `#[serde(default)]`

Proof: `cargo build --release` exit 0; `./gradlew compileDebugKotlin` BUILD SUCCESS (PeerManager.kt unchanged)

---

## AC4 — ark-relay-server binary

**Status**: PASS

`packages/ark-relay-server/`:

- `Cargo.toml`: `[[bin]] name = "ark-relay-server"`, deps: tokio, axum, tokio-tungstenite, rusqlite (bundled), serde_json, uuid
- `src/main.rs`: WebSocket server on `ARK_RELAY_PORT` (default 8765), api_key validation
- `src/relay.rs`: `RelayRooms` with HashMap<space_id, HashMap<device_id, WsSender>>, catch-up replay, broadcast, SQLite store
- `src/db.rs`: events table (space_id, device_id, entity_json, hlc, stored_at), WAL mode

Proof: `cargo build --release` exit 0 (raw/relay-build.txt: "0 errors, 2 warnings")

---

## AC5 — Swift UniFFI scaffold

**Status**: PASS

Files exist at `packages/ark-core/swift/`:

- `ArkCoreExample.swift` — uses `ArkCore()`, `FfiSyncConfig(spaceId:...)`, `startSync(config:listener:)`
- `generated/ark_core.swift` — generated UniFFI bindings (2.7KB)
- `generated/ark_coreFFI.h` — generated C header (845B)

No `.xcodeproj` or `.xcworkspace` present (ls returns no matches).

---

## AC6 — Integration test: relay round-trip

**Status**: PASS

`packages/ark-core/rust/tests/relay_round_trip.rs` — in-process relay server on random port, clients A and B connect to same space_id, A broadcasts `LiveChange`, B receives within 5s timeout.

Proof: `cargo test --test relay_round_trip` — 1 passed in 0.11s (raw/relay-test.txt)

---

## AC7 — @arksync/node package exists

**Status**: PASS

`packages/arksync-node/package.json`: `"name": "@arksync/node"`, `"build": "tsc"` script present.

---

## AC8 — ArkClient class is complete and type-correct

**Status**: PASS

`packages/arksync-node/src/ark-client.ts` implements all required methods:

- `constructor(opts: ArkClientOptions)` with spaceId, deviceId, deviceName, port, relayUrl, relayApiKey, sidecarPath, requestFn, onEventFn
- `start()`, `stop()`, `broadcastChange()`, `getConnectedPeers()`
- `onPeerConnected()`, `onPeerDisconnected()`, `onEntityChanged()` — all return unsubscribe fn

Proof: `bun run build` (tsc) exits 0 with no TypeScript errors (raw/arksync-node-build.txt)

---

## AC9 — Delphi Electron uses @arksync/node

**Status**: PASS

- `apps/delphi/ts/package.json` line 28: `"@arksync/node": "workspace:*"`
- `apps/delphi/ts/electron/main.ts` line 6: `import { ArkClient } from '@arksync/node'`
- `main.ts` does NOT import from `./peer-manager`, `./sync-server`, `./sync-client`, `./peer-discovery`, `./broadcast-discovery`, or `@arksync/core`
- `sidecar.ts` retained with all DB operation wrappers

Proof: `bun run build` in apps/delphi/ts exits 0, "2346 modules transformed" (raw/ts-build.txt)

---

## AC10 — Legacy Electron TS sync files deleted

**Status**: PASS (fixed 2026-04-09)

`apps/delphi/ts/src/services/sync/ark-client.ts` deleted. All import sites updated to `@/services/sync/ark-types` (new file with pure data conversion utilities and a no-op `arkSync` stub). `peer-bridge.ts` import updated to `./ark-types`.

Verification:

- `ls apps/delphi/ts/src/services/sync/ark-client.ts` → No such file
- `grep -r "ark-client" apps/delphi/ts/src/` → empty (exit 1)
- `bun run build` in apps/delphi/ts → exit 0, 2346 modules transformed, 0 TS errors

---

## AC11 — Legacy Kotlin sync files deleted

**Status**: PASS (fixed 2026-04-09)

All 4 legacy files deleted:

- `ArkSyncClient.kt` — deleted; ViewModels now use `peerManager.broadcastTodoChange(...)` directly
- `ArkPeerManager.kt` — deleted; MainActivity no longer injects or calls it
- `ArkPeerProtocol.kt` — deleted (was only used internally by ArkPeerManager)
- `ArkEventMapper.kt` — deleted; `SyncStatus` enum reference in `ConnectionIndicator.kt` updated to use `LanSyncState`

Changed files: `TodoViewModel.kt`, `SmartListViewModel.kt`, `InboxViewModel.kt`, `LogbookViewModel.kt`, `TodayViewModel.kt`, `UpcomingViewModel.kt`, `TrashViewModel.kt`, `ProjectViewModel.kt`, `SettingsViewModel.kt`, `MainActivity.kt`, `ConnectionIndicator.kt`

Verification:

- `ls ArkSyncClient.kt ArkPeerManager.kt ArkPeerProtocol.kt ArkEventMapper.kt` → all "No such file"
- `grep -r "ArkSyncClient|ArkPeerManager|ArkPeerProtocol|ArkEventMapper" apps/delphi/kotlin/app/src/main/java/` → empty (exit 1)
- `./gradlew compileDebugKotlin` → BUILD SUCCESSFUL in 24s

---

## AC12 — packages/arksync deleted; zero @arksync/core imports remain

**Status**: PASS

- `packages/arksync/` deleted (ls shows it's gone)
- Root `package.json` uses `"packages/*"` glob — no explicit entry needed
- `grep -r "@arksync/core" apps/ packages/` returns empty (exit 1)
- Inlined implementations in:
  - `ts/src/services/sync/hlc.ts` — full HLC class
  - `ts/src/services/sync/lan-protocol.ts` — all protocol types and functions
  - `ts/src/services/sync/peer-protocol.ts` — crypto helpers and peer types
  - `ts/src/services/space/space-manager.ts` — space code functions
- Tests for deleted TS SyncServer/SyncClient removed; pure utility tests continue to pass

---

## AC13 — Electron starts without errors

**Status**: UNKNOWN

Requires interactive session with display server. Build passes (AC9), sidecar path wiring uses same getSidecarBinaryPath() logic.

---

## AC14 — Android APK builds and binds ports

**Status**: UNKNOWN

`./gradlew compileDebugKotlin` passes. Full APK install + adb port check requires physical device.

---

## AC15 — Electron → Android sync

**Status**: UNKNOWN (requires both devices on same LAN)

---

## AC16 — Android → Electron sync

**Status**: UNKNOWN (requires both devices on same LAN)

---

## AC17 — packages/ark-core/CLAUDE.md updated

**Status**: PASS

`packages/ark-core/CLAUDE.md` Runtime status section now states:

- Sync layer LIVE
- Beacon in Rust (beacon.rs)
- Relay transport live (relay_transport.rs, mesh.rs)
- Relay server: packages/ark-relay-server/
- TODO block replaced with "Runtime transition — COMPLETE"
- Architecture table includes all new files (beacon.rs, relay_transport.rs, mesh.rs, ffi.rs, host.rs)

---

## AC18 — Delphi CLAUDE.md files updated

**Status**: PASS (fixed 2026-04-09)

`apps/delphi/CLAUDE.md` changes:

- Electron file tree: removed `peer-manager.ts`, `peer-discovery.ts`, `peer-protocol.ts` entries
- `services/sync/` description: updated from `ark-client (legacy)` to `ark-types`
- Removed "Legacy: ArkSyncClient" section
- "Где живёт код" table: replaced legacy peer-manager.ts/sync-server.ts/sync-client.ts with `@arksync/node`, `ark-types.ts`
- "Ark WebSocket relay (legacy/отключён)" section replaced with "Relay транспорт (@arksync/node / Rust)" referencing `relay_transport.rs` and `mesh.rs`
- Address filter implementation line updated to reference `beacon.rs` instead of deleted `broadcast-discovery.ts`

`apps/delphi/kotlin/CLAUDE.md` line 131 already correctly states: "Legacy `ArkSyncClient`, `ArkPeerManager`, `ArkPeerProtocol`, `ArkEventMapper` have been deleted."

Verification:

- `grep "peer-manager.ts\|peer-discovery.ts\|peer-protocol.ts" apps/delphi/CLAUDE.md` → empty
- `grep "ts/src/services/sync/ark-client" apps/delphi/CLAUDE.md` → empty
- `grep "relay_transport\|mesh.ms\|arksync-node" apps/delphi/CLAUDE.md` → 5 matches

---

## Build summary

| Build                                      | Exit | Notes                                     |
| ------------------------------------------ | ---- | ----------------------------------------- |
| `cargo build --release` (ark-core)         | 0    | 1 warning (unused fields in StartSync)    |
| `cargo test` (ark-core)                    | 0    | 95 passed                                 |
| `cargo test --test relay_round_trip`       | 0    | 1 passed                                  |
| `cargo build --release` (ark-relay-server) | 0    | 2 warnings                                |
| `cargo test` (ark-relay-server)            | 0    | 0 tests (integration covered by ark-core) |
| `bun run build` (@arksync/node)            | 0    | tsc clean                                 |
| `bun run build` (apps/delphi/ts)           | 0    | 2346 modules transformed                  |
| `./gradlew compileDebugKotlin`             | 0    | BUILD SUCCESS                             |

## Raw artifacts

- `raw/build.txt` — ark-core cargo build --release
- `raw/test-unit.txt` — ark-core cargo test
- `raw/relay-test.txt` — cargo test --test relay_round_trip
- `raw/relay-build.txt` — ark-relay-server cargo build --release
- `raw/arksync-node-build.txt` — @arksync/node bun run build
- `raw/ts-build.txt` — apps/delphi/ts bun run build
- `raw/kotlin-build.txt` — ./gradlew compileDebugKotlin
- `raw/legacy-check.txt` — grep for legacy symbols (only swift-archive, not active code)
