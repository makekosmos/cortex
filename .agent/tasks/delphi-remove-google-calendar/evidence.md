# Evidence: Remove Delphi Google Calendar Integration

## Verification Summary

- AC1: PASS — `apps/delphi/ts/electron/main.ts` no longer registers Google Calendar IPC, and `apps/delphi/ts/electron/google-calendar.ts` was removed.
- AC2: PASS — `CalendarShell.vue` now renders only local task entries, and the calendar UI no longer depends on Google connection/configuration state.
- AC3: PASS — `SettingsPage.vue` no longer exposes Google OAuth/configuration controls.
- AC4: PASS — Google Calendar-specific source files were removed from `apps/delphi/ts`; shared calendar date/view-mode helpers now live under `src/services/calendar/`.
- AC5: PASS — `bunx tsc --noEmit` completed successfully in `apps/delphi/ts`.

## Commands

- `bunx tsc --noEmit`
- `rg -n 'google-calendar|useGoogleCalendar|GoogleCalendarViewMode|GoogleCalendarRange' apps/delphi/ts`

## Raw Artifacts

- `raw/delphi-google-calendar-diff.txt`
- `raw/verification-summary.txt`
- `raw/tsc-noemit.txt`
