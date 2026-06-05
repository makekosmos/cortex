# Evidence — Focus Mode unification

## Research

- Raycast Focus core page: start session selects duration and distracting apps/websites to block; goal-based sessions; floating focus bar; edit/pause/snooze behavior.
- Raycast v1.89.0 changelog: "Start Focus Session" command starts with goal, duration, apps/websites to block.
- Raycast v1.92.0 changelog: custom focus categories and extra session/status-bar actions.

## Implementation

- Archived current Horologion source to `.agent/tasks/2026-06-05-focus-mode-unification/archive/horologion-extension-source-2026-06-05.zip`.
- Disabled Horologion as active extension/workspace package by moving:
  - `extensions/horologion/manifest.json` → `extensions/horologion/manifest.archived.json`
  - `extensions/horologion/package.json` → `extensions/horologion/package.archived.json`
- Added shell-owned Focus Session orchestration in `shell/electron/focus-session.ts`.
- Reused the existing Raycast-compatible host instead of a bespoke Focus renderer:
  - `shell/electron/raycast/view-host.ts` now accepts built-in Raycast element views.
  - Focus opens through neutral `#command-host` route backed by `shell/src/views/RaycastHostView.vue` + `shell/src/raycast-host/RaycastFormView.vue`.
  - Removed the temporary `#focus-session` route and `window.kepler.focusSession.*` renderer API.
  - Launcher invocation passes the current launcher `BrowserWindow` as `hostWindow`, so Focus renders inside the same shell window; focus-widget/legacy IPC can still fall back to a separate host window.
  - `shell/src/main.ts` hash routing is reactive now, so switching an existing launcher window from launcher hash to `#command-host` re-renders the root component instead of leaving the old launcher view mounted.
- Added launcher command `kepler:focus-session`.
- Focus widget edit/open legacy action now opens shell Focus Session.
- Focus Session starts/stops backend `pomodoro.*`, applies selected `focus.set_active_state` through shell `applyFocusBlock`, and creates/closes `time_entry_obj` via `ArkClient.invokeOperation("upsert_object")`.
- Docs updated and regenerated through `bun run docs:sync`.

## Verification

- PASS `bun run shell:typecheck`
- PASS `bun run lint`
- PASS `bun run ark:guard:writes`
- PASS `bun run docs:check`
- PASS `bun run ark:smoke`
- PASS visual verify screenshots:
  - `.tmp/visual/2026-06-05-focus-mode-unification/focus-raycast-host-desktop.png`
  - `.tmp/visual/2026-06-05-focus-mode-unification/focus-raycast-host-mobile.png`
  - `.tmp/visual/2026-06-05-focus-mode-unification/focus-command-host-hashchange.png`
- PASS visual check:
  - Raycast host rendered non-empty Focus form.
  - Desktop and narrow mobile viewports show Russian labels, form fields, tag picker, and action footer without overlap.
  - Existing shell page switches to `#command-host` and renders the Focus form after `hashchange`.

## Notes

- Remote archived GitHub repository creation/push was out of scope per spec.
- Raycast app-level blocking, per-app snooze, browser tab restore, and Apple Focus integration are not implemented; current Kosmos Focus Mode remains domain blocklist + pomodoro + time tracking.
