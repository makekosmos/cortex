# Evidence

## Implementation

- `shell/electron/raycast/view-model.ts` now preserves `Detail.actions` as a normalized `ActionPanel` child.
- `shell/src/raycast-host/model.ts` exposes `detailActions()` and keeps `ActionPanel` children out of markdown fallback collection.
- `shell/src/raycast-host/RaycastDetailView.vue` renders a root-detail action footer when a Raycast session is present and executes supported actions through guarded session IPC.
- `shell/src/views/RaycastHostView.vue` passes the session id to root `Detail` views.
- Unit fixtures cover trusted command runner snapshots and host model parsing.
- Docs updated in `docs-site/concepts/extension-host.md` and `docs-site/apps/kepler-roadmap.md`.

## Verification

- PASS: `bun test tests\unit\raycast-view-model.test.ts tests\unit\raycast-command-runner.test.ts` — 22 tests, 114 expectations.
- PASS: `bun run shell:typecheck`.
- PASS: visual verify via `.tmp\visual\2026-06-04-raycast-host\capture-detail-actions.mjs`; screenshot saved at `.tmp\visual\2026-06-04-raycast-host\raycast-detail-actions-1000x720.png`.
- PASS: `bun test tests\unit\raycast-api.test.ts tests\unit\raycast-command-runner.test.ts tests\unit\raycast-manifest.test.ts tests\unit\raycast-view-model.test.ts` — 26 tests, 131 expectations.
- PASS: `bun run docs:sync`.
- PASS: `bun run docs:check`.
- PASS: `bun run ark:guard:writes`.
- PASS: `$env:CARGO_TARGET_DIR='D:\Personal\Hobby\Coding\kosmos\.tmp\cargo-ark-smoke'; bun run ark:smoke`.
- Note: `ark:smoke` still reports the existing Rust warning `function collect_state_events is never used` in `services\kepler-backend\src\dictation\host.rs`; smoke passed.
