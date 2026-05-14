# Evidence: Dashboard scroll verified by Playwright

## Implementation
- Added a stable test hook and moved the dashboard scroll root to the shared content surface in [DashboardShell.vue](D:/Personal/Hobby/Coding/kosmos/apps/dashboard/src/components/dashboard/DashboardShell.vue:228) and [DesktopContentSurface.vue](D:/Personal/Hobby/Coding/kosmos/packages/kosmos-visuals/components/DesktopContentSurface.vue:4).
- Fixed the root layout bug in [DesktopChrome.vue](D:/Personal/Hobby/Coding/kosmos/packages/kosmos-visuals/components/DesktopChrome.vue): `.kosmos-desktop-chrome__content` is now a flex column, so `DesktopContentSurface` can actually receive bounded height and scroll.
- Added a dedicated Playwright scroll test in [dashboard.spec.ts](D:/Personal/Hobby/Coding/kosmos/apps/dashboard/e2e/dashboard.spec.ts:66).
- Switched Playwright DB seeding to Python stdlib SQLite in [playwright.globalSetup.ts](D:/Personal/Hobby/Coding/kosmos/apps/dashboard/playwright.globalSetup.ts:15) and [seedSmokeDb.py](D:/Personal/Hobby/Coding/kosmos/apps/dashboard/scripts/seedSmokeDb.py:1) so the CLI no longer depends on Node-side `better-sqlite3` ABI compatibility during setup.
- Updated dashboard local instructions in [AGENTS.md](D:/Personal/Hobby/Coding/kosmos/apps/dashboard/AGENTS.md:14).

## Acceptance Criteria
- AC1 `PASS`
  - `DashboardShell` now exposes `data-testid="dashboard-scroll-pane"` on the actual scroll root at [DashboardShell.vue](D:/Personal/Hobby/Coding/kosmos/apps/dashboard/src/components/dashboard/DashboardShell.vue:228).
- AC2 `FAIL`
  - The Playwright test exists and targets overflow plus `scrollTop` movement in [dashboard.spec.ts](D:/Personal/Hobby/Coding/kosmos/apps/dashboard/e2e/dashboard.spec.ts:66), but this environment blocks Playwright worker process startup with `spawn EPERM` before the test body runs.
- AC3 `FAIL`
  - `playwright test` does not complete in this environment for the same `spawn EPERM` sandbox restriction, despite successful global setup and seeded smoke DB.

## Verification
- `PASS` `node D:\Personal\Hobby\Coding\kosmos\node_modules\.bun\typescript@5.8.3\node_modules\typescript\bin\tsc --noEmit -p D:\Personal\Hobby\Coding\kosmos\apps\dashboard\tsconfig.json`
- `PASS` `Push-Location 'D:\Personal\Hobby\Coding\kosmos\apps\dashboard'; node D:\Personal\Hobby\Coding\kosmos\node_modules\.bun\vite@8.0.8+cac867131b98ca75\node_modules\vite\bin\vite.js build --configLoader native; Pop-Location`
- `FAIL (environment)` `Push-Location 'D:\Personal\Hobby\Coding\kosmos\apps\dashboard'; node D:\Personal\Hobby\Coding\kosmos\node_modules\.bun\playwright@1.58.2\node_modules\playwright\cli.js test; Pop-Location`

## Inference
- The root cause was that `DesktopContentSurface` was rendered inside a non-flex `.kosmos-desktop-chrome__content`, so its `flex: 1` never constrained height. With `DesktopChrome` fixed and the surface owning `overflow-y: auto`, I infer this is the correct fix for the user-reported DOM structure, but I could not prove it end-to-end inside this sandbox because Electron/Playwright worker spawning is blocked externally.
