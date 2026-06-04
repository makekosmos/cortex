# Evidence

## Implementation

- `packages/raycast-api/src/components.ts` now exports `Form.Description`.
- `shell/src/raycast-host/model.ts` preserves `Form.Description` and `Form.Separator` in field order and uses `text` as description display content.
- `shell/src/raycast-host/RaycastFormView.vue` renders descriptions and separators as non-interactive form rows and excludes them from submit values.
- `tests/unit/raycast-view-model.test.ts` covers description/separator normalization alongside submit behavior.
- Docs updated in `docs-site/concepts/extension-host.md` and `docs-site/apps/kepler-roadmap.md`.

## Verification

- PASS: `bun test tests\unit\raycast-view-model.test.ts tests\unit\raycast-api.test.ts` — 14 tests, 92 expectations.
- PASS: `bun run shell:typecheck`.
- PASS: visual verify via `.tmp\visual\2026-06-04-raycast-host\capture-form-static-fields.mjs`; screenshot saved at `.tmp\visual\2026-06-04-raycast-host\raycast-form-static-fields-1000x720.png`.
- PASS: `bun test tests\unit\raycast-api.test.ts tests\unit\raycast-command-runner.test.ts tests\unit\raycast-manifest.test.ts tests\unit\raycast-view-model.test.ts` — 28 tests, 140 expectations.
- PASS: `bun run docs:sync`.
- PASS: `bun run docs:check`.
- PASS: `bun run ark:guard:writes`.
- PASS: `$env:CARGO_TARGET_DIR='D:\Personal\Hobby\Coding\kosmos\.tmp\cargo-ark-smoke'; bun run ark:smoke`.
- Note: `ark:smoke` still reports the existing Rust warning `function collect_state_events is never used` in `services\kepler-backend\src\dictation\host.rs`; smoke passed.
