# Task: Collapse Dragged Task Slot

## Goal
When a task is dragged in Delphi, the original item should stop occupying layout space so only one visible placeholder slot remains during drag.

## Scope
- `packages/kosmos-visuals/components/TodoRow.vue`
- `apps/delphi/ts/src/pages/WeekPage.vue`
- verification artifacts in `.agent/tasks/delphi-drag-collapse-slot/`

## Component Map
- `TodoRow.vue`: shared row drag behavior for list-based task pages; should collapse the source row while its placeholder slot is shown.
- `WeekPage.vue`: week board card drag behavior; should collapse the dragged card's original slot while drop indicators remain active.

## Acceptance Criteria
- AC1: Dragging a `TodoRow` no longer leaves the original row occupying list space during drag.
- AC2: Dragging a week-board task card no longer leaves the original card occupying list space during drag.
- AC3: Drop targeting still works after the layout collapse.
- AC4: TypeScript verification for `apps/delphi/ts` passes after the change.
