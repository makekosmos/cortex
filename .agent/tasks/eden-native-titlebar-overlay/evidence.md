# Evidence: Eden Native Titlebar Overlay

## Verification Summary
- AC1: PASS — `apps/eden/ts/main/main.ts` no longer sets `titleBarOverlay.height` in the BrowserWindow options or in `setTitleBarOverlay()`.
- AC2: PASS — `apps/eden/ts/src/Titlebar.css` now sizes the local loading/setup titlebar from `env(titlebar-area-y, ...)` and `env(titlebar-area-height, ...)`.
- AC3: PASS — no renderer-to-main overlay height synchronization was added; the change only removes manual height handling and imports the local CSS explicitly.
- AC4: PASS — `bunx tsc --noEmit` completed successfully in `apps/eden/ts`.

## Commands
- `bunx tsc --noEmit`

## Raw Artifacts
- `raw/eden-titlebar-diff.txt`
- `raw/verification-summary.txt`
- `raw/tsc-noemit.txt`
