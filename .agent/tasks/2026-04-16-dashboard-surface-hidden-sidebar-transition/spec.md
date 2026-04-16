# Task: Dashboard surface transition on hidden sidebar

## Goal
When the dashboard sidebar is hidden, the content surface should visually detach from the sidebar: the left border disappears and the top-left radius animates smoothly to `0`.

## Acceptance Criteria
- AC1: `DesktopContentSurface` supports toggling the left divider and top-left radius through props/CSS variables.
- AC2: Dashboard drives those props from sidebar hidden state so the left border disappears and the radius transitions to `0` when the sidebar is hidden.
- AC3: `apps/dashboard` still passes typecheck verification after the change.
