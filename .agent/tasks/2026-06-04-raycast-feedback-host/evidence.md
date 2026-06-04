# Evidence - Raycast feedback host slice

Verified at: 2026-06-04T22:20:00+03:00

## Summary

PASS. Trusted Raycast command callbacks now bridge feedback events to
the renderer, generic `Action` callbacks dispatch through guarded IPC, and
`confirmAlert` delegates to host native confirmation adapters.

## Acceptance Criteria

- PASS: `showToast` and `showHUD` create `RaycastFeedbackEvent` payloads in the
  runtime adapter.
- PASS: preload/shared types expose `window.kepler.raycast.onFeedback`.
- PASS: `RaycastHostView.vue` renders feedback with local Kosmos-token overlay.
- PASS: `confirmAlert` delegates to host adapters in command-runner, view-host,
  and main no-view launches.
- PASS: List, Grid, and Form hosts dispatch generic `Action` callbacks through
  guarded session IPC.
- PASS: docs updated in `docs-site/concepts/extension-host.md` and
  `docs-site/apps/kepler-roadmap.md`.

## Automated Checks

- PASS: `bun test tests/unit/raycast-api.test.ts tests/unit/raycast-command-runner.test.ts`
  - 13 tests passed, 0 failed, 39 expectations.
- PASS: `bun test tests/unit/raycast-command-runner.test.ts tests/unit/raycast-view-model.test.ts`
  - 17 tests passed, 0 failed, 77 expectations.
- PASS: `bun run shell:typecheck`
- PASS: `bun test tests/unit/raycast-api.test.ts tests/unit/raycast-manifest.test.ts tests/unit/raycast-command-runner.test.ts tests/unit/raycast-view-model.test.ts tests/unit/extension-permissions.test.ts`
  - 30 tests passed, 0 failed, 108 expectations.
- PASS: `bun run docs:sync`
- PASS: `bun run docs:check`
- PASS: `bun run ark:guard:writes`
- PASS: `bun run ark:smoke`
  - ARK smoke matrix passed.
  - Existing warning observed: `collect_state_events` is unused in
    `services/kepler-backend/src/dictation/host.rs`.

## Visual Verification

- PASS: local Vite renderer with mocked generic `Action` callback and mocked
  feedback event.
- Screenshot:
  `.tmp/visual/2026-06-04-raycast-host/raycast-feedback-toast-1000x720.png`
- Observed state: feedback card `Сохранено` renders in the top-right overlay
  with readable contrast and Kosmos tokens after clicking `Показать toast`.

## Remaining Scope

Toast update APIs, primary toast actions, custom renderer confirm UI,
permission prompts, and persisted diagnostics/log UI remain for later Raycast
phases.
