# 2026-04-23 Arrancador quality 10 pass

## Goal

Raise Arrancador quality in the places that currently cap architecture, readability, atomicity, testability, and runtime speed.

This task does not promise a mathematically perfect application. It must deliver concrete, verified improvements on the highest-impact hotspots found in the current codebase:

- route-level Vue components doing too much orchestration;
- library install-status checks that can fan out into many parallel IPC calls;
- IPC/main-process organization that is hard to verify;
- missing focused tests around new quality/performance boundaries.

## Acceptance Criteria

### AC1: Library install checks are bounded and cache-aware

The Library route must no longer blindly call `gamesApi.isInstalled` for every game on every deep game-list mutation. Install status resolution must:

- derive work from stable game identity/path signatures;
- reuse cached results for unchanged games;
- limit concurrent backend checks;
- ignore stale async results after a newer request starts.

### AC2: Library route is more atomic

Library import/drop orchestration and install-status orchestration must be moved out of `LibraryPage.vue` into focused composables. `LibraryPage.vue` should become primarily page composition, filtering state, and template wiring.

### AC3: New behavior is covered by focused tests

Add tests proving:

- unchanged games reuse cached install-status values;
- install checks are concurrency-limited;
- stale install-status runs do not overwrite newer results;
- dropped path import behavior remains covered through an extracted composable or pure helper boundary.

### AC4: Existing behavior remains green

Fresh verification must pass from the current codebase:

- `bun run typecheck` in `apps/arrancador`;
- `bun run test` in `apps/arrancador`.

### AC5: Proof artifacts exist

Write task artifacts under `.agent/tasks/2026-04-23-arrancador-quality-10-pass/`:

- `spec.md`;
- `evidence.md`;
- `evidence.json`;
- raw command outputs for verification.

If verification is not `PASS`, write `problems.md`, apply the smallest defensible fix, and re-run verification.
