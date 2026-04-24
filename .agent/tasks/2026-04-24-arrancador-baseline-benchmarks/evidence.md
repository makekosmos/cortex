# Evidence: Arrancador Baseline Benchmarks

## Acceptance Criteria

- AC1: PASS. Reusable benchmark scripts added under `apps/arrancador/bench/`.
- AC2: PASS. `apps/arrancador/package.json` exposes `bench:baseline`.
- AC3: PASS. Benchmark writes structured JSON with schema version, backend id, fixture metadata, timings, throughput, and event-loop lag.
- AC4: PASS. Baseline output recorded at `raw/baseline.json`; summary CSV recorded at `raw/baseline-summary.csv`.
- AC5: PASS. Benchmark fails if executable scan count differs from expected count or restored backup hash differs from the source fixture.
- AC6: PASS. Fresh `typecheck`, `test`, `biome:check`, and `bench:baseline` runs completed successfully.

## Baseline Snapshot

Backend under test: `ts-electron-main`.

| Metric | Result |
| --- | ---: |
| scan_executables_stream | 136.19 ms |
| scan throughput | 52,867.32 files/s |
| scan time to first result | 3.95 ms |
| scan cancellation latency | 0.45 ms |
| backup create directory | 293.32 ms |
| backup create throughput | 5.59 MB/s |
| backup restore directory | 260.56 ms |
| backup restore throughput | 6.30 MB/s |
| sqlite open/init | 125.85 ms |
| sqlite insert 1000 games | 3621.66 ms |
| sqlite insert throughput | 276.12 rows/s |
| sqlite get all 1000 games | 8.96 ms |
| sqlite search | 0.69 ms |

Correctness markers:

- Scan fixture: 7200 files, 720 expected executables, 720 found executables.
- Backup fixture: 420 files, 1,720,320 bytes, restore hash parity `true`.

## Raw Artifacts

- `raw/baseline.json`
- `raw/baseline-summary.csv`
- `raw/bench-baseline.txt`
- `raw/typecheck.txt`
- `raw/test.txt`
- `raw/biome-check.txt`
- `raw/diff.txt`
- `raw/status.txt`

## Fresh Verification

- `bun run typecheck`: PASS.
- `bun run test`: PASS, 24 files / 72 tests passed.
- `bun run biome:check`: PASS, 191 files checked.
- `bun run bench:baseline -- --out=..\..\.agent\tasks\2026-04-24-arrancador-baseline-benchmarks\raw\baseline.json`: PASS.

## Notes

The benchmark intentionally runs through Electron with `ELECTRON_RUN_AS_NODE=1` because the current backend uses `better-sqlite3` compiled for Electron, not regular Node or Bun.
