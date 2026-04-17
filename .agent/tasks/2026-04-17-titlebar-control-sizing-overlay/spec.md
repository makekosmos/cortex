# Spec

## Titlebar Control Sizing And Windows Overlay Alignment

### Goal
Bring shared titlebar history controls to the same visual size as other titlebar buttons, and align Windows native window controls with the shared titlebar height so the native controls occupy the full titlebar band without vertical mismatch.

### Acceptance Criteria
- AC1: Shared titlebar exposes a reusable control size contract for titlebar buttons.
- AC2: `TitlebarHistoryControls` uses the shared titlebar control sizing instead of its own divergent hardcoded sizing.
- AC3: Dashboard titlebar buttons remain visually aligned with the shared control size after the shared changes.
- AC4: Eden titlebar toggle is brought to the same size/radius as the other titlebar buttons without changing the rest of sidebar icon sizing.
- AC5: Shared titlebar height is aligned with the Windows native overlay height so the native controls occupy the same vertical band in Dashboard, Delphi, and Eden.
- AC6: TypeScript typechecks pass for `apps/dashboard`, `apps/delphi/ts`, and `apps/eden/ts`.
