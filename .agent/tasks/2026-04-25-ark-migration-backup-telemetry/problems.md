# Problems

## P1. Delphi Playwright Electron launch is blocked before `app.ready`

Status: unresolved external/runtime blocker.

`bun run --cwd apps/delphi/ts e2e` launches Electron with Playwright on an isolated temporary profile and test ARK DB, but the Electron process exits before `app.ready` with native `STATUS_BREAKPOINT` (`exitCode=2147483651`). The startup trace proves Delphi main code loads and reaches `before-whenReady`; no JavaScript exception is emitted.

Mitigations attempted:

- Moved legacy space-file migration out of top-level module load and into `app.whenReady()`.
- Kept a module-level `mainWindow` reference.
- Made Playwright launch use visible window mode instead of background-hidden mode.
- Added startup guard and test-only startup trace.
- Added Electron launch switches: `--no-sandbox`, `--disable-gpu`, `--disable-software-rasterizer`.
- Made Playwright-mode `uncaughtException` log and quit instead of rethrowing.

Current impact:

- Unit/integration migration coverage passes.
- Delphi and Eden production builds pass.
- The Electron Playwright smoke path remains blocked before renderer UI can be inspected.
