# Command Results

## PASS

- `bun run --cwd apps/eden/ts test:ark-migration`
  - Result: PASS.
  - Output included: `{"status":"ok","objectTypes":["note_obj","research_note"],"objects":["note-a","note-b"],"links":["note-a:related:note-b"],"failureStatus":"partial_failure"}`.
- `bun run --cwd apps/delphi/ts test`
  - Result: PASS.
  - Output included: `Test Files 8 passed (8)`, `Tests 99 passed (99)`.
- `bun run --cwd apps/delphi/ts build`
  - Result: PASS.
  - Output included successful Vite renderer/main/preload builds, electron-builder packaging, and `cargo build --release --manifest-path ../../../packages/ark-core/rust/Cargo.toml --bin ark-core-rpc`.
- `bun run --cwd apps/eden/ts build`
  - Result: PASS.
  - Output included successful Heart release build, `ark-core-rpc` release build, TypeScript, renderer, main, and preload builds.
- `git diff --check`
  - Result: PASS.
  - Output included only existing line-ending warnings.
- `rg -n "Test database isolation|isolated test databases|main/user ARK database" AGENTS.md`
  - Result: PASS.
  - Output confirmed the root `AGENTS.md` invariant.

## BLOCKED

- `bun run --cwd apps/delphi/ts e2e`
  - Result: BLOCKED/FAIL.
  - The test uses a temporary profile and test ARK DB under `os.tmpdir()`.
  - Electron reaches Delphi main module startup and registers IPC, then the process exits before `app.ready` with native `STATUS_BREAKPOINT` (`exitCode=2147483651`).
  - Startup log:

```text
[2026-04-25T09:33:17.035Z] main-module-loaded
[2026-04-25T09:33:17.040Z] fs-ipc-registered
[2026-04-25T09:33:17.040Z] space-ipc-registered
[2026-04-25T09:33:17.040Z] db-ipc-registered
[2026-04-25T09:33:17.041Z] sidecar-events-registered
[2026-04-25T09:33:17.041Z] sync-ipc-registered
[2026-04-25T09:33:17.041Z] before-whenReady
```

  - Tried fixes/diagnostics: lazy legacy space migration, retained `mainWindow`, visible Playwright window mode, startup guard, startup trace, Playwright launch switches `--no-sandbox --disable-gpu --disable-software-rasterizer`, and softer Playwright `uncaughtException` behavior.
