# 2026-04-23 Arrancador backup IPC service boundary

## Goal

Tighten the Electron backup IPC boundary by moving SQL/save-path helper logic out of `electron/main/ipc/backup-handlers.ts`.

The backup IPC module should register channels and orchestrate calls, while persistence and save-path helper functions live in a service module.

## Acceptance Criteria

### AC1: Backup IPC helper logic is extracted

Move backup row mapping, backup list/latest lookup, backup reconciliation, game backup state lookup, manifest loading, save-path override resolution, backup root resolution, and released-year formatting out of `backup-handlers.ts`.

### AC2: Backup IPC module remains behavior-compatible

`backup-handlers.ts` must keep existing IPC channel names, payload shapes, return values, and renderer progress events.

### AC3: Service boundary is readable

The extracted service module should expose focused functions with typed inputs/outputs and should not import Electron IPC APIs.

### AC4: Checks remain green

Fresh verification must pass from `apps/arrancador`:

- `bun run typecheck`;
- `bun run test`;
- `bun run lint`.

### AC5: Proof artifacts exist

Write task artifacts under `.agent/tasks/2026-04-23-arrancador-backup-ipc-service-boundary/`:

- `spec.md`;
- `evidence.md`;
- `evidence.json`;
- raw command outputs.

If verification is not `PASS`, write `problems.md`, apply the smallest defensible fix, and re-run verification.
