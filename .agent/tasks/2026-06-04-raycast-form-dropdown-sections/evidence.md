# Evidence

## Implementation

- `shell/src/raycast-host/model.ts` now exposes `optionSections` for `Form.Dropdown` while preserving flat `options` for existing callers.
- `shell/src/raycast-host/RaycastFormView.vue` renders dropdown groups as native `optgroup` blocks and loose options as plain options.
- `tests/unit/raycast-view-model.test.ts` covers mixed loose dropdown items plus a titled `Form.Dropdown.Section`.
- Docs updated in `docs-site/concepts/extension-host.md` and `docs-site/apps/kepler-roadmap.md`.

## Verification

- PASS: `bun test tests\unit\raycast-view-model.test.ts tests\unit\raycast-command-runner.test.ts` — 23 tests, 118 expectations.
- PASS: `bun run shell:typecheck`.
- PASS: visual verify via `.tmp\visual\2026-06-04-raycast-host\capture-form-dropdown-sections.mjs`; screenshot saved at `.tmp\visual\2026-06-04-raycast-host\raycast-form-dropdown-sections-1000x720.png`.
- PASS: `bun test tests\unit\raycast-api.test.ts tests\unit\raycast-command-runner.test.ts tests\unit\raycast-manifest.test.ts tests\unit\raycast-view-model.test.ts` — 27 tests, 135 expectations.
- PASS: `bun run docs:sync`.
- PASS: `bun run docs:check`.
- PASS: `bun run ark:guard:writes`.
- PASS: `$env:CARGO_TARGET_DIR='D:\Personal\Hobby\Coding\kosmos\.tmp\cargo-ark-smoke'; bun run ark:smoke`.
- Note: `ark:smoke` still reports the existing Rust warning `function collect_state_events is never used` in `services\kepler-backend\src\dictation\host.rs`; smoke passed.
