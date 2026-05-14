# Task Spec: ark-core-rust

## Metadata
- Task ID: ark-core-rust
- Created: 2026-04-04
- Repo root: /Users/kirill/Documents/projects/kepler

## Guidance Sources
- `/CLAUDE.md` (repo task proof loop)
- `/apps/delphi/CLAUDE.md` (P2P sync protocol, entity models, sidecar architecture)
- `/apps/delphi/kotlin/CLAUDE.md` (Android sync, entity types)

## Original Task Statement

Create a single Rust crate `packages/ark-core/rust/` that replaces:
1. `packages/arksync/` (TS sync engine, ~2434 LOC) -- P2P sync protocol
2. `apps/delphi/ts/sidecar/` (Rust delphi-db, ~686 LOC) -- SQLite CRUD
3. Kotlin sync code in `apps/delphi/kotlin/.../data/sync/` (SyncServer.kt, LanSyncClient.kt, ~1400 LOC)

The crate provides a unified DB layer + sync layer + UniFFI bindings, packaged both as a stdin/stdout JSON-RPC binary (for Electron) and as a shared library (for Android/iOS via UniFFI).

---

## Acceptance Criteria

### DB Layer

**AC1** -- SQLite schema identical to current sidecar.
The crate opens a SQLite database (rusqlite, bundled feature) and creates tables `todos`, `projects`, `areas`, `tags`, `headings`, `sync_kv` with the exact column definitions from `apps/delphi/ts/sidecar/src/main.rs` (lines 399-466). PRAGMAs: `journal_mode=WAL`, `synchronous=NORMAL`, `foreign_keys=ON`, `busy_timeout=5000`.

**AC2** -- CRUD operations for all entity types.
The crate exposes the same set of operations as the current sidecar: `init`, `load_all`, `upsert_todo`, `delete_todo`, `batch_upsert_todos`, `upsert_project`, `delete_project`, `upsert_area`, `upsert_tag`, `upsert_heading`, `delete_heading`, `get_sync_kv`, `set_sync_kv`, `clear_all`, `delete_trashed`. Upsert semantics use `INSERT OR REPLACE`. Boolean fields stored as INTEGER (0/1). `tag_ids` and `checklist_items` stored as JSON TEXT. `recurrence_rule` stored as nullable JSON TEXT.

**AC3** -- Stdin/stdout JSON-RPC mode.
When built as a binary (`ark-core-rpc`), the crate reads one JSON object per line from stdin, dispatches to the appropriate handler, and writes one JSON response line to stdout. Request format: `{"operation": "snake_case", ...params}`. Response format: `{"ok": true, "data": ...}` or `{"ok": false, "error": "..."}`. Wire format is identical to the current delphi-db sidecar for backward compatibility with the existing Electron sidecar bridge (`ts/electron/sidecar.ts`).

### Sync Layer

**AC4** -- HLC (Hybrid Logical Clock).
Implement `HLC` with `tick()`, `merge(remote)`, `compare(a, b)`, `to_string()`, `from_string(s)`. String format: `<ISO8601>:<counter:06d>:<device_id>`. Parsing splits on `:` after the `Z` character (matching TS implementation in `packages/arksync/src/hlc.ts`). Compare order: wall_time lexicographic, then counter numeric, then device_id lexicographic (LWW tie-break).

**AC5** -- Version vectors and diff computation.
`VersionVector` = `HashMap<String, String>` (entity_id -> HLC string). Implement `compute_vector_diff(local, remote) -> HashSet<String>` returning entity IDs the local side needs from remote. Implement `compute_local_excess(local, remote) -> HashSet<String>` (inverse).

**AC6** -- Batch splitting.
`split_into_batches(entities) -> Vec<Vec<SyncEntity>>` respecting `MAX_BATCH_SIZE=100` and `MAX_BATCH_BYTES=1_048_576` (1 MB, measured as JSON serialized byte length). If adding an entity would exceed either limit, start a new batch.

**AC7** -- Protocol messages.
All nine message types are represented as a Rust enum with serde `#[serde(tag = "type")]` discriminator:
- `hello` (protocol_version, device_id, device_name, space_id, addresses?)
- `version_vector` (vector)
- `sync_changes` (batch_id, entities, is_last)
- `sync_ack` (batch_id, accepted)
- `live_change` (change_id, entity)
- `live_ack` (change_id)
- `peer_list` (peers: Vec<PeerRecord>)
- `ping` (ts)
- `pong` (ts)

JSON serialization/deserialization must be byte-compatible with the TS `serializeMessage()`/`deserializeMessage()` output for the same logical message. All field names use snake_case. `SyncEntity` has fields: `type` (renamed to `entity_type` or `r#type` internally), `id`, `data` (arbitrary JSON object), `hlc`, `deleted` (optional bool).

