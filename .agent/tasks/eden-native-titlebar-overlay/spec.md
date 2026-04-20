# Task: Eden Native Titlebar Overlay

## Goal
Align `apps/eden/ts` with the native Windows titlebar overlay strategy already used in `apps/delphi/ts`, so overlay sizing is owned by Electron/Windows instead of manual height calculations.

## Scope
- `apps/eden/ts/main/main.ts`
- `apps/eden/ts/src/Titlebar.css`
- verification artifacts in `.agent/tasks/eden-native-titlebar-overlay/`

## Acceptance Criteria
- AC1: `apps/eden/ts` no longer sets a custom `titleBarOverlay.height` in the BrowserWindow config or via `setTitleBarOverlay()`.
- AC2: Eden's local loading/setup titlebar uses system `titlebar-area-*` env variables for its vertical sizing instead of a fixed overlay height assumption.
- AC3: The change does not reintroduce renderer-to-main overlay height synchronization logic.
- AC4: TypeScript verification for `apps/eden/ts` passes after the change.
