# Evidence

## Implementation

- Added public `List.Item.Detail` alias to `packages/raycast-api/src/components.ts`.
- Added `List.Item.Detail.Metadata` alias that reuses existing `Detail.Metadata*` snapshot node types.
- Updated `normalizeRaycastNode(...)` to preserve `metadata` passed through `List.Item.Detail`.
- Host model now reads markdown and metadata from `List.Item.Detail` through the existing detail renderer path.
- Updated Raycast compatibility docs/roadmap to mention `List.Item.Detail` aliases.

## Verification

- PASS: `bun test tests\unit\raycast-api.test.ts tests\unit\raycast-view-model.test.ts` (14 pass, 109 expectations).
- PASS: `bun run shell:typecheck`.
- PASS: `bun test tests\unit\raycast-api.test.ts tests\unit\raycast-command-runner.test.ts tests\unit\raycast-manifest.test.ts tests\unit\raycast-view-model.test.ts` (31 pass, 162 expectations).
- PASS: `bun run ark:guard:writes`.
- PASS: `bun run docs:sync`.
- PASS: `bun run docs:check`.
- PASS: `$env:CARGO_TARGET_DIR='D:\Personal\Hobby\Coding\kosmos\.tmp\cargo-ark-smoke'; bun run ark:smoke`.
  - Known warnings remain: `collect_state_events` is unused and `inlineDynamicImports` is ignored with code splitting.
