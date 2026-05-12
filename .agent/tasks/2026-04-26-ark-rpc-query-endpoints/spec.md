# Task: ARK RPC query endpoints

## Context

`@kepler/ark` currently exposes convenience reads such as `objects.listByType`
and `objects.getMany`, but those methods still filter or fan out in the SDK.
Arrancador also prefers ARK usage snapshots, then filters process candidates in
TypeScript. These should become real `ark-core-rpc` operations backed by Rust
SQLite queries so app code can call a stable ARK API instead of knowing table
details or loading more data than needed.

All automated verification must use isolated test/smoke databases only.

## Acceptance Criteria

- AC1: `ark-core-rpc` exposes object query operations for `list_objects_by_type`
  and `get_objects_by_ids`.
- AC2: `ark-core-rpc` exposes usage process candidate queries for recent and
  text search use cases.
- AC3: `@kepler/ark` calls these RPC operations directly rather than filtering
  `list_objects` or fanning out `get_object`.
- AC4: Arrancador process search uses the new ARK SDK process query methods
  before read-only SQLite fallback.
- AC5: Docs mention that these are real ARK runtime query endpoints.
- AC6: Fresh verification passes on the current workspace using only test/smoke
  databases.
