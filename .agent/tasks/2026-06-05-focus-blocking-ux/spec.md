# 2026-06-05 — Focus blocking UX repair

## Context

The first app-blocking implementation used raw `app_index.list_all` entries as the Focus block picker model. The user expected to block the same app/game entries visible in the Shell launcher, not pick technical exe/process-like rows. The Focus command also has broken mention/chip ergonomics and the widget shows an unrequested protection icon.

## Scope

In scope:

- Treat launcher-visible app commands as the source of truth for Focus app blocking.
- Store blocked app metadata with id and display name, and block launcher app launches by exact id or normalized display name.
- Keep block UI as textarea with `@` mention, but render selected task/app tokens as removable pills.
- Fix task mention selection so clicking/keyboard selection consistently adds the task token while preserving free-form goal text.
- Fix mention popover shadow/background in dark UI.
- Remove the unrequested protection/shield affordance from the focus widget.
- Show a Shell blocked-app overlay when a blocked app launch is attempted.
- Add a 3-second hover hold before enabling a 5-minute snooze button for that blocked app.

Out of scope:

- Killing already-running processes.
- Blocking apps launched outside Kosmos.
- Windows service/helper process blocking.

## Acceptance Criteria

**AC1.** Focus "Блокировка" suggestions are launcher app entries with user-facing name/icon, not exe/path rows.

**AC2.** Starting Focus stores selected apps as metadata and launcher refuses a blocked app by id or normalized app name.

**AC3.** Goal field supports free text plus one selected task token; selecting/removing the token is reliable and does not leave the mention picker stuck.

**AC4.** Mention popovers have dark-theme-correct background/shadow and no white glow.

**AC5.** Focus widget no longer shows a protection/shield icon merely because blocking is enabled.

**AC6.** Blocked launcher attempt shows an overlay with the app name and a snooze control that becomes clickable only after hovering for 3 seconds; snooze permits that app for 5 minutes.

**AC7.** Verification includes unit/regression tests, typecheck/lint, visual screenshots for Focus form and blocked overlay, `ark:guard:writes`, `ark:smoke`, and docs sync/check.
