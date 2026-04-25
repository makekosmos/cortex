# Arrancador Architecture 10 Pass Spec

Task ID: `2026-04-24-arrancador-architecture-10`

## Context

Arrancador is an Electron + Vue desktop launcher. Current quality is high but not yet excellent because several boundary modules still mix responsibilities:

- `electron/main/ipc/backup-handlers.ts` owns IPC registration, database reads/writes, backup orchestration, settings persistence, progress event mapping, and achievement side effects.
- `src-vue/pages/GameDetailPage.vue` still owns launch orchestration, backup/restore preflight, process killing, user confirmation, store refreshes, and user-facing notifications.
- Architecture tests exist, but they do not yet enforce the next-level boundaries for backup IPC and game detail route orchestration.

Reference principles used for this task:

- Electron security guidance: narrow preload API, context isolation, no raw Electron exposure, validate IPC surface and senders.
- Vue Composition API guidance: composables encapsulate reusable stateful logic; route-level views should compose focused state and components.
- Microsoft layered/clean architecture guidance: business/application logic should be separated from presentation and infrastructure details; dependency direction should favor core/application code through explicit ports.
- OWASP input validation guidance: validate untrusted/client-provided data on the trusted side, use centralized allowlist-style routines, and reject invalid payloads early.

## Scope

This pass is intentionally scoped to high-leverage architecture improvements with evidence:

1. Move backup application workflow out of IPC registration into a focused main-process application service.
2. Move Game Detail launch/preflight workflow out of the route component into a focused Vue composable with explicit dependencies.
3. Add tests that lock those boundaries and cover the extracted behavior.
4. Run fresh verification and record evidence.

Out of scope:

- Rewriting the whole app.
- Changing product behavior or UI design.
- Replacing SQLite, Electron, Vue, Bun, or the Rust sidecar.
- Reverting unrelated existing user changes in the dirty worktree.

## Acceptance Criteria

AC1: Backend backup IPC boundary is thin.
- `electron/main/ipc/backup-handlers.ts` must delegate backup workflow to a focused service/module.
- The handler file must no longer directly import low-level backup implementation functions such as `createBackup`, `restoreBackup`, `deleteBackup`, `discoverBackupInfo`, `findGameSaves`, or backup DB helper functions.
- Backup commands retain the same IPC channel names and payload/result shapes.

AC2: Backup workflow is testable away from Electron IPC.
- A new or existing non-IPC module must expose backup workflow functions through dependency injection/ports.
- Unit tests must cover at least:
  - save path lookup emits `game:save-path-missing` when no path is found.
  - create backup persists backup rows, updates game backup state, emits progress, and resets one-shot compression skip.
  - restore backup rejects missing backup IDs.

AC3: Game Detail route is a composition surface for launch orchestration.
- `src-vue/pages/GameDetailPage.vue` must delegate launch/preflight/kill-process behavior to a composable.
- The route component must no longer call `backupApi.checkRestoreNeeded`, `backupApi.shouldBackupBeforeLaunch`, `backupApi.checkBackupNeeded`, `backupApi.create`, `backupApi.restore`, `gamesApi.launch`, `gamesApi.killProcesses`, or `gamesApi.getRunningInstances` directly.
- Existing child component props/emits remain compatible.

AC4: Launch workflow is tested as stateful Vue logic.
- A composable test must cover:
  - running game path: confirm, kill matching processes, update running count, no launch.
  - restore-before-launch path.
  - backup-before-launch path.
  - launch failure notification path.

AC5: Architecture guardrails are strengthened.
- `architecture-boundaries.test.ts` or equivalent tests must fail if backup IPC grows low-level backup imports again.
- A route-level budget/check must cover Game Detail launch orchestration imports/calls.

AC6: Verification is fresh and passing.
- `bun run typecheck`
- `bun run test`
- `bun run test:coverage`
- `bun run test:e2e`
- `cargo test --manifest-path sidecar\Cargo.toml`

AC7: Proof artifacts exist.
- `.agent/tasks/2026-04-24-arrancador-architecture-10/evidence.md`
- `.agent/tasks/2026-04-24-arrancador-architecture-10/evidence.json`
- Raw command output files for every verification command.

## Component Map

Vue:

- `GameDetailPage.vue`: route-level composition surface; wires route, store, child components, and composables.
- `useGameLaunchFlow.ts`: owns launch/preflight state and actions; inputs are current game, running state setter, store refresh/update callbacks, API ports, confirm, notify, and log.
- Existing `GameDetailHeroSection`, `GameDetailSections`, `GameDetailDialogs`: presentational/feature sections with typed props and emits; no new responsibilities.

Electron main:

- `backup-handlers.ts`: IPC registration only; payload extraction and calls to application service.
- `backup-workflow.ts`: application service for backup use cases; depends on DB, settings, artifact engine, manifest/save path helpers, emit callback, and achievements through explicit dependencies.
- Existing backup engine modules under `services/backup/`: filesystem/archive/manifest implementation details.

## Stop Condition

Completion can only be claimed when AC1-AC7 are all PASS in `evidence.md` and `evidence.json`.
