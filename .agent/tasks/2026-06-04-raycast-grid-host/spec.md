# Raycast Grid host vertical slice

## Classification

`FULL_LOOP` — task touches Raycast view component parity, shell renderer UI, host action lifecycle, docs, visual verification, and smoke coverage.

## Goal

Add the first safe Raycast `Grid` root host for trusted extensions, matching the existing Kosmos Raycast host patterns and keeping privileged actions behind guarded session IPC.

## Scope

In scope:

- `@raycast/api` Grid item prop typing.
- Snapshot normalization for `Grid.Item.actions`.
- Renderer model for `Grid`, `Grid.Section`, `Grid.Item`, and `Grid.EmptyView`.
- Vue renderer for grid sections, item cards, image/placeholder previews, search filtering, selection, and ActionPanel.
- Builtin action execution for selected grid item through existing guarded session IPC.
- Unit tests, shell typecheck, visual screenshot, docs update, and ARK guards/smoke.

Out of scope:

- Full Raycast Grid accessories, insets, fit modes, pagination, remote image cache, and metadata.
- Untrusted user-installed command sandboxing.
- React/TSX runtime and bundler.
- Publishing/release/version bump.

## Acceptance Criteria

**AC1.** `Grid` root snapshots render in `#raycast-host` instead of falling through to unsupported view.

**AC2.** Grid sections/items preserve section titles, item title/subtitle/keywords, image source, actions, and empty view fallback.

**AC3.** Grid renderer provides search filtering, stable selected item behavior, card layout, image/placeholder previews, and ActionPanel.

**AC4.** Builtin actions on grid items execute through guarded Raycast session IPC and show visible Russian status.

**AC5.** Verification evidence records unit tests, `bun run shell:typecheck`, visual screenshot, docs sync/check, ARK guard, and ARK smoke.
