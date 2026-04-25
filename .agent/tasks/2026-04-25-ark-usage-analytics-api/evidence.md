# Evidence: ARK Usage Analytics API

Verification result: PASS

## Acceptance Criteria

- AC1: PASS. `ark-core` now exposes `UsageAnalyticsSnapshot` with summary, daily trend, hourly heatmap, top apps, and recent sessions.
- AC2: PASS. `ark-core-rpc` handles `get_usage_analytics` with `range_days`, `top_apps_limit`, and `recent_sessions_limit`.
- AC3: PASS. `@arksync/node` exposes `usage.analytics.snapshot(...)`.
- AC4: PASS. Focused Rust test verifies summary aggregation and zero-filled daily trend points.
- AC5: PASS. Fresh Rust checks/tests/formatting, `@arksync/node` typecheck/build, SDK verifier, and `git diff --check` passed.

## Raw Artifacts

- `cargo-check.txt`
- `cargo-test-focused.txt`
- `cargo-test-full.txt`
- `cargo-fmt-check.txt`
- `arksync-node-typecheck.txt`
- `arksync-node-build.txt`
- `verify-arksync-node-usage-analytics.ts`
- `verify-arksync-node-usage-analytics.txt`
- `source-evidence.txt`
- `git-diff-check.txt`

## Notes

- `cargo test` still reports pre-existing warnings in `tests/relay_round_trip.rs`; all tests pass.
- `git diff --check` reports CRLF conversion warnings only and exits 0.
