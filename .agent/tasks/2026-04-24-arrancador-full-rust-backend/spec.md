# Task Spec: Arrancador Full Rust Backend Migration

## Original Task

Finish the Rust sidecar transition for Arrancador. The renderer and public IPC should remain stable, while heavy backend work moves out of TypeScript/Electron main and into Rust. Run final measurements only after the full migration pass.

## Scope

- Keep Electron main as the app orchestration and IPC boundary.
- Keep Vue renderer and preload/public IPC contracts unchanged.
- Extend the Arrancador Rust sidecar beyond scan to cover filesystem-heavy backup/save operations.
- Move Ark-owned SQLite access away from direct Arrancador `better-sqlite3` access where a compatible Rust/Ark boundary exists or can be added safely.
- Preserve the launcher-local `arrancador.db` unless replacing it can be done without breaking current service contracts and tests.
- Preserve TypeScript fallbacks where needed for safe rollout, but default heavy operations should prefer Rust.
- Record final benchmark comparison against the preserved pre-Rust baseline only after implementation and verification.

## Acceptance Criteria

- AC1: Rust sidecar remains the default path for executable scanning.
- AC2: Rust sidecar is the default path for backup create/restore directory operations measured by the benchmark.
- AC3: Save path discovery / filesystem-heavy backup helpers are either Rust-backed or explicitly documented as not safely migratable in this pass with a narrow reason.
- AC4: Ark-owned SQLite access no longer gains new direct `better-sqlite3` dependencies; existing direct Ark DB access is reduced where safely possible or documented with the exact next API needed in `ark-core-rpc`.
- AC5: Launcher-local `arrancador.db` ownership decision is documented: migrate now only if safe, otherwise keep as local store with bounded rationale.
- AC6: Build/package wiring includes the Rust sidecar.
- AC7: Fresh verification passes for Rust tests/build, TypeScript typecheck/tests/biome, Electron main build, sidecar smoke tests, benchmark, and compare.
- AC8: Final evidence includes only final benchmark comparison numbers against `apps/arrancador/bench/baselines/pre-rust-ts-electron-main-2026-04-24.json`.

## Non-Goals

- Do not reintroduce Tauri.
- Do not change renderer UI.
- Do not rename existing public IPC commands/events.
- Do not perform destructive data migration of user databases without an explicit reversible migration plan.

## Verification Plan

1. Run Rust sidecar tests.
2. Run sidecar smoke tests for scan and backup operations.
3. Run `bun run typecheck`.
4. Run `bun run test`.
5. Run `bun run biome:check`.
6. Run `bun run build:main`.
7. Run `bun run bench:baseline -- --out=<task raw path>/final.json`.
8. Run `bun run bench:compare -- --current=<task raw path>/final.json --out=<task raw path>/final-compare.json`.
9. Record raw outputs and final evidence.
