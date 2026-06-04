# 2026-06-04 raycast-grid-controlled

## Context

Raycast-compatible Grid commands already render sectioned cards and selected-item actions in the Kosmos shell. The `@raycast/api` shim exposes controlled Grid props, but the host still kept search and selection as local-only renderer state.

## Scope

In scope:

- Preserve `Grid.searchText`, `Grid.selectedItemId`, and `Grid.filtering` in the host model.
- Dispatch `Grid.onSearchTextChange` and `Grid.onSelectionChange` callbacks through guarded session IPC.
- Make the Vue Grid host initialize from controlled props and honor `filtering: false`.
- Add regression coverage, docs, proof evidence, and a visual screenshot.

Out of scope:

- `Grid.Dropdown` / `searchBarAccessory` parity.
- `List.throttle` / `Grid` throttling behavior.
- Rich Grid metadata/accessories beyond the existing card host.

## Acceptance Criteria

AC1. `bun test tests\unit\raycast-view-model.test.ts` passes and covers controlled Grid props and callbacks.

AC2. `bun run shell:typecheck` passes after the Vue renderer changes.

AC3. A Playwright visual check captures a Raycast Grid with initial search text, `filtering: false`, and callback-driven search/selection status.

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

Grid controlled state is implemented separately from List so each root host has direct regression coverage and visual evidence.
