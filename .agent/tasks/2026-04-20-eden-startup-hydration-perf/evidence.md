# Evidence - Eden startup hydration perf

## Scope

- `apps/eden/ts/src/store/eden.ts`
- `apps/eden/ts/src/App.vue`
- `apps/eden/ts/src/App.css`
- `.agent/tasks/2026-04-20-eden-startup-hydration-perf/spec.md`
- `.agent/tasks/2026-04-20-eden-startup-hydration-perf/evidence.json`
- `.agent/tasks/2026-04-20-eden-startup-hydration-perf/raw/tsc.txt`
- `.agent/tasks/2026-04-20-eden-startup-hydration-perf/raw/build.txt`
- `.agent/tasks/2026-04-20-eden-startup-hydration-perf/raw/dev.txt`
- `.agent/tasks/2026-04-20-eden-startup-hydration-perf/raw/playwright.txt`

## Implementation summary

- Split Eden startup into two phases in `src/store/eden.ts`:
  - `initApp()` now loads only base config needed to mount the shell.
  - `hydrateVaultData()` now performs `listEntries()` and `listNoteTypes()` asynchronously after the shell is already allowed to render.
- Added `isHydratingVault` store state so the renderer can distinguish "bootstrap not ready" from "shell ready, vault data still loading".
- Added a non-blocking `app-main-loading` state in `src/App.vue` for both normal and zen layouts.
- Updated the "open my-space automatically" watcher so it waits for vault hydration instead of racing the new background load.

## Why this addresses the startup lag

- Before the change, `isInitializing` stayed `true` until Eden had finished:
  - reading base settings
  - listing entries
  - listing note types
  - restoring or creating the default personal-space entry
- That meant the shell DOM could not mount until the entire vault hydration path completed.
- After the change, shell render is released immediately after base settings load, and vault hydration continues in the background.

## Verification

### Commands

- `node apps/eden/ts/node_modules/typescript/bin/tsc -p apps/eden/ts/tsconfig.json --noEmit`
- `bun run build` in `apps/eden/ts`
- `bun run dev` in `apps/eden/ts`
- `npx playwright test tests/app.spec.ts -g "should create a custom note type and render a typed note header" --config playwright.config.ts`

### Results

- TypeScript: PASS
- Production build: PASS
- Dev startup diagnostic: PASS as diagnostic capture
  - Vite became ready in about `458-510ms`.
  - In this Codex environment the Electron child process then failed with `spawn EPERM`, so the app window could not be launched here.
- Playwright diagnostic: PASS as diagnostic capture
  - The Playwright runner also failed here with `spawn EPERM`, so worker processes could not start in this environment.

## Acceptance mapping

- AC1: PASS - `initApp()` no longer awaits `listEntries()` and `listNoteTypes()` before clearing `isInitializing`; that work moved into `hydrateVaultData()`.
- AC2: PASS - `App.vue` now shows an in-shell loading message while `eden.isHydratingVault` is true.
- AC3: PASS - TypeScript and `bun run build` both succeeded on current code.
- AC4: PASS - raw logs capture current `bun run dev` and Playwright behavior, including the `spawn EPERM` environment blocker.

## Remaining risk

- I could not complete a real visual run of the Electron window inside this Codex environment because Electron and Playwright worker startup are blocked by `spawn EPERM`.
- Based on the code path and the successful build/typecheck, the startup fix is structurally sound, but final visual confirmation still depends on running the app in a local environment where Electron worker spawn is allowed.
