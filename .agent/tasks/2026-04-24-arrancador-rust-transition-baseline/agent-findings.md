# Agent Findings

## Backend Boundary Agent

- Renderer remains a pure Vue/TypeScript consumer of preload IPC helpers.
- Heavy work is currently in Electron main TypeScript, not renderer.
- Scan IPC is isolated around `scan_executables_stream`, `cancel_scan`, `get_running_processes`, and `scan:*` events.
- Backup work is heavier and riskier because it mutates user files and shells out for archives.
- SQLite is central persistence through `better-sqlite3` in Electron main.
- Recommended first Rust migration slice: `scan_executables_stream`, keeping Electron main as the IPC boundary and swapping only the implementation behind the existing scan service.

## Rust Tooling Agent

- Use the existing Electron sidecar model, not Tauri.
- Current architecture tests intentionally forbid `src-tauri` and Tauri scripts/packages.
- Reusable repo pattern exists in Delphi/Eden: build a Rust sidecar with Cargo, package it through Electron `extraResources`, communicate over JSON lines, and resolve dev vs packaged binary paths in Electron main.
- Rust crates in the repo use normal Cargo tests and independent `Cargo.lock` files rather than a root workspace.

## Benchmark Agent

- Preserve `apps/arrancador/bench/baselines/pre-rust-ts-electron-main-2026-04-24.json` as the canonical immutable baseline.
- Compare benchmark results by `name`, not array position.
- Keep result names stable after Rust migration.
- Treat correctness rows as hard failures: scan counts, backup hash parity, and SQLite row counts.
- SQLite event-loop lag currently has weak sampling because synchronous work can finish before interval sampling; keep it as historical data, but do not use it alone as a hard migration gate.

## Decision

The first Rust transition task should be a scan sidecar. It is read-only, already benchmarked, already cancelable, and has the smallest blast radius. Do not start with backup restore/delete or SQLite.
