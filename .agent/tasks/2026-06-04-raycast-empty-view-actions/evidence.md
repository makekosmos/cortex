# Evidence

## Implementation

- `normalizeRaycastNode(...)` now preserves `actions` for `List.EmptyView` and `Grid.EmptyView`.
- Added `listEmptyActions(...)` and `gridEmptyActions(...)` host model helpers.
- `RaycastListView.vue` and `RaycastGridView.vue` now use empty-state actions in the footer when the visible result set is empty and the view is not loading.
- Item actions still take precedence whenever a visible item is selected.
- Updated Raycast compatibility docs/roadmap to mention EmptyView footer actions.
- Added visual verification fixture for empty List and empty Grid action footers.

## Verification

- PASS: `bun test tests\unit\raycast-view-model.test.ts` (12 pass, 95 expectations).
- PASS: `bun run shell:typecheck`.
- PASS: `node .tmp\visual\2026-06-04-raycast-host\capture-empty-view-actions.mjs`.
  - Screenshot: `.tmp\visual\2026-06-04-raycast-host\raycast-empty-list-actions-1000x720.png`.
  - Screenshot: `.tmp\visual\2026-06-04-raycast-host\raycast-empty-grid-actions-1000x720.png`.
- PASS: `bun test tests\unit\raycast-api.test.ts tests\unit\raycast-command-runner.test.ts tests\unit\raycast-manifest.test.ts tests\unit\raycast-view-model.test.ts` (31 pass, 156 expectations).
- PASS: `bun run docs:sync`.
- PASS: `bun run docs:check`.
- PASS: `bun run ark:guard:writes`.
- PASS: `$env:CARGO_TARGET_DIR='D:\Personal\Hobby\Coding\kosmos\.tmp\cargo-ark-smoke'; bun run ark:smoke`.
  - Known warnings remain: `collect_state_events` is unused, Vite plugin timings, and `inlineDynamicImports` ignored with code splitting.
