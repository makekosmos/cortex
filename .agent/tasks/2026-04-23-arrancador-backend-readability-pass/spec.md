# Task Spec: Arrancador Backend Readability Pass

## Original Task

Review Arrancador again, make a concrete improvement plan, execute it, then reassess architecture, readability, atomicity, and testing on a 10-point scale. Also clarify what the backend is currently built on.

## Scope

Improve the highest-value backend readability and architecture weak spots without changing product behavior:

- Keep Electron bootstrap/lifecycle code focused on app runtime orchestration.
- Move service factory composition out of `electron/main/backend.ts`.
- Make feature IPC handler modules easier to read and review.
- Add a regression guard that preserves the split.

## Acceptance Criteria

- AC1: `electron/main/backend.ts` no longer owns the detailed service factory graph; service composition lives in a dedicated module.
- AC2: Runtime behavior and public IPC channel names remain unchanged.
- AC3: Dense one-line IPC handlers in the game/app/shell-scan modules are reformatted into readable handler blocks.
- AC4: Architecture tests guard the new runtime-services/backend boundary.
- AC5: Fresh verification passes for `bun run typecheck`, `bun run test`, `bun run biome:check`, `bun run build:renderer`, `bun run build:main`, and `bun run build:preload`.
- AC6: Evidence artifacts are written under `.agent/tasks/2026-04-23-arrancador-backend-readability-pass/`.

## Non-Goals

- No UI redesign.
- No IPC contract changes.
- No migration of Vue beta dependencies.
- No broad backend service rewrites.
- No changes outside Arrancador except proof artifacts.

## Verification Plan

1. Inspect relevant backend and test files.
2. Apply the smallest refactor that separates service composition from Electron runtime lifecycle.
3. Reformat focused IPC handler modules.
4. Extend architecture boundary tests.
5. Run all required checks fresh and record raw output.
