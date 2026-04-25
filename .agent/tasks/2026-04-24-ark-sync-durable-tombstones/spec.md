# ARK Sync Durable Tombstones

## Context

ARK sync currently represents deletes as transient live changes. Once a row is physically removed from SQLite, later version-vector sync cannot send that deletion to an offline peer because `load_entities` only collects existing rows.

The sync paths also remove deleted entity ids from the local version vector, which weakens resurrection prevention because the delete HLC is no longer durable in the vector.

## Scope

Add a minimal durable tombstone state for deleted sync entities and make the existing sync paths preserve delete HLCs.

## Acceptance Criteria

- AC1: Schema initialization creates a durable `sync_tombstones` table for deleted sync entities.
- AC2: Applying a `SyncEntity` with `deleted=true` physically deletes the current row and records a tombstone containing entity id, type, and delete HLC.
- AC3: Applying a non-deleted entity removes any existing tombstone for the same id after the upsert succeeds.
- AC4: `load_entities` includes durable tombstones as `SyncEntity { deleted: Some(true), data: {}, hlc }` so offline peers can receive deletes during later version-vector sync.
- AC5: Sync server/client paths retain the delete HLC in the version vector instead of removing the entity id on successful delete apply.
- AC6: Regression tests prove tombstones are emitted after delete, survive backend recreation, and are cleared by a newer live entity.
- AC7: Fresh Rust verification commands are recorded in this task directory and all acceptance criteria are `PASS`.

## Out Of Scope

- A full oplog or `sync_entities` state table.
- Tombstone compaction/GC.
- Changing the wire protocol.
- Changing version vector key format away from the current entity id key.
