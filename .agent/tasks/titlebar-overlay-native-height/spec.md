# Task: Titlebar Overlay Native Height

## Goal
Remove custom runtime height calculation for Windows `titleBarOverlay` in Delphi TS and let Electron use the standard system height for native window controls.

## Scope
- `packages/kepler-visuals/components/Titlebar.vue`
- `apps/delphi/ts/electron/main.ts`
- verification artifacts in `.agent/tasks/titlebar-overlay-native-height/`

## Acceptance Criteria
- AC1: Shared `Titlebar` no longer measures and sends overlay height to Electron main.
- AC2: Electron main no longer computes or sets a custom `titleBarOverlay.height`, relying on native/system overlay sizing instead.
- AC3: TypeScript verification for `apps/delphi/ts` passes after the change.

## Source of Truth
- Electron docs: when `titleBarOverlay.height` is omitted, window controls default to the standard system height.
