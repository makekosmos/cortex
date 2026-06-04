# 2026-06-04 raycast-grid-dropdown

## Context

The List host already supports Raycast dropdown search accessories. Grid exposes `Grid.Dropdown` in the shim but did not accept `searchBarAccessory` or render dropdown options in the shell host.

## Scope

In scope:

- Add `Grid.searchBarAccessory` to the local `@raycast/api` shim.
- Normalize `Grid.Dropdown` from root props into the snapshot.
- Extract grouped Grid dropdown options in the host model.
- Render the Grid dropdown next to search and dispatch `onChange` callbacks through guarded session IPC.
- Add focused tests, docs, proof evidence, and a visual screenshot.

Out of scope:

- Filtering Grid items by dropdown value inside the host.
- Grid.Dropdown controlled `value` beyond the existing `defaultValue` / `value` snapshot read.
- Grid accessory types other than dropdown.

## Acceptance Criteria

AC1. Focused tests pass and cover `Grid.Dropdown` snapshot/model extraction.

AC2. `bun run shell:typecheck` passes after API and Vue renderer changes.

AC3. A Playwright visual check captures Grid search plus dropdown and verifies dropdown callback status.

AC4. `bun run docs:sync` and `bun run docs:check` pass after docs updates.

AC5. The full Raycast unit suite plus `ark:guard:writes` and `ark:smoke` pass.

## Verification commands

- `bun test tests\unit\raycast-view-model.test.ts tests\unit\raycast-api.test.ts`
- `bun run shell:typecheck`
- Visual script under `.tmp/visual/2026-06-04-raycast-host/`
- `bun test tests\unit\raycast-api.test.ts tests\unit\raycast-manifest.test.ts tests\unit\raycast-command-runner.test.ts tests\unit\raycast-view-model.test.ts tests\unit\extension-permissions.test.ts`
- `bun run docs:sync`
- `bun run docs:check`
- `bun run ark:guard:writes`
- `bun run ark:smoke`

## Out of Scope Decisions

The dropdown callback is delivered to trusted extension code; semantic filtering by selected dropdown value remains extension-owned state for now.
