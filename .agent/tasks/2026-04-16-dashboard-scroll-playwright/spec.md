# Task: Dashboard scroll verified by Playwright

## Goal
Make dashboard content scrolling work in the running Electron app and verify it with Playwright CLI against a real window.

## Acceptance Criteria
- AC1: Dashboard exposes a stable scroll container test hook for the main content pane.
- AC2: A Playwright test proves the overview page can scroll downward in the running Electron app when content exceeds the viewport.
- AC3: `apps/dashboard` Playwright CLI suite passes after the fix.
