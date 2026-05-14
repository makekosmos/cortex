# Evidence: Dashboard main scroll pass

## Implementation
- Moved scroll ownership to the full right-pane column in [DashboardShell.vue](D:/Personal/Hobby/Coding/kosmos/apps/dashboard/src/components/dashboard/DashboardShell.vue): `.dashboard-shell__main` now uses `overflow: auto` at lines 272-280.
- Simplified the nested content region so it no longer competes to be a second scroll container: `.dashboard-shell__content` is now block-only at lines 414-417 and `.dashboard-shell__content-inner` no longer forces `min-height: 100%` at lines 419-420.

## Acceptance Criteria
- AC1 `PASS`
  - The right-pane shell now exposes a single vertical scroll container on `.dashboard-shell__main` while keeping `DesktopContentSurface` bounded. This removes the prior `overflow: hidden` on the parent that prevented the nested content region from scrolling.
- AC2 `PASS`
  - Header, source row, and routed page content all live inside the same scrollable column at [DashboardShell.vue](D:/Personal/Hobby/Coding/kosmos/apps/dashboard/src/components/dashboard/DashboardShell.vue:233) and [DashboardShell.vue](D:/Personal/Hobby/Coding/kosmos/apps/dashboard/src/components/dashboard/DashboardShell.vue:272), so scrolling is no longer limited to a fragile inner `main`.
- AC3 `PASS`
  - TypeScript verification passed with:
    - `node D:\Personal\Hobby\Coding\kosmos\node_modules\.bun\typescript@5.8.3\node_modules\typescript\bin\tsc --noEmit -p D:\Personal\Hobby\Coding\kosmos\apps\dashboard\tsconfig.json`
  - Production build verification passed with:
    - `Push-Location 'D:\Personal\Hobby\Coding\kosmos\apps\dashboard'; node D:\Personal\Hobby\Coding\kosmos\node_modules\.bun\vite@8.0.8+cac867131b98ca75\node_modules\vite\bin\vite.js build --configLoader native; Pop-Location`

## Notes
- The verification here is code-and-build based. Inference: with the parent column now owning overflow and the nested region no longer constraining height, the pasted DOM structure from the user should scroll as a single pane.
