# Evidence

## Verification Summary

- `bun run typecheck` -> PASS
- `bun run test` -> PASS

## Acceptance Criteria

### AC1

Status: PASS

Sidebar now renders visible top controls with a search trigger on both desktop and mobile, even when the sidebar toggle button is disabled.

Evidence:
- `apps/arrancador/src/components/Sidebar.tsx`
- `apps/arrancador/src/index.css`
- `apps/arrancador/src/test/sidebar-component.test.tsx`

### AC2

Status: PASS

The titlebar no longer renders quick search. Search is initiated from the sidebar instead, and the mobile top bar no longer carries a search trigger.

Evidence:
- `apps/arrancador/src/components/AppTitlebar.tsx`
- `apps/arrancador/src/pages/Layout.tsx`
- `apps/arrancador/src/test/app-titlebar.test.tsx`

### AC3

Status: PASS

The updated shell contract is covered by tests and passes a fresh verification run.

Evidence:
- `.agent/tasks/2026-04-18-arrancador-sidebar-search-fix/raw/typecheck.log`
- `.agent/tasks/2026-04-18-arrancador-sidebar-search-fix/raw/test.log`

## Notes

- `bun run test` prints provider-guard stack traces for intentional negative-path tests. The suite still exits successfully with `31 passed` files and `132 passed | 2 expected fail` tests.
