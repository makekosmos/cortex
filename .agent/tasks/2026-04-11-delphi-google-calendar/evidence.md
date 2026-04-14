# Evidence: Delphi TS Google Calendar

## Verification Summary

- `AC1` PASS
  - New route `/calendar` is registered in [apps/delphi/ts/src/router/index.ts](/Users/kirill/Documents/projects/kepler/apps/delphi/ts/src/router/index.ts:3).
  - Sidebar exposes the new Calendar entry in [apps/delphi/ts/src/components/SideBar.vue](/Users/kirill/Documents/projects/kepler/apps/delphi/ts/src/components/SideBar.vue:69).
  - The page title and 4-button segmented switch are rendered together in [apps/delphi/ts/src/pages/CalendarPage.vue](/Users/kirill/Documents/projects/kepler/apps/delphi/ts/src/pages/CalendarPage.vue:20) and [apps/delphi/ts/src/components/calendar/CalendarViewSwitch.vue](/Users/kirill/Documents/projects/kepler/apps/delphi/ts/src/components/calendar/CalendarViewSwitch.vue:16).

- `AC2` PASS
  - View state, persisted mode selection, visible ranges, and prev/next/today navigation are implemented in [apps/delphi/ts/src/composables/useCalendarState.ts](/Users/kirill/Documents/projects/kepler/apps/delphi/ts/src/composables/useCalendarState.ts:32).
  - Day/4-day/week timeline rendering is implemented in [apps/delphi/ts/src/components/calendar/CalendarTimeGrid.vue](/Users/kirill/Documents/projects/kepler/apps/delphi/ts/src/components/calendar/CalendarTimeGrid.vue:38).
  - Month rendering is implemented in [apps/delphi/ts/src/components/calendar/CalendarMonthGrid.vue](/Users/kirill/Documents/projects/kepler/apps/delphi/ts/src/components/calendar/CalendarMonthGrid.vue:24).
  - Local Delphi tasks and Google events are merged into one surface in [apps/delphi/ts/src/components/calendar/CalendarShell.vue](/Users/kirill/Documents/projects/kepler/apps/delphi/ts/src/components/calendar/CalendarShell.vue:52).

- `AC3` PASS
  - Electron OAuth uses a validated `state`, PKCE verifier/challenge, loopback redirect, and token exchange in [apps/delphi/ts/electron/google-calendar.ts](/Users/kirill/Documents/projects/kepler/apps/delphi/ts/electron/google-calendar.ts:294).
  - IPC registration and renderer access are wired via [apps/delphi/ts/electron/main.ts](/Users/kirill/Documents/projects/kepler/apps/delphi/ts/electron/main.ts:722) and [apps/delphi/ts/src/composables/useGoogleCalendar.ts](/Users/kirill/Documents/projects/kepler/apps/delphi/ts/src/composables/useGoogleCalendar.ts:60).
  - Session restore is derived from persisted config/session in [apps/delphi/ts/electron/google-calendar.ts](/Users/kirill/Documents/projects/kepler/apps/delphi/ts/electron/google-calendar.ts:250).

- `AC4` PASS
  - Durable cache and session/config persistence live under app-owned files using protected JSON or plain JSON fallback in [apps/delphi/ts/electron/google-calendar.ts](/Users/kirill/Documents/projects/kepler/apps/delphi/ts/electron/google-calendar.ts:110) and [apps/delphi/ts/electron/google-calendar.ts](/Users/kirill/Documents/projects/kepler/apps/delphi/ts/electron/google-calendar.ts:160).
  - Cached snapshot is loaded before refresh through the shared renderer composable in [apps/delphi/ts/src/composables/useGoogleCalendar.ts](/Users/kirill/Documents/projects/kepler/apps/delphi/ts/src/composables/useGoogleCalendar.ts:39).
  - Range coverage and stale-refresh policy are covered by tests in [apps/delphi/ts/src/services/google-calendar/cache.test.ts](/Users/kirill/Documents/projects/kepler/apps/delphi/ts/src/services/google-calendar/cache.test.ts:7).

- `AC5` PASS
  - Token expiry skew and restorable-session rules are covered by tests in [apps/delphi/ts/src/services/google-calendar/session.test.ts](/Users/kirill/Documents/projects/kepler/apps/delphi/ts/src/services/google-calendar/session.test.ts:7).
  - Refresh-token renewal and revoked-credential handling live in [apps/delphi/ts/electron/google-calendar.ts](/Users/kirill/Documents/projects/kepler/apps/delphi/ts/electron/google-calendar.ts:459) and [apps/delphi/ts/electron/google-calendar.ts](/Users/kirill/Documents/projects/kepler/apps/delphi/ts/electron/google-calendar.ts:648).
  - Renderer-side secrets are not written to `localStorage`; OAuth/session persistence is handled in Electron main, while settings UI only forwards commands in [apps/delphi/ts/src/pages/SettingsPage.vue](/Users/kirill/Documents/projects/kepler/apps/delphi/ts/src/pages/SettingsPage.vue:427).

- `AC6` PASS
  - Fresh `tsc --noEmit` pass was captured in `.agent/tasks/2026-04-11-delphi-google-calendar/artifacts/typecheck.txt`.
  - Fresh targeted Vitest pass was captured in `.agent/tasks/2026-04-11-delphi-google-calendar/artifacts/vitest.txt`.

## Commands

- `cd apps/delphi/ts && ./node_modules/.bin/tsc --noEmit`
- `cd apps/delphi/ts && ./node_modules/.bin/vitest run src/services/google-calendar/cache.test.ts src/services/google-calendar/session.test.ts`

## Raw Artifacts

- `.agent/tasks/2026-04-11-delphi-google-calendar/artifacts/typecheck.txt`
- `.agent/tasks/2026-04-11-delphi-google-calendar/artifacts/vitest.txt`
- `.agent/tasks/2026-04-11-delphi-google-calendar/artifacts/git-status.txt`

## Notes

- The repo already had unrelated dirty state in `packages/kepler-visuals/theme/css-variables.css`; it was left untouched.
- Live Google sign-in was not exercised against a real tenant in this session because no OAuth credentials were provided in the repository, but the implemented Electron flow, storage, and refresh path were verified in code and through supporting tests.
