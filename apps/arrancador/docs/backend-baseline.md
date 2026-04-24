# Arrancador Backend Baseline

This file preserves the pre-Rust backend reference point for Arrancador.

## Current Backend

- Renderer: Vue/TypeScript.
- Backend boundary: Electron main process, TypeScript.
- Native storage: `better-sqlite3` loaded inside Electron main.
- Heavy operations currently measured here: executable scanning, backup create/restore, SQLite open/init/insert/read/search.

The preserved machine-readable baseline is:

- `apps/arrancador/bench/baselines/pre-rust-ts-electron-main-2026-04-24.json`

## Baseline Metrics

| Operation | Metric | Pre-Rust value |
| --- | --- | ---: |
| scan_executables_stream | duration | 136.19 ms |
| scan_executables_stream | throughput | 52,867.32 files/s |
| scan_executables_stream | time to first result | 3.95 ms |
| scan_executables_cancel | cancel latency | 0.45 ms |
| backup_create_directory | duration | 293.32 ms |
| backup_create_directory | throughput | 5.59 MB/s |
| backup_restore_directory | duration | 260.56 ms |
| backup_restore_directory | throughput | 6.30 MB/s |
| sqlite_open_init | duration | 125.85 ms |
| sqlite_insert_games | duration | 3621.66 ms |
| sqlite_insert_games | throughput | 276.12 rows/s |
| sqlite_games_get_all | duration | 8.96 ms |
| sqlite_games_get_all | throughput | 111,607.14 rows/s |
| sqlite_games_search | duration | 0.69 ms |

Correctness checks in the benchmark:

- Scan fixture: 7200 files, 720 expected executables, 720 found executables.
- Backup fixture: 420 files, 1,720,320 bytes, restored hash parity must be `true`.

## Commands

Run a fresh benchmark:

```powershell
bun run bench:baseline -- --out=bench/results/current.json
```

Compare a fresh benchmark with the preserved pre-Rust baseline:

```powershell
bun run bench:compare -- --current=bench/results/current.json
```

The compare command prints CSV-style rows and can also write structured JSON:

```powershell
bun run bench:compare -- --current=bench/results/current.json --out=bench/results/compare.json
```

## Rust Transition Rule

After a Rust backend slice is introduced, run the same benchmark fixture and compare the new JSON against the preserved pre-Rust baseline. A Rust migration is only objectively better when it preserves correctness and improves at least one target metric without hiding regressions in latency, throughput, cancellation, or event-loop lag.
