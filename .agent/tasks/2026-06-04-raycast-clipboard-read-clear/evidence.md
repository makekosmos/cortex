# Evidence

## Implementation

- Added `Clipboard.readText()`, `Clipboard.read()`, and `Clipboard.clear()` to
  the private `@raycast/api` runtime shim.
- Extended the memory runtime so copy/paste/read/clear share the same text
  state.
- Extended the trusted command runner clipboard adapter with optional
  `readText()` and `clear()` bridge methods.
- Kept existing `copy` and `paste` behavior intact; `clear()` falls back to
  writing an empty string when the host adapter has no explicit clear method.
- Added shim-level and trusted command-runner fixture coverage.
- Documented the compatibility slice in `docs-site/concepts/extension-host.md`
  and `docs-site/apps/kepler-roadmap.md`.

## Verification

- PASS: `bun test tests\unit\raycast-api.test.ts tests\unit\raycast-command-runner.test.ts`
  - 18 tests passed, 69 expectations.
- PASS: `bun test tests\unit\raycast-api.test.ts tests\unit\raycast-command-runner.test.ts tests\unit\raycast-manifest.test.ts tests\unit\raycast-view-model.test.ts`
  - 34 tests passed, 180 expectations.
- PASS: `bun run shell:typecheck`
- PASS: `bun run docs:sync`
- PASS: `bun run ark:guard:writes`
- PASS: `bun run docs:check`
- PASS after retry with longer timeout: `$env:CARGO_TARGET_DIR='D:\Personal\Hobby\Coding\kosmos\.tmp\cargo-ark-smoke'; bun run ark:smoke`
  - First smoke attempt timed out after 184 seconds without a test failure.
  - Retry passed ARK write boundary guard, ARK core tests, kepler-backend tests,
    shell typecheck/build, and extension builds.
  - Known existing warning observed: `collect_state_events` is unused in
    `services\kepler-backend\src\dictation\host.rs`.

Visual verification was not needed for this slice because it only changes the
runtime clipboard bridge and no renderer UI behavior changed.
