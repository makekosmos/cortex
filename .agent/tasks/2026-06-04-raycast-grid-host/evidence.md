# Evidence - Raycast Grid host vertical slice

Verified at: 2026-06-04T18:45:00+03:00

## Summary

PASS. Raycast-compatible trusted view commands can now render a first Grid host
surface in the shell renderer. The slice covers `Grid`, `Grid.Section`,
`Grid.Item`, `Grid.EmptyView`, card selection, image and placeholder previews,
search filtering, and selected-item `ActionPanel` execution through the guarded
Raycast session IPC path.

## Acceptance Criteria

- PASS: `@raycast/api` exposes typed `Grid` / `Grid.Item` primitives.
- PASS: the main-process Raycast view normalizer serializes sectioned Grid
  trees, item metadata, accessory text, image payloads, and nested item actions.
- PASS: the shell Raycast host routes `snapshot.root.type === "Grid"` to a
  dedicated Vue host.
- PASS: Grid UI follows existing Kosmos visual tokens, supports search and
  empty state copy in Russian, and keeps the selected item action panel visible.
- PASS: `Action.CopyToClipboard` from a selected Grid item goes through the same
  guarded session action IPC as List actions.
- PASS: docs mention the current Grid support boundary and remaining rich
  navigation work.

## Automated Checks

- PASS: `bun test tests/unit/raycast-api.test.ts tests/unit/raycast-manifest.test.ts tests/unit/raycast-command-runner.test.ts tests/unit/raycast-view-model.test.ts tests/unit/extension-permissions.test.ts`
  - 22 tests passed, 0 failed, 78 expectations.
- PASS: `bun run shell:typecheck`
- PASS: `bun run docs:sync`
- PASS: `bun run docs:check`
- PASS: `bun run ark:guard:writes`
- PASS: `bun run ark:smoke`
  - ARK smoke matrix passed.
  - ARK core Rust tests: 166 passed.
  - ark-core RPC tests: 8 passed.
  - proptest invariants: 3 passed.
  - relay round-trip: 1 passed.
  - sync round-trip: 5 passed.
  - kepler-backend lib tests and extension builds completed.
  - Existing warning observed: `collect_state_events` is unused in
    `services/kepler-backend/src/dictation/host.rs`; not introduced by this
    slice.

## Visual Verification

- PASS: local Vite preview with mocked `window.kepler.raycast` snapshot.
- Screenshot:
  `.tmp/visual/2026-06-04-raycast-host/raycast-grid-copy-status-1000x720.png`
- Observed state: title `Галерея игр`, search field, section `Избранное`,
  selected `Disco Elysium` card with placeholder preview, `Outer Wilds` image
  preview, action button `Скопировать`, status `Скопировано`, no visible
  overlap or clipped text at 1000x720.

## Remaining Scope

Grid rich parity is intentionally not complete yet. Still open for later
Raycast phases: masonry/fit variants, metadata accessories beyond the initial
text/title/subtitle model, keyboard shortcuts, nested navigation stack parity,
menu-bar commands, real TS/TSX bundling and HMR, preferences UI, and untrusted
extension sandboxing.
