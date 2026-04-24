# Delphi Calendar Toolbar View Switch

## Original Task Statement

User request summary:

- in the calendar, remove the settings button;
- put the calendar view switch (`день`, `4 дня`, `неделя`, `месяц`) there instead.

## Summary

Move the existing calendar view switch from the page header into the calendar toolbar, replacing the settings button currently shown on the right side of the toolbar.

This should reduce duplication and keep calendar-specific controls inside the calendar surface.

## Component Map

- `CalendarPage.vue`
  - Responsibility: keep page title and own calendar state.
- `CalendarShell.vue`
  - Responsibility: render toolbar controls and emit view-mode changes upward.
- `CalendarViewSwitch.vue`
  - Responsibility: remain the dedicated reusable view switch component.

## Acceptance Criteria

- AC1: The calendar toolbar no longer renders the `Настройки` button.
- AC2: The calendar toolbar renders the existing `CalendarViewSwitch` in its place.
- AC3: Changing the switch in the toolbar updates the calendar view mode correctly.
- AC4: `CalendarPage.vue` no longer renders a duplicate view switch in the page header.
- AC5: Delphi TypeScript compile check passes after the change.
- AC6: Delphi production build passes after the change.

## Constraints

- Keep the diff focused on calendar UI composition.
- Reuse the existing `CalendarViewSwitch.vue` component.
- Keep state ownership in the parent page and use explicit props/emits.

## Non-Goals

- Calendar visual redesign beyond this control move.
- Settings route changes.
- Calendar data model changes.

## Verification Plan

1. Run Delphi TypeScript compile check.
2. Run Delphi production build.
3. Inspect code to confirm:
   - `CalendarShell.vue` contains the view switch and no settings button
   - `CalendarPage.vue` no longer duplicates the switch in the page header
