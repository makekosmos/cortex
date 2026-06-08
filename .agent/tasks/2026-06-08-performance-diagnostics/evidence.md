# Evidence

Task: `2026-06-08-performance-diagnostics`

Verified at: 2026-06-08

## AC1

Verdict: PASS

Evidence:

- `platform/desktop/electron/diagnostics.ts` registers:
  - `kepler:diagnostics:metrics`
  - `kepler:diagnostics:trace-start`
  - `kepler:diagnostics:trace-stop`
  - `kepler:diagnostics:window-move-benchmark`
- Metrics include `app.getAppMetrics()`, `app.getGPUFeatureStatus()`, `app.getGPUInfo("basic")`, and current `BrowserWindow` metadata.
- `node_modules\.bin\tsc.exe --noEmit` passed from `platform/desktop`.
- Final `node scripts/ark-smoke.mjs` passed and included shell `tsc`, shell build, and extension builds.

## AC2

Verdict: PASS

Evidence:

- `platform/desktop/shared/ipc-types.ts` defines diagnostics metrics and window benchmark types.
- `platform/desktop/electron/preload.ts` exposes `metrics`, `traceStart`, `traceStop`, and `windowMoveBenchmark` under `window.kepler.diagnostics`.
- `node_modules\.bin\tsc.exe --noEmit` passed from `platform/desktop`.

## AC3

Verdict: PASS

Evidence:

- `platform/runtime/src/ws_server.rs` intercepts `diagnostics.snapshot`.
- Snapshot JSON includes:
  - `rpc`
  - `file_index`
  - `usage_tracker`
  - `app_index`
- `cargo check --manifest-path Cargo.toml -p kepler-backend` passed.
- Final `node scripts/ark-smoke.mjs` passed, including `cargo test --manifest-path platform/runtime/Cargo.toml --lib`.

## AC4

Verdict: PASS

Evidence:

- `platform/runtime/src/diagnostics.rs` adds bounded per-operation RPC samples with count, p50, p95, max.
- `platform/runtime/src/ws_server.rs` records durations for local intercepts and ark-host-routed requests.
- `diagnostics.snapshot` is observed and returned locally without routing to `ark-core-rpc`.
- Unit tests added:
  - `diagnostics::tests::rpc_snapshot_reports_percentiles_and_max`
  - `diagnostics::tests::rpc_samples_are_bounded_per_operation`
- Final `node scripts/ark-smoke.mjs` passed and ran 341 `kepler-backend` lib tests, including the new diagnostics tests.

## AC5

Verdict: PASS

Commands:

- `cargo fmt --all --manifest-path Cargo.toml` — PASS.
- `cargo check --manifest-path Cargo.toml -p kepler-backend` — PASS.
- `node_modules\.bin\tsc.exe --noEmit` from `platform/desktop` — PASS.
- `node scripts/check-ark-write-boundaries.mjs` — PASS.
- `node scripts/ark-smoke.mjs` — PASS.

Notes:

- `bun run ark:guard:writes`, `bun run ark:smoke`, and `bun run typecheck` reported "Script not found" in this shell session despite package.json containing the scripts. I ran the underlying script commands directly: `node scripts/check-ark-write-boundaries.mjs`, `node scripts/ark-smoke.mjs`, and `node_modules\.bin\tsc.exe --noEmit`.
- Some commands needed sandbox escalation because Cargo and TypeScript write build artifacts such as `target/debug/.cargo-lock` and `.tsbuildinfo`.
