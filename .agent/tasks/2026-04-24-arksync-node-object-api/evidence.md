# Evidence: @arksync/node Object API

## Verdict

PASS

## Acceptance Criteria

- AC1: PASS. `@arksync/node` exports TypeScript types for `ArkObjectRecord`, `ArkObjectTypeRecord`, `ArkObjectLinkRecord`, and `ArkSearchResult`.
- AC2: PASS. `ArkClient.objects` exposes `list`, `get`, `upsert`, `delete`, and `search`.
- AC3: PASS. `ArkClient.objectTypes` exposes `list`, `get`, `upsert`, and `delete`.
- AC4: PASS. `ArkClient.links` exposes `list`, `upsert`, and `delete`.
- AC5: PASS. Self-managed object API calls initialize the sidecar DB before the first object request.
- AC6: PASS. Injected `requestFn` mode remains legacy-shaped and sends the expected existing `ark-core-rpc` operation names and payloads.
- AC7: PASS. Fresh TypeScript verification commands are recorded in this task directory.

## Raw Artifacts

- `typecheck.txt`: `bun run typecheck` in `packages/arksync-node`.
- `build.txt`: `bun run build` in `packages/arksync-node`.
- `verify-object-api.txt`: object API behavior proof script.
- `git-diff-check.txt`: whitespace/error diff check for touched files.
- `source-evidence.txt`: source locations for exported types, namespaces, and RPC operation mapping.
- `git-diff.txt`: current patch for the task.

## Commands

```text
cmd /c "bun run typecheck > D:\Personal\Hobby\Coding\kosmos\.agent\tasks\2026-04-24-arksync-node-object-api\typecheck.txt 2>&1"
cmd /c "bun run build > D:\Personal\Hobby\Coding\kosmos\.agent\tasks\2026-04-24-arksync-node-object-api\build.txt 2>&1"
cmd /c "bun D:\Personal\Hobby\Coding\kosmos\.agent\tasks\2026-04-24-arksync-node-object-api\verify-object-api.ts > D:\Personal\Hobby\Coding\kosmos\.agent\tasks\2026-04-24-arksync-node-object-api\verify-object-api.txt 2>&1"
cmd /c "git diff --check -- packages/arksync-node/src/ark-client.ts packages/arksync-node/src/index.ts .agent/tasks/2026-04-24-arksync-node-object-api/spec.md .agent/tasks/2026-04-24-arksync-node-object-api/verify-object-api.ts > D:\Personal\Hobby\Coding\kosmos\.agent\tasks\2026-04-24-arksync-node-object-api\git-diff-check.txt 2>&1"
```

## Notes

`git diff --check` returned exit code 0. It reported only Git line-ending warnings for `packages/arksync-node/src/ark-client.ts` and `packages/arksync-node/src/index.ts`.
