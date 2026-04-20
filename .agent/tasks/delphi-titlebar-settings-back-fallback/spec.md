# Delphi Titlebar Settings Back Fallback

## Goal

Make the titlebar back button do something useful on the `/settings` route even when there is no recorded router history entry.

## Acceptance Criteria

- AC1: On `/settings`, the titlebar back button is enabled even when the local Electron history stack is empty.
- AC2: In Electron runtime, pressing titlebar back on `/settings` without a history entry navigates out of settings to `/`.
- AC3: Outside `/settings`, existing titlebar back/forward behavior remains unchanged.
- AC4: `bunx tsc --noEmit` passes in `apps/delphi/ts`.
