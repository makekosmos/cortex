# Evidence

## Implementation

- Added a private `@raycast/api` `Keyboard` runtime export with
  `Keyboard.Shortcut.Common`.
- Covered common shortcut constants for `Copy`, `Open`, `Save`, and `Close`.
- Reused the existing action shortcut serialization/normalization path, so
  `Keyboard.Shortcut.Common.Copy` can be passed directly to `Action` `shortcut`.
- Documented the compatibility slice in `docs-site/concepts/extension-host.md`
  and the Raycast roadmap notes.

## Verification

- PASS: `bun test tests\unit\raycast-api.test.ts tests\unit\raycast-view-model.test.ts`
  - 14 tests passed, 114 expectations.
- PASS: `bun run shell:typecheck`
- PASS: `bun test tests\unit\raycast-api.test.ts tests\unit\raycast-command-runner.test.ts tests\unit\raycast-manifest.test.ts tests\unit\raycast-view-model.test.ts`
  - 31 tests passed, 167 expectations.
- PASS: `bun run docs:sync`
- PASS: `bun run ark:guard:writes`
- PASS: `bun run docs:check`
- PASS: `$env:CARGO_TARGET_DIR='D:\Personal\Hobby\Coding\kosmos\.tmp\cargo-ark-smoke'; bun run ark:smoke`
  - ARK write boundary guard, ARK core tests, kepler-backend tests, shell
    typecheck/build, and extension builds passed.
  - Known existing warning observed: `collect_state_events` is unused in
    `services\kepler-backend\src\dictation\host.rs`.

Visual verification was not needed for this slice because it only adds API
constants that flow through the already verified action shortcut renderer.
