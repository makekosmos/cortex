# 2026-04-23 Arrancador game detail UI components

## Goal

Continue the Arrancador quality push by thinning `src-vue/pages/GameDetailPage.vue`.

The page already has several feature composables, but its template still owns multiple substantial UI sections. This keeps the route component harder to read and makes future visual changes risky.

## Acceptance Criteria

### AC1: Backup/save UI is componentized

Move the backup/save-path section markup from `GameDetailPage.vue` into `src-vue/components/game-detail/GameDetailBackupSection.vue`.

The component must receive state through typed props and emit explicit events for actions.

### AC2: Metadata UI is componentized

Move the compact RAWG/manual metadata section markup from `GameDetailPage.vue` into `src-vue/components/game-detail/GameDetailMetadataSection.vue`.

The component must receive state through typed props and emit explicit events for actions.

### AC3: Route component remains the orchestration surface

`GameDetailPage.vue` should keep data loading, launch/pre-launch flow, composable wiring, and modal ownership. It should not own the presentational markup for the extracted sections.

### AC4: Behavior is unchanged

Existing UI labels, button disabled states, emitted actions, and data display remain compatible with the current page behavior.

### AC5: Checks remain green

Fresh verification must pass from `apps/arrancador`:

- `bun run typecheck`;
- `bun run test`;
- `bun run lint`.

### AC6: Proof artifacts exist

Write task artifacts under `.agent/tasks/2026-04-23-arrancador-game-detail-ui-components/`:

- `spec.md`;
- `evidence.md`;
- `evidence.json`;
- raw command outputs.

If verification is not `PASS`, write `problems.md`, apply the smallest defensible fix, and re-run verification.
