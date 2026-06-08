# 2026-06-08 — Usage tracker window snapshot

## Context

Usage tracker currently checks every active session independently. Each tick samples the foreground window, then every active session calls `process_window_state`, which resolves the process image path and scans all top-level windows with `EnumWindows`. With many active sessions and many windows this becomes roughly `active_sessions * windows` polling every second.

## Scope

In scope:

- Build one top-level window snapshot per tick and reuse it for all active sessions.
- Cache `pid -> normalized_exe_path` with process birth-time validation to avoid stale PID reuse.
- Bound active sessions by pruning long-hidden inactive entries and applying a hard session cap.
- Add diagnostics counters for tick duration, active sessions, EnumWindows calls, process path queries, and visible window count.
- Preserve `KEPLER_USAGE_TRACKER=0` as the A/B kill switch.

Out of scope:

- Changing ARK schema or write APIs.
- Turning usage tracker into a service or adding UI.
- Proving real desktop CPU deltas beyond local/static verification if a representative long-running desktop benchmark is not available in this run.

## Acceptance Criteria

**AC1.** `enum_windows_calls_per_tick == 1` while usage tracker is enabled and processing active sessions; no code path calls `EnumWindows` once per active session.

**AC2.** `usage_tracker.tick_ms` diagnostics are emitted and the tick implementation is structured so a representative desktop can validate p95 `< 10ms` from those counters.

**AC3.** `KEPLER_USAGE_TRACKER=0` still disables tracker startup, and diagnostics expose enough counters to compare `KEPLER_USAGE_TRACKER=0` vs `1` idle CPU after the fix.

**AC4.** Process path caching validates PID reuse using process birth/start time before reusing a cached normalized exe path.

**AC5.** Active sessions are bounded: long-hidden inactive sessions are finalized, and a hard cap prevents unbounded in-memory growth.

**AC6.** Regression tests cover the new one-snapshot/session-pruning/cache contracts at unit level where possible, and required Kosmos guards for `platform/runtime/src/usage_tracker` are run or explicitly reported if blocked.
