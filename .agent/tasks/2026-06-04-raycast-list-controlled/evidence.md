# Evidence: raycast-list-controlled

## Changes

- `shell/electron/raycast/view-model.ts` registers `List.onSearchTextChange` and `List.onSelectionChange` as dedicated callback ids.
- `shell/src/raycast-host/model.ts` exposes helpers for controlled List props and callback nodes.
- `shell/src/raycast-host/RaycastListView.vue` initializes search/selection from snapshot props, honors `filtering: false`, and dispatches search/selection callbacks through guarded session IPC.
- `tests/unit/raycast-view-model.test.ts` covers controlled props and callback payloads.
- `docs-site/concepts/extension-host.md` and `docs-site/apps/kepler-roadmap.md` document the new supported List surface.

## Verification

- PASS: `bun test tests\unit\raycast-view-model.test.ts`
  - 7 pass, 0 fail, 54 expectations.
- PASS: `bun run shell:typecheck`

## Visual Evidence

- Initial attempt: `bun .tmp\visual\2026-06-04-raycast-host\capture-list-controlled.mjs`
  timed out and left only the capture process running. The process was stopped,
  the script was updated with `finally { await browser.close() }`, and the
  successful rerun used Node.
- PASS: `node .tmp\visual\2026-06-04-raycast-host\capture-list-controlled.mjs`
  - Verified `filtering: false` keeps 2 visible list items after a non-matching search.
  - Verified search callback status `Поиск обновлён` and selection callback status `Выбрано`.
- Screenshot: `.tmp/visual/2026-06-04-raycast-host/raycast-list-controlled-1000x720.png`

## Full Checks

- PASS: `bun test tests\unit\raycast-api.test.ts tests\unit\raycast-manifest.test.ts tests\unit\raycast-command-runner.test.ts tests\unit\raycast-view-model.test.ts tests\unit\extension-permissions.test.ts`
  - 31 pass, 0 fail, 116 expectations.
- PASS: `bun run docs:sync`
- PASS: `bun run docs:check`
- PASS: `bun run ark:guard:writes`
- PASS: `bun run ark:smoke`
  - First run failed because Windows locked `target\debug\ark-core-rpc.exe` behind the visual Vite/Electron dev tree.
  - The visual Vite/Electron tree was stopped, then smoke was rerun with `CARGO_TARGET_DIR=D:\Personal\Hobby\Coding\kosmos\.tmp\cargo-ark-smoke`.
  - Existing warning remained: unused test helper `collect_state_events` in `services\kepler-backend\src\dictation\host.rs`.
