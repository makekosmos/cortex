# Evidence: raycast-menu-bar-host

## Changes

- `shell/electron/extension-host.ts` maps Raycast `menu-bar` manifest commands to declared `raycast-menu-bar`.
- `shell/electron/raycast/command-runner.ts` can execute trusted `menu-bar` commands when `commandMode: "menu-bar"` is requested.
- `shell/electron/raycast/view-host.ts` and `shell/electron/main.ts` route `raycast-menu-bar` commands through the trusted Raycast host.
- `shell/src/raycast-host/model.ts` exposes `MenuBarExtra` title/loading/section/item helpers.
- `shell/src/raycast-host/RaycastMenuBarExtraView.vue` renders menu sections/items/submenus and dispatches item callbacks.
- `shell/src/views/RaycastHostView.vue` renders root `MenuBarExtra` snapshots.
- `tests/unit/raycast-command-runner.test.ts` and `tests/unit/raycast-view-model.test.ts` cover the new slice.
- `docs-site/concepts/command-bus.md`, `docs-site/concepts/extension-host.md`, and `docs-site/apps/kepler-roadmap.md` document the new support.

## Verification

- PASS: `bun test tests\unit\raycast-command-runner.test.ts tests\unit\raycast-view-model.test.ts tests\unit\raycast-manifest.test.ts`
  - 23 pass, 0 fail, 115 expectations.
- PASS: `bun run shell:typecheck`

## Visual Evidence

- PASS: `node .tmp\visual\2026-06-04-raycast-host\capture-menu-bar-extra.mjs`
  - Verified `MenuBarExtra.Section`, `MenuBarExtra.Item`, and `MenuBarExtra.Submenu` render in the Raycast host.
  - Verified item callback status `Готово`.
- Screenshot: `.tmp/visual/2026-06-04-raycast-host/raycast-menu-bar-extra-1000x720.png`

## Full Checks

- PASS: `bun test tests\unit\raycast-api.test.ts tests\unit\raycast-manifest.test.ts tests\unit\raycast-command-runner.test.ts tests\unit\raycast-view-model.test.ts tests\unit\extension-permissions.test.ts`
  - 34 pass, 0 fail, 137 expectations.
- PASS: `bun run docs:sync`
- PASS: `bun run docs:check`
- PASS: `bun run ark:guard:writes`
- PASS: `bun run ark:smoke`
  - Run with `CARGO_TARGET_DIR=D:\Personal\Hobby\Coding\kosmos\.tmp\cargo-ark-smoke` to avoid Windows debug-target lock issues from local dev shells.
  - Existing warning remained: unused test helper `collect_state_events` in `services\kepler-backend\src\dictation\host.rs`.
