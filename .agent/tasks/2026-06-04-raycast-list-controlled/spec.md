# 2026-06-04 raycast-list-controlled

## Context

Raycast-compatible trusted view commands already render List snapshots in the Kosmos shell. The host still treated search text, selected item, and filtering as purely local UI state even though the `@raycast/api` shim already exposes Raycast-style controlled List props.

## Scope

In scope:

- Preserve `List.searchText`, `List.selectedItemId`, and `List.filtering` in the normalized snapshot.
- Register `List.onSearchTextChange` and `List.onSelectionChange` callbacks in the trusted callback registry.
- Make the Vue List host initialize from controlled props, respect `filtering: false`, and dispatch search/selection callbacks through guarded session IPC.
- Add regression coverage, docs, and a visual screenshot for the controlled List state.

Out of scope:

- Controlled Grid props.
- Debounced/throttled search behavior for `List.throttle`.
- Untrusted Raycast extension execution.

## Acceptance Criteria

AC1. `bun test tests\unit\raycast-view-model.test.ts` passes and covers controlled List props and callbacks.

AC2. `bun run shell:typecheck` passes after the Vue renderer changes.

AC3. A Playwright visual check captures a Raycast List with initial search text, `filtering: false`, and callback-driven selection/search status.

AC4. `bun run docs:sync` and `bun run docs:check` pass after updating Raycast compatibility docs.

AC5. The full Raycast unit suite plus `ark:guard:writes` and `ark:smoke` pass.

## Verification commands

- `bun test tests\unit\raycast-view-model.test.ts`
- `bun run shell:typecheck`
- Visual script under `.tmp/visual/2026-06-04-raycast-host/`
- `bun test tests\unit\raycast-api.test.ts tests\unit\raycast-manifest.test.ts tests\unit\raycast-command-runner.test.ts tests\unit\raycast-view-model.test.ts tests\unit\extension-permissions.test.ts`
- `bun run docs:sync`
- `bun run docs:check`
- `bun run ark:guard:writes`
- `bun run ark:smoke`

## Out of Scope Decisions

Grid exposes similar props in the shim, but this slice intentionally keeps to List because the current Raycast host work was interrupted at List controlled props.
