# Evidence: @arksync/node README

## Verdict

PASS

## Acceptance Criteria

- AC1: PASS. `packages/arksync-node/README.md` exists.
- AC2: PASS. The README documents self-managed sidecar and injected sidecar modes.
- AC3: PASS. The README documents that self-managed mode requires `dbPath` and runs `init` automatically before sync/object/usage calls.
- AC4: PASS. The README documents relay options as explicitly rejected today.
- AC5: PASS. The README includes examples for sync start, object API, object type/link API, and usage API.
- AC6: PASS. The README tells Electron main services to use the SDK instead of opening Ark SQLite directly.
- AC7: PASS. Fresh documentation verification commands are recorded in this task directory.

## Raw Artifacts

- `verify-readme.txt`: README content verification script output.
- `git-diff-check.txt`: whitespace/error diff check for touched files.
- `source-evidence.txt`: source locations for key documented sections.
- `git-diff.txt`: current patch for the task.
- `problems.md`: first-pass verifier issue and fix record.

## Commands

```text
cmd /c "bun D:\Personal\Hobby\Coding\kosmos\.agent\tasks\2026-04-24-arksync-node-readme\verify-readme.ts > D:\Personal\Hobby\Coding\kosmos\.agent\tasks\2026-04-24-arksync-node-readme\verify-readme.txt 2>&1"
cmd /c "git diff --check -- packages/arksync-node/README.md .agent/tasks/2026-04-24-arksync-node-readme/spec.md .agent/tasks/2026-04-24-arksync-node-readme/verify-readme.ts .agent/tasks/2026-04-24-arksync-node-readme/problems.md > D:\Personal\Hobby\Coding\kosmos\.agent\tasks\2026-04-24-arksync-node-readme\git-diff-check.txt 2>&1"
```
