# Focus Mode Unification

Started: 2026-06-05
Class: FULL_LOOP

## Context

User asked to remove Horologion as a separate Kosmos application/window and fold the Focus/Pomodoro workflow into shell-owned Focus Mode, inspired by Raycast Focus.

External reference checked on 2026-06-05:

- Raycast Focus feature page: focus sessions start from a form with goal, duration, and apps/websites to block; a floating Focus Bar tracks remaining time and exposes pause/complete/manage controls.
- Raycast Focus manual: commands include Start Focus Session, Toggle Focus Session, Create/Search/Import Focus Categories, in-session Edit/Pause/Complete/Snooze, and optional menu bar/floating bar surfaces.

## Scope

In scope:

- Remove Horologion from the active extension/app surface so it no longer appears as a separate app/window or launcher command.
- Preserve Horologion source as a downloadable local archive artifact before removal.
- Add shell-owned Focus Session UI that lets the user choose duration, write a focus goal, optionally pick/link a Delphi task, and choose an existing Focus blocklist.
- Use existing backend `pomodoro.*` lifecycle as the timer source of truth.
- Use existing `focus.*` backend state and shell `applyFocusBlock` path for domain blocking.
- Keep the floating focus widget functional; its edit/open action should open shell Focus Session instead of Horologion.
- Preserve basic time tracking for focus sessions by creating/stopping `time_entry_obj` through ARK-safe APIs from shell.
- Update launcher/internal commands to expose Focus Mode commands instead of Horologion extension commands.
- Update tests/docs references that would otherwise assert Horologion as an active app.

Out of scope:

- Remote GitHub repository creation/push for the archived Horologion source.
- App-level/process blocking and browser tab restore/snooze parity beyond existing hosts-based domain blocking.
- Full Horologion historical list/edit UI parity.
- Changing backend pomodoro protocol shape unless existing shell APIs cannot support the workflow safely.
- Bump/release.

## Acceptance Criteria

**AC1.** Horologion is not an active Kosmos extension/app: no launcher open command, no auto-open as a separate BrowserWindow, and build/discovery no longer treats `extensions/horologion` as an active extension.

**AC2.** Current Horologion source is preserved before removal as a local downloadable archive artifact documented in evidence.

**AC3.** Shell exposes a Focus Session view/command that supports Raycast-like start flow: duration selection, goal text, optional Delphi task selection or freeform goal, and blocklist selection.

**AC4.** Starting a Focus Session calls backend `pomodoro.start` and, when a blocklist is selected, activates `focus.set_active_state` through the existing shell/backend path. Stopping/completing deactivates the blocklist and stops the timer.

**AC5.** Focus widget remains the visible running-session surface and its click/edit/menu actions open shell Focus Session, not Horologion.

**AC6.** Focus sessions create/close ARK `time_entry_obj` records via safe `ArkClient` operations, preserving at least title, task id/title if selected, `startedAt`, `endedAt`, and `source: "pomodoro"`.

**AC7.** No Focus Mode forbidden rules are violated: no direct hosts writes outside helper/service, no direct pomodoro state mutation outside `pomodoro.*`, no direct focus widget BrowserWindow manipulation outside `focus-widget.ts`, and no raw SQL writes in app/shell code.

**AC8.** Relevant tests/typecheck/lint/guards pass: shell typecheck, unit/e2e tests updated for the new Focus shell workflow, `bun run ark:guard:writes`, `bun run ark:smoke`, and visual verification screenshots for the Focus Session view and running widget state.
