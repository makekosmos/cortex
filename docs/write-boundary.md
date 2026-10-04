# Write boundary

The user SQLite database has exactly one writer: `ArkService` inside the
Engine process. Everything else — GPUI apps, package workers, the Engine's
own subsystems — goes through the op surface.

## Allowed write paths

1. **Engine → `ArkHost` → `ArkService`** (`runtime/src/ark_host.rs`):
   in-process `call(op, params)`. The single FIFO worker thread is the
   serialization point.
2. **External processes → Engine API** (`POST /v1/rpc`, WebSocket): callers
   never open the database; they issue ops the Engine forwards or handles.
3. **Inside ark-core**: `ark_core::db` functions on the service's
   `SqliteStorageBackend` connection.

## Bans

- No process other than the Engine opens the user database. No sidecar, no
  renderer, no package worker.
- No direct `conn.execute(INSERT/UPDATE/DELETE)` against syncable tables
  (`objects`, `object_types`, `object_links`, `tracked_apps`,
  `usage_sessions`, `usage_events`, `usage_days`, `sync_kv`) without the
  matching `bump_sync_version_vector` / `record_usage_sequence` call —
  silent writes are lost by sync and corrupt version vectors.
- No destructive migrations (`DROP TABLE`, incompatible `ALTER COLUMN`).
- No `Mutex::lock().unwrap()` in production paths — recover poison via
  `unwrap_or_else(|e| e.into_inner())`.

## Verifying a new write path

1. The write executes on the ArkService worker thread (or is routed there).
2. If the table syncs, sync state is bumped in the same transaction.
3. Errors return `{"ok":false,"error"}` — no panics across the boundary.
4. Test isolation: tests write only under `.tmp`, OS temp, or fixture dirs —
   never a real user data dir.
