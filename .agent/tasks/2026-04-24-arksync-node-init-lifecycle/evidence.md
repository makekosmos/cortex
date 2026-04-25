# Evidence

Task: `2026-04-24-arksync-node-init-lifecycle`

Verification date: 2026-04-24

## Acceptance Criteria

### AC1

PASS. `ArkClientOptions` now includes `dbPath?: string` with documentation that it is required when `requestFn` is not provided and ignored when a sidecar request function is injected.

Raw evidence:

- `source-evidence.txt`
- `git-diff.txt`

### AC2

PASS. In self-managed mode, `start()` calls `ensureInitialized()` before `start_sync`. `ensureInitialized()` sends `{ operation: "init", dbPath }` only when no delegate request function exists and the client is not already initialized.

The lifecycle proof script verifies two consecutive `start()` calls produce:

```text
init -> start_sync -> start_sync
```

Raw evidence:

- `verify-lifecycle.ts`
- `verify-lifecycle.txt`
- `source-evidence.txt`

### AC3

PASS. In injected `requestFn` mode, `ensureInitialized()` returns without requiring `dbPath` and without sending `init`.

The lifecycle proof script verifies injected mode produces:

```text
start_sync
```

Raw evidence:

- `verify-lifecycle.ts`
- `verify-lifecycle.txt`

### AC4

PASS. Self-managed mode without `dbPath` throws:

```text
@arksync/node: dbPath is required when requestFn is not provided
```

The lifecycle proof script verifies no sidecar request is attempted before this error.

Raw evidence:

- `verify-lifecycle.ts`
- `verify-lifecycle.txt`

### AC5

PASS. `packages/arksync-node` TypeScript verification passes.

Raw evidence:

- `typecheck.txt`
- `build.txt`
- `git-diff-check.txt`

## Commands

- `bun run typecheck` in `packages/arksync-node`: PASS
- `bun run build` in `packages/arksync-node`: PASS
- `bun .agent/tasks/2026-04-24-arksync-node-init-lifecycle/verify-lifecycle.ts`: PASS
- `git diff --check -- packages/arksync-node/src/ark-client.ts .agent/tasks/2026-04-24-arksync-node-init-lifecycle/spec.md .agent/tasks/2026-04-24-arksync-node-init-lifecycle/verify-lifecycle.ts`: PASS

## Optional Smoke

An optional real sidecar smoke was attempted with both Bun and Node using the compiled `ark-core-rpc.exe`. Both attempts failed at the host process spawn boundary with `spawn EPERM` before any ARK request was sent. Running the binary directly from PowerShell exits cleanly. This is recorded as an environment limitation, not as an acceptance criterion failure.

Raw evidence:

- `standalone-smoke.txt`
- `standalone-smoke-node.txt`
