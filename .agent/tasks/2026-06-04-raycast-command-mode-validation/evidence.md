# Evidence

## Implementation

- Added strict command mode parsing in `shell/electron/raycast/manifest.ts`.
- Missing, `null`, or empty `commands[].mode` still defaults to `view`.
- Explicit values are accepted only for `view`, `no-view`, and `menu-bar`.
- Commands with an explicit unknown mode are skipped, and a package with no valid commands is rejected.
- Updated Raycast manifest unit tests and extension-host docs to describe the parser contract.

## Verification

- PASS: `bun test tests\unit\raycast-manifest.test.ts tests\unit\raycast-command-runner.test.ts`
  - 16 tests passed, 50 expectations.
- PASS: `bun run shell:typecheck`
- PASS: `bun test tests\unit\raycast-api.test.ts tests\unit\raycast-command-runner.test.ts tests\unit\raycast-manifest.test.ts tests\unit\raycast-view-model.test.ts`
  - 30 tests passed, 144 expectations.
- PASS: `bun run docs:sync`
- PASS: `bun run docs:check`
- PASS: `bun run ark:guard:writes`
- PASS: `$env:CARGO_TARGET_DIR='D:\Personal\Hobby\Coding\kosmos\.tmp\cargo-ark-smoke'; bun run ark:smoke`
  - Existing warning observed: `function collect_state_events is never used` in `services\kepler-backend\src\dictation\host.rs`.
