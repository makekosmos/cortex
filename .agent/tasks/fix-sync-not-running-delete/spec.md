# Task Spec: fix-sync-not-running-delete

## Metadata

- Task ID: fix-sync-not-running-delete
- Created: 2026-04-10
- Repo root: /workspace

## Guidance sources

- `/workspace/AGENTS.md`
- `/workspace/apps/delphi/AGENTS.md`
- `/workspace/.agents/skills/vue-best-practices/SKILL.md`
- `/workspace/.agents/skills/ark-sync/SKILL.md`

## Original task statement

User reports repeated Electron main-process errors like:

- `[Main] broadcast_change via sidecar failed: Error: Sync not running`

The latest occurrences happen while deleting tasks in Delphi Electron.

## Acceptance criteria

### AC1

When the sidecar reports `Sync not running` during `lan-sync:broadcastChange`, Electron main must treat it as a benign stale-sync-state condition, avoid noisy warning spam, and return `false` to the renderer.

### AC2

When Electron main detects `Sync not running` from sidecar sync operations, it must reset its in-memory sync state (`syncActive` and related peer/runtime cache) so subsequent status checks do not continue to report sync as active.

### AC3

`lan-sync:getStatus` must return `{ active: false, peers: 0, peerNames: [] }` after a stale runtime is detected instead of claiming sync is still active with zero peers.

### AC4

The Delphi Electron TypeScript code must still type-check after the change.

## Constraints

- Keep the diff minimal and scoped to Delphi Electron sync lifecycle handling.
- Do not change the wire format or Ark sidecar protocol.
- Do not change renderer CRUD semantics for task deletion.

## Non-goals

- Reworking the full sync architecture.
- Fixing unrelated Electron/macOS menu warnings.
- Adding new sync UI.

## Verification plan

- Inspect `apps/delphi/ts/electron/main.ts` for explicit stale-sync handling.
- Run `npx tsc -p apps/delphi/ts/tsconfig.node.json`.
- Confirm `lan-sync:getStatus` and `lan-sync:broadcastChange` use the shared stale-sync handling path.
