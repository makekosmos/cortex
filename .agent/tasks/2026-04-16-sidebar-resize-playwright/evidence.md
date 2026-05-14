# Evidence

- AC1: `packages/kosmos-visuals/components/DesktopChrome.vue` now uses `overflow: visible` for `kosmos-desktop-chrome__sidebar`, so the shared resize handle is no longer clipped by the sidebar slot container.
- AC2: `packages/kosmos-visuals/components/Sidebar.vue` exposes stable selectors:
  - `data-testid="kosmos-sidebar"`
  - `data-testid="kosmos-sidebar-resize-handle"`
- AC3: `apps/dashboard/e2e/dashboard.spec.ts` now contains `sidebar resize handle changes dashboard sidebar width`, and `apps/dashboard/scripts/runE2ESmoke.ts` contains a direct smoke path for the same drag-resize flow.
- AC4: Not verified in this sandbox because both Playwright CLI worker startup and direct Electron launch are blocked by `spawn EPERM`, which is external to the test assertions.
