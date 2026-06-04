# Raycast List.Item.Detail Aliases

## Goal

Expose Raycast-compatible `List.Item.Detail` and `List.Item.Detail.Metadata` aliases in the private `@raycast/api` shim while reusing the existing host detail/metadata renderer.

## Acceptance Criteria

- `packages/raycast-api` exposes `List.Item.Detail`.
- `List.Item.Detail.Metadata` and nested metadata helpers serialize as existing `Detail.Metadata*` node types.
- `normalizeRaycastNode(...)` preserves `metadata` passed to `List.Item.Detail`.
- Host model reads markdown and metadata from `List.Item.Detail`.
- Unit verification covers API serialization and host model extraction.
