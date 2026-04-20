# Evidence

## Verification Summary

- `bun run typecheck` -> PASS
- `bun run test` -> PASS

## Acceptance Criteria

### AC1

Status: PASS

Desktop scroll is confined to inner app containers instead of leaking to the document/page level.

Evidence:
- `apps/arrancador/src/index.css`
- `apps/arrancador/src/pages/Layout.tsx`

### AC2

Status: PASS

Persisted sidebar width is sanitized on load, preventing the desktop sidebar from starting at effectively zero width and leaving its resize rail unreachable.

Evidence:
- `apps/arrancador/src/components/Sidebar.tsx`
- `apps/arrancador/src/pages/Layout.tsx`
- `apps/arrancador/src/test/sidebar-component.test.tsx`

### AC3

Status: PASS

Fresh verification passed and the sidebar width recovery path is covered by tests.

Evidence:
- `.agent/tasks/2026-04-19-arrancador-shell-scroll-sidebar-fix/raw/typecheck.log`
- `.agent/tasks/2026-04-19-arrancador-shell-scroll-sidebar-fix/raw/test.log`

## Notes

- `bun run test` prints expected provider-guard stack traces for intentional negative-path tests while still exiting successfully.
