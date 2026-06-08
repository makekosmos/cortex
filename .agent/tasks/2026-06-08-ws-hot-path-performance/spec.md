# WS Hot Path Performance

## Context

Launcher refresh can call `commands.list`, `app_index.list_all`, and `file_index.search` while backend event streams are active. Current `app_index.list_all` inlines base64 icons for up to 500 apps in the WS request path, and `file_index.search` executes synchronous SQLite/ranking work directly in the connection task. This can increase RPC latency, payload size, and UI jitter.

## Scope

In scope:

- Collect baseline diagnostics before optimization: Windows process counters, Electron diagnostics metrics, backend `diagnostics.snapshot`, and `windowMoveBenchmark`.
- Compare A/B runs with relevant runtime flags: `KEPLER_USAGE_TRACKER=0`, `KEPLER_SKIP_SYNC=1`, `KEPLER_FILE_INDEX_INITIAL_RESCAN=0`, `KOSMOS_WINDOW_EFFECTS=flat`.
- Remove inline bulk base64 icons from `app_index.list_all`; return lightweight icon references instead.
- Move blocking icon file reads/base64 encoding and file-index search work out of the async WS connection hot path.
- Improve WS select fairness so incoming RPC frames are not deprioritized behind outgoing event broadcasts.
- Log/diagnose RPC response payload sizes.
- Keep changes minimal and avoid unrelated command bus, sync, schema, or UI redesign work.

Out of scope:

- New user-facing diagnostics UI.
- Custom icon protocol implementation beyond returning stable renderer-safe references.
- Full WPR investigation unless baseline/A-B data remains ambiguous.
- Release/version bump.

## Acceptance Criteria

**AC1.** Baseline and A/B evidence includes typeperf-style 120s Windows process counters, Electron diagnostics metrics, backend `diagnostics.snapshot`, and `windowMoveBenchmark`, or records why a probe could not be collected.

**AC2.** `app_index.list_all` warm p95 is below 100ms in local evidence, and the response payload is below 200KB for the measured app set without inline 500-icon base64 data.

**AC3.** `commands.list` warm p95 is below 50ms in local evidence.

**AC4.** `file_index.search` and app icon base64 encoding no longer perform blocking FS/SQLite work directly in the WS connection task.

**AC5.** Under an event storm of at least 1000 events/min, warm RPC p95 does not grow dramatically relative to the no-storm run, and incoming frames are not biased behind outgoing event branches.

**AC6.** Verification includes focused Rust/TypeScript checks plus required substantial-task guards, or explicitly records any command that could not run.
