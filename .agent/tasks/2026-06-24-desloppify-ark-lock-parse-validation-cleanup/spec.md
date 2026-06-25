# Desloppify ARK Lock Parse Validation Cleanup

## Classification

FULL_LOOP. This touches `@kosmos/ark` bootstrap code, so ARK write-boundary and forbidden ARK docs were loaded first.

## Goal

Remove the final `JSON_PARSE_CAST` finding in `core/ark/packages/ark/src/ensure-kepler.ts` without changing ARK write behavior or backend launch semantics.

## Change

- Added `KeplerLockInfo` and protocol-version shape guards.
- `readLockIfAlive` now parses lock-file JSON as `unknown` and returns only validated lock info.
- Kept stale lock cleanup, PID liveness check, and backend launch behavior unchanged.
- Fixed two autostart test fixture paths that tripped `ark:guard:writes` by avoiding direct forbidden `path.join(..., "Kosmos", ...)` literals while preserving the same fixture paths.

## Verification

- PASS: `rtk err bun test core/ark/packages/ark/tests/ensure-kepler.test.ts`
- PASS: `rtk err bun test platform/desktop/electron/settings-autostart.test.ts`
- PASS: `rtk err bun run ark:guard:writes`
- PASS: `rtk err bun run --cwd platform/desktop typecheck`
- PASS/expected failure status: `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-ark-lock-parse-validation-cleanup.json"`

The scan exits 1 because repository findings remain, but the targeted finding disappeared and all `JSON_PARSE_CAST` findings are now cleared.
