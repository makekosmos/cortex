# Evidence - Raycast Action.LaunchCommand slice

Verified at: 2026-06-04T19:45:00+03:00

## Summary

PASS. `Action.LaunchCommand` is now available in the Raycast API shim, included
in ActionPanel extraction, submitted by List/Grid/Form hosts through guarded
session IPC, and resolved by the main-process Raycast view host through the
session `launchCommand` adapter.

## Acceptance Criteria

- PASS: `Action.LaunchCommand` added to `packages/raycast-api/src/components.ts`.
- PASS: `actionNodes()` includes `Action.LaunchCommand`.
- PASS: List, Grid, and Form hosts send `Action.LaunchCommand` to
  `window.kepler.raycast.action`.
- PASS: `view-host.ts` maps action props to `LaunchCommandOptions` and invokes
  the session launcher.
- PASS: docs updated in `docs-site/concepts/extension-host.md` and
  `docs-site/apps/kepler-roadmap.md`.

## Automated Checks

- PASS: `bun test tests/unit/raycast-api.test.ts tests/unit/raycast-view-model.test.ts tests/unit/raycast-command-runner.test.ts`
  - 13 tests passed, 0 failed, 62 expectations.
- PASS: `bun run shell:typecheck`
- PASS: `bun test tests/unit/raycast-api.test.ts tests/unit/raycast-manifest.test.ts tests/unit/raycast-command-runner.test.ts tests/unit/raycast-view-model.test.ts tests/unit/extension-permissions.test.ts`
  - 24 tests passed, 0 failed, 85 expectations.
- PASS: `bun run docs:sync`
- PASS: `bun run docs:check`
- PASS: `bun run ark:guard:writes`
- PASS: `bun run ark:smoke`
  - ARK smoke matrix passed.

## Visual Verification

No separate screenshot was required for this slice: it reuses the existing
ActionPanel button rendering and only adds a new action type handled by the same
button component. The prior List/Grid screenshots cover ActionPanel placement;
execution behavior is covered by unit/type checks and the guarded session IPC
path.

## Remaining Scope

Cross-extension launch permission prompts and keyboard shortcut parity remain
for later Raycast phases.
