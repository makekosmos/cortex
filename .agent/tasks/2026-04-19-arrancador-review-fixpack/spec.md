# 2026-04-19 Arrancador Review Fixpack

## Context

`apps/arrancador` has a cluster of confirmed regressions in the new shell/titlebar/sidebar work:

- desktop titlebar history is implemented as a private stack that diverges from router history
- the `/settings` fallback back-button behavior does not respect real navigation history
- the mobile sidebar is implemented as an off-canvas `div` instead of an actual modal sheet/dialog
- hidden/offscreen sidebar content remains mounted in the tab order on mobile
- desktop search is no longer reachable when the sidebar is hidden or the mobile drawer is closed
- Windows now uses frameless chrome without replacement window controls
- the sidebar resize affordance is hard to grab because of z-index layering
- tests do not reliably catch the real shell/sidebar/search regressions

## Acceptance Criteria

- AC1: titlebar back/forward uses real router/browser history semantics instead of a private divergent stack, and `/settings` no longer fakes history with a hardcoded fallback
- AC2: mobile sidebar is presented as a proper modal interaction, hidden menu content is not keyboard-focusable when closed, and the menu has an in-panel dismiss control
- AC3: desktop search remains reachable when the sidebar is hidden, and spotlight state resets correctly on dismiss/reopen and route changes
- AC4: Windows frameless mode still exposes working minimize/maximize/close controls, and the shell keeps usable desktop sidebar resize/collapse interactions
- AC5: tests are updated to exercise the real regression paths closely enough to catch these behaviors, and fresh verification passes for `bun run typecheck` and targeted `bun run test`
