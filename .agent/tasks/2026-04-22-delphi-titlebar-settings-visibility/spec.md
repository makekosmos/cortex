# Delphi Titlebar Settings Visibility

## Original Task Statement

User report summary:

- the settings button in the titlebar is not visible enough to notice.

## Summary

Make the Delphi titlebar settings control visually explicit so it is immediately discoverable.

The button currently renders as a small icon-only control next to the status dot, which makes it easy to miss. Replace it with a clearer button that includes both icon and text while keeping the existing `/settings` navigation behavior.

## Acceptance Criteria

- AC1: Delphi titlebar renders a clearly visible settings control with label text, not only a tiny icon.
- AC2: Clicking that control still navigates to `/settings`.
- AC3: The active settings route still shows the control in an active/selected visual state.
- AC4: Delphi TypeScript compile check passes after the change.
- AC5: Delphi production build passes after the change.

## Constraints

- Keep the diff focused on titlebar settings discoverability.
- Reuse the existing `openSettings()` route flow.
- Do not redesign the whole titlebar layout.

## Non-Goals

- Settings page redesign.
- Status dot redesign.
- Sidebar navigation changes.

## Verification Plan

1. Run Delphi TypeScript compile check.
2. Run Delphi production build.
3. Inspect `App.vue` to confirm the settings control is now a visible text+icon button in the titlebar.
