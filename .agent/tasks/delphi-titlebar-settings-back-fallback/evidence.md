# Verification Evidence

## Scope

Task: `delphi-titlebar-settings-back-fallback`

## Acceptance Criteria

- AC1: PASS
  `apps/delphi/ts/src/App.vue:221` defines `canExitSettingsViaBack`, and `apps/delphi/ts/src/App.vue:223` enables titlebar back on `/settings` even when the Electron back stack is empty.
- AC2: PASS
  `apps/delphi/ts/src/App.vue:233` adds a `/settings` fallback inside `navigateBack()` that routes to `/` when there is no stored history target.
- AC3: PASS
  The existing stack-driven back/forward behavior remains intact outside settings; the new fallback is only used when `canExitSettingsViaBack` is true in `apps/delphi/ts/src/App.vue:237` and `:256`.
- AC4: PASS
  `bunx tsc --noEmit` succeeded in `apps/delphi/ts` on April 17, 2026.

## Commands

```powershell
bunx tsc --noEmit
```

## Notes

- Verification here is code-level plus typecheck. I did not run an interactive Electron UI session inside this sandbox.
