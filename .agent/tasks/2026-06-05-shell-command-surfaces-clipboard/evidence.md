# Evidence

## Implementation

- PASS: `kepler:clipboard-history` now opens as `LauncherView` mode via `kepler:clipboard-history:open-shell`; standalone Clipboard `BrowserWindow` and `#clipboard-history` route were removed.
- PASS: `kepler:focus-session` now opens as `LauncherView` mode via `kepler:focus-session:open-shell`; built-in focus-session no longer imports or opens Raycast host.
- PASS: Shell header/back model is shared by Clipboard and Focus command pages. Focus hides the search input; Clipboard keeps filter input/actions.
- PASS: Clipboard history persists to instance-scoped `clipboard-history.json`, with settings for retention days and max storage bytes. Default retention is 30 days.
- PASS: Settings has a dedicated "Буфер обмена" page with retention controls and current storage stats.

## Regression Checks

- PASS: `bun test tests/unit/clipboard-history-store.test.ts` — 11/11.
- PASS: `bun run shell:typecheck`.
- PASS: `bun run lint`.
- PASS: `bun run --cwd shell build:js:shell`.
- PASS: `bun run docs:sync`.
- PASS: `bun run docs:check`.
- PASS: `bun run ark:guard:writes`.
- PASS: `bun run ark:smoke` — first run timed out after 184s; rerun with a 420s timeout passed.

Some verification commands initially failed with `windows sandbox: setup refresh failed with status exit code: 1`; each was rerun once with the same command and narrow escalation per `windows-sandbox`.

## Visual Verify

Captured with `.tmp/visual/2026-06-05-shell-command-surfaces-clipboard/capture.mjs` against rebuilt `shell/dist`.

- PASS: `.tmp/visual/2026-06-05-shell-command-surfaces-clipboard/shell-clipboard-720x460.png` — Clipboard renders inside the same Shell dimensions with back button, filter header, list/detail content, and bottom actions.
- PASS: `.tmp/visual/2026-06-05-shell-command-surfaces-clipboard/shell-focus-720x460.png` — Focus renders inside the same Shell dimensions with back button, no search input, form/status content, visible blocklist row, and footer actions without overlap.
- PASS: `.tmp/visual/2026-06-05-shell-command-surfaces-clipboard/shell-focus-idle-720x460.png` — empty Focus state has no timer on the right side, keeps the idle prompt centered, and uses one goal field with task mention hint plus duration dropdown.
- PASS: `.tmp/visual/2026-06-05-shell-command-surfaces-clipboard/settings-clipboard-880x560.png` — Settings shows the dedicated Clipboard retention page.

## Follow-Up Fix: Focus Empty Start + Horologion-Style Controls

- PASS: `FocusCommandPanel` no longer sends Vue reactive arrays directly through Electron IPC; `buildFocusSessionStartInput(...)` returns a structured-clone-safe DTO.
- PASS: Focus goal/task input is unified: typing `@` opens Delphi task suggestions, and selected task is sent as `taskId` / `taskTitle`.
- PASS: `focus-session.ts::listTasks` hides completed/canceled/trashed Delphi tasks from mention suggestions.
- PASS: duration is selected by dropdown with 25/45/60/90 minute presets and a custom minute input.
- PASS: active Shell Focus panel exposes pause/resume, skip, and stop controls, matching widget-level pomodoro controls.
- PASS: right-side idle panel no longer shows timer/total duration and is visually centered.
- PASS: `bun test tests/unit/focus-command-payload.test.ts`.
