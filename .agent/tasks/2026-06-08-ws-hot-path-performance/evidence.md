# Evidence

Task: `2026-06-08-ws-hot-path-performance`

Verified at: 2026-06-08

## Measurements

| Probe                                      |                                    Baseline |                        Fixed, no flags |                          Fixed + flags | Result / suspicion                                                                        | Next fix                                                                                     |
| ------------------------------------------ | ------------------------------------------: | -------------------------------------: | -------------------------------------: | ----------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| `app_index.list_all` warm p95              |                                      43.7ms |                                  2.9ms |                                  2.8ms | Inline base64 icon reads were the main hot-path cost.                                     | Implement real cached `kosmos-icon://app/<id>` protocol/endpoint for lazy visible-row icons. |
| `app_index.list_all` p95 payload           |                               373,977 bytes |                           20,106 bytes |                           20,103 bytes | Bulk icon payload was far above 200KB even with only 76 apps.                             | Keep list payload metadata-only; add payload-size alert if it regresses.                     |
| `app_index` icon reads after warm run      |                                       2,294 |                                      0 |                                      0 | `list_all` no longer reads icons in refresh path.                                         | Keep `search` top-N icon reads lazy or cached.                                               |
| `commands.list` warm p95                   |                                      0.25ms |                                 0.40ms |                                 0.22ms | Meets AC; flags do not materially change command bus list.                                | If real launcher still janks, investigate Electron main merge/render path.                   |
| Event storm ~1000/min, `commands.list` p95 |                                      0.91ms |                                 0.71ms |                                 0.77ms | No dramatic p95 growth; removing `biased` keeps incoming RPC fair in this probe.          | Add a unit/integration stress test if future event fan-out grows.                            |
| `file_index.search` warm p95               |                                      0.29ms |                                 0.66ms |                                 0.39ms | Current isolated index is empty, but work is now off the WS task via `spawn_blocking`.    | Re-run with a large seeded file-index DB before optimizing ranking/FTS.                      |
| Backend `diagnostics.snapshot`             |                                   Collected |                              Collected |                              Collected | Payload-byte stats now appear for hot operations.                                         | Extend payload stats to all local branches if needed.                                        |
| `typeperf 120s`                            | Failed: counter not specified / policy text |                            Failed same |                            Failed same | `typeperf` cannot collect these counters from this non-interactive shell on this machine. | Run elevated/perf-log-user WPR/typeperf outside Codex if OS-level counters are required.     |
| Fallback 120s process sampler              |                       Harness syntax failed | 120 samples, CPU +1.30s, WS max 45.1MB | 120 samples, CPU +1.19s, WS max 43.8MB | Backend idle/storm CPU stable after fix; flags mostly disable usage diagnostics state.    | Fix baseline sampler is unnecessary unless comparing exact process CPU before code change.   |
| Electron `diagnostics.metrics`             |               Not collected before code fix |                          Not collected |   Collected in headless with flat flag | Browser 113.9MB WS, Network 58.0MB, Tab 70.3MB; GPU software-disabled under headless.     | Repeat headed/manual if visual compositor behavior is the target.                            |
| `windowMoveBenchmark`                      |               Not collected before code fix |                          Not collected |   flatTest p95 32ms, launcher p95 32ms | Headless benchmark shows stable intervals but not real visible compositor performance.    | Use WPR/headed capture if user-visible window dragging remains janky.                        |

Flags for fixed + flags run:

```text
KEPLER_USAGE_TRACKER=0
KEPLER_SKIP_SYNC=1
KEPLER_FILE_INDEX_INITIAL_RESCAN=0
KOSMOS_WINDOW_EFFECTS=flat
```

Raw artifacts:

- `.agent/tasks/2026-06-08-ws-hot-path-performance/raw/baseline-backend-bench.json`
- `.agent/tasks/2026-06-08-ws-hot-path-performance/raw/after-no-flags-backend-bench.json`
- `.agent/tasks/2026-06-08-ws-hot-path-performance/raw/after-flags-backend-bench.json`
- `.agent/tasks/2026-06-08-ws-hot-path-performance/raw/after-no-flags-process-sampler.csv`
- `.agent/tasks/2026-06-08-ws-hot-path-performance/raw/after-flags-process-sampler.csv`
- `.agent/tasks/2026-06-08-ws-hot-path-performance/raw/electron-after-flags-electron-bench.json`

## AC1

Verdict: PASS with recorded probe gaps.

Evidence:

- Backend baseline and A/B were collected through `raw/bench-backend.mjs`.
- `diagnostics.snapshot` was collected for baseline, fixed/no-flags, and fixed/flags.
- `typeperf` was attempted on every backend run and failed with status `4026531842`; raw failure text is stored in each JSON.
- 120s fallback process sampler succeeded for fixed/no-flags and fixed/flags.
- Electron `diagnostics.metrics` and `windowMoveBenchmark` were collected through `raw/bench-electron.mjs` after the fix with `KOSMOS_WINDOW_EFFECTS=flat`.

## AC2

Verdict: PASS.

Evidence:

- Fixed/no-flags: `app_index.list_all` warm p95 `2.9ms`, payload p95 `20,106` bytes.
- Fixed/flags: `app_index.list_all` warm p95 `2.8ms`, payload p95 `20,103` bytes.
- Both are below p95 `< 100ms` and payload `< 200KB`.

## AC3

Verdict: PASS.

Evidence:

- Fixed/no-flags: `commands.list` warm p95 `0.40ms`.
- Fixed/flags: `commands.list` warm p95 `0.22ms`.
- Both are below `50ms`.

## AC4

Verdict: PASS.

Evidence:

- `app_index.list_all` no longer reads icon files or base64-encodes in `handle_app_index_op`.
- `app_index.search` uses `inline_icon_data_url()`, which wraps FS read/base64 work in `tokio::task::spawn_blocking`.
- `file_index.search` dispatch wraps `FileIndex::search()` in `tokio::task::spawn_blocking`.

## AC5

Verdict: PASS.

Evidence:

- `tokio::select!` in `handle_connection` no longer uses `biased`.
- Event storm run sent/received ~965 events in 60s.
- Fixed/no-flags storm `commands.list` p95: `0.71ms`.
- Fixed/flags storm `commands.list` p95: `0.77ms`.
- Warm no-storm `commands.list` p95 was `0.40ms` and `0.22ms`; absolute storm p95 stayed under 1ms.

## AC6

Verdict: PASS.

Commands:

- `cargo fmt --all --manifest-path Cargo.toml` — PASS.
- `cargo check --manifest-path Cargo.toml -p kepler-backend` — PASS.
- `cargo test --manifest-path Cargo.toml -p kepler-backend diagnostics` — PASS.
- `bun run --cwd platform/desktop typecheck` — PASS.
- `bun run ark:guard:writes` — PASS.
- `bun run docs:sync` — PASS.
- `bun run docs:check` — PASS.
- `bun run ark:smoke` — PASS.
