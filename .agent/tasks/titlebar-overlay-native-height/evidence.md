# Evidence: Titlebar Overlay Native Height

## Summary
- Removed renderer-driven `titleBarOverlay.height` synchronization.
- Removed explicit custom overlay height from Electron main.
- Windows native window controls now rely on Electron/OS default sizing instead of app-side height calculations.

## Acceptance Criteria

### AC1
Shared `Titlebar` no longer measures and sends overlay height to Electron main.

Result: PASS

Evidence:
- `packages/kosmos-visuals/components/Titlebar.vue` no longer contains `ResizeObserver`, `useTemplateRef`, or `titlebar:setOverlayHeight` IPC calls.
- Raw artifact: `raw/native-titlebar-diff.txt`

### AC2
Electron main no longer computes or sets a custom `titleBarOverlay.height`, relying on native/system overlay sizing instead.

Result: PASS

Evidence:
- `apps/delphi/ts/electron/main.ts` no longer sets `height` in `setTitleBarOverlay(...)`
- Windows `BrowserWindow` constructor no longer provides `titleBarOverlay.height`
- Raw artifact: `raw/native-titlebar-diff.txt`

### AC3
TypeScript verification for `apps/delphi/ts` passes after the change.

Result: PASS

Evidence:
- Command: `bunx tsc --noEmit`
- Result: success, exit code `0`
- Raw artifact: `raw/tsc-noemit.txt`

## Notes
- This is the smallest safe fix after the custom adaptive-height approach proved ineffective in runtime behavior.
- Native caption-button sizing is now delegated back to Electron/Windows.
