# Autoupdater nesting cleanup verification spec

Objective: verify the current autoupdater nesting cleanup in `platform/desktop/electron/autoupdater-host.ts`.

Success criteria:

- `scheduleNativeInstallFallback` is used from `autoUpdater.on("update-downloaded")`.
- No duplicate inline `setTimeout` fallback remains in that handler.
- Helper strings are valid Cyrillic.
- `rtk err bun run --cwd platform/desktop build:js:shell` passes.
- `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-autoupdater-nesting-cleanup.json"` is produced.
- The target `DEEP_NESTING` finding for `platform\\desktop\\electron\\autoupdater-host.ts` is cleared in the after scan.
