# 2026-04-23 Arrancador backend IPC atomization

## Goal

Continue the Arrancador quality push by reducing the Electron main-process hotspot in `electron/main/backend.ts`.

The current backend file owns runtime construction and a large multi-feature IPC registration block. This makes architecture, reading, testing, and future feature work worse because unrelated command groups sit in one long function.

This task extracts IPC registration into feature-scoped modules while keeping runtime construction and existing service behavior intact.

## Acceptance Criteria

### AC1: IPC handler registration is feature-scoped

Move non-backup IPC handler registration out of `backend.ts` into focused modules under `electron/main/ipc/`.

Required groups:

- games / metadata / achievements;
- settings / stats / system / notifications / catalogue;
- window / shell / dialog / scan.

### AC2: Backend runtime remains the composition root

`backend.ts` must still own:

- runtime state;
- database paths;
- runtime service creation;
- startup Ark sync;
- backup-specific helpers until a later pass.

The new IPC modules must receive explicit dependencies instead of importing runtime globals from `backend.ts`.

### AC3: Command behavior is unchanged

Existing IPC channel names, payload shapes, and return values must remain compatible with `src/types/ipc.ts` and `electron/shared/ipc.ts`.

### AC4: Architecture guard covers backend IPC modules

Add or update tests to assert that active backend IPC registration is no longer one monolithic `registerIpcHandlers` block and that new IPC module files exist.

### AC5: Existing checks remain green

Fresh verification must pass from `apps/arrancador`:

- `bun run typecheck`;
- `bun run test`.

### AC6: Proof artifacts exist

Write task artifacts under `.agent/tasks/2026-04-23-arrancador-backend-ipc-atomization/`:

- `spec.md`;
- `evidence.md`;
- `evidence.json`;
- raw command outputs.

If verification is not `PASS`, write `problems.md`, apply the smallest defensible fix, and re-run verification.
