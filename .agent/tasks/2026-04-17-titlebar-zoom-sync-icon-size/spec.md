# Spec

## Titlebar Zoom Sync And Smaller Icons

### Goal
Keep the shared titlebar at `36px`, make Windows native overlay controls track the current zoom factor, and reduce titlebar icon sizing so the chrome looks less heavy.

### Acceptance Criteria
- AC1: Shared titlebar height is `36px`.
- AC2: Dashboard, Delphi, and Eden update Windows native `titleBarOverlay.height` from the current zoom factor instead of a fixed stale size.
- AC3: Shared history control icons are smaller than before.
- AC4: Dashboard, Delphi, and Eden leading titlebar icon buttons use smaller icon sizing.
- AC5: TypeScript checks pass for `apps/dashboard`, `apps/delphi/ts`, and `apps/eden/ts`.
