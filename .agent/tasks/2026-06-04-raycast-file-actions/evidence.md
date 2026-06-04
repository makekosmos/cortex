# Evidence - Raycast file/system actions slice

Verified at: 2026-06-04T20:55:00+03:00

## Summary

PASS. The API shim, trusted command runner, guarded view IPC, and Vue
host action surfaces now support Raycast-style paste/show-in-folder/trash
actions plus basic system utilities.

## Acceptance Criteria

- PASS: `@raycast/api` exports `open`, `showInFinder`, and `trash`.
- PASS: `Action.Paste`, `Action.ShowInFinder`, and `Action.Trash` create
  serializable Raycast action nodes.
- PASS: trusted no-view commands can call system utilities through an injected
  Electron shell adapter.
- PASS: trusted view commands receive the same system adapter for registered
  callbacks.
- PASS: List, Grid, and Form hosts render and dispatch the new action types.
- PASS: Electron guarded session IPC handles paste, show-in-folder, and trash.
- PASS: docs updated in `docs-site/concepts/extension-host.md` and
  `docs-site/apps/kepler-roadmap.md`.

## Automated Checks

- PASS: `bun test tests/unit/raycast-api.test.ts tests/unit/raycast-command-runner.test.ts tests/unit/raycast-view-model.test.ts`
  - 16 tests passed, 0 failed, 72 expectations.
- PASS: `bun run shell:typecheck`
- PASS: `bun test tests/unit/raycast-api.test.ts tests/unit/raycast-manifest.test.ts tests/unit/raycast-command-runner.test.ts tests/unit/raycast-view-model.test.ts tests/unit/extension-permissions.test.ts`
  - 27 tests passed, 0 failed, 95 expectations.
- PASS: `bun run docs:sync`
- PASS: `bun run docs:check`
- PASS: `bun run ark:guard:writes`
- PASS: `bun run ark:smoke`
  - First smoke attempt timed out at 184s without test failure/output.
  - Second smoke attempt with a larger timeout passed the ARK smoke matrix.
  - Existing warning observed: `collect_state_events` is unused in
    `services/kepler-backend/src/dictation/host.rs`.

## Visual Verification

- PASS: local Vite renderer with mocked Raycast List snapshot and file action
  buttons.
- Screenshot:
  `.tmp/visual/2026-06-04-raycast-host/raycast-file-actions-1000x720.png`
- Observed state: `Вставить`, `Показать в папке`, and `Удалить` render in the
  footer without overlap; clicking `Удалить` shows status `Удалено`.

## Remaining Scope

Untrusted JS sandboxing, richer `Action.Open` application targeting, and trash
confirmation UX remain for later Raycast phases.
