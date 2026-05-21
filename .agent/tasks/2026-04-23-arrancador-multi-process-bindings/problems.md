# Problems And Fixes

## Resolved

### P1: One game was still modeled as one executable

- Added persistent `game_process_bindings` with support for both `exe_path` and `process_name`.
- Wired validation so one binding cannot silently attach to multiple games.

### P2: Ark usage attribution only matched the primary executable

- Reworked Ark usage hydration and playtime range aggregation to build a binding index from both primary paths and explicit bindings.
- Usage rows now prefer exact executable-path matches and fall back to process-name bindings when path data is missing.

### P3: Runtime process operations only acted on the primary executable

- Games service now resolves all bound matches and uses them for running-instance counting and process termination.

### P4: UI had no non-drag-and-drop way to attach usage-tracker processes

- Added a game-detail bindings section and picker modal.
- The picker shows 10 recent processes by default, debounces search, and supports multi-select add.
- Fixed picker state so selections survive when the result list changes after a search.

### P5: E2E runner failed immediately when Playwright could not fork workers

- Added a fallback inline E2E runner in `scripts/run-e2e-inline.ts`.
- `scripts/run-e2e.ts` now retries through that inline path when `playwright test` cannot start normally.

## Remaining External Blocker

### B1: This environment denies Playwright browser launch

- Even after bypassing the normal Playwright worker runner, Chromium launch fails with `EPERM`.
- This is not an Arrancador application error; it is an execution-policy restriction of the current environment.
- Raw evidence: `raw/test-e2e-final.txt`.
