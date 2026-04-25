# Task: ARK RPC Object Type/Link Local Sync State

## Context

Object and usage writes through `ark-core-rpc` already persist local HLC sync state and tombstones so later LAN sync can observe those changes. `object_type` and `object_link` writes still go through RPC without the same local sync state. Arrancador game migration needs to move target writes to the SDK, and that SDK path must be sync-visible for object type setup and future link writes.

## Scope

- Extend `ark-core-rpc` object type and object link write requests with optional `device_id`.
- Record local version-vector HLCs on object type/link upserts.
- Record durable sync tombstones on object type/link deletes.
- Make `@arksync/node` send its configured `deviceId` for object type/link writes.
- Add focused Rust and TypeScript verification.

## Acceptance Criteria

- AC1: `UpsertObjectType`, `DeleteObjectType`, `UpsertObjectLink`, and `DeleteObjectLink` accept optional `device_id` without breaking old callers.
- AC2: Object type and object link upserts through `ark-core-rpc` update `lan_sync.version_vector` with an HLC ending in the caller device id.
- AC3: Object type and object link deletes through `ark-core-rpc` persist rows in `sync_tombstones` with the correct entity type and id.
- AC4: `@arksync/node` sends `device_id` for object type/link upsert and delete operations.
- AC5: Fresh verification passes: Rust checks/tests/formatting, `@arksync/node` typecheck/build, SDK device-id verifier, and `git diff --check`.