**AC8** -- WebSocket server.
Async WS server on port `21531` using tokio + tokio-tungstenite. Accepts incoming connections. Protocol flow per connection:
1. Receive `hello` from client, validate `protocol_version == 1`, reply with own `hello`.
2. Send `version_vector`, receive remote `version_vector`.
3. Exchange `peer_list` (send after short delay ~100ms).
4. Compute diff, send `sync_changes` batches, wait for `sync_ack` per batch (timeout 10s, max 3 retries).
5. Receive client's `sync_changes` batches, reply with `sync_ack`.
6. Enter live mode: incoming `live_change` -> apply + `live_ack`; outgoing mutations -> `live_change` to all peers.
7. Ping via WS-level ping frames every 15s.

Server state tracks per-connection: device_id, device_name, addresses, authenticated flag, sync_complete flag, pending ACKs, queued live changes.

**AC9** -- WebSocket client.
Async WS client using tokio-tungstenite. Takes a `PeerRecord` (device_id, device_name, addresses). Races connections to ALL addresses in parallel with 5s connect timeout; first successful connection wins, others dropped. Same protocol flow as server but initiates the `hello`. Reconnect with exponential backoff: base 2s, max 30s, reset on successful sync. Stopped flag to prevent reconnect after explicit stop.

**AC10** -- Peer record management.
`PeerRecord` struct: device_id, device_name, addresses (Vec<String>), last_seen (ISO 8601), last_address (Option<String>). `merge_peer_records(existing, incoming) -> Vec<PeerRecord>` unions addresses (dedup), keeps newest last_seen, preserves last_address from newer record. Known peers persisted via `sync_kv` under key `sync.peers` as JSON.

**AC11** -- Space codes.
- `generate_space_code() -> String`: 12 random Base32-Crockford characters (alphabet `0123456789ABCDEFGHJKMNPQRSTVWXYZ`).
- `encode_ipv4(ip) -> Option<String>`: encode IPv4 into 7 Base32-Crockford chars (32-bit IP left-shifted by 3 bits, then 7x5-bit digits).
- `decode_ipv4(encoded) -> Option<String>`: reverse.
- `generate_extended_code(code, ipv4) -> Option<String>`: 19-char = 12-char secret + 7-char encoded IPv4.
- `format_space_code(code) -> String`: dashes for 7/12/19-char codes (XXXX-XXX / XXXX-XXXX-XXXX / XXXX-XXXX-XXXX-XXXX-XXX).
- `parse_space_code(input) -> Option<String>`: accept 7 or 12-char codes, strip dashes, uppercase, validate alphabet, return raw.
- `derive_space_id(code) -> String`: SHA-256 of uppercase raw code, first 16 hex chars.
- QR payload: `generate_qr_payload(code, addresses)`, `parse_qr_payload(payload)`. URI scheme: `ark://join?code=...&addrs=...`. Also parse 19-char extended codes and bare codes.

**AC12** -- StorageBackend trait.
```rust
#[async_trait]
trait StorageBackend: Send + Sync {
    async fn load_entities(&self, vector: &VersionVector) -> Vec<SyncEntity>;
    async fn apply_entity(&self, entity: &SyncEntity);
    async fn get_kv(&self, key: &str) -> Option<String>;
    async fn set_kv(&self, key: &str, value: &str);
}
```
A `SqliteStorageBackend` implementation backed by the DB layer (AC1-AC2) that implements this trait. `load_entities` returns all entities whose HLC is present in the given version vector. `apply_entity` does upsert-or-delete based on the `deleted` flag.

### UniFFI Bindings

**AC13** -- UniFFI export surface.
The crate uses `uniffi` (proc-macro or UDL) to export the following functions/types for Kotlin and Swift:
- `open_db(path: String)` -- open database, create schema
- All CRUD operations from AC2
- `start_server(space_id, device_id, device_name, addresses)` / `stop_server()`
- `start_client(peer_record, device_id, device_name, space_id, own_addresses)` / `stop_client(peer_id)`
- `broadcast_live_change(entity: SyncEntity)`
- Space code functions: `generate_space_code`, `derive_space_id`, `parse_space_code`, `format_space_code`, `generate_qr_payload`, `parse_qr_payload`
- Callback interface for change notifications, peer connect/disconnect events

**AC14** -- Dual build targets.
`Cargo.toml` supports:
- `cargo build` produces a binary (`ark-core-rpc`) for stdin/stdout JSON-RPC (Electron sidecar replacement).
- `cargo build --lib` produces a cdylib (`libark_core.so`/`.dylib`) for UniFFI consumption.
Both targets share the same core code. The binary is defined as `[[bin]]`, the library as `[lib] crate-type = ["cdylib", "lib"]`.

### Integration and Testing

