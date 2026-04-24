# 2026-04-23 Arrancador GameDetail atomization

## Goal

Continue the Arrancador quality push by reducing the next largest Vue route hotspot: `src-vue/pages/GameDetailPage.vue`.

The route currently owns multiple independent flows at once:

- backup list/progress/create/restore;
- launch preflight with backup/restore checks;
- save-path discovery and editing;
- user note/rating/play-status state;
- RAWG metadata search/apply;
- process binding add/remove.

This task extracts high-impact stateful side-effect logic into focused composables while preserving current UI and behavior.

## Component / Composable Map

- `GameDetailPage.vue`: route composition surface, hero/layout/template, destructive navigation actions.
- `useGameBackups.ts`: backup list, progress subscription, manual backup and restore actions.
- `useGameSavePath.ts`: save-path draft, locate/open/choose/insert/save actions.
- `useGameMetadataSearch.ts`: RAWG search/apply and edit-dialog form state.
- Existing `useGameStatus.ts`: installed/running polling remains the process status source.

## Acceptance Criteria

### AC1: Backup flow is extracted

`GameDetailPage.vue` must no longer directly own backup list loading, backup progress event subscription, manual backup creation, or restore implementation. Those must live in `useGameBackups.ts`.

### AC2: Save-path flow is extracted

`GameDetailPage.vue` must no longer directly own save-path lookup, picker, token insertion, open, or save implementation. Those must live in `useGameSavePath.ts`.

### AC3: Metadata/edit flow is extracted

`GameDetailPage.vue` must no longer directly own RAWG query/results/search/apply or edit-form save implementation. Those must live in `useGameMetadataSearch.ts`.

### AC4: Behavior is covered by focused tests

Add or update tests proving:

- backup composable sorts loaded backups and tracks progress only for the active game;
- save-path composable inserts `{PATHTOGAME}` safely and saves empty paths as `null`;
- metadata composable applies metadata and resets search state.

### AC5: Existing checks remain green

Fresh verification must pass from `apps/arrancador`:

- `bun run typecheck`;
- `bun run test`.

### AC6: Proof artifacts exist

Write task artifacts under `.agent/tasks/2026-04-23-arrancador-game-detail-atomization/`:

- `spec.md`;
- `evidence.md`;
- `evidence.json`;
- raw command outputs.

If verification is not `PASS`, write `problems.md`, apply the smallest defensible fix, and re-run verification.
