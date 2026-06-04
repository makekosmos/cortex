# Evidence

## Implementation

- Added `icon: string | null` to `RaycastListItemModel`.
- Preserved `List.Item.icon` through existing `imageProp(...)` handling.
- Rendered item icons in `RaycastListView.vue` as fixed 24px image slots so text and accessories keep their alignment.
- Updated Raycast compatibility docs/roadmap to mention `List.Item.icon`.
- Added visual verification fixture with one icon row and one non-icon row.

## Verification

- PASS: `bun test tests\unit\raycast-view-model.test.ts` (12 pass, 98 expectations).
- PASS: `bun run shell:typecheck`.
- PASS: `node .tmp\visual\2026-06-04-raycast-host\capture-list-item-icon.mjs`.
  - Screenshot: `.tmp\visual\2026-06-04-raycast-host\raycast-list-item-icon-1000x720.png`.
- PASS: `bun test tests\unit\raycast-api.test.ts tests\unit\raycast-command-runner.test.ts tests\unit\raycast-manifest.test.ts tests\unit\raycast-view-model.test.ts` (31 pass, 163 expectations).
- PASS: `bun run docs:sync`.
- PASS: `bun run docs:check`.
- PASS: `bun run ark:guard:writes`.
- PASS: `$env:CARGO_TARGET_DIR='D:\Personal\Hobby\Coding\kosmos\.tmp\cargo-ark-smoke'; bun run ark:smoke`.
  - Known warnings remain: `collect_state_events` is unused and `inlineDynamicImports` is ignored with code splitting.
