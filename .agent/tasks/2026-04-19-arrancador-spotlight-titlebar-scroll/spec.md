# Task: Arrancador spotlight titlebar removal and keyboard scroll sync

## Goal

Remove the spotlight trigger from the desktop titlebar and ensure keyboard navigation inside the spotlight keeps the active result visible.

## Acceptance Criteria

- AC1: Hiding the sidebar no longer causes a search trigger to appear in the desktop titlebar.
- AC2: Arrow-key navigation inside spotlight scrolls the active result into view instead of allowing the selection to move outside the visible viewport.
- AC3: Relevant Arrancador tests cover the updated titlebar and spotlight behavior.
- AC4: `bun run typecheck` and `bun run test` pass in `apps/arrancador`.
