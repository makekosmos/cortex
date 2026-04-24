# Evidence

## Summary

The original Electron packaging crash caused by `ajv`/`ajv-keywords` resolution is no longer present after repairing the local Bun junctions. The default Delphi build script was also corrected to choose valid Electron targets for the current platform instead of always requesting both macOS and Windows packaging.

## Acceptance Criteria

- AC1: PASS
  - `apps/delphi/ts/package.json` no longer invokes `electron-builder --mac --win` from the default `build` script.
  - The script now calls `node scripts/run-electron-builder.mjs`, which selects targets by `process.platform`.

- AC2: PASS
  - `apps/delphi/ts/scripts/run-electron-builder.mjs` maps:
    - `win32 -> --win`
    - `darwin -> --mac`
    - `linux -> --linux`
  - On this Windows environment, packaging proceeds with `platform=win32`.

- AC3: PASS
  - Windows packaging config now sets `toolsets.winCodeSign` to `1.1.0`.
  - `win.signAndEditExecutable` is set to `false`, so unsigned Windows builds no longer rely on the legacy editable-signing path.
  - The previously reported legacy `winCodeSign-2.6.0.7z` symlink-extraction failure is not present in the fresh verification log.

- AC4: PASS
  - `apps/delphi/ts/package.json` now packages the existing Windows sidecar binary from `sidecar/target/release/delphi-db.exe`.
  - It is copied to `ark-core/ark-core-rpc.exe`, which matches the packaged runtime lookup in `apps/delphi/ts/electron/sidecar.ts`.

- AC5: BLOCKED
  - `bun run build` now passes all TypeScript and Vite stages and reaches Windows packaging without the original `winCodeSign` failure.
  - Verification stops at `app-builder.exe` execution with `spawn EPERM` in this sandboxed environment.
  - Raw log: `.agent/tasks/2026-04-23-delphi-build-fix/raw/build-after-win-codesign-fix.log`

## Raw Artifacts

- Resolution check: `.agent/tasks/2026-04-23-delphi-build-fix/raw/resolution.json`
- Build log: `.agent/tasks/2026-04-23-delphi-build-fix/raw/build.log`
- Updated build log after winCodeSign fix: `.agent/tasks/2026-04-23-delphi-build-fix/raw/build-after-win-codesign-fix.log`
