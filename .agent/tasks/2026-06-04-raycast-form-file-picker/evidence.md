# Evidence - Raycast Form.FilePicker host slice

Verified at: 2026-06-04T21:25:00+03:00

## Summary

PASS. The Raycast form host can render `Form.FilePicker`, invoke a
session-guarded native picker IPC, and store selected paths as form values.

## Acceptance Criteria

- PASS: `@raycast/api` exposes typed `Form.FilePicker` props.
- PASS: `formModel()` returns `string[]` defaults plus multiple/file/directory
  flags for FilePicker fields.
- PASS: preload/shared types expose `window.kepler.raycast.pickFiles`.
- PASS: Electron main validates the Raycast session window before opening the
  native file dialog.
- PASS: `RaycastFormView.vue` renders the picker, selected path rows, remove
  behavior, and form value updates.
- PASS: docs updated in `docs-site/concepts/extension-host.md` and
  `docs-site/apps/kepler-roadmap.md`.

## Automated Checks

- PASS: `bun test tests/unit/raycast-command-runner.test.ts tests/unit/raycast-view-model.test.ts`
  - 14 tests passed, 0 failed, 69 expectations.
- PASS: `bun run shell:typecheck`
- PASS: `bun test tests/unit/raycast-api.test.ts tests/unit/raycast-manifest.test.ts tests/unit/raycast-command-runner.test.ts tests/unit/raycast-view-model.test.ts tests/unit/extension-permissions.test.ts`
  - 27 tests passed, 0 failed, 99 expectations.
- PASS: `bun run docs:sync`
- PASS: `bun run docs:check`
- PASS: `bun run ark:guard:writes`
- PASS: `bun run ark:smoke`
  - ARK smoke matrix passed.
  - Existing warning observed: `collect_state_events` is unused in
    `services/kepler-backend/src/dictation/host.rs`.

## Visual Verification

- PASS: local Vite renderer with mocked Raycast Form snapshot and mocked
  `pickFiles` result.
- Screenshot:
  `.tmp/visual/2026-06-04-raycast-host/raycast-form-file-picker-1000x720.png`
- Observed state: `Выбрать` button, two selected path rows, and `Сохранить`
  action render without clipping or overlap at 1000x720.

## Remaining Scope

Drag-and-drop, file filters, persisted drafts, and untrusted sandbox permission
prompts remain for later Raycast phases.
