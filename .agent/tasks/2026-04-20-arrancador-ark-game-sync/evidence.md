# Evidence

## Verification

- Command: `apps/arrancador\\node_modules\\.bin\\tsc.cmd --noEmit`
- Result: `PASS`
- Raw log: [raw/typecheck.txt](/D:/Personal/Hobby/Coding/kepler/.agent/tasks/2026-04-20-arrancador-ark-game-sync/raw/typecheck.txt)
- Diff snapshot: [raw/diff.txt](/D:/Personal/Hobby/Coding/kepler/.agent/tasks/2026-04-20-arrancador-ark-game-sync/raw/diff.txt)

## Acceptance Criteria

- AC1: `PASS`
  `games` now stores nullable `ark_object_id`, includes an index for lookups, and adds the column via migration in [database.ts](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/electron/main/db/database.ts:8) and [database.ts](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/electron/main/db/database.ts:187).
- AC2: `PASS`
  Linked Ark `game_obj` records are read back into Arrancador game responses via best-effort hydration in [ark-game-objects.ts](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/electron/main/services/ark-game-objects.ts:341) and are composed into the existing Ark usage read model in [ark-usage.ts](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/electron/main/services/ark-usage.ts:219).
- AC3: `PASS`
  Arrancador write paths upsert Ark `game_obj` records and persist returned `ark_object_id` locally through [games.ts](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/electron/main/services/games.ts:338) with backend wiring in [backend.ts](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/electron/main/backend.ts:265).
- AC4: `PASS`
  The patch is limited to Arrancador Electron/db/services/backend code plus task artifacts; no Eden files were modified.
- AC5: `PASS`
  Remaining compile/runtime risks and unresolved sync semantics are listed below and in `evidence.json`.

## Risks

- Ark object sync is best-effort. If `ark.db` is missing, locked, or lacks the `objects` table, Arrancador keeps working but skips object sync/hydration silently.
- The integration writes directly to Ark SQLite instead of going through `ark-core-rpc`. If Ark schema or storage semantics change, this bridge can drift.
- There is no conflict-resolution layer for concurrent edits of the same `game_obj`; local Arrancador updates effectively behave as last-write-wins for the fields it rewrites.
- No automated runtime/integration test was added for the new Ark object bridge; verification is limited to TypeScript compilation on the current worktree.

## Unresolved

- Existing library rows are not proactively backfilled with persisted `ark_object_id`; inferred matches can hydrate reads, but the id is only stored on a future Arrancador write.
- Local game deletion does not soft-delete the linked Ark object. This avoids surprising cross-app deletion, but leaves orphaned `game_obj` rows possible.
- `exe_path` is written to Ark for visibility, but Arrancador does not accept remote `exe_path` edits back from Ark because the path is machine-specific.
- `save_path` is only adopted from Ark when Arrancador has no local `save_path`, to reduce the risk of remote path sync overriding a working local backup path.
