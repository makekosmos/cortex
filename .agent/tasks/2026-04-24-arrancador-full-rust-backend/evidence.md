# Evidence: Arrancador Full Rust Backend Migration

## Acceptance Criteria

- AC1: PASS. Executable scanning defaults to Rust sidecar with TypeScript fallback.
- AC2: PASS. Directory backup create/restore measured by the benchmark defaults to Rust sidecar with TypeScript fallback.
- AC3: PASS WITH BOUNDS. Save file enumeration remains TypeScript in this pass; the heavy copy/restore work moved to Rust. Full save discovery should move with a dedicated `backup_discover_saves` sidecar API because it includes registry/env/Steam/glob behavior.
- AC4: PASS WITH BOUNDS. Ark direct `better-sqlite3` access was not expanded. Existing access is documented as the next Rust API target: `list_game_objects`, sync-aware object upsert, usage aggregate/query, backfill, and migration APIs in `ark-core-rpc`.
- AC5: PASS. Launcher-local `arrancador.db` remains local storage. It should be ported to Rust implementation only with a separate local DB access-layer migration, not moved into Ark.
- AC6: PASS. Build/package wiring includes the Rust sidecar.
- AC7: PASS. Fresh verification passed.
- AC8: PASS. Final benchmark comparison is recorded in `raw/final-compare.json` and summarized below.

## Final Benchmark Summary

Baseline: `ts-electron-main`.

Current: `electron-main+rust-sidecar`.

`raw/final-compare.json` reports `correctnessPass: true`.

| Operation                | Metric               |          Baseline |           Current | Result           |
| ------------------------ | -------------------- | ----------------: | ----------------: | ---------------- |
| scan_executables_stream  | duration             |         136.19 ms |         105.29 ms | improved 22.69%  |
| scan_executables_stream  | p95 event-loop lag   |           0.44 ms |           0.07 ms | improved 84.09%  |
| scan_executables_stream  | throughput           | 52,867.32 files/s | 68,382.56 files/s | improved 29.35%  |
| scan_executables_stream  | time to first result |           3.95 ms |           6.40 ms | regressed 62.03% |
| scan_executables_stream  | found executables    |               720 |               720 | same             |
| scan_executables_cancel  | duration             |           5.77 ms |           5.99 ms | regressed 3.81%  |
| scan_executables_cancel  | cancel latency       |           0.45 ms |           0.34 ms | improved 24.44%  |
| backup_create_directory  | duration             |         293.32 ms |         225.38 ms | improved 23.16%  |
| backup_create_directory  | p95 event-loop lag   |           1.48 ms |           0.27 ms | improved 81.76%  |
| backup_create_directory  | throughput           |         5.59 MB/s |         7.28 MB/s | improved 30.23%  |
| backup_restore_directory | duration             |         260.56 ms |         192.96 ms | improved 25.94%  |
| backup_restore_directory | p95 event-loop lag   |           0.50 ms |           0.36 ms | improved 28.00%  |
| backup_restore_directory | throughput           |         6.30 MB/s |         8.50 MB/s | improved 34.92%  |
| backup_restore_directory | hash parity          |              true |              true | same             |

SQLite rows remain `ts-electron-main` in the final benchmark. Their apparent improvements are local run variance, not a claimed Rust DB migration.

## Verification

- `cargo test --manifest-path sidecar/Cargo.toml`: PASS, 4 tests.
- `cargo build --release --manifest-path sidecar/Cargo.toml`: PASS.
- Sidecar smoke: scan PASS, backup copy/restore PASS.
- `bun run typecheck`: PASS.
- `bun run test`: PASS, 25 files / 75 tests.
- `bun run biome:check`: PASS.
- `bun run build:main`: PASS.
- `bun run bench:baseline -- --out=.../raw/final.json`: PASS.
- `bun run bench:compare -- --current=.../raw/final.json --out=.../raw/final-compare.json`: PASS.

## Current Architecture State

Moved to Rust sidecar by default:

- executable scan;
- directory backup copy/create;
- directory backup restore by manifest.

Still TypeScript/Electron main:

- save root discovery and file enumeration;
- zip compression/expansion and legacy mapping restore;
- backup listing/deletion/pruning orchestration;
- launcher-local `arrancador.db`;
- direct existing Ark DB access.

## Next Required Rust APIs

To finish the DB side safely, add or expand `ark-core-rpc` APIs instead of hand-opening Ark DBs from Arrancador:

- `list_game_objects`;
- sync-aware `apply_local_object`;
- usage aggregate/daily/process query APIs;
- `backfill_legacy_usage`;
- `migrate_game_objects`.

Then replace Arrancador's direct Ark `better-sqlite3` services behind existing service contracts.

## Raw Artifacts

- `raw/final.json`
- `raw/final-summary.csv`
- `raw/final-compare.json`
- `raw/final-compare.csv`
- `raw/final-compare.txt`
- `raw/cargo-test.txt`
- `raw/cargo-build-release.txt`
- `raw/backup-copy-smoke.txt`
- `raw/backup-restore-smoke.txt`
- `raw/typecheck.txt`
- `raw/test.txt`
- `raw/biome-check.txt`
- `raw/build-main.txt`
- `raw/bench-baseline.txt`
- `raw/diff.txt`
- `raw/status.txt`
