# 2026-04-23 Arrancador game detail component tests

## Goal

Close the remaining test coverage gap for the newly extracted game-detail components.

The components were split out to improve readability and atomicity. This task adds focused tests for their public contracts so future edits cannot silently break props, emits, `v-model`, or key rendered states.

## Acceptance Criteria

### AC1: Backup section component has focused tests

Add tests for `GameDetailBackupSection.vue` covering rendered backup state and key emitted actions / `v-model` updates.

### AC2: Metadata section component has focused tests

Add tests for `GameDetailMetadataSection.vue` covering metadata display and action emits.

### AC3: Modal components have focused tests

Add tests for `GameDescriptionModal.vue`, `GameRatingModal.vue`, `GameEditDialog.vue`, and `GameMetadataSearchModal.vue` covering close/save/search/apply contracts.

### AC4: Backup progress toast has focused tests

Add tests for `BackupProgressToast.vue` covering stage label and progress count rendering.

### AC5: Checks remain green

Fresh verification must pass from `apps/arrancador`:

- `bun run typecheck`;
- `bun run test`;
- `bun run lint`.

### AC6: Proof artifacts exist

Write task artifacts under `.agent/tasks/2026-04-23-arrancador-game-detail-component-tests/`:

- `spec.md`;
- `evidence.md`;
- `evidence.json`;
- raw command outputs.

If verification is not `PASS`, write `problems.md`, apply the smallest defensible fix, and re-run verification.
