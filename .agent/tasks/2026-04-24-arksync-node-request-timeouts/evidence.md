# Evidence: @arksync/node Request Timeouts

## Verdict

PASS

## Acceptance Criteria

- AC1: PASS. `ArkClientOptions` exposes optional `requestTimeoutMs`.
- AC2: PASS. Self-managed requests use `DEFAULT_REQUEST_TIMEOUT_MS` when no override is provided.
- AC3: PASS. Timed-out requests are removed and reject with an error containing the operation name.
- AC4: PASS. Matched responses clear timers and settle from the response.
- AC5: PASS. `failAll`, `killChild`, and direct stdin write failure clear timers before removing or rejecting pending requests.
- AC6: PASS. Injected `requestFn` mode is unchanged and is not wrapped by SDK-side timeouts.
- AC7: PASS. Fresh TypeScript verification commands are recorded in this task directory.

## Raw Artifacts

- `typecheck.txt`: `bun run typecheck` in `packages/arksync-node`.
- `build.txt`: `bun run build` in `packages/arksync-node`.
- `verify-request-timeouts.txt`: timeout behavior proof script.
- `git-diff-check.txt`: whitespace/error diff check for touched files.
- `source-evidence.txt`: source locations for timeout implementation.
- `git-diff.txt`: current patch for the task.

## Commands

```text
cmd /c "bun run typecheck > D:\Personal\Hobby\Coding\kepler\.agent\tasks\2026-04-24-arksync-node-request-timeouts\typecheck.txt 2>&1"
cmd /c "bun run build > D:\Personal\Hobby\Coding\kepler\.agent\tasks\2026-04-24-arksync-node-request-timeouts\build.txt 2>&1"
cmd /c "bun D:\Personal\Hobby\Coding\kepler\.agent\tasks\2026-04-24-arksync-node-request-timeouts\verify-request-timeouts.ts > D:\Personal\Hobby\Coding\kepler\.agent\tasks\2026-04-24-arksync-node-request-timeouts\verify-request-timeouts.txt 2>&1"
cmd /c "git diff --check -- packages/arksync-node/src/ark-client.ts .agent/tasks/2026-04-24-arksync-node-request-timeouts/spec.md .agent/tasks/2026-04-24-arksync-node-request-timeouts/verify-request-timeouts.ts > D:\Personal\Hobby\Coding\kepler\.agent\tasks\2026-04-24-arksync-node-request-timeouts\git-diff-check.txt 2>&1"
```

## Notes

`git diff --check` returned exit code 0. It reported only the existing Git line-ending warning for `packages/arksync-node/src/ark-client.ts`.
