# Evidence - Raycast Detail.Metadata host slice

Verified at: 2026-06-04T20:10:00+03:00

## Summary

PASS. The shell Raycast host can now render root `Detail` snapshots and
`Detail.Metadata` rows for labels, links, separators, and tag lists.

## Acceptance Criteria

- PASS: `RaycastHostView.vue` routes `snapshot.root.type === "Detail"` to
  `RaycastDetailView`.
- PASS: `detailMarkdown()` skips `Detail.Metadata` children when building
  markdown fallback text.
- PASS: `detailMetadataItems()` extracts label/link/tag/separator models.
- PASS: new `RaycastMetadataView.vue` renders metadata with scoped CSS and
  escaped interpolation only.
- PASS: command-runner fixture includes a trusted root Detail command with
  metadata.
- PASS: docs updated in `docs-site/concepts/extension-host.md` and
  `docs-site/apps/kepler-roadmap.md`.

## Automated Checks

- PASS: `bun test tests/unit/raycast-view-model.test.ts tests/unit/raycast-api.test.ts`
  - 7 tests passed, 0 failed, 43 expectations.
- PASS: `bun run shell:typecheck`
- PASS: `bun test tests/unit/raycast-api.test.ts tests/unit/raycast-manifest.test.ts tests/unit/raycast-command-runner.test.ts tests/unit/raycast-view-model.test.ts tests/unit/extension-permissions.test.ts`
  - 26 tests passed, 0 failed, 92 expectations.
- PASS: `bun run docs:sync`
- PASS: `bun run docs:check`
- PASS: `bun run ark:guard:writes`
- PASS: `bun run ark:smoke`
  - ARK smoke matrix passed.

## Visual Verification

- PASS: local Vite renderer with mocked root Detail snapshot.
- Screenshot:
  `.tmp/visual/2026-06-04-raycast-host/raycast-detail-metadata-1000x720.png`
- Observed state: root `Detail` title `Карточка проекта`, markdown heading,
  metadata label `Статус`, link `Документация`, separator, tag pills
  `raycast`, `kosmos`, `ui-runtime`; no clipping or overlap at 1000x720.

## Remaining Scope

Guarded link opening, icon/color metadata parity, and QuickLook remain for
later Raycast phases.
