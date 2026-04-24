# Task Spec: Arrancador Rust Scan Sidecar

## Original Task

Move the first heavy Arrancador backend operation to Rust through an Electron sidecar, not Tauri. Preserve existing renderer and IPC contracts. Clarify the current SQLite state and how rusqlite/Ark should fit later.

## Scope

- Add an Arrancador Rust sidecar crate for executable scanning.
- Add a JSON-lines protocol between Electron main TypeScript and the Rust sidecar.
- Replace or wrap the existing executable scan implementation so `scan_executables_stream` can use the Rust sidecar while keeping IPC event semantics unchanged.
- Keep the TypeScript scan implementation as a fallback if the sidecar is unavailable.
- Add build/package wiring for the sidecar.
- Add tests for the Rust scan logic and TypeScript sidecar client/protocol behavior where practical.
- Run benchmark comparison against the preserved pre-Rust baseline.

## Acceptance Criteria

- AC1: A Rust sidecar crate exists under `apps/arrancador/sidecar/`.
- AC2: The sidecar supports executable scan over JSON-lines stdin/stdout and has Rust tests.
- AC3: Electron main has a sidecar client/service that streams scan entries through the existing `scan_executables_stream` IPC path without renderer API changes.
- AC4: The app can fall back to the TypeScript scanner if the sidecar cannot start.
- AC5: Package/build scripts include a native sidecar build path and packaging metadata includes the sidecar binary.
- AC6: Fresh verification passes for TypeScript checks/tests, Rust tests, sidecar scan smoke test, benchmark, and benchmark compare.
- AC7: Evidence records the current SQLite reality: Arrancador still uses `better-sqlite3`; `rusqlite`/Ark migration is a later sidecar/storage slice.

## Non-Goals

- Do not migrate SQLite or Ark persistence in this task.
- Do not reintroduce Tauri.
- Do not change renderer UI or public IPC names.
- Do not remove the TypeScript scan fallback yet.

## Verification Plan

1. Run sidecar Rust tests.
2. Run a sidecar scan smoke test on a fixture.
3. Run `bun run typecheck`.
4. Run `bun run test`.
5. Run `bun run biome:check`.
6. Run `bun run bench:baseline -- --out=<task raw path>/rust-scan-current.json`.
7. Run `bun run bench:compare -- --current=<task raw path>/rust-scan-current.json`.
8. Record raw outputs and final comparison.
