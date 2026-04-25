# Changed Files Scope

This artifact separates the hardening scope from unrelated/pre-existing dirty
worktree state. It is a review/staging guide, not a request to revert anything.

## Proof Artifacts

- `.agent/tasks/2026-04-24-arrancador-10-hardening/spec.md`
- `.agent/tasks/2026-04-24-arrancador-10-hardening/evidence.md`
- `.agent/tasks/2026-04-24-arrancador-10-hardening/evidence.json`
- `.agent/tasks/2026-04-24-arrancador-10-hardening/problems.md`
- `.agent/tasks/2026-04-24-arrancador-10-hardening/changed-files.md`
- `.agent/tasks/2026-04-24-arrancador-10-hardening/independent-review.md`
- `.agent/tasks/2026-04-24-arrancador-10-hardening/independent-review.json`
- `.agent/tasks/2026-04-24-arrancador-10-hardening/raw/`

## Task-Owned Tooling And Verification

- `apps/arrancador/e2e/bridge-mock.ts`
- `apps/arrancador/package.json`
- `apps/arrancador/tsconfig.json`
- `apps/arrancador/tsconfig.electron.json`
- `apps/arrancador/tsconfig.node.json`
- `apps/arrancador/vitest.config.mjs`
- `apps/arrancador/scripts/run-e2e.ts`
- `bun.lock`

## Task-Owned Electron Main / Ark Usage / Backup / Sidecar Hardening

- `apps/arrancador/electron/main/backend.ts`
- `apps/arrancador/electron/main/db/sqlite.ts`
- `apps/arrancador/electron/main/ipc/backup-handlers.ts`
- `apps/arrancador/electron/main/ipc/backup-handlers.test.ts`
- `apps/arrancador/electron/main/ipc/shell-scan-handlers.ts`
- `apps/arrancador/electron/main/ipc/shell-scan-handlers.test.ts`
- `apps/arrancador/electron/main/shared-ipc.test.ts`
- `apps/arrancador/electron/main/services/ark-usage.ts`
- `apps/arrancador/electron/main/services/ark-usage/`
- `apps/arrancador/electron/main/services/ark-usage-backfill.ts`
- `apps/arrancador/electron/main/services/ark-usage-backfill/`
- `apps/arrancador/electron/main/services/backup/copy.ts`
- `apps/arrancador/electron/main/services/backup/copy.test.ts`
- `apps/arrancador/electron/main/services/backup/index.ts`
- `apps/arrancador/electron/main/services/backup/index.test.ts`
- `apps/arrancador/electron/main/services/backup/path-expansion.ts`
- `apps/arrancador/electron/main/services/backup/path-expansion.test.ts`
- `apps/arrancador/electron/main/services/backup/restore.ts`
- `apps/arrancador/electron/main/services/backup/restore.test.ts`
- `apps/arrancador/electron/main/services/backup/save-root-resolver.ts`
- `apps/arrancador/electron/main/services/backup/types.ts`
- `apps/arrancador/electron/main/services/backup-workflow.ts`
- `apps/arrancador/electron/main/services/backup-workflow.test.ts`
- `apps/arrancador/electron/main/services/backup-workflow/`
- `apps/arrancador/electron/main/services/games.ts`
- `apps/arrancador/electron/main/services/games.test.ts`
- `apps/arrancador/electron/main/services/games/loading.ts`
- `apps/arrancador/electron/main/services/games/service-types.ts`
- `apps/arrancador/electron/main/services/helpers/disk.ts`
- `apps/arrancador/electron/main/services/helpers/process.ts`
- `apps/arrancador/electron/main/services/settings.ts`
- `apps/arrancador/electron/main/services/usage-process-search.ts`
- `apps/arrancador/electron/main/sidecar/arrancador-sidecar.ts`
- `apps/arrancador/electron/main/sidecar/arrancador-sidecar-protocol.test.ts`
- `apps/arrancador/electron/main/sidecar/arrancador-sidecar.test.ts`
- `apps/arrancador/electron/main/windows.ts`
- `apps/arrancador/electron/main/windows.test.ts`
- `apps/arrancador/electron/shared/ipc.ts`
- `apps/arrancador/sidecar/src/backup.rs`
- `apps/arrancador/sidecar/src/main.rs`
- `apps/arrancador/sidecar/src/protocol.rs`
- `apps/arrancador/sidecar/src/scan.rs`

## Task-Owned Vue Contract Hardening

- `apps/arrancador/src-vue/components/game-detail/GameBackLink.vue`
- `apps/arrancador/src-vue/components/game-detail/GameDangerZone.vue`
- `apps/arrancador/src-vue/components/game-detail/GameDetailDialogs.vue`
- `apps/arrancador/src-vue/components/game-detail/GameDetailSections.vue`
- `apps/arrancador/src-vue/components/game-detail/GameMissingState.vue`
- `apps/arrancador/src-vue/components/game-detail/GameMetadataSearchModal.vue`
- `apps/arrancador/src-vue/components/library/`
- `apps/arrancador/src-vue/components/scan/ScanResultsList.vue`
- `apps/arrancador/src-vue/components/settings/ArkSettingsSection.vue`
- `apps/arrancador/src-vue/composables/useGameLaunchFlow.ts`
- `apps/arrancador/src-vue/pages/GameDetailPage.vue`
- `apps/arrancador/src-vue/pages/LibraryPage.vue`
- `apps/arrancador/src-vue/test/architecture-boundaries.test.ts`
- `apps/arrancador/src-vue/test/game-detail-components.test.ts`
- `apps/arrancador/src-vue/test/game-detail-composition.test.ts`
- `apps/arrancador/src-vue/test/library-components.test.ts`
- `apps/arrancador/src-vue/test/scan-results-list.test.ts`
- `apps/arrancador/src-vue/test/use-game-launch-flow.test.ts`

## Related Broader Or Pre-Existing Dirty State

These task directories are present in the worktree and should be reviewed or
staged separately if commit-level atomicity is required. All changed Arrancador
source/test files needed by the current hardening closure are listed above.

- `.agent/tasks/2026-04-24-arrancador-architecture-10/`
- `.agent/tasks/2026-04-24-arrancador-full-10/`
- `.agent/tasks/2026-04-24-arrancador-quality-10-continuation/`
- `.agent/tasks/2026-04-24-arrancador-quality-10-hardening/`
