# Task Spec: Arrancador Baseline Benchmarks

## Original Task

Independently measure current Arrancador backend behavior and record objective baseline numbers before any Rust migration.

## Scope

Add and run a repeatable baseline benchmark suite for the current TypeScript/Electron-main backend:

- executable scan throughput and time-to-first-result,
- scan cancellation latency,
- backup create/restore throughput and hash parity,
- SQLite init/insert/read/search latency,
- event-loop lag during each operation.

## Acceptance Criteria

- AC1: A reusable benchmark script exists under `apps/arrancador/bench/`.
- AC2: `package.json` exposes a command to run the baseline benchmark.
- AC3: The benchmark writes structured JSON results.
- AC4: Baseline results are recorded under `.agent/tasks/2026-04-24-arrancador-baseline-benchmarks/raw/`.
- AC5: Correctness checks in the benchmark verify expected scan counts and backup restore hash parity.
- AC6: Fresh verification passes for `bun run typecheck`, `bun run test`, and `bun run bench:baseline`.

## Non-Goals

- No Rust implementation yet.
- No performance gate thresholds yet.
- No changes to app runtime behavior.
- No UI work.

## Verification Plan

1. Run `bun run typecheck`.
2. Run `bun run test`.
3. Run `bun run bench:baseline -- --out=<task raw path>/baseline.json`.
4. Record command outputs and summarize results in evidence.
