# Task Spec — Kosmos settings open regression

## Task ID / Path

- `2026-06-15-shell-settings-open-broken`
- `.agent/tasks/2026-06-15-shell-settings-open-broken/spec.md`

## Original task statement

> Task: Создай frozen spec для бага: «При попытке открыть настройки сейчас — ничего не происходит». Область вероятно Kosmos desktop shell/settings UI/command bus. Нужно диагностировать и исправить так, чтобы открытие настроек из текущего UI снова работало. Следуй docs-site/concepts/proof-loop.md если нужно. Верни путь к spec.md, task id, acceptance criteria и suggested verification commands. Не меняй продуктовый код.

## Context

- In the shell, `settings:open` is a built-in command in `platform/desktop/electron/commands.ts` and is routed through `kepler:commands:invoke` in `platform/desktop/electron/main.ts`.
- The actual Settings window lives in `platform/desktop/electron/settings-window.ts` and is opened through the existing IPC bridge (`window.kepler.settings.open()` / `ipcMain.handle("kepler:settings:open")`).
- The bug is treated as a `FULL_LOOP` issue because it touches shell UI + command bus + window lifecycle, and the fix must preserve the current settings entry point rather than introducing a new one.

## Scope

In scope:

- Diagnose why the existing Settings entry point does not visibly open the window.
- Restore the existing settings-open path used by the current UI.
- Keep the current command id / IPC namespace / window role intact.

Out of scope:

- New settings pages, new routes, or redesign of the settings UI.
- Changes to ARK/data/sync/schema/write-boundary.
- Broad command bus redesign or brand/namespacing cleanup.
- Unrelated refactors in launcher, extensions, or app-specific settings screens.

## Assumptions

- “Current UI” means the existing shell-owned settings entry point(s) already wired to `settings:open` / `window.kepler.settings.open()`, not a new launcher flow.
- The minimal acceptable fix is the smallest layer that restores the existing user-visible behavior.
- If the bug is caused by a hidden lifecycle edge case, the fix must preserve repeated open/focus behavior for an already-created Settings window.

## Constraints and non-goals

- Do not change product code outside the minimum shell/settings-open path needed to fix the regression.
- Do not change the user-facing settings surface beyond restoring open behavior.
- Do not alter command ids, IPC channel names, or the shell’s settings window role unless strictly required for compatibility.
- Do not introduce ARK/data/sync writes or new architecture boundaries.

## Acceptance Criteria

**AC1.** Triggering the existing Settings action from the current UI opens the Kosmos Settings window again instead of doing nothing.

**AC2.** The existing command-bus path for settings remains functional: `settings:open` still resolves through the shell command invoke flow and reaches the settings window open logic.

**AC3.** The existing renderer IPC path remains functional: `window.kepler.settings.open()` opens or re-focuses the same Settings window without requiring a new route or a new UI entry point.

**AC4.** Repeated Settings opens are safe: invoking the action multiple times does not create duplicate visible Settings windows and does not regress headless/test-mode behavior.

**AC5.** The fix does not change unrelated shell commands, app-specific settings panes, or command bus semantics outside the Settings-open path.

## Verification plan

- Run the shell build/typecheck path for `platform/desktop`.
- Run the targeted settings/open window smoke or e2e coverage that exercises repeated open behavior.
- Manually smoke the current UI in desktop dev mode and confirm Settings visibly opens from the existing entry point.
