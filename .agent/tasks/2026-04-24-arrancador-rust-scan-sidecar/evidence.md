# Evidence: Arrancador Rust Scan Sidecar

## Acceptance Criteria

- AC1: PASS. Rust sidecar crate exists under `apps/arrancador/sidecar/`.
- AC2: PASS. The sidecar supports `scan_executables` over JSON-lines stdin/stdout and has Rust tests.
- AC3: PASS. Electron main routes `scanExecutablesStream()` through the sidecar while keeping IPC command/event contracts unchanged.
- AC4: PASS. If the sidecar cannot start, scan falls back to the TypeScript scanner.
- AC5: PASS. Package scripts build the sidecar and `electron-builder.yml` packages the release binary as an extra resource.
- AC6: PASS. Fresh verification passed for Rust tests/build, TypeScript checks/tests, sidecar smoke, benchmark, compare, and Electron main bundle.
- AC7: PASS. SQLite state and rusqlite/Ark target are recorded below.

## Scan Comparison

Baseline: `ts-electron-main`.

Current: `electron-main+rust-scan-sidecar`.

`raw/compare-rust-scan.json` reports `correctnessPass: true`.

| Metric | Pre-Rust | Rust scan sidecar | Status |
| --- | ---: | ---: | --- |
| scan duration | 136.19 ms | 134.38 ms | improved 1.33% |
| scan p95 event-loop lag | 0.44 ms | 0.24 ms | improved 45.45% |
| scan throughput | 52,867.32 files/s | 53,579.40 files/s | improved 1.35% |
| time to first result | 3.95 ms | 7.47 ms | regressed 89.11% |
| found executables | 720 | 720 | same |
| cancel duration | 5.77 ms | 5.65 ms | improved 2.08% |
| cancel latency | 0.45 ms | 0.39 ms | improved 13.33% |

The remaining time-to-first-result regression is sidecar startup/protocol overhead. The next optimization is to keep the sidecar process warm instead of spawning it per scan.

## Verification

- `cargo test --manifest-path sidecar/Cargo.toml`: PASS, 3 Rust tests.
- `cargo build --release --manifest-path sidecar/Cargo.toml`: PASS.
- Sidecar smoke fixture: PASS, 2 expected executable events and final count 2.
- `bun run typecheck`: PASS.
- `bun run test`: PASS, 25 files / 75 tests.
- `bun run biome:check`: PASS.
- `bun run build:main`: PASS.
- `bun run bench:baseline -- --out=.../rust-scan-current.json`: PASS.
- `bun run bench:compare -- --current=.../rust-scan-current.json --out=.../compare-rust-scan.json`: PASS.

## SQLite / Ark State

Arrancador is not using `rusqlite` for its local app DB yet. Current state:

- `arrancador.db` is still opened from Electron main with `better-sqlite3`.
- Arrancador also still reads/writes selected-space Ark DB files directly through `better-sqlite3` in its Ark usage/object services.
- `rusqlite` already exists in `packages/ark-core/rust`; that is the right Rust-owned SQLite boundary for Ark data.

The later DB migration should not be a broad rewrite of every Arrancador SQLite call at once. The clean next target after scan is Ark-owned data only: usage reads/backfill, game object hydrate/sync, process search, and selected-space Ark DB switching through `ark-core-rpc` or an expanded `@arksync/node` facade. Keep launcher-local `arrancador.db` separate unless we define a dedicated migration plan.

## Raw Artifacts

- `raw/cargo-test.txt`
- `raw/cargo-build-release.txt`
- `raw/sidecar-smoke.txt`
- `raw/typecheck.txt`
- `raw/test.txt`
- `raw/biome-check.txt`
- `raw/build-main.txt`
- `raw/bench-baseline.txt`
- `raw/rust-scan-current.json`
- `raw/rust-scan-summary.csv`
- `raw/compare-rust-scan.txt`
- `raw/compare-rust-scan.json`
- `raw/compare-rust-scan.csv`
- `raw/diff.txt`
- `raw/status.txt`
