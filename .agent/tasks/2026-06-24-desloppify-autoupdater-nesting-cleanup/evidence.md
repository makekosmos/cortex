# Autoupdater nesting cleanup verification

- Objective: verify the worker edit in `platform/desktop/electron/autoupdater-host.ts`.
- Handler check: `autoUpdater.on("update-downloaded")` now calls `scheduleNativeInstallFallback(info.version)`.
- Inline fallback check: no duplicate inline `setTimeout` fallback remains in that handler.
- String check: the fallback helper strings are valid Cyrillic, not mojibake.
- Build: `rtk err bun run --cwd platform/desktop build:js:shell` passed with Vite warnings only.
- Scan: `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-autoupdater-nesting-cleanup.json"` exited 1 as expected.
- Delta: `DEEP_NESTING` for `platform\\desktop\\electron\\autoupdater-host.ts` went from 1 to 0.
- Overall scan: score stayed 9; findings went from 380 to 379; high went from 223 to 222; medium and low stayed 113 and 44.
