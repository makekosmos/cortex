# Evidence

## Result

PASS

## Acceptance Criteria

### AC1

PASS

- `Arrancador` still resolves current target `ark.db` from the shared selection file, but now runtime state also stores the active Ark DB path and refreshes when it changes.
- Runtime refresh logic is wired in [backend.ts](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/electron/main/backend.ts) via [ark-runtime.ts](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/electron/main/ark-runtime.ts).

### AC2

PASS

- Every IPC-backed operation now passes through `getRuntimeState()`, which compares the current shared selected space against the cached runtime path and recreates Ark-dependent services when needed.
- Startup Ark sync also uses the refreshed runtime instead of the originally captured startup services.

### AC3

PASS

- Regression coverage added in [ark-runtime.test.ts](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/electron/main/ark-runtime.test.ts).
- Test verifies both no-op reuse when the Ark DB target is unchanged and service recreation when the target path changes.

### AC4

PASS

- `bun test electron/main/ark-runtime.test.ts` — PASS
- `bun run test` — PASS
- `bun run build:main` — PASS

## Raw Artifacts

- [bun-test-runtime.txt](/D:/Personal/Hobby/Coding/kepler/.agent/tasks/2026-04-21-arrancador-shared-ark-follow/artifacts/bun-test-runtime.txt)
- [vitest.txt](/D:/Personal/Hobby/Coding/kepler/.agent/tasks/2026-04-21-arrancador-shared-ark-follow/artifacts/vitest.txt)
- [build-main.txt](/D:/Personal/Hobby/Coding/kepler/.agent/tasks/2026-04-21-arrancador-shared-ark-follow/artifacts/build-main.txt)
