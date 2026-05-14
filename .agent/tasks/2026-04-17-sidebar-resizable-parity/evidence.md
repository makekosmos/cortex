# Evidence: Restore ResizableSidebar behavior in `Sidebar`

## Summary

- Restored built-in sidebar wrapper/resize styling in `Sidebar.vue` by importing `./sidebar.css`.
- Restored the optional `dragRegion` prop in `Sidebar.vue` and applied `WebkitAppRegion: "drag"` to the sidebar content wrapper.
- Revalidated consumer compatibility with fresh TypeScript checks and a fresh `apps/dashboard` production build.

## Acceptance Criteria

### AC1

`Sidebar.vue` includes the base resizable-sidebar wrapper/handle styling required for its resize/collapse behavior without requiring each consumer to import `components/sidebar.css` manually.

Status: PASS

Proof:
- [`packages/kosmos-visuals/components/Sidebar.vue`](/D:/Personal/Hobby/Coding/kosmos/packages/kosmos-visuals/components/Sidebar.vue) now performs a side-effect import of `./sidebar.css`.
- Fresh dashboard build output contains the wrapper and resize-handle CSS classes from `sidebar.css`:
  - `.kosmos-sidebar-wrapper`
  - `.kosmos-sidebar-resize-handle`
  - `body.sidebar-resizing`
- Raw artifact: [dashboard-dist-css-grep.txt](/D:/Personal/Hobby/Coding/kosmos/.agent/tasks/2026-04-17-sidebar-resizable-parity/raw/dashboard-dist-css-grep.txt)

### AC2

`Sidebar.vue` restores the old optional `dragRegion` behavior from `ResizableSidebar`, so consumers can opt into `-webkit-app-region: drag` on the sidebar surface while interactive descendants remain clickable.

Status: PASS

Proof:
- [`packages/kosmos-visuals/components/Sidebar.vue`](/D:/Personal/Hobby/Coding/kosmos/packages/kosmos-visuals/components/Sidebar.vue) now exposes `dragRegion?: boolean` with default `false`.
- The component computes `contentStyle` and applies `WebkitAppRegion: "drag"` to `.kosmos-sidebar-content` when `dragRegion` is enabled.
- The built dashboard bundle contains the compiled `WebkitAppRegion` branch and the no-drag interactive selectors from `sidebar.css`.
- Raw artifacts:
  - [dashboard-dist-js-grep.txt](/D:/Personal/Hobby/Coding/kosmos/.agent/tasks/2026-04-17-sidebar-resizable-parity/raw/dashboard-dist-js-grep.txt)
  - [dashboard-dist-css-grep.txt](/D:/Personal/Hobby/Coding/kosmos/.agent/tasks/2026-04-17-sidebar-resizable-parity/raw/dashboard-dist-css-grep.txt)

### AC3

Existing `Sidebar` usage remains compatible, and the dashboard sidebar resize flow still verifies successfully against the current codebase.

Status: PASS

Proof:
- Fresh consumer type checks passed for both current users of `@kosmos/visuals` sidebar:
  - [dashboard-tsc.txt](/D:/Personal/Hobby/Coding/kosmos/.agent/tasks/2026-04-17-sidebar-resizable-parity/raw/dashboard-tsc.txt)
  - [delphi-tsc.txt](/D:/Personal/Hobby/Coding/kosmos/.agent/tasks/2026-04-17-sidebar-resizable-parity/raw/delphi-tsc.txt)
- Fresh `apps/dashboard` production build passed on the current codebase and emitted updated assets containing the restored sidebar code paths:
  - [dashboard-vite-build.txt](/D:/Personal/Hobby/Coding/kosmos/.agent/tasks/2026-04-17-sidebar-resizable-parity/raw/dashboard-vite-build.txt)
  - [dashboard-dist-css-grep.txt](/D:/Personal/Hobby/Coding/kosmos/.agent/tasks/2026-04-17-sidebar-resizable-parity/raw/dashboard-dist-css-grep.txt)
  - [dashboard-dist-js-grep.txt](/D:/Personal/Hobby/Coding/kosmos/.agent/tasks/2026-04-17-sidebar-resizable-parity/raw/dashboard-dist-js-grep.txt)
- The current dashboard shell still wires the same sidebar resize/toggle contract through `SidebarConfig`, `hidden`, `configChange`, and `update:hidden`.

Additional note:
- A direct Electron smoke execution was attempted against the fresh build, but the environment blocks GUI process spawn with `electron.launch: spawn EPERM`.
- Raw artifact: [dashboard-runtime-smoke.txt](/D:/Personal/Hobby/Coding/kosmos/.agent/tasks/2026-04-17-sidebar-resizable-parity/raw/dashboard-runtime-smoke.txt)

## Checks Run

1. `bun run --cwd packages/kosmos-visuals format`
2. `apps/dashboard/node_modules/.bin/tsc.exe --noEmit -p apps/dashboard/tsconfig.json`
3. `apps/dashboard/node_modules/.bin/tsc.exe --noEmit -p apps/delphi/ts/tsconfig.json`
4. `Push-Location apps/dashboard; vite build --configLoader native`
5. `Push-Location apps/dashboard; node --experimental-strip-types scripts/runE2ESmoke.ts` (blocked by `spawn EPERM`)
