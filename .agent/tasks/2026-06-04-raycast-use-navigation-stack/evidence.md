# Evidence

## Implementation

- Added `navigation` hooks to the trusted Raycast command runtime adapter.
- Added a session-owned navigation stack in `shell/electron/raycast/view-host.ts`.
- Normalized pushed targets with the same callback registry used by the original root snapshot.
- Published guarded `kepler:raycast:snapshot-updated` events to the owning Raycast session window.
- Exposed `raycast.onSnapshotUpdated(...)` through preload and shared IPC types.
- Updated `RaycastHostView.vue` to subscribe/unsubscribe snapshot updates and swap the displayed root.
- Updated docs for the new `useNavigation().push/pop/popToRoot` behavior.

## Verification

- PASS: `bun test tests\unit\raycast-command-runner.test.ts`
  - 13 tests passed, 41 expectations.
- PASS: `node .tmp\visual\2026-06-04-raycast-host\capture-use-navigation-stack.mjs`
  - Screenshot: `.tmp/visual/2026-06-04-raycast-host/raycast-use-navigation-stack-1000x720.png`
- PASS: `bun test tests\unit\raycast-api.test.ts tests\unit\raycast-command-runner.test.ts tests\unit\raycast-manifest.test.ts tests\unit\raycast-view-model.test.ts`
  - 31 tests passed, 149 expectations.
- PASS: `bun run shell:typecheck`
- PASS: `bun run docs:sync`
- PASS: `bun run docs:check`
- PASS: `bun run ark:guard:writes`
- PASS: `$env:CARGO_TARGET_DIR='D:\Personal\Hobby\Coding\kosmos\.tmp\cargo-ark-smoke'; bun run ark:smoke`
  - Existing warning observed: `function collect_state_events is never used` in `services\kepler-backend\src\dictation\host.rs`.
