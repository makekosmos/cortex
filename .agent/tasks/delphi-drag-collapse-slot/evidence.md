# Evidence: Collapse Dragged Task Slot

## Verification Summary

- AC1: PASS — shared `TodoRow.vue` now keeps the source row in place as an invisible drag source, so the layout no longer hard-reorders on every pointer move.
- AC2: PASS — `WeekPage.vue` now uses a custom drag image and hides the source card in place, so only one visible drag copy remains during native drag.
- AC3: PASS — drop targeting still works after the layout change; shared rows now use line indicators instead of a moving placeholder, and week-board targeting still uses `draggingId`.
- AC4: PASS — `bunx tsc --noEmit` completed successfully in `apps/delphi/ts`.

## Commands

- `bunx tsc --noEmit`

## Raw Artifacts

- `raw/drag-collapse-diff.txt`
- `raw/verification-summary.txt`
- `raw/tsc-noemit.txt`
