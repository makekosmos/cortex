# Delphi Build Fix

## Goal

Make `apps/delphi/ts` build reliably on Windows by removing the invalid macOS packaging request from the default build flow and preserving platform-appropriate Electron packaging.

## Acceptance Criteria

- AC1: On Windows, `bun run build` in `apps/delphi/ts` must not invoke `electron-builder --mac`.
- AC2: The default build flow must still invoke `electron-builder` with at least one valid target for the current platform.
- AC3: The Windows packaging config must not rely on the legacy `winCodeSign-2.6.0.7z` path that fails on symlink extraction in this environment.
- AC4: Packaged Windows resources must reference an existing sidecar executable path and runtime location.
- AC5: The build command must complete successfully on this Windows environment after the script change and the repaired local dependency links.
