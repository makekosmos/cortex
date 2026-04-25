# ARK Sync Apply Errors

## Context

The ARK Rust sync storage path currently ignores failures while applying incoming entities. Deserialization errors, unknown entity types, and database write errors can be swallowed, while sync code continues updating version vectors and acknowledgements as if the payload was accepted.

This weakens local-first correctness because a peer can believe a change was applied when it was not.

## Scope

Make the existing `StorageBackend::apply_entity` path return `Result<(), String>` and propagate failures through current sync server/client call sites with the smallest defensible diff.

## Acceptance Criteria

- AC1: `StorageBackend::apply_entity` returns `Result<(), String>` instead of silently completing.
- AC2: `SqliteStorageBackend::apply_entity_blocking` propagates JSON decode, unknown entity type, and database CRUD errors.
- AC3: Sync server/client call sites only update version vectors, accepted counts, callbacks, and rebroadcasts after successful apply.
- AC4: RPC/local broadcast paths surface apply failures to their caller instead of returning success.
- AC5: Regression tests prove invalid sync payloads and unknown entity types return errors.
- AC6: Fresh Rust verification commands are recorded in this task directory and all acceptance criteria are marked PASS only if checks pass.

## Out Of Scope

- Durable tombstones or a new oplog table.
- Changing the wire format of sync acknowledgements.
- Adding retry queues for rejected changes.
- Relay/auth changes.
