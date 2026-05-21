# Arrancador Architecture 10 Pass Evidence

Task ID: `2026-04-24-arrancador-architecture-10`

Verification date: 2026-04-24

## Source Guidance Used

- Electron Security checklist: https://www.electronjs.org/docs/latest/tutorial/security
- Electron Context Isolation: https://www.electronjs.org/docs/latest/tutorial/context-isolation
- Vue Composables guide: https://vuejs.org/guide/reusability/composables
- Microsoft layered / clean architecture guidance: https://learn.microsoft.com/en-us/dotnet/architecture/modern-web-apps-azure/common-web-application-architectures
- OWASP input validation guidance: https://owasp.org/www-project-secure-coding-practices-quick-reference-guide/stable-en/02-checklist/05-checklist

## Implementation Summary

- Added `apps/arrancador/electron/main/services/backup-workflow.ts`.
  - Moves backup application workflow out of Electron IPC registration.
  - Uses explicit ports for settings, achievements, renderer events, DB, and low-level backup internals.
- Replaced `apps/arrancador/electron/main/ipc/backup-handlers.ts` with thin IPC registration that delegates to `createBackupWorkflow`.
- Added `apps/arrancador/src-vue/composables/useGameLaunchFlow.ts`.
  - Owns launch, running-process close, restore-before-launch, backup-before-launch, state flags, confirmations, and notifications.
- Updated `apps/arrancador/src-vue/pages/GameDetailPage.vue`.
  - Removes direct launch/backup orchestration calls from the route component.
- Added behavior tests:
  - `apps/arrancador/electron/main/services/backup-workflow.test.ts`
  - `apps/arrancador/src-vue/test/use-game-launch-flow.test.ts`
- Strengthened architecture guardrails in `apps/arrancador/src-vue/test/architecture-boundaries.test.ts`.
- Added extracted modules to `apps/arrancador/vitest.config.mjs` coverage include list.

## Acceptance Criteria

AC1: PASS

- `backup-handlers.ts` now imports only `ipcMain`, `createBackupWorkflow`, and `WithRuntime`.
- It no longer imports low-level backup functions, backup DB helpers, settings-store helpers, or DB helpers.
- IPC channel names and payload/result delegation remain intact.

AC2: PASS

- `backup-workflow.ts` exposes the backup workflow through explicit dependency ports.
- Tests cover save-path missing event emission, backup persistence/progress/settings reset, missing restore rejection, delete, backup-needed, restore-needed, and settings update/list flows.

AC3: PASS

- `GameDetailPage.vue` delegates launch/preflight behavior to `useGameLaunchFlow`.
- Architecture tests forbid direct route-level calls to `backupApi.checkRestoreNeeded`, `backupApi.restore`, `backupApi.shouldBackupBeforeLaunch`, `backupApi.checkBackupNeeded`, `backupApi.create`, `gamesApi.launch`, `gamesApi.getRunningInstances`, and `gamesApi.killProcesses`.

AC4: PASS

- `use-game-launch-flow.test.ts` covers:
  - running game close path,
  - restore-before-launch path,
  - backup-before-launch path,
  - launch failure notification path.

AC5: PASS

- `architecture-boundaries.test.ts` now guards backup IPC against low-level backup implementation imports.
- It also guards Game Detail route against launch orchestration regression.

AC6: PASS

- `bun run typecheck`: PASS.
- `bun run test`: PASS, 37 files / 119 tests.
- `bun run test:coverage`: PASS, statements 79.5%, branches 73.09%, functions 74.52%, lines 80.88%.
- `bun run test:e2e`: PASS, 4 tests.
- `cargo test --manifest-path sidecar\Cargo.toml`: PASS, 6 tests.

AC7: PASS

- Raw artifacts:
  - `final-typecheck.txt`
  - `final-test.txt`
  - `final-coverage.txt`
  - `final-e2e.txt`
  - `final-cargo-sidecar-test.txt`
- Structured artifact:
  - `evidence.json`

## Verification Commands

All commands were run against the current working copy after implementation.

```text
bun run typecheck
bun run test
bun run test:coverage
bun run test:e2e
cargo test --manifest-path sidecar\Cargo.toml
```

## Result

PASS. All acceptance criteria are satisfied against the current codebase and fresh command results.
