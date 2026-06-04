# Evidence

## Implementation

- Added a trusted command-runner bridge file for `@raycast/api/jsx-runtime`.
- The bridge exports `jsx`, `jsxs`, and `Fragment`, building Raycast elements
  via the existing `createRaycastElement` runtime.
- Extended command import rewriting to handle both double-quoted and
  single-quoted `@raycast/api/jsx-runtime` imports before the regular
  `@raycast/api` bridge replacement.
- Added a compiled TSX-style view command fixture that imports components from
  `@raycast/api` and `jsx/jsxs` from `@raycast/api/jsx-runtime`.
- Documented the compatibility slice in `docs-site/concepts/extension-host.md`
  and `docs-site/apps/kepler-roadmap.md`.

## Verification

- PASS: `bun test tests\unit\raycast-command-runner.test.ts`
  - 15 tests passed, 49 expectations.
- PASS: `bun test tests\unit\raycast-api.test.ts tests\unit\raycast-command-runner.test.ts tests\unit\raycast-manifest.test.ts tests\unit\raycast-view-model.test.ts`
  - 33 tests passed, 175 expectations.
- PASS: `bun run shell:typecheck`
- PASS: `bun run docs:sync`
- PASS: `bun run ark:guard:writes`
- PASS: `bun run docs:check`
- PASS: `$env:CARGO_TARGET_DIR='D:\Personal\Hobby\Coding\kosmos\.tmp\cargo-ark-smoke'; bun run ark:smoke`
  - ARK write boundary guard, ARK core tests, kepler-backend tests, shell
    typecheck/build, and extension builds passed.
  - Known existing warning observed: `collect_state_events` is unused in
    `services\kepler-backend\src\dictation\host.rs`.

Visual verification was not needed for this slice because it only changes the
trusted command import bridge; the renderer output is covered by snapshot
normalization tests.
