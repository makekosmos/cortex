# Task Spec: Arrancador Save Locator Atomization

## Original Task

Push Arrancador quality beyond the previous score by making a real architectural/readability improvement, then verify and reassess.

## Scope

Refactor the largest backend backup service hotspot:

- Split save-root resolution from backup save discovery.
- Keep public backup service behavior unchanged.
- Add focused tests around save locator behavior.
- Preserve current IPC/API contracts.

## Acceptance Criteria

- AC1: `electron/main/services/backup/save-locator.ts` no longer owns environment token expansion, Steam path discovery, and heuristic root discovery details.
- AC2: A focused backup root-resolution module owns path context, manifest-root resolution, and heuristic-root resolution.
- AC3: Public functions exported by `save-locator.ts` keep their existing names and behavior.
- AC4: Electron main tests cover save locator public behavior with temporary filesystem fixtures.
- AC5: Fresh verification passes for `bun run typecheck`, `bun run test`, `bun run biome:check`, `bun run build:main`, and `bun run build:preload`.
- AC6: Evidence artifacts are written under `.agent/tasks/2026-04-23-arrancador-save-locator-atomization/`.

## Non-Goals

- No changes to backup IPC contracts.
- No changes to archive/copy/restore behavior.
- No new dependencies.
- No UI work.
- No broad rewrite of the backup engine.

## Verification Plan

1. Extract path/root resolution into a new module under `electron/main/services/backup/`.
2. Keep `save-locator.ts` focused on building unique roots, collecting files, and public save locator API.
3. Add temp-directory tests for override root discovery, file collection, save-path lookup, and path tokenization.
4. Run required checks and record raw outputs.
