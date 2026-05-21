# Evidence: Kosmos visuals titlebar chrome

## Summary

- Added a reusable shared titlebar in `packages/kosmos-visuals/components/Titlebar.vue`.
- Added a reusable shared desktop chrome layout in `packages/kosmos-visuals/components/DesktopChrome.vue`.
- Extended `Sidebar.vue` with `reserveTopInset` so the shared sidebar can sit below a separate titlebar without keeping the old macOS top safe area.
- Migrated `apps/dashboard` to the shared desktop chrome, moving top chrome actions into the titlebar and keeping the sidebar fixed beneath it.
- Updated dashboard Electron window configuration so macOS uses native traffic lights and Windows uses native overlay controls.
- Removed chrome borders from the titlebar and sidebar; the content pane now owns the visible separation borders.

## Acceptance Criteria

### AC1

Status: PASS

Evidence:

- `packages/kosmos-visuals/components/Titlebar.vue` exports an OS-aware titlebar.
- macOS layout uses `padding-left: var(--kosmos-mac-traffic-light-left-safe-area, 92px)`.
- Windows reserves right safe area with `--kosmos-windows-controls-safe-area` for native overlay controls instead of drawing custom buttons.
- Titlebar background now uses `var(--sidebar-bg)` so it visually merges with the sidebar.

### AC2

Status: PASS

Evidence:

- `packages/kosmos-visuals/components/DesktopChrome.vue` composes the titlebar above a `body` row that contains optional `sidebar` and main content slots.
- Sidebar/content layout now lives under the titlebar, so the sidebar no longer needs to bleed into the titlebar area.
- `packages/kosmos-visuals/components/Sidebar.vue` gained `reserveTopInset` and conditional top-bar rendering to support being mounted below an external titlebar.
- `packages/kosmos-visuals/components/sidebar.css` no longer draws the sidebar border; `DesktopChrome.vue` now places borders on the content pane (`border-top` and `border-left`).

### AC3

Status: PASS

Evidence:

- `apps/dashboard/src/components/dashboard/DashboardShell.vue` now uses `DesktopChrome`.
- Sidebar toggle, DB chooser, refresh, and reset actions moved into `#titlebar-leading`.
- `KosmosSidebar` now renders with `:show-toggle="false"` and `:reserve-top-inset="false"`.
- Main content remains the only scrollable area via `dashboard-shell__content { overflow: auto; }`.

### AC4

Status: PASS

Evidence:

- `apps/dashboard/electron/main.ts` uses macOS `titleBarStyle: "hiddenInset"` with native `trafficLightPosition`.
- `apps/dashboard/electron/main.ts` uses Windows `titleBarOverlay` with native overlay controls aligned to the shared titlebar height.
- `apps/dashboard/src/components/dashboard/DashboardShell.vue` no longer renders or wires custom window buttons; shared chrome now assumes OS-native controls.

### AC5

Status: PASS

Evidence:

- `Set-Location apps/dashboard; bun run typecheck` passed.
- `Set-Location apps/dashboard; bun x vite build --configLoader native` passed for renderer, Electron main, and preload bundles.
- `bun run build` remains blocked in this sandbox only because `electron-rebuild` fails with `spawn EPERM`; this is an environment restriction around native rebuild spawning, not a code failure in the migrated chrome/titlebar implementation.
