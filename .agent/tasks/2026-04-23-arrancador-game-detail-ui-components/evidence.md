# Evidence: 2026-04-23 Arrancador game detail UI components

## Result

PASS

## Acceptance Criteria

### AC1: Backup/save UI is componentized

PASS. Added `apps/arrancador/src-vue/components/game-detail/GameDetailBackupSection.vue` with typed props, explicit events, and `v-model` bindings for save-path draft/history visibility.

### AC2: Metadata UI is componentized

PASS. Added `apps/arrancador/src-vue/components/game-detail/GameDetailMetadataSection.vue` with typed props and explicit `searchRawg` / `edit` events.

### AC3: Route component remains the orchestration surface

PASS. `GameDetailPage.vue` keeps composable wiring, launch/pre-launch flow, modal ownership, and mutation functions. Extracted components are presentational/action emitters.

### AC4: Behavior is unchanged

PASS. Existing labels, disabled states, displayed values, and action paths were preserved through props and events.

### AC5: Checks remain green

PASS.

- `bun run typecheck`: PASS
- `bun run test`: PASS, 19 files and 50 tests passed
- `bun run lint`: PASS

### AC6: Proof artifacts exist

PASS. Raw artifacts:

- `typecheck.txt`
- `test.txt`
- `lint.txt`
- `game-detail-line-count.txt`
- `game-detail-components.txt`

## Metrics

- `GameDetailPage.vue` line count: 1049
- Added focused game-detail components: 2
