# Task: Titlebar Overlay Adaptive Height

## Goal

Make Electron window controls in Delphi TS adapt vertically to the effective titlebar height on Windows by updating `titleBarOverlay.height` from the renderer's measured titlebar height.

## Scope

- `packages/kosmos-visuals/components/Titlebar.vue`
- `apps/delphi/ts/electron/main.ts`
- verification artifacts in `.agent/tasks/titlebar-overlay-adaptive-height/`

## Acceptance Criteria

- AC1: The shared `Titlebar` component measures its rendered height on Windows Electron and sends height updates to the main process.
- AC2: Electron main process updates `titleBarOverlay.height` using the latest renderer-measured titlebar height and reapplies it on zoom changes.
- AC3: TypeScript verification for `apps/delphi/ts` passes after the change.

## Notes

- Renderer CSS alone cannot resize the native caption buttons.
- The actual control height must be updated via Electron `titleBarOverlay.height`.
