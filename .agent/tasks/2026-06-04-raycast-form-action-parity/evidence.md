# Evidence

## Implementation

- Expanded `RaycastFormView.vue` action execution to support `Action.CopyToClipboard`, `Action.OpenInBrowser`, and `Action.Open` through the existing guarded session IPC.
- Preserved existing generic `Action`, `Action.Paste`, `Action.ShowInFinder`, `Action.Trash`, `Action.LaunchCommand`, and `Action.SubmitForm` behavior.
- Added local `Action.Push` support for Form footers by opening the pushed `Detail` target in a right-side detail pane.
- Updated Raycast compatibility docs/roadmap.

## Verification

- PASS: `bun test tests\unit\raycast-view-model.test.ts`
  - 12 tests passed, 92 expectations.
- PASS: `node .tmp\visual\2026-06-04-raycast-host\capture-form-action-parity.mjs`
  - Screenshot: `.tmp/visual/2026-06-04-raycast-host/raycast-form-action-parity-1000x720.png`
- PASS: `bun test tests\unit\raycast-api.test.ts tests\unit\raycast-command-runner.test.ts tests\unit\raycast-manifest.test.ts tests\unit\raycast-view-model.test.ts`
  - 31 tests passed, 152 expectations.
- PASS: `bun run shell:typecheck`
- PASS: `bun run docs:sync`
- PASS: `bun run docs:check`
- PASS: `bun run ark:guard:writes`
- PASS: `$env:CARGO_TARGET_DIR='D:\Personal\Hobby\Coding\kosmos\.tmp\cargo-ark-smoke'; bun run ark:smoke`
  - Existing warning observed: `function collect_state_events is never used` in `services\kepler-backend\src\dictation\host.rs`.
