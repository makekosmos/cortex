# Problems Found During Verification

## Review Regressions

1. `apps/delphi/ts/electron/google-calendar.ts`
   `syncGoogleCalendar()` returned the current `syncInFlight` promise immediately, so a later visible range requested during an in-flight refresh was dropped.

2. `apps/delphi/ts/src/pages/SettingsPage.vue`
   The Google Calendar section no longer exposed any inputs that call `saveConfig()`, leaving packaged Electron builds without a user-visible OAuth configuration path.

3. `apps/delphi/ts/src/App.vue`
   The non-Electron branch stopped after `setHydrated(true)`, which removed the existing browser bootstrap/auth path and left `build:web` unable to reconnect.
