# Evidence: Shared status dot for dashboard

## Summary

- Added reusable `StatusDot` to `@kosmos/visuals`.
- Dashboard now uses the shared status dot with a click-open popover instead of a local plain circle.
- Windows titlebar safe area was increased so the dot sits further left and no longer crowds native controls.
- Success tone was darkened to a calmer green closer to the intended chrome palette.

## Acceptance Criteria

### AC1

Status: PASS

Evidence:

- `packages/kosmos-visuals/components/StatusDot.vue` exports a reusable tone-based status dot.
- It has icon-like hover background treatment.
- It opens a click-popover and closes on outside click or `Escape`.

### AC2

Status: PASS

Evidence:

- `apps/dashboard/src/components/dashboard/DashboardShell.vue` now imports and renders `StatusDot`.
- Dashboard passes a custom popover body with Ark DB status copy and current DB path.

### AC3

Status: PASS

Evidence:

- `packages/kosmos-visuals/components/StatusDot.vue` uses darker success tone `#4f9a75`.
- `packages/kosmos-visuals/components/Titlebar.vue` now reserves more Windows safe area (`156px`) so trailing status UI stays left of native controls.
- Dashboard no longer relies on a local raw status-circle style.

### AC4

Status: PASS

Evidence:

- `Set-Location apps/dashboard; bun run typecheck` passed.
- Direct Vite build passed via Node invocation of the installed Vite CLI:
  - `Push-Location apps/dashboard; node D:\Personal\Hobby\Coding\kosmos\node_modules\.bun\vite@8.0.8+cac867131b98ca75\node_modules\vite\bin\vite.js build --configLoader native; Pop-Location`
- This verifies renderer, Electron main, and preload bundles after the shared status-dot refactor.
