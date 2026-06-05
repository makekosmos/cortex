# Evidence — Focus launcher commands

Verified at: 2026-06-05T21:00:00Z (Europe/Moscow local evening).

## AC1 — Idle command list

Verdict: PASS.

Evidence:

- `bun test tests/unit/focus-launcher-commands.test.ts` passes
  `idle: keeps «Начать фокус», drops raw command-bus focus actions`.
- Visual screenshot: `.tmp/visual/2026-06-05-focus-launcher-commands/launcher-idle.png`.
- Screenshot text contained «Начать фокус» and did not contain raw
  `kepler:focus-pause/resume/complete` titles.

## AC2 — Active running command list

Verdict: PASS.

Evidence:

- `bun test tests/unit/focus-launcher-commands.test.ts` passes
  `active running: replaces «Начать фокус» with widget-parity commands`.
- Visual screenshot: `.tmp/visual/2026-06-05-focus-launcher-commands/launcher-active-running.png`.
- Screenshot text contained «Приостановить фокус», «Отметить задачу выполненной»,
  «Завершить фокус», «Редактировать фокус», and did not contain «Начать фокус».

## AC3 — Active paused command list

Verdict: PASS.

Evidence:

- `bun test tests/unit/focus-launcher-commands.test.ts` passes
  `active paused: pause toggle reads «Продолжить фокус»`.
- Visual screenshot: `.tmp/visual/2026-06-05-focus-launcher-commands/launcher-active-paused.png`.
- Screenshot text contained «Продолжить фокус».

## AC4 — Edit mode footer copy

Verdict: PASS.

Evidence:

- `shell/src/views/LauncherView.vue::enterFocusMode(true)` sets `focusEditMode`.
- Focus-mode footer renders `{{ focusEditMode ? "Продолжить" : "Начать фокус" }}`.
- Covered by `bun run shell:typecheck`.

## AC5 — Done command write path

Verdict: PASS.

Evidence:

- `shell/electron/focus-session.ts::completeFocusSession` calls `markTaskDone`.
- `markTaskDone` uses `invoke("upsert_object", { object: record })` for `task_obj`.
- `bun run ark:guard:writes` passed.

## AC6 — State updates from widget/backend

Verdict: PASS.

Evidence:

- `LauncherView.vue` subscribes to `window.kepler.focusSession.onUpdated` and refreshes
  `focusSnapshot`.
- `focus-session.ts` broadcasts `kepler:focus-session:updated` after
  start/pause/resume/skip/stop/complete and backend phase changes.
- Covered by `bun run shell:typecheck` and visual mocked snapshots.

## AC7 — Launcher stays open for session controls

Verdict: PASS.

Evidence:

- `LauncherView.vue::invokeSelected` handles synthetic focus command ids and returns without
  calling `window.kepler.window.hide()`.
- `FOCUS_EDIT_ID` routes to `enterFocusMode(true)`.
- Covered by `bun run shell:typecheck`.

## Commands

Initial verification attempts failed with
`windows sandbox: setup refresh failed with status exit code: 1`. Per `windows-sandbox`
recovery, the same commands were rerun with scoped escalation.

- `bun test tests/unit/focus-launcher-commands.test.ts` — PASS, 4 tests / 7 expects.
- `bun run shell:typecheck` — PASS.
- `bun run ark:guard:writes` — PASS.
- `bun run --cwd shell build:js:shell` — PASS, used to build stable renderer for visual
  verification.
- `node .tmp/visual/2026-06-05-focus-launcher-commands/capture-focus-launcher.mjs` — PASS,
  produced three screenshots.

## Not Covered

- Real Electron end-to-end session with live backend/widget click flow was not run in this turn;
  the visible command states were verified with a mocked preload against the built renderer, and
  lifecycle write boundary was verified by guard/typecheck.
