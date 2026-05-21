# Evidence: Dashboard scroll and hidden sidebar gap

## Summary

- Restored dashboard content scrolling by removing the conflicting fixed-height rule from the main flex container.
- Removed the leftover hidden sidebar gap by setting dashboard sidebar `hiddenWidth` to `0`.

## Acceptance Criteria

### AC1

Status: PASS

Evidence:

- `apps/dashboard/src/components/dashboard/DashboardShell.vue` no longer applies `height: 100%` to `dashboard-shell__main`.
- The main shell now relies on `flex: 1` and `min-height: 0`, allowing the nested `dashboard-shell__content` scroller to own overflow again.

### AC2

Status: PASS

Evidence:

- `apps/dashboard/src/components/dashboard/DashboardShell.vue` now passes `:hidden-width="0"` to `KosmosSidebar`.
- Hiding the sidebar no longer reserves a collapsed 80px strip.

### AC3

Status: PASS

Evidence:

- `Set-Location apps/dashboard; bun run typecheck` passed.
- Direct Vite build passed via Node invocation of the installed Vite CLI:
  - `Push-Location apps/dashboard; node D:\Personal\Hobby\Coding\kosmos\node_modules\.bun\vite@8.0.8+cac867131b98ca75\node_modules\vite\bin\vite.js build --configLoader native; Pop-Location`
