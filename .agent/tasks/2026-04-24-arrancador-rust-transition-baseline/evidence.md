# Evidence: Arrancador Rust Transition Baseline

## Acceptance Criteria

- AC1: PASS. Preserved pre-Rust snapshot at `apps/arrancador/bench/baselines/pre-rust-ts-electron-main-2026-04-24.json`.
- AC2: PASS. Added `apps/arrancador/bench/compare-baseline.ts`.
- AC3: PASS. Added `bench:compare` in `apps/arrancador/package.json`.
- AC4: PASS. Added `apps/arrancador/docs/backend-baseline.md`.
- AC5: PASS. Fresh `typecheck`, `test`, `biome:check`, `bench:baseline`, and `bench:compare` completed successfully.
- AC6: PASS. Parallel agent findings are recorded in `agent-findings.md` and used to select scan as the first Rust slice.

## Preserved Baseline

- Source proof artifact: `.agent/tasks/2026-04-24-arrancador-baseline-benchmarks/raw/baseline.json`
- In-tree baseline: `apps/arrancador/bench/baselines/pre-rust-ts-electron-main-2026-04-24.json`
- SHA256 match recorded in `raw/baseline-hashes.txt`.

## Verification

- `bun run typecheck`: PASS.
- `bun run test`: PASS, 24 files / 72 tests passed.
- `bun run biome:check`: PASS, 193 files checked.
- `bun run bench:baseline -- --out=..\..\.agent\tasks\2026-04-24-arrancador-rust-transition-baseline\raw\current.json`: PASS.
- `bun run bench:compare -- --current=..\..\.agent\tasks\2026-04-24-arrancador-rust-transition-baseline\raw\current.json --out=..\..\.agent\tasks\2026-04-24-arrancador-rust-transition-baseline\raw\compare-current.json`: PASS.

## Current Compare Result

`raw/compare-current.json` reports `correctnessPass: true`.

Performance deltas vary between repeated TS/Electron-main runs, which is expected on a local machine. The important transition baseline is the preserved snapshot, not a single subsequent current run.

## Raw Artifacts

- `raw/typecheck.txt`
- `raw/test.txt`
- `raw/biome-check.txt`
- `raw/bench-baseline.txt`
- `raw/current.json`
- `raw/current-summary.csv`
- `raw/compare-current.txt`
- `raw/compare-current.json`
- `raw/compare-self.txt`
- `raw/compare-self.json`
- `raw/baseline-hashes.txt`
- `raw/diff.txt`
- `raw/status.txt`

## Next Rust Slice

Start with a Rust sidecar implementation for executable scanning. Keep Electron main as the IPC boundary, keep renderer contracts unchanged, and compare the Rust scan benchmark against `pre-rust-ts-electron-main-2026-04-24.json`.
