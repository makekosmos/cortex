# Evidence: raycast-grid-dropdown

## Changes

- `packages/raycast-api/src/components.ts` adds `Grid.searchBarAccessory`.
- `shell/electron/raycast/view-model.ts` normalizes `Grid.searchBarAccessory`.
- `shell/src/raycast-host/model.ts` extracts `Grid.Dropdown` groups/options/default value.
- `shell/src/raycast-host/RaycastGridView.vue` renders the dropdown and dispatches its callback through guarded session IPC.
- `tests/unit/raycast-view-model.test.ts` covers grouped Grid dropdown extraction.
- `docs-site/concepts/extension-host.md` and `docs-site/apps/kepler-roadmap.md` document the support.

## Verification

- PASS: `bun test tests\unit\raycast-view-model.test.ts tests\unit\raycast-api.test.ts`
  - 11 pass, 0 fail, 80 expectations.
- PASS: `bun run shell:typecheck`

## Visual Evidence

- PASS: `node .tmp\visual\2026-06-04-raycast-host\capture-grid-dropdown.mjs`
  - Verified Grid search input and dropdown render together.
  - Verified dropdown callback status `Выбрано`.
- Screenshot: `.tmp/visual/2026-06-04-raycast-host/raycast-grid-dropdown-1000x720.png`

## Full Checks

- PASS: `bun test tests\unit\raycast-api.test.ts tests\unit\raycast-manifest.test.ts tests\unit\raycast-command-runner.test.ts tests\unit\raycast-view-model.test.ts tests\unit\extension-permissions.test.ts`
  - 34 pass, 0 fail, 140 expectations.
- PASS: `bun run docs:sync`
- PASS: `bun run docs:check`
- PASS: `bun run ark:guard:writes`
- PASS: `bun run ark:smoke`
  - Run with `CARGO_TARGET_DIR=D:\Personal\Hobby\Coding\kosmos\.tmp\cargo-ark-smoke` to avoid Windows debug-target lock issues from local dev shells.
  - Existing warning remained: unused test helper `collect_state_events` in `services\kepler-backend\src\dictation\host.rs`.
