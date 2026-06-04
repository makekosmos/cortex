# Evidence

## Implementation

- `shell/src/raycast-host/model.ts` now exposes `actionShortcut()` and `matchesActionShortcut()` for Raycast action shortcut props.
- `shell/src/raycast-host/RaycastActionPanel.vue` renders shortcut labels and dispatches matching `keydown` events through the existing action execution path.
- Letter shortcuts match by `KeyboardEvent.code`, preserving behavior under non-Latin keyboard layouts.
- `tests/unit/raycast-view-model.test.ts` covers shortcut labels and RU-layout-safe matching.
- Docs updated in `docs-site/concepts/extension-host.md` and `docs-site/apps/kepler-roadmap.md`.

## Verification

- PASS: `bun test tests\unit\raycast-view-model.test.ts tests\unit\raycast-api.test.ts` — 14 tests, 91 expectations.
- PASS: `bun run shell:typecheck`.
- PASS: visual verify via `.tmp\visual\2026-06-04-raycast-host\capture-action-shortcuts.mjs`; screenshot saved at `.tmp\visual\2026-06-04-raycast-host\raycast-action-shortcuts-1000x720.png`.
- PASS: `bun test tests\unit\raycast-api.test.ts tests\unit\raycast-command-runner.test.ts tests\unit\raycast-manifest.test.ts tests\unit\raycast-view-model.test.ts` — 28 tests, 139 expectations.
- PASS: `bun run docs:sync`.
- PASS: `bun run docs:check`.
- PASS: `bun run ark:guard:writes`.
- PASS: `$env:CARGO_TARGET_DIR='D:\Personal\Hobby\Coding\kosmos\.tmp\cargo-ark-smoke'; bun run ark:smoke`.
- Note: `ark:smoke` still reports the existing Rust warning `function collect_state_events is never used` in `services\kepler-backend\src\dictation\host.rs`; smoke passed.
