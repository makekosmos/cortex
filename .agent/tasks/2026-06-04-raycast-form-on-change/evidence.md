# Evidence

## Implementation

- `shell/src/raycast-host/model.ts` now exposes per-field callback ids captured from normalized `onChange` props.
- `shell/src/raycast-host/RaycastFormView.vue` invokes guarded session IPC for text-like, dropdown, checkbox, and file picker value changes when a field callback id exists.
- `tests/unit/raycast-view-model.test.ts` covers form field callback id extraction and callback execution.
- Docs updated in `docs-site/concepts/extension-host.md` and `docs-site/apps/kepler-roadmap.md`.

## Verification

- PASS: `bun test tests\unit\raycast-view-model.test.ts tests\unit\raycast-command-runner.test.ts` — 24 tests, 125 expectations.
- PASS: `bun run shell:typecheck`.
- PASS: visual verify via `.tmp\visual\2026-06-04-raycast-host\capture-form-on-change.mjs`; screenshot saved at `.tmp\visual\2026-06-04-raycast-host\raycast-form-on-change-1000x720.png`.
- PASS: `bun test tests\unit\raycast-api.test.ts tests\unit\raycast-command-runner.test.ts tests\unit\raycast-manifest.test.ts tests\unit\raycast-view-model.test.ts` — 28 tests, 142 expectations.
- PASS: `bun run docs:sync`.
- PASS: `bun run docs:check`.
- PASS: `bun run ark:guard:writes`.
- PASS: `$env:CARGO_TARGET_DIR='D:\Personal\Hobby\Coding\kosmos\.tmp\cargo-ark-smoke'; bun run ark:smoke`.
- Note: `ark:smoke` still reports the existing Rust warning `function collect_state_events is never used` in `services\kepler-backend\src\dictation\host.rs`; smoke passed.
