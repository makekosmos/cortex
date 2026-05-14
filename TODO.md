# TODO

## Legacy Note

The previous contents of this file described an older `packages/ark/` Python/server-era plan. That is no longer the active ARK architecture.

Current ARK runtime documentation:

- [`packages/ark-core/README.md`](./packages/ark-core/README.md)
- Rust runtime: `packages/ark-core/rust`
- Node/Electron SDK: `packages/kosmos-ark` (`@kepler/ark`)
- Compatibility SDK name: `packages/arksync-node` (`@arksync/node`)
- Canonical desktop sidecar: `ark-core-rpc`

## Current ARK Priorities

- Keep app writes going through `ark-core-rpc` / `@kepler/ark`.
- Keep direct Rust writers on `ark_core::db` helpers so sync state is updated consistently.
- Delphi tasks migrate automatically at app startup into `task_obj` records in the generic object model.
- Eden notes and custom typed notes are ARK objects (`note_obj` or their custom object type). Heart remains available for editor/vault-specific behavior and one-time migration/import/export work.
- Arrancador game writes, usage backfill, game hydration, usage summaries, and process search should prefer `@kepler/ark`; read-only SQLite is only a fallback when the ARK runtime is unavailable.
- Dashboard is a read-only ARK inspector/analytics app. It may inspect selected ARK SQLite databases from Electron main, but must never write to ARK tables.
- Keep the smoke matrix in `docs/ARK-SMOKE-MATRIX.md` current and ensure every automated check uses an isolated test DB or temporary app-data path.
- Use `bun run ark:guard:writes` before changing app data services; direct app-service writes to ARK tables are forbidden.
- ARK runtime now has object query, usage process query, and game playtime summary endpoints. If read-only ARK SQL needs to be removed completely, continue replacing fallback paths with dedicated runtime analytics endpoints.
- Delphi legacy DB sidecar is removed; `task_obj` ARK objects are the task source of truth after startup migration.
