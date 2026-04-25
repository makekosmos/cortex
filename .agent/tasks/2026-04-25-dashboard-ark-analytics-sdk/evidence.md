# Evidence: Dashboard Reads Usage Analytics Through ARK SDK

Verification result: PASS

## Acceptance Criteria

- AC1: PASS. `apps/dashboard/electron/services/analytics.ts` no longer imports `openSqliteDatabase`/`better-sqlite3` or contains usage SQL; `no-direct-sql.txt` confirms.
- AC2: PASS. Dashboard analytics calls `usage.analytics.snapshot(...)` from an injected provider or `ArkClient`.
- AC3: PASS. Dashboard package now has `build:sidecar`, `build:sidecar:dev`, `@arksync/node`, and `extraResources` for `ark-core-rpc.exe`; full build passed.
- AC4: PASS. Focused verifier proves missing DB status returns without creating a DB file and injected analytics returns readable snapshot status.
- AC5: PASS. Dashboard typecheck/build, focused verifier, no-direct-SQL grep, and `git diff --check` passed.

## Raw Artifacts

- `dashboard-typecheck.txt`
- `dashboard-build.txt`
- `verify-dashboard-analytics.ts`
- `verify-dashboard-analytics.txt`
- `no-direct-sql.txt`
- `source-evidence.txt`
- `git-diff-check.txt`
- `git-diff.txt`
- `problems.md`

## Notes

- The first Dashboard build failed in sandbox because `electron-rebuild` could not spawn node-gyp (`spawn EPERM`). Re-running the same build outside sandbox succeeded.
- `git diff --check` reports CRLF conversion warnings only and exits 0.
