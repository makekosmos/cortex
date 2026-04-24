# Evidence: 2026-04-23 Arrancador backup IPC extraction

## Result

PASS

## Acceptance Criteria

### AC1: Backup IPC registration is feature-scoped

PASS. Backup/save IPC registration moved to `apps/arrancador/electron/main/ipc/backup-handlers.ts`.

### AC2: Shared settings persistence is not duplicated

PASS. Low-level settings persistence helpers now live in `apps/arrancador/electron/main/services/settings-store.ts` and are used by both `backend.ts` and `backup-handlers.ts`.

### AC3: Backend remains a compact composition root

PASS. `backend.ts` now registers feature IPC modules and owns runtime construction, database path selection, Ark sync, and app lifecycle wiring. Backup row types and save-discovery implementation no longer live in `backend.ts`.

### AC4: Command behavior is unchanged

PASS. Existing backup/save IPC channel names, payload shapes, progress events, and return values were moved without renderer API changes.

### AC5: Architecture guard covers the new boundary

PASS. `architecture-boundaries.test.ts` now requires `backup-handlers.ts` and asserts `backend.ts` has zero direct `ipcMain.handle` registrations.

### AC6: Existing checks remain green

PASS.

- `bun run typecheck`: PASS
- `bun run test`: PASS, 19 files and 50 tests passed
- `bun run lint`: PASS

### AC7: Proof artifacts exist

PASS. Raw artifacts:

- `typecheck.txt`
- `test.txt`
- `lint.txt`
- `backend-ipc-handle-count.txt`
- `backend-line-count.txt`
- `ipc-modules.txt`
- `diff-stat.txt`
- `tracked-diff.patch`

## Metrics

- `backend.ts` direct `ipcMain.handle` count: 0
- `backend.ts` line count: 308
- IPC module files under `electron/main/ipc`: 5
