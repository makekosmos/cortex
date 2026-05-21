# Evidence

## Verification Summary

- `bun run typecheck` -> PASS
- `bun run test` -> PASS
- `bun x playwright screenshot ...` -> BLOCKED (`spawn EPERM` while launching Playwright browser)

## Acceptance Criteria

### AC1

Status: FAIL

Playwright CLI was invoked, but runtime capture was blocked by the environment before the browser could launch.

Evidence:

- `.agent/tasks/2026-04-19-arrancador-playwright-dom-diagnosis/raw/playwright-cli-error.log`

### AC2

Status: PASS

The desktop shell no longer has the accidental resize-handle click path that could collapse the sidebar into an apparently empty state without clear intent.

Evidence:

- `apps/arrancador/src/components/Sidebar.tsx`
- `apps/arrancador/src/index.css`

### AC3

Status: PASS

Fresh typecheck and test runs pass after the shell/perf fixes.

Evidence:

- `.agent/tasks/2026-04-19-arrancador-playwright-dom-diagnosis/raw/typecheck.log`
- `.agent/tasks/2026-04-19-arrancador-playwright-dom-diagnosis/raw/test.log`

## Notes

- Route modules are now lazy-loaded in `src/router.tsx`, reducing eager renderer boot work in dev.
- `bun run test` prints expected provider-guard stack traces for intentional negative-path tests while still exiting successfully.
