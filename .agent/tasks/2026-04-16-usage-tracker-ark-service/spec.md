# Task Spec - usage-tracker extraction into Ark

## Original task statement

- Remove duplicate/local workspace clutter from `apps/arrancador` now that the repo can use a root Bun workspace.
- Extract the old activity tracker out of `Arrancador`.
- Expand it into a standalone tracker for all Windows applications, not only game launches from `Arrancador`.
- Write tracking data directly into Ark DB.
- `Arrancador` should read usage data from the database instead of owning or launching the tracker.
- Target Windows first, while keeping the implementation extensible for a later macOS backend.

## Goal

Create a standalone Windows-first `services/usage-tracker` background executable that writes first-class usage entities into Ark DB, extend `packages/ark-core` so those entities are stored and sync-capable, and decouple `apps/arrancador` from the embedded watcher so it reads usage/playtime from Ark-backed data instead of a private tracker runtime.

## Assumptions

- `services/` is the correct top-level home for long-running/background applications in this monorepo.
- `usage-tracker` is a normal user-level background executable, not a Windows Service.
- Direct DB writes are acceptable for `v1`, but the written entities must still participate in Ark sync by updating Ark's version-vector state.
- `Arrancador` should stop owning/launching the tracker now, but may keep local game/library metadata that is unrelated to usage tracking.
- `Arrancador` can keep transitional local fields such as `games.total_playtime` and `games.last_played` for compatibility, as long as Ark-backed usage data becomes the primary read source when available.
- It is acceptable to clean generated or duplicate workspace artifacts in `apps/arrancador` that are not core source/config.

## Acceptance Criteria

- AC1: `packages/ark-core/rust` has first-class schema/type/storage support for usage tracking entities required by the tracker: `TrackedApp`, `UsageSession`, and `UsageEvent`.
- AC2: Ark usage entities are persisted in a way that is compatible with future sync, including version-vector updates for locally written usage records.
- AC3: A new standalone Windows-first `services/usage-tracker` exists outside `apps/arrancador` and is wired to write directly into Ark DB.
- AC4: `apps/arrancador` no longer owns or launches the embedded watcher runtime and no longer depends on the old private watcher crate/path.
- AC5: `Arrancador` reads playtime/usage from Ark-backed DB data through a stable adapter layer without changing renderer IPC contracts.
- AC6: `Arrancador` cleanup removes watcher-specific or duplicate workspace artifacts that are now dead weight, without deleting unrelated core app files.
- AC7: Root workspace configuration recognizes `services/*` so the repo structure reflects the new long-running service layout.
- AC8: Docs/task artifacts are updated to describe the new ownership split: Ark stores usage, `usage-tracker` writes it, and `Arrancador` consumes it.

## Constraints

- Keep all workflow artifacts under `.agent/tasks/2026-04-16-usage-tracker-ark-service/`.
- Use the smallest defensible diff that achieves the ownership split and preserves current app behavior where possible.
- Do not regress unrelated Electron, Ark sync, or mobile flows.
- Windows is the only required runtime target for the tracker in this task.

## Non-goals

- Full Windows autostart installer flow.
- macOS tracker implementation.
- Replacing all legacy `src-tauri` code in `apps/arrancador`.
- Realtime live-broadcast of usage changes from the tracker into already-running Ark peers beyond making the data sync-capable through Ark storage semantics.

## Bounded implementation plan

1. Finalize Ark storage support for usage entities and direct-write sync readiness.
2. Add `services/usage-tracker` with Windows foreground-window polling, idle tracking, and Ark DB persistence.
3. Update root workspace config for `services/*`.
4. Remove old watcher ownership from `apps/arrancador` and redirect playtime reads to Ark-backed adapters.
5. Clean watcher-specific/dead duplicate artifacts under `apps/arrancador`.
6. Run targeted verification, update evidence, and apply the smallest safe fix if verification fails.

## Verification plan

- Inspect code paths to confirm `Arrancador` no longer launches the old watcher.
- Verify `usage-tracker` project structure and its Ark DB write path.
- Verify Ark storage code contains schema + CRUD/load/apply support for usage entities.
- Run targeted build/test/typecheck commands where possible for `packages/ark-core/rust`, `services/usage-tracker`, and `apps/arrancador`.
- Record any sandbox/network limitations explicitly in `evidence.md`, `evidence.json`, and `problems.md`, then reverify the current tree.
