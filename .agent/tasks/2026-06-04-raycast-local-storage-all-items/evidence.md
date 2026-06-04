# Evidence

## Implementation

- Added `LocalStorage.allItems()` to the private `@raycast/api` runtime shim.
- Extended the in-memory runtime adapter with a shallow all-items snapshot.
- Extended the trusted Raycast command runner JSON-backed local storage adapter
  with `all()` and bridged it through `RaycastRuntimeAdapter`.
- Added shim-level and trusted command-runner fixture coverage.
- Documented the compatibility slice in `docs-site/concepts/extension-host.md`
  and `docs-site/apps/kepler-roadmap.md`.

## Verification

- PASS: `bun test tests\unit\raycast-api.test.ts tests\unit\raycast-command-runner.test.ts`
  - 16 tests passed, 58 expectations.
- PASS: `bun test tests\unit\raycast-api.test.ts tests\unit\raycast-command-runner.test.ts tests\unit\raycast-manifest.test.ts tests\unit\raycast-view-model.test.ts`
  - 32 tests passed, 169 expectations.
- PASS: `bun run shell:typecheck`
- PASS: `bun run docs:sync`
- PASS: `bun run ark:guard:writes`
- PASS: `bun run docs:check`
- PASS: `$env:CARGO_TARGET_DIR='D:\Personal\Hobby\Coding\kosmos\.tmp\cargo-ark-smoke'; bun run ark:smoke`
  - ARK write boundary guard, ARK core tests, kepler-backend tests, shell
    typecheck/build, and extension builds passed.
  - Known existing warning observed: `collect_state_events` is unused in
    `services\kepler-backend\src\dictation\host.rs`.

Visual verification was not needed for this slice because it only adds a
runtime storage API and no renderer UI behavior changed.
