# Evidence: @arksync/node Usage API

## Verdict

PASS

## Acceptance Criteria

- AC1: PASS. `@arksync/node` exports `ArkTrackedAppRecord`, `ArkUsageSessionRecord`, `ArkUsageEventRecord`, and `ArkUsageSnapshot`.
- AC2: PASS. `ArkClient.usage.loadAll()` returns only `trackedApps`, `usageSessions`, and `usageEvents` from `load_all`.
- AC3: PASS. `usage.trackedApps.upsert/delete` map to `upsert_tracked_app` and `delete_tracked_app`.
- AC4: PASS. `usage.sessions.upsert/delete` map to `upsert_usage_session` and `delete_usage_session`.
- AC5: PASS. `usage.events.upsert/delete` map to `upsert_usage_event` and `delete_usage_event`.
- AC6: PASS. Self-managed usage API calls initialize before the first usage request.
- AC7: PASS. Injected `requestFn` mode remains legacy-shaped and sends expected operation names and payloads.
- AC8: PASS. Fresh TypeScript verification commands are recorded in this task directory.

## Raw Artifacts

- `typecheck.txt`: `bun run typecheck` in `packages/arksync-node`.
- `build.txt`: `bun run build` in `packages/arksync-node`.
- `verify-usage-api.txt`: usage API behavior proof script.
- `git-diff-check.txt`: whitespace/error diff check for touched files.
- `source-evidence.txt`: source locations for exported types, namespace, and RPC operation mapping.
- `git-diff.txt`: current patch for the task.

## Commands

```text
cmd /c "bun run typecheck > D:\Personal\Hobby\Coding\kosmos\.agent\tasks\2026-04-24-arksync-node-usage-api\typecheck.txt 2>&1"
cmd /c "bun run build > D:\Personal\Hobby\Coding\kosmos\.agent\tasks\2026-04-24-arksync-node-usage-api\build.txt 2>&1"
cmd /c "bun D:\Personal\Hobby\Coding\kosmos\.agent\tasks\2026-04-24-arksync-node-usage-api\verify-usage-api.ts > D:\Personal\Hobby\Coding\kosmos\.agent\tasks\2026-04-24-arksync-node-usage-api\verify-usage-api.txt 2>&1"
cmd /c "git diff --check -- packages/arksync-node/src/ark-client.ts packages/arksync-node/src/index.ts .agent/tasks/2026-04-24-arksync-node-usage-api/spec.md .agent/tasks/2026-04-24-arksync-node-usage-api/verify-usage-api.ts > D:\Personal\Hobby\Coding\kosmos\.agent\tasks\2026-04-24-arksync-node-usage-api\git-diff-check.txt 2>&1"
```

## Notes

`git diff --check` returned exit code 0. It reported only Git line-ending warnings for `packages/arksync-node/src/ark-client.ts` and `packages/arksync-node/src/index.ts`.
