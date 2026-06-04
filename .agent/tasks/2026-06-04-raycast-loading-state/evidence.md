# Evidence - Raycast List/Grid loading state slice

Verified at: 2026-06-04T21:45:00+03:00

## Summary

PASS. The Raycast host now extracts and renders `isLoading` for `List`
and `Grid` roots.

## Acceptance Criteria

- PASS: `listIsLoading()` extracts `List.props.isLoading`.
- PASS: `gridIsLoading()` extracts `Grid.props.isLoading`.
- PASS: `RaycastListView.vue` renders `Загрузка...` and suppresses empty text
  while loading.
- PASS: `RaycastGridView.vue` renders `Загрузка...` and suppresses empty text
  while loading.
- PASS: docs updated in `docs-site/concepts/extension-host.md` and
  `docs-site/apps/kepler-roadmap.md`.

## Automated Checks

- PASS: `bun test tests/unit/raycast-view-model.test.ts`
  - 6 tests passed, 0 failed, 45 expectations.
- PASS: `bun run shell:typecheck`
- PASS: `bun test tests/unit/raycast-api.test.ts tests/unit/raycast-manifest.test.ts tests/unit/raycast-command-runner.test.ts tests/unit/raycast-view-model.test.ts tests/unit/extension-permissions.test.ts`
  - 28 tests passed, 0 failed, 103 expectations.
- PASS: `bun run docs:sync`
- PASS: `bun run docs:check`
- PASS: `bun run ark:guard:writes`
- PASS: `bun run ark:smoke`
  - ARK smoke matrix passed.
  - Existing warning observed: `collect_state_events` is unused in
    `services/kepler-backend/src/dictation/host.rs`.

## Visual Verification

- PASS: local Vite renderer with mocked loading List snapshot.
- Screenshot:
  `.tmp/visual/2026-06-04-raycast-host/raycast-list-loading-1000x720.png`
- Observed state: `Загрузка...` appears in the list body, `List.EmptyView`
  text is suppressed, and there is no overlap at 1000x720.

## Remaining Scope

Animated spinner parity, partial stale-result overlays, async search callbacks,
and throttling remain for later Raycast phases.
