# Task: Sidebar token parity and focus-only DB refresh

## Goal
Align dashboard sidebar button colors with `@kosmos/visuals` token usage instead of local approximations, and make dashboard DB refresh run every 5 seconds only while the window is focused.

## Acceptance Criteria
- AC1: Sidebar button and project-link states in `packages/kosmos-visuals` use sidebar-specific theme variables rather than dashboard-local color approximations.
- AC2: `apps/dashboard` refreshes snapshot data every 5 seconds only while the window is focused and visible, and stops polling when it is not.
- AC3: `apps/dashboard` still passes renderer typecheck and build in the current repository environment.
