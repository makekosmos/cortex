# Evidence: Dashboard status dot and shared content surface

## Summary
- Dashboard status chip in the titlebar was reduced to a minimal Delphi-style dot.
- A reusable `DesktopContentSurface` component was added to `@kepler/visuals` for desktop content padding, border, and left-corner radius.
- Dashboard now consumes the shared content surface instead of owning those shell settings locally.

## Acceptance Criteria

### AC1
Status: PASS

Evidence:
- `apps/dashboard/src/components/dashboard/DashboardShell.vue` now renders `dashboard-shell__status-dot-button` instead of the previous text pill.
- The dot uses the same state-color pattern as Delphi’s connection dot: green/gold/red by state, without the large badge body.

### AC2
Status: PASS

Evidence:
- `packages/kepler-visuals/components/DesktopContentSurface.vue` was added.
- It owns reusable desktop-shell props for `paddingTop`, `paddingInline`, `paddingBottom`, `radiusTopLeft`, and `radiusBottomLeft`.
- It is exported from `packages/kepler-visuals/components/index.ts` and `packages/kepler-visuals/index.ts`.

### AC3
Status: PASS

Evidence:
- `apps/dashboard/src/components/dashboard/DashboardShell.vue` wraps its main content in `DesktopContentSurface`.
- `DesktopChrome.vue` no longer owns the content border/radius/background contract directly.
- Dashboard shell keeps only layout-specific flex/gap rules, while shared visuals own the desktop content surface styling.

### AC4
Status: PASS

Evidence:
- `Set-Location apps/dashboard; bun run typecheck` passed.
- `Set-Location apps/dashboard; bun x vite build --configLoader native` passed for renderer, Electron main, and preload bundles.
