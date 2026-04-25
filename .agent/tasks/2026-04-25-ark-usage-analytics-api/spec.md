# Task: ARK Usage Analytics API

## Context

Dashboard currently assembles usage analytics by opening ARK SQLite directly and running SQL in Electron main. The ARK runtime should own usage analytics queries so apps can call a stable API instead of depending on table internals.

## Scope

- Add usage analytics result types to `ark-core`.
- Add `db::load_usage_analytics` for summary, daily trend, hourly heatmap, top apps, and recent sessions.
- Expose the query through `ark-core-rpc`.
- Add `@arksync/node` `usage.analytics.snapshot(...)`.
- Add focused Rust and TypeScript verification.

## Acceptance Criteria

- AC1: `ark-core` exposes usage analytics data matching Dashboard's current shapes: summary, daily trend, hourly heatmap, top apps, recent sessions.
- AC2: `ark-core-rpc` has a request operation for usage analytics with range and limit parameters.
- AC3: `@arksync/node` exposes `usage.analytics.snapshot({ rangeDays, topAppsLimit, recentSessionsLimit })`.
- AC4: Focused Rust tests prove analytics are computed from ARK usage tables and zero-filled daily trend dates are included.
- AC5: Fresh verification passes: `cargo check`, focused/full `cargo test`, `cargo fmt --check`, `@arksync/node` typecheck/build, SDK verifier, and `git diff --check`.
