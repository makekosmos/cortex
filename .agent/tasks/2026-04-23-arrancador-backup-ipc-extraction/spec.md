# 2026-04-23 Arrancador backup IPC extraction

## Goal

Finish the Electron main-process atomization by moving backup/save IPC handling out of `electron/main/backend.ts`.

After the previous pass, `backend.ts` still owns backup IPC handlers and backup-specific helper functions. That keeps the composition root too large and mixes runtime construction with save-discovery and backup persistence details.

## Acceptance Criteria

### AC1: Backup IPC registration is feature-scoped

Move backup/save IPC handler registration into `electron/main/ipc/backup-handlers.ts`.

The moved handlers include:

- Ludusavi path/settings handlers;
- backup directory/settings handlers;
- save path discovery handlers;
- backup create/list/restore/delete handlers;
- backup/restore need checks.

### AC2: Shared settings persistence is not duplicated

Extract reusable low-level settings helpers into a shared main-process module and use it from both runtime service construction and backup IPC.

### AC3: Backend remains a compact composition root

`backend.ts` should register feature IPC modules and keep runtime construction, database path selection, Ark sync, and app lifecycle wiring. It should not contain backup-specific row types or save-discovery implementation.

### AC4: Command behavior is unchanged

Existing IPC channel names, payload shapes, progress events, and return values remain compatible with current renderer API contracts.

### AC5: Architecture guard covers the new boundary

Update architecture tests so the backup IPC module is required and `backend.ts` has only minimal direct IPC registration.

### AC6: Existing checks remain green

Fresh verification must pass from `apps/arrancador`:

- `bun run typecheck`;
- `bun run test`.

### AC7: Proof artifacts exist

Write task artifacts under `.agent/tasks/2026-04-23-arrancador-backup-ipc-extraction/`:

- `spec.md`;
- `evidence.md`;
- `evidence.json`;
- raw command outputs.

If verification is not `PASS`, write `problems.md`, apply the smallest defensible fix, and re-run verification.
