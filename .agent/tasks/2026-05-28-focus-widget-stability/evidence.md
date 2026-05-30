# Evidence — 2026-05-28-focus-widget-stability

Verified at: 2026-05-28T21:52:44+03:00

## AC1 — PASS

Backend/main-process update no longer overwrites a concrete label with generic `Фокус`/`Перерыв` in the same active mode.

Evidence:

- Code: `shell/electron/focus-widget.ts::setFocusState` preserves the previous concrete label when the incoming patch only carries a default focus label.
- Regression: `tests/e2e/focus-widget-controls.spec.ts` test `focus widget: backend generic label не перетирает конкретное название`.
- Command: `bun run test:e2e tests/e2e/focus-widget-controls.spec.ts`
- Result: `7 passed`.

## AC2 — PASS

Baseline focus widget shows only countdown and label; action buttons are visually hidden and removed from pointer hit-testing until hover/focus. On hover/focus controls appear in the same content area instead of to the right. The hover target is explicitly no-drag, with dragging moved to a six-dot handle. The visual mode indicator is an internal progress fill instead of a left stripe.

Evidence:

- Code: `shell/src/views/FocusWidgetView.vue` places controls in `.actions` overlay with `opacity: 0`, `pointer-events: none`, and reveals it on `.widget:hover` / `.widget:focus-within`; `.content` is `no-drag`, `.drag-handle` is the only `drag` surface.
- Code: `shell/electron/focus-widget.ts` and `extensions/horologion/src/lib/usePomodoroSession.ts` now pass `totalSec` so the widget can render fill progress from elapsed/total phase time.
- Regression: `focus widget: controls появляются только на hover/focus (pomodoro mode)` asserts `.actions` opacity `0` before hover and `1` after hover, plus the no-drag/drag split.
- Command: `bun run test:e2e tests/e2e/focus-widget-controls.spec.ts`
- Result: `7 passed`.

## AC3 — PASS

Inline controls remain accessible through hover/focus and retain existing actions/aria labels for pomodoro and stopwatch modes.

Evidence:

- Code: `FocusWidgetView.vue` still uses `IconButton` from `@kosmos/visuals` with `aria-label` for pause/resume, skip, stop, and close.
- Regression: existing pause, skip, stop, and stopwatch-no-skip tests now hover the widget before clicking and pass.
- Command: `bun run test:e2e tests/e2e/focus-widget-controls.spec.ts`
- Result: `7 passed`.

## AC4 — PASS

Existing focus-widget checks pass in headless mode on isolated test data dirs.

Evidence:

- Command: `bun run test:e2e tests/e2e/focus-widget-controls.spec.ts`
- Result: `7 passed`.
- Supporting checks:
  - `bun run --cwd shell typecheck` — PASS.
  - `bun run format:check shell/electron/focus-widget.ts shell/src/views/FocusWidgetView.vue shell/electron/preload.ts shell/electron/extension-preload.ts shell/shared/ipc-types.ts extensions/horologion/src/lib/usePomodoroSession.ts extensions/horologion/src/global.d.ts tests/e2e/helpers/horologion.ts tests/e2e/focus-widget-controls.spec.ts` — PASS after `totalSec`/progress update.
  - `bun run format:check shell/src/views/FocusWidgetView.vue tests/e2e/focus-widget-controls.spec.ts docs-site/agents/postmortems.md` — PASS after the drag-region fix.
  - `bun run format:check shell/electron/focus-widget.ts shell/src/views/FocusWidgetView.vue tests/e2e/focus-widget-controls.spec.ts docs-site/agents/postmortems.md .agent/tasks/2026-05-28-focus-widget-stability/spec.md` — PASS before the follow-up.
  - `bun run docs:check` — PASS.
  - `bun run ark:smoke` — PASS on retry with longer timeout. First run timed out at 244s without assertion output; second run completed in 57.9s with `ARK smoke matrix passed`.
