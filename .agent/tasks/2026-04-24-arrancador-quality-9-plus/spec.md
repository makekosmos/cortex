# Arrancador Quality 9+ Spec

## Goal

Raise `apps/arrancador` to at least 9/10 on architecture, readability, atomicity, and testing without removing existing app functionality.

## Current Risk Model

The likely risk areas are:

- Large files that still mix orchestration, domain rules, and UI details.
- Electron IPC contracts duplicated across type maps, preload registry, API wrappers, and handlers.
- Renderer coverage gate configured as 100% while actual measured coverage is much lower.
- Native/Rust and Electron main work that can regress filesystem/process/backup behavior if refactored too broadly.

## Plan

1. Capture baseline metrics before implementation.
2. Preserve public behavior and IPC/API surface unless a change is explicitly covered by tests.
3. Refactor high-impact hotspots with small, reversible steps:
   - Split backend/game service helpers where this reduces file size and coupling.
   - Split large Vue route/component concerns into composables or child components when contracts are clear.
   - Keep route pages as composition surfaces following Vue Composition API patterns.
4. Strengthen verification:
   - Keep `typecheck`, unit tests, and build checks green.
   - Make coverage gating honest: either improve coverage or scope thresholds to tested production logic rather than keeping an impossible global 100% gate.
   - Add focused tests for newly extracted logic and any behavior moved across files.
5. Re-measure after changes and compare with baseline.
6. If verification fails, record problems, apply the smallest safe fix, and re-run checks.

## Acceptance Criteria

- AC1: `bun run typecheck` passes in `apps/arrancador`.
- AC2: `bun run test` passes in `apps/arrancador`.
- AC3: `bun run build:renderer`, `bun run build:main`, and `bun run build:preload` pass in `apps/arrancador`.
- AC4: A coverage command exists and completes successfully with thresholds that reflect the intended checked surface; no fake global 100% gate may remain unless it actually passes.
- AC5: Existing active Arrancador IPC command names and event names remain present unless explicitly documented as removed; no feature category is dropped.
- AC6: Architecture boundary tests continue to pass and still enforce Electron/Vue-only active runtime boundaries.
- AC7: At least one major readability/atomicity hotspot is materially improved with smaller files or clearer module responsibility, with tests covering the moved behavior.
- AC8: Baseline and final metrics are captured in task artifacts and compared in `evidence.md`.
- AC9: Verification result is `PASS` only if all ACs pass against the current codebase and current command results.

## Non-Goals

- Do not redesign the UI.
- Do not remove backup, scan, Ark usage, process binding, catalogue, metadata, settings, notification, or system functionality.
- Do not reintroduce React or Tauri runtime paths.
