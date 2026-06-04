# Evidence

## Implementation

- Added `Form.TagPicker` and `Form.TagPicker.Item` to the local `@raycast/api` shim.
- Extended the form host model to expose TagPicker options and normalize `defaultValue` / `value` as `string[]`.
- Rendered TagPicker as Kosmos-token tag chips in `RaycastFormView.vue`.
- Tag selection updates form values, invokes field-level `onChange` with `string[]`, and is included in `Action.SubmitForm` payloads.
- Updated extension-host docs and roadmap status text.

## Verification

- PASS: `bun test tests\unit\raycast-view-model.test.ts`
  - 12 tests passed, 89 expectations.
- PASS: `node .tmp\visual\2026-06-04-raycast-host\capture-form-tag-picker.mjs`
  - Screenshot: `.tmp/visual/2026-06-04-raycast-host/raycast-form-tag-picker-1000x720.png`
- PASS: `bun test tests\unit\raycast-api.test.ts tests\unit\raycast-command-runner.test.ts tests\unit\raycast-manifest.test.ts tests\unit\raycast-view-model.test.ts`
  - 30 tests passed, 147 expectations.
- PASS: `bun run shell:typecheck`
- PASS: `bun run docs:sync`
- PASS: `bun run docs:check`
- PASS: `bun run ark:guard:writes`
- PASS: `$env:CARGO_TARGET_DIR='D:\Personal\Hobby\Coding\kosmos\.tmp\cargo-ark-smoke'; bun run ark:smoke`
  - Existing warning observed: `function collect_state_events is never used` in `services\kepler-backend\src\dictation\host.rs`.
