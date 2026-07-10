# Focus session feedback and state-aware actions

## Problem

Focus feedback and controls are inconsistent across the widget, Shell launcher,
and Focus UI. The widget completion button bypasses the focus-session lifecycle,
blocked feedback uses the work/red accent, and an active session does not expose
the same current-state actions everywhere.

## Component map

- `platform/desktop/src/views/FocusWidgetView.vue` — renders the compact widget
  and sends user intent through the preload API.
- `platform/desktop/electron/focus-widget-ipc.ts` — maps widget controls/menu
  entries to the canonical focus-session lifecycle.
- `platform/desktop/electron/focus-session.ts` — owns pause/resume/skip/stop/
  complete side effects and broadcasts the resulting snapshot.
- `platform/desktop/electron/focus-overlay.ts` and
  `platform/desktop/src/views/FocusBlockOverlay.vue` — reuse one transparent,
  topmost surface for blocked and completed feedback.
- `platform/desktop/src/lib/focusLauncherCommands.ts` and `LauncherView.vue` —
  derive launcher commands from the current focus snapshot.
- Focus/settings views — expose the same actions without duplicating lifecycle
  logic.

## Acceptance criteria

1. Blocked-app feedback uses a yellow accent on the edge glow and popup.
2. Completing a focused task runs the canonical completion lifecycle and shows
   green edge feedback plus a top popup saying that the task is completed.
3. An active session can be explicitly cancelled without marking its task done.
4. While a session is active, Shell does not show `Начать фокус`; it shows
   pause/resume, skip, complete, cancel, and edit actions for the current state.
5. Focus/settings UI exposes the same state-aware actions, with Russian labels.
6. All overlay pixels outside the intentional glow/popup remain transparent.
7. Focus lifecycle/command tests and desktop type checks pass; the blocked,
   completed, and active-command states are visually verified where possible.

## Non-goals

- No new focus state store or command framework.
- No changes to ARK schemas or sync boundaries.
- No unrelated Eden changes.

## Proof

- Focused unit tests for command derivation and lifecycle routing.
- Desktop TypeScript/Vue checks.
- Runtime screenshots for blocked, completed, and active-session command states.
