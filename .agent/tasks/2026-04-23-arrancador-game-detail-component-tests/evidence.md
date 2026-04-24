# Evidence: 2026-04-23 Arrancador game detail component tests

## Result

PASS

## Acceptance Criteria

### AC1: Backup section component has focused tests

PASS. `game-detail-components.test.ts` covers `GameDetailBackupSection.vue` rendering, create/toggle/restore emits, and save-path/history `v-model` updates.

### AC2: Metadata section component has focused tests

PASS. The same test file covers `GameDetailMetadataSection.vue` display and action emits.

### AC3: Modal components have focused tests

PASS. The test file covers `GameDescriptionModal.vue`, `GameRatingModal.vue`, `GameEditDialog.vue`, and `GameMetadataSearchModal.vue` close/save/search/apply contracts.

### AC4: Backup progress toast has focused tests

PASS. The test file covers `BackupProgressToast.vue` stage label and progress count rendering.

### AC5: Checks remain green

PASS.

- `bun run typecheck`: PASS
- `bun run test`: PASS, 20 files and 56 tests passed
- `bun run lint`: PASS

### AC6: Proof artifacts exist

PASS. Raw artifacts:

- `typecheck.txt`
- `test.txt`
- `lint.txt`
- `component-test-line-count.txt`
- `test-file-count.txt`

## Metrics

- Added focused component test file: `apps/arrancador/src-vue/test/game-detail-components.test.ts`
- Component test file line count: 263
- Full suite: 20 test files, 56 tests
