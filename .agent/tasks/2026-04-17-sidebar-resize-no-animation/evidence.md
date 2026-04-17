# Evidence: Manual sidebar resize no longer animates

## Summary

- Removed width transition while the shared sidebar is actively being resized by drag.
- Kept collapse/expand animation untouched by leaving the existing `.animating` rule in place.
- Focused type checks passed for dashboard, Delphi, and Eden consumers.

## Acceptance Criteria

### AC1

Manual drag resize does not animate sidebar width.

Status: PASS

Proof:
- `packages/kepler-visuals/components/sidebar.css` now defines `.kepler-sidebar-wrapper.is-resizing { transition: none; }`.
- The shared sidebar already toggles `is-resizing` during pointer-driven resize, so width updates now apply immediately while dragging.
- Raw artifacts:
  - [sidebar-css-grep.txt](/D:/Personal/Hobby/Coding/kepler/.agent/tasks/2026-04-17-sidebar-resize-no-animation/raw/sidebar-css-grep.txt)
  - [sidebar-css-diff.txt](/D:/Personal/Hobby/Coding/kepler/.agent/tasks/2026-04-17-sidebar-resize-no-animation/raw/sidebar-css-diff.txt)

### AC2

Collapse and expand animation remains intact.

Status: PASS

Proof:
- The existing `.kepler-sidebar-wrapper.animating` rule remains unchanged.
- No hide/show logic in `Sidebar.vue` was modified for this fix, so only the drag-resize path changes behavior.
- Raw artifacts:
  - [sidebar-css-grep.txt](/D:/Personal/Hobby/Coding/kepler/.agent/tasks/2026-04-17-sidebar-resize-no-animation/raw/sidebar-css-grep.txt)
  - [sidebar-css-diff.txt](/D:/Personal/Hobby/Coding/kepler/.agent/tasks/2026-04-17-sidebar-resize-no-animation/raw/sidebar-css-diff.txt)

### AC3

Focused checks pass after the styling change.

Status: PASS

Proof:
- `apps/dashboard` typecheck passed.
- `apps/delphi/ts` typecheck passed.
- `apps/eden/ts` typecheck passed.
- Raw artifacts:
  - [dashboard-tsc.txt](/D:/Personal/Hobby/Coding/kepler/.agent/tasks/2026-04-17-sidebar-resize-no-animation/raw/dashboard-tsc.txt)
  - [delphi-tsc.txt](/D:/Personal/Hobby/Coding/kepler/.agent/tasks/2026-04-17-sidebar-resize-no-animation/raw/delphi-tsc.txt)
  - [eden-tsc.txt](/D:/Personal/Hobby/Coding/kepler/.agent/tasks/2026-04-17-sidebar-resize-no-animation/raw/eden-tsc.txt)

## Checks Run

1. `apps/dashboard/node_modules/.bin/tsc.exe --noEmit -p apps/dashboard/tsconfig.json`
2. `apps/dashboard/node_modules/.bin/tsc.exe --noEmit -p apps/delphi/ts/tsconfig.json`
3. `apps/dashboard/node_modules/.bin/tsc.exe --noEmit -p apps/eden/ts/tsconfig.json`

## Notes

- This is intentionally a narrow CSS-only fix: drag resize becomes immediate, while explicit sidebar open/close animation stays as-is.
