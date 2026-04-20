# Delphi Titlebar History Controls

## Goal

Restore working back/forward navigation for the titlebar controls in `apps/delphi/ts`.

## Acceptance Criteria

- AC1: In Electron runtime, the titlebar back button becomes enabled after navigating away from the initial route.
- AC2: In Electron runtime, clicking the titlebar back button navigates to the previous route instead of doing nothing.
- AC3: In Electron runtime, clicking the titlebar forward button after a back navigation returns to the newer route.
- AC4: The fix does not depend on `router.options.history.state.back/forward` for Electron memory history.
- AC5: `bunx tsc --noEmit` passes in `apps/delphi/ts`.
