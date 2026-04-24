# Evidence: 2026-04-23 Arrancador backup IPC service boundary

## Result

PASS

## Acceptance Criteria

### AC1: Backup IPC helper logic is extracted

PASS. Added `apps/arrancador/electron/main/services/backup-ipc-support.ts` for backup root resolution, game backup state lookup, manifest loading, save-path override resolution, backup list/latest lookup, row mapping, reconciliation, and released-year formatting.

### AC2: Backup IPC module remains behavior-compatible

PASS. `backup-handlers.ts` keeps the same IPC channel registrations, payload handling, return shapes, and renderer progress event forwarding.

### AC3: Service boundary is readable

PASS. The extracted support module exposes typed functions and does not import Electron IPC APIs.

### AC4: Checks remain green

PASS.

- `bun run typecheck`: PASS
- `bun run test`: PASS, 19 files and 50 tests passed
- `bun run lint`: PASS

### AC5: Proof artifacts exist

PASS. Raw artifacts:

- `typecheck.txt`
- `test.txt`
- `lint.txt`
- `backup-handlers-line-count.txt`
- `backup-ipc-support-line-count.txt`

## Metrics

- `backup-handlers.ts` line count: 295
- `backup-ipc-support.ts` line count: 157
