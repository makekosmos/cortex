# Evidence

## Implementation

- Added `List.Item.accessories` extraction to the Raycast host model.
- Supported string accessories plus object accessories with `text`, `title`, `tag`, or `date`.
- Rendered accessories as compact right-side chips in `RaycastListView.vue` using Kosmos tokens.
- Updated Raycast compatibility docs/roadmap.

## Verification

- PASS: `bun test tests\unit\raycast-view-model.test.ts`
  - 12 tests passed, 90 expectations.
- PASS: `node .tmp\visual\2026-06-04-raycast-host\capture-list-accessories.mjs`
  - Screenshot: `.tmp/visual/2026-06-04-raycast-host/raycast-list-accessories-1000x720.png`
- PASS: `bun test tests\unit\raycast-api.test.ts tests\unit\raycast-command-runner.test.ts tests\unit\raycast-manifest.test.ts tests\unit\raycast-view-model.test.ts`
  - 31 tests passed, 150 expectations.
- PASS: `bun run shell:typecheck`
- PASS: `bun run docs:sync`
- PASS: `bun run docs:check`
- PASS: `bun run ark:guard:writes`
- PASS: `$env:CARGO_TARGET_DIR='D:\Personal\Hobby\Coding\kosmos\.tmp\cargo-ark-smoke'; bun run ark:smoke`
  - Existing warning observed: `function collect_state_events is never used` in `services\kepler-backend\src\dictation\host.rs`.
