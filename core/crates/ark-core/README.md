# ARK Core

ARK Core is the canonical local-first data runtime for Kosmos. It is a Rust library backed by SQLite; the Kosmos Engine hosts it in-process via `ark_core::service::ArkService`.

## Source Layout

The Rust workspace lives in `core/`. Its main crate boundaries are:

- `ark-core` exposes `ArkService` and composes runtime, storage, sync, and
  network components. It owns request admission and the database actor
  lifecycle while preserving the Engine JSON contract.
- `ark-store` owns SQLite connections, schema and SQL adapters, the single
  writer, transaction execution, snapshots, and restore.
- `ark-application` defines the capabilities shared between Core, sync, and
  storage.
- `ark-sync` owns sync policy, protocol state, and connected-session
  orchestration.
- `ark-net` owns concrete network transports.
- `ark-domain` contains shared domain types.

## Build And Test

```powershell
cargo nextest run -p ark-core
pnpm run test:ark-core-storage-boundary
node scripts/check-ark-dependency-graph.mjs --profile desktop --target x86_64-pc-windows-msvc
node scripts/check-ark-dependency-graph.mjs --profile mobile --target aarch64-linux-android
```

## Service Contract

`ark_core::service::ArkService` serves the JSON-RPC operation set in-process: the Engine submits requests and receives responses/events directly, with the same newline-delimited JSON framing semantics the retired `ark-core-rpc` sidecar used on stdin/stdout.

Requests may include an optional `id`; responses echo it. Requests without `id` still work.

Typical lifecycle:

1. `init` with `dbPath`.
2. Object/usage CRUD and query operations through RPC.
3. Optional `start_sync` for LAN sync.
4. `stop_sync` before shutdown.

When `relay_url` is provided, the service starts a relay bridge beside LAN sync. The bridge exchanges `hello`, `version_vector`, `sync_changes`, and `live_change` frames through the relay server.

`start_sync` accepts an optional `auth_secret`. When configured, LAN/P2P peers must prove they know the same secret by sending `auth_nonce` + `auth_hmac` in the `hello` message. This authenticates peers; it does not encrypt WebSocket traffic.

## Snapshot Restore (KOS-51)

Privileged ops over the Core-owned `<db_dir>/backups/` directory:

- `db_backup_list` → `{ "backups": [{ "id", "size_bytes", "modified_ms" }] }` — regular files only.
- `db_backup_validate` `{ "backup_id": "<basename>" }` → `{ "id", "exists", "integrity_ok", "schema_match", "valid", "error?" }` — dry-run; never mutates live DB.
- `db_backup_restore` `{ "backup_id": "<basename>" }` → `{ "id", "restored": true, "objects", "links" }` — atomic in-place restore via SQLite Online Backup API into the live connection.

`backup_id` is a basename only — no paths/traversal. Sources are opened without following symlinks/reparse points, staged into `backups/`, checked with `PRAGMA integrity_check` and a normalized `sqlite_schema` + migration-provenance fingerprint against the live DB before apply. A pre-restore rollback snapshot is kept until post-restore verification passes; verification failure restores the rollback. If the rollback itself fails, the snapshot is preserved as `ark.db.pre-restore-failed-*` (listed by `db_backup_list`, restorable via `db_backup_restore`) and the error reports the preserved id — prior data is never discarded. `db_backup` and `db_backup_restore` are serialized by a shared gate — they never overlap. Callers should reload UI after a successful `db_backup_restore`.

Production backups use `ark.db.backup-v1-*` plus a JSON manifest containing the id, database byte length, SHA-256, and schema fingerprint. The staged SQLite Online Backup is integrity-checked and both files are flushed before publication. The manifest is published first as the visibility marker, then the database name; list/restore ignore a v1 database without a manifest and a manifest without a database. Publication uses same-directory no-clobber links, so a destination collision preserves the existing files. On Unix the containing directory is also synced after publication. On Windows Rust's portable filesystem API does not flush directory metadata here: the files are synced and verified, but sudden power loss can still lose the newly published names. SQLite commit durability is separate: the live connection requests WAL + `synchronous=FULL`; actual power-loss guarantees still depend on the filesystem, device, and their honoring flush requests. The local benchmark in `docs/experiments/2026-10-16-ark-sync-durability.md` compares FULL/NORMAL commit timing on one macOS environment and is not a cross-platform durability or production-latency measurement.

## TypeScript Integration

Historical TypeScript callers used `@kosmos/ark` from Electron main or another
trusted Node process. The sidecar-based example below is retained for migration
reference and does not describe the current in-process Engine integration.

```ts
import { ArkClient } from "@kosmos/ark";

const ark = new ArkClient({
  spaceId: "main",
  deviceId: "my-app-main",
  deviceName: "My App",
  dbPath: "C:/Users/me/AppData/Roaming/Kosmos/spaces/main/ark.db",
  sidecarPath: "path/to/ark-core-rpc.exe",
});

await ark.objects.upsert({
  id: "object-id",
  typeId: "game_obj",
  title: "Example",
  contentJson: { type: "doc", content: [] },
  propsJson: { source: "my-app" },
  createdAt: new Date().toISOString(),
  updatedAt: new Date().toISOString(),
  deletedAt: null,
});
```

The SDK initializes the sidecar before write/read calls when it owns the process. Apps that already own a sidecar can inject `requestFn` and `onEventFn`.

## Data Model

ARK currently contains:

- Legacy task tables: `todos`, `projects`, `areas`, `tags`, `headings`
- Usage tables: `tracked_apps`, `usage_sessions`, `usage_events`
- Generic object model: `object_types`, `objects`, `object_links`
- Sync metadata: `sync_kv`, `sync_tombstones`

New app-domain data should prefer the generic object model unless it is high-volume usage/analytics data.

Runtime query endpoints include:

- `list_objects_by_type`
- `get_objects_by_ids`
- `list_recent_usage_processes`
- `search_usage_processes`
- `get_usage_game_playtime_summary`

## Sync State

Local writes through `ArkService` update `lan_sync.version_vector` and durable delete tombstones. `ark_store::sync_versions::bump_sync_version_vector` is the shared persistence helper for direct Rust writers such as `services/usage-tracker`; callers keep the version update inside their existing transaction.

Delete propagation depends on durable tombstones in `sync_tombstones`. Apply errors are surfaced as `Result` values instead of being silently ignored.

## App Integration Rules

- Renderer processes should not talk to ARK directly.
- Electron preload should expose narrow app-specific APIs.
- Electron main should use `@kosmos/ark` or app-specific wrappers around it.
- Apps should not write directly into ARK SQLite tables.
- Migration scripts may read source databases directly, but target ARK writes should go through ARK RPC/SDK.
- If a Rust process must write directly, it must call `ark_core::db` helpers that update sync state.

## Current Limitations

- Relay sync is wired into the in-process service (`ark_core::service::ArkService`).
- LAN sync supports optional HMAC peer authentication, but traffic is not encrypted yet.
- Generic object search uses SQLite FTS5 when available and falls back to safe in-memory matching when FTS is unavailable or a query cannot be parsed.
- Some historical docs/scripts remain as migration aids and should not be treated as the current integration contract.
