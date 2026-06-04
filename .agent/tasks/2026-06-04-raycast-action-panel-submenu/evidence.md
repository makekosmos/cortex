# Evidence

## Implementation

- Added `ActionPanel.Submenu` to `packages/raycast-api/src/components.ts`.
- Updated `shell/src/raycast-host/model.ts` so action sections preserve submenu entries for rendering, while `actionNodes(...)` flattens submenu leaf actions for shortcut dispatch and execution lookup.
- Added `actionSubmenuActions(...)` for rendering submenu child actions without exposing nested container nodes as executable actions.
- Updated `shell/src/raycast-host/RaycastActionPanel.vue` with a compact submenu trigger/popover, child action execution, and popover z-index/overflow styling using Kosmos visual tokens.
- Covered API serialization and host model flattening in unit tests.
- Added visual verification fixture/script for opening a submenu and executing a child action.

## Verification

- PASS: `bun test tests\unit\raycast-api.test.ts tests\unit\raycast-view-model.test.ts` (14 pass, 102 expectations).
- PASS: `node .tmp\visual\2026-06-04-raycast-host\capture-action-panel-submenu.mjs`.
  - Screenshot: `.tmp\visual\2026-06-04-raycast-host\raycast-action-panel-submenu-1000x720.png`.
- PASS: `bun test tests\unit\raycast-api.test.ts tests\unit\raycast-command-runner.test.ts tests\unit\raycast-manifest.test.ts tests\unit\raycast-view-model.test.ts` (31 pass, 154 expectations).
- PASS: `bun run shell:typecheck`.
- PASS: `bun run docs:sync`.
- PASS: `bun run docs:check`.
- PASS: `bun run ark:guard:writes`.
- PASS: `$env:CARGO_TARGET_DIR='D:\Personal\Hobby\Coding\kosmos\.tmp\cargo-ark-smoke'; bun run ark:smoke`.
  - Known warning remains: `services\kepler-backend\src\dictation\host.rs`: `collect_state_events` is unused.
