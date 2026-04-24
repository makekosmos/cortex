# Arrancador E2E, Biome, And IPC Hardening Spec

## Task ID
2026-04-23-arrancador-e2e-biome-ipc-hardening

## Goal
Complete the remaining Arrancador follow-ups: verify and fix Playwright E2E, reduce Biome warning noise, and harden renderer-to-main IPC beyond TypeScript-only channel narrowing.

## Scope
- Run `bun run test:e2e` in `apps/arrancador` and fix failures caused by the Electron + Vue bridge migration.
- Reduce or eliminate actionable `bun run biome:check` warnings in active Arrancador source without hiding generated output.
- Replace renderer-facing generic IPC usage with explicit bridge methods where practical and add runtime validation for IPC channel/payload/event access.
- Preserve existing user changes outside `apps/arrancador` and task artifacts.

## Acceptance Criteria
- AC1: `bun run test:e2e` passes in `apps/arrancador`, or a documented external browser/runtime blocker is captured with raw output and no app-code failure remains.
- AC2: `bun run biome:check` exits successfully and active-source warning noise is materially reduced; remaining warnings, if any, are documented.
- AC3: Renderer IPC no longer relies only on compile-time typing; preload/shared bridge exposes explicit command methods or a typed command surface, and runtime validation rejects unknown channels/events and malformed payloads for security-sensitive handlers.
- AC4: Existing required checks still pass: `bun run typecheck`, `bun run test`, `bun run biome:check`, `bun run build:renderer`, `bun run build:main`, `bun run build:preload`.
- AC5: Evidence artifacts are written under `.agent/tasks/2026-04-23-arrancador-e2e-biome-ipc-hardening/`, including `evidence.md`, `evidence.json`, and raw command output.

## Non-goals
- Do not redesign the Arrancador UI.
- Do not change backend business behavior except where required to stabilize tests or IPC validation.
- Do not remove binary-only leftover Tauri assets if deletion is blocked by the execution environment.