**AC15** -- Wire protocol compatibility.
The Rust implementation MUST produce and consume messages that are byte-compatible with the existing TS arksync (`packages/arksync/`). A Rust Android client MUST be able to sync with a TS Electron server and vice versa. Key compatibility points:
- JSON field names: snake_case (e.g., `protocol_version`, `device_id`, `space_id`, `batch_id`, `is_last`, `change_id`, `last_seen`, `last_address`)
- `SyncEntity.type` field serializes as `"type"` in JSON (not `entity_type`)
- `SyncEntity.data` is an arbitrary JSON object (`serde_json::Value` / `Map<String, Value>`)
- `SyncEntity.deleted` is omitted when false/absent (serde `skip_serializing_if`)
- `protocol_version` = 1
- HLC string format: `<ISO8601>:<06d counter>:<device_id>`
- Version vector: JSON object `{ "entity_id": "hlc_string", ... }`
- Batch IDs and change IDs: `<timestamp_ms>-<random_6_alphanumeric>`
- UUIDs: always lowercase

**AC16** -- Unit tests.
Tests for:
- HLC tick, merge, compare, to/from string round-trip
- Version vector diff computation
- Batch splitting (entity count limit, byte limit)
- Space code generate/format/parse round-trip, IPv4 encode/decode round-trip, QR payload round-trip, derive_space_id
- Peer record merging
- Protocol message serialization/deserialization round-trip (verify JSON field names match TS output)
- DB schema creation and CRUD operations (in-memory SQLite)

---

## Constraints

- Rust 2021 edition.
- Dependencies: `rusqlite` (bundled), `tokio` (rt-multi-thread, macros, net, time, sync, io), `tokio-tungstenite`, `serde` + `serde_json`, `uniffi`, `hmac` + `sha2`, `uuid`, `async-trait`, `rand`, `base32` (or manual Crockford).
- No `unsafe` code outside FFI boundaries (UniFFI-generated code exempt).
- All public API uses `String` for IDs (not typed UUID wrapper) to match TS/Kotlin.
- `camelCase` for JSON-RPC request/response fields (matching current sidecar wire format: `dbPath`, `tagIds`, `checklistItems`, etc.). `snake_case` for sync protocol message fields (matching current arksync wire format: `protocol_version`, `device_id`, etc.).
- The crate path is `packages/ark-core/rust/`.

---

## Non-Goals

- Do NOT delete or modify `packages/arksync/` (TS code stays; Electron migration is a separate task).
- Do NOT rewrite Kotlin Delphi UI or Android Room DAOs.
- Do NOT touch Swift code (archived).
- Do NOT change the wire protocol (cross-platform compatibility is mandatory).
- Do NOT implement UDP broadcast discovery (Android uses its own `BroadcastDiscovery.kt`; discovery remains platform-specific).
- Do NOT implement the `peer_hello` / `peer_hello_ack` HMAC auth flow from `peer-protocol.ts` -- it is defined but not used in the current sync-server/sync-client. Auth is via `space_id` matching in the `hello` message.
- Do NOT implement NAPI bindings (Electron integration stays as child-process stdin/stdout for now).

---

## Assumptions

1. **No HMAC auth on wire.** The current sync protocol does NOT use HMAC peer authentication at the WebSocket layer. `peer-protocol.ts` types (`peer_hello`, `peer_hello_ack`) are defined but never referenced in `sync-server.ts` or `sync-client.ts`. Authentication is implicit via `space_id` match. This spec mirrors the actual behavior.
2. **Embedded sub-entities.** `SyncEntity.type` values used across platforms are: `todo`, `project`, `area`, `tag`, `heading`. Checklist items and tag cross-refs are embedded within the `todo` entity's `data` JSON (as `checklistItems` array and `tagIds` array), not synced as separate entity types. The Kotlin CLAUDE.md mentions them as sync entity types, but `SyncEntityParser.kt` embeds them inside the todo JSON. This spec follows the actual wire format.
3. **No `notes` entity type.** The `notes` entity type mentioned in the original task statement does not appear in the current sidecar schema or sync code. Excluded from scope.
4. **Hard delete semantics.** The `deleted` field on `SyncEntity` triggers a hard DELETE from the database (not a soft-delete flag), matching current behavior in both TS and Kotlin.
5. **Synchronous SHA-256.** `derive_space_id` in Rust will use synchronous SHA-256 (via `sha2` crate) rather than async `crypto.subtle` as in the TS version.

---

## Verification Plan

1. **Build check**: `cargo build` succeeds for both binary and library targets. `cargo clippy -- -D warnings` passes.
2. **Unit tests**: `cargo test` passes all tests from AC16.
3. **Schema parity**: Open a DB created by the Rust crate and one created by the TS sidecar; compare `sqlite_master` output for table definitions (exact column names, types, defaults).
4. **JSON-RPC compatibility**: Send the same JSON-RPC requests to both the TS sidecar and the Rust `ark-core-rpc` binary; verify responses are structurally identical.
5. **Protocol message compatibility**: Serialize each message type in Rust and compare against known-good JSON strings derived from the TS implementation (unit tests with hardcoded expected JSON).
6. **HLC interop**: Round-trip HLC strings between Rust `from_string`/`to_string` with test fixtures matching TS output format.
7. **Space code interop**: Generate codes and QR payloads in Rust, verify they parse correctly against expected format via unit test fixtures.
8. **UniFFI generation**: `uniffi-bindgen generate` produces Kotlin bindings without errors. Swift bindings generate without errors (build not required).
