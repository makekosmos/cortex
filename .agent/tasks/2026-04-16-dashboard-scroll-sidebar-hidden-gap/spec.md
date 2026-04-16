# Task: Dashboard scroll and hidden sidebar gap

## Goal
Restore dashboard page scrolling after the shared content-surface refactor and remove the leftover sidebar gap when the sidebar is hidden.

## Acceptance Criteria
- AC1: Dashboard main content scrolls again when page content exceeds the viewport height.
- AC2: Hiding the dashboard sidebar no longer leaves a reserved collapsed gap.
- AC3: `apps/dashboard` still passes typecheck verification after the fixes.
