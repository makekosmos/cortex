# Evidence

## Verification Summary

- `bun run typecheck` -> PASS
- `bun run test` -> PASS

## Acceptance Criteria

### AC1

Status: PASS

Titlebar back/forward now follows real browser/router history semantics via the current history index, and `/settings` no longer fakes a back path with a hardcoded fallback.

Evidence:

- `apps/arrancador/src/pages/Layout.tsx`
- `apps/arrancador/src/test/layout.test.tsx`

### AC2

Status: PASS

The mobile sidebar now uses a real sheet/dialog interaction, hidden menu content is removed from the focus path when closed, and the sheet keeps an in-panel dismiss control.

Evidence:

- `apps/arrancador/src/pages/Layout.tsx`
- `apps/arrancador/src/components/ui/sidebar.tsx`
- `apps/arrancador/src/test/layout.test.tsx`
- `apps/arrancador/src/test/ui-sidebar.test.tsx`

### AC3

Status: PASS

Search remains reachable when the desktop sidebar is hidden, and spotlight resets its query/selection on dismiss-reopen and route changes.

Evidence:

- `apps/arrancador/src/components/Spotlight.tsx`
- `apps/arrancador/src/components/AppTitlebar.tsx`
- `apps/arrancador/src/components/Sidebar.tsx`
- `apps/arrancador/src/pages/Layout.tsx`
- `apps/arrancador/src/test/spotlight.test.tsx`
- `apps/arrancador/src/test/layout.test.tsx`

### AC4

Status: PASS

Windows frameless mode keeps usable fallback window controls, overlay symbol colors stay in sync with the native theme, and desktop sidebar resize/collapse interactions remain reachable.

Evidence:

- `apps/arrancador/electron/main/windows.ts`
- `apps/arrancador/src/components/AppTitlebar.tsx`
- `apps/arrancador/src/components/Sidebar.tsx`
- `apps/arrancador/src/index.css`
- `apps/arrancador/src/test/app-titlebar.test.tsx`
- `apps/arrancador/src/test/sidebar-component.test.tsx`

### AC5

Status: PASS

Regression coverage now exercises the shell, sidebar, spotlight, sheet, and native rebuild paths closely enough to catch the reviewed breakages, and fresh verification passes on the current codebase.

Evidence:

- `apps/arrancador/src/test/layout.test.tsx`
- `apps/arrancador/src/test/app-titlebar.test.tsx`
- `apps/arrancador/src/test/spotlight.test.tsx`
- `apps/arrancador/src/test/sidebar-component.test.tsx`
- `apps/arrancador/src/test/ui-sidebar.test.tsx`
- `apps/arrancador/src/test/ui-primitives.test.tsx`
- `apps/arrancador/src/test/rebuild-native-if-needed.test.ts`
- `apps/arrancador/package.json`
- `apps/arrancador/vitest.config.mjs`
- `.agent/tasks/2026-04-19-arrancador-review-fixpack/raw/typecheck.log`
- `.agent/tasks/2026-04-19-arrancador-review-fixpack/raw/test.log`

## Notes

- `bun run test` now uses a dedicated `vitest.config.mjs` with `pool: "threads"` and native config loading so the suite runs reliably in the current Windows+Bun sandbox, where Vitest fork workers hit `spawn EPERM`.
- `bun run test` still prints expected provider-guard stack traces for intentional negative-path tests while exiting successfully.
