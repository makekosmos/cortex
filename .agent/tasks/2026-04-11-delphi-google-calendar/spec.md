# Task Spec: Delphi TS Google Calendar

## Original Task

`apps/delphi/ts - нужно добавить страницу календаря с видом на день, 4 дня, неделю, месяц, выбор происходит мультикнопкой (4 связанных кнопки идущих рядом) справа от названия страницы. Также интегрироваться с гугл календарем. Надежно. Так, чтобы достаточно было просто войти в свой аккаунт (OAuth) и это прям зашивалось в данные приложения, чтобы при перезапусках и тд - все было ок`

## Scope

Add a dedicated calendar feature to `apps/delphi/ts` with:

- a new calendar page in the Vue/Electron app
- four switchable calendar views: day, 4 days, week, month
- a segmented 4-button view switch positioned to the right of the page title
- Google Calendar OAuth sign-in for the Electron app
- durable session persistence across app restarts
- durable local caching of Google calendars/events so the calendar still has data on startup and offline

## Assumptions

- The primary target is the Electron runtime. Web-only OAuth support is out of scope for this task.
- One Google account per local app profile is sufficient.
- Integration is read-only for Google Calendar data in this task: fetch calendars and events, render them in the calendar UI, refresh them, and persist them locally. Editing Google events is out of scope.
- Existing Delphi tasks remain local-first and are rendered together with Google events when a task has a scheduled date.
- Google OAuth client credentials will be supplied through app settings or environment-backed config surfaced by the app; the implementation must not hardcode secrets in renderer code.

## Constraints

- Follow Vue 3 Composition API with `<script setup lang="ts">`.
- Keep route view components thin and move non-trivial calendar logic into feature components/composables/services.
- Local state is the source of truth for displayed calendar data; network refresh is a side effect.
- Secrets must not be stored in renderer `localStorage`.
- Acceptance is based on the current code and fresh verification results only.

## Component Map

- `src/pages/CalendarPage.vue`: route-level composition surface for the calendar feature.
- `src/components/calendar/CalendarShell.vue`: feature container that owns toolbar, status, and active view composition.
- `src/components/calendar/CalendarViewSwitch.vue`: segmented 4-button switch with typed props/emits.
- `src/components/calendar/CalendarGrid*.vue`: focused presentational views for day, 4-day, week, and month layouts.
- `src/composables/useCalendarState.ts`: reactive date/view state, navigation, and derived visible range.
- `src/services/google-calendar/*`: OAuth/session persistence, API client, cache persistence, and normalization utilities.
- `electron/main.ts` + `electron/preload.ts`: Electron-only OAuth flow, secure token persistence, and IPC bridge.

## Acceptance Criteria

- AC1. Navigation and route
  - The app exposes a dedicated calendar page reachable from the sidebar and router.
  - The page header shows the title and a 4-button segmented control on the same row, with options for day, 4 days, week, and month.
  - The active view is visually clear and switching views updates the rendered calendar without a full app reload.

- AC2. Calendar rendering
  - The calendar page renders four working views: day, 4 days, week, and month.
  - Each view supports navigating backward/forward and returning to today.
  - Scheduled Delphi tasks appear in the relevant day cells/columns.
  - Google Calendar events appear in the relevant day cells/columns after sync.

- AC3. Reliable Google OAuth for Electron
  - The app offers Google sign-in/sign-out in-app for the Electron runtime.
  - OAuth uses a validated state parameter and PKCE.
  - After first successful sign-in, the app can restore the Google session after a full restart without requiring the user to log in again, as long as the refresh token remains valid.

- AC4. Durable persistence and offline-first behavior
  - Google auth/session metadata is stored durably in app-owned storage, not only in memory.
  - Google calendar/event data is cached durably in app-owned storage.
  - On app restart, the calendar page can render the last cached Google data before or without a successful live refresh.
  - A failed network refresh does not wipe previously cached calendar data.

- AC5. Reliability and failure handling
  - Expired access tokens are refreshed automatically when possible.
  - Invalid or revoked credentials transition to a disconnected state without crashing the app.
  - The UI surfaces connection/sync state and last sync error/message at a basic level.
  - Renderer code does not persist Google secrets/tokens in browser `localStorage`.

- AC6. Verification coverage
  - TypeScript typecheck passes for `apps/delphi/ts`.
  - New or updated automated tests cover the main calendar derivation logic and at least one persistence/auth-related behavior.

## Non-goals

- Editing, creating, or deleting Google Calendar events from Delphi
- Multi-account Google support
- Full CalDAV or non-Google providers
- Cross-device sync of Google OAuth credentials through Ark

## Verification Plan

1. Run `cd apps/delphi/ts && npx tsc --noEmit`.
2. Run targeted tests for new calendar/auth/cache logic.
3. Inspect the calendar route, segmented switch, and settings/connect UI in code.
4. Verify persisted-storage paths and token handling logic in Electron main/preload code.
5. Record evidence in `.agent/tasks/2026-04-11-delphi-google-calendar/`.
