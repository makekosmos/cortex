# Arrancador Architecture Guards Spec

Task ID: `2026-04-23-arrancador-architecture-guards`

## Goal

Make the recent Arrancador quality improvements durable by adding automated architecture boundary checks for the main regressions discovered during review and refactoring.

## Acceptance Criteria

AC1. Renderer boundary regressions are guarded.

- Vue renderer files must not import Electron, Node builtins, Electron main modules, or Tauri APIs.
- Renderer code must continue using the bridge/API layer.

AC2. Deprecated runtime/test dependencies are guarded.

- Active Arrancador source must not import React runtime packages.
- Active Arrancador source must not import `bun:test`.
- Active Arrancador source must not reference Tauri runtime globals or APIs.

AC3. The guard is part of the default test suite.

- The architecture guard is a Vitest test included by `bun run test`.

AC4. Verification is fresh and recorded.

- Run `bun run typecheck`.
- Run `bun run test`.
- Run `bun run biome:check`.
- Attempt `bun run test:e2e`; if blocked by local Playwright spawn permissions, record the blocker.

AC5. Proof artifacts are recorded.

- Create `evidence.md`, `evidence.json`, and raw command artifacts under `.agent/tasks/2026-04-23-arrancador-architecture-guards/`.

## Non-Goals

- Do not change runtime behavior.
- Do not remove unrelated dirty worktree files.
- Do not rewrite Electron IPC in this pass.
