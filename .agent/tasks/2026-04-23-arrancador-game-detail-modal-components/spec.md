# 2026-04-23 Arrancador game detail modal components

## Goal

Finish thinning `src-vue/pages/GameDetailPage.vue` by moving modal and progress overlay markup into focused components.

The route should remain responsible for orchestration and state ownership, while modal components handle presentational structure and typed user events.

## Acceptance Criteria

### AC1: Description and rating modals are componentized

Move description and rating modal markup into focused components under `src-vue/components/game-detail/`.

### AC2: Edit and RAWG search modals are componentized

Move edit-game and metadata-search modal markup into focused components under `src-vue/components/game-detail/`.

### AC3: Backup progress overlay is componentized

Move backup progress overlay markup into a focused component under `src-vue/components/game-detail/`.

### AC4: Route component stays the state owner

`GameDetailPage.vue` keeps modal state, form state, API actions, and composable wiring. New components receive typed props / `v-model` bindings and emit typed events.

### AC5: Behavior is unchanged

Existing labels, disabled states, data display, close behavior, save behavior, RAWG apply behavior, and progress text remain compatible with the current page.

### AC6: Checks remain green

Fresh verification must pass from `apps/arrancador`:

- `bun run typecheck`;
- `bun run test`;
- `bun run lint`.

### AC7: Proof artifacts exist

Write task artifacts under `.agent/tasks/2026-04-23-arrancador-game-detail-modal-components/`:

- `spec.md`;
- `evidence.md`;
- `evidence.json`;
- raw command outputs.

If verification is not `PASS`, write `problems.md`, apply the smallest defensible fix, and re-run verification.
