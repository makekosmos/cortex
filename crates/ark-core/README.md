# ARK Core

ARK Core is the canonical local-first data runtime for Kosmos. It is a Rust library plus the `ark-core-rpc` sidecar binary backed by SQLite.

## Source Layout

- `crates/ark-core` - Rust crate `ark-core`
- `crates/ark-core/src/main.rs` - `ark-core-rpc` stdin/stdout sidecar
- `crates/ark-core/src/db.rs` - SQLite schema, CRUD, sync storage adapter
- `crates/ark-core/src/sync_server.rs` and `sync_client.rs` - LAN sync runtime
- `@makekosmos/ark@0.1.1` - pinned TypeScript SDK package for Electron main/Node callers

## Build And Test

```powershell
cargo build --manifest-path crates/ark-core/Cargo.toml --bin ark-core-rpc
cargo test --manifest-path crates/ark-core/Cargo.toml
bun run ark:smoke
```

## Sidecar Contract

`ark-core-rpc` reads newline-delimited JSON requests from stdin and writes newline-delimited JSON responses/events to stdout.

Modern callers may include an optional request `id`; responses echo it. Legacy callers without `id` still work.

Typical lifecycle:

1. `init` with `dbPath`.
2. Object/usage CRUD and query operations through RPC.
3. Optional `start_sync` for LAN sync.
4. `stop_sync` before shutdown.

When `relay_url` is provided, `ark-core-rpc` starts a relay bridge beside LAN sync. The bridge exchanges `hello`, `version_vector`, `sync_changes`, and `live_change` frames through the relay server.

`start_sync` accepts an optional `auth_secret`. When configured, LAN/P2P peers must prove they know the same secret by sending `auth_nonce` + `auth_hmac` in the `hello` message. This authenticates peers; it does not encrypt WebSocket traffic.

## TypeScript Integration

Use `@kosmos/ark` from Electron main or another trusted Node process:

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

Local writes through `ark-core-rpc` update `lan_sync.version_vector` and durable delete tombstones. `ark_core::db::bump_sync_version_vector` is the shared Rust helper for direct Rust writers such as `services/usage-tracker`.

Delete propagation depends on durable tombstones in `sync_tombstones`. Apply errors are surfaced as `Result` values instead of being silently ignored.

## App Integration Rules

- Renderer processes should not talk to ARK directly.
- Electron preload should expose narrow app-specific APIs.
- Electron main should use `@kosmos/ark` or app-specific wrappers around it.
- Apps should not write directly into ARK SQLite tables.
- Migration scripts may read source databases directly, but target ARK writes should go through ARK RPC/SDK.
- If a Rust process must write directly, it must call `ark_core::db` helpers that update sync state.

## Current Limitations

- Relay sync is wired into both `ark-core-rpc` and the UniFFI `ArkCore::start_sync` facade.
- LAN sync supports optional HMAC peer authentication, but traffic is not encrypted yet.
- Generic object search uses SQLite FTS5 when available and falls back to safe in-memory matching when FTS is unavailable or a query cannot be parsed.
- Some historical docs/scripts remain as migration aids and should not be treated as the current integration contract.
