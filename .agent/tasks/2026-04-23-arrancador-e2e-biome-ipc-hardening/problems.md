# Problems And Fixes

## Resolved

### P1: Playwright webServer command could not spawn shell-composed command

- `bun run test:e2e` initially failed at `webServer.command` with `spawn EPERM` for `bun run build && bun run preview`.
- Replaced package `test:e2e` with a Bun orchestration script that builds, starts Vite preview, waits for the preview URL, and then runs Playwright with an external server.

### P2: E2E runner could accidentally test against a stale preview server

- A previous preview process on port `4174` caused the new preview process to exit while Playwright still connected to the stale server.
- The E2E runner now allocates a free local port for each run and passes it to Playwright through `ARRANCADOR_E2E_BASE_URL`.
- The runner also treats early preview exit as a setup failure instead of continuing against an unrelated server.

### P3: E2E tests still targeted removed React/Tauri-era UI

- Smoke tests used non-hash routes and stale selectors.
- Catalogue tests expected the old local catalogue CRUD page, while the active Vue page is now RAWG showcase/search/add-to-library.
- Updated E2E tests, the Electron bridge mock, and stable Vue `data-testid` anchors for the current app behavior.

### P4: E2E bridge mock did not match the Electron bridge surface

- The app now calls `window.arrancador.commands.<channel>()`, while the mock still approximated older generic invocation behavior.
- Updated the mock to expose the commands surface, emit event payloads like the Electron preload bridge, and implement the RAWG/search/add-game handlers needed by the current Vue flows.

### P5: Biome warning noise hid useful signal

- Baseline: `bun run biome:check -- --max-diagnostics=500` reported 310 warnings.
- Fixed actionable warnings in active code and added targeted Biome overrides for Vue template false positives and global CSS specificity noise.
- Result: `bun run biome:check` reports zero diagnostics.

### P6: Renderer IPC exposed a generic runtime invoke bridge

- Replaced the renderer-facing `window.arrancador.invoke(...)` bridge with explicit `window.arrancador.commands.<channel>(payload?)` methods.
- Added runtime channel/event allowlists and payload validation for sensitive handlers such as shell/path/dialog/scan/autostart/settings/disk-speed calls.
- Kept `src/lib/ipc.ts` as an internal typed adapter so feature APIs did not need broad rewrites.
