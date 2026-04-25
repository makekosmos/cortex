# Evidence: Arrancador Ark Game Migration Writes Through SDK

Verification result: PASS

## Acceptance Criteria

- AC1: PASS. `ark-game-migration.ts` no longer uses target `runInTransaction`, `targetDb`, or `INSERT` statements for Ark target writes; `no-direct-writes.txt` confirms.
- AC2: PASS. Missing `game_obj` type is created through `target.objectTypes.upsert`.
- AC3: PASS. Migrated/merged game objects are written through `target.objects.upsert`.
- AC4: PASS. Focused verifier covers new object migration, executable-path merge, unchanged-object skip, and type creation.
- AC5: PASS. Arrancador typecheck, focused Bun migration verifier, build:main, focused Biome check, no-direct-write grep, and `git diff --check` passed.

## Raw Artifacts

- `arrancador-typecheck.txt`
- `arrancador-build-main.txt`
- `biome-ark-game-migration.txt`
- `verify-ark-game-migration.ts`
- `verify-ark-game-migration.txt`
- `vitest-ark-game-migration.txt`
- `no-direct-writes.txt`
- `git-diff-check.txt`
- `source-evidence.txt`
- `git-diff.txt`
- `problems.md`

## Notes

- Vitest did not execute because Vite config loading fails in this environment with `spawn EPERM`; the focused Bun verifier exercises the same migration logic without Vite.
- `git diff --check` reports CRLF conversion warnings only and exits 0.
