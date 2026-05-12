# Evidence: Packaged smoke checks for Arrancador and Dashboard

## Verification status

PASS

## Acceptance criteria

- AC1 PASS: Arrancador has `bun run smoke:packaged`.
- AC2 PASS: The Arrancador smoke script sets temporary `APPDATA`, `LOCALAPPDATA`, and `ARK_DB_PATH` before launching Electron.
- AC3 PASS: The Arrancador smoke script launches `release/win-unpacked/arrancador.exe`, waits for the renderer, verifies the isolated write profile and exits without user interaction.
- AC4 PASS: Dashboard packaged/Electron smoke is documented and was run through `bun run --cwd apps/dashboard test:e2e:smoke`.
- AC5 PASS: Evidence records command results and confirms database/app-data isolation.

## Code changes verified

- `apps/arrancador/scripts/run-packaged-smoke.ts`
  - New isolated packaged Electron startup smoke.
- `apps/arrancador/package.json`
  - New `smoke:packaged` script.
  - Uses `electron-builder --dir -c.win.signAndEditExecutable=false` for smoke only, avoiding Windows code-sign cache symlink requirements.
- `docs/ARK-SMOKE-MATRIX.md`
  - Documents Arrancador packaged smoke.
- `apps/arrancador/AGENTS.md`
  - Lists the packaged smoke command.
- `apps/dashboard/AGENTS.md`
  - Notes Dashboard smoke DB isolation.

## Commands

- `bun run --cwd apps/arrancador typecheck`
- `bun run --cwd apps/dashboard typecheck`
- `bun run --cwd apps/dashboard test:e2e:smoke`
- `bun run --cwd apps/arrancador smoke:packaged`
- `git diff --check`

All verification commands passed. Dashboard and Arrancador Electron smoke commands required an outside-sandbox rerun because sandbox process restrictions blocked native child process spawning.
