# Evidence: ARK read endpoints for Arrancador

Result: PASS

## Acceptance Criteria

- AC1 PASS: `@kosmos/ark` exposes object read helpers `listByType` and `getMany`; Arrancador usage reads use `ArkUsageApi.loadAll()` through `createArkUsageSnapshotLoader`.
- AC2 PASS: Arrancador game hydration, usage aggregation, playtime stats, and usage process search prefer `@kosmos/ark`; direct SQLite remains only as read-only fallback when the ARK runtime is unavailable.
- AC3 PASS: Dashboard remains documented as a read-only database inspector and was not converted away from read-only SQLite.
- AC4 PASS: Legacy Delphi replacement is documented in `docs/DELPHI-LEGACY-DB-DECISION.md`; Eden Heart/ARK split and open product decisions are documented in `docs/EDEN-HEART-ARK-BOUNDARY.md`.
- AC5 PASS: Fresh verification passed on isolated test/smoke databases.

## Verification

- PASS: `bun run --cwd packages/kosmos-ark typecheck`
- PASS: `bun run --cwd apps/arrancador test -- ark-usage ark-game-objects`
- PASS: `bun run --cwd apps/arrancador typecheck`
- PASS: `bun run ark:guard:writes`
- PASS: `bun run ark:smoke` after escalated rerun for Playwright/Electron worker spawn
- PASS: `git diff --check` with line-ending warnings only

## Notes

The first `bun run ark:smoke` attempt failed at the Eden Playwright step with `spawn EPERM` after earlier checks had passed. The same smoke command passed when rerun with escalation for Playwright/Electron process spawning.
