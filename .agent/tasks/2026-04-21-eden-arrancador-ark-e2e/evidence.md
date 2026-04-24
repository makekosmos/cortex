# Evidence

## Scope

This task adds automated verification assets for the shared Ark DB flow between `Arrancador` and `Eden`.

## Files Added

- `apps/eden/ts/tests/arrancador-ark-sync.spec.ts`
- `apps/eden/ts/scripts/verifyArrancadorArkSync.ts`

## What The Test Covers

The Playwright Electron spec now:

1. creates an isolated temp root;
2. creates temp `HOME`, `APPDATA`, `LOCALAPPDATA`, vault, and Arrancador DB paths;
3. writes a temp shared `selected-space.json`;
4. launches `Eden` under that isolated environment;
5. initializes Ark built-in object types;
6. creates a game through real Arrancador services:
   - `openGameDatabase`
   - `createGamesService`
   - `createArkGameObjectService`
7. verifies that a `game_obj` row exists in `ark.db`;
8. relaunches `Eden`;
9. verifies that the synced game appears in Eden entry listing and can be opened in the editor;
10. removes the temp root in `finally`.

The standalone verifier script covers the same environment pattern but uses direct DB setup as a fallback path for local debugging.

## Verification Runs

### PASS

- `bun run build`
  - artifact: `artifacts/eden-build.txt`
- `bun x oxlint tests/arrancador-ark-sync.spec.ts scripts/verifyArrancadorArkSync.ts --deny-warnings`
  - artifact: `artifacts/oxlint.txt`

### FAIL

- `bun x playwright test arran.*sync.spec.ts --config playwright.config.ts`
  - artifact: `artifacts/playwright-runner.txt`
  - failure: `spawn EPERM` while Playwright tries to fork its worker process
- `node --experimental-strip-types scripts/verifyArrancadorArkSync.ts`
  - artifact: `artifacts/playwright-script.txt`
  - failure: `electron.launch: spawn EPERM` while Playwright tries to start Electron

## Acceptance Criteria Status

- AC1: PASS
  - The repository now contains an automated Playwright Electron test for this scenario.
- AC2: PASS
  - The test creates a fully isolated temp environment and temp shared-space selection.
- AC3: PASS
  - The test creates a game through Arrancador services and verifies `game_obj` in `ark.db`.
- AC4: PASS
  - The test implementation verifies appearance in Eden entry listing and editor open flow.
- AC5: FAIL
  - Cleanup logic is implemented, but the runtime verification run does not complete successfully in this environment because process spawning is blocked with `EPERM`.

## Encoding Check

- `spec.md` was rewritten in ASCII to avoid mojibake in proof-loop artifacts.
- The new test and script files are ASCII-only and passed `oxlint`.

## Conclusion

Implementation is in place and statically checked, but the end-to-end runtime proof is currently blocked by environment-level process spawning restrictions.
