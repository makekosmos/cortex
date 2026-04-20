# Evidence: Titlebar Overlay Adaptive Height

## Summary
- Added renderer-side titlebar height measurement in the shared `Titlebar` component.
- Added main-process IPC handling to update Windows `titleBarOverlay.height` from the measured titlebar height.
- Reapply logic now derives overlay height from the stored renderer height and current zoom factor, so caption buttons can scale vertically with the effective titlebar height.

## Acceptance Criteria

### AC1
The shared `Titlebar` component measures its rendered height on Windows Electron and sends height updates to the main process.

Result: PASS

Evidence:
- `packages/kepler-visuals/components/Titlebar.vue` now:
  - uses `useTemplateRef("root")`
  - observes the header with `ResizeObserver`
  - invokes `window.electronAPI.invoke("titlebar:setOverlayHeight", measuredHeight)`
- Raw artifact: `raw/adaptive-titlebar-diff.txt`

### AC2
Electron main process updates `titleBarOverlay.height` using the latest renderer-measured titlebar height and reapplies it on zoom changes.

Result: PASS

Evidence:
- `apps/delphi/ts/electron/main.ts` now:
  - stores per-window titlebar heights in `WeakMap<BrowserWindow, number>`
  - computes overlay height from stored CSS height and `webContents.getZoomFactor()`
  - handles IPC `titlebar:setOverlayHeight`
  - reapplies on `zoom-changed`
- Raw artifact: `raw/adaptive-titlebar-diff.txt`

### AC3
TypeScript verification for `apps/delphi/ts` passes after the change.

Result: PASS

Evidence:
- Command: `bunx tsc --noEmit`
- Result: success, exit code `0`
- Raw artifact: `raw/tsc-noemit.txt`

## Residual Risk
- Runtime visual verification of the native Windows caption buttons was not possible inside the sandboxed environment, so this task is verified by code path inspection and TypeScript validation rather than by a live Electron window check.
