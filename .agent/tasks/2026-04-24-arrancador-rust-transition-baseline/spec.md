# Task Spec: Arrancador Rust Transition Baseline

## Original Task

Before starting the Rust backend transition, preserve the current Arrancador TypeScript/Electron-main backend state and benchmark numbers so later Rust runs can be compared objectively. Use several agents to inspect the project in parallel.

## Scope

- Preserve the current benchmark JSON as the immutable pre-Rust baseline in the project tree.
- Add a comparison command that can compare a future benchmark JSON against the preserved baseline.
- Add a short backend baseline document that explains what is being compared and why.
- Keep implementation focused on measurement and transition readiness; do not migrate backend operations to Rust in this task.

## Acceptance Criteria

- AC1: The pre-Rust benchmark snapshot is stored under `apps/arrancador/bench/baselines/`.
- AC2: A compare script exists under `apps/arrancador/bench/` and can compare another benchmark JSON with the preserved baseline.
- AC3: `apps/arrancador/package.json` exposes a compare command.
- AC4: A document under `apps/arrancador/docs/` records the current backend, preserved metrics, and how to rerun/compare.
- AC5: Fresh verification passes for `bun run typecheck`, `bun run test`, `bun run biome:check`, `bun run bench:baseline`, and the new compare command.
- AC6: Agent findings are recorded in task evidence and used to pick the first Rust migration slice.

## Non-Goals

- No Rust backend implementation yet.
- No IPC API changes yet.
- No renderer UI changes.
- No performance thresholds/gates beyond compare reporting.

## Verification Plan

1. Run `bun run typecheck`.
2. Run `bun run test`.
3. Run `bun run biome:check`.
4. Run `bun run bench:baseline -- --out=<task raw path>/current.json`.
5. Run the new compare command against `current.json`.
6. Record command outputs, comparison output, and agent summaries in evidence.
