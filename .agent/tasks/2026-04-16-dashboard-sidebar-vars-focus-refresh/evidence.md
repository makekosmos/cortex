# Evidence

## Verification summary

- AC1 `PASS`: Sidebar button, toggle, divider, section label, and project-link states in `packages/kosmos-visuals` now consume `--sidebar-*` variables directly instead of mixing generic dashboard/local tokens.
- AC2 `PASS`: `apps/dashboard/src/composables/useDashboardData.ts` now starts a `5000 ms` refresh interval only when the window is focused and visible, and stops polling on blur or hidden visibility.
- AC3 `PASS`: Dashboard renderer typecheck and build both passed after the sidebar-token and polling changes.

## Commands

```powershell
Set-Location D:\Personal\Hobby\Coding\kosmos\apps\dashboard; .\node_modules\.bin\tsc.exe
Set-Location D:\Personal\Hobby\Coding\kosmos\apps\dashboard; .\node_modules\.bin\vite.exe build --configLoader native
```

## Results

- `tsc.exe`: passed with exit code `0`
- `vite build --configLoader native`: passed with exit code `0`

## Code references

- Sidebar token usage:
  - `packages/kosmos-visuals/components/SidebarButton.vue`
  - `packages/kosmos-visuals/components/Sidebar.vue`
- Focus-only polling:
  - `apps/dashboard/src/composables/useDashboardData.ts`
