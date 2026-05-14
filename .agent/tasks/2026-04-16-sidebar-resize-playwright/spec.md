# Sidebar resize playwright

## Goal

Restore reliable sidebar resizing in shared `kosmos-visuals` and prove it with Playwright against `apps/dashboard`.

## Acceptance Criteria

- AC1: The shared sidebar resize hit area is not clipped by the desktop chrome layout.
- AC2: `apps/dashboard` exposes stable selectors for sidebar width and resize handle.
- AC3: A Playwright test drags the sidebar resize handle and verifies sidebar width changes.
- AC4: `apps/dashboard` Playwright CLI passes for the resize scenario on the current codebase.
