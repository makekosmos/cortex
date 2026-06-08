# Performance Diagnostics

## Context

Kosmos needs repeatable diagnostics for CPU, memory, GPU, backend counters, and window movement jitter so performance work can be based on measurements rather than guesses.

## Scope

In scope:

- Electron main-process diagnostics IPC for app metrics, GPU info/status, Chromium trace start/stop, and repeatable window move benchmark.
- Backend WS operation `diagnostics.snapshot` with JSON counters for RPC latency and subsystem state.
- Lightweight in-memory diagnostics collection for local runtime operations handled in `ws_server`.
- Type/preload contract updates where renderer/test code can call the diagnostics IPC.

Out of scope:

- Performance optimization based on the collected data.
- UI screens for these diagnostics.
- Destructive SQLite checks or direct writes to synced ARK tables.
- WPR/typeperf automation scripts.

## Acceptance Criteria

**AC1.** Electron exposes `kepler:diagnostics:metrics`, `kepler:diagnostics:trace-start`, `kepler:diagnostics:trace-stop`, and `kepler:diagnostics:window-move-benchmark` IPC handlers. Metrics include app process metrics, GPU feature status/basic info, and current BrowserWindow metadata.

**AC2.** The preload/type contract exposes diagnostics methods for metrics, trace start/stop, and window move benchmark without requiring renderer Node/Electron access.

**AC3.** Backend WS exposes `diagnostics.snapshot` returning JSON with `rpc.by_operation` latency/count stats and subsystem sections for `file_index`, `usage_tracker`, and `app_index`.

**AC4.** Local WS operations handled by `ws_server` record per-operation duration/count stats and include `diagnostics.snapshot` itself in the response shape without routing it to `ark-core-rpc`.

**AC5.** Verification includes relevant TypeScript/Rust checks and the repository-required substantial-task smoke/guard commands, or records any command that could not be run.
