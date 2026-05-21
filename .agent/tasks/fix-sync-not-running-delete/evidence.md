# Evidence: fix-sync-not-running-delete

## Summary

Implemented a minimal Electron main-process fix to treat `Sync not running` as stale runtime state instead of a noisy operational warning during task deletion and other sync IPC paths.

## Acceptance criteria

### AC1 — PASS

When `lan-sync:broadcastChange` receives a sidecar error whose message contains `Sync not running`, Electron main now routes it through `handleStaleSyncRuntime(...)`, resets local sync state, logs a low-noise `console.info(...)`, and returns `false`.

Proof:

- Code: `apps/delphi/ts/electron/main.ts:565-576`
- Helper detection: `apps/delphi/ts/electron/main.ts:75-91`
- Raw artifact: `.agent/tasks/fix-sync-not-running-delete/raw/code-locations.txt`
- Raw diff: `.agent/tasks/fix-sync-not-running-delete/raw/main.diff`

### AC2 — PASS

Electron main now centralizes stale-runtime cleanup in `resetSyncRuntimeState()` and calls it from stale sidecar handling, explicit stop, and leave-space flows.

Proof:

- State reset helper: `apps/delphi/ts/electron/main.ts:79-84`
- Stale runtime handler: `apps/delphi/ts/electron/main.ts:86-92`
- Stop path uses helper: `apps/delphi/ts/electron/main.ts:530-536`
- Leave-space path uses helper: `apps/delphi/ts/electron/main.ts:625-631`
- Raw artifact: `.agent/tasks/fix-sync-not-running-delete/raw/code-locations.txt`

### AC3 — PASS

`lan-sync:getStatus` now converts stale runtime detection into `{ active: false, peers: 0, peerNames: [] }` instead of returning `active: true` after the sidecar has already lost the sync runtime.

Proof:

- Code: `apps/delphi/ts/electron/main.ts:540-561`
- Raw artifact: `.agent/tasks/fix-sync-not-running-delete/raw/code-locations.txt`

### AC4 — PASS

The Delphi Electron node-side TypeScript still type-checks after the change.

Proof:

- Command: `cd /workspace/apps/delphi/ts && ./node_modules/.bin/tsc -p tsconfig.node.json`
- Exit code: `0`
- Raw artifacts:
  - `.agent/tasks/fix-sync-not-running-delete/raw/tsc-node.meta`
  - `.agent/tasks/fix-sync-not-running-delete/raw/tsc-node.txt`

## Commands run

- `cd /workspace/apps/delphi/ts && ./node_modules/.bin/tsc -p tsconfig.node.json`
- `cd /workspace && rg -n "isSyncNotRunningError|resetSyncRuntimeState|handleStaleSyncRuntime|lan-sync:getStatus|lan-sync:broadcastChange|return \{ active: false, peers: 0, peerNames: \[\] \}" apps/delphi/ts/electron/main.ts`
- `cd /workspace && git diff -- apps/delphi/ts/electron/main.ts`

## Notes

- I also attempted `oxlint` on `electron/main.ts`, but the workspace image is missing the native `oxlint` binding on this machine. This did not block the required ACs because AC4 only requires TypeScript type-checking.
