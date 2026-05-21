# Spec

## Titlebar Height 36 With Native Windows Controls

### Goal

Set the shared desktop titlebar height to `36px` and align Windows native `titleBarOverlay.height` to the same value so the native controls get a taller hit area while remaining native.

### Acceptance Criteria

- AC1: Shared titlebar height is `36px`.
- AC2: Dashboard, Delphi, and Eden Windows `titleBarOverlay.height` are `36`.
- AC3: Shared titlebar button sizing remains unchanged after the height increase.
- AC4: TypeScript checks pass for `apps/dashboard`, `apps/delphi/ts`, and `apps/eden/ts`.
