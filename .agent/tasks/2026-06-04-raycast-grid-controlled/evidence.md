# Evidence: raycast-grid-controlled

## Changes

- `shell/src/raycast-host/model.ts` exposes helpers for controlled Grid props and callback nodes.
- `shell/src/raycast-host/RaycastGridView.vue` initializes search/selection from snapshot props, honors `filtering: false`, and dispatches search/selection callbacks through guarded session IPC.
- `tests/unit/raycast-view-model.test.ts` covers controlled Grid props and callback payloads.
- `docs-site/concepts/extension-host.md` and `docs-site/apps/kepler-roadmap.md` document the supported Grid controlled surface.

## Verification

- PASS: `bun test tests\unit\raycast-view-model.test.ts`
  - 8 pass, 0 fail, 62 expectations.
- PASS: `bun run shell:typecheck`

## Visual Evidence

- PASS: `node .tmp\visual\2026-06-04-raycast-host\capture-grid-controlled.mjs`
  - Verified `filtering: false` keeps 2 visible grid items after a non-matching search.
  - Verified search callback status `Поиск обновлён` and selection callback status `Выбрано`.
- Screenshot: `.tmp/visual/2026-06-04-raycast-host/raycast-grid-controlled-1000x720.png`

## Full Checks

- PASS: `bun test tests\unit\raycast-api.test.ts tests\unit\raycast-manifest.test.ts tests\unit\raycast-command-runner.test.ts tests\unit\raycast-view-model.test.ts tests\unit\extension-permissions.test.ts`
  - 32 pass, 0 fail, 124 expectations.
- PASS: `bun run docs:sync`
- PASS: `bun run docs:check`
- PASS: `bun run ark:guard:writes`
- PASS: `bun run ark:smoke`
  - Run with `CARGO_TARGET_DIR=D:\Personal\Hobby\Coding\kosmos\.tmp\cargo-ark-smoke` to avoid Windows debug-target lock issues from local dev shells.
  - Existing warning remained: unused test helper `collect_state_events` in `services\kepler-backend\src\dictation\host.rs`.
