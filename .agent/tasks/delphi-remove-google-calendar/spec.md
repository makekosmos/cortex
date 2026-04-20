# Task: Remove Delphi Google Calendar Integration

## Goal
Remove the current Google Calendar integration from `apps/delphi/ts` while keeping the local task calendar surface functional and leaving room to add external calendar sync back later.

## Scope
- `apps/delphi/ts/electron/main.ts`
- `apps/delphi/ts/electron/google-calendar.ts`
- `apps/delphi/ts/src/composables/useGoogleCalendar.ts`
- `apps/delphi/ts/src/components/calendar/CalendarShell.vue`
- `apps/delphi/ts/src/pages/SettingsPage.vue`
- `apps/delphi/ts/src/services/google-calendar/*`
- verification artifacts in `.agent/tasks/delphi-remove-google-calendar/`

## Component Map
- `CalendarShell.vue`: renders the calendar surface and should remain focused on local task-derived entries only.
- `SettingsPage.vue`: renders app settings and should stop exposing Google OAuth/configuration controls.
- `electron/main.ts`: wires Electron IPC and should stop registering Google Calendar endpoints.
- `services/google-calendar/*`: contains Google-specific contracts/helpers and should be removed or reduced only to pieces still needed by the task calendar.

## Acceptance Criteria
- AC1: Delphi no longer registers or exposes Google Calendar IPC handlers.
- AC2: Calendar UI in `delphi/ts` no longer depends on Google Calendar connection/configuration state and still renders task entries.
- AC3: Settings UI in `delphi/ts` no longer shows Google Calendar controls or OAuth configuration logic.
- AC4: Unused Google Calendar-specific source files are removed from `apps/delphi/ts`.
- AC5: TypeScript verification for `apps/delphi/ts` passes after the change.
