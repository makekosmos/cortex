# Evidence

## Acceptance Criteria

- AC1: PASS
  - `apps/delphi/ts/src/App.vue` now mounts `DesktopChrome` and wraps routed content in `DesktopContentSurface`.
  - The old fixed connection popover was replaced by a shared `StatusDot` in the titlebar.

- AC2: PASS
  - `apps/eden/ts/src/App.vue` now mounts `DesktopChrome` for the non-zen shell and wraps primary content in `DesktopContentSurface`.
  - The old inline titlebar/sidebar/content layout remains disabled as legacy markup.

- AC3: PASS
  - Delphi sidebar hidden state still flows through `useSidebarState`.
  - Eden sidebar hidden state still flows through `layout.widgetSidebarHidden`, and `EdenSidebar.vue` now disables its built-in toggle/inset so the shared titlebar owns that behavior.

- AC4: PASS
  - `apps/delphi/ts/electron/main.ts` now keeps macOS on `hiddenInset` and configures Windows native overlay controls for the shared titlebar.
  - `apps/eden/ts/main/main.ts` now keeps macOS on `hiddenInset` and configures Windows native overlay controls for the shared titlebar.

- AC5: PASS
  - `apps/delphi/AGENTS.md` documents the shared `DesktopChrome` / `DesktopContentSurface` contract for the TS desktop shell.
  - `apps/eden/ts/AGENTS.md` documents the shared chrome contract and warns against reintroducing manual titlebar offset hacks.

- AC6: FAIL
  - `tsc --noEmit` passes for both `apps/delphi/ts` and `apps/eden/ts`.
  - `vite build` does not complete in this sandboxed environment. The failure happens before app-level bundling, inside dependency/config resolution (`spawn EPERM` in Vite externalize-deps on Windows; Delphi also hits Tailwind oxide native dependency loading).

## Commands

- PASS: `node D:\Personal\Hobby\Coding\kosmos\node_modules\.bun\typescript@5.8.3\node_modules\typescript\lib\tsc.js --noEmit -p D:\Personal\Hobby\Coding\kosmos\apps\delphi\ts\tsconfig.json`
- PASS: `node D:\Personal\Hobby\Coding\kosmos\node_modules\.bun\typescript@5.8.3\node_modules\typescript\lib\tsc.js --noEmit -p D:\Personal\Hobby\Coding\kosmos\apps\eden\ts\tsconfig.json`
- FAIL: `bun run build` in `apps/delphi/ts`
- FAIL: `bun run build` in `apps/eden/ts`

## Notes

- The build failures are environment-level in this session, not template/typecheck failures in the modified Vue files.
- Agent work was split across Delphi Electron/docs, Eden Electron/docs, and Eden sidebar adapter, then reviewed locally before verification.
