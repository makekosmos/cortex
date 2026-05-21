# Arrancador Electron + Vue Stabilization Spec

## Task ID

2026-04-23-arrancador-electron-vue-stabilization

## Goal

Stabilize `apps/arrancador` after the Electron + Vue migration by fixing the identified review findings and making the active verification path trustworthy.

## Scope

- Fix Ark usage read-model SQLite handle lifecycle.
- Align Arrancador documentation and scripts with the active Electron + Vue stack.
- Remove React/Tauri from active verification and package metadata where they are no longer active runtime paths.
- Make renderer tests and lint checks meaningful for the current Vue renderer.
- Preserve existing user work and avoid unrelated repository changes.

## Acceptance Criteria

- AC1: Ark usage/statistics reads close SQLite handles they open and avoid opening duplicate handles for one stats request.
- AC2: `apps/arrancador/AGENTS.md`, `README.md`, and package scripts describe Electron + Vue as the active stack and do not direct contributors to active React/Tauri work.
- AC3: Active package metadata and checks no longer include Tauri dependencies/scripts or React-specific test entrypoints for Arrancador.
- AC4: `biome:check` excludes generated/build output and does not fail because of generated renderer chunks.
- AC5: `bun run typecheck`, `bun run test`, `bun run biome:check`, `bun run build:renderer`, `bun run build:main`, and `bun run build:preload` pass in `apps/arrancador`.
- AC6: Evidence artifacts are written under `.agent/tasks/2026-04-23-arrancador-electron-vue-stabilization/`, including `evidence.md`, `evidence.json`, and raw command output.

## Non-goals

- Do not redesign Arrancador UI.
- Do not delete legacy source trees unless required for active verification.
- Do not modify unrelated apps/packages except where required by Arrancador's active imports.
