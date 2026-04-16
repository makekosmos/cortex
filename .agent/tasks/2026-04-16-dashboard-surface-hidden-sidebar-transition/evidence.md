# Evidence: Dashboard surface transition on hidden sidebar

## Summary
- `DesktopContentSurface` now supports hiding its left divider via a prop.
- Dashboard binds the shared surface to sidebar hidden state so the left divider disappears and the top-left radius animates to `0` when the sidebar is hidden.

## Acceptance Criteria

### AC1
Status: PASS

Evidence:
- `packages/kepler-visuals/components/DesktopContentSurface.vue` now accepts `showLeftBorder`.
- The component drives left-divider color through `--kepler-content-border-left-color`.
- The shared surface now animates border-left color and left-corner radius values.

### AC2
Status: PASS

Evidence:
- `apps/dashboard/src/components/dashboard/DashboardShell.vue` passes `:show-left-border="!sidebarConfig.hidden"`.
- It also passes `:radius-top-left="sidebarConfig.hidden ? '0px' : '16px'"`.
- This ties the content-shell chrome directly to sidebar hidden state.

### AC3
Status: PASS

Evidence:
- `Set-Location apps/dashboard; bun run typecheck` passed.
