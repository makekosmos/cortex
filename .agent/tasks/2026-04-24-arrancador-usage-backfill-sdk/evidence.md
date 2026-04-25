# Evidence: Arrancador Usage Backfill Via SDK

## Verdict

PASS

## Acceptance Criteria

- AC1: PASS. Backfill tracked app writes now call `usage.trackedApps.upsert`.
- AC2: PASS. Backfill session writes now call `usage.sessions.upsert`.
- AC3: PASS. `ark-usage-backfill.ts` no longer contains manual `lan_sync.version_vector` writes or `bumpVersionVector`.
- AC4: PASS. Marker and backfill device id continue to use `sync_kv`.
- AC5: PASS. Existing tests still prove forced reruns skip existing sessions and avoid duplicating tracked apps.
- AC6: PASS. Fresh Arrancador TypeScript, tests, build, lint, and diff checks are recorded in this task directory.

## Raw Artifacts

- `typecheck.txt`: `bun run typecheck` in `apps/arrancador`.
- `test-ark-usage-backfill.txt`: focused Vitest run for usage backfill tests.
- `biome-check.txt`: focused Biome check for touched files.
- `build-main.txt`: Arrancador Electron main bundle build.
- `no-direct-usage-writes.txt`: proof that direct usage entity SQL and manual version vector writes are gone from the touched service.
- `git-diff-check.txt`: whitespace/error diff check for touched files.
- `source-evidence.txt`: source locations for SDK usage and retained marker/device-id behavior.
- `git-diff.txt`: current patch for the task.
- `problems.md`: failed first-pass checks and fixes.

## Notes

This task depends on `2026-04-24-ark-rpc-local-write-sync-state`, where `ark-core-rpc` usage writes were changed to update sync state.
