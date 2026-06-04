# Evidence

## Implementation

- Added `Action.Pop` and `Action.PopToRoot` factories to `packages/raycast-api/src/components.ts`.
- Both factories register runtime-backed callbacks through `navigationPop()` and `navigationPopToRoot()`.
- Added both action types to host `ACTION_TYPES`, so they are executable action nodes and shortcut candidates.
- Added Russian fallback labels `Назад` and `К началу` in `RaycastActionPanel.vue`.
- Updated List, Grid, Detail, and Form hosts to forward both action types through guarded session IPC.
- Updated Raycast compatibility docs/roadmap to mention navigation actions.
- Added visual verification fixture for executing `Action.Pop` and receiving a guarded snapshot update.

## Verification

- PASS: `bun test tests\unit\raycast-api.test.ts tests\unit\raycast-command-runner.test.ts tests\unit\raycast-view-model.test.ts` (27 pass, 147 expectations).
- PASS: `bun run shell:typecheck`.
- PASS: `node .tmp\visual\2026-06-04-raycast-host\capture-navigation-actions.mjs`.
  - Screenshot: `.tmp\visual\2026-06-04-raycast-host\raycast-navigation-actions-1000x720.png`.
- PASS: `bun test tests\unit\raycast-api.test.ts tests\unit\raycast-command-runner.test.ts tests\unit\raycast-manifest.test.ts tests\unit\raycast-view-model.test.ts` (31 pass, 158 expectations).
- PASS: `bun run docs:sync`.
- PASS: `bun run docs:check`.
- PASS: `bun run ark:guard:writes`.
- PASS: `$env:CARGO_TARGET_DIR='D:\Personal\Hobby\Coding\kosmos\.tmp\cargo-ark-smoke'; bun run ark:smoke`.
  - Known warnings remain: `collect_state_events` is unused and `inlineDynamicImports` is ignored with code splitting.
