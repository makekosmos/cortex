# Evidence - Arrancador shell aligned to Delphi desktop chrome

Verdict: PASS

## Summary
- `arrancador` desktop shell previously diverged from `delphi/ts` in two key ways: the titlebar used a centered title plus explicit window buttons, and the sidebar used collapse semantics instead of Delphi's persisted `hidden + width` config model.
- The current codebase now follows the Delphi shell contract in React: titlebar-leading uses sidebar toggle plus history controls, desktop sidebar uses persisted hidden/width config with resize, and the main surface uses the same shell composition pattern.
- App-specific route items remain `arrancador`-specific by design; the alignment work targeted shell behavior and composition, not Delphi's task-domain navigation.

## Acceptance criteria
### AC1
PASS
- Desktop titlebar now uses the Delphi-like leading composition in [AppTitlebar.tsx](</D:/Personal/Hobby/Coding/kosmos/apps/arrancador/src/components/AppTitlebar.tsx:16>) with toggle + history controls from [TitlebarHistoryControls.tsx](</D:/Personal/Hobby/Coding/kosmos/apps/arrancador/src/components/TitlebarHistoryControls.tsx:1>).
- The old centered titlebar identity/title layout is no longer used.

### AC2
PASS
- Desktop sidebar now persists `{ width, hidden }` under `arrancador-sidebar-config` in [Sidebar.tsx](</D:/Personal/Hobby/Coding/kosmos/apps/arrancador/src/components/Sidebar.tsx:17>).
- Hidden mode is the primary desktop toggle state, and the resize handle / keyboard shortcut behavior are implemented in [Sidebar.tsx](</D:/Personal/Hobby/Coding/kosmos/apps/arrancador/src/components/Sidebar.tsx:119>).

### AC3
PASS
- Layout now composes titlebar, sidebar, and content surface in the Delphi shell pattern in [Layout.tsx](</D:/Personal/Hobby/Coding/kosmos/apps/arrancador/src/pages/Layout.tsx:155>).
- The content surface border/radius adjust based on sidebar visibility in [Layout.tsx](</D:/Personal/Hobby/Coding/kosmos/apps/arrancador/src/pages/Layout.tsx:235>) with shared shell styles in [index.css](</D:/Personal/Hobby/Coding/kosmos/apps/arrancador/src/index.css:307>).
- Mobile navigation remains available through the separate mobile top bar and overlay sidebar in [Layout.tsx](</D:/Personal/Hobby/Coding/kosmos/apps/arrancador/src/pages/Layout.tsx:183>).

### AC4
PASS
- Electron native window options now align more closely with Delphi's contract in [windows.ts](</D:/Personal/Hobby/Coding/kosmos/apps/arrancador/electron/main/windows.ts:74>).
- Windows uses `frame: false` with `titleBarOverlay`, while macOS keeps `hiddenInset`; Linux is no longer forced into the Windows-style custom frame path.

### AC5
PASS
- Regression coverage was updated for the new shell behavior:
  - [app-titlebar.test.tsx](</D:/Personal/Hobby/Coding/kosmos/apps/arrancador/src/test/app-titlebar.test.tsx:1>)
  - [sidebar-component.test.tsx](</D:/Personal/Hobby/Coding/kosmos/apps/arrancador/src/test/sidebar-component.test.tsx:1>)
  - [layout.test.tsx](</D:/Personal/Hobby/Coding/kosmos/apps/arrancador/src/test/layout.test.tsx:1>)
- Fresh verification on the current codebase passed:
  - `bun run typecheck`
  - `bun run test`
  - `bun run build:main`
  - `bun run build:preload`
  - `bun run build:renderer`

## Raw artifacts
- [typecheck.txt](</D:/Personal/Hobby/Coding/kosmos/.agent/tasks/2026-04-18-arrancador-delphi-shell/raw/typecheck.txt>)
- [test.txt](</D:/Personal/Hobby/Coding/kosmos/.agent/tasks/2026-04-18-arrancador-delphi-shell/raw/test.txt>)
- [build-main.txt](</D:/Personal/Hobby/Coding/kosmos/.agent/tasks/2026-04-18-arrancador-delphi-shell/raw/build-main.txt>)
- [build-preload.txt](</D:/Personal/Hobby/Coding/kosmos/.agent/tasks/2026-04-18-arrancador-delphi-shell/raw/build-preload.txt>)
- [build-renderer.txt](</D:/Personal/Hobby/Coding/kosmos/.agent/tasks/2026-04-18-arrancador-delphi-shell/raw/build-renderer.txt>)
- [delphi-alignment.txt](</D:/Personal/Hobby/Coding/kosmos/.agent/tasks/2026-04-18-arrancador-delphi-shell/raw/delphi-alignment.txt>)
