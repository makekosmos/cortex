# Task: Dashboard Reads Usage Analytics Through ARK SDK

## Context

`apps/dashboard/electron/services/analytics.ts` currently opens ARK SQLite directly and duplicates usage analytics SQL. ARK now exposes `get_usage_analytics` through `ark-core-rpc` and `@arksync/node`.

## Scope

- Replace Dashboard runtime analytics SQL with `@arksync/node` usage analytics calls.
- Keep DB path resolution/status behavior in Dashboard.
- Add binary path resolution for `ark-core-rpc` in Dashboard dev/packaged layouts.
- Add `@arksync/node` dependency and package `ark-core-rpc` with Dashboard.
- Update smoke analytics script for async loading.
- Add a focused verifier using an injected analytics provider.

## Acceptance Criteria

- AC1: Dashboard analytics service no longer imports `better-sqlite3`/`openSqliteDatabase` or runs usage SQL.
- AC2: Dashboard calls `usage.analytics.snapshot(...)` from an injected provider or `ArkClient`.
- AC3: Dashboard package builds/packages the canonical `ark-core-rpc` sidecar.
- AC4: Missing/unreadable DB status behavior is preserved without creating new DB files.
- AC5: Fresh verification passes: Dashboard typecheck/build, focused verifier, no-direct-SQL grep, and `git diff --check`.
