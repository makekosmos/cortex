# ark-core-rpc Local Write Sync State

## Context

Electron apps are being moved away from direct Ark SQLite writes and toward `ark-core-rpc` / `@arksync/node`. For that to be safe, local writes through `ark-core-rpc` must update Ark sync state atomically enough that later LAN/P2P sync can discover those changes even when the live sync runtime is not currently running.

## Scope

Ensure object and usage writes handled by `ark-core-rpc` record durable sync metadata.

## Acceptance Criteria

- AC1: `upsert_object`, `upsert_tracked_app`, `upsert_usage_session`, and `upsert_usage_event` requests update `lan_sync.version_vector` for the written entity.
- AC2: `delete_object`, `delete_tracked_app`, `delete_usage_session`, and `delete_usage_event` requests update `lan_sync.version_vector` and persist a sync tombstone for the deleted entity.
- AC3: Live upserts clear any prior tombstone for the same entity id.
- AC4: Write requests accept an optional `device_id`; when absent, a stable local fallback device id is used.
- AC5: `@arksync/node` sends its configured `deviceId` on object and usage write requests.
- AC6: Fresh Rust and TypeScript verification commands are recorded in this task directory and all acceptance criteria are `PASS`.

## Out Of Scope

- Reworking the full sync model into `sync_entities`.
- Adding live broadcast for every local CRUD request.
- Changing direct Rust `ark_core::db` callers.
- Converting Arrancador usage backfill in this task.
