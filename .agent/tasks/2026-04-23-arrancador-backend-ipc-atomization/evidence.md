# Evidence: 2026-04-23 Arrancador backend IPC atomization

## Result

PASS

## Acceptance Criteria

### AC1: IPC handler registration is feature-scoped

PASS. Added feature-scoped IPC modules:

- `apps/arrancador/electron/main/ipc/game-handlers.ts`
- `apps/arrancador/electron/main/ipc/app-handlers.ts`
- `apps/arrancador/electron/main/ipc/shell-scan-handlers.ts`
- `apps/arrancador/electron/main/ipc/types.ts`

### AC2: Backend runtime remains the composition root

PASS. `backend.ts` still owns runtime state, database path selection, service creation, startup Ark sync, and backup-specific helpers. IPC modules receive explicit dependencies through registration functions.

### AC3: Command behavior is unchanged

PASS. Existing IPC channel names and handler payload contracts were moved without changing the renderer-facing command surface.

### AC4: Architecture guard covers backend IPC modules

PASS. `architecture-boundaries.test.ts` now asserts the IPC module files exist and that `backend.ts` stays below the local IPC handler threshold.

### AC5: Existing checks remain green

PASS.

- `bun run typecheck`: PASS
- `bun run test`: PASS, 19 files and 50 tests passed

### AC6: Proof artifacts exist

PASS. Raw artifacts:

- `typecheck.txt`
- `test.txt`
- `backend-ipc-handle-count.txt`
- `backend-line-count.txt`
- `ipc-modules.txt`
- `diff-stat.txt`
- `tracked-diff.patch`

## Metrics

- `backend.ts` local `ipcMain.handle` count: 17
- `backend.ts` line count: 721
- New IPC module files under `electron/main/ipc`: 4
