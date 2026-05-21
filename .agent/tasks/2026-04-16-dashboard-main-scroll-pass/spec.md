# Task: Dashboard main scroll pass

## Goal

Fix dashboard right-pane scrolling by moving scroll ownership from the nested content node to the full main content column.

## Acceptance Criteria

- AC1: Dashboard content area scrolls reliably when overview/session pages exceed the viewport height.
- AC2: Scrolling works across the whole right pane, not only inside a fragile nested scroll region.
- AC3: `apps/dashboard` still passes typecheck verification after the layout change.
