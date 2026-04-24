# Arrancador Quality 10 Continuation Spec

## Goal

Continue improving `apps/arrancador` beyond the previous 9+ pass while preserving all functionality. The code is still in development, so controlled experiments are allowed, but every experiment must be reversible, measured, and verified.

## Constraints

- Do not remove feature categories: game library, game detail, launch/process bindings, scan/import, backups, Ark usage, metadata, settings, catalogue, notifications, system tooling.
- Do not reintroduce React or Tauri runtime paths.
- Prefer small, verifiable extractions over broad rewrites.
- If a refactor cannot be protected by tests or build checks, skip it for this pass.

## Plan

1. Capture a baseline from the current post-9+ state.
2. Select low-risk hotspots that still hurt readability or atomicity.
3. Extract focused modules/components without changing public contracts.
4. Add tests for moved logic and contracts.
5. Run full verification and compare with baseline.

## Acceptance Criteria

- AC1: `bun run typecheck` passes in `apps/arrancador`.
- AC2: `bun run lint` passes in `apps/arrancador`.
- AC3: `bun run test` passes in `apps/arrancador`.
- AC4: `bun run test:coverage` passes in `apps/arrancador`.
- AC5: `bun run build:renderer`, `bun run build:main`, and `bun run build:preload` pass.
- AC6: At least one additional hotspot is improved in readability/atomicity, with file-size or module-boundary evidence.
- AC7: New or adjusted tests cover the refactored behavior.
- AC8: Baseline and final metrics are captured and compared.
- AC9: No IPC command/event surface is intentionally removed.
