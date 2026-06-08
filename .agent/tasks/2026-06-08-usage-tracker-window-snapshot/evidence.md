# Evidence — 2026-06-08 usage tracker window snapshot

Verified on 2026-06-08 in `D:\Personal\Hobby\Coding\kosmos`.

## AC1 — one EnumWindows per tick

Verdict: PASS.

- Static check: `rg -n "EnumWindows|process_has_visible_window|capture_foreground_window_with_diagnostics|KEPLER_USAGE_TRACKER" platform/runtime/src/usage_tracker platform/runtime/src/main.rs`
- Result: `EnumWindows` appears only in `platform/runtime/src/usage_tracker/windows_capture.rs:83`, inside `WindowSnapshot::capture`; `process_has_visible_window` no longer exists.
- Runtime smoke: `.agent/tasks/2026-06-08-usage-tracker-window-snapshot/raw/backend-diag-on.stderr.txt` contains `usage_tracker.enum_windows_calls_per_tick=1` in both diagnostics lines.

## AC2 — tick diagnostics and p95

Verdict: PASS for current desktop sample; representative long-day CPU comparison remains a manual follow-up if desired.

- Runtime command: hidden `kepler-backend.exe` run for 66s with isolated `KOSMOS_DATA_DIR`, `KOSMOS_DB_PATH`, `KEPLER_SKIP_SYNC=1`, `KEPLER_FILE_INDEX=0`, `USAGE_TRACKER_POLL_MS=1000`.
- Result: first diagnostics line reported `usage_tracker.tick_ms=5` and `usage_tracker.tick_ms.p95=5`; interval diagnostics after ~60s reported `usage_tracker.tick_ms=1` and `usage_tracker.tick_ms.p95=0`.
- Raw log: `raw/backend-diag-on.stderr.txt`.

## AC3 — KEPLER_USAGE_TRACKER A/B switch

Verdict: PASS.

- Runtime command: hidden `kepler-backend.exe` run for 5s with isolated data dir and `KEPLER_USAGE_TRACKER=0`.
- Result: stderr contains `[kepler-backend] KEPLER_USAGE_TRACKER=0 — usage_tracker disabled` and `ready`.
- Raw log: `raw/backend-diag-off.stderr.txt`.

## AC4 — PID path cache validates birth time

Verdict: PASS.

- Focused tests: `$env:CARGO_TARGET_DIR='.tmp/cargo-usage-tracker-test'; cargo test -p kepler-backend usage_tracker`
- Result: 15 passed, including `process_probe_cache_reuses_path_after_birth_time_probe`.
- Implementation: `ProcessProbeCache::resolve_normalized_exe_path` checks `GetProcessTimes` creation time before reusing cached normalized exe path.

## AC5 — active sessions are bounded

Verdict: PASS.

- Focused tests: `$env:CARGO_TARGET_DIR='.tmp/cargo-usage-tracker-test'; cargo test -p kepler-backend usage_tracker`
- Result: 15 passed, including `hidden_inactive_session_expires_after_ttl` and `overflow_session_keys_prunes_hidden_background_before_visible`.
- Implementation: hidden/background sessions finalize after `HIDDEN_INACTIVE_SESSION_TTL_MS`; `MAX_ACTIVE_SESSIONS` caps the map at 256 via pruning.

## AC6 — required guards

Verdict: PASS.

- `cargo fmt -p kepler-backend` passed.
- `$env:CARGO_TARGET_DIR='.tmp/cargo-usage-tracker-test'; cargo test -p kepler-backend usage_tracker` passed: 15/15.
- `node scripts/check-ark-write-boundaries.mjs` passed: `ARK write boundary guard passed.`
- `node scripts/ark-smoke.mjs` with `ARK_SMOKE_TASK_ID=2026-06-08-usage-tracker-window-snapshot` and `CARGO_TARGET_DIR=.tmp/cargo-ark-smoke` passed: `ARK smoke matrix passed.`
- `node scripts/sync-agents-docs.mjs` passed.
- `node scripts/check-docs-freshness.mjs` passed: `stale references не найдено`.

Note: `bun run ark:guard:writes` and `bun run docs:check` returned `Script not found` in this shell even though root `package.json` contains the scripts, so the same source scripts were run directly with `node`.
